//! Hook de teclado de bajo nivel: la unica forma de ganarle una tecla al sistema.
//!
//! `RegisterHotKey` es la via limpia y es la que se usa primero, pero pierde siempre contra
//! un `WH_KEYBOARD_LL`: los hooks de bajo nivel ven la pulsacion antes que el sistema de
//! atajos y pueden tragarsela. Eso es lo que hace el shell con Win+Mayus+S, y lo que hacen
//! los programas de captura que ya vienen instalados. Con el atajo registrado y funcionando
//! sobre el papel, no llega ni una pulsacion y no hay nada en la app que lo explique.
//!
//! Este hook mira **tres combinaciones y ninguna mas**: el atajo de captura que el usuario
//! haya elegido, la tecla Impr Pant y Win+Mayus+S. No guarda ninguna tecla, no las apunta en
//! ningun sitio y deja pasar todo lo demas sin tocarlo.

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::OnceLock;

use tauri::AppHandle;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, HHOOK,
    KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::windows_mgr::{self, OverlayIntent};

/// Codigos virtuales de las dos teclas de captura de Windows.
const VK_S: u32 = 0x53;
const VK_SNAPSHOT: u32 = 0x2C;

/// Bits de modificador, en el mismo orden que los guarda `objetivo`.
const MOD_CTRL: u32 = 1 << 8;
const MOD_SHIFT: u32 = 1 << 9;
const MOD_ALT: u32 = 1 << 10;
const MOD_WIN: u32 = 1 << 11;

/// Por aqui se avisa al hilo que abre el overlay.
///
/// **El callback no puede hacer nada mas que esto.** Mientras un hook de bajo nivel no
/// devuelve el control, Windows tiene parada la entrada de teclado y raton de TODO el
/// sistema. Hablar ahi con el bucle de eventos de Tauri, que es lo que hacia la primera
/// version, comparte cerrojos con el hilo principal: si ese hilo esta ocupado congelando
/// las pantallas para una captura, el equipo entero se queda tieso.
static AVISO: OnceLock<SyncSender<()>> = OnceLock::new();
/// El atajo de captura del usuario, empaquetado como modificadores + codigo virtual.
/// Cero significa que no hay ninguno que vigilar.
static OBJETIVO: AtomicU32 = AtomicU32::new(0);
/// Si ademas hay que quedarse con Impr Pant y con Win+Mayus+S.
static TECLAS_DE_WINDOWS: AtomicBool = AtomicBool::new(false);
static INSTALADO: AtomicBool = AtomicBool::new(false);
/// Que modificadores estan abajo, contados por el propio hook.
///
/// Dentro de un hook de bajo nivel `GetAsyncKeyState` no vale para esto: el estado del
/// teclado se actualiza *despues* de que la cadena de hooks decida, asi que preguntando ahi
/// se pierden pulsaciones enteras. Lo unico fiable es apuntar cada tecla al verla pasar.
static MODS: AtomicU32 = AtomicU32::new(0);

/// Si la tecla es un modificador, devuelve cual.
fn bit_de_modificador(vk: u32) -> Option<u32> {
    match vk {
        0x11 | 0xA2 | 0xA3 => Some(MOD_CTRL),   // Control, izquierdo y derecho
        0x10 | 0xA0 | 0xA1 => Some(MOD_SHIFT),  // Shift
        0x12 | 0xA4 | 0xA5 => Some(MOD_ALT),    // Alt
        0x5B | 0x5C => Some(MOD_WIN),           // Win
        _ => None,
    }
}

/// Solo lo que el propio hook ha ido apuntando. Nada de preguntarle al sistema: dentro de
/// un hook de bajo nivel cada llamada se paga con la entrada de todo el escritorio parada,
/// y ademas `GetAsyncKeyState` ahi no es de fiar, que es como se perdio Win+Mayus+S.
fn modificadores() -> u32 {
    MODS.load(Ordering::Relaxed)
}

/// Traduce un atajo del formato de Tauri ("CmdOrCtrl+Shift+KeyS") a modificadores y codigo
/// virtual. Devuelve None cuando la tecla final no es de las que se pueden vigilar aqui.
pub fn empaquetar(atajo: &str) -> Option<u32> {
    let mut mods = 0;
    let mut vk = None;
    for parte in atajo.split('+').filter(|p| !p.is_empty()) {
        match parte.to_ascii_lowercase().as_str() {
            "cmdorctrl" | "commandorcontrol" | "ctrl" | "control" => mods |= MOD_CTRL,
            "shift" => mods |= MOD_SHIFT,
            "alt" | "option" => mods |= MOD_ALT,
            "super" | "meta" | "win" | "windows" | "cmd" | "command" => mods |= MOD_WIN,
            otra => vk = codigo_virtual(otra),
        }
    }
    vk.map(|codigo| mods | codigo)
}

fn codigo_virtual(tecla: &str) -> Option<u32> {
    let bytes = tecla.as_bytes();
    if let Some(letra) = tecla.strip_prefix("key") {
        // KeyA..KeyZ son las letras, y su codigo virtual es la mayuscula en ASCII.
        let letra = letra.as_bytes().first()?.to_ascii_uppercase();
        return letra.is_ascii_uppercase().then_some(letra as u32);
    }
    if let Some(digito) = tecla.strip_prefix("digit") {
        let digito = digito.as_bytes().first()?;
        return digito.is_ascii_digit().then_some(*digito as u32);
    }
    if bytes.first() == Some(&b'f') && bytes.len() <= 3 {
        // F1..F12 van seguidas a partir de 0x70.
        let numero: u32 = tecla[1..].parse().ok()?;
        return (1..=12).contains(&numero).then_some(0x6F + numero);
    }
    match tecla {
        "printscreen" => Some(VK_SNAPSHOT),
        "space" => Some(0x20),
        "enter" | "return" => Some(0x0D),
        "tab" => Some(0x09),
        _ => None,
    }
}

