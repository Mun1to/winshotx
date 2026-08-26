use std::sync::atomic::{AtomicU32, Ordering};

use serde::Serialize;

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::capture::{self, Rect};
use crate::error::Result;
use crate::state::AppState;

pub const OVERLAY_PREFIX: &str = "overlay-";

static OVERLAY_SEQUENCE: AtomicU32 = AtomicU32::new(0);
pub const RECORDER_PREFIX: &str = "recorder-";
pub const EDITOR_LABEL: &str = "editor";

/// Con que intencion se ha abierto el overlay. Lo decide el atajo que se pulso, y
/// es lo que deja que el modo instantaneo copie al soltar sin cargarse la grabacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayIntent {
    Capture,
    Record,
}

/// Congela la pantalla y abre un overlay por monitor.
/// Congelar primero es lo que hace que la seleccion sea estable y precisa al pixel.
///
/// Las ventanas overlay se reutilizan entre capturas en vez de crearse de cero cada vez:
/// construir un WebView2 nuevo costaba entre 200ms y 800ms por pantalla (variable, y el
/// peor coste con tres monitores), y ese tiempo desaparece del todo si la ventana ya
/// existe, oculta, de la vez anterior. `close_overlays` las esconde en vez de cerrarlas
/// para que quede algo que reutilizar. Cuando se reutiliza una ventana no llega un
/// remontaje de React que dispare su arranque solo, asi que se le avisa con
/// `EVENT_OVERLAY_SHOW` para que recargue la captura nueva.
pub fn open_overlays(app: &AppHandle, intent: OverlayIntent) -> Result<()> {
    let arranque = std::time::Instant::now();
    let state = app.state::<AppState>();
    if state.is_recording() {
        return Ok(());
    }
    // Si el atajo se pulsa dos veces muy seguidas, la segunda pulsacion se ignora en vez
    // de arrancar otra captura por encima de la que ya esta en marcha: la primera va a
    // dejar el overlay abierto igual, y dejar que dos disparos capturen pantalla a la vez
    // solo consigue que las dos vayan mas lentas.
    let Some(_candado) = state.intentar_capturar() else {
        return Ok(());
    };
    *state.intent.write() = intent;

    let freezes = capture::freeze_all(&state.freeze_dir())?;
    let monitors: Vec<_> = freezes.iter().map(|f| f.monitor.clone()).collect();
    for m in &monitors {
        eprintln!(
            "[medir-mon] id={} x={} y={} w={} h={} scale={} primary={} label={:?}",
            m.id, m.x, m.y, m.width, m.height, m.scale, m.is_primary, m.label
        );
    }
    *state.freezes.write() = freezes;

    // Si algun monitor de la ultima vez ya no existe (se desconecto una pantalla), su
    // ventana se queda escondida en el pool: no molesta a nadie ahi.
    let etiquetas_actuales: std::collections::HashSet<String> = monitors
        .iter()
        .map(|m| format!("{OVERLAY_PREFIX}{}", m.id))
        .collect();
    for (label, window) in app.webview_windows() {
        if label.starts_with(OVERLAY_PREFIX) && !etiquetas_actuales.contains(&label) {
            let _ = window.hide();
        }
    }

    let mut windows = Vec::with_capacity(monitors.len());
    for monitor in &monitors {
        let label = format!("{OVERLAY_PREFIX}{}", monitor.id);

        let window = if let Some(existente) = app.get_webview_window(&label) {
            // El payload va ya construido en el propio evento: sin esto, el frontend
            // tendria que pedirlo aparte con un invoke (overlay_bootstrap) despues de
            // enterarse, y esa vuelta de IPC completa se ahorra entera. Se manda ANTES de
            // ensennarla: asi el frontend ya ha vaciado su pantalla vieja (vuelve al
            // BootScreen) para cuando la ventana se hace visible, en vez de enseñar un
            // instante la captura de la vez anterior.
            // `emit` manda el evento a TODAS las ventanas, no solo a `existente`: con eso
            // las tres pantallas recibian el payload de la ultima que se procesaba en
            // este bucle y se pisaban entre si. `emit_to` con la etiqueta de esta ventana
            // es lo que lo manda solo a ella.
            if let Ok(payload) = crate::commands::build_overlay_payload(&state, monitor.id) {
                let _ = existente.emit_to(label.as_str(), crate::EVENT_OVERLAY_SHOW, payload);
            }
            existente
        } else {
            build_overlay_window(app, &label, monitor.id)?
        };

        // Posicion y tamanno en pixeles fisicos: el escalado por DPI no debe tocarlos.
        // Se repiten en cada apertura por si el monitor cambio de sitio o de resolucion
        // desde la ultima vez que se uso esta ventana.
        window.set_position(PhysicalPosition::new(monitor.x, monitor.y))?;
        window.set_size(PhysicalSize::new(monitor.width, monitor.height))?;
        window.show()?;
        // TEMPORAL: confirmar que la ventana termina de verdad donde se le pidio.
        if let Ok(pos) = window.outer_position() {
            eprintln!(
                "[medir-win] {}: pedido=({},{}) real=({},{})",
                label, monitor.x, monitor.y, pos.x, pos.y
            );
        }
        windows.push((monitor, window));
    }

    // El foco de teclado se pide una sola vez, a la pantalla donde esta el cursor: es la
    // que el usuario esta mirando y donde va a empezar a arrastrar o a pulsar Escape.
    let cursor = cursor_position();
    let objetivo = cursor
        .and_then(|(x, y)| windows.iter().find(|(m, _)| m.contains(x, y)))
        .or_else(|| windows.iter().find(|(m, _)| m.is_primary))
        .or_else(|| windows.first());
    if let Some((_, window)) = objetivo {
        // Sin esto, el primer clic del usuario se lo come Windows para activar
        // la ventana y la seleccion nunca empieza.
        let _ = window.set_focus();
        #[cfg(windows)]
        force_foreground(window);
    }

    eprintln!("[medir] open_overlays: {:?}", arranque.elapsed());
    Ok(())
}

