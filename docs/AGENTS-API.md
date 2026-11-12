# winshotx for AI agents

*[Leer en español](AGENTS-API.es.md)*

An AI agent (Claude Code, Codex, Cursor or any other that speaks MCP) can use winshotx on its own:
**record the screen, stop, export a finished MP4 or GIF, and take screenshots**, without anybody
touching the mouse, and it is told where each file ended up. It needs winshotx 0.4.5 or newer.

There are two ways in, and both do the same:

- **`winshotx.exe mcp`**, an MCP server over stdio. This is the one to give an agent: it lists its
  tools, and the agent reads what each one is for.
- **`winshotx.exe agent <command>`**, for scripts and terminals. It writes one line of JSON with
  the answer and exits with 0 or 1.

Both are clients of the winshotx app that lives in the system tray, which is the one that records
and exports. If it is not running, the first call starts it in the background, so that call takes
a few seconds longer. Nothing leaves the computer: they talk to the app through a local pipe that
only your own Windows user can open.

## Where winshotx.exe is

| How you installed it | Path |
|---|---|
| The installer from GitHub, or `winget install Mun1to.winshotx` | `%LOCALAPPDATA%\winshotx\winshotx.exe`, unless you picked another folder in the installer |
| The portable zip | Wherever you unzipped it |
| The Microsoft Store | Not yet: the Store package has no command line of its own. Use the installer or the portable build for agents |

The installer writes down the folder it used, so this PowerShell line finds the exe whichever folder
you picked (with the portable build, write the path yourself instead):

```powershell
$wx = Join-Path (Get-ItemProperty 'HKCU:\Software\munir\winshotx').'(default)' 'winshotx.exe'
```

## Add it to your agent

The commands below are for PowerShell and use `$wx` from the line above.

### Claude Code

```powershell
claude mcp add --scope user winshotx -- $wx mcp
```

`--scope user` makes it available in every project; leave it out to add it to the current
project only. `claude mcp list` shows it, and `/mcp` inside Claude Code shows its tools. Claude Code
waits as long as a tool needs, so a long export needs nothing else.

### Codex

```powershell
codex mcp add winshotx -- $wx mcp
```

Then give it more time per call in `~/.codex/config.toml` (`%USERPROFILE%\.codex\config.toml`):
depending on the version, Codex cuts a tool call after 60 to 300 seconds, and exporting a long
recording can take longer. The block ends up like this:

```toml
[mcp_servers.winshotx]
command = 'C:\Users\<you>\AppData\Local\winshotx\winshotx.exe'
args = ["mcp"]
tool_timeout_sec = 600
```

### Cursor

In `~/.cursor/mcp.json` for every project, or `.cursor/mcp.json` for one. `${userHome}` is your
user folder; if you installed winshotx somewhere else, put that path in `command`.

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

### Any other MCP client

A local stdio server: the command is the path to `winshotx.exe` and the only argument is `mcp`.
It speaks MCP revision `2025-06-18` (clients of newer revisions fall back to it), one JSON-RPC
message per line, and has tools only.

## The tools

| Tool | What it does | Arguments |
|---|---|---|
| `status` | Whether winshotx is running and recording right now | none |
| `screens` | The screens, with index, position, size, scale and which one is primary | none |
| `windows` | The visible windows, with id, title, process and bounds | none |
| `capture` | A PNG screenshot of a screen, a window or a region, with no overlay and no sound | a target; `output` |
| `record_start` | Starts recording a screen, a window or a region, and answers once it has really started | a target; `camera`, `systemAudio`, `microphone`, `cue`, `format`, `maxSeconds` |
| `record_stop` | Stops and saves the recording, without opening the editor | none |
| `export` | Turns a saved recording into an MP4 or GIF, with the drawn cursor, click rings, zoom on clicks and background | `session`, `output`; `style`, `trim`, `zoomOnClicks`, `speed` |
| `open_in_editor` | Opens a recording in the editor so the person can polish it by hand | `session` |

**The target** is exactly one of: `screen` (an index from `screens`), `windowId` (an id from
`windows`), `windowTitle` (part of a window title) or `region` (`{x, y, width, height}`).
Coordinates are physical pixels of the virtual desktop, the same ones `screens` and `windows`
return, so they can be negative on a screen to the left of or above the primary one.

