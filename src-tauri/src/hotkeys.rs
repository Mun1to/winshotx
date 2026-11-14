use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::settings::Settings;
use crate::state::AppState;
use crate::windows_mgr::{self, OverlayIntent};

/// La tecla suelta, tal y como la escribe el parser de atajos.
pub const PRINT_SCREEN: &str = "PrintScreen";

/// La otra tecla de captura de Windows. Solo se consigue cuando la S esta fuera de los
/// atajos de la tecla Windows, y ni asi hasta que el usuario vuelve a iniciar sesion.
pub const WIN_SHIFT_S: &str = "Super+Shift+KeyS";

/// Que atajos han quedado activos de verdad. Si otra aplicacion ya tiene cogida la
/// combinacion, el registro falla y el usuario tiene derecho a enterarse.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub capture: bool,
    /// La tecla que captura y abre el editor de una vez. Puede fallar sin que sea culpa
    /// de nadie: la combinación de fábrica lleva la tecla Windows, y esas se las puede
    /// haber quedado otra aplicación.
    pub edit: bool,
    pub record: bool,
    /// La tecla que se queda con los ultimos segundos. Solo se pide cuando el anillo esta
    /// encendido: quitarle una combinacion a alguien que no usa la funcion seria cobrarle
    /// por algo que no ha pedido.
    pub replay: bool,
    pub print_screen: bool,
    /// Si el shell ha soltado ya `Win+Mayus+S`. Puede estar pedida en los ajustes y no
    /// conseguida: la lista de teclas apagadas no vale hasta que el escritorio la relee.
    pub win_shift_s: bool,
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

