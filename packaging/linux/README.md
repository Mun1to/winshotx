# packaging/linux

**winshotx is not released for Linux yet.** There are no official packages and no download. The
CI builds a .deb, an .rpm and an AppImage on Debian 12 only to test them. This page is for people
packaging or building the app from source.

## Building from source

The system libraries, on Debian 12 (the base the CI builds on):

```
apt-get install pkg-config clang libclang-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev \
  libdbus-1-dev libpipewire-0.3-dev libgbm-dev libdrm-dev libwayland-dev \
  libxcb1-dev libxcb-randr0-dev libxcb-shm0-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  patchelf file xdg-utils
```

The Rust tests write real MP4 files, so they also need the encoders at run time:
`gstreamer1.0-plugins-base`, `gstreamer1.0-plugins-good` and `gstreamer1.0-plugins-bad`.

Then, with Rust, Node.js 24 and pnpm 11:

```
pnpm install --frozen-lockfile
pnpm tauri build --bundles deb,rpm,appimage
```

The packages are signed for the updater, so that last command needs `TAURI_SIGNING_PRIVATE_KEY`
in the environment.

## What has been tested

Everything below ran in a virtual session on Fedora 44, never on a physical machine.

| Desktop | Result |
|---|---|
| GNOME 50, Wayland | Region capture, copy and paste into another app, save to `~/Pictures/winshotx`, pin, the editor, the history (open, send to the trash), dragging the thumbnail into another app, Esc, both shortcuts through the GlobalShortcuts portal, and two monitors |
| X11 with openbox or xfwm4 | Region capture, copy and paste into another app, save to `~/Pictures/winshotx`, pin, the editor, the history (open, send to the trash), dragging the thumbnail into another app, Esc and the thumbnail stack |

Not tested yet: KDE Plasma, Hyprland, mouse input on Sway, more than one monitor on X11, display
scaling, NVIDIA drivers and real hardware. Screen recording has its own list below.

The packages the CI builds were installed in clean containers: the .deb on Debian 12 and 13 and
on Ubuntu 24.04 and 26.04, the .rpm on Fedora 44 and 45, and the AppImage on Debian 12 and
Ubuntu 24.04. They install without asking for anything else and the app starts. On Fedora 44 the
AppImage opens a blank window (in that virtual session WebKitGTK cannot create its EGL display):
use the .rpm there.

Opening an entry of the history goes through `xdg-open`, so the .deb and the .rpm depend on
`xdg-utils` (`bundle.linux.*.depends` in `tauri.conf.json`). The AppImage cannot depend on
anything: it uses the `xdg-open` of the system.

On Wayland the desktop decides where windows go, so the thumbnail appears where the compositor
puts it (centred on GNOME) and not in the bottom right corner.

On GNOME, the first launch asks to confirm the shortcuts ("Add Keyboard Shortcuts") and the first
capture asks for permission ("Allow winshotx to Take Screenshots?").

## Screen recording

Frames come from MIT-SHM on X11, and from the ScreenCast portal plus PipeWire on Wayland. Both
feed the same frame cache as on Windows, so the editor, trimming and the GIF export are the same
code.

