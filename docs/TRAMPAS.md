# Trampas de Tauri v2 + Windows que costaron sangre

Cinco fallos reales encontrados montando winshotx. Ninguno da error claro: la app se
cuelga o no hace nada. Si vuelve a pasar algo raro con ventanas, empieza por aquí.

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

## Extra: los eventos globales tropiezan con ventanas muertas

`app.emit(...)` intenta entregar a **todas** las ventanas, incluidas las que se acaban de
cerrar, y suelta `PostMessage failed … El identificador de la ventana no es válido`. El
contador de la barra de grabación se quedaba a cero por esto. Se emite ventana a ventana
recorriendo `app.webview_windows()` y filtrando por prefijo de etiqueta.
