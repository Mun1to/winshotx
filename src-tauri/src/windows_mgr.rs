use std::sync::atomic::{AtomicU32, Ordering};

use serde::Serialize;

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::capture::{self, Rect};
use crate::error::Result;
use crate::state::{AppState, CandadoCaptura};

pub const OVERLAY_PREFIX: &str = "overlay-";

static OVERLAY_SEQUENCE: AtomicU32 = AtomicU32::new(0);
pub const RECORDER_PREFIX: &str = "recorder-";
pub const EDITOR_LABEL: &str = "editor";
/// La ventanita de la cuenta atras del temporizador. Solo hay una, en la pantalla donde
/// este el raton, y como los overlays se esconde en vez de cerrarse.
pub const COUNTDOWN_LABEL: &str = "countdown";

/// Lado de la ventanita de la cuenta atras, en pixeles logicos: en una pantalla al 150 %
/// tiene que verse igual de grande, no mas pequenna.
const CUENTA_LADO: f64 = 132.0;

/// Lo que se le da al escritorio para repintar el hueco de la cuenta atras antes de
/// congelar. Sin esto la captura se lleva la ventanita dentro, y en el centro de la
/// pantalla eso no se le escapa a nadie.
const MARGEN_REPINTADO: std::time::Duration = std::time::Duration::from_millis(120);

/// Con que intencion se ha abierto el overlay. Lo decide el atajo que se pulso, y
/// es lo que deja que el modo instantaneo copie al soltar sin cargarse la grabacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayIntent {
    Capture,
    Record,
}