/// Si el hook llego a instalarse. Cuando esta puesto, las teclas se consiguen aunque
/// `RegisterHotKey` las de por ocupadas, que es justo lo que pasa con las de Windows.
pub fn instalado() -> bool {
    INSTALADO.load(Ordering::SeqCst)
}

/// Que combinaciones tiene que vigilar el hook. Se llama en cada cambio de ajustes.
pub fn vigilar(atajo_captura: &str, teclas_de_windows: bool) {
    OBJETIVO.store(empaquetar(atajo_captura).unwrap_or(0), Ordering::Relaxed);
    TECLAS_DE_WINDOWS.store(teclas_de_windows, Ordering::Relaxed);
}

fn es_nuestra(vk: u32) -> bool {
    let mods = modificadores();
    let objetivo = OBJETIVO.load(Ordering::Relaxed);
    if objetivo != 0 && objetivo == (mods | vk) {
        return true;
    }
    if TECLAS_DE_WINDOWS.load(Ordering::Relaxed) {
        // Las dos de Windows: la tecla suelta y la combinacion del shell.
        if vk == VK_SNAPSHOT && mods & (MOD_CTRL | MOD_ALT | MOD_WIN) == 0 {
            return true;
        }
        if vk == VK_S && mods & MOD_WIN != 0 && mods & MOD_SHIFT != 0 {
            return true;
        }
    }
    false
}

/// Abre el overlay sin bloquear el hook. Windows desinstala un hook de bajo nivel que
/// tarde demasiado en responder, asi que aqui solo se encola y se vuelve enseguida.
fn disparar() {
    let Some(aviso) = AVISO.get() else { return };
    // `try_send` no espera jamas: si ya hay una captura en camino, esta pulsacion sobra.
    // Un `send` normal aqui bastaria para colgar el teclado del sistema entero.
    let _ = aviso.try_send(());
}

unsafe extern "system" fn callback(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == 0 {
        let mensaje = wparam.0 as u32;
        let bajada = mensaje == WM_KEYDOWN || mensaje == WM_SYSKEYDOWN;
        let subida = mensaje == WM_KEYUP || mensaje == WM_SYSKEYUP;
        if bajada || subida {
            let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);

            // Primero se apunta el modificador, y despues se mira la combinacion: al llegar
            // la tecla final, los modificadores ya estan contados.
            if let Some(bit) = bit_de_modificador(info.vkCode) {
                let previo = MODS.load(Ordering::Relaxed);
                MODS.store(
                    if bajada { previo | bit } else { previo & !bit },
                    Ordering::Relaxed,
                );
                return CallNextHookEx(None, code, wparam, lparam);
            }

            if es_nuestra(info.vkCode) {
                if bajada {
                    disparar();
                }
                // Se traga tambien la subida: dejar pasar media pulsacion deja al programa
                // de debajo esperando una tecla que nunca se suelta.
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Instala el hook en su propio hilo, con su bucle de mensajes. Windows exige que el hilo
/// que instala un hook de bajo nivel atienda mensajes, o deja de llamarlo sin avisar.
pub fn instalar(app: AppHandle) {
    if INSTALADO.swap(true, Ordering::SeqCst) {
        return;
    }

    // Cabe un aviso y solo uno: mientras se atiende una captura, las pulsaciones que
    // lleguen se descartan en vez de amontonarse.
    let (emisor, receptor) = sync_channel::<()>(1);
    let _ = AVISO.set(emisor);
    std::thread::spawn(move || {
        for _ in receptor {
            let copia = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Err(error) = windows_mgr::open_overlays(&copia, OverlayIntent::Capture) {
                    eprintln!("[hook] no se ha podido abrir el overlay: {error}");
                }
            });
        }
    });

    std::thread::spawn(|| unsafe {
        let hook: HHOOK = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(callback), None, 0) {
            Ok(hook) => hook,
            Err(error) => {
                eprintln!("[atajo] no se ha podido instalar el hook de teclado: {error}");
                INSTALADO.store(false, Ordering::SeqCst);
                return;
            }
        };
        eprintln!("[atajo] hook de teclado instalado");

        let mut mensaje = MSG::default();
        while GetMessageW(&mut mensaje, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&mensaje);
            DispatchMessageW(&mensaje);
        }
        let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empaqueta_los_atajos_de_siempre() {
        assert_eq!(empaquetar("CmdOrCtrl+Shift+KeyS"), Some(MOD_CTRL | MOD_SHIFT | VK_S));
        assert_eq!(
            empaquetar("CmdOrCtrl+Shift+Digit2"),
            Some(MOD_CTRL | MOD_SHIFT | 0x32)
        );
        assert_eq!(empaquetar("Alt+KeyX"), Some(MOD_ALT | 0x58));
        assert_eq!(empaquetar("PrintScreen"), Some(VK_SNAPSHOT));
        assert_eq!(empaquetar("CmdOrCtrl+Alt+F5"), Some(MOD_CTRL | MOD_ALT | 0x74));
    }

    #[test]
    fn una_tecla_que_no_se_entiende_no_vigila_nada() {
        assert_eq!(empaquetar("CmdOrCtrl+Shift+Insert"), None);
        assert_eq!(empaquetar(""), None);
    }
}
