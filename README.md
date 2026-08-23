<div align="center">

**English** · [Español](README.es.md)

# winshotx

**Screenshots and GIF/MP4 screen recording for Windows.**
Local, no account, no cloud, no bundled binaries.

[![MIT license](https://img.shields.io/badge/license-MIT-3b82f6)](LICENSE)
[![Windows 10/11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4)](#status)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.11-ffc131)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-1.82%2B-dea584)](https://www.rust-lang.org)
[![2 MB installer](https://img.shields.io/badge/installer-2%20MB-22c55e)](#install)

The look and flow of CleanShot X, the frame by frame editing of ScreenToGif and the global
shortcuts of ShareX, in a 2 MB installer.

[![Download winshotx for Windows](https://img.shields.io/badge/download-for%20Windows-0078d4?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/Mun1to/winshotx/releases/latest/download/winshotx-setup.exe)

<img src="docs/img/ajustes.png" alt="winshotx settings panel" width="760">

</div>

> **The interface is in Spanish.** The code, the commits and this page are in English, but every
> label you see in the app is Spanish. Translating it is not planned; open an issue if you want it.

## What it does

- **Region capture** over a frozen screenshot, with a pixel magnifier showing the exact colour in
  hex and automatic snapping to system windows: one click on a window selects the whole thing.
- **Region recording** at 15, 30 or 60 fps through Windows Graphics Capture, with no FFmpeg and no
  overlays leaking into the video.
- **Editor** with a thumbnail strip, A/B trimming, looped playback, scaling with locked aspect
  ratio and a quality control.
- **Export** to GIF, MP4 or PNG, to disk and to the clipboard: an image pastes as an image, and a
  GIF or MP4 pastes as a file into Slack, Discord or Explorer.
- **Everything stays local.** No account, no telemetry, no uploads. What you capture never leaves
  your machine.

## Install

[**Download the installer**](https://github.com/Mun1to/winshotx/releases/latest/download/winshotx-setup.exe)
· 2.2 MB · installs for your user only and never asks for administrator rights. Older versions are
in [Releases](../../releases).

Since 0.1.2 it updates itself: **Ajustes → Actualizaciones** shows a button when a new version is
out, and one click downloads it, installs it and restarts the app. Downloads are signed and the
signature is verified before anything is installed, so a tampered file is rejected.

It lives in the system tray with no window of its own. Windows hides new tray icons, so if you
cannot see it, look behind the `^` arrow on the taskbar.

## Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+Shift+2` | Capture a region |
| `Ctrl+Shift+5` | Record a region · press again to stop |
| `Enter` | Copy the selection to the clipboard |
| `Ctrl+S` | Save the selection |
| `E` | Open the selection in the editor |
| `G` / `V` | Record the selection as GIF / video |
| `M` | Mute or unmute the audio |
| `Ctrl+A` | Select the whole monitor |
| `←↑→↓` | Move the selection · `Shift` for steps of 10 · `Alt` resizes |
| `Esc` | Cancel |

In the editor: `space` plays, `I` and `O` mark the start and end of the trim, `←` `→` step through
frames, `Ctrl+S` exports with whatever the panel has set, and `Esc` closes.

Both global shortcuts can be changed in the settings by clicking them and typing the new
combination. If another application already holds it, the field turns red and says so.

## How it is built

| Layer | Choice | Why |
|---|---|---|
| Desktop | [Tauri 2](https://tauri.app) | small binary, system webview, Rust backend |
| Updates | Tauri `updater` plugin, minisign signatures | one button, without leaving the app or installing blind |
| Interface | React 19 + Vite + Tailwind 4 + framer-motion | independent windows, native feeling animation |
| Still capture | [`xcap`](https://crates.io/crates/xcap) | enumerates monitors and windows with their real coordinates |
| Recording | [`windows-capture`](https://crates.io/crates/windows-capture) | Windows Graphics Capture, 60 fps at no CPU cost |
| MP4 | Media Foundation, hardware H.264 | 0 MB of dependencies, system acceleration |
| GIF | [`gif`](https://crates.io/crates/gif) + [`color_quant`](https://crates.io/crates/color_quant) | global palette, dithering and frame differencing |
| Editing cache | lossless [QOI](https://qoiformat.org) | fast to write and editable frame by frame |

### Two decisions that explain the rest

**The selection overlay is not a transparent window.** The screen is captured, shown frozen, and
the selection happens on top of that image. It sidesteps the Tauri v2 transparency bug on Windows,
removes the flicker of moving content underneath, and gives a pixel exact magnifier for free.

**No bundled FFmpeg.** MP4 is written by Media Foundation in hardware, and the GIF is produced in
pure Rust with a global palette, Floyd–Steinberg dithering and writing only the rectangle that
changed between frames. If you happen to have `ffmpeg` on your `PATH`, the editor offers a maximum
quality engine (`palettegen`) as well, but it is never downloaded and never shipped.

## Development

```bash
pnpm install
pnpm approve-builds --all
pnpm tauri dev      # starts the app (it lives in the system tray)
pnpm tauri build    # NSIS installer in target/release/bundle/nsis
```

Publishing a version needs the private signing key, which is **not in this repository** and without
which the updater would reject the download:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/winshotx.key)"
pnpm tauri build
node scripts/publicar.mjs --publicar   # writes latest.json and creates the release
```

Backend tests:

```bash
cd src-tauri
cargo test
```

The integration tests are not pretend: they capture the real screen, record a clip through Windows
Graphics Capture and export GIF and MP4 files that are then read back to check they are valid.

[`docs/TRAMPAS.md`](docs/TRAMPAS.md) (in Spanish) collects the seven Tauri v2 traps on Windows that
cost hours of debugging: synchronous commands that freeze the interface, window labels that cannot
be reused, the first click being eaten by the system, a canvas tainted by the `asset:` protocol, an
incomplete CSP that only shows up in the installer, and the signing key that is passed one way and
not the other. Read it before touching windows or webview security.

## Status

Runs on Windows 10 1903 or newer. macOS and Linux compile, but the capture functions return "not
implemented": the backend sits behind a `CaptureBackend` trait, so adding them means writing
`capture/mac.rs` and `capture/linux.rs`.

**What is missing:** system audio is not recorded yet. It needs WASAPI in loopback mode to feed the
encoder; the switch is already in the interface, disabled and saying so.

## License

[MIT](LICENSE). Use it, change it and sell it if you want.
