# Trampas de Tauri v2 + Windows que costaron sangre

Ocho fallos reales encontrados montando winshotx. Ninguno da error claro: la app se
cuelga, sale en negro o no hace nada. Si vuelve a pasar algo raro con ventanas, empieza
por aquí. El número 6 es el peor de todos, porque no se ve en desarrollo.

## 1. Un comando síncrono congela toda la interfaz

`#[tauri::command] pub fn algo()` se ejecuta **en el hilo del bucle de eventos**. Si dentro
se crea una ventana, esa creación espera al bucle de eventos… que está ocupado ejecutando el
comando. Resultado: "(No responde)" en el título y ventana en blanco.

**Regla de la casa: todos los comandos son `async fn`.** Sin excepciones, aunque no
esperen nada. Ojo: un comando `async` con `State<T>` necesita el lifetime explícito
(`State<'_, AppState>`) y no puede devolver un tipo sin envolver, hay que usar `Result`.

## 2. Crear ventanas desde el hilo de un atajo global también cuelga

Mismo problema por otra puerta: el manejador de `tauri-plugin-global-shortcut` no debe abrir
ni cerrar ventanas directamente. En `recorder::stop` la apertura del editor se lanza con
`std::thread::spawn` para que ocurra en un hilo neutral.

## 3. Las etiquetas de ventana no se pueden reutilizar

Cerrar una ventana es asíncrono. Si el atajo se pulsa dos veces seguidas,
`WebviewWindowBuilder::new(app, "overlay-0", …)` falla con "ya existe una ventana con esa
etiqueta" y el overlay no vuelve a abrirse nunca. Por eso cada ronda añade un contador:
`overlay-0-7`, `recorder-8`, `editor-9`.

Consecuencia directa: las `capabilities` tienen que usar patrones (`"editor-*"`), no
etiquetas fijas, o esas ventanas se quedan sin permisos.

## 4. El primer clic sobre el overlay se lo come Windows

Una ventana que aparece sin ser la ventana activa recibe los `pointermove` (el ratón la
sobrevuela) pero el primer `pointerdown` se consume activándola: el usuario arrastra y no
pasa nada. Se arregla con `.focused(true)` en el builder más una llamada a
`SetForegroundWindow` sobre el `HWND` después de `show()`.

Y en el frontend, nada de `setPointerCapture`: durante el arrastre se escucha en `window`
(`pointermove` / `pointerup` / `pointercancel`).

## 5. Un canvas alimentado por el protocolo `asset:` queda contaminado

La lupa lee píxeles con `getImageData`. Si la imagen se carga con
`convertFileSrc()` directamente, el canvas es de otro origen y la lectura lanza
`SecurityError` en silencio: la lupa muestra siempre `#000000`. La solución es descargar el
PNG con `fetch`, pasarlo a `createImageBitmap` y dibujarlo desde el blob, que ya es del
mismo origen.

## 6. La CSP no se aplica en desarrollo, y en el instalador deja el overlay en negro

La más cara de todas. `"csp": "default-src 'self'; img-src 'self' asset: …"` parece
completa, pero `connect-src` **no está declarada**, así que hereda de `default-src`, o sea
`'self'`. El PNG de la pantalla congelada no viaja por `'self'`: viaja por el protocolo
asset, que en Windows es `http://asset.localhost`. Ese `fetch` se bloquea, el overlay se
queda sin fondo y el usuario ve un rectángulo negro tapando la pantalla entera.

Lo que lo hace tan traicionero: **en desarrollo no pasa nunca**. La página la sirve Vite,
y la CSP solo la inyecta Tauri cuando sirve los assets embebidos. Se prueba en `dev`,
funciona, se publica el instalador y ahí la app está rota.

El IPC va por el mismo camino (`fetch` a `http://ipc.localhost` en Windows), pero ese sí
tiene red de seguridad: cuando la CSP lo bloquea, Tauri cae a `postMessage` y sigue
funcionando, más lento y con un aviso en la consola. Por eso los comandos respondían
mientras el fondo no cargaba: el síntoma parecía "la imagen no se ve", no "la CSP está mal".

Lo que hay que declarar:

```json
"csp": "default-src 'self'; connect-src 'self' ipc: http://ipc.localhost asset: http://asset.localhost blob: data:; …"
```

Dos defensas más, porque una config no se compila y nadie la revisa:

- `src-tauri/tests/csp.rs` lee `tauri.conf.json` y se pone rojo si alguien recorta esas
  fuentes. Incluye la CSP antigua como caso de prueba, para demostrar que muerde.
- El overlay tiene una segunda vía: si el protocolo asset falla, pide el PNG por el
  comando `freeze_bytes` y lo pinta igual. Y si tampoco, muestra el error con una salida
  a mano y se cierra solo a los 8 segundos, en vez de dejar la pantalla secuestrada.

**Regla de la casa: lo que se toca en la CSP se prueba con `pnpm tauri build`, nunca con
`pnpm tauri dev`.** Y ojo, `cargo build --release` a secas no sirve: ese binario sigue
apuntando al servidor de desarrollo y arranca con un error de conexión.

## 7. La clave de firma se pasa por `TAURI_SIGNING_PRIVATE_KEY`, no por su ruta

