# packaging/linux

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
```

A second `winshotx` started with one of those flags hands it to the running one and exits, so
the key works only while winshotx is running. Sway does not read XDG autostart entries, so the
first line of this example starts it with the session (`~/.config/sway/config`):

```
exec winshotx --autostart
bindsym Print exec winshotx --capture
bindsym Shift+Print exec winshotx --edit
```

For the AppImage, use the path of the `.AppImage` file instead of `winshotx`; the app shows the
exact command. The same commands work as custom shortcuts on any other desktop, and on Hyprland,
whose portal registers the shortcuts but leaves binding them to `hyprland.conf`.

Screenshots on Sway go through `xdg-desktop-portal-wlr`, which needs PipeWire running.
