//! Esquinas redondeadas de verdad. Dibujarlas en CSS sobre una ventana transparente
//! deja un halo negro alrededor; Windows 11 sabe recortarlas él mismo.

#[cfg(windows)]
pub fn rounded_corners(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };

    let Ok(handle) = window.hwnd() else { return };
    unsafe {
        let hwnd = HWND(handle.0);
        let preference = DWMWCP_ROUND;
        // En Windows 10 este atributo no existe: el error se ignora sin más.
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const preference).cast(),
            std::mem::size_of_val(&preference) as u32,
        );
    }
}

#[cfg(not(windows))]
pub fn rounded_corners(_window: &tauri::WebviewWindow) {}

/// Una ventana que no coge el foco JAMAS, ni al mostrarse ni al hacerle clic encima.
///
/// El `.focused(false)` del constructor solo vale para el primer momento: en cuanto se
/// esconde y se vuelve a mostrar, `ShowWindow` la activa como a cualquier otra. Para la
/// cuenta atras eso no es un detalle estetico, es lo unico que decide si la funcion sirve:
/// el temporizador existe para fotografiar un menu abierto, y robarle el foco al menu lo
/// cierra, que es exactamente lo que veniamos a evitar. `WS_EX_NOACTIVATE` se lo prohibe
/// a Windows, y `WS_EX_TOOLWINDOW` la deja fuera de Alt+Tab.
#[cfg(windows)]
pub fn never_focus(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    let Ok(handle) = window.hwnd() else { return };
    unsafe {
        let hwnd = HWND(handle.0);
        let actual = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let nuevo = actual | (WS_EX_NOACTIVATE.0 as isize) | (WS_EX_TOOLWINDOW.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, nuevo);
    }
}

#[cfg(not(windows))]
pub fn never_focus(_window: &tauri::WebviewWindow) {}

/// Mueve la ventana a ese punto y la ensenna, de una sola vez y sin activarla.
///
/// Es para los overlays, que esperan aparcados fuera de las pantallas a tener su imagen:
/// en el momento de aparecer cada llamada a Windows cuenta, y `set_position` mas `show`
/// eran dos, cada una con su vuelta por el compositor. `SWP_NOACTIVATE` porque el foco se
/// le da aparte y solo a la ventana de la pantalla del raton. Devuelve `false` si no ha
/// podido, para que quien llama use el camino normal.
#[cfg(windows)]
pub fn colocar_y_ensennar(window: &tauri::WebviewWindow, x: i32, y: i32) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    let Ok(handle) = window.hwnd() else { return false };
    unsafe {
        SetWindowPos(
            HWND(handle.0),
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE,
        )
        .is_ok()
    }
}

#[cfg(not(windows))]
pub fn colocar_y_ensennar(_window: &tauri::WebviewWindow, _x: i32, _y: i32) -> bool {
    false
}

/// Deja la ventana fuera de lo que se graba, sin esconderla.
///
/// `WDA_EXCLUDEFROMCAPTURE` le dice al compositor de Windows que, para quien fotografia o
/// graba la pantalla, esa ventana no esta: quien esta delante la sigue viendo y la sigue
/// pulsando, y en el video sale lo que hay detras. Es lo mismo que hacen las aplicaciones
/// de videollamada para que su propia ventana no se cuele al compartir pantalla.
///
/// Existe desde Windows 10 2004. En uno mas viejo la llamada falla, se devuelve `false` y
/// la ventana se queda como estaba, que es exactamente lo de antes.
#[cfg(windows)]
pub fn fuera_de_la_captura(window: &tauri::WebviewWindow, fuera: bool) -> bool {
    let Ok(handle) = window.hwnd() else { return false };
    return fuera_de_la_captura_hwnd(handle.0 as isize, fuera);
}

/// Lo mismo, sobre una ventana de Windows a pelo. Va aparte para poder probarlo: crear un
/// `WebviewWindow` en una prueba significa levantar media aplicacion, y una ventana del
/// sistema son tres lineas.
#[cfg(windows)]
pub fn fuera_de_la_captura_hwnd(ventana: isize, fuera: bool) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    };

    let hwnd = HWND(ventana as *mut core::ffi::c_void);
    let afinidad = if fuera { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
    return unsafe { SetWindowDisplayAffinity(hwnd, afinidad) }.is_ok();
}

#[cfg(not(windows))]
pub fn fuera_de_la_captura(_window: &tauri::WebviewWindow, _fuera: bool) -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        WDA_NONE, WINDOW_EX_STYLE, WS_POPUP,
    };

    /// Una ventana de verdad, de la clase que ya trae Windows, y fuera de todas las
    /// pantallas para que no la vea nadie mientras corren las pruebas.
    fn ventana_de_prueba() -> isize {
        let clase: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(clase.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                -20_000,
                -20_000,
                40,
                20,
                None,
                None,
                None,
                None,
            )
        }
        .expect("no se ha podido crear la ventana de prueba");
        hwnd.0 as isize
    }

    fn afinidad(ventana: isize) -> u32 {
        let mut valor = 0u32;
        unsafe {
            GetWindowDisplayAffinity(HWND(ventana as *mut core::ffi::c_void), &mut valor)
                .expect("no se ha podido leer la afinidad");
        }
        valor
    }

    /// Que Windows acepte la orden es lo unico que no se puede saber leyendo el codigo: en
    /// una version vieja, o en una ventana que no la admita, `SetWindowDisplayAffinity`
    /// devuelve error y el menu seguiria saliendo en el video sin que nadie se entere.
    #[test]
    fn la_ventana_se_puede_sacar_de_la_captura_y_devolver() {
        let ventana = ventana_de_prueba();
        assert!(
            fuera_de_la_captura_hwnd(ventana, true),
            "Windows no ha aceptado sacar la ventana de la captura"
        );
        assert_eq!(afinidad(ventana), WDA_EXCLUDEFROMCAPTURE.0);

        assert!(fuera_de_la_captura_hwnd(ventana, false));
        assert_eq!(
            afinidad(ventana),
            WDA_NONE.0,
            "tiene que volver a poder fotografiarse cuando ya no se graba"
        );

        unsafe {
            let _ = DestroyWindow(HWND(ventana as *mut core::ffi::c_void));
        }
    }
}