El CLI documenta tres variables y solo una funciona de verdad al compilar:
`TAURI_SIGNING_PRIVATE_KEY_PATH` se ignora y el build muere al final, después de haber
generado el instalador, con «A public key has been found, but no private key». Hay que
pasarle **el contenido** del archivo:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/winshotx.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""   # la clave se generó sin contraseña
pnpm tauri build
```

Y dos cosas que hay que tener claras del actualizador:

- **La versión que se instala hoy es la que decide si mañana puede actualizarse.** Quien
  tenga la 0.1.1 no se enterará nunca de que existe la 0.1.2, porque su copia no lleva
  actualizador. El salto se da a mano una vez y ya.
- **El `latest.json` no se escribe a mano.** Lo genera `scripts/publicar.mjs` a partir del
  `.sig` que acaba de producir el bundler. Una firma vieja pegada ahí dentro no da error al
  publicar: falla en el equipo del usuario, al intentar actualizar, que es donde no lo vas a ver.

## Extra: los eventos globales tropiezan con ventanas muertas

`app.emit(...)` intenta entregar a **todas** las ventanas, incluidas las que se acaban de
cerrar, y suelta `PostMessage failed … El identificador de la ventana no es válido`. El
contador de la barra de grabación se quedaba a cero por esto. Se emite ventana a ventana
recorriendo `app.webview_windows()` y filtrando por prefijo de etiqueta.

## 8. `WebviewWindow::emit(...)` no es "emitir desde esta ventana", es "emitir a todas"

Llamarlo sobre una instancia concreta (`ventana.emit(...)`) engaña: parece que el evento sale
de esa ventana y llega a esa ventana, pero es exactamente el mismo broadcast global que
`app.emit(...)` (comparten el mismo `manager()` por debajo). Esto mordió de verdad el 26 de
agosto de 2026, cuando el overlay pasó a reutilizar una ventana por monitor en vez de crear
una nueva en cada captura (ver `windows_mgr::open_overlays` y la memoria
`winshotx-overlay-multimonitor`): al avisar a cada ventana reutilizada de que había una
captura nueva con `ventana.emit(EVENT_OVERLAY_SHOW, payload_de_ese_monitor)`, las **tres**
ventanas lo recibían, y la última emitida ganaba en las tres pantallas a la vez.

**La forma correcta es `emit_to(etiqueta, evento, payload)`**, con la etiqueta de la ventana
concreta como primer argumento, no un método distinto sobre una instancia distinta.

**Y la segunda mitad, que es la que costo de verdad: `emit_to` era necesario y NO bastaba.**
Munir seguia viendo su pantalla principal duplicada en las otras, en la vista previa y en el
recorte final. Se comprobo que no era ninguna de las sospechas obvias: los `freeze-N.bmp` en
disco eran correctos (verificado con Python/PIL), la posicion real de cada ventana coincidia con
la pedida, y Rust recortaba de la pantalla correcta (hay dos tests en `capture/mod.rs` que lo
fijan).

La causa estaba en el **otro lado**. El frontend hacia:

```ts
listen(EVENTS.overlayShow, (e) => boot(e.payload))   // <- sin target
```

`listen` sin `target` se registra como `{ kind: 'Any' }`; lo dice el propio paquete en
`@tauri-apps/api/event.d.ts`: *"The event target to listen to, defaults to `{ kind: 'Any' }`"*.
Y en `tauri`, `event/listener.rs` decide asi si un oyente recibe:

```rust
*target == EventTarget::Any || filter.as_ref().map(|f| f(target)).unwrap_or(true)
```

donde `target` es el del **oyente**: si se registro como `Any`, la funcion devuelve `true` **sin
llegar a mirar el filtro**. O sea que un oyente `Any` se salta entero cualquier `emit_to`. Las
tres ventanas recibian los tres payloads del bucle de `open_overlays` y se quedaban con el
ultimo, asi que las tres pintaban y recortaban la misma pantalla.

**El arreglo es pasar la etiqueta de la ventana al registrarse:**

```ts
listen(EVENTS.overlayShow, (e) => boot(e.payload), { target: getCurrentWindow().label })
```

Se pasa como **cadena** a proposito: una cadena se convierte en `AnyLabel`, igual que
`emit_to(label, ...)` en Rust, y `filter_target` en `manager/mod.rs` casa `AnyLabel` con
`AnyLabel` por etiqueta. Los dos lados hablan el mismo idioma sin depender de si el destino es
Window, Webview o WebviewWindow.

**La leccion general, que vale para cualquier evento de esta app:** en Tauri v2 el destino de un
evento lo deciden **los dos lados**. Acotar solo el que emite no sirve de nada si el que escucha
se apunto a todo. Y el `target` del `listen` parece opcional porque el tipo lo deja fuera; no lo
es en cuanto hay mas de una ventana con el mismo codigo dentro.

Los eventos que SI tienen que llegar a todas las ventanas (`overlayMode` y `overlayTakeScreen`,
que son como se sincroniza la barra de modos entre pantallas) se quedan sin `target` a
proposito. Ahi el broadcast es la funcion, no el fallo.

## 9. `.focused(false)` no impide que una ventana reutilizada robe el foco

La cuenta atras del temporizador existe para poder fotografiar un menu abierto: se pulsa el
atajo, se abre el menu, y a los tres segundos se congela la pantalla con el menu dentro. Si la
ventanita del numero coge el foco, el menu se cierra y la funcion no sirve para nada.

`WebviewWindowBuilder::focused(false)` **solo vale la primera vez**. Es una opcion de
construccion: le dice a Windows que no active la ventana al crearla. Pero esa ventana no se
cierra entre capturas, se esconde y se reutiliza (igual que los overlays, ver la trampa 3), y el
`show()` de la segunda vez es un `ShowWindow` normal y corriente, que activa la ventana como
activaria cualquier otra. O sea: la primera cuenta atras respeta el menu y la segunda lo cierra.

**El arreglo es un estilo extendido sobre el HWND, no una opcion del constructor:**

```rust
let actual = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
SetWindowLongPtrW(hwnd, GWL_EXSTYLE, actual | WS_EX_NOACTIVATE as isize | WS_EX_TOOLWINDOW as isize);
```

`WS_EX_NOACTIVATE` es una propiedad de la ventana, no del momento en que se muestra: Windows deja
de activarla para siempre, tambien al hacerle clic encima. Vive en
`platform::window_style::never_focus` y se aplica una sola vez, al construirla.

**Cuidado con lo que eso implica:** una ventana que nunca coge el foco tampoco recibe teclado. Por
eso durante la cuenta atras no se puede pulsar Escape para cancelar, y esta aceptado a proposito:
la alternativa seria registrar un Escape global durante esos segundos, que se lo quitaria a la
aplicacion que se esta fotografiando.

## 10. Una ventana nueva sin su etiqueta en `capabilities` se queda muda

`src-tauri/capabilities/default.json` lleva una lista de etiquetas (`main`, `overlay-*`,
`recorder-*`, `editor-*`...) y los permisos solo alcanzan a las ventanas que casan con ella. Una
ventana con una etiqueta nueva **no hereda nada**: `listen`, `emit` y las llamadas de ventana
fallan, y fallan como una promesa rechazada dentro de un `useEffect`, o sea que en la ventana no
se ve ningun error, solo una pantalla que no reacciona.

Al anadir la cuenta atras del temporizador (etiqueta `countdown`) hubo que meterla en esa lista.
**Es lo primero que hay que mirar cuando una ventana nueva no recibe sus eventos**, antes de
sospechar de la trampa 8.

## 11. El audio del sistema: cuatro trampas seguidas

Sacar el sonido de los altavoces y meterlo en el MP4 tiene cuatro sitios donde se falla sin que
nadie avise. Los cuatro salieron el 27 de agosto de 2026 al cerrar la META 2.

**El crate `windows` esconde `IMMDevice::Activate` detras de features que no se llaman como
esperas.** Con `Win32_Media_Audio` y `Win32_System_Com` puestas, el metodo sigue sin existir y el
compilador solo dice `no method named Activate found`. Hacen falta ademas
`Win32_System_Com_StructuredStorage` y `Win32_System_Variant`, porque el parametro de activacion es
un `PROPVARIANT`. Cuando falte un metodo de COM, mirar en el codigo del crate el `#[cfg(all(feature
= ...))]` que lo envuelve, en vez de adivinar.

**`WAVEFORMATEX` viene empaquetada y no deja coger referencias a sus campos.** Un `assert_eq!` sobre
dos campos suyos no compila: `error[E0793]: reference to field of packed struct is unaligned`. Se
copian a variables sueltas primero. No es un aviso, es un error, y aparece en el sitio mas tonto.

**Windows entrega el sonido en coma flotante de 32 bits y el codificador AAC quiere enteros de
16.** Nadie se queja: `send_audio_buffer` acepta los bytes igual y el MP4 sale mudo o con un
chirrido. La conversion esta en `record::audio::a_pcm16`, y recorta a [-1, 1] antes de escalar
porque una muestra por encima de uno da la vuelta al convertirla y suena a chasquido.

**Y `send_audio_buffer` IGNORA la marca de tiempo que se le pasa.** Lo dice su propio codigo
(`_timestamp: i64, // ignored to guarantee monotonic audio timing`): coloca el sonido contando las
muestras que ha recibido. Consecuencia practica: **un trozo de sonido que se pierda por el camino
no deja un hueco, desplaza todo lo que viene detras.** Por eso los buferes que Windows marca como
`SILENT` (que llegan vacios a proposito) hay que rellenarlos de ceros a mano y enviarlos igual, en
vez de saltarselos.

## 12. Una foto de la pantalla no ve la mitad de lo que dice la aplicacion

`scripts/ver-ventana.mjs` fotografia una pantalla entera y sirve para mirarla con los ojos, pero
**lo que vive en un atributo no sale en la foto**. El 27 de agosto de 2026 la barra de captura
llevaba semanas hablando espannol con la aplicacion en ingles y nadie lo habia visto: sus botones
son solo iconos, asi que todo lo que dicen esta en `title` y en `aria-label`, y ninguno de los dos
se dibuja hasta que alguien pasa el raton por encima.

Lo mismo pasa con lo que solo aparece al pasar por encima. El banco ahora manda `pointerenter` y
`pointerover` a cada antepasado del punto (el "entra" **no burbujea**, hay que mandarlo uno a uno),
pero aun asi solo ve un estado por foto.

**Lo que si lo caza:** una prueba que monta la pantalla en ingles y comprueba que no queda ni una
frase espannola en `document.body.textContent` ni en los `aria-label`. Estan en
`SettingsApp.test.tsx`, `ModeBar.test.tsx` y `FrameStrip.test.tsx`.

Y la regla que salio de montarlas: **una prueba que nunca se ha visto roja no ha probado nada.**
Las dos de idioma se estrenaron rompiendo la traduccion a mano, viendolas fallar, y devolviendo el
codigo a su sitio.

## 13. Un OCR no lee letras dibujadas a mano

