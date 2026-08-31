//! El menu de la bandeja, dibujado por winshotx en vez de por Windows.
//!
//! Un menu del sistema no sabe ensennar un interruptor, ni el atajo de cada cosa, ni decir
//! que version tiene puesta: son entradas de texto y poco mas. Esto es una ventana sin
//! marco, con las mismas piezas y los mismos colores que el resto de la aplicacion, y es
//! ademas lo que deja poner ahi el anillo de los ultimos segundos como lo que es, un
//! interruptor, y no como una entrada que enciende y otra que apaga.
//!
//! **Se prepara al arrancar, escondida** (ver `precalentar`). Se hizo al reves durante un
//! tiempo, creandola en el primer clic derecho para no tener otra webview encendida todo
//! el dia, y ese primer clic se notaba: hay que crear la ventana, cargar la interfaz,
//! pedir el estado y medirse antes de que se vea nada. Medido, tener el menu preparado
//! cuesta entre 7 y 11 MB, no las decenas que se temian.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

use crate::error::Result;
use crate::windows_mgr::{monitor_de, MonitorSitio};

pub const TRAY_MENU_LABEL: &str = "tray-menu";

/// Ancho fijo, en pixeles logicos. El alto lo dice la propia interfaz al medirse.
const MENU_ANCHO: f64 = 284.0;
/// Con lo que nace, hasta que la interfaz mide lo suyo.
const MENU_ALTO_INICIAL: f64 = 300.0;
/// El aire contra el icono de la bandeja y contra los bordes de la pantalla.
const MARGEN: f64 = 8.0;

/// Donde estaba el icono la ultima vez que se abrio, para volver a colocarlo cuando la
/// interfaz diga cuanto mide: el menu crece hacia ARRIBA, no hacia abajo.
static ULTIMO_ANCLAJE: Mutex<Option<(i32, i32)>> = Mutex::new(None);

/// Cuando se escondio por perder el foco.
///
/// Volver a pulsar el icono de la bandeja tiene que CERRAR el menu, y ahi pasan dos cosas
/// seguidas: el clic le da el foco a la barra de tareas (y el menu se esconde solo) y
/// despues llega el evento del clic, que ya lo ve escondido y lo volveria a abrir. Con la
/// hora del ultimo escondite se distingue «estaba cerrado» de «lo acabo de cerrar yo».
static ESCONDIDO_EN: Mutex<Option<Instant>> = Mutex::new(None);

/// Lo que tarda el clic en llegar despues de que el menu pierda el foco. Medido a ojo y
/// generoso: mas de un cuarto de segundo entre las dos cosas ya es otro clic distinto.
const RECIEN_ESCONDIDO: Duration = Duration::from_millis(300);

/// Si el menu esta ensennado fuera de las pantallas para que cargue (ver `precalentar`).
///
/// Mientras dure, la ventana esta «visible» para Windows pero no la ve nadie, asi que no
/// cuenta como menu abierto: sin esto, un clic derecho durante el arranque cerraria un
/// menu que el usuario nunca vio en vez de abrirlo.
static CALENTANDO: AtomicBool = AtomicBool::new(false);

/// Cuanto se deja pasar desde que arranca la aplicacion antes de preparar el menu.
///
/// Preparalo DENTRO del arranque costaba: crear la ventana son unos 270 ms y ademas su
/// interfaz se pone a cargar compitiendo con las tres ventanas del pool de overlays, que
/// se crean en ese mismo momento. Munir lo noto a la primera: «ahora tarda mas que antes
/// en cargar al principio». El menu no lo necesita nadie en los primeros segundos de vida
/// de la aplicacion, asi que se espera a que no haya nadie esperando.
const ESPERA_ANTES_DE_CALENTAR: Duration = Duration::from_secs(6);

/// Cuantas veces se vuelve a esperar si la aplicacion esta ocupada. Con la espera de
/// arriba son dos minutos largos: pasados esos, se prepara igual, porque si no la primera
/// vez que hiciera falta el menu volveria a costar lo de siempre.
const ESPERAS_MAXIMAS: u32 = 20;

