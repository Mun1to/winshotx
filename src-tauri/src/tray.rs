use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::Result;
use crate::state::AppState;
use crate::windows_mgr::{self, OverlayIntent};

/// El menu del boton derecho, en el idioma que toque. Se arma aparte de `build` porque
/// hay que volver a montarlo entero al cambiar de idioma: una entrada de menu de Windows
/// no se puede renombrar, se reemplaza el menu.
fn construir_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>> {
    let idioma = app.state::<AppState>().settings.read().language;
    let textos = crate::textos::menu(idioma);
    let capture = MenuItem::with_id(app, "capture", textos.capturar, true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", textos.grabar, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", textos.ajustes, true, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", textos.actualizaciones, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", textos.salir, true, None::<&str>)?;
    Menu::with_items(
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
    )
    .map_err(Into::into)
}

/// Vuelve a poner el menu, ya en el idioma nuevo. Lo llama el cambio de ajustes: sin esto
/// la aplicacion se queda en ingles y su menu de la bandeja en espannol hasta reiniciar.
pub fn rehacer_menu(app: &AppHandle) -> Result<()> {
    let menu = construir_menu(app)?;
    if let Some(icono) = app.tray_by_id("winshotx") {
        icono.set_menu(Some(menu))?;
    }
    Ok(())
}

/// La app vive en la bandeja: no hay ventana principal hasta que se piden los ajustes.
pub fn build(app: &AppHandle) -> Result<()> {
    let menu = construir_menu(app)?;

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