La primera prueba del lector de texto dibujaba «HI» con rectangulos negros gordos sobre blanco, que
para una persona se lee sin dudar. `Windows.Media.Ocr` devolvio **cadena vacia**, y durante un rato
parecio que el motor no funcionaba.

No era el motor: un OCR esta entrenado con letras de verdad, con sus curvas y su grosor variable, y
unas barras rectas no se le parecen a ninguna. La prueba buena **dibuja el texto con GDI usando la
fuente del sistema** (`CreateFontW` + `TextOutW` sobre un `CreateDIBSection`), que ademas es lo mas
parecido a la captura que va a leer de verdad. Con eso lee `"winshotx lee texto"` a la primera.

Dos detalles de GDI que cuestan un rato: el `biHeight` del `BITMAPINFOHEADER` va **negativo** para
que las filas salgan de arriba abajo, que es como las quiere `image`, y los pixeles del DIB vienen
en **BGRA**, no en RGBA.

Y una del crate `windows` 0.62: para esperar a una operacion de WinRT el metodo es **`.join()`**, no
`.get()`. Es un metodo inherente de `IAsyncOperation`, asi que no hace falta importar ningun trait
(el trait `Async` que sugiere el compilador es privado y no se puede importar).

## 14. Contar pixeles tocados no es medir un trazo

Las anotaciones del editor salieron con el trazo de **un pixel de ancho** y trece pruebas en
verde. Ninguna lo vio porque todas contaban lo mismo: cuantos pixeles habia dejado de ser
blancos. Un rectangulo de un pixel de grosor toca cientos de pixeles, asi que la cuenta salia
alta y la prueba pasaba.

La causa estaba en el suavizado del disco con el que se engorda el trazo:

```rust
// Mal: a distancia = radio, el borde vale exactamente 0 y no se pinta nada.
let borde = (radio as f32 - distancia).min(1.0);

// Bien: se mide hasta medio pixel POR FUERA del radio.
let borde = (radio + 0.5 - distancia).clamp(0.0, 1.0);
```

Con la version mala, un disco de radio 1 pintaba **un solo pixel**: el del centro. Y los radios
son medios pixeles (un trazo de 3 tiene radio 1,5), asi que guardarlos en enteros redondeaba
hacia abajo y empeoraba el problema.

**Lo que lo caza:** una prueba que cuente filas SEGUIDAS pintadas en el borde y las compare con
el grosor que dice tener. Y, sobre todo, una prueba `--ignored` que deje un PNG para mirar. En la
imagen se veia a la primera; en los numeros, no.

Es la misma leccion que el audio y que los textos sin traducir, dicha de otra forma: **una
prueba que mide una consecuencia no mide la cosa.** Pixeles tocados es una consecuencia del
grosor, no el grosor.

## 15. `object-contain` deja franjas, y una capa encima no las conoce

La vista previa del editor se ajusta con `object-contain`: se hace todo lo grande que puede
sin deformarse y deja franjas a los lados o arriba y abajo. Encima van las capas de anotar
y de recortar, con sus coordenadas **de 0 a 1 sobre la captura**.

Esas capas iban estiradas al hueco entero (`absolute inset-0 size-full`), asi que sus
coordenadas contaban las franjas como si fueran parte de la captura. Una flecha puesta en el
borde de la imagen se guardaba un poco mas alla, y al exportar salia desplazada. **Con una
captura vertical en una ventana ancha el desplazamiento era de media pantalla.**

Costo un rato encontrarlo porque en el caso normal (una captura horizontal en una ventana
horizontal) el error es de un dos por ciento y no se nota mirando.

**Lo que no funciona:** poner `aspect-ratio` en la caja y confiar en que el navegador la
ajuste. Dentro de un contenedor con contenido propio, esa propiedad se resuelve de maneras
distintas segun el tipo de contenedor: en `flex` el hijo toma el tamanno de su contenido y
la cuenta se vuelve circular, en `grid` con `place-items-center` tampoco salio.

**Lo que si:** medir el hueco con un `ResizeObserver` y calcular la caja a mano
(`src/lib/contener.ts`), que ademas se prueba sin ventana. La regla general: **si una capa
tiene que caer encima de una imagen contenida, se mide; no se negocia con el CSS.**

## 16. Un manejador de teclas con dependencias incompletas cierra la ventana

En el editor, `Escape` tiene que soltar primero lo que se este haciendo (el marco de
recorte, la herramienta de dibujar) y solo cerrar si no hay nada puesto. Cerrar el editor
**tira los fotogramas del disco**, asi que un Escape sin querer no puede llevarse por
delante una grabacion de tres minutos.

El manejador vive en un `useEffect` y lee ese estado. Con las dependencias sin actualizar,
el cierre se queda con la foto vieja: cree que no hay nada puesto y cierra igual. El codigo
se lee perfecto y hace lo contrario de lo que dice.

Y hay una segunda mitad: al meter una funcion (`cerrar`) en las dependencias hay que
comprobar que ya este declarada mas arriba, o el render entero revienta con «Cannot access
before initialization», que en React se ve como una pantalla en blanco.

Lo caza una prueba de las de verdad: pulsar `c`, pulsar `Escape`, y comprobar que la ventana
**no** se ha cerrado. Esta en `EditorApp.test.tsx`.

## 17. `starts_with` sobre una ruta no impide salirse de la carpeta

Una ventana anclada le manda a Rust la ruta de su PNG para copiarlo, guardarlo o leerlo. Se
comprobaba que empezara por la carpeta de ancladas, que parece suficiente y no lo es:

```
C:\...\Temp\winshotx\pins\..\..\..\.ssh\id_rsa
```

empieza por la carpeta buena y acaba donde quiera. La comprobacion tiene que **rechazar
cualquier componente `..`**, no solo mirar el principio. `canonicalize` tambien valdria pero
exige que el archivo ya exista, que aqui no siempre pasa.

En `commands.rs`, con una prueba que se pone roja contra el codigo de antes.

## 18. `image::imageops::resize` cuesta 60 ms por fotograma, y el filtro no tiene la culpa

Munir, el 27 de agosto de 2026, probando el zoom en `tauri dev`: *«por que tarda tanto en
guardar el video? se ha quedado pillado lo de saving»*.

No estaba colgado: estaba escalando. **Sin zoom, un fotograma que ya mide lo que se pide no
se escala, pasa tal cual.** Con zoom, cada fotograma se recorta a un trozo y hay que
estirarlo al tamanno final, asi que aparece un escalado que antes no existia, en TODOS los
fotogramas del tramo acercado.

Medido sobre 640x400 → 1280x800:

| | release | debug |
|---|---:|---:|
| `image` con Lanczos3 | 62 ms | 2.167 ms |
| `image` con Triangle | 53 ms | 1.138 ms |
| `image` con Nearest | 52 ms | 602 ms |
| **`encode::escalar::ampliar`** | **3,8 ms** | **19 ms** |

**La pista fue que `Nearest` costara lo mismo que `Lanczos3`.** Si el filtro mas tonto tarda
igual que el mas fino, el tiempo no se va en filtrar: se va en el camino generico que
recorre `image` para servir a cualquier tipo de pixel. Aqui solo hay un caso, RGBA de 8
bits, y se puede ir derecho: tabla de pesos en punto fijo, un bucle plano y las filas
repartidas con rayon.

Y dos cosas mas que salieron de la misma medicion:

1. **`opt-level = 3` NO hacia falta, y se probo.** El primer intento fue compilar `image` y
   `winshotx` a toda velocidad en vez de para ocupar poco. Ganaba de 91 a 62 ms, pero el
   instalador subia de 2,31 a **2,66 MB**: 360 KB por un tercio de mejora. Con el escalado
   propio, el mismo codigo corre a **2 ms con `opt-level = "s"`**, o sea que la velocidad
   venia del algoritmo y no del compilador. Se quito, y el instalador se queda como estaba.
   La leccion: **medir las dos cosas antes de pagar tamanno por velocidad.**
2. **En debug el factor no es 20, es hasta 85.** Un video de un minuto con zoom habria
   tardado cuarenta minutos en `tauri dev` y dos en la version instalada. Al probar
   rendimiento, la version instalada; `dev` solo para ver si algo funciona.

