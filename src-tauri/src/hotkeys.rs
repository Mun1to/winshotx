use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::settings::Settings;
use crate::state::AppState;
use crate::windows_mgr;

/// Que atajos han quedado activos de verdad. Si otra aplicacion ya tiene cogida la
/// combinacion, el registro falla y el usuario tiene derecho a enterarse.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub capture: bool,
    pub record: bool,
}

/// Registra los dos atajos globales. Si el sistema ya tiene cogido uno,
/// el otro sigue funcionando: nunca se aborta el arranque por esto.
pub fn register(app: &AppHandle, settings: &Settings) -> ShortcutStatus {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    let mut status = ShortcutStatus::default();

    match settings.capture_shortcut.parse::<Shortcut>() {
        Err(error) => eprintln!("atajo de captura invalido: {error}"),
        Ok(shortcut) => {
            if let Err(error) = manager.on_shortcut(shortcut, |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    if let Err(error) = windows_mgr::open_overlays(app) {
                        eprintln!("no se ha podido abrir el overlay: {error}");
                    }
                }
            }) {
                eprintln!(
                    "el atajo de captura {} ya lo usa otra aplicacion: {error}",
                    settings.capture_shortcut
                );
            } else {
                status.capture = true;
            }
        }
    }

    if let Ok(shortcut) = settings.record_shortcut.parse::<Shortcut>() {
        if let Err(error) = manager.on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            // Con una grabacion en curso el mismo atajo la para: no hay que ir al raton.
            if app.state::<AppState>().is_recording() {
                if let Err(error) = crate::recorder::stop(app) {
                    eprintln!("no se ha podido parar la grabacion: {error}");
                }
            } else {
                let _ = windows_mgr::open_overlays(app);
            }
        }) {
            eprintln!(
                "el atajo de grabacion {} ya lo usa otra aplicacion: {error}",
                settings.record_shortcut
            );
        } else {
            status.record = true;
        }
    }

    *app.state::<AppState>().shortcuts.write() = status;
    status
}
