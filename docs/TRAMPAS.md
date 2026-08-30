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