Bilineal y no Lanczos3 **a proposito**: Lanczos3 sirve para REDUCIR, donde hay que promediar
muchos pixeles en uno. Ampliando, sus lobulos negativos dejan halos en los bordes con
contraste. Reduciendo se sigue usando el de `image`.

## 19. Media Foundation acaba el video en el ULTIMO fotograma, no cuando acaba

Rescatar treinta segundos de una pantalla quieta daba un video de **66 milisegundos**. No
era el anillo (la sesion decia 3000 ms, correctos, y los fotogramas estaban), ni el
reproductor: era el codificador.

`VideoEncoder::send_frame_buffer(buffer, timestamp)` dice cuando **empieza** cada
fotograma, y nada mas. Media Foundation termina el archivo un `1/fps` despues del ultimo
que reciba. Con mil fotogramas eso son 33 milisegundos perdidos al final, invisibles. Con
una pantalla quieta, que el cache guarda en UN fotograma que dura tres segundos, se pierde
el video entero.

**El arreglo:** mandar el ultimo fotograma **dos veces**, la segunda en el instante en que
deberia acabar. Es identico al anterior, asi que al comprimirlo no ocupa nada.

**Como se encontro:** con el test `el_anillo_traga_la_pantalla_de_verdad`, que captura la
pantalla de verdad y lee la duracion de la cabecera `mvhd` del MP4 escrito. Ninguna prueba
sintetica lo habria visto, porque todas mandaban fotogramas de 33 ms. La que lo vio fue la
que se encontro un escritorio parado.

Afecta igual a exportar: una grabacion en la que no se movio nada salia corta.

## 20. Un boton apagado que no lo parece es un boton roto

Lo rescatado del anillo abre el editor **antes** de tener el video de vista previa, que se
escribe por detras. El boton de reproducir se ponia `disabled` mientras tanto, con un
`title` que lo explicaba. Y aun asi, Munir lo pulso, no paso nada, y lo dio por roto: el
`title` hay que descubrirlo dejando el raton quieto encima, y la clase del boton no tenia
ningun `disabled:` que lo apagara. Se veia **exactamente igual** que uno que funciona.

Tres cosas salieron de ahi, y las tres valen para cualquier boton de la app:

1. **Un `disabled` sin estilo `disabled:` no existe.** Los botones de deshacer de la barra
   de anotar ya llevaban `disabled:opacity-30`; este no.
2. **Un `title` no es un aviso**, es una nota al pie. Lo que hay que esperar se dice en
   pantalla, y si se sabe cuanto falta, con el numero.
3. **Un aviso que solo se manda cuando algo sale bien deja el fallo invisible.** El evento
   de «ya esta la vista previa» ahora se manda tambien cuando no ha salido.

Y una cuarta, del mismo dia: **`video.play()` se pide desde el clic, no desde un efecto.**
Lanzado desde un `useEffect` que corre despues, llega fuera del gesto de la persona y el
navegador puede rechazarlo; encima devuelve una promesa que se estaba tragando un
`.catch(() => undefined)`. Ahora sale del propio manejador del boton y el motivo se ensenna.

**Como se investigo, para la proxima vez que algo «no responde» dentro de una webview:**
el arbol de accesibilidad de Windows (UI Automation) lee el estado real de los controles de
una ventana de Tauri **sin tocarla**: `AutomationElement` filtrando por `ProcessId`, y cada
boton dice su `Name` y su `IsEnabled`. Asi se supo, sin preguntar y sin abrir nada, que en
ese momento el boton ya estaba habilitado y que el fallo habia sido la espera de antes.

## 21. Copiar al portapapeles leia del CACHE, no de lo que se acababa de exportar

Munir dibujo una flecha roja sobre una captura, le dio al boton de copiar y la pego: sin
flecha. El archivo guardado si la tenia.

`copy_result` hacia esto para un PNG:

```rust
let image = record::read_frame(session, request.from)?;   // el fotograma EN CRUDO
```

O sea que todo lo que hace exportar se quedaba fuera de lo pegado: las marcas dibujadas
encima, el recorte, el marco, el escalado y el zoom de los clics. Y no se notaba en las
pruebas porque las 26 del exportador comprobaban el ARCHIVO, que estaba perfecto.

**El arreglo:** copiar lee el archivo que se acaba de escribir. Y la decision (imagen o
archivo) esta separada de la llamada al portapapeles, en `que_se_pega`, para poder probarla
sin tocarle el portapapeles a nadie. Esa funcion **no recibe la sesion a proposito**: si
alguien vuelve a sacar los pixeles del cache, necesitara la sesion otra vez y la prueba
dejara de compilar.

**La leccion general:** cuando una funcion tiene delante el resultado ya hecho (un archivo
recien escrito) y ademas la manera de rehacerlo, la que rehace es la que se equivoca.

## 22. Arreglar el escalado en UN sitio no arregla el escalado

La trampa 18 dejo escrito que `image::imageops::resize` cuesta 60 ms por fotograma y que la
culpa no es del filtro sino del camino generico del crate. Se escribio `escalar::ampliar` y
se puso donde dolia. Once dias despues, exportar una grabacion de 50 segundos tardaba **dos
minutos**, y el reparto medido sobre el caso de Munir (1890x1052, 1320 fotogramas) era este:

| paso | por fotograma |
|---|---:|
| sacar el fotograma del cache | 7 ms |
| **escalar a 1280 con `image`** | **64 ms** |
| codificar y escribir | 17 ms |

O sea que el 68 % del tiempo seguia yendose por la misma linea de siempre, porque
`escalar::ampliar` solo cubria AMPLIAR y quedaban **cuatro** sitios reduciendo con
`image`: `exporter::enmarcar_y_anotar`, `mp4::encode`, `gif::scaled` y `marco::enmarcar`.
Cada uno con su `if` y su comentario explicando por que ahi si tocaba Lanczos3.

**El arreglo:** una sola puerta, `escalar::a_medida`, que decide entre ampliar, reducir o
los dos, y que es la unica que se llama desde los cuatro sitios. Exportar paso de **95,6 ms
a 20,4 ms** por fotograma: los dos minutos de Munir son ahora 27 segundos.

**La leccion:** cuando la solucion a un problema medido es una funcion nueva, el trabajo no
acaba al escribirla. Acaba cuando `grep` de la funcion vieja no devuelve nada en el camino
caliente. Un `if` que elige entre lo rapido y lo lento acaba eligiendo lo lento.

## 23. Reducir con los limites en numeros enteros adelgaza las letras

Al bajar de 1890 a 1280, cada pixel de salida cubre 1,477 de entrada. Si los limites se
calculan con division entera, unas veces se promedian **dos** pixeles y otras **uno**:

```rust
let a = (i * origen / destino) as u32;      // 0, 1, 2, 4, 5, 7, 8...
let b = ((i + 1) * origen / destino) as u32;
```

Un color plano sobrevive y un ajedrez sale gris, que era justo lo que comprobaban las dos
pruebas que habia. Pero **el texto de una captura sale con unas letras mas finas que otras**,
porque cada trazo cae en un reparto distinto. Una reja de un pixel se iba 128 del gris.

**El arreglo:** reparto por area, con pesos fraccionarios. Cada pixel de entrada entra por
la parte que solapa, y los pesos de cada eje suman exactamente uno (el redondeo se lo lleva
el peso mas gordo, si no la imagen se va aclarando). La diferencia contra Lanczos3 sobre una
captura de verdad bajo de 1,40 a **0,41** de 255, y el peor pixel de 201 a 56.

**La prueba que lo ve:** una reja de un pixel reducida de 1890 a 1280 tiene que quedarse a
menos de 50 del gris. Con limites enteros daba 128 exactos.

## 24. `set_string` de `clipboard_win` VACIA el portapapeles antes de escribir

Copiar un video dejaba el archivo (CF_HDROP), que es lo que entienden el explorador y los
chats. Pero al pegar en una caja de texto no aparecia nada, porque un archivo no es texto, y
la app decia «Copiado» al mismo tiempo: copiar habia ido bien.

Poner tambien la ruta como texto parecia una linea:

