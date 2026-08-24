use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::Result;
use crate::state::AppState;
use crate::windows_mgr::{self, OverlayIntent};

/// La app vive en la bandeja: no hay ventana principal hasta que se piden los ajustes.
pub fn build(app: &AppHandle) -> Result<()> {
    let capture = MenuItem::with_id(app, "capture", "Capturar región", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Grabar región", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Ajustes…", true, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", "Buscar actualizaciones…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &capture,
            &record,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &update,
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
                let _ = windows_mgr::open_overlays(app, OverlayIntent::Capture);
            }
            "record" => {
                if app.state::<AppState>().is_recording() {
                    let _ = crate::recorder::stop(app);
                } else {
                    let _ = windows_mgr::open_overlays(app, OverlayIntent::Record);
                }
            }
            "settings" => {
                let _ = windows_mgr::show_settings(app);
            }
            // La comprobacion vive en la ventana de ajustes: desde la bandeja se
            // abre y se le dice que mire, para no tener dos caminos distintos.
            "update" => {
                let _ = windows_mgr::show_settings(app);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit(crate::EVENT_CHECK_UPDATE, ());
                }
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
                let _ = windows_mgr::open_overlays(tray.app_handle(), OverlayIntent::Capture);
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
