//! Esconder los iconos del escritorio mientras se congela la pantalla.
//!
//! Windows no tiene ninguna llamada para esto: la unica via es esconder la ventana que
//! los dibuja, `SHELLDLL_DefView`, que es hija del escritorio. Lo que se esconde es la
//! vista entera, asi que el fondo de pantalla se queda y solo desaparecen los iconos.
//!
//! **Lo importante es devolverlos.** Si se quedan escondidos, el usuario no puede
//! arreglarlo desde el menu del escritorio (ese menu tambien vive en la ventana que
//! acabamos de esconder) y tendria que reiniciar el Explorador. Por eso esto devuelve un
//! guardian que los saca de nuevo al destruirse, y por eso en `windows_mgr` se esconden
//! solo el instante que dura el disparo y no toda la seleccion.

/// Los iconos vuelven solos cuando esto se destruye, salga la captura bien o mal.
pub struct IconosEscondidos {
    #[cfg(windows)]
    hwnd: windows::Win32::Foundation::HWND,
}

// Un HWND es un identificador opaco del sistema, no un puntero a memoria de este proceso:
// `ShowWindow` funciona desde cualquier hilo. Hace falta para que el guardian viva en el
// hilo que congela las pantallas, que no es el principal.
#[cfg(windows)]
unsafe impl Send for IconosEscondidos {}

#[cfg(windows)]
impl Drop for IconosEscondidos {
    fn drop(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOW};
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }
}

#[cfg(not(windows))]
impl Drop for IconosEscondidos {
    fn drop(&mut self) {}
}

/// `None` si no hay nada que esconder: o no se encuentra el escritorio, o ya estaba sin
/// iconos porque el usuario los quito el mismo. En los dos casos hay que no tocar nada,
/// y sobre todo no devolver un guardian que al soltarse le ENSEÑE los iconos a quien
/// habia elegido tenerlos apagados.
#[cfg(windows)]
pub fn esconder() -> Option<IconosEscondidos> {
    use windows::Win32::UI::WindowsAndMessaging::{IsWindowVisible, ShowWindow, SW_HIDE};

    let hwnd = unsafe { vista_del_escritorio() }?;
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        let _ = ShowWindow(hwnd, SW_HIDE);
    }

    // El fondo de pantalla se repinta cuando le llega el turno al Explorador, no cuando
    // se lo pedimos: capturar en el mismo instante puede devolver la pantalla de antes,
    // con los iconos todavia puestos. Esta espera es el precio entero de la opcion y solo
    // se paga con ella encendida. 120 ms es una estimacion, no una medicion: si en alguna
    // maquina se cuelan iconos, el numero a subir es este.
    std::thread::sleep(std::time::Duration::from_millis(120));

    Some(IconosEscondidos { hwnd })
}

#[cfg(not(windows))]
pub fn esconder() -> Option<IconosEscondidos> {
    None
}

/// La ventana que dibuja los iconos. Cuelga de `Progman`, salvo cuando el fondo de
/// pantalla es una presentacion o un video: entonces el Explorador crea un `WorkerW` por
/// encima y se lleva la vista ahi. Hay que mirar en los dos sitios, y el segundo se
/// recorre a mano porque puede haber varios `WorkerW` y solo uno tiene la vista dentro.
#[cfg(windows)]
unsafe fn vista_del_escritorio() -> Option<windows::Win32::Foundation::HWND> {
    use windows::core::w;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, FindWindowW};

    let vista_de = |padre: HWND| -> Option<HWND> {
        unsafe { FindWindowExW(Some(padre), None, w!("SHELLDLL_DefView"), None) }.ok()
    };

    if let Ok(progman) = unsafe { FindWindowW(w!("Progman"), None) } {
        if let Some(vista) = vista_de(progman) {
            return Some(vista);
        }
    }

    let mut anterior: Option<HWND> = None;
    loop {
        let worker = unsafe { FindWindowExW(None, anterior, w!("WorkerW"), None) }.ok()?;
        if let Some(vista) = vista_de(worker) {
            return Some(vista);
        }
        anterior = Some(worker);
    }
}