```rust
formats::FileList.write_clipboard(&list)?;
formats::Unicode.write_clipboard(&ruta)?;   // y aqui desaparece el archivo
```

`FileList` usa `NoClear` por dentro y `Unicode` usa `DoClear`, asi que la segunda llamada
vacia el portapapeles y borra la primera. Leerlo despues devolvia `OSError 1168, no se ha
encontrado el elemento`, y con las dos pruebas del portapapeles corriendo a la vez, un
`STATUS_HEAP_CORRUPTION`.

**El arreglo:** `raw::set_file_list` y despues `raw::set_string_with(&ruta,
options::NoClear)`. Comprobado desde OTRO proceso, que es la unica forma de saber que el
portapapeles quedo bien: `FileDrop, FileNameW, FileName, UnicodeText, Text`.

## 25. Chrome no fotografia una ventana de menos de 500 px, y no lo dice

El menu de la bandeja mide **284 px** de ancho (`tray_menu::MENU_ANCHO`). Al fotografiarlo con
`pnpm ver --menu --ancho=284`, la foto salia con la mitad derecha vacia: sin la version, sin el
interruptor del anillo y sin los atajos. Parecia un fallo de la ventana.

No lo era. **Chrome tiene un ancho minimo de ventana de unos 500 px**, asi que a
`--window-size=284,420` pinta la pagina a 500 y despues recorta la imagen a 284. Todo lo que
estaba a la derecha existia, pero se quedaba fuera del recorte. Se comprobo con `--dump-dom`:
el HTML llevaba la version, el atajo y el `role="switch"` en su sitio.

**Como se fotografia una ventana estrecha:** `--escala=2` le pide a Chrome 568 px fisicos, que
si respeta, con `--force-device-scale-factor=2` para que la pagina se pinte a 284 CSS. La foto
sale al doble de resolucion y representa la ventana de verdad.

**Lo que casi pasa:** estuve a punto de "arreglar" el menu anadiendo `min-w-0` a unas clases
que no tenian ningun problema. El cambio se revirtio. **Antes de arreglar lo que se ve mal en
una captura, hay que comprobar que la captura esta bien hecha**, y el DOM es la forma barata de
saberlo: si el dato esta en el HTML, el fallo es de como se mira.

## 26. Un ajuste que no lee nadie es una mentira que pasa las pruebas

`play_sound` llevaba versiones en la pantalla de ajustes: se guardaba, se leia al arrancar, se
sincronizaba entre ventanas y tenia su interruptor. Lo unico que no hacia era sonar, porque
**ninguna linea del proyecto lo consultaba**. Munir, el 30 de agosto de 2026: *«y no suena
ningun sonido xd»*.

Nada lo detectaba: las pruebas comprobaban que el ajuste se guardaba y se leia, que es
exactamente lo que hacia bien.

**Como se caza:** `grep` de cada nombre de ajuste en todo el arbol y mirar cuantos sitios lo
usan **aparte** de la pantalla que lo pinta y del archivo que lo guarda. Si son cero, ese
interruptor no hace nada. Se auditaron los 31 y solo fallaba este.

**El arreglo, para la proxima:** `platform/sonido.rs` toca un WAV incrustado con `include_bytes!`
usando `PlaySoundW`, que ya viene con Windows. Cero dependencias nuevas y 12 KB de instalador.
Y una prueba comprueba la cabecera del WAV, porque `PlaySoundW` **solo sabe PCM**: un mp3
renombrado a `.wav` no sonaria y no daria ningun error.

## 27. Lo que se busca por su posicion o por su texto entero se rompe sin decir nada

Dos fallos de la misma familia aparecieron el mismo dia, y los dos llevaban tiempo puestos sin
que nada avisara.

**El primero, en `frontlaxweb/generar-en.mjs`.** Las descripciones de la pagina viven en
atributos y no pueden llevar `data-en`, asi que se traducen con una tabla, `ATRIBUTOS`, donde la
clave es **la frase espannola entera**. Cuando el tamanno del instalador paso de 2,2 a 2,49 MB,
las tres claves dejaron de casar y la pagina inglesa se quedo sirviendo su `description`, su
`og:description` y su `twitter:description` **en castellano**. Eso es lo que ensenna Google
debajo del titulo ingles y lo que sale al pegar el enlace en un chat.

**El segundo, en `scripts/ver-ventana.mjs`.** Para fotografiar el tour se abria pinchando *el
tercer boton del bloque «winshotx»*. Ese bloque cambio de filas y el tercer boton paso a ser el
interruptor de arrancar con Windows: el tour no se abria, la foto salia de la ventana normal y
parecia correcta.

**Lo que tienen en comun:** los dos aciertan mientras nadie toca nada, los dos fallan con un
cambio que no los menciona, y los dos **no producen ningun error**: producen una pagina o una
foto que parece bien.

**Los dos arreglos, que son el mismo:**

1. Buscar por lo que no cambia. El boton del tour se busca ahora por su nombre, `"Tour"`, que se
   escribe igual en los dos idiomas y no depende de cuantas filas tenga un bloque.
2. Y donde no queda mas remedio que comparar texto entero, **una guardia que se plante**:
   `generar-en.mjs` lleva la cuenta de que claves ha encontrado y sale con codigo 1 nombrando las
   que ya no aparecen. Se probo rompiendo una a proposito: sin esa prueba, una guardia que no
   salta es tan muda como el fallo que venia a cazar.

## 28. Chrome sin cabeza tampoco respeta el meta viewport

Hermana de la 25, y la misma leccion por otro lado: **la foto puede estar mal aunque la pagina
este bien**.

Al fotografiar `frontlaxweb/index.html` con `--window-size=390,640` para ver el pie en un movil,
la pagina sale pintada a ancho de escritorio y recortada a 390 px: el contenido aparece corrido
hacia la derecha y los textos cortados. **No es un fallo de la web.** Chrome sin cabeza no aplica
`<meta name="viewport">` por su cuenta; para eso hace falta emular un dispositivo de verdad, que
es lo que hacen Playwright o el protocolo de depuracion con
`Emulation.setDeviceMetricsOverride`.

Asi que, hasta que ese servidor arranque, **lo movil no se da por comprobado con una captura**:
o se emula el dispositivo, o se dice que no se ha mirado.

## 29. La herramienta de mirar la ventana ensennaba una ventana que no existe

`scripts/ver-ventana.mjs --escala=2` no daba la misma foto mas nitida: daba **otra ventana**.
Multiplicaba `--window-size` por la escala, y ese parametro va en pixeles de CSS, no fisicos.
Resultado: la pagina se pintaba a 1680x1020 en vez de a 840x510, el contenido salia en su
tamanno de siempre y quedaba media foto de espacio vacio. Justo lo contrario del fallo del
alto de la trampa 27, y con el mismo efecto: creer que sobra sitio donde no sobra.

Se vio al preparar las capturas de la Store, porque una ficha con medio lienzo negro canta.
Sin ese encargo habria seguido ahi.

**Y en la misma herramienta, el idioma tampoco llegaba entero.** `--idioma=en` solo entraba
en los ajustes de mentira, que son de la ventana principal. El editor no los pide: lee el
idioma de `localStorage` antes que nada, para que la primera pintada ya salga bien. Asi que
`--idioma=en` daba un editor **en espannol** sin que nada avisara. Ahora el idioma y el tema
se siembran en `localStorage` antes del script de la app, que es donde los busca cada ventana.

## 30. Partner Center: cuatro formas de perder lo que acabas de escribir

Rellenando el envio de la Microsoft Store con Playwright, cuatro cosas que fallan calladas:

1. **El campo de subir capturas acepta un archivo y punto.** Reusar el mismo input reemplaza
   la anterior en vez de anadir: cinco subidas seguidas dejaban **una** captura. Cada subida
   crea un hueco nuevo al final, asi que el input que toca es el numero de capturas que ya hay.
2. **Los campos no tienen etiqueta accesible, y por posicion se cruzan.** Al responder que si
   a lo de la informacion personal aparece un campo mas y todos los indices se corren: la URL
   de privacidad acabo dentro del hueco de la web, la web dentro del de soporte y la de
   soporte dentro del **numero de telefono**. Se localizan por el texto de su bloque.