/// Cuanto se espera a que la interfaz diga que ya se ha pintado antes de esconderla igual.
///
/// La sennal buena es la llamada a `redimensionar`. Este plazo es la red de seguridad para
/// que un fallo del frontend no deje una ventana suelta fuera de la pantalla.
const CALENTAMIENTO_MAXIMO: Duration = Duration::from_secs(10);

/// Donde se pone el menu, dado el punto del icono de la bandeja.
///
/// **Encima del icono**, que es donde cabe: la barra de tareas suele estar abajo, asi que
/// un menu que creciera hacia abajo saldria de la pantalla. Si no hay sitio arriba (barra
/// de tareas arriba del todo), se pone debajo.
///
/// Va aparte y sin tocar ninguna ventana para poder probarla con la barra de tareas en
/// cualquier sitio y con monitores de coordenadas negativas, que es donde esto se rompe.
fn sitio_del_menu(
    anclaje: (i32, i32),
    ancho: i32,
    alto: i32,
    monitor: Option<MonitorSitio>,
) -> (i32, i32) {
    let (ax, ay) = anclaje;
    let escala = monitor.map(|m| m.escala).unwrap_or(1.0);
    let margen = (MARGEN * escala) as i32;

    let mut x = ax - ancho / 2;
    let mut y = ay - alto - margen;

    if let Some(m) = monitor {
        // Nunca contra cero: un monitor a la izquierda del principal tiene la x negativa,
        // y recortar a cero manda la ventana a la pantalla equivocada. Ya mordio tres
        // veces en esta misma aplicacion.
        let izquierda = m.x + margen;
        let derecha = (m.x + m.ancho - ancho - margen).max(izquierda);
        x = x.clamp(izquierda, derecha);

        let arriba = m.y + margen;
        if y < arriba {
            y = ay + margen;
        }
        let abajo = (m.y + m.alto - alto - margen).max(arriba);
        y = y.clamp(arriba, abajo);
    }
    (x, y)
}

/// La ventana del menu, creandola si es la primera vez.
fn ventana(app: &AppHandle) -> Result<tauri::WebviewWindow> {
    if let Some(existente) = app.get_webview_window(TRAY_MENU_LABEL) {
        return Ok(existente);
    }

    let window = WebviewWindowBuilder::new(
        app,
        TRAY_MENU_LABEL,
        WebviewUrl::App("tray-menu.html".into()),
    )
    .title("winshotx")
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .shadow(true)
    .visible(false)
    .inner_size(MENU_ANCHO, MENU_ALTO_INICIAL)
    .build()?;

    // Las esquinas las redondea Windows, no un `border-radius` sobre una ventana
    // transparente: eso deja un halo negro alrededor. Trampa conocida de esta app.
    crate::platform::window_style::rounded_corners(&window);

    // Un menu se cierra al pulsar en otro sitio. El del sistema lo hacia solo; aqui se
    // hace al perder el foco, que es lo mismo visto desde dentro.
    let suyo = app.clone();
    window.on_window_event(move |evento| {
        if let tauri::WindowEvent::Focused(false) = evento {
            *ESCONDIDO_EN.lock() = Some(Instant::now());
            esconder(&suyo);
        }
    });
    Ok(window)
}

/// Deja el menu hecho y con su interfaz cargada antes de que nadie lo pida.
///
/// **Una ventana escondida no arranca su interfaz.** Se comprobo el 31 de agosto de 2026:
/// creada al arrancar y escondida, quince segundos despues seguia sin pedir su estado ni
/// pintarse. WebView2 no navega hasta que la ventana se ensenna, asi que crearla y dejarla
/// escondida solo ahorra los ~270 ms del cascarron y deja lo caro para el primer clic.
///
/// Por eso se ensenna una vez **fuera de todas las pantallas**, donde no la ve nadie, y se
/// esconde en cuanto la interfaz avisa de su alto (`redimensionar`), que es la sennal de
/// que ya esta pintada. `CALENTAMIENTO_MAXIMO` la esconde igual si esa sennal no llega.
/// Prepara el menu cuando el arranque ya ha terminado, sin robarle tiempo.
///
/// Vuelve al momento, asi que se puede llamar desde el arranque sin retrasarlo. Ver
/// `ESPERA_ANTES_DE_CALENTAR` y `precalentar`.
pub fn precalentar_cuando_no_estorbe(app: &AppHandle) {
    let suyo = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(ESPERA_ANTES_DE_CALENTAR);
        // Y si justo en ese momento se esta capturando o grabando, se espera. El menu no
        // corre ninguna prisa; lo que no puede es ponerse por delante de una captura, que
        // es exactamente lo que se noto al prepararlo dentro del arranque.
        for _ in 0..ESPERAS_MAXIMAS {
            if !hay_algo_en_marcha(&suyo) {
                break;
            }
            std::thread::sleep(ESPERA_ANTES_DE_CALENTAR);
        }
        let copia = suyo.clone();
        let _ = suyo.run_on_main_thread(move || precalentar(&copia));
    });
}