**Paths:** `output` ends in `.png` for `capture`, and in `.mp4` or `.gif` for `export`, which picks
the format from it. A relative path is taken from the agent's working directory. Without `output`,
a screenshot goes to the folder set in winshotx's settings. Every answer gives the absolute path of
the file it wrote.

**Defaults:** without `camera`, `systemAudio` or `microphone`, a recording uses what the settings
say. `cue` (winshotx's short start and stop sound) is off unless the agent asks for it. Without
`style`, `export` uses the same look the editor opens with; `style` is the name of a style the
person saved in the editor. `zoomOnClicks` goes from 0 (no zoom) to 1, and `speed` is 1 for real
time (the editor offers 0.5 to 4).

## Making a video, step by step

What an agent does to record a demo of a web page and hand back an MP4:

1. `windows`, to find the browser: `{"ok":true,"windows":[{"id":1180,"title":"My app - Edge","process":"msedge.exe",...}]}`
2. `record_start` with `{"windowId": 1180, "maxSeconds": 120}`: `{"ok":true,"session":"3f9c2a7b1d"}`
3. It clicks and types through the demo. The click rings and the zoom come later, from the clicks
   winshotx wrote down while recording.
4. `record_stop`: `{"ok":true,"session":"3f9c2a7b1d","seconds":41.2}`
5. `export` with `{"session": "3f9c2a7b1d", "output": "demo.mp4", "zoomOnClicks": 0.5}`:
   `{"ok":true,"file":"C:\\work\\my-app\\demo.mp4","seconds":41.2,"bytes":5321456}`

`maxSeconds` is a safety limit: if the agent forgets to stop, the recording ends on its own.
`record_stop` afterwards still works and waits until the recording is saved.

## From the command line: `winshotx.exe agent`

The same commands, with options instead of JSON: `status`, `screens`, `windows`, `capture`,
`record`, `stop`, `export` and `open`. Each one writes one line of JSON (the same answer the MCP
tools return) and exits with 0 if it went well and 1 if not. `winshotx.exe agent --help` lists every
option.

```powershell
$wx = Join-Path (Get-ItemProperty 'HKCU:\Software\munir\winshotx').'(default)' 'winshotx.exe'
$r = & $wx agent record --screen 0 --max-seconds 60 | ConvertFrom-Json
# ... do what you want to show ...
& $wx agent stop
# {"ok":true,"session":"3f9c2a7b1d","seconds":12.3}
& $wx agent export --session $r.session --out "$PWD\demo.mp4" --zoom-on-clicks 0.5
# {"ok":true,"file":"C:\\work\\demo.mp4","seconds":12.3,"bytes":1843200}
```

The options: a target (`--screen 0`, `--window "Edge"` or `--region x,y,width,height`), `--out`,
`--camera`, `--mic`, `--system-audio`, `--cue`, `--format gif`, `--max-seconds 30`, `--style`,
`--trim 1.5,9`, `--zoom-on-clicks 0.5` and `--speed 2`.

`winshotx.exe` is a desktop app, not a console program, so **read its output** (pipe it or assign
it, as above): that is what makes PowerShell wait for the answer.

## What the person at the computer sees

- **A recording started by an agent looks exactly like one of yours**: the red frame around what
  is being recorded and the small bar with the timer. winshotx never records in secret. The person
  can stop it from the bar like any other recording.
- **Nothing makes a sound** unless the agent passes `cue`.
- A screenshot shows nothing at all: no overlay, no flash and no sound.
- The editor only opens when the agent calls `open_in_editor`. Exporting does not open any window.

## Answers and errors

Every answer is a JSON object. When it went well it starts with `"ok":true`; when it did not, it is
`{"ok":false,"error":"..."}`, with a sentence that says what to do (for example, that a recording
is already running). Through MCP, a failed call comes back with `isError: true` and the same JSON
as text, so the agent can read it and try again.

## Timeouts

`record_stop` and `export` answer when the work is done. Exporting a long recording can take about
as long as the recording itself (a 20 second 1080p tutorial takes around 11 s), so if your agent
cuts tool calls short, give it more time. Claude Code already waits; in Codex it is
`tool_timeout_sec`, as above.
