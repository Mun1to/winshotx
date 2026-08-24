use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

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

/// Registra los atajos globales y devuelve cuales han quedado puestos de verdad.
///
/// Se sueltan uno a uno los que pedimos la vez anterior, en vez de llamar a
/// `unregister_all`: ese vacia la tabla del plugin **antes** de hablar con Windows, asi que
/// en cuanto una sola llamada falla el plugin cree que no tiene nada registrado, Windows
/// sigue teniendolo, y desde ese momento cualquier atajo devuelve "HotKey already
/// registered" aunque el usuario no haya repetido ninguna tecla. Con la lista propia eso no
/// puede pasar: se pide soltar exactamente lo que se pidio poner.
pub fn register(app: &AppHandle, settings: &Settings) -> ShortcutStatus {
    let manager = app.global_shortcut();
    let state = app.state::<AppState>();

    let previos = std::mem::take(&mut *state.registered.write());
    for shortcut in previos {
        if let Err(error) = manager.unregister(shortcut) {
            eprintln!("no se ha podido soltar un atajo: {error}");
        }
    }

    let mut status = ShortcutStatus::default();
    let mut puestos: Vec<Shortcut> = Vec::new();

    match settings.capture_shortcut.parse::<Shortcut>() {
        Err(error) => eprintln!("atajo de captura invalido: {error}"),
        Ok(shortcut) => {
            if let Err(error) = manager.on_shortcut(shortcut, on_capture) {
                eprintln!(
                    "el atajo de captura {} no se ha podido registrar: {error}",
                    settings.capture_shortcut
                );
            } else {
                status.capture = true;
                puestos.push(shortcut);
            }
        }
    }

    // Impr Pant es la tecla que la gente ya tiene en los dedos, pero en Windows 11 se
    // la queda la Herramienta de Recortes. Si ese ajuste sigue puesto, esto se registra
    // sin quejarse y luego no llega ninguna pulsacion: por eso se apaga antes, en
    // `commands::use_print_screen`, y no aqui.
    if settings.print_screen_capture {
        match PRINT_SCREEN.parse::<Shortcut>() {
            Err(error) => eprintln!("la tecla Impr Pant no se entiende: {error}"),
            Ok(shortcut) => {
                if let Err(error) = manager.on_shortcut(shortcut, on_capture) {
                    eprintln!("la tecla Impr Pant no se ha podido registrar: {error}");
                } else {
                    status.print_screen = true;
                    puestos.push(shortcut);
                }
            }
        }
    }

    match settings.record_shortcut.parse::<Shortcut>() {
        Err(error) => eprintln!("atajo de grabacion invalido: {error}"),
        Ok(shortcut) => {
            let resultado = manager.on_shortcut(shortcut, |app, _shortcut, event| {
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
            });
            if let Err(error) = resultado {
                eprintln!(
                    "el atajo de grabacion {} no se ha podido registrar: {error}",
                    settings.record_shortcut
                );
            } else {
                status.record = true;
                puestos.push(shortcut);
            }
        }
    }

    // Lo que dice el plugin y lo que dice el sistema no siempre coinciden: el registro se
    // encola en el hilo principal y puede darse por bueno antes de que Windows conteste.
    let de_verdad = |texto: &str| -> bool {
        texto
            .parse::<Shortcut>()
            .map(|s| manager.is_registered(s))
            .unwrap_or(false)
    };
    eprintln!(
        "[atajo] registrados: captura={} ({}, sistema={}) grabacion={} imprPant={} (sistema={})",
        status.capture,
        settings.capture_shortcut,
        de_verdad(&settings.capture_shortcut),
        status.record,
        status.print_screen,
        de_verdad(PRINT_SCREEN)
    );
    *state.registered.write() = puestos;
    *state.shortcuts.write() = status;
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

    /// Las combinaciones que se ofrecen en la bienvenida tienen que existir de verdad.
    /// Estan escritas en el frontend, y una errata ahi solo se veria al pulsarlas.
    #[test]
    fn las_combinaciones_sugeridas_se_entienden() {
        for sugerida in [
            "CmdOrCtrl+Shift+Digit2",
            "CmdOrCtrl+Shift+Digit5",
            "CmdOrCtrl+Alt+KeyA",
            "CmdOrCtrl+Alt+KeyR",
            "Alt+KeyX",
            "Alt+KeyV",
            "CmdOrCtrl+Shift+KeyS",
        ] {
            assert!(sugerida.parse::<Shortcut>().is_ok(), "{sugerida}");
        }
    }
}