/// Si la aplicacion esta a lo suyo: capturando o grabando.
///
/// La captura se reconoce por el pool de overlays: si alguna de esas ventanas esta a la
/// vista, hay una seleccion en marcha en alguna pantalla.
fn hay_algo_en_marcha(app: &AppHandle) -> bool {
    if app.state::<crate::state::AppState>().is_recording() {
        return true;
    }
    app.webview_windows().iter().any(|(etiqueta, ventana)| {
        etiqueta.starts_with(crate::windows_mgr::OVERLAY_PREFIX)
            && ventana.is_visible().unwrap_or(false)
    })
}

fn precalentar(app: &AppHandle) {
    let window = match ventana(app) {
        Ok(w) => w,
        Err(error) => {
            eprintln!("[bandeja] no se ha podido preparar el menú: {error}");
            return;
        }
    };
    CALENTANDO.store(true, Ordering::SeqCst);
    let _ = window.set_position(aparcadero(app));
    let _ = window.show();

    let suyo = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(CALENTAMIENTO_MAXIMO);
        if CALENTANDO.swap(false, Ordering::SeqCst) {
            eprintln!("[bandeja] el menú no ha avisado de que estaba listo; lo escondo igual");
            let copia = suyo.clone();
            let _ = suyo.run_on_main_thread(move || esconder(&copia));
        }
    });
}

/// Un punto fuera de todas las pantallas, para calentar el menu sin que se vea.
///
/// No vale un numero grande y negativo a ojo: con un monitor a la izquierda del principal,
/// las coordenadas negativas son pantalla de verdad. Se busca la esquina de mas arriba y
/// mas a la izquierda de todo el escritorio y se sale de ahi por el alto del menu.
fn aparcadero(app: &AppHandle) -> PhysicalPosition<i32> {
    let mut x = 0;
    let mut y = 0;
    if let Ok(monitores) = app.available_monitors() {
        for m in monitores {
            x = x.min(m.position().x);
            y = y.min(m.position().y);
        }
    }
    let alto = (MENU_ALTO_INICIAL * 2.0) as i32;
    PhysicalPosition::new(x - alto, y - alto)
}

pub fn esconder(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(TRAY_MENU_LABEL) {
        let _ = window.hide();
    }
}

