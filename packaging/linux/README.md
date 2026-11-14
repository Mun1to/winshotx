# packaging/linux

- `com.munir.winshotx.desktop`: a hidden desktop entry (`NoDisplay=true`) named after the app
  identifier. xdg-desktop-portal only accepts `register_host_app("com.munir.winshotx")` when it
  finds a desktop file with that exact name, and without that registration the GlobalShortcuts
  portal refuses the app ("An app id is required"). The visible menu entry is still the one the
  Tauri bundler writes (`winshotx.desktop`, named after `productName`). The .deb and the .rpm
  install this file through `bundle.linux.*.files` in `tauri.conf.json`; the AppImage writes
  the same entry into `~/.local/share/applications` at runtime (`platform/portal.rs`).