**MP4 comes from the system's GStreamer.** winshotx ships no H.264 encoder: the format has
patents and x264 is GPL. It writes MP4 with the GStreamer that every system with WebKitGTK
already has, through `openh264enc` (Cisco's codec, which each distribution distributes itself)
and whichever AAC encoder is installed (`fdkaacenc`, `voaacenc` or `avenc_aac`). `x264enc` is
never used. The same library reads the videos chosen as animated backgrounds and the custom
click sounds that are not WAV.

The packages only *recommend* the plugins, so they install even where those are not available:

| | Recommends | What a clean install with WebKitGTK already has |
|---|---|---|
| .deb | `gstreamer1.0-plugins-good`, `gstreamer1.0-plugins-bad` | Debian 12 and 13: everything. Ubuntu 22.04, 24.04 and 26.04: no H.264 at all (neither encoder nor decoder) until `gstreamer1.0-plugins-bad` is installed |
| .rpm | `gstreamer1-plugins-good`, `gstreamer1-plugins-bad-free`, `gstreamer1-plugin-openh264` | Fedora 44 and 45: everything except `gstreamer1-plugin-openh264` (38 KiB, from the Cisco repository Fedora enables by default) |

On Debian and Ubuntu, `gstreamer1.0-plugins-bad` itself depends on libx265 (GPL) for another
plugin; winshotx neither links nor loads it.

When the H.264 encoder is missing, the export panel says so before saving, with the command
that installs it (`sudo apt install gstreamer1.0-plugins-bad` or
`sudo dnf install gstreamer1-plugin-openh264`), the MP4 button is disabled and GIF keeps
working. Without an AAC encoder the MP4 is saved without sound, and the panel says that too.
Installing the package while winshotx is running is enough: it looks again when its window
gets the focus.

**The AppImage cannot write MP4.** It carries its own GStreamer core, the one from Debian 12
where CI builds it, and no plugins, so no encoder travels inside it. That core looks for plugins
next to itself and finds none, not even `playbin` or an audio sink: there is no MP4, and
WebKitGTK has nothing to play the preview or the sound tracks with. The export panel says so and
names the .deb and .rpm packages, instead of a command that would change nothing. GIF, PNG and
JPG do not go through GStreamer.

Pointing that core at the system's plugin folder (`GST_PLUGIN_SYSTEM_PATH_1_0`) was measured
with the AppImage CI built, and winshotx does not do it. It loads the plugins and writes an MP4
on Debian 12 and Ubuntu 22.04. It loads none on Ubuntu 24.04, Debian 13 and Fedora 44, whose
plugins are built for a newer GLib and GStreamer than the ones inside the AppImage
(`undefined symbol: g_once_init_enter_pointer`, `incompatible version (plugin: 1.24, gst: 1.22)`).

**The desktop asks once.** On Wayland the desktop asks which screen to share. winshotx keeps the
restore token the portal returns in `screencast-token`, in the app's config directory, so the
next recording starts without the question. The file is created with mode 600 and the token is
never written to a log: whoever has it can record the screen without being asked. If the desktop
rejects the token it asks again; winshotx then stores the new token, or deletes the file if the
question is cancelled. On GNOME there is only a token when "Remember this selection" is ticked
in the dialog; without it GNOME asks on every recording.

**There is not always a recording bar.** Linux has no way to keep a window out of a recording, so
on X11 the bar is placed outside the recorded area, and there is no bar at all when the whole
screen is recorded or on Wayland (where a window cannot choose its position). Such a recording
is stopped with the record shortcut, from the tray icon, with `winshotx --record`, or from the
desktop's own screen sharing indicator (GNOME, KDE).

**How to stop is said before recording, not during.** Anything winshotx showed while recording
(a notification, a window) would end up in the video, so it shows nothing. Instead the region
picker says how to stop, in one line under the mode bar, whenever video or GIF is selected. The
line names only what that desktop has: no tray on stock GNOME, no shortcut if the desktop has not
set one, and `winshotx --record` when there is neither.

**No screenshots while recording.** On Windows the capture screen and its thumbnail are kept out
of the video, so a screenshot can be taken in the middle of a recording. Linux cannot do that:
while a recording is running the capture shortcut does nothing, and a thumbnail that was on
screen is closed when the recording starts.

**Sound needs PipeWire.** System audio and the microphone are captured through PipeWire, the
library that already brings the Wayland frames. Where PipeWire runs but does not carry audio
(Ubuntu 22.04 and Linux Mint 21 use PulseAudio for sound), recording works without sound, and
the two switches in Settings are off with a line that says why. Sound on PulseAudio-only systems
is not supported yet.

**The editor plays media from a local server.** WebKitGTK plays video and audio through
GStreamer, which cannot open Tauri's `asset:` protocol: without help the sound tracks of a
recording never load in the editor. So on Linux the editor asks a small HTTP server inside
winshotx for them. It listens on 127.0.0.1 on a random port, only answers requests for its own
`Host`, needs a random token in the path (never logged, also stripped from any error message
the page reports), and only serves video and audio files from two folders: the recordings in
the app's cache and the animated backgrounds the user imported. It lives as long as winshotx
does, with one token per launch. A request has two seconds in total to arrive, and at most 32
are served at once. What the page still fails to load is written to the app log.

**What was tried**, in the same virtual session on Fedora 44:

- X11 (openbox): a region and the whole screen to GIF, a region to video with the editor
  opening afterwards, the bar outside the recorded area, the pointer drawn by the exporter.
- GNOME 50 on Wayland: a region to GIF, with the desktop's question the first time and without
  it the second; a token the desktop rejects, with the question cancelled and answered; and
  stopping from GNOME's screen sharing indicator, which saves the recording.
- Sound, with a virtual output and a virtual microphone: each lands in its own track and lasts
  as long as the video. Without audio devices, and without PipeWire running, the recording is
  saved without sound.
- MP4, on X11 and on GNOME: a region recorded as video opens the editor with its preview, the
  preview plays, and saving writes an H.264 MP4. That file was read back frame by frame: it
  lasts as long as the recording and shows the same colours at the same instants as the
  preview. Recorded with the virtual output and microphone, the editor plays both tracks and
  the MP4 carries both, mixed, in AAC.
- A video imported as an animated background, on X11: it moves behind the recording in the
  saved MP4.
- With the H.264 plugin hidden from GStreamer, on X11 and on GNOME: no preview, the export
  panel shows the command for that distribution (seen in English and Spanish, with the Fedora
  and the Ubuntu command), the MP4 buttons and Ctrl+S do nothing, and GIF is still saved. With
  the AAC encoder hidden, the MP4 is saved without sound and the panel says so.
- The packages, installed in clean containers the way people install them
  (`apt install ./winshotx.deb`, `dnf install ./winshotx.rpm`), on Debian 12, Ubuntu 22.04, 24.04
  and 26.04 and Fedora 44: the recommended plugins come with them, and afterwards GStreamer has
  `openh264enc` and an AAC encoder and writes an MP4 with them. `x264enc` is not installed.
  Without recommended packages (Ubuntu 24.04 and Fedora 44) the package still installs, and
  there is no encoder.
- An AppImage, simulated on X11 (the `APPIMAGE` variable set and an empty plugin folder): the
  export panel says that this AppImage cannot use the system's encoders and names the .deb and
  .rpm packages, in English and Spanish.

Not tried: KDE Plasma, Sway and Hyprland, more than one monitor, display scaling, 30-bit
screens, real sound hardware, and PulseAudio-only systems (Ubuntu 22.04, Linux Mint 21) beyond
a PipeWire without audio devices. Hardware encoders (VA-API) are not used: the MP4 is encoded
on the CPU. A custom click sound in MP3 is read through GStreamer, which is only covered by a
test with a real file: clicks are not recorded on Linux yet, so there is nothing to hear it in.
An AppImage built from this code has not been run: what is said about it comes from the one CI
built before, unpacked, and from its GStreamer core loaded by itself.

## Reading text

On Windows the text reader comes with the system. Linux has none, and one bundled inside would
weigh more than winshotx itself, so winshotx runs the `tesseract` command of the system
(Apache-2.0). It is behind `T` in the region picker and on a pinned capture, "Copy text" and
"Hide data" in the editor (in a screenshot and across a whole recording), and the search box of
the history.

**The command is run, not linked.** Ubuntu 22.04 ships Tesseract 4 (`libtesseract4`) and the
others ship 5 (`libtesseract5`): a package depending on one would not install where the other
is. The command behaves the same in both. Checked in clean containers of Debian 12 and 13,
Ubuntu 22.04, 24.04 and 26.04 and Fedora 44 and 45 (versions 4.1.1 to 5.5.4).

**What it is given.** The image goes in grey through standard input, enlarged three times up to
2.2 megapixels and twice up to 5 (interface text is a third of the size Tesseract expects: at
its own size it read 25 of the 44 words of a settings window, enlarged 40 to 42), and inverted
when it is dark, because Tesseract 4 cannot read light text on a dark background. A very tall
image (a scrolling capture) is read in strips of 4,000 pixels. A read that takes more than 60
seconds is abandoned.

**Languages.** It reads with the languages of the system that are installed (`LANGUAGE`,
`LC_ALL`, `LC_MESSAGES`, `LANG`), plus English when it is there; with whatever is installed if
none of those is. With Spanish in front, Tesseract 5.5 reads the `@` of an e-mail address as an
`O` at these sizes, so "Hide data" missed the addresses. When English is installed and is not
the only language, a second pass in English alone runs at the same time, and only the e-mail
addresses are taken from it.

**The packages recommend it**, like the GStreamer plugins:

| | Recommends |
|---|---|
| .deb | `tesseract-ocr` (brings English), `tesseract-ocr-spa` |
| .rpm | `tesseract` (brings English), `tesseract-langpack-spa` |

Measured in clean containers, on top of what winshotx already installs (WebKitGTK and the
GStreamer plugins, some 300 MB of download): 9 to 10 MB of download and 25 to 28 MB of disk on
Debian 12 and 13 and Ubuntu 22.04, 24.04 and 26.04 (10 MB of it is `tesseract-ocr-osd`, which
`tesseract-ocr` depends on), and 5 MiB and 13 MiB on Fedora 44 and 45.

**When it is missing**, the app says so where the text was asked for, with the command for that
distribution and the language of the system (`sudo apt install tesseract-ocr tesseract-ocr-spa`,
`sudo dnf install tesseract tesseract-langpack-spa`); on other distributions it names the
package. The history has no search box until Tesseract is there: it looks again every time a
capture is saved or the history is opened, and then reads what was saved in the meantime.

**The AppImage uses the system's Tesseract too.** Nothing travels inside it. The command is
started without the AppImage's own library paths, like `xdg-open`. Measured with the AppImage
CI built, unpacked, and a probe in the place of its binary: with the environment its `AppRun`
leaves, the system's `tesseract` loads 13 to 32 libraries from inside the AppImage, and on
Ubuntu 26.04 it does not even start (`libsystemd.so.0: version 'LIBSYSTEMD_254' not found`).
Without those variables it loads none, and reads the same image the same way on the seven
distributions.

**What was tried**, in the same virtual session on Fedora 44 (Tesseract 5.5.4):

- X11 (openbox), with the system in Spanish: `T` in the region picker, on a pinned capture and in
  the editor, each pasted into another app, with the accents and the e-mail address whole.
  "Hide data" on a screenshot, whose saved PNG was read back (the rest of the text is there, the
  address and the phone number are not), and across a recording, whose MP4 was read back frame
  by frame. The history reads the saved captures and its search box finds one by a word typed
  without its accent.
- GNOME 50 on Wayland, with the system in English: the same gestures on a screenshot. With
  English alone there is no second pass.
- Without `tesseract` in the app's `PATH`, on X11 and on GNOME: the region picker, the pinned
  capture and the editor show the sentence and the whole command (Fedora's, and on X11 also
  Ubuntu's with a borrowed `os-release`), and the history has no search box. On X11, with the
  command made available while the app is running, opening the history again reads the two
  captures saved without it and shows the box.
- The command itself, in clean containers of the seven distributions above: it reads from
  standard input, answers the same twelve `tsv` columns, and fails with a clear message when a
  language is missing.

Not tried: a package built with the new `Recommends` (CI has to build it), an AppImage built
from this code reading text (what ran inside the one CI built before was a probe, not
winshotx), the app with Tesseract 4 (only the command was run there), and text in Chinese,
Japanese or right-to-left scripts.

## How the app identifies itself

On Linux the app presents itself to the desktop as `com.munir.winshotx` (the identifier in
`tauri.conf.json`), not as the binary name:

- GTK announces every window with that app id (`set_prgname` in `src-tauri/src/lib.rs`), so the
  desktop matches the windows to `com.munir.winshotx.desktop`.
- The app registers with xdg-desktop-portal under the same id (`platform/portal.rs`). The
  GlobalShortcuts portal refuses apps without an id, and GNOME only shows the screenshot
  permission dialog when `<id>.desktop` is the focused app, so both names must be the same.
- The portal only accepts the registration if it finds a desktop file with that exact name.

Files:

- `com.munir.winshotx.desktop`: the menu entry the .deb and the .rpm install (through
  `bundle.linux.*.files` in `tauri.conf.json`).
- `winshotx.desktop.hbs`: the template for the entry the Tauri bundler always writes, named after
  `productName`. It is kept hidden (`NoDisplay=true`) so the menu shows a single winshotx.

The AppImage installs nothing, so it writes a hidden `com.munir.winshotx.desktop` pointing to
itself into `~/.local/share/applications` at runtime.

## Shortcuts on desktops without the GlobalShortcuts portal

On X11 winshotx grabs its keys itself. On Wayland it asks the desktop through the GlobalShortcuts
portal (GNOME 48+, KDE Plasma). Sway and the other wlroots compositors have no such portal, so
the welcome screen and the settings show the commands to bind by hand instead:

```
winshotx --capture   # capture a region
winshotx --edit      # capture and open the editor
winshotx --record    # record a region, or stop the recording in progress
```

A second `winshotx` started with one of those flags hands it to the running one and exits, so
the key works only while winshotx is running. Sway does not read XDG autostart entries, so the
first line of this example starts it with the session (`~/.config/sway/config`):

```
exec winshotx --autostart
bindsym Print exec winshotx --capture
bindsym Shift+Print exec winshotx --edit
bindsym Ctrl+Print exec winshotx --record
```

For the AppImage, use the path of the `.AppImage` file instead of `winshotx`; the app shows the
exact command. The same commands work as custom shortcuts on any other desktop, and on Hyprland,
whose portal registers the shortcuts but leaves binding them to `hyprland.conf`.

Screenshots on Sway go through `xdg-desktop-portal-wlr`, which needs PipeWire running.
