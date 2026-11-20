# packaging/linux

**winshotx for Linux comes out with version 0.6.0**, for x86_64, as a .deb, an .rpm and an
AppImage. The three files are on the
[releases page](https://github.com/Mun1to/winshotx/releases/latest):

- **Debian, Ubuntu and their family:** `sudo apt install ./winshotx_0.6.0_amd64.deb`
- **Fedora:** `sudo dnf install ./winshotx-0.6.0-1.x86_64.rpm`
- **Anywhere else, the AppImage:** `chmod +x winshotx_0.6.0_amd64.AppImage` and run it. **The
  AppImage cannot write MP4**: there a recording is saved as a GIF (see "Screen recording"). Where
  the .deb or the .rpm can be installed, use that one.

`apt` and `dnf` also bring what the packages recommend: the GStreamer plugins that write the MP4
and Tesseract, which reads the text in an image.

The rest of this page is for people packaging or building the app from source. It says what was
tried and what was not.

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

Debug builds and `cargo test` compile `libspa` without its debug assertions
(`[profile.dev.package.libspa]` in `Cargo.toml`). In a debug build pipewire-rs asserts that every
"done" event carries the mark of an asynchronous call, and the PipeWire 0.3.48 of Ubuntu 22.04
sends one without it while winshotx lists the sound devices. The assertion fires inside a
callback that C calls, where a panic cannot unwind, so the process aborted at startup
(`assertion failed: is_async(val)`). Release builds never had that check: the packages were not
affected. Seen on Ubuntu 22.04 in a container: the debug build of the commit before aborts, and
the same code with that line starts and lists the devices.

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
capture asks for permission ("Allow winshotx to Take Screenshots?"). If the shortcuts question is
closed without adding them, winshotx does not ask again on its own: "Change" next to each shortcut
in Settings, and on the welcome screen, brings the question back (see "Shortcuts on Wayland").

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

**A video that cannot be saved as MP4 opens in the editor.** With "Open the editor when it ends"
turned off, a recording is saved on its own, with nothing opened. Where no MP4 can be written
(the AppImage, or a system without the H.264 encoder) a video used to stay in the temporary
folder: no file, no notice and no way back to it. Now it opens in the editor, whose export panel
says what is missing and saves it as a GIF. A GIF recording is still saved on its own. The rule
has its unit tests (`recorder::destino`); it has not been tried with the app yet.

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

**The frame around the recorded area is X11 only.** It is the same blue line as on Windows
(amber while paused), drawn by four plain X windows that the window manager does not manage
and the mouse goes through. For the same reason as the bar it always sits outside the recorded
area: a side that does not fit between the area and the edge of its screen is not drawn, and
there is no frame when the whole screen is recorded. With a compositor running the line has its
soft corners and its glow; without one the windows are cut to the shape of the line (the Shape
extension) and the glow is left out. The windows are of type `dock` with the class
`winshotx-marco`: compositors leave that type without a shadow, and a shadow of theirs would
fall inside the video. On Wayland there is no frame, since a window cannot choose its position
and the desktop already shows that the screen is being recorded.

**Clicks and shortcuts are recorded on X11 only.** While it records, winshotx asks the X server
sixty times a second where the pointer is and which buttons and keys are down (`QueryPointer`
and `QueryKeymap`). It is the same polling as on Windows, with no grab and no event hook, so no
other program loses a key or is slowed down. From it come the clicks (for the rings, the click
sound and the zoom), the shortcuts shown in the pill, the zoom key and the stretches of typing,
with the rules of Windows and in the same code: only shortcuts are kept, never what is typed,
and AltGr counts as typing. Keys are translated to Windows key codes with the keyboard layout in
use, and the table is rebuilt when the layout changes. A click outside the recorded area is not
kept: that is where the recording bar is, and pausing is not part of the video. On Wayland an
application cannot read the mouse buttons or the keyboard while another window has them, so a
recording made there has no clicks, no shortcuts and no zoom marks. Settings says so on the zoom
key, and the editor does not offer "Show the shortcuts" for a recording that has none.

**How to stop is said before recording, not during.** Anything winshotx showed while recording
(a notification, a window) would end up in the video, so it shows nothing. Instead the region
picker says how to stop, in one line under the mode bar, whenever video or GIF is selected. The
line names only what that desktop has: no tray on stock GNOME, no shortcut if the desktop has not
set one, and `winshotx --record` when there is neither.

**No screenshots while recording.** On Windows the capture screen and its thumbnail are kept out
of the video, so a screenshot can be taken in the middle of a recording. Linux cannot do that:
while a recording is running the capture shortcut does nothing, and a thumbnail that was on
screen is closed when the recording starts.

**Sound comes from PipeWire, or from PulseAudio where PipeWire does not carry it.** System audio
and the microphone are captured through PipeWire, the library that already brings the Wayland
frames. Where PipeWire runs but has no audio devices (Ubuntu 22.04 and Linux Mint 21 use
PulseAudio for sound), winshotx asks PulseAudio instead, through GStreamer's `pulsesrc`: one
stream for what is playing (the monitor of the default output) and one for the microphone. It
asks for 10 ms chunks and places each one by its timestamp, not by when it arrives: with a
player that keeps a two-second buffer, PulseAudio's default delivers the sound in bursts that
land up to 1.4 s away from where they played. The buffer it asks for is three seconds long.
That is not a delay: it is the most PulseAudio hands over in one piece, and a whole block of the
source has to fit in it, because not every source delivers 10 ms at a time (PulseAudio's test
source, `module-sine-source`, delivers two-second blocks). With a 200 ms buffer each of those
blocks arrived cut up, part of it was lost and the rest landed 1.8 s late. If PipeWire has any
audio device, PulseAudio is not even asked, so nothing changes where sound already worked; the
app log says which one carries the sound of each recording. This goes for "Always be recording"
too.

`pulsesrc` is in GStreamer's "good" plugins: the package `gstreamer1.0-plugins-good` on Debian
and Ubuntu, which the .deb already recommended, and `gstreamer1-plugins-good` on Fedora. (On
Debian and Ubuntu `gstreamer1.0-pulseaudio` still exists, as an empty transitional package.)
With PulseAudio running but without the plugin, the two switches in Settings are off and name
the package by its Debian and Ubuntu name, since those are the systems where sound is still
PulseAudio's (Ubuntu 22.04, Linux Mint 21, Debian with Xfce). With neither PipeWire audio nor
PulseAudio they say that instead, and recording works without sound.

**The app's own sounds.** The shutter after a capture and the marimba before a recording are the
same two sounds as on Windows, played through GStreamer's `autoaudiosink`, which picks
PulseAudio (or the PipeWire that answers in its place) and falls back to ALSA. As on Windows,
when a recording has sound the marimba plays to its end before anything that could record it is
opened, so it never ends up in the video; that delays the start by about 0.75 s. Without sound
to record it plays while the recording starts.

**One shutter on GNOME.** On GNOME on Wayland the screenshot comes from the portal, and GNOME
Shell plays its own shutter every time it takes one. winshotx cannot turn that off, so its own
click stays silent there: one capture, one shutter. The row in Settings says so, because GNOME's
shutter plays whether the switch is on or off. winshotx only keeps quiet when it is sure: the
windows are on Wayland, the desktop calls itself GNOME and GNOME Shell's screenshot service is
on the session bus. If GNOME's event sounds are switched off (`org.gnome.desktop.sound
event-sounds`), GNOME's shutter is silent and winshotx's comes back. The click after a recording
is saved without the editor does not go through the portal and plays everywhere.

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
- The frame, on X11 with openbox alone and with picom running: four windows around the recorded
  area, blue while recording and amber while paused, back on top of a window opened over them,
  a click on the line reaching the window underneath, none left after stopping, and nothing of
  theirs inside the area or in the saved MP4. With the area against the left edge of the screen
  that side is not drawn, and there is none when the whole screen is recorded. On GNOME the
  recording has no frame and is saved as before. Not tried: more than one monitor, a display
  scaled above 100%, and the compositors of KDE, Xfce and GNOME on X11.
- Clicks and keys, on X11, with a left click, a right click, a shortcut (Ctrl+Shift+P), the
  zoom key, a word typed and a capital letter, all made while recording: the recording keeps
  the two clicks at their place inside the area, that one shortcut, one zoom mark and one
  stretch of typing, and none of the clicks made on the recording bar to pause and resume. The
  editor then offers the click rings and puts the zoom of the mark on the timeline, and the
  saved MP4 shows the ring of each click and the pill of the shortcut. With the click sound
  switched on, that MP4 has a sound track. With the keyboard switched to Spanish in the middle
  of the recording, an @ typed with AltGr and an ñ are not taken for shortcuts. With the rings
  and the keys asked for before recording, they are already in the preview: the pill, and each
  ring in the frames the screen produces during its 0.4 s. A still screen sends no frames, as
  on Windows: with a test window that changed every 0.4 s a ring fell in a single frame,
  barely open, and with one that changed every 0.15 s both rings were seen. The same gestures
  with the build from before this leave nothing in the recording. On GNOME the recording is
  saved as before, without clicks or shortcuts, and the editor does not show their group (no
  gestures were made there: nothing reads them). Not tried: a real mouse and keyboard (the
  gestures came from `xdotool`, at the pace of a person), layouts other than US and Spanish,
  and an input method (IBus) in between.
- GNOME 50 on Wayland: a region to GIF, with the desktop's question the first time and without
  it the second; a token the desktop rejects, with the question cancelled and answered; and
  stopping from GNOME's screen sharing indicator, which saves the recording.
- Sound, with a virtual output and a virtual microphone: each lands in its own track and lasts
  as long as the video. Without audio devices, and without PipeWire running, the recording is
  saved without sound.
- The app's own sounds, with the virtual output and a listener recording everything that came
  out of it. On X11 the shutter is heard when a capture is copied. With sound being recorded,
  the marimba is heard to its end before the recording starts (0.69 s of sound, played in
  0.72 s) and is not in the recorded track, where the only sound is a 440 Hz tone played three
  seconds in. The build from before plays neither. On GNOME 50 each capture has one shutter,
  GNOME's own (0.31 s, when the screenshot is taken); winshotx's is not heard, the row in
  Settings says so, and the marimba is heard before the recording and is not in it. With
  GNOME's event sounds switched off, GNOME's shutter is not heard, winshotx's is, and the row
  says nothing.
- On Ubuntu 22.04, in a container with its PulseAudio 15.99 and a PipeWire 0.3.48 that carries
  no sound, with the app built on Debian 12 the way CI builds the .deb (it asks for glibc 2.35
  at most). The log says PulseAudio carries the sound. A 440 Hz tone played 3.02 s into the
  recording is at 3.08 s of the system track and lasts what it lasted. The shutter and the
  marimba are heard, and the marimba is not in the recording. With a virtual microphone that
  delivers 10 ms at a time, its track is whole (8.74 s of tone in a recording of 8.78 s). With
  PulseAudio's test source, which delivers two-second blocks, it is whole too and starts
  0.03 s in; the block still being filled when the recording stops never arrives, so with a
  source like that up to two seconds at the end are silent. With the 200 ms buffer the same
  track came in four pieces, with gaps of 0.19 and 1.39 s. The build from before records the
  same session without sound and plays neither of its own. With the PulseAudio plugin taken
  away, the two switches are off and name the package.
- The same PulseAudio pipeline without the app, on that Ubuntu, changing only the size of the
  buffer. With the two-second-block source: 200 ms loses 0.19 s and places the rest 1.8 s
  late, 1 s loses 0.99 s, and 2, 3 and 6 s lose nothing and start within 0.01 s of their place.
  A source that delivers 10 ms at a time is whole and starts within 0.03 s of its place with
  200 ms, 1 s, 3 s and 6 s. With every core busy (one spinning process per core) that source
  still loses nothing, while the two-second-block source lost a whole block in five runs out
  of eight, with 3, 4 and 6 s alike.
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
screens, and real sound hardware: every output and microphone above was a virtual one. With
PulseAudio's virtual output, sound played in the first two seconds of a recording can be
missing (that output works in two-second blocks and only changes pace when the current one
ends); a sound card should not do that, and none was tried. Nor were Linux Mint 21, recordings
hours long (how far a sound card's clock drifts from the system's), or whether KDE Plasma plays
a shutter of its own when its portal takes the screenshot (winshotx's plays there). Hardware
encoders (VA-API) are not used: the MP4 is encoded
on the CPU. A custom click sound in MP3 is read through GStreamer, which is only covered by a
test with a real file: the click sound tried in a saved MP4 was the built-in one.
An AppImage built from this code has not been run: what is said about it comes from the one CI
built before, unpacked, and from its GStreamer core loaded by itself.

## Webcam

"Record the camera too" (off by default) records the webcam into its own file next to the
recording, `camara.mp4`, like on Windows. The editor puts it in a corner and the saved MP4
carries it.

**Where the camera comes from.** winshotx does not open `/dev/video0` itself. It asks GStreamer
which video sources there are (`GstDeviceMonitor`, class `Video/Source`) and lets it build the
element that reads the chosen one: `pipewiresrc` where cameras go through PipeWire, `v4l2src`
where they do not. When both see the same camera, PipeWire's provider hides the V4L2 one, so it
is not listed twice. In PipeWire a screen being shared is a video source too (winshotx's own,
while it records on Wayland), so only what PipeWire marks as a camera (`media.role=Camera`, set
by PipeWire itself since at least 0.3.48) is listed. The Camera portal is not used: it is for
sandboxed apps, and outside a sandbox the PipeWire session already shows the cameras.

**The path.** The camera is asked for its format closest to 720p at 30 frames per second, the
same rule as on Windows; MJPEG is decompressed with `jpegdec`. Frames are converted to RGBA, at
most 1080 pixels high, and encoded with the same system H.264 encoder that writes the preview.
A camera that refuses to start in the chosen format, or that does not say which formats it has,
is read in whatever it gives (`decodebin`). As on Windows, the camera is turned on before the
recording starts and the recording waits for its first frame, three seconds at most. Without a
camera, or if it cannot be turned on, the recording goes on without it and the app log says
why. Without an H.264 encoder there is no camera file, just as there is no MP4, so **the
AppImage records no camera**.

**The packages.** `v4l2src` and `jpegdec` are in the GStreamer "good" plugins, which the
packages already recommend. `pipewiresrc` is in PipeWire's own GStreamer plugin:

| | Package | Recommended by winshotx |
|---|---|---|
| .rpm | `pipewire-gstreamer` | yes |
| .deb | `gstreamer1.0-pipewire` | no |

GNOME Shell depends on that plugin (checked for Debian 12 and 13, Ubuntu 22.04 and 24.04 and
Fedora 44), so on GNOME it is already installed. Measured in clean containers, on top of what
winshotx already installs: on Fedora 44 and 45, where PipeWire is always there, the plugin adds
85 KiB of download and 185 KiB of disk. On Ubuntu 22.04, where the PipeWire daemon already comes
with what winshotx installs, 45 kB and 154 kB. On Debian 12 and 13 and Ubuntu 24.04 and 26.04 the
container had no PipeWire daemon, the plugin depends on it, and `apt` brought 15 to 23 packages
(1.9 to 4.6 MB of download, 8.5 to 18.9 MB of disk), WirePlumber and `pipewire-pulse` among
them. A screenshot tool should not be what installs a second sound server on a PulseAudio
system, so the .deb does not recommend it. Without the plugin, cameras are read straight
through V4L2; a camera that only libcamera can drive needs
`sudo apt install gstreamer1.0-pipewire`.

**What was tried**, in the same virtual session on Fedora 44. Its kernel (WSL2) has no V4L2, so
the camera was a GStreamer test source published in PipeWire as one (`pipewiresink
mode=provide`, `media.class=Video/Source`, `media.role=Camera`): colour bars, 640x480 at 30
frames per second.

- X11 (openbox) and GNOME 50 on Wayland: a region recorded as video with the setting on.
  `camara.mp4` is in the session, 640x480 in H.264, and lasts as long as the recording (10,033
  ms against 9,999 on X11; 7,199 against 7,169 on GNOME). The session points at it with an
  offset of 32 and 11 ms. The editor shows it in the corner and the saved MP4 carries it, seen
  in a frame read back from the file.
- The same steps with the build from before: no camera file, and nothing in the saved MP4.
- X11, with the setting on and no camera: the recording is saved without it and the log says
  that no camera is connected.
- The settings window on X11: the row shows the name of the camera, or that none is connected.
- Rust tests where a GStreamer source acts as the camera: one that gives MJPEG, one that lets
  the size and the rate be chosen, one that refuses the chosen format, one that does not say
  its formats, a pause that must not be in the file, a recording that never starts, and a
  camera that fails.
- In clean containers of Debian 13, Ubuntu 22.04 and 26.04 and Fedora 45: GStreamer has
  `v4l2src` and its device provider without the PipeWire plugin, and `pipewiresrc` and its
  provider with it; asking for cameras with no PipeWire running and no camera ends at once.

Not tried: a real camera, the V4L2 path (`v4l2src`), a camera driven by libcamera, more than
one camera, a camera unplugged while recording, a camera another program is using, PipeWire
older than 1.6 (the code was read for 0.3.48, 0.3.65 and 1.0.5, not run), and a package built
with the new `Recommends` (CI has to build it).

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

## Scrolling capture

`S` in the region picker: select what scrolls, and winshotx scrolls the page down by itself and
stitches it into one image, which opens in the editor. The stitching and the decision of how far
to turn the wheel are the same code as on Windows (`capture/scroll.rs`), which never depended on
the platform. What changes is where the photos come from and who turns the wheel.

**On X11 everything is asked of the X server** (`capture/scroll/linux.rs`): the photos through
MIT-SHM, the wheel through XTEST (buttons 4 and 5, which the server delivers to whatever is under
the pointer), and Esc grabbed with `XGrabKey` for as long as the capture lasts. Esc is not
grabbed through the global shortcut plugin: `global-hotkey` 0.8 asks to release a key without
waiting for the server, and after a capture Esc stayed grabbed for the whole desktop.

**On Wayland a program can neither read the screen nor touch the pointer**, so both are asked
of the desktop through the RemoteDesktop portal, with the screen inside the same session
(`capture/scroll/wayland.rs`, `record/linux/wayland.rs`). The photos are the last frame the
desktop sent, without the pointer in it. The desktop asks once and the answer is kept in
`~/.config/com.munir.winshotx/remotedesktop-token` (mode 600), apart from the token of the
recordings, which only covers the screen.

- **In GNOME's question, "Allow Remote Interaction" is off by default.** Shared without it,
  winshotx sees the screen but cannot turn the wheel: nothing scrolls by itself, and it
  stitches whatever the person scrolls. While the desktop is going to ask, the region picker
  says so before starting ("When the desktop asks, allow remote interaction: without it you
  will have to scroll it yourself"). A permission without the pointer is never kept: kept, the
  desktop would not ask again and the page would never scroll by itself, with nothing in
  winshotx to change it.
- **Where there is no RemoteDesktop portal** (the wlroots desktops), only the screen is asked
  for, with the token of the recordings, and the person scrolls.
- If the question is cancelled, nothing is captured and it is not asked again until the next
  capture.

**Nothing of ours can be kept out of a photo on Linux**: what is on the screen ends up in the
image. On X11 the frame goes outside the area and the bar is only shown when it fits entirely
outside it (never with the whole screen). On Wayland a window does not choose its place, so
there is no bar and no frame, and Esc belongs to the program in front. How to stop before the
end is said in the region picker, before starting:

- Esc on X11.
- The capture key. While a scrolling capture runs, on Linux that key ends it instead of
  opening another capture on top, and so does `winshotx --capture`, which is that key on the
  desktops where shortcuts are set by hand.
- On Wayland, the desktop's own "stop sharing" button (GNOME, KDE).

Stopped early, it keeps what it has so far. It also ends by itself at the end of the page.

What was tried, in virtual sessions on Fedora 44, with synthetic input and a GTK window showing
a known long page, so that every row of the stitched image could be compared with the page:

- X11 with openbox: it scrolls to the end by itself and the image saved from the editor is the
  page, row by row; Esc, the capture key and `winshotx --capture` stop it halfway and the image
  is shorter but right; with the whole screen there is no bar; afterwards the capture key opens
  a capture as usual and Esc closes it.
- GNOME 50 on Wayland: the desktop asks; answered with remote interaction allowed, it scrolls
  to the end by itself and the saved image is the page, row by row; the token is kept with mode
  600 and is not written to the log; the second capture does not ask, and `winshotx --capture`
  stops it without opening another capture. Answered without remote interaction, nothing
  scrolls by itself, the wheel of the person is stitched, no token is kept and the next capture
  asks again. Cancelled, nothing is captured. Stopped from GNOME's screen sharing indicator, it
  keeps what it had and opens the editor.
- GNOME with the RemoteDesktop portal taken away (`XDG_DESKTOP_PORTAL_DIR`), which is what a
  wlroots desktop looks like: only the screen is asked for, the wheel of the person is
  stitched, and the second capture does not ask.
- Next to it, on both: recording a region still works, and it shares the screen code.

Not tried: a real wheel and a real keyboard, a real browser (smooth scrolling, sticky headers,
pages that load more at the bottom) on Linux, KDE Plasma (its RemoteDesktop question and its
wheel), Sway and Hyprland themselves, display scaling (the pointer is placed in the desktop's
logical pixels, which is only covered by a unit test), more than one monitor, and X11 with a
compositor.

## AI agents

`winshotx mcp` and `winshotx agent <command>` work on Linux as on Windows (`docs/AGENTS-API.md`):
the same commands, the same answers and the same code (`src-tauri/src/agente`). What had to be
written is the channel to the app and how the app is started.

**The channel is a Unix socket**, `winshotx-agent.sock` in `$XDG_RUNTIME_DIR` (`agente/tuberia.rs`).
That folder belongs to the user, has mode 700 and dies with the session, which is what the named
pipe gives on Windows. winshotx does not listen if the folder is somebody else's or others can
open it; the socket has mode 600; the app drops a connection that comes from another user
(`SO_PEERCRED`), and the client does not send a word to a socket another user is listening on.
The socket of an app that died is replaced by the next one. Abstract sockets were ruled out:
they have no permissions.

**Agents start their tools with almost no environment.** The official MCP SDK passes only
`HOME`, `LOGNAME`, `PATH`, `SHELL`, `TERM` and `USER`: no `XDG_RUNTIME_DIR`, no `DISPLAY`, no
`WAYLAND_DISPLAY`. So:

- Without `XDG_RUNTIME_DIR`, both sides use `/run/user/<uid>`, which is where systemd puts it.
- With `DISPLAY` or `WAYLAND_DISPLAY`, the client starts the app itself (`agente/cliente.rs`):
  through a launcher that puts it in its own session (`setsid`), with stdin, stdout and stderr on
  `/dev/null` and no other descriptor inherited (`close_range`), and exits. The app is left
  hanging from the system, not from the agent: an agent that kills its MCP server, or the whole
  process group, does not take the app or a running recording with it.
- Without them, a window cannot be opened from there. The client asks systemd's user manager,
  which does have the session's variables on any desktop that uses the portals:
  `systemd-run --user --collect --quiet -- winshotx --started-by-agent`. It first checks, with
  `systemctl --user show-environment`, that systemd knows of a desktop; where it does not, or
  there is no systemd, the answer says what to do (open winshotx once, or give the command the
  variables) instead of waiting for an app that cannot start.

`--started-by-agent` does nothing by itself: started with any argument the app does not open its
settings, and this one tells in the process list who started it. From an AppImage the app is
started through the `.AppImage` file (`$APPIMAGE`), not through the binary inside its mount.

**What changes for the agent** is said by the MCP server itself (its instructions and the
description of `record_start` are written per system, `agente/mcp.rs`):

- On X11 the window list comes from the window manager (`_NET_CLIENT_LIST_STACKING`, through
  xcap), with the name of the executable from `/proc`. A window is photographed with `GetImage`
  on the window itself.
- On Wayland a program cannot see the windows of the others: `windows`, and any target given as
  a window, answer with an error that says to use `screen` or `region`.
- A recording started by an agent looks like one started by hand, which on Linux means: on X11
  the frame and the bar when they fit outside the recorded area (with the whole screen, only the
  tray menu changes); on Wayland, the desktop's own screen sharing indicator.

**On Wayland it is the desktop that asks the person**, and the agent's call waits:

- The first `record` makes the desktop ask which screen to share, with no time limit on our
  side. With "remember" ticked the next ones do not ask, also after the app is restarted (the
  token of the recordings, mode 600).
- A screenshot goes through the Screenshot portal. GNOME asks for that permission the first
  time and only while the app is the window in front, which it is not when an agent asks. So
  the same is done as with a shortcut (`platform::portal::pedir_permiso_con_la_ventana`): the
  settings window is brought up once per session and the permission is asked from there, and
  the agent gets a sentence that says what is going on instead of the portal's
  "Portal request didn't succeed: Other".
- A screenshot asked for while recording leaves nothing in the video on GNOME 50 (no extra
  frames around it), so it is not refused.
- In a profile where nobody has answered winshotx's shortcuts question yet, GNOME shows it when
  the app starts, also when an agent starts it. That is the first-run question, not something of
  the agents, and once answered it does not come back: accepted, GNOME keeps the keys; closed,
  winshotx remembers the no (see "Shortcuts on Wayland").

What was tried, in virtual sessions on Fedora 44, with the app itself and a GTK window showing a
known page:

- X11 with openbox: every command. The screenshots of the screen, of a region and of a window
  are, pixel by pixel, what the X server shows; the window list has the page and none of
  winshotx's windows; a region is recorded with `--max-seconds`, stopped and exported to MP4 and
  GIF, the whole screen is recorded, and `open` opens the editor; the frame and the bar are seen
  around the region. With the app killed, the next command starts it in 0.3 s, first of its own
  session, with nothing inherited and no window. The MCP server closed the way Claude Code
  closes it (SIGINT, SIGTERM, SIGKILL), its whole process group killed, and a SIGHUP to the
  group: the app and its recording go on. Another user (`nobody`) cannot connect; with the
  folder and the socket opened on purpose it connects and is dropped; and the client refuses a
  socket another user listens on without sending it anything. With only the six variables: it
  answers with the app open; with the app closed systemd starts it (it hangs from `systemd`, in
  its own `run-*.service`) and it captures; and when systemd knows of no desktop the answer says
  what to do, in 0.1 s. Next to it: the capture by hand still opens and Esc closes it, and a
  region is still recorded by hand.
- GNOME 50 on Wayland: `windows` and a window target say there is no window list; the
  screenshots of the screen and of a region are the page, pixel by pixel, and nothing is left in
  the desktop's pictures folder; the first `record` waits while the desktop asks and starts once
  answered, the second starts in half a second without asking, and so does one after the app is
  killed and started again by the next command; a region with `--max-seconds`, `stop`, `export`
  and `open`; the MCP server over stdio. Without the screenshot permission: the agent gets the
  sentence, winshotx's window comes up with GNOME's question, and once allowed the next capture
  works. With two monitors (1600x1000 and 1280x800): `screens` lists both, each one is
  photographed with its own size, and a region across both is the edge of one next to the edge
  of the other. Next to it: the capture by hand and recording a region by hand.

Not tried: a real agent (Claude Code, Codex, Cursor) on Linux, only a scripted MCP client that
starts and closes the server the way they document; KDE Plasma (its questions and its portal);
a distribution without systemd with an agent that passes no variables (what is tried is systemd
knowing of no desktop, which takes the same path); the AppImage; more than one monitor on X11
(this Xvfb does not list monitors made with `xrandr --setmonitor`), and recording the second
one on Wayland; and display scaling.

## Always be recording

"Always be recording" (Settings, "The last few seconds") keeps the last 15, 30 or 60 seconds of
the screen at all times, and a key saves them and opens them in the editor without stopping. It
is the ring of the Windows version (`replay.rs`, `record/buffer.rs`): the screen as tiles on
disk, only the ones that changed, the oldest dropped. What had to be written for Linux is where
the screen, the pointer, the clicks and the sound come from.

- **The screen** comes from the recording code (`record/linux`): MIT-SHM on X11, the ScreenCast
  portal and PipeWire on Wayland. It hands over whole frames, and only when the screen changes.
- **The pointer** is not in the image (except on the Wayland desktops that can only give it
  already drawn): where it went and which cursor it had are noted by the capture, and the
  editor draws it, as in a recording.
- **Clicks and shortcuts** are read on X11 only (on Wayland a program cannot), every 16 ms and
  on a thread of their own (`Vigia` in `replay.rs`). They were first read between one frame and
  the next, on the thread that saves them, and saving a frame takes longer than a quick click
  lasts: with the whole screen changing, a third of the clicks were missing from what was
  rescued, and three out of four at 2560x1440.
- **Sound** goes through PipeWire into one ring in memory per source, and comes out as the two
  tracks of a recording: `sonido-pc.wav` and `microfono.wav`.

**On Wayland the desktop asks, and shows that it is sharing.** The first time the switch is
turned on the desktop asks which screen to share, so "Which screen" is not shown in Settings.
The answer is kept in `~/.config/com.munir.winshotx/replay-token` (mode 600), apart from the
token of the recordings: each use spends a token and the desktop hands back another, and with a
single file one would overwrite the other. While it is on, the desktop shows its own screen
sharing indicator. It cannot be removed, and the row in Settings says so before it is turned on.

- **At startup it only comes back if the permission was kept.** Without it, starting the ring
  would make the desktop ask at login, with nothing of winshotx in sight. The setting is turned
  off instead, and its row says why in grey ("turned off at startup: it needs your permission to
  share the screen") until the switch is touched. A setting left on with nothing recording would
  be a lie to whoever counts on rescuing something.
- **Stopping the sharing from the desktop's indicator turns the setting off** as well, and the
  row says so ("turned off because screen sharing was stopped").
- The reason is kept in `replay-apagado`, next to `settings.json`, so that it is still there
  after a restart.

**The key that rescues** is Ctrl+Shift+6 on X11, grabbed like the others. On Wayland it is one
more shortcut asked of the desktop (see "Shortcuts on Wayland"), and only while the ring is on.
Without a key, "Save now" in the same section and `winshotx --replay` do the same.

**What it costs**, measured with the release binary on Fedora 44 under WSL2 (Core Ultra 7 255H),
a 1600x1000 screen, both sound sources recording a tone and the pointer moving. Share of one
core used by winshotx, with the screen still, with a small window changing ten times a second,
and with the whole screen changing thirty times a second:

| | still | a small part changes | everything changes |
|---|---|---|---|
| X11, 15 fps | 2.5 % | 4.5 % | 8.1 % |
| X11, 60 fps | 5.2 % | 7.4 % | 16.9 % |
| GNOME Wayland, 15 fps | 1.2 % | 3.6 % | 10.1 % |
| GNOME Wayland, 60 fps | 1.0 % | 3.7 % | 15.1 % |

With the ring off winshotx uses nothing measurable. Memory goes from 180 MB to 220-240 MB on
X11 and from 172 MB to 280-286 MB on GNOME. On disk, 15 seconds of a whole screen changing take
15 MB at 15 fps and 31 MB at 60. Rescuing takes about 0.6 s until the session is there. The
binary grows by 152 KB (1.4 %).

What was tried, in virtual sessions on Fedora 44, with the app itself:

- X11 with openbox: on at startup, and on, off and on again from Settings (turning it off
  deletes its folder); the key, `winshotx --replay` and a third rescue after a capture and a
  recording made with the ring on; the session that comes out lasts its 15 seconds, with the
  pointer, a left click, a right click and a shortcut where they were made, and each sound
  source on its own track with its own tone; its preview and the exported MP4 last the same and
  carry both tones. Clicks held for 120, 80, 60, 40 and 25 ms with the screen still, a small
  part changing and everything changing, at 1600x1000 and at 2560x1440, 15 and 60 fps: all 40 of
  each series are in what is rescued (before the thread of their own, up to 30 were missing).
- GNOME 50 on Wayland: nothing is asked at startup with the ring off; turning it on asks for
  the screen and then for the key. That question closed: the ring stays on, the three keys are
  the same and still work, `atajos-sin-rescate` is left, and `winshotx --replay` and "Save now"
  rescue, with both tracks and their tones. After a restart the ring comes back by itself and
  nothing is asked. "Change" asks again; accepted, the four keys are bound, the file is gone
  and the key rescues; after another restart there are four and no question. With the first
  question of all closed (`atajos-sin-pedir`): a start does not ask, turning the ring on does
  (the four), and closing it again leaves the ring on. Stopped from GNOME's indicator: the ring
  and the setting go off, the row says why, and the app carries on.
- GNOME with the setting on and no permission kept: nothing is asked at startup, the setting is
  off, the row says why, also after another restart, and turning it on by hand clears it.
- With "Remember this selection" unticked in GNOME's question no token is kept, so the next
  start is the case above.

**GNOME asks for the key again every time the ring is turned back on.** Turning it off binds the
three keys alone, which is what frees the fourth for other programs, and GNOME then forgets it.

Not tried: KDE Plasma (its questions and its indicator), Sway and Hyprland themselves (only the
unit tests of the row with the command), a real pointer and a real keyboard, real sound devices,
more than one monitor, display scaling, and X11 with a compositor.

## The settings window

The settings window always opens at 900x723 unless someone stretches it, and then at the size
they left it (`src/lib/ventanaFija.ts`). On GNOME Wayland it used to open bigger on every start
without anyone touching it: 952x822, then 1004x921, then 1056x1020, until it no longer fitted
the screen. There GTK draws the title bar inside the window itself, with its shadow around it,
and the inner size Tauri reports, like the one in its resize event, counts both (26 px of shadow
on each side and 47 of title bar). Just showing the window looked like someone had stretched it,
so that size was saved and the next start gave it to the page.

On Linux the inner size is now the one the page itself measures, and nothing is saved while the
page still measures nothing (started with the session, the window is hidden). Tried with the app
on GNOME 50 (Fedora 44, virtual session): 900x723 on three starts in a row, with nothing saved;
stretched to 996x771 by dragging its corner, that size is saved and the next two starts open at
996x771. On X11 (Xvfb with openbox), where the window manager draws the title bar and this never
failed, the same before and after the change. Not tried: KDE Plasma, display scaling.

**Known issue in 0.6.0: right after an update, the "How it will look" drawing may not appear
until the mouse moves.** After an update the window opens on "Help and winshotx", a section
without that drawing. The first click on a section that has it changes the page, and WebKitGTK
does not paint the change until the next event: moving the mouse over the window is enough, and
it happens only that once. Seen on X11 and on GNOME on Wayland in the virtual session (Fedora 44,
WebKitGTK 2.54.2), with the release binary too; not tried on a physical machine. It does not
happen when the window opens on "Capture", which is every other start, nor on the welcome screen.

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

## Shortcuts on Wayland

On X11 winshotx grabs its keys itself. On Wayland it asks the desktop through the GlobalShortcuts
portal (GNOME 48+, KDE Plasma), and the desktop asks the person.

**A question that was closed is not asked again.** GNOME only stores the shortcuts when the
person presses "Add". Closed any other way, nothing is stored, and winshotx used to ask again on
every start (the one at login and the one by an agent too) and every time a setting was changed.
Now it leaves an empty file, `atajos-sin-pedir`, next to `settings.json`, and while that file is
there it only asks when the person presses "Change" in Settings or on the welcome screen. Adding
the shortcuts from there removes the file.

The portal does not say that it was the person who said no: GNOME answers 2 ("other") both when
the question is closed and when its own settings window fails, and 1 ("cancelled") only when the
app withdraws the request (`settings_bind_shortcuts_done` in xdg-desktop-portal-gnome's
`src/globalshortcuts.c`). What tells them apart is time: a refusal that took a second or more was
answered by a person, one that comes back in milliseconds is a desktop that cannot bind them.
That second kind leaves no file and, from "Change", still leads to the commands below.

Tried on GNOME 50 (Fedora 44, virtual session, clean profile): the question closed with Esc on
the first start; no question on the next start, on a start by an agent, or after changing a
setting; "Change" shows it again, closing it again leaves "Change" in place, and "Add" from there
binds the keys, after which GNOME no longer asks. Not tried: KDE Plasma, and the same home
directory moved from a desktop where the question was closed to another desktop (the file does
not know which desktop asked, so the other one is asked from "Change" too).

**The key that rescues the last few seconds is a fourth shortcut**, asked for only while "Always
be recording" is on. The portal has no call to add one shortcut to a session: the session is
closed and the whole list is asked for again, so GNOME shows its question with the four of them,
the three it already had and the new one.

- Turning the switch on is something the person does, so the desktop is asked even if the first
  question had been closed (`atajos-sin-pedir`). A start never asks.
- If that question is closed, the three keys of before are bound again as they were (GNOME has
  them stored and does not ask), the ring stays on without a key, and a second empty file,
  `atajos-sin-rescate`, keeps later starts from asking for the fourth. "Change" on its row asks
  again, and adding the key removes the file.

**The "How it will look" panel says the key that works.** That panel, on the right of the
settings, used to write the shortcut kept in the app's own settings, which on Wayland is only
what winshotx suggests to the desktop: with the row next to it saying "No key", the panel still
said "Ctrl + Shift + 2 and you pick the area with the mouse". On Wayland it now says the
desktop's key (as the desktop writes it, when it cannot be read), "No key yet: ask for one with
«Change»" when the desktop gave none, and the command to bind where shortcuts are set by hand.
With "Always be recording" off its key has not been asked for and "Change" is disabled, so that
line says the key is picked when turning it on. Tried on GNOME 50 with the app: the three
panels without keys (the first question closed), with the keys GNOME gave, and with the ring
off again. Sway and Hyprland only in the unit tests.

### Desktops without the GlobalShortcuts portal

Sway and the other wlroots compositors have no such portal, so the welcome screen and the
settings show the commands to bind by hand instead:

```
winshotx --capture   # capture a region
winshotx --edit      # capture and open the editor
winshotx --record    # record a region, or stop the recording in progress
```

The row of the key that rescues, in "The last few seconds", shows a fourth one there:
`winshotx --replay`, which saves the last few seconds while "Always be recording" is on.

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