/// Capturar y abrir el editor de una vez. Lo unico que cambia es la intencion: el
/// overlay es el mismo y la foto es la misma, pero al soltar la seleccion no sale la barra
/// para elegir, se abre el editor.
fn on_edit(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    if let Err(error) = windows_mgr::open_overlays(app, OverlayIntent::Edit) {
        eprintln!("no se ha podido abrir el overlay del editor: {error}");
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
///
/// **De uno en uno.** Los comandos async de Tauri corren en varios hilos a la vez, y cada
/// cambio en los ajustes llama aqui: dos clics seguidos (o el selector de color) metian dos
/// registros a la vez. El segundo cogia la lista vacia porque el primero aun no la habia
/// guardado, pedia F2 cuando el primero ya la tenia, Windows decia "ya registrada" y la
/// lista que quedaba no la llevaba. Desde ahi nadie la soltaba, cada registro fallaba y
/// los ajustes decian "esa combinacion esta cogida" sobre una tecla que era nuestra.
pub fn register(app: &AppHandle, settings: &Settings) -> ShortcutStatus {
    static DE_UNO_EN_UNO: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    let _turno = DE_UNO_EN_UNO.lock();

    // En Wayland el plugin no puede coger ninguna tecla (solo sabe X11) y aun asi dice que la
    // tiene: los atajos los pone el escritorio, por el portal.
    #[cfg(target_os = "linux")]
    if crate::platform::xdg::es_wayland() {
        return atajos_de_wayland(app, settings);
    }

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

    // La captura que va derecha al editor. Si la combinacion no se consigue, se dice y
    // se sigue: perder esta tecla no puede dejar sin capturar a nadie.
    match settings.edit_shortcut.parse::<Shortcut>() {
        Err(error) => eprintln!("atajo del editor invalido: {error}"),
        Ok(shortcut) => {
            if puestos.contains(&shortcut) {
                eprintln!(
                    "[atajo] {} ya esta cogida por otro atajo de winshotx",
                    settings.edit_shortcut
                );
            } else if let Err(error) = manager.on_shortcut(shortcut, on_edit) {
                eprintln!(
                    "el atajo del editor {} no se ha podido registrar: {error}",
                    settings.edit_shortcut
                );
            } else {
                status.edit = true;
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

    // La tecla de los ultimos segundos solo existe mientras el anillo esta encendido.
    if settings.replay_enabled {
        match settings.replay_shortcut.parse::<Shortcut>() {
            Err(error) => eprintln!("atajo de los ultimos segundos invalido: {error}"),
            Ok(shortcut) => {
                let resultado = manager.on_shortcut(shortcut, |app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if let Err(error) = crate::replay::save(app) {
                        eprintln!("no se han podido guardar los ultimos segundos: {error}");
                    }
                });
                if let Err(error) = resultado {
                    eprintln!(
                        "el atajo de los ultimos segundos {} no se ha podido registrar: {error}",
                        settings.replay_shortcut
                    );
                } else {
                    status.replay = true;
                    puestos.push(shortcut);
                }
            }
        }
    }

    // Win+Mayus+S solo se pide si se acepto pagar lo que cuesta, y va la ultima porque
    // antes hay que saber si el usuario ya la puso como atajo suyo.
    //
    // Ese caso existe y no es raro: quien viene de la Herramienta de Recortes escribe esa
    // misma combinacion en su atajo de capturar. Entonces la tecla YA es nuestra, y pedirla
    // otra vez falla con "ya registrada", que es exactamente el mismo error que devuelve
    // cuando la tiene el escritorio. Sin distinguirlos, los ajustes decian "el escritorio
    // todavia la tiene" sobre una tecla que estaba funcionando.
    if settings.take_win_shift_s {
        if let Ok(shortcut) = WIN_SHIFT_S.parse::<Shortcut>() {
            if puestos.contains(&shortcut) {
                status.win_shift_s = true;
                eprintln!("[atajo] Win+Mayus+S ya es nuestra: es un atajo del usuario");
            } else if manager.on_shortcut(shortcut, on_capture).is_ok() {
                status.win_shift_s = true;
                puestos.push(shortcut);
                eprintln!("[atajo] Win+Mayus+S tambien es nuestra");
            } else {
                eprintln!("[atajo] Win+Mayus+S sigue siendo del escritorio");
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
        "[atajo] registrados: captura={} ({}, sistema={}) editor={} ({}, sistema={}) grabacion={} imprPant={} (sistema={})",
        status.capture,
        settings.capture_shortcut,
        de_verdad(&settings.capture_shortcut),
        status.edit,
        settings.edit_shortcut,
        de_verdad(&settings.edit_shortcut),
        status.record,
        status.print_screen,
        de_verdad(PRINT_SCREEN)
    );
    *state.registered.write() = puestos;
    *state.shortcuts.write() = status;
    status
}

/// Los atajos de Wayland: se le piden al escritorio la captura y la captura al editor, con las
/// teclas de los ajustes como sugerencia. El escritorio pregunta (GNOME ensenna un dialogo
/// donde la persona confirma o cambia la tecla) y lo que quede puesto lo apunta el portal en
/// `AppState::shortcuts` cuando conteste. Grabar no se pide hasta que grabe (Fase 2 del plan de
/// Linux), ni Impr Pant, que en GNOME y en KDE ya es del escritorio.
#[cfg(target_os = "linux")]
fn atajos_de_wayland(app: &AppHandle, settings: &Settings) -> ShortcutStatus {
    use crate::platform::portal::{pedir_atajos, Peticion};
    // Los mismos textos de la interfaz (`src/lib/textos-en.ts` y `textos-zh.ts`).
    let (captura, editor) = match crate::tray::idioma(app) {
        "es" => ("Capturar una región", "Capturar y abrir el editor"),
        "zh" => ("区域截图", "截图并打开编辑器"),
        _ => ("Capture a region", "Capture and open the editor"),
    };
    let tecla_captura = tecla_para_el_portal(&settings.capture_shortcut);
    // Dos atajos con la misma tecla no los pone nadie: el del editor va entonces sin sugerencia
    // y la elige la persona en el dialogo.
    let tecla_editor =
        tecla_para_el_portal(&settings.edit_shortcut).filter(|tecla| Some(tecla) != tecla_captura.as_ref());
    pedir_atajos(
        app,
        vec![
            Peticion { id: "capture", descripcion: captura.into(), tecla: tecla_captura },
            Peticion { id: "edit", descripcion: editor.into(), tecla: tecla_editor },
        ],
    );
    *app.state::<AppState>().shortcuts.read()
}

/// Un atajo de los ajustes (`CmdOrCtrl+Shift+2`, el formato del plugin) escrito como lo pide el
/// portal: los modificadores `CTRL`, `ALT`, `SHIFT` y `LOGO`, y la tecla con su nombre de
/// xkbcommon (`2`, `l`, `Print`, `F5`, `Return`), unidos con `+`. Lo dice la especificacion
/// de atajos de freedesktop (specifications.freedesktop.org/shortcuts, `CTRL+SHIFT+a`).
///
/// `None` si la tecla no tiene equivalente seguro: entonces se pide el atajo sin sugerencia y
/// la tecla la pone la persona en el dialogo del escritorio, que es mejor que proponer otra.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn tecla_para_el_portal(atajo: &str) -> Option<String> {
    let mut modificadores: Vec<&str> = Vec::new();
    let mut tecla: Option<String> = None;
    for parte in atajo.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let modificador = match parte.to_ascii_lowercase().as_str() {
            "cmdorctrl" | "commandorcontrol" | "ctrl" | "control" => Some("CTRL"),
            "alt" | "option" | "altgr" => Some("ALT"),
            "shift" => Some("SHIFT"),
            "super" | "meta" | "cmd" | "command" => Some("LOGO"),
            _ => None,
        };
        match modificador {
            Some(m) => {
                if !modificadores.contains(&m) {
                    modificadores.push(m);
                }
            }
            // Dos teclas que no son modificadores: no es un atajo que se pueda escribir.
            None if tecla.is_some() => return None,
            None => tecla = Some(nombre_xkb(parte)?),
        }
    }
    let tecla = tecla?;
    // Siempre en el mismo orden, el de los ejemplos de la especificacion.
    let mut partes: Vec<&str> =
        ["CTRL", "ALT", "SHIFT", "LOGO"].into_iter().filter(|m| modificadores.contains(m)).collect();
    partes.push(&tecla);
    Some(partes.join("+"))
}

/// El nombre de xkbcommon de una tecla del plugin (sin el `XKB_KEY_` delante).
fn nombre_xkb(tecla: &str) -> Option<String> {
    // `KeyA` y `Digit2` son los codigos que guarda el selector de teclas; `A` y `2`, lo que
    // escribe una persona a mano.
    let sin_prefijo = tecla
        .strip_prefix("Key")
        .filter(|resto| resto.len() == 1)
        .or_else(|| tecla.strip_prefix("Digit").filter(|resto| resto.len() == 1))
        .unwrap_or(tecla);
    let mut letras = sin_prefijo.chars();
    if let (Some(c), None) = (letras.next(), letras.next()) {
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_lowercase().to_string());
        }
    }
    if let Some(numero) = sin_prefijo.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
        return (1..=24).contains(&numero).then(|| format!("F{numero}"));
    }
    if let Some(cifra) = sin_prefijo.strip_prefix("Numpad").filter(|n| n.len() == 1 && n.chars().all(|c| c.is_ascii_digit())) {
        return Some(format!("KP_{cifra}"));
    }
    let nombre = match sin_prefijo.to_ascii_lowercase().as_str() {
        "printscreen" | "print" => "Print",
        "space" => "space",
        "enter" | "return" => "Return",
        "tab" => "Tab",
        "escape" | "esc" => "Escape",
        "backspace" => "BackSpace",
        "delete" => "Delete",
        "insert" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" => "Page_Up",
        "pagedown" => "Page_Down",
        "arrowup" | "up" => "Up",
        "arrowdown" | "down" => "Down",
        "arrowleft" | "left" => "Left",
        "arrowright" | "right" => "Right",
        "minus" => "minus",
        "equal" => "equal",
        "comma" => "comma",
        "period" => "period",
        "slash" => "slash",
        "backslash" => "backslash",
        "semicolon" => "semicolon",
        "quote" => "apostrophe",
        "backquote" => "grave",
        "bracketleft" => "bracketleft",
        "bracketright" => "bracketright",
        "pause" => "Pause",
        "scrolllock" => "Scroll_Lock",
        _ => return None,
    };
    Some(nombre.into())
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
            // La del editor. Lleva la tecla Windows, como Win+Mayus+S.
            "Super+Shift+KeyL",
        ] {
            assert!(sugerida.parse::<Shortcut>().is_ok(), "{sugerida}");
        }
    }

    /// Las teclas de fabrica y las que propone la bienvenida, como las pide el portal de atajos.
    #[test]
    fn los_atajos_se_escriben_como_los_pide_el_portal() {
        assert_eq!(tecla_para_el_portal("CmdOrCtrl+Shift+2").as_deref(), Some("CTRL+SHIFT+2"));
        assert_eq!(tecla_para_el_portal("Super+Shift+KeyL").as_deref(), Some("SHIFT+LOGO+l"));
        assert_eq!(tecla_para_el_portal("Ctrl+Alt+A").as_deref(), Some("CTRL+ALT+a"));
        assert_eq!(tecla_para_el_portal("Alt+X").as_deref(), Some("ALT+x"));
        assert_eq!(tecla_para_el_portal("Shift+Digit5").as_deref(), Some("SHIFT+5"));
        assert_eq!(tecla_para_el_portal("PrintScreen").as_deref(), Some("Print"));
        assert_eq!(tecla_para_el_portal("Ctrl+F12").as_deref(), Some("CTRL+F12"));
        assert_eq!(tecla_para_el_portal("Ctrl+Shift+Numpad7").as_deref(), Some("CTRL+SHIFT+KP_7"));
        assert_eq!(tecla_para_el_portal("Ctrl+Shift+Backquote").as_deref(), Some("CTRL+SHIFT+grave"));
        assert_eq!(tecla_para_el_portal("Shift+ArrowLeft").as_deref(), Some("SHIFT+Left"));
    }

    /// Lo que no tiene traduccion segura no se inventa: se pide sin sugerencia.
    #[test]
    fn lo_que_no_se_puede_escribir_va_sin_sugerencia() {
        assert_eq!(tecla_para_el_portal(""), None);
        assert_eq!(tecla_para_el_portal("Ctrl+Shift"), None);
        assert_eq!(tecla_para_el_portal("Ctrl+A+B"), None);
        assert_eq!(tecla_para_el_portal("Ctrl+MediaPlayPause"), None);
        assert_eq!(tecla_para_el_portal("Ctrl+F25"), None);
    }
}
