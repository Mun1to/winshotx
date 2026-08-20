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
