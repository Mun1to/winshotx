<div align="center">

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/img/logo.svg"><img src=".github/readme/logo-light.svg" alt="winshotx" height="88"></picture>

# winshotx

**Recorta la pantalla antes de que parpadees**

Una **alternativa libre a la Herramienta de Recortes de Windows**: captura de región con lupa de
píxel, **grabación en GIF y MP4** y un editor con estudio. Enseña la selección ya pintada
114 ms después del atajo con tres pantallas, gasta 33 MB de memoria y cabe en un instalador de
2,85 MB. Sin cuenta, sin nube, sin telemetría y sin FFmpeg empaquetado.

<a href="https://github.com/Mun1to/winshotx/releases/latest/download/winshotx-setup.exe"><picture><source media="(prefers-color-scheme: dark)" srcset=".github/readme/download-windows-dark.es.svg"><img src=".github/readme/download-windows-light.es.svg" alt="Descargar para Windows"></picture></a>
<a href="https://winshotx.com/"><picture><source media="(prefers-color-scheme: dark)" srcset=".github/readme/download-web-dark.es.svg"><img src=".github/readme/download-web-light.es.svg" alt="Probarla en el navegador"></picture></a>
<a href="https://apps.microsoft.com/detail/9P1NKWNRXD6Z"><img src="https://get.microsoft.com/images/es%20dark.svg" alt="Microsoft Store" height="52"></a>

[Licencia MIT](LICENSE) · Windows 10 y 11 · Instalador 2,85 MB