3. **`text=Guardar borrador` no encuentra el boton; `text="Guardar borrador"` si.** Con
   comillas es coincidencia exacta. Y ese boton solo existe mientras haya cambios sin guardar,
   asi que buscarlo antes de tocar nada da cero y parece que no lo hay.
4. **El precio no se guarda solo, pero lo parece.** Al elegirlo desaparece el aviso de que
   falta, y aun asi se pierde al recargar si no se pulsa guardar. El sintoma no es un error:
   es que «Enviar para certificacion» se queda apagado sin decir cual de las seis secciones
   le falta.

**Y la peor, que es de las que no se ven:** guardar la pagina de la ficha cuando las filas de
las caracteristicas todavia no habian terminado de cargar **las borro las once**, en los dos
idiomas, sin un solo mensaje. La descripcion y las capturas seguian ahi, asi que a ojo la
pagina parecia bien. Solo se vio al mirar la foto de la ficha entera.

**La leccion, que es la de siempre:** despues de guardar un formulario que no controlas, se
recarga y se cuenta lo que hay. Que el guardado no de error no significa que haya guardado lo
que estaba en la pantalla.

## 31. Una palabra escrita a mano que se ve puesta y no se guarda

Las palabras clave de la ficha de la Store se quedaron vacias en el primer envio, y hubo que
cancelar la certificacion para meterlas. Dos fallos encadenados, los dos silenciosos:

**El primero es de bulto: el campo nunca se miro.** Se relleno la ficha buscando los campos
que ya se habia decidido rellenar, en vez de listar lo que la pagina tenia. Las palabras clave
viven debajo de «Informacion adicional» y no salen al enumerar los `input[type=text]`, porque
no son un campo de texto: son un `he-select` multiple con `freeform` dentro de `#search-terms`.

**El segundo es mas fino.** Al teclear una palabra y pulsar Enter, la etiqueta aparece en el
campo, con su aspa y todo. Se guarda, no da ningun error, y al recargar **no hay ninguna**. Lo
que falta es sacar el foco: con un **Tab** despues del Enter se confirma y persiste.

La pista de que el problema era el texto escrito a mano fue probar con una de las palabras que
el propio control recomienda: esa si persistia. Comparar lo que funciona con lo que no vale
mas que mirar diez veces lo que no funciona.

**Y de propina, un limite que no se ve hasta que estorba:** son 7 palabras clave como maximo.
Al probar se dejo una de mas, y la ficha se quedo en «Incompleto» sin que la pagina marcase
nada en rojo. El mensaje, «No puede agregar mas de 7 palabras clave», solo salia leyendo el
texto del propio bloque. El boton de enviar se quedaba apagado, igual que con lo del precio de
la trampa 30, y por el mismo motivo: **el estado de una seccion se calcula en el servidor y no
se explica en la pantalla donde esta el fallo**.

## 32. Un raton de mentira no enciende el `:hover`, y la foto sale igual que sin el

`scripts/ver-ventana.mjs --raton=x,y` decia, en su propio comentario, «sin raton no hay hover».
Lo que manda son `PointerEvent` creados a mano, y eso despierta lo que un componente decide al
recibirlos (un `onPointerEnter` que cambia un estado), pero **no la pseudoclase `:hover` de
CSS**, que solo la enciende el raton de verdad del navegador. Todo lo que se pinta con `hover:`
o `group-hover:` de Tailwind sale en la foto **exactamente igual que si nadie estuviera encima**,
sin un error, sin un aviso y sin diferencia visible con el caso bueno.

Se vio al comprobar que la barra de arriba del overlay se aparta al pasar por encima: la foto
con `--raton` encima de la barra salia identica a la de sin raton. Era cierto en la foto y falso
en la app.

La herramienta no puede arreglarlo por dentro: fotografia con Chrome de linea de ordenes
(`--screenshot`), que no tiene por donde mover un puntero. Asi que se le puso `--servir`, que
deja la pantalla montada y escribe su URL en vez de fotografiar, y el raton de verdad lo pone
Playwright desde fuera con `page.mouse.move`. Es el mismo reparto que en la trampa 28: cada
herramienta hace lo que puede hacer de verdad, y lo que no, se dice.

## 33. CORREGIDA: lo que cuesta despertar una ventana escondida son ~310 ms

**Esta trampa estuvo escrita al reves durante unas horas y hay que contar por que**, porque
el error de metodo vale mas que el dato.

Decia que «una ventana escondida no arranca su interfaz», y de ahi salio un arreglo del menu
de la bandeja que la ensennaba fuera de las pantallas para «despertarla». **Era falso.** El
binario con el que se midio estaba compilado con `cargo build --release` a secas, y ese
binario **no lleva la interfaz dentro**: la busca en el servidor de desarrollo. Con Vite
apagado, cada ventana ensennaba la pagina de error de Edge («localhost rechazo la conexion»)
y por eso ningun cronometro del frontend disparaba nunca. Se dio por hecho que la interfaz
no arrancaba, cuando lo que pasaba es que no existia.

**Lo que hay de verdad, medido con `pnpm tauri build`, que es el que la mete dentro:**

| Momento | Desde que se pulsa el atajo |
|---|---|
| Pantallas congeladas y escritas | 123 ms |
| Ventanas ensennadas | 296 ms |
| **El overlay reacciona al aviso** | **606 ms** |
| Congelado de 8 MB leido | 688 ms |
| **Imagen pintada, lo que se ve** | **689-717 ms** |

O sea que el atajo tarda **700 ms**, no los 296 que se creian, y **el trozo mas gordo son los
310 ms que pasan entre que Rust ensenna la ventana y el navegador de dentro se entera**.
Windows suspende el navegador de una ventana escondida, y los overlays viven escondidos
entre captura y captura para poder reutilizarlos: ese es el peaje del pool, y no estaba
medido. Leer el BMP de 8 MB y pintarlo son 83 ms, que no era el problema.

## 34. Preguntarle algo a una ventana desde otro hilo espera SIN PLAZO, y cuelga la app

La 0.2.13 se le quedo frita a Munir nada mas instalarla. El proceso estaba vivo, con su
icono en la bandeja, pero **no respondia a un solo mensaje de Windows** (`Responding=False`)
y tenia **dos ventanas de captura a 800x600**, que es el tamanno con el que nacen antes de
que `precrear_overlays` les de el de su monitor: el arranque se habia quedado a medias.

Lo unico que cambiaba respecto a la 0.2.12 era una comprobacion de tres lineas, «¿hay una
captura en marcha?», hecha **desde un hilo aparte**:

```rust
app.webview_windows().iter().any(|(etiqueta, ventana)| {
    etiqueta.starts_with(OVERLAY_PREFIX) && ventana.is_visible().unwrap_or(false)
})
```

`is_visible()` parece una consulta inocente y no lo es. En `tauri-runtime-wry`,
`send_user_message` mira en que hilo estas: si es el principal, atiende el mensaje ahi
mismo; si no, lo manda al bucle de eventos y espera la respuesta con un **`rx.recv()` sin
plazo** (macro `getter!`). Basta con que el bucle este ocupado creando ventanas para que ese
hilo se quede clavado para siempre, y con el, lo que venga detras.

**La regla: todo lo que toca ventanas se hace en el hilo principal.** El hilo de fondo solo
sirve para dormir; cuando toca trabajar, se manda el trabajo entero con `run_on_main_thread`
y alli dentro se pregunta y se actua sin canales de por medio.

### Y lo que de verdad fallo fue la comprobacion

Se probo seis veces antes de publicar y las seis salieron bien. Es una carrera: depende del
momento exacto en que el hilo despierta. **Una pasada en verde de algo que depende del
tiempo no prueba nada**, y se dio por bueno igual.

De ahi sale `scripts/arranca-bien.mjs`: arranca la aplicacion, espera a que pase el momento
peligroso y comprueba que responde y que las ventanas se terminaron de crear. Con
`--vueltas=6 --carga` lo hace seis veces con la maquina saturada, que es cuando estas cosas
salen. Ninguna prueba de `cargo test` podia ver esto: el fallo no esta en ninguna funcion,
esta en dos hilos esperandose.

