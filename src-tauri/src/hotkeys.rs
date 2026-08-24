use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{
    GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::settings::Settings;
use crate::state::AppState;
use crate::windows_mgr::{self, OverlayIntent};

/// La tecla suelta, tal y como la escribe el parser de atajos.
pub const PRINT_SCREEN: &str = "PrintScreen";

/// Que atajos han quedado activos de verdad. Si otra aplicacion ya tiene cogida la
/// combinacion, el registro falla y el usuario tiene derecho a enterarse.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub capture: bool,
    pub record: bool,
    pub print_screen: bool,
}

/// Abrir el overlay para capturar. Lo comparten el atajo de captura y la tecla
/// Impr Pant, que hacen exactamente lo mismo desde dos teclas distintas.
fn on_capture(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    if let Err(error) = windows_mgr::open_overlays(app, OverlayIntent::Capture) {
        eprintln!("no se ha podido abrir el overlay: {error}");
    }
}

/// Registra los atajos globales. Si el sistema ya tiene cogido uno,
/// los demas siguen funcionando: nunca se aborta el arranque por esto.
pub fn register(app: &AppHandle, settings: &Settings) -> ShortcutStatus {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    let mut status = ShortcutStatus::default();

    match settings.capture_shortcut.parse::<Shortcut>() {
        Err(error) => eprintln!("atajo de captura invalido: {error}"),
        Ok(shortcut) => {
            if let Err(error) = manager.on_shortcut(shortcut, on_capture) {
                eprintln!(
                    "el atajo de captura {} ya lo usa otra aplicacion: {error}",
                    settings.capture_shortcut
                );
            } else {
                status.capture = true;
            }
        }
    }

    // Impr Pant es la tecla que la gente ya tiene en los dedos, pero en Windows 11 se
    // la queda la Herramienta de Recortes. Si ese ajuste sigue puesto, esto se registra
    // sin quejarse y luego no llega ninguna pulsacion: por eso se apaga antes, en
    // `commands::use_print_screen`, y no aqui.
    if settings.print_screen_capture {
        if let Ok(shortcut) = PRINT_SCREEN.parse::<Shortcut>() {
            if let Err(error) = manager.on_shortcut(shortcut, on_capture) {
                eprintln!("la tecla Impr Pant ya la usa otra aplicacion: {error}");
            } else {
                status.print_screen = true;
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
                let _ = windows_mgr::open_overlays(app, OverlayIntent::Record);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// El nombre de la tecla se escribe a mano en una constante: si el parser deja de
    /// entenderlo, esto lo dice aqui y no en la maquina de alguien que pulsa y no pasa nada.
    #[test]
    fn la_tecla_impr_pant_se_entiende() {
        assert!(PRINT_SCREEN.parse::<Shortcut>().is_ok());
    }
}
