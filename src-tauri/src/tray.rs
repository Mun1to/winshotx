use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::Result;
use crate::state::AppState;
use crate::windows_mgr::{self, OverlayIntent};

/// La app vive en la bandeja: no hay ventana principal hasta que se piden los ajustes.
///
/// **Sin menu del sistema.** El del boton derecho lo dibuja winshotx (`tray_menu.rs`),
/// porque uno de Windows no sabe ensennar un interruptor, ni el atajo de cada cosa, ni
/// decir que version esta puesta. Aqui solo se escuchan los clics del icono.
pub fn build(app: &AppHandle) -> Result<()> {
    let mut builder = TrayIconBuilder::with_id("winshotx")
        .tooltip("winshotx")
        .on_tray_icon_event(|tray, event| {
            let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            else {
                return;
            };
            match button {
                // Izquierdo: lo que se viene a hacer nueve de cada diez veces.
                MouseButton::Left => {
                    let _ = windows_mgr::open_overlays(tray.app_handle(), OverlayIntent::Capture);
                }
                // Derecho: el menu, anclado justo donde esta el icono.
                MouseButton::Right => {
                    crate::tray_menu::alternar(
                        tray.app_handle(),
                        (position.x as i32, position.y as i32),
                    );
                }
                MouseButton::Middle => {}
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Lo que hace cada entrada del menu, en un solo sitio.
///
/// Lo llaman el menu dibujado (`tray_menu.rs`, por el comando `tray_menu_action`) y el
/// menu del sistema, que sigue existiendo de reserva. Estaba escrito dentro del manejador
/// del menu nativo, y dejarlo ahi habria significado dos listas de acciones que se
/// separarian al primer cambio.
pub fn ejecutar(app: &AppHandle, id: &str) {
    match id {
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
            "replay" => {
                if let Err(error) = crate::replay::save(app) {
                    eprintln!("no se han podido guardar los ultimos segundos: {error}");
                }
            }
            // Donde acaban las capturas guardadas. Sin esto habia que abrir los ajustes
            // para llegar a una carpeta que se visita todos los dias.
            "folder" => {
                let carpeta = app.state::<AppState>().settings.read().save_directory.clone();
                let _ = std::fs::create_dir_all(&carpeta);
                let _ = crate::platform::open_folder(&std::path::PathBuf::from(carpeta));
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
    }
}