## 35. Los 28 ms se midieron con UNA pantalla, y con tres son diez veces mas

winshotx anuncia, en el README y en toda la web, **28 ms desde que se pulsa el atajo hasta
que la seleccion esta en pantalla**, contra los 920 ms de la Herramienta de Recortes. Es la
cifra que mas se defiende del producto.

En la maquina de Munir, con **tres pantallas** (1920x1080, 1080x1920 y 1536x960), ese mismo
camino tardaba **270 ms con el anillo apagado y hasta 586 ms con el encendido**. Diez o
veinte veces la cifra publicada, y nadie lo habia visto porque nunca se midio con mas de una
pantalla.

**El motivo estaba a la vista en `capture::freeze_all`:** las pantallas se fotografiaban en
un `for`, una detras de otra. Escribir los archivos si estaba paralelizado con
`thread::scope` justo debajo, asi que la mitad del trabajo iba en paralelo y la otra mitad
en fila. Con una pantalla eso no se nota; con tres se paga tres veces.

Puestas a la vez, una por hilo (cada uno se busca su monitor, porque `xcap::Monitor` lleva un
puntero crudo y no se puede mandar entre hilos):

| Con el anillo encendido | Antes | Ahora |
|---|---|---|
| Congelar | 157 / 176 / 392 ms | 116-138 ms |
| Hasta ensennar la seleccion | 277 / 378 / **586** ms | **234-272 ms** |

No es 3x, porque las capturas comparten la tuberia de video, pero **el peor caso pasa de 586
a 272** y sobre todo desaparece la irregularidad, que es lo que se nota: antes el mismo atajo
tardaba el doble o la mitad segun el momento.

**Lo que hay que hacer con la cifra publicada:** o se mide en una maquina de varias pantallas
y se dice el rango, o se dice con que configuracion se midio. Un numero que solo se cumple
con un monitor, anunciado sin decirlo, es una cifra que se rompe sola en cuanto alguien
conecta la segunda.

## 36. `cargo build --release` NO deja una aplicacion que se pueda mirar

Es la trampa que hizo falsa la 33 y que costo una tarde entera. Compilar con:

```
cargo build --release
```

deja un `winshotx.exe` que arranca, ensenna sus ventanas y **no tiene la interfaz dentro**:
cada ventana intenta cargarla de `http://localhost:1421`, el servidor de Vite. Si ese
servidor no esta levantado, las ventanas ensennan la pagina de error del navegador y la
aplicacion parece funcionar a medias sin decir por que.

El que sirve para mirar es:

```
pnpm tauri build
```

que ejecuta antes `pnpm build` y empaqueta `dist/` dentro del binario.

**Como se ve desde fuera, para reconocerlo la proxima vez:** las ventanas se abren, tienen su
tamanno, responden, y por dentro estan en blanco o con un mensaje de error del navegador. Y
cualquier cronometro que dependa del frontend **no dispara nunca**, que es justo lo que
parece «la interfaz no arranca».

**La regla: todo lo que se mida del frontend se mide sobre un binario hecho con
`pnpm tauri build`.** Y si un cronometro del frontend no imprime NADA, la primera sospecha
no es el codigo: es que no hay frontend.

## 37. Pintar el congelado directo del protocolo asset es MAS lento que copiarlo a memoria

El overlay trae el congelado con `fetch`, lo mete en un blob y de ahi al `<img>`. Parece un
rodeo tonto: son 8 MB por pantalla que se leen del disco, se copian a JavaScript, se
envuelven y se decodifican, y ademas `createImageBitmap` los vuelve a decodificar para la
lupa. Lo evidente es darle la URL del protocolo asset al `<img>` y que el navegador lo cargue
una sola vez sin pasar por JavaScript.

**Medido, doce capturas: el camino "evidente" tarda casi el doble.**

| | Del atajo a ver la imagen |
|---|---|
| Con `fetch` a un blob (lo que hay) | 460-600 ms |
| Directo del protocolo asset al `<img>` | **777-1477 ms** |

No se investigo mas alla del numero, porque el numero ya decide: **se revirtio**. La sospecha
es que el manejador del protocolo asset entrega peor a un `<img>` que a un `fetch`, pero eso
esta sin comprobar y aqui se anota como lo que es, una sospecha.

**Lo que si vale como regla:** en este proyecto, cualquier idea de rendimiento se mide antes
de quedarsela, aunque parezca de cajon que va a mejorar. Esta parecia de cajon.

## 38. El arranque con Windows guarda una RUTA, y esa ruta caduca

Encontrada el 4 de septiembre de 2026, y llevaba mordiendo desde el 27 de agosto sin que
nadie la viera.

Munir dijo: *«tienes que estar todo el rato actualizando la app»*, y mando una foto de sus
ajustes. En la esquina ponia **Version 0.1.18**. La publicada era la 0.2.18.

**Lo que habia en su maquina:**

| Donde | Version | Quien la abre |
|---|---|---|
| `C:\Apps\Random APPS\winshotx\winshotx.exe` | 0.2.18 | el menu inicio, y el desinstalador registrado |
| `%LOCALAPPDATA%\winshotx\winshotx.exe` | **0.1.18** | **el arranque con Windows** |

La clave `HKCU\...\CurrentVersion\Run\winshotx` apuntaba a la segunda. Asi que cada vez que
encendia el ordenador le arrancaba la 0.1.18, la 0.1.18 miraba si habia version nueva,
encontraba la 0.2.18 y le pedia actualizar. Actualizaba, el instalador escribia en la carpeta
buena, la app se reiniciaba desde ahi, y **el registro seguia apuntando a la vieja**. Al
siguiente encendido, otra vez.

**De donde sale:** `autostart::set_registro` escribe `std::env::current_exe()` el dia que se
pulsa el interruptor, y nadie lo vuelve a mirar nunca. Basta con reinstalar en otra carpeta
(el asistente de Tauri deja elegirla, y Munir eligio una) para que esa ruta se quede
apuntando a un ejecutable que ya no manda. **No hace falta ni tocar el interruptor.**

**Lo que esto se llevo por delante, y es lo peor:** durante una semana, la aplicacion que
corria en su maquina despues de cada reinicio era la del 27 de agosto. Todo lo que se
publico esos dias para que fuera mas rapido pudo no estar corriendo cuando el decia que iba
lento. Las seis versiones de la noche del 31 se midieron en la maquina de aqui, no en la suya.

**Arreglado** en `autostart::revisar_ruta`, que se llama al arrancar (no al pulsar nada) y
corrige la entrada si no apunta a este ejecutable. Cinco pruebas, incluida la ruta exacta que
tenia el.

**La regla, que vale para cualquier cosa que guarde una ruta absoluta:** una ruta guardada en
el registro, en un acceso directo o en un archivo de ajustes es una foto del dia que se
escribio. Si algo puede mudarse, hay que comprobarla al arrancar, no confiar en que quien la
escribio la mantenga al dia.

**Y la de metodo:** antes de creerse un informe de rendimiento de alguien, **mirar que
version tiene puesta**. La respuesta estaba en la esquina de una captura de pantalla que ya
se habia mirado varias veces sin leerla.

### El arreglo, que la primera vez salio peor que la enfermedad

La primera version de `revisar_ruta` escribia `current_exe()` a secas: «si el arranque no
apunta a mi, que apunte a mi». Media hora despues, `scripts/arranca-bien.mjs` lanzo el binario
recien compilado de `C:\ct
elease\winshotx.exe` para comprobar que arrancaba bien, ese
binario ejecuto el arreglo, y **el arranque de Munir paso a apuntar a un binario de pruebas**.
O sea: el codigo escrito para que no le arrancara la copia equivocada le dejo apuntando a una
copia todavia peor, que ni siquiera esta instalada.

**La regla que faltaba:** «cual me estoy ejecutando» y «cual esta instalada» son dos preguntas
distintas, y `current_exe()` solo contesta la primera. La segunda la contesta el
`InstallLocation` que el instalador deja en la clave de desinstalacion. Ahora solo toca el
arranque el ejecutable que vive dentro de esa carpeta; una copia suelta (de pruebas, recien
descargada, en un pendrive) no manda sobre el arranque de nadie.

