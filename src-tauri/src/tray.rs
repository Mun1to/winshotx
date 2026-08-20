use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::error::Result;
use crate::state::AppState;
use crate::windows_mgr;

/// La app vive en la bandeja: no hay ventana principal hasta que se piden los ajustes.
pub fn build(app: &AppHandle) -> Result<()> {
    let capture = MenuItem::with_id(app, "capture", "Capturar región", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Grabar región", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Ajustes…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &capture,
            &record,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("winshotx")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("winshotx")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "capture" => {
                let _ = windows_mgr::open_overlays(app);
            }
            "record" => {
                if app.state::<AppState>().is_recording() {
                    let _ = crate::recorder::stop(app);
                } else {
                    let _ = windows_mgr::open_overlays(app);
                }
            }
            "settings" => {
                let _ = windows_mgr::show_settings(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = windows_mgr::open_overlays(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
