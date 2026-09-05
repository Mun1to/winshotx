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