fn esta_a_la_vista(app: &AppHandle) -> bool {
    if CALENTANDO.load(Ordering::SeqCst) {
        return false;
    }
    app.get_webview_window(TRAY_MENU_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// Lo coloca sobre el icono de la bandeja y lo ensenna.
fn mostrar(app: &AppHandle, anclaje: (i32, i32)) -> Result<()> {
    // Si se estaba calentando, se acabo: a partir de aqui la ventana es un menu de verdad
    // y su proximo `redimensionar` tiene que colocarla, no esconderla.
    CALENTANDO.store(false, Ordering::SeqCst);
    let window = ventana(app)?;
    let escala = window.scale_factor().unwrap_or(1.0);
    let medida = window
        .outer_size()
        .unwrap_or(PhysicalSize::new(
            (MENU_ANCHO * escala) as u32,
            (MENU_ALTO_INICIAL * escala) as u32,
        ));

    *ULTIMO_ANCLAJE.lock() = Some(anclaje);
    let (x, y) = sitio_del_menu(
        anclaje,
        medida.width as i32,
        medida.height as i32,
        monitor_de(anclaje.0, anclaje.1),
    );
    let _ = window.set_position(PhysicalPosition::new(x, y));
    window.show()?;
    window.set_focus()?;
    // La ventana se reutiliza, asi que hay que decirle que vuelva a leer el estado: el
    // anillo puede haberse encendido desde los ajustes desde la ultima vez.
    let _ = window.emit_to(TRAY_MENU_LABEL, crate::EVENT_TRAY_MENU_OPENED, ());
    Ok(())
}

/// Abre el menu, o lo cierra si ya estaba abierto.
pub fn alternar(app: &AppHandle, anclaje: (i32, i32)) {
    if esta_a_la_vista(app) {
        esconder(app);
        return;
    }
    // Y si acaba de esconderse por el foco, este clic es el que lo estaba cerrando.
    if let Some(cuando) = *ESCONDIDO_EN.lock() {
        if cuando.elapsed() < RECIEN_ESCONDIDO {
            return;
        }
    }
    if let Err(error) = mostrar(app, anclaje) {
        eprintln!("[bandeja] no se ha podido abrir el menú: {error}");
        // Sin menu no habria ni forma de salir de la aplicacion: los ajustes tienen su
        // boton de Salir, asi que ese es el plan B.
        let _ = crate::windows_mgr::show_settings(app);
    }
}

/// Le da a la ventana el alto que ha medido su contenido.
///
/// Se vuelve a colocar despues de crecer: el menu esta anclado al icono de la bandeja por
/// su borde de ABAJO, asi que si solo cambiara el alto, crecería hacia abajo y se metería
/// debajo de la barra de tareas.
pub fn redimensionar(app: &AppHandle, alto: f64) {
    let Some(window) = app.get_webview_window(TRAY_MENU_LABEL) else {
        return;
    };
    // Esta es la sennal de que la interfaz ya esta montada y pintada. Si aun se estaba
    // calentando, el trabajo esta hecho: se esconde y se queda lista para el primer clic.
    if CALENTANDO.swap(false, Ordering::SeqCst) {
        let _ = window.hide();
        return;
    }
    let escala = window.scale_factor().unwrap_or(1.0);
    let ancho = (MENU_ANCHO * escala) as u32;
    let alto_px = (alto * escala).round().max(80.0) as u32;
    let _ = window.set_size(PhysicalSize::new(ancho, alto_px));

    if let Some(anclaje) = *ULTIMO_ANCLAJE.lock() {
        let (x, y) = sitio_del_menu(
            anclaje,
            ancho as i32,
            alto_px as i32,
            monitor_de(anclaje.0, anclaje.1),
        );
        let _ = window.set_position(PhysicalPosition::new(x, y));
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

    /// La barra de tareas suele estar abajo: el menu tiene que crecer hacia arriba.
    #[test]
    fn el_menu_sale_encima_del_icono() {
        let (x, y) = sitio_del_menu((1700, 1050), 268, 300, Some(monitor(0, 0, 1920, 1080)));
        assert_eq!(x, 1700 - 134, "no ha quedado centrado sobre el icono");
        assert_eq!(y, 1050 - 300 - 8);
    }

    /// Y con la barra de tareas arriba del todo, debajo, que es donde cabe.
    #[test]
    fn con_la_barra_arriba_sale_debajo() {
        let (_, y) = sitio_del_menu((960, 20), 268, 300, Some(monitor(0, 0, 1920, 1080)));
        assert_eq!(y, 28, "se ha salido por arriba de la pantalla");
    }

    /// El icono esta en la esquina derecha: el menu no puede salirse por ahi.
    #[test]
    fn no_se_sale_por_la_derecha() {
        let (x, _) = sitio_del_menu((1915, 1050), 268, 300, Some(monitor(0, 0, 1920, 1080)));
        assert_eq!(x, 1920 - 268 - 8);
    }

    /// Y en un monitor a la izquierda del principal, con coordenadas NEGATIVAS, se queda
    /// en el suyo. Recortar contra cero lo mandaria a la pantalla principal.
    #[test]
    fn se_queda_en_el_monitor_de_coordenadas_negativas() {
        let (x, y) = sitio_del_menu(
            (-1800, 1050),
            268,
            300,
            Some(monitor(-1920, 0, 1920, 1080)),
        );
        assert!(x >= -1912 && x < 0, "se ha ido a la pantalla principal: {x}");
        assert_eq!(y, 742);
    }
}
