# winshotx para agentes de IA

*[Read in English](AGENTS-API.md)*

Un agente de IA (Claude Code, Codex, Cursor o cualquiera que hable MCP) puede usar winshotx por su
cuenta: **grabar la pantalla, parar, sacar un MP4 o un GIF ya montado y hacer capturas**, sin que
nadie toque el ratón, y se entera de dónde ha quedado cada archivo. Hace falta winshotx 0.4.5 o
posterior.

Hay dos puertas, y las dos hacen lo mismo:

- **`winshotx.exe mcp`**, un servidor MCP por la entrada y la salida estándar. Es la que se le da a
  un agente: enseña sus herramientas, y el agente lee para qué sirve cada una.
- **`winshotx.exe agent <orden>`**, para guiones y terminales. Escribe una línea de JSON con la
  respuesta y sale con 0 o 1.

Las dos son clientes de la app de winshotx que vive en la bandeja, que es la que graba y exporta. Si
no está abierta, la primera llamada la arranca por detrás, y por eso esa tarda unos segundos más.
Nada sale del ordenador: hablan con la app por una tubería local que solo puede abrir tu mismo
usuario de Windows.

## Dónde está winshotx.exe

| Cómo lo instalaste | Ruta |
|---|---|
| El instalador de GitHub, o `winget install Mun1to.winshotx` | `%LOCALAPPDATA%\winshotx\winshotx.exe`, salvo que eligieras otra carpeta en el instalador |
| La portable en zip | Donde la descomprimiste |
| La Microsoft Store | Todavía no: el paquete de la Store no tiene línea de órdenes propia. Para agentes, usa el instalador o la portable |

El instalador apunta la carpeta que usó, así que esta línea de PowerShell encuentra el exe sea cual
sea la carpeta que elegiste (con la portable, escribe tú la ruta):

```powershell
$wx = Join-Path (Get-ItemProperty 'HKCU:\Software\munir\winshotx').'(default)' 'winshotx.exe'
```

## Añadirlo a tu agente

Las órdenes de abajo son para PowerShell y usan el `$wx` de la línea de arriba.

### Claude Code

```powershell
claude mcp add --scope user winshotx -- $wx mcp
```

`--scope user` lo deja puesto en todos los proyectos; sin él, se añade solo al proyecto en el que
estás. `claude mcp list` lo enseña, y `/mcp` dentro de Claude Code enseña sus herramientas. Claude
Code espera lo que haga falta a cada herramienta, así que una exportación larga no necesita nada más.

### Codex

```powershell
codex mcp add winshotx -- $wx mcp
```

Después dale más tiempo a cada llamada en `~/.codex/config.toml` (`%USERPROFILE%\.codex\config.toml`):
según la versión, Codex corta una herramienta a los 60 o a los 300 segundos, y exportar una grabación
larga puede tardar más. El bloque queda así:

```toml
[mcp_servers.winshotx]
command = 'C:\Users\<tú>\AppData\Local\winshotx\winshotx.exe'
args = ["mcp"]
tool_timeout_sec = 600
```

### Cursor

En `~/.cursor/mcp.json` para todos los proyectos, o en `.cursor/mcp.json` para uno. `${userHome}`
es tu carpeta de usuario; si instalaste winshotx en otro sitio, pon esa ruta en `command`.

```json
{
  "mcpServers": {
    "winshotx": {
      "type": "stdio",
      "command": "${userHome}\\AppData\\Local\\winshotx\\winshotx.exe",
      "args": ["mcp"]
    }
  }
}
```

### Cualquier otro cliente de MCP

Un servidor local por la entrada y la salida estándar: la orden es la ruta de `winshotx.exe` y el
único argumento es `mcp`. Habla la revisión `2025-06-18` de MCP (los clientes de revisiones más
nuevas vuelven a ella), un mensaje de JSON-RPC por línea, y solo tiene herramientas.

## Las herramientas

Los nombres son los de la API, en inglés, porque los usan agentes de cualquier sitio.

| Herramienta | Qué hace | Argumentos |
|---|---|---|
| `status` | Si winshotx está abierto y si está grabando ahora mismo | ninguno |
| `screens` | Las pantallas, con su índice, posición, tamaño, escala y cuál es la principal | ninguno |
| `windows` | Las ventanas visibles, con su id, título, programa y medidas | ninguno |
| `capture` | Una captura en PNG de una pantalla, una ventana o una zona, sin overlay y sin sonido | un objetivo; `output` |
| `record_start` | Empieza a grabar una pantalla, una ventana o una zona, y contesta cuando ya ha empezado de verdad | un objetivo; `camera`, `systemAudio`, `microphone`, `cue`, `format`, `maxSeconds` |
| `record_stop` | Para y guarda la grabación, sin abrir el editor | ninguno |
| `export` | Convierte una grabación guardada en un MP4 o un GIF, con el puntero dibujado, los aros de los clics, el zoom a los clics y el fondo | `session`, `output`; `style`, `trim`, `zoomOnClicks`, `speed` |
| `open_in_editor` | Abre una grabación en el editor para que la persona la retoque a mano | `session` |