[Comparar con la Herramienta de Recortes](#herramienta-de-recortes-contra-winshotx) · [Última versión](https://github.com/Mun1to/winshotx/releases/latest) · [English](README.md)

</div>

<p align="center"><img src="frontlaxweb/social.png" alt="winshotx: recorta la pantalla antes de que parpadees"></p>

## Cómo funciona

<p align="center"><picture>
<source media="(prefers-color-scheme: dark) and (max-width: 600px)" srcset=".github/readme/how-it-works-dark-narrow.es.svg">
<source media="(max-width: 600px)" srcset=".github/readme/how-it-works-light-narrow.es.svg">
<source media="(prefers-color-scheme: dark)" srcset=".github/readme/how-it-works-dark.es.svg">
<img src=".github/readme/how-it-works-light.es.svg" alt="Pulsa el atajo: Se captura la pantalla, se muestra congelada y se selecciona encima. Arrastra sobre la zona: Sobre la pantalla congelada, con lupa 6× que da el color exacto en hexadecimal y ajuste a las ventanas: un clic coge la ventana entera. Suelta: Aparece una barra sobre la selección: copiar, guardar, editar Exporta: GIF, MP4, PNG o JPG, a disco y al portapapeles: la imagen se pega como imagen y el GIF o el MP4 se pegan como archivo en Slack, Discord o el Explorador.">
</picture></p>

**La interfaz habla español e inglés**, y arranca en el que tenga puesto Windows. Se puede fijar
uno de los dos en **Ajustes > La app > Aspecto**. Los identificadores y los comentarios del código
están en español; los commits y la portada en inglés van aparte.

## Qué hace

|  |  |
|---|---|
| **Captura de región** | Sobre la pantalla congelada, con lupa 6× que da el color exacto en hexadecimal y ajuste a las ventanas: un clic coge la ventana entera. |
| **Tapar datos personales** | Un clic en el editor encuentra los correos, teléfonos, tarjetas, cuentas bancarias y DNI de la imagen y deja un mosaico encima de cada uno, que destruye los píxeles en vez de emborronarlos. Nada se reconoce por la forma: la tarjeta tiene que pasar su dígito de control, el IBAN su resto y el DNI la letra que le toca, así que una factura no sale medio tapada. |
| **Grabar una sola ventana** | Pulsa `W` con una ventana resaltada y se graba ella, aunque algo la tape: un aviso que salta encima ya no acaba dentro del vídeo. Y si mueves la ventana, los aros de los clics van con ella. |
| **Grabación de la región** | A 15, 30 o 60 fps con Windows Graphics Capture, sin que se cuelen overlays en el vídeo. Un marco rojo fino enseña lo que se está grabando (ámbar en pausa), la barra de debajo dice si está entrando el sonido, y descartar pregunta dos veces. |
| **Recortar en la línea de tiempo** | La grabación entera cabe a lo ancho: arrastras sus dos bordes azules para quedarte con un trozo, y la vista previa enseña el corte exacto mientras arrastras. Corta al milisegundo, también dentro de un rato en el que la pantalla no cambia, y el play se para al final del trozo. Proporciones de un clic (9:16, 4:5, 1:1, 16:9), encuadrar la imagen, escala con proporción bloqueada y control de calidad. |
| **Estudio de grabación** | Un panel «Estudio»: ocho fondos con aire, esquinas redondeadas y sombra alrededor del vídeo; el puntero dibujado del tamaño que quieras sin pixelarse, con movimiento suave que le quita los temblores de la mano y la opción de esconderlo cuando se para; la cámara acercándose a cada clic y a cada atajo a la velocidad que elijas, siguiendo al ratón mientras está cerca; aros en los clics, los atajos en pantalla y el volumen. **Todo se decide al exportar**, sobre fotogramas que ya están en disco, y **todo se ve en vivo en la vista previa del editor**: lo que ves es lo que sale. |
| **Velocidad al exportar** | 0,5×, 2× o 4× sobre lo grabado. Un tutorial tiene tramos en los que no pasa nada, y esos se ven a 2× sin perder nada. |
| **Exportación** | GIF, MP4, PNG o JPG, a disco y al portapapeles: la imagen se pega como imagen y el GIF o el MP4 se pegan como **archivo** en Slack, Discord o el Explorador. |
| **Los últimos segundos** | Graba siempre, tira lo viejo, y con una tecla se queda con los últimos 15, 30 o 60 segundos. Lo bueno de una pantalla casi siempre pasa **antes** de que a nadie se le ocurra grabarla. |
| **Audio del sistema** | Lo que suena por tus altavoces entra en el MP4, cogido de la salida por defecto. Sin instalar nada, y lo sigues oyendo mientras se graba. |
| **Capturar con espera** | 3 o 5 segundos antes de que se congele la pantalla, con la cuenta atrás en el centro. Es la única forma de fotografiar un menú abierto, porque al pulsar el atajo se cierra. |
| **El texto de una imagen** | `T` sobre la selección, o el botón del editor, lee lo que pone en la imagen con el lector de Windows y lo copia. También en una grabación, sobre el fotograma que estés mirando. |
| **Tus últimas capturas** | El menú de la bandeja enseña las últimas que guardaste como miniaturas, y un clic vuelve a dejarla en el portapapeles. No hay archivo de historial: la carpeta ES el historial, así que lo que borres por fuera desaparece de ahí solo. |
| **Español, inglés y chino** | Sigue el idioma de Windows, o fijas el que quieras. El chino lo aportó alguien de fuera. |
| **Claro y oscuro** | Sigue el tema de Windows y cambia con él, o fijas el que quieras. |
| **Todo local** | Ni cuenta, ni telemetría, ni subidas. La única llamada a la red es mirar si hay versión nueva en GitHub. |

## Capturas

<p align="center"><img src="docs/img/ajustes.png" alt="Panel de ajustes de winshotx"></p>

<p align="center">Panel de ajustes de winshotx</p>

## Herramienta de Recortes contra winshotx

Mismo equipo con Windows 11, tres pasadas cada una y las dos arrancando desde cero. El cronómetro
para cuando la selección ya se ve en pantalla. La
[tabla entera](https://winshotx.com/#frente-a-frente) tiene las diecinueve filas,
incluidas las cinco que gana la de Windows.

|  | winshotx | Herramienta de Recortes |
|---|---|---|
| Del atajo a la selección en pantalla, ya pintada (tres pantallas) | **114 ms** | 920 ms |
| Memoria mientras capturas | **33 MB** | 253 MB |
| Memoria esperando quieta | **31 MB** | 98 MB |
| Graba GIF | **sí** | no |
| Editor con línea de tiempo y estudio | **sí** | no |
| Elegir el atajo de teclado | **sí** | no |
| Dibujar y anotar encima | **seis marcas, con pasos numerados** | bolígrafo, formas y texto |
| Copiar el texto de la imagen | **sí, con la tecla T y también dentro del editor** | sí |
| Audio del sistema al grabar | **sí, y el micrófono en su propia pista** | sí |
| Temporizador antes de capturar | 3 o 5 segundos | **3, 5 o 10 segundos** |
| Tapar datos personales | **de un clic, y el mosaico destruye los píxeles** | automático, y difumina |

## Atajos

| Atajo | Acción |
|---|---|
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>2</kbd> | Capturar región |
| <kbd>Win</kbd> + <kbd>Shift</kbd> + <kbd>L</kbd> | Capturar una región y abrirla en el editor, sin pasar por la barra |
| En el editor | El escudo tapa los correos, teléfonos, tarjetas, cuentas y DNI que encuentre |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>5</kbd> | Grabar región · púlsalo otra vez para terminar |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>6</kbd> | Quedarte con lo último que pasó · mientras «los últimos segundos» esté encendido |
| <kbd>Impr Pant</kbd> | Capturar región · si le quitas la tecla a la Herramienta de Recortes |
| <kbd>Enter</kbd> | Copiar la selección al portapapeles |
| <kbd>Ctrl</kbd> + <kbd>S</kbd> | Guardar la selección en la carpeta de capturas |
| <kbd>Ctrl</kbd> + <kbd>Mayús</kbd> + <kbd>S</kbd> | Guardarla donde quieras: Windows pregunta carpeta y nombre |
| <kbd>E</kbd> | Abrir la selección en el editor |
| <kbd>A</kbd> | Anclar la selección: se queda flotando encima de todo, en su sitio |
| <kbd>T</kbd> | Copiar el texto de la selección, leído por el lector de Windows |
| <kbd>G</kbd> / <kbd>V</kbd> | Grabar la selección como GIF / vídeo |
| <kbd>W</kbd> | Grabar **solo la ventana de debajo del cursor**, aunque algo la tape |
| <kbd>Ctrl</kbd> + <kbd>A</kbd> | Seleccionar el monitor entero |
| <kbd>←↑>↓</kbd> | Mover la selección · con `Shift` de 10 en 10 · con `Alt` redimensiona |
| <kbd>Esc</kbd> | Cancelar |

## Instalación

[**Descargar el instalador**](https://github.com/Mun1to/winshotx/releases/latest/download/winshotx-setup.exe)
· 2,85 MB · se instala solo para tu usuario, así que Windows no pide permisos de administrador. Las
versiones anteriores están en [Releases](../../releases).

Otros tres caminos, la misma app:

```
winget install Mun1to.winshotx
```

o el **portable** `winshotx-<versión>-portable-x64.zip` de
[Releases](../../releases): un ejecutable, sin instalar nada, con los ajustes igualmente en
`%APPDATA%\winshotx`.

o la [**Microsoft Store**](https://apps.microsoft.com/detail/9P1NKWNRXD6Z), donde el paquete lo
firma Microsoft y lo mantiene al día la propia Store, así que el actualizador de la app se aparta.

Al abrirse vive en la bandeja del sistema, sin ventana. Windows esconde los iconos nuevos: si no lo
ves, está detrás de la flecha `^` de la barra de tareas.

**Se actualiza sola.** En **Ajustes > Actualizaciones** aparece el botón cuando hay versión nueva, y
con un clic se descarga, se instala y se reinicia. Las descargas van firmadas y la app comprueba la
firma antes de instalar nada, así que un archivo manipulado no entra.

## ¿Es esto lo que buscabas?

- **"Quiero algo mejor que la Herramienta de Recortes."** Es esto, y se abre 33 veces antes. La de
  Windows sigue ganando en anotar, en leer el texto de la imagen y en el audio del sistema, y la
  tabla de arriba lo dice.
- **"¿Cómo grabo un GIF de la pantalla en Windows?"** Pulsa `Ctrl+Shift+5`, arrastra sobre la zona,
  púlsalo otra vez para parar y exporta a GIF desde el editor. Sin instalar FFmpeg.
- **"Necesito un grabador de pantalla ligero."** 33 MB mientras captura y 31 esperando en la
  bandeja.
- **"Algo como ShareX o CleanShot X, pero más simple."** Los mismos atajos globales y el mismo
  overlay sobre la pantalla congelada, sin cientos de ajustes.
- **"Quiero sacar un color de la pantalla."** La lupa te da el hexadecimal del píxel bajo el cursor
  con zoom 6×.
- **"¿Sube mis capturas a algún sitio?"** No. Ni cuenta, ni telemetría, ni más red que mirar si hay
  versión nueva en GitHub.

## Atajos

En el editor: arrastra los bordes azules de la línea de tiempo para recortar, o pulsa `I` y `O` para
cortar donde está la vista previa; `espacio` reproduce el trozo que te quedas, `C` encuadra la imagen,
`←` `>` avanzan fotograma a fotograma, `Ctrl+S` exporta con los ajustes del panel, `Ctrl+Mayús+S`
pregunta antes dónde guardarlo y `Esc` cierra.

En el editor, para anotar: `1` flecha, `2` rectángulo, `3` texto, `4` resaltar, `5` paso numerado
y `6` tapar datos. `Ctrl+Z` deshace la última marca. Los pasos se numeran solos, así que meter uno
en medio no deja dos treses en la misma imagen.

Los dos atajos globales se cambian desde Ajustes pulsando el campo y tecleando la combinación
nueva. Si otra aplicación ya la tiene cogida, el campo se pone rojo y avisa.

La tecla `Impr Pant` la tiene la Herramienta de Recortes por un valor del registro del usuario, así
que registrar el atajo por su cuenta parece funcionar y luego no llega ninguna pulsación. El
interruptor de Ajustes apaga ese valor y coge la tecla, y al desactivarlo lo deja como estaba.
`Win+Mayús+S` la atiende Windows por delante de cualquier programa, hook o atajo. Lo único que se la
quita es apagar la S en `DisabledHotkeys`, y eso hace el mismo interruptor: cuesta perder `Win+S`, la
búsqueda, y al desactivarlo se devuelve todo tal cual. Esa lista la lee el escritorio solo al
arrancar, así que hace falta que vuelva a arrancar: el botón **Aplicar** de esa misma fila reinicia
el Explorador, que tarda dos segundos y no cierra nada, en vez de obligar a cerrar sesión. Si se
prefiere quitar la Herramienta de Recortes entera, la app abre la pantalla de Windows donde se
desinstala, pero nunca desinstala nada por su cuenta.

## Dos formas de capturar

Las dos usan el mismo atajo y la misma selección. Lo que cambia es el momento de soltar:

| Perfil | Qué pasa al soltar el ratón |
|---|---|
| **Sale la barra** | Aparece una barra sobre la selección: copiar, guardar, editar |
| **Se copia sola** | No aparece nada · la imagen ya está en el portapapeles |

El perfil se elige en la bienvenida del primer arranque, y se cambia cuando quieras desde Ajustes.

## Qué sale del recorte, y de qué pantalla

Arriba y en el centro, donde Windows pone la suya, hay una barra con cuatro botones. Los tres
primeros dicen **qué sale**: foto, vídeo o GIF, elegido antes de recortar y con las teclas `F`,
`V` y `G`. El cuarto dice **de dónde**: al pulsarlo, cada monitor saca su propio número en el
centro, y con `1`, `2` o `3` te llevas esa pantalla entera, esté donde esté el ratón en ese
momento. Un clic sobre la pantalla hace lo mismo.

Grabar respeta el perfil: con «se copia sola» arranca al soltar, y con «sale la barra» te deja
ajustar el recuadro antes, porque equivocarse en una grabación cuesta minutos y no una tecla.

## Para agentes de IA

Un agente de IA (Claude Code, Codex, Cursor o cualquier otro cliente de MCP) puede manejar winshotx
por su cuenta: graba la pantalla, para, saca un MP4 o un GIF ya montado y hace capturas, y se entera
de dónde ha quedado cada archivo. Para añadirlo a Claude Code, escribe esto en PowerShell:

```
claude mcp add --scope user winshotx -- "$env:LOCALAPPDATA\winshotx\winshotx.exe" mcp
```

Es donde lo dejan el instalador y winget, salvo que eligieras otra carpeta (la guía trae una línea
que la encuentra); con la portable, pon la carpeta donde la descomprimiste.
Sin MCP, `winshotx.exe agent` hace lo mismo desde cualquier terminal y contesta con una línea de
JSON. **Una grabación que empieza un agente se ve igual que una tuya**, con el marco rojo y la
barra, así que siempre sabes cuándo se graba la pantalla. Cómo se añade a cada agente, y todas las
órdenes: [docs/AGENTS-API.es.md](docs/AGENTS-API.es.md).

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

**El overlay de selección no es una ventana transparente.** Se captura la pantalla, se muestra
congelada y se selecciona encima. Esquiva el bug de transparencia de Tauri v2 en Windows, elimina el
parpadeo del contenido en movimiento y regala una lupa exacta al píxel.

**Nada de FFmpeg empaquetado.** El MP4 lo escribe Media Foundation por hardware y el GIF se genera
en Rust puro con paleta global, dithering Floyd-Steinberg y escritura solo del rectángulo que cambia
entre fotogramas. Si tienes `ffmpeg` en el `PATH`, el editor ofrece además un motor de máxima
calidad (`palettegen`), pero nunca se descarga ni se distribuye.

[`docs/TRAMPAS.md`](docs/TRAMPAS.md) recoge las siete trampas de Tauri v2 en Windows que costaron
horas: comandos síncronos que congelan la interfaz, ventanas que no se pueden crear desde el hilo de
un atajo global, etiquetas de ventana que no se reutilizan, el primer clic que se come el sistema,
un canvas contaminado por el protocolo `asset:`, una CSP incompleta que solo falla en el instalador
y la clave de firma que se pasa de una forma y no de otra.

## Estado

Funciona en Windows 10 1903 o superior. macOS y Linux compilan, pero toda función específica de
plataforma devuelve "esta función solo está implementada en Windows": captura, grabación,
codificación MP4, portapapeles y arranque automático están detrás de `#[cfg(windows)]` con un stub
para el resto, así que portarlo es rellenar esos stubs.

**Lo que falta:** tapar datos personales solo (winshotx los tapa a mano, con mosaico), un
temporizador de más de cinco segundos, y cualquier idioma que no sea español o inglés. La
comparativa entera de la web dice qué filas sigue ganando la Herramienta de Recortes.

<details>
<summary><b>Desarrollo</b></summary>

```bash
pnpm install
pnpm approve-builds --all
pnpm tauri dev      # arranca la app (vive en la bandeja del sistema)
pnpm tauri build    # instalador NSIS en target/release/bundle/nsis
```

Las pruebas del backend no son de mentira: capturan la pantalla real, graban un clip con Windows
Graphics Capture y exportan GIF y MP4 que luego se vuelven a leer para comprobar que son válidos.

```bash
cd src-tauri
cargo test
```

La página está en [`frontlaxweb/`](frontlaxweb) y la sirve Cloudflare Pages. Cada push pasa las
comprobaciones y solo entonces despliega, así que el inglés, las huellas de los archivos y el
sitemap tienen que cuadrar primero:

```bash
node frontlaxweb/generar-en.mjs      # rehace /en/, el bloque de preguntas y las huellas
python frontlaxweb/generar-social.py # rehace las dos tarjetas de 1200x630
```

Para publicar una versión hace falta la clave privada de firma, que **no está en el repositorio** y
sin la cual el actualizador no aceptaría la descarga:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/winshotx.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""   # la clave se generó sin contraseña
pnpm tauri build
node scripts/publicar.mjs --publicar   # prepara latest.json y crea la release
```

`publicar.mjs` se niega a seguir si `package.json` y `Cargo.toml` no dicen la misma versión, o si el
`.sig` es más viejo que el instalador, que es lo que pasa cuando compilas sin la clave.

Después de publicar, `node scripts/verificar-firma.mjs` se baja lo que sirve la release de verdad y
comprueba la firma contra la clave pública que lleva la app dentro, con ed25519 sobre BLAKE2b-512.
Firmar con otra clave es el fallo que no da ningún error: el actualizador rechaza en silencio cada
descarga y ninguna copia instalada se mueve.

Los manifiestos de winget están en [`packaging/winget`](packaging/winget).

</details>

## Licencia

[MIT](LICENSE). Úsalo, cámbialo y véndelo si quieres. Hecho por
[Munir Torres](https://munito.dev).

<br>

<p align="center">Hecho por <a href="https://munito.dev"><b>Munir Torres</b></a>. También suyo: <a href="https://adeorq.com">Adeorq</a> · <a href="https://vocript.app">VoCript</a> · <a href="https://github.com/Mun1to/Expoal">Expoal</a> · <a href="https://github.com/Mun1to/hotkeyconfig">hotkeyconfig</a> · <a href="https://github.com/Mun1to/froede">froede</a></p>
