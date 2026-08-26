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
        let hwnd = HWND(handle.0 as *mut std::ffi::c_void);
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
        let hwnd = HWND(handle.0 as *mut std::ffi::c_void);
        let actual = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let nuevo = actual | (WS_EX_NOACTIVATE.0 as isize) | (WS_EX_TOOLWINDOW.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, nuevo);
    }
}

#[cfg(not(windows))]
pub fn never_focus(_window: &tauri::WebviewWindow) {}