**El objetivo** es uno solo de estos: `screen` (un índice de `screens`), `windowId` (un id de
`windows`), `windowTitle` (un trozo del título de una ventana) o `region` (`{x, y, width, height}`).
Las coordenadas son píxeles físicos del escritorio virtual, las mismas que devuelven `screens` y
`windows`, así que pueden ser negativas en una pantalla a la izquierda o encima de la principal.

**Las rutas:** `output` acaba en `.png` en `capture`, y en `.mp4` o `.gif` en `export`, que saca de
ahí el formato. Una ruta relativa se toma desde la carpeta de trabajo del agente. Sin `output`, una
captura va a la carpeta que digan los ajustes de winshotx. Cada respuesta da la ruta completa del
archivo que ha escrito.

**Lo de serie:** sin `camera`, `systemAudio` ni `microphone`, una grabación usa lo que digan los
ajustes. `cue` (el sonido corto de empezar y parar de winshotx) va apagado salvo que el agente lo
pida. Sin `style`, `export` usa el mismo aspecto con el que se abre el editor; `style` es el nombre
de un estilo que la persona guardó en el editor. `zoomOnClicks` va de 0 (sin zoom) a 1, y `speed` es
1 para la velocidad real (el editor ofrece de 0,5 a 4).

## Un vídeo, paso a paso

Lo que hace un agente para grabar la demo de una página web y devolver un MP4:

1. `windows`, para encontrar el navegador: `{"ok":true,"windows":[{"id":1180,"title":"Mi app - Edge","process":"msedge.exe",...}]}`
2. `record_start` con `{"windowId": 1180, "maxSeconds": 120}`: `{"ok":true,"session":"3f9c2a7b1d"}`
3. Hace clic y escribe a lo largo de la demo. Los aros de los clics y el zoom vienen después, de los
   clics que winshotx apuntó mientras grababa.
4. `record_stop`: `{"ok":true,"session":"3f9c2a7b1d","seconds":41.2}`
5. `export` con `{"session": "3f9c2a7b1d", "output": "demo.mp4", "zoomOnClicks": 0.5}`:
   `{"ok":true,"file":"C:\\trabajo\\mi-app\\demo.mp4","seconds":41.2,"bytes":5321456}`

`maxSeconds` es un tope de seguridad: si al agente se le olvida parar, la grabación se acaba sola.
`record_stop` después sigue funcionando y espera a que la grabación esté guardada.

## Desde la línea de órdenes: `winshotx.exe agent`

Las mismas órdenes, con opciones en vez de JSON: `status`, `screens`, `windows`, `capture`,
`record`, `stop`, `export` y `open`. Cada una escribe una línea de JSON (la misma respuesta que dan
las herramientas de MCP) y sale con 0 si ha ido bien y con 1 si no. `winshotx.exe agent --help`
enseña todas las opciones.

```powershell
$wx = Join-Path (Get-ItemProperty 'HKCU:\Software\munir\winshotx').'(default)' 'winshotx.exe'
$r = & $wx agent record --screen 0 --max-seconds 60 | ConvertFrom-Json
# ... lo que quieras enseñar ...
& $wx agent stop
# {"ok":true,"session":"3f9c2a7b1d","seconds":12.3}
& $wx agent export --session $r.session --out "$PWD\demo.mp4" --zoom-on-clicks 0.5
# {"ok":true,"file":"C:\\trabajo\\demo.mp4","seconds":12.3,"bytes":1843200}
```

Las opciones: un objetivo (`--screen 0`, `--window "Edge"` o `--region x,y,ancho,alto`), `--out`,
`--camera`, `--mic`, `--system-audio`, `--cue`, `--format gif`, `--max-seconds 30`, `--style`,
`--trim 1.5,9`, `--zoom-on-clicks 0.5` y `--speed 2`.

`winshotx.exe` es una aplicación de escritorio, no un programa de consola, así que **lee lo que
contesta** (pásalo a otra orden o guárdalo en una variable, como arriba): eso es lo que hace que
PowerShell espere a la respuesta.

## Lo que ve quien está delante del ordenador

- **Una grabación que empieza un agente se ve igual que una tuya**: el marco rojo alrededor de lo que
  se graba y la barra pequeña con el contador. winshotx nunca graba a escondidas. La persona puede
  pararla desde la barra como cualquier otra grabación.
- **No suena nada** salvo que el agente pase `cue`.
- Una captura no enseña nada: ni overlay, ni destello, ni sonido.
- El editor solo se abre cuando el agente llama a `open_in_editor`. Exportar no abre ninguna ventana.

## Respuestas y errores

Cada respuesta es un objeto JSON. Si ha ido bien, lleva `"ok":true`; si no, es
`{"ok":false,"error":"..."}`, con una frase en inglés que dice qué hacer (por ejemplo, que ya hay una
grabación en marcha). Por MCP, una llamada que falla vuelve con `isError: true` y el mismo JSON como
texto, para que el agente lo lea y lo intente otra vez.

## Tiempos de espera

`record_stop` y `export` contestan cuando el trabajo está hecho. Exportar una grabación larga puede
tardar más o menos lo que dura la grabación (un tutorial de 20 segundos en 1080p tarda unos 11 s),
así que si tu agente corta antes las llamadas, dale más tiempo. Claude Code ya espera; en Codex es
`tool_timeout_sec`, como arriba.
