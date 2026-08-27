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