fn build_overlay_window(app: &AppHandle, label: &str, monitor_id: u32) -> Result<tauri::WebviewWindow> {
    let url = WebviewUrl::App(format!("overlay.html?monitor={monitor_id}").into());
    Ok(WebviewWindowBuilder::new(app, label, url)
        .title("winshotx")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .visible(false)
        .build()?)
}

/// Crea de antemano las ventanas overlay de los monitores actuales, ocultas, sin esperar
/// a la primera captura del dia. Se llama una vez al arrancar la app: en ese momento no
/// hay ninguna ventana visible que el usuario este esperando (winshotx arranca en la
/// bandeja), asi que el coste de montar cada WebView2 no lo nota nadie. Sin esto, la
/// primera vez que se pulsa el atajo tras abrir la app tiene que pagar ese coste igual
/// que antes del pool.
pub fn precrear_overlays(app: &AppHandle) {
    let Ok(monitors) = xcap::Monitor::all() else {
        return;
    };
    for (index, monitor) in monitors.iter().enumerate() {
        let label = format!("{OVERLAY_PREFIX}{index}");
        if app.get_webview_window(&label).is_some() {
            continue;
        }
        let (Ok(x), Ok(y), Ok(width), Ok(height)) =
            (monitor.x(), monitor.y(), monitor.width(), monitor.height())
        else {
            continue;
        };
        let Ok(window) = build_overlay_window(app, &label, index as u32) else {
            continue;
        };
        let _ = window.set_position(PhysicalPosition::new(x, y));
        let _ = window.set_size(PhysicalSize::new(width, height));
    }
}

#[cfg(windows)]
fn cursor_position() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    Some((point.x, point.y))
}

#[cfg(not(windows))]
fn cursor_position() -> Option<(i32, i32)> {
    None
}

/// Esconde el overlay en vez de cerrarlo: la ventana se reutiliza en la siguiente
/// captura, ver `open_overlays`.
pub fn close_overlays(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(OVERLAY_PREFIX) {
            let _ = window.hide();
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
        let _ = window.unminimize();
        let _ = window.set_focus();
        // Lo mismo que el overlay, y por lo mismo: `show` y `set_focus` no bastan cuando
        // la orden no viene de un clic del usuario en NUESTRA ventana. Desde el menu de la
        // bandeja, o desde una segunda instancia que pasa el testigo, Windows nos niega el
        // primer plano y la ventana se quedaba escondida sin decir ni un error.
        #[cfg(windows)]
        force_foreground(&window);
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