/// Punto de entrada del atajo de captura: prepara la pantalla y abre la seleccion.
///
/// Entre pulsar la tecla y congelar puede haber un paso previo, y ese paso es lo unico
/// que distingue esta funcion de `congelar_y_abrir`. Hay dos cosas que **tienen que pasar
/// antes** de la foto y no despues, asi que ninguna de las dos puede ser una tecla del
/// overlay como el resto: el temporizador (que existe para que la pantalla siga como
/// estaba cuando se pulso el atajo) y esconder los iconos del escritorio.
pub fn open_overlays(app: &AppHandle, intent: OverlayIntent) -> Result<()> {
    let state = app.state::<AppState>();
    if state.is_recording() {
        return Ok(());
    }
    // Si el atajo se pulsa dos veces muy seguidas, la segunda pulsacion se ignora en vez
    // de arrancar otra captura por encima de la que ya esta en marcha: la primera va a
    // dejar el overlay abierto igual, y dejar que dos disparos capturen pantalla a la vez
    // solo consigue que las dos vayan mas lentas. Con el temporizador puesto, el candado
    // cubre tambien la espera: pulsar otra vez durante la cuenta atras no la reinicia.
    let Some(candado) = state.intentar_capturar() else {
        return Ok(());
    };
    *state.intent.write() = intent;

    // La espera es solo del atajo de capturar, aunque el overlay sea el mismo. Al grabar,
    // la pantalla congelada es un fondo para elegir la region y no la foto que se lleva
    // nadie: hacer esperar cinco segundos ahi solo seria una espera.
    let segundos = state.settings.read().capture_delay_seconds;
    if segundos == 0 || intent == OverlayIntent::Record {
        return congelar_y_abrir(app, candado);
    }

    // **El temporizador no puede dormir aqui.** Esta funcion la llama el manejador del
    // atajo global, y ese manejador corre dentro del bucle de eventos del hilo principal:
    // el plugin crea una ventana oculta que recibe el `WM_HOTKEY` y una ventana atiende
    // sus mensajes en el hilo que la creo. Un `sleep` aqui congela la aplicacion entera
    // durante los tres o cinco segundos, incluida la cuenta atras que se acaba de
    // ensennar, que se quedaria clavada en el primer numero.
    mostrar_cuenta_atras(app, segundos);
    let espera = app.clone();
    std::thread::spawn(move || {
        // La cuenta atras se esconde un pelin ANTES del final, no en el mismo instante en
        // que se congela. Esconder una ventana no borra su hueco de la pantalla: hay que
        // darle al escritorio el tiempo de repintar lo que habia debajo, o la captura se
        // lleva la ventanita dentro. Ese margen sale de la cuenta atras, no se suma
        // detras, asi que la espera total sigue siendo la que pidio el usuario y el ultimo
        // numero se ve 880 ms en vez de 1000: no se nota.
        let total = std::time::Duration::from_secs(u64::from(segundos));
        std::thread::sleep(total.saturating_sub(MARGEN_REPINTADO));

        // Volver al hilo principal no es opcional: manejar ventanas desde otro hilo se
        // queda esperando al bucle de eventos.
        let para_esconder = espera.clone();
        let _ = espera.run_on_main_thread(move || esconder_cuenta_atras(&para_esconder));

        std::thread::sleep(MARGEN_REPINTADO);
        let seguir = espera.clone();
        let _ = espera.run_on_main_thread(move || {
            if let Err(error) = congelar_y_abrir(&seguir, candado) {
                eprintln!("no se ha podido abrir el overlay: {error}");
            }
        });
    });
    Ok(())
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
///
/// El candado entra por parametro y se suelta al terminar: con el temporizador puesto se
/// cogio hace tres segundos, en otro hilo, y no se puede volver a pedir aqui.
fn congelar_y_abrir(app: &AppHandle, _candado: CandadoCaptura) -> Result<()> {
    let state = app.state::<AppState>();

    // Los iconos se esconden solo lo que dura el disparo, no toda la seleccion: el
    // overlay tapa el escritorio de todas formas, asi que tenerlos escondidos mas rato no
    // se ve en la imagen y si aumenta la posibilidad de dejarselos escondidos a alguien.
    // El guardian los devuelve tambien si `freeze_all` sale por el `?`.
    let esconder_iconos = state.settings.read().hide_desktop_icons;
    let iconos = if esconder_iconos {
        crate::platform::desktop_icons::esconder()
    } else {
        None
    };

    let freezes = capture::freeze_all(&state.freeze_dir())?;
    drop(iconos);
    let monitors: Vec<_> = freezes.iter().map(|f| f.monitor.clone()).collect();
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

    Ok(())
}

/// Ensenna los segundos que faltan, abajo a la derecha de la pantalla donde esta el raton.
///
/// La ventana se crea la primera vez que se usa y se queda escondida para las siguientes,
/// igual que los overlays. No se precrea al arrancar como ellos porque casi nadie usa el
/// temporizador, y las cifras de memoria en reposo estan publicadas: la primera cuenta
/// atras del dia paga el WebView2 y sale unas decimas tarde, las demas salen al instante.
fn mostrar_cuenta_atras(app: &AppHandle, segundos: u32) {
    let window = match app.get_webview_window(COUNTDOWN_LABEL) {
        Some(existente) => {
            // Trampa 8: el destino de un evento lo deciden los dos lados. Aqui solo hay
            // una ventana escuchando, pero se escribe dirigido igual que en el overlay
            // para que nadie copie de aqui la version que da problemas con varias.
            let _ = existente.emit_to(COUNTDOWN_LABEL, crate::EVENT_COUNTDOWN, segundos);
            existente
        }
        None => {
            // La primera vez el numero viaja en la URL: la pagina todavia no esta cargada
            // cuando se emitiria el evento, asi que no habria nadie escuchandolo.
            let url = WebviewUrl::App(format!("cuenta.html?segundos={segundos}").into());
            let construida = WebviewWindowBuilder::new(app, COUNTDOWN_LABEL, url)
                .title("winshotx")
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .shadow(true)
                .focused(false)
                .visible(false)
                .inner_size(CUENTA_LADO, CUENTA_LADO)
                .build();
            let Ok(nueva) = construida else {
                eprintln!("no se ha podido crear la cuenta atras");
                return;
            };
            crate::platform::window_style::rounded_corners(&nueva);
            // Sin esto la cuenta atras cierra el menu que se venia a fotografiar.
            crate::platform::window_style::never_focus(&nueva);
            nueva
        }
    };

    colocar_cuenta_atras(&window);
    let _ = window.show();
}

/// La pone en el centro del monitor donde esta el cursor, que es el que el usuario esta
/// mirando. Se recoloca en cada disparo porque el raton puede estar en otra pantalla que
/// la vez anterior, y porque cada pantalla puede tener su propio escalado.
///
/// En el centro y no en una esquina porque es donde se mira sin buscarla: con tres
/// pantallas, un numero en la esquina de una de ellas se pierde. Tapa lo que haya debajo
/// durante la cuenta, pero eso no llega a la captura: se esconde antes de congelar.
fn colocar_cuenta_atras(window: &tauri::WebviewWindow) {
    let Ok(monitores) = xcap::Monitor::all() else {
        return;
    };
    let cursor = cursor_position();
    let dentro = |m: &xcap::Monitor, x: i32, y: i32| -> bool {
        let (Ok(mx), Ok(my), Ok(mw), Ok(mh)) = (m.x(), m.y(), m.width(), m.height()) else {
            return false;
        };
        x >= mx && x < mx + mw as i32 && y >= my && y < my + mh as i32
    };
    let elegido = cursor
        .and_then(|(x, y)| monitores.iter().find(|m| dentro(m, x, y)))
        .or_else(|| monitores.iter().find(|m| m.is_primary().unwrap_or(false)))
        .or_else(|| monitores.first());

    let Some(monitor) = elegido else { return };
    let (Ok(x), Ok(y), Ok(width), Ok(height)) = (
        monitor.x(),
        monitor.y(),
        monitor.width(),
        monitor.height(),
    ) else {
        return;
    };

    // El escalado que cuenta es el de la pantalla de destino, no el de donde este la
    // ventana ahora mismo: en un equipo con una pantalla al 150 % y otra al 100 %,
    // preguntarselo a la ventana da el numero de la pantalla equivocada.
    let escala = f64::from(monitor.scale_factor().unwrap_or(1.0));
    let lado = (CUENTA_LADO * escala) as u32;

    let _ = window.set_size(PhysicalSize::new(lado, lado));
    let _ = window.set_position(PhysicalPosition::new(
        x + (width as i32 - lado as i32) / 2,
        y + (height as i32 - lado as i32) / 2,
    ));
}

/// La esconde y la deja en cero, que la pagina dibuja como el icono de la camara y no
/// como un numero: asi, si la proxima vez se ve un fotograma con el valor viejo, lo que
/// se ve es "ya voy" y no un tres que ya no significa nada.
fn esconder_cuenta_atras(app: &AppHandle) {
    let Some(window) = app.get_webview_window(COUNTDOWN_LABEL) else {
        return;
    };
    let _ = window.hide();
    let _ = window.emit_to(COUNTDOWN_LABEL, crate::EVENT_COUNTDOWN, 0u32);
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

    colocar_barra(&window, region);
    window.show()?;
    Ok(())
}

/// Pone la barra debajo de la region, y dentro de la pantalla donde esta esa region.
///
/// Antes se usaba `x.max(0)`, y eso mandaba la barra a la pantalla principal siempre que
/// se grababa en un monitor colocado a la izquierda o encima del principal: ahi las
/// coordenadas del escritorio son NEGATIVAS, y recortarlas a cero es literalmente saltar
/// de pantalla. Y el escalado se le preguntaba a la ventana, que acaba de nacer y todavia
/// esta en la pantalla equivocada.
fn colocar_barra(window: &tauri::WebviewWindow, region: Rect) {
    let centro = (
        region.x + region.width as i32 / 2,
        region.y + region.height as i32 / 2,
    );
    let (x, y) = sitio_de_la_barra(region, monitor_de(centro.0, centro.1));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// La cuenta de donde va la barra, aparte del resto para poder probarla sin pantallas.
///
/// Si no se sabe en que monitor cae, se coloca debajo de la region y se deja estar: es
/// preferible una barra un poco fuera de sitio que una barra en otra pantalla.
fn sitio_de_la_barra(region: Rect, monitor: Option<MonitorSitio>) -> (i32, i32) {
    let escala = monitor.map(|m| m.escala).unwrap_or(1.0);
    let ancho_px = (BARRA_ANCHO * escala) as i32;
    let alto_px = (BARRA_ALTO * escala) as i32;
    let hueco = (12.0 * escala) as i32;

    let mut x = region.x + (region.width as i32 - ancho_px) / 2;
    let mut y = region.y + region.height as i32 + hueco;

    if let Some(m) = monitor {
        let margen = (8.0 * escala) as i32;
        let izquierda = m.x + margen;
        let derecha = (m.x + m.ancho - ancho_px - margen).max(izquierda);
        x = x.clamp(izquierda, derecha);
        // Si la region llega al borde de abajo, la barra se sube encima en vez de quedarse
        // colgando fuera de la pantalla o saltando a la de al lado.
        let tope = m.y + m.alto - alto_px - margen;
        if y > tope {
            y = (region.y - alto_px - hueco).max(m.y + margen);
        }
    }
    (x, y)
}

/// Ancho y alto de la barra de grabacion, en pixeles logicos. Los mismos que pide el
/// constructor de la ventana: si se separan, la barra deja de quedar centrada.
const BARRA_ANCHO: f64 = 360.0;
const BARRA_ALTO: f64 = 52.0;

/// Los datos de un monitor que hacen falta para colocar una ventana encima.
#[derive(Debug, Clone, Copy)]
struct MonitorSitio {
    x: i32,
    y: i32,
    ancho: i32,
    alto: i32,
    escala: f64,
}

/// El monitor que contiene ese punto del escritorio, o el principal si el punto se ha
/// quedado en tierra de nadie (pasa entre monitores desalineados).
fn monitor_de(x: i32, y: i32) -> Option<MonitorSitio> {
    let monitores = xcap::Monitor::all().ok()?;
    let leer = |m: &xcap::Monitor| -> Option<MonitorSitio> {
        Some(MonitorSitio {
            x: m.x().ok()?,
            y: m.y().ok()?,
            ancho: m.width().ok()? as i32,
            alto: m.height().ok()? as i32,
            escala: f64::from(m.scale_factor().ok()?),
        })
    };
    let dentro = |s: &MonitorSitio| x >= s.x && x < s.x + s.ancho && y >= s.y && y < s.y + s.alto;

    monitores
        .iter()
        .filter_map(leer)
        .find(dentro)
        .or_else(|| monitores.iter().filter_map(leer).next())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, ancho: i32, alto: i32) -> MonitorSitio {
        MonitorSitio {
            x,
            y,
            ancho,
            alto,
            escala: 1.0,
        }
    }

    fn region(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// El fallo que Munir vio el 27 de agosto de 2026: grabo en el monitor vertical de la
    /// izquierda y la barra aparecio en otro. Ese monitor tiene coordenadas NEGATIVAS en
    /// el escritorio, y el codigo hacia `x.max(0)`, que es literalmente "salta a la
    /// pantalla principal".
    #[test]
    fn la_barra_se_queda_en_el_monitor_de_la_izquierda() {
        let izquierdo = monitor(-1080, 0, 1080, 1920);
        let (x, y) = sitio_de_la_barra(region(-900, 200, 600, 400), Some(izquierdo));
        assert!(
            x < 0,
            "la barra se ha ido a la pantalla principal: x = {x}, y el monitor acaba en 0"
        );
        assert!(x >= izquierdo.x, "y tampoco puede salirse por la izquierda");
        assert_eq!(y, 612, "debajo de la región, doce píxeles más abajo");
    }

    /// Debajo de la region es lo normal, y centrada con ella.
    #[test]
    fn la_barra_va_centrada_debajo_de_la_region() {
        let principal = monitor(0, 0, 1920, 1080);
        let (x, y) = sitio_de_la_barra(region(500, 100, 400, 300), Some(principal));
        assert_eq!(x, 500 + (400 - 360) / 2, "centrada con la región");
        assert_eq!(y, 100 + 300 + 12);
    }

    /// Y si la region llega abajo del todo, la barra se sube encima en vez de salirse.
    #[test]
    fn si_no_cabe_debajo_la_barra_se_pone_encima() {
        let principal = monitor(0, 0, 1920, 1080);
        let (_, y) = sitio_de_la_barra(region(300, 700, 800, 370), Some(principal));
        assert!(y < 700, "tenía que subirse encima de la región y está en {y}");
        assert!(y >= 8, "y sin salirse por arriba");
    }

    /// Una region ancha en un monitor estrecho: la barra se pega al borde, pero dentro.
    #[test]
    fn la_barra_nunca_se_sale_por_los_lados() {
        let estrecho = monitor(1920, 0, 400, 800);
        let (x, _) = sitio_de_la_barra(region(1930, 10, 380, 200), Some(estrecho));
        assert!(x >= 1928, "se sale por la izquierda: {x}");
        assert!(x + 360 <= 1920 + 400 - 8, "se sale por la derecha: {x}");
    }
}