Se vio porque **despues de tocarle el registro se volvio a leer para comprobarlo**. Sin esa
segunda lectura se habria publicado tal cual, y el sintoma habria sido el mismo de antes con
otra ruta.

## 39. CORREGIDA: el `/S` del instalador de Tauri SI instala en silencio

En el buzon estaba escrito que el `/S` del instalador NSIS de Tauri abre el asistente igual y
que por eso no se le podia actualizar la app a nadie desde fuera. **Es falso**, y por creerlo se
estuvo dejando que Munir pulsara «Actualizar» a mano version tras version.

Comprobado el 4 de septiembre de 2026, con la app cerrada antes:

    winshotx_0.2.20_x64-setup.exe /S

Termina con **codigo 0**, sin ninguna ventana, y la version instalada en
`C:\Apps\Random APPS\winshotx` pasa de 0.2.18 a 0.2.20. Los ajustes del usuario (atajos,
carpeta, idioma, arranque con Windows) siguen intactos despues.

**La regla:** un apunte que dice «esto no se puede» y que nadie ha vuelto a comprobar caduca
igual que una medicion. Este llevaba dias mandando trabajo manual a Munir.

## 40. La Store rechazo winshotx por una pagina de error, y el riesgo estaba apuntado

El 1 de septiembre de 2026 la Microsoft Store rechazo el envio. El informe, que hay que ir a
buscar a `products/<id>/certification/reports/<uuid>`, decia dos cosas:

| Politica | Lo que decia |
|---|---|
| `10.1.2.10 Functionality` | **Unusable Feature: Display error page at launch.** ASUS EXPERTBOOK P5405CSA, build 26200.9168 |
| `10.2.4.1 Software Dependencies` | No se declaran las dependencias de software no integrado. **Undisclosed software: MS Visual C++** |

**Lo que se descarto, una por una, antes de tocar nada:**

1. **No era el binario sin interfaz** (la trampa 36): se descomprimio el `.msix` enviado y su
   `winshotx.exe` lleva `overlay.html`, `index.html` y `editor.html` dentro.
2. **No era la carpeta de solo lectura del paquete**: WebView2 escribe su perfil en
   `%LOCALAPPDATA%\com.munir.winshotx\EBWebView`, no junto al ejecutable.
3. **No depende del Visual C++ Redistributable**: las dependencias del binario son
   `api-ms-win-crt-*`, o sea el Universal CRT, que viene con Windows desde el 10. Su escaner
   lo lee como «MS Visual C++»; declararlo cuesta una linea y discutirlo cuesta una ronda.

**Lo que quedaba, y estaba escrito en el buzon desde el dia del envio:** *«si la maquina donde
Microsoft certifique no tiene el runtime de WebView2, la app arranca sin ventana... es el
unico punto por donde le veo un rechazo»*. Se cumplio.

**No se pudo reproducir**, y conviene decirlo: esta maquina tiene WebView2, y para instalar el
MSIX y probarlo hace falta el modo desarrollador, que pide permisos de administrador.

**Lo que se hizo, que arregla el sintoma pase lo que pase:** `platform::webview` comprueba
WebView2 **antes de crear ninguna ventana** y, si no esta, lo dice con un `MessageBoxW` del
sistema (que no necesita WebView2 para pintarse) en el idioma de Windows, y sale. Una pagina
de error del navegador no dice ni que ha pasado ni que hacer; un cuadro de dialogo que nombra
lo que falta y donde se consigue, si.

**La leccion:** un riesgo apuntado como «poco probable» y sin comprobar es un riesgo que
sigue entero. Ese punto llevaba escrito desde el 31 de agosto con la palabra «probable»
delante, y fue exactamente lo que paso.

## 41. El GIF iba un 10 % mas rapido que la grabacion, y nadie lo habia visto

Encontrada el 5 de septiembre de 2026 al medir el codificador GIF, no al mirarlo: un GIF
«va rapido» sin mas y a nadie le llamo la atencion.

El GIF cuenta el tiempo en centesimas de segundo. Cada fotograma de 33 ms se redondeaba a 3
centesimas **por separado**, asi que treinta fotogramas de 990 ms salian como 90 centesimas:
un clip grabado a 30 fps se reproducia un 10 % mas deprisa de lo que paso. Ahora el redondeo
se arrastra de un fotograma al siguiente (`gif::Reloj`) y la duracion total cuadra. Hay una
prueba que codifica doce fotogramas, vuelve a abrir el archivo y suma los retardos.

**La regla:** cuando algo se redondea por unidad y se suma, el redondeo se acumula. Se
redondea el total y se reparte la diferencia, no al reves.

### Y las tres cosas que salieron de medir ese mismo codificador

1. **Calidad 100 tardaba 352 ms por fotograma; calidad 80, 46.** No era la calidad: era que
   la paleta se entrenaba con toda la muestra, y la muestra crecia con el tamanno del clip.
   Once millones de pixeles a calidad 100 son veintisiete segundos antes de escribir nada.
   Lo que hay que acotar es cuantos pixeles VISITA el entrenamiento, no cuantos hay.
2. **Una cache de colores compartida entre hilos salia distinta cada vez.** Se guardaba la
   respuesta por «casilla» (los seis bits altos de cada canal), y dos colores de la misma
   casilla con respuestas distintas: ganaba el hilo que llegara primero. Una prueba que
   codifica dos veces y compara byte a byte lo cazo a la primera. Una casilla por color
   exacto (16 MB que viven lo que dura la exportacion) es determinista por construccion, y
   ademas mas fiel: el error de color bajo de 1,35 a 1,27 niveles.
3. **Para comparar con lo de antes, lo de antes se trajo de git a un modulo de pruebas**
   (`git show HEAD:ruta > gif_viejo.rs`, `#[cfg(test)] mod gif_viejo;`) y el banco codifico
   con los dos, midiendo tiempo, tamanno y **error de color contra el original** (decodificar
   el GIF y comparar pixel a pixel). Sin el error medido, «mas rapido» podia estar escondiendo
   «peor», y con la primera version de la cache lo estaba. El modulo se borro al terminar.

| calidad | antes | ahora |
|---|---|---|
| 50 | 18 ms/fotograma, error 2,01 | 6,6 ms/fotograma, error 1,26 |
| 80 | 37 ms/fotograma, error 1,35 | 8,4 ms/fotograma, error 1,27 |
| 100 | **308 ms/fotograma**, error 1,54 | 8,2 ms/fotograma, error 1,30 |

Medido en release, 90 fotogramas de 1280x720 con casi todo quieto. El banco esta en
`encode/bench_gif.rs`: `cargo test --release --lib medir_gif -- --ignored --nocapture`.

### Lo demas que se midio ese dia, con sus bancos

| Camino | Antes | Ahora | Banco |
|---|---|---|---|
| Recortar la seleccion de la pantalla congelada | 25 ms (volvia a abrir el BMP de 8 MB) | 2 ms (la imagen se queda en memoria) | `capture/bench_freeze.rs` |
| Juntar las tres pantallas (tecla `0`) | 110 ms | 17 ms | idem |
| Congelar las tres pantallas | 110 ms | 85-90 ms (el BMP se escribe a mano) | idem |
| Miniaturas al parar de grabar, a 1080p | 7,2 ms/fotograma | 2,7 ms/fotograma | `record/bench_thumbs.rs` |
| Leer los fotogramas al exportar | 12 ms/fotograma (`read_frame` reconstruia desde el ultimo entero) | 4 ms/fotograma (`LectorEnOrden`) | idem |
| El fotograma grande del editor al saltar con el raton | 180 ms (PNG con la compresion de fabrica) | 28 ms (`png::save_fast`) | `encode/png.rs` |

**Lo que NO se midio, y por que:** el camino del atajo hasta ver la imagen en el overlay
(trampas 33 y 37) necesita abrir los overlays encima del escritorio de Munir, y eso no se hace
sin el. Queda apuntado en el buzon con una hipotesis: los 300 ms que pasan entre ensennar la
ventana y que el navegador reaccione podrian ser un cambio de DPI al mover la ventana del
aparcadero a su pantalla, si sus pantallas no van todas al mismo escalado.
