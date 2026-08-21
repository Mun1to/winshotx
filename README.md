<div align="center">

# winshotx

**Captura de pantalla y grabación de GIF/MP4 para Windows.**
Local, sin cuenta, sin nube y sin binarios externos.

[![Licencia MIT](https://img.shields.io/badge/licencia-MIT-3b82f6)](LICENSE)
[![Windows 10/11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4)](#estado)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.11-ffc131)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-1.82%2B-dea584)](https://www.rust-lang.org)
[![Instalador 2 MB](https://img.shields.io/badge/instalador-2%20MB-22c55e)](#instalación)

La estética y el flujo de CleanShot X, la edición fotograma a fotograma de ScreenToGif y los
atajos globales de ShareX, en un instalador de 2 MB.

[![Descargar winshotx para Windows](https://img.shields.io/badge/descargar-para%20Windows-0078d4?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/Mun1to/winshotx/releases/latest/download/winshotx-setup.exe)

<img src="docs/img/ajustes.png" alt="Panel de ajustes de winshotx" width="760">

</div>

## Qué hace

- **Captura de región** sobre la pantalla congelada, con lupa de píxel que muestra el color exacto
  en hexadecimal y ajuste automático a las ventanas del sistema: un clic encima de una ventana la
  selecciona entera.
- **Grabación de la región** a 15, 30 o 60 fps con Windows Graphics Capture, sin FFmpeg y sin
  overlays que se cuelen en el vídeo.
- **Editor** con tira de miniaturas, recorte A/B, reproducción en bucle, escala con proporción
  bloqueada y control de calidad.
- **Exportación** a GIF, MP4 o PNG, a disco y al portapapeles: la imagen se pega como imagen y el
  GIF o el MP4 se pegan como archivo en Slack, Discord o el Explorador.
- **Todo local.** No hay cuenta, ni telemetría, ni subidas. Lo que capturas no sale de tu equipo.

## Instalación

[**Descargar el instalador**](https://github.com/Mun1to/winshotx/releases/latest/download/winshotx-setup.exe)
· 2,2 MB · se instala solo para tu usuario y no pide permisos de administrador. Las versiones
anteriores están en [Releases](../../releases).

A partir de la 0.1.2 se actualiza sola: en **Ajustes → Actualizaciones** aparece el botón cuando
hay versión nueva, y con un clic se descarga, se instala y se reinicia. Las descargas van firmadas
y la app comprueba la firma antes de instalar nada, así que un archivo manipulado no entra.

Al abrirse vive en la bandeja del sistema, sin ventana. Windows esconde los iconos nuevos: si no lo
ves, está detrás de la flecha `^` de la barra de tareas.

## Atajos

| Atajo | Acción |
|---|---|
| `Ctrl+Shift+2` | Capturar región |
| `Ctrl+Shift+5` | Grabar región · púlsalo otra vez para terminar |
| `Enter` | Copiar la selección al portapapeles |
| `Ctrl+S` | Guardar la selección |
| `E` | Abrir la selección en el editor |
| `G` / `V` | Grabar la selección como GIF / vídeo |
| `M` | Silenciar o activar el audio |
| `Ctrl+A` | Seleccionar el monitor entero |
| `←↑→↓` | Mover la selección · con `Shift` de 10 en 10 · con `Alt` redimensiona |
| `Esc` | Cancelar |

En el editor: `espacio` reproduce, `I` y `O` marcan inicio y final del recorte, `←` `→` avanzan
fotograma a fotograma, `Ctrl+S` exporta con los ajustes del panel y `Esc` cierra.

Los dos atajos globales se cambian desde Ajustes pulsando sobre ellos y tecleando la combinación
nueva. Si otra aplicación ya la tiene cogida, el campo se pone rojo y avisa.

## Cómo está construido

| Capa | Elección | Por qué |
|---|---|---|
| Escritorio | [Tauri 2](https://tauri.app) | binario pequeño, webview del sistema, backend en Rust |
| Actualización | plugin `updater` de Tauri, firmas minisign | un botón, sin salir de la app y sin instalar nada a ciegas |
| Interfaz | React 19 + Vite + Tailwind 4 + framer-motion | ventanas independientes, animación nativa |
| Captura estática | [`xcap`](https://crates.io/crates/xcap) | enumera monitores y ventanas con sus coordenadas reales |
| Grabación | [`windows-capture`](https://crates.io/crates/windows-capture) | Windows Graphics Capture, 60 fps sin coste de CPU |
| MP4 | Media Foundation, H.264 por hardware | 0 MB de dependencias, aceleración del sistema |
| GIF | [`gif`](https://crates.io/crates/gif) + [`color_quant`](https://crates.io/crates/color_quant) | paleta global, dithering y diferencia entre fotogramas |
| Caché de edición | [QOI](https://qoiformat.org) sin pérdida | rápido de escribir y editable fotograma a fotograma |

### Dos decisiones que explican el resto

**El overlay de selección no es una ventana transparente.** Se captura la pantalla, se muestra
congelada y se selecciona encima. Esquiva el bug de transparencia de Tauri v2 en Windows, elimina el
parpadeo del contenido en movimiento y regala una lupa exacta al píxel.

**Nada de FFmpeg empaquetado.** El MP4 lo escribe Media Foundation por hardware y el GIF se genera
en Rust puro con paleta global, dithering Floyd–Steinberg y escritura solo del rectángulo que cambia
entre fotogramas. Si tienes `ffmpeg` en el `PATH`, el editor ofrece además un motor de máxima
calidad (`palettegen`), pero nunca se descarga ni se distribuye.

## Desarrollo

```bash
pnpm install
pnpm approve-builds --all
pnpm tauri dev      # arranca la app (vive en la bandeja del sistema)
pnpm tauri build    # instalador NSIS en target/release/bundle/nsis
```

Para publicar una versión hace falta la clave privada de firma, que **no está en el repositorio**
y sin la cual el actualizador no aceptaría la descarga:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/winshotx.key)"
pnpm tauri build
node scripts/publicar.mjs --publicar   # prepara latest.json y crea la release
```

Pruebas del backend:

```bash
cd src-tauri
cargo test
```

Las pruebas de integración no son de mentira: capturan la pantalla real, graban un clip con Windows
Graphics Capture y exportan GIF y MP4 que luego se releen para comprobar que son válidos.

En [`docs/TRAMPAS.md`](docs/TRAMPAS.md) están los seis fallos de Tauri v2 en Windows que costaron
horas de depuración: comandos síncronos que congelan la interfaz, etiquetas de ventana que no se
pueden reutilizar, el primer clic que se come el sistema, el canvas contaminado por el protocolo
`asset:` y una CSP incompleta que solo se nota en el instalador. Si vas a tocar ventanas o la
seguridad del webview, léelo antes.

## Estado

Funciona en Windows 10 1903 o superior. macOS y Linux compilan, pero las funciones de captura
devuelven "no implementado": el backend está detrás de un trait `CaptureBackend`, así que añadirlos
es escribir `capture/mac.rs` y `capture/linux.rs`.

**Lo que falta:** el audio del sistema todavía no se graba. Hace falta WASAPI en modo loopback para
alimentar al codificador; el interruptor ya está en la interfaz y avisa de que no está disponible.

## Licencia

[MIT](LICENSE). Úsalo, cámbialo y véndelo si quieres.
