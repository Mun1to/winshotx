use std::sync::atomic::{AtomicU32, Ordering};

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::capture::{self, Rect};
use crate::error::Result;
use crate::state::AppState;

pub const OVERLAY_PREFIX: &str = "overlay-";

/// Cerrar una ventana de Tauri es asincrono: si se reutilizara la etiqueta, el
/// siguiente disparo del atajo fallaria con "ya existe una ventana con esa etiqueta".
static OVERLAY_SEQUENCE: AtomicU32 = AtomicU32::new(0);
pub const RECORDER_PREFIX: &str = "recorder-";
pub const EDITOR_LABEL: &str = "editor";

/// Congela la pantalla y abre un overlay por monitor.
/// Congelar primero es lo que hace que la seleccion sea estable y precisa al pixel.
pub fn open_overlays(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    if state.is_recording() {
        return Ok(());
    }
    close_overlays(app);

    let freezes = capture::freeze_all(&state.freeze_dir())?;
    let monitors: Vec<_> = freezes.iter().map(|f| f.monitor.clone()).collect();
    *state.freezes.write() = freezes;

    let round = OVERLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    for monitor in monitors {
        let label = format!("{OVERLAY_PREFIX}{}-{round}", monitor.id);
        let url = WebviewUrl::App(format!("overlay.html?monitor={}", monitor.id).into());
        let window = WebviewWindowBuilder::new(app, &label, url)
            .title("winshotx")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .shadow(false)
            .visible(false)
            // Sin esto, el primer clic del usuario se lo come Windows para activar
            // la ventana y la seleccion nunca empieza.
            .focused(true)
            .build()?;

        // Posicion y tamanno en pixeles fisicos: el escalado por DPI no debe tocarlos.
        window.set_position(PhysicalPosition::new(monitor.x, monitor.y))?;
        window.set_size(PhysicalSize::new(monitor.width, monitor.height))?;
        window.show()?;
        let _ = window.set_focus();
        #[cfg(windows)]
        force_foreground(&window);
    }
    Ok(())
}

pub fn close_overlays(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(OVERLAY_PREFIX) {
            let _ = window.close();
        }
    }
}

/// Barra flotante que acompanna a la grabacion, justo debajo de la region.
/// Las ventanas se crean en el hilo principal: hacerlo desde el hilo de un comando
/// bloquea a la espera del bucle de eventos y la grabacion se queda colgada.
pub fn open_recorder(app: &AppHandle, region: Rect) -> Result<()> {
    close_recorder(app);
    let label = format!(
        "{RECORDER_PREFIX}{}",
        OVERLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("recorder.html".into()))
        .title("winshotx")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(true)
        .visible(false)
        .inner_size(360.0, 52.0)
        .build()?;
    crate::platform::window_style::rounded_corners(&window);

    let scale = window.scale_factor().unwrap_or(1.0);
    let width_px = (360.0 * scale) as i32;
    let x = region.x + (region.width as i32 - width_px) / 2;
    let y = region.y + region.height as i32 + (12.0 * scale) as i32;
    window.set_position(PhysicalPosition::new(x.max(0), y.max(0)))?;
    window.show()?;
    Ok(())
}

pub fn close_recorder(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(RECORDER_PREFIX) {
            let _ = window.close();
        }
    }
}

/// Editor de recorte y exportacion para una sesion ya grabada.
pub fn open_editor(app: &AppHandle, session_id: &str) -> Result<()> {
    for (label, window) in app.webview_windows() {
        if label.starts_with(EDITOR_LABEL) {
            let _ = window.close();
        }
    }
    let label = format!(
        "{EDITOR_LABEL}-{}",
        OVERLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let url = WebviewUrl::App(format!("editor.html?session={session_id}").into());
    let window = WebviewWindowBuilder::new(app, &label, url)
        .title("winshotx · editor")
        .decorations(false)
        .shadow(true)
        .resizable(true)
        .inner_size(1080.0, 700.0)
        .min_inner_size(760.0, 520.0)
        .center()
        .visible(true)
        .build()?;
    crate::platform::window_style::rounded_corners(&window);
    let _ = window.set_min_size(Some(LogicalSize::new(760.0, 520.0)));
    window.set_focus()?;
    Ok(())
}

/// La ventana de ajustes no se destruye al cerrarla, solo se esconde, asi que su
/// interfaz sigue montada con los datos del arranque. Al volver a mostrarla hay que
/// decirselo para que refresque el tamanno de la cache y mire si hay version nueva.
pub fn show_settings(app: &AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.set_focus()?;
        let _ = window.emit(crate::EVENT_SETTINGS_SHOWN, ());
    }
    Ok(())
}

/// Windows solo deja robar el primer plano a quien tiene derecho a ello.
/// Al venir de un atajo global lo tenemos, pero hay que pedirlo explicitamente.
#[cfg(windows)]
fn force_foreground(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, SW_SHOW, ShowWindow};

    let Ok(handle) = window.hwnd() else { return };
    unsafe {
        let hwnd = HWND(handle.0 as *mut std::ffi::c_void);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}
