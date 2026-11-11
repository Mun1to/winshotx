//! La miniatura que sale en la esquina después de capturar una foto (E2 del plan de
//! CleanShot X).
//!
//! Es la pieza que lleva la captura a donde va: **se arrastra a cualquier app** (WhatsApp,
//! Discord, un correo, una carpeta) como si fuera un archivo del Explorador, y desde ella se
//! copia, se guarda, se edita o se ancla. Si se hacen varias seguidas se apilan, y cada una
//! se va sola al rato que diga «Ajustes».
//!
//! **Una sola ventana para toda la pila**, no una por miniatura: cada ventana es un
//! navegador entero (WebView2) que tarda medio segundo en nacer y gasta memoria, y con tres
//! capturas seguidas serían tres. Esta se crea con la primera captura y después solo se
//! esconde y se vuelve a enseñar (las etiquetas no se pueden reutilizar al cerrar, trampa 3).
//!
//! **No coge el foco jamás** (`never_focus`). Se captura algo para llevarlo a la app que está
//! delante, y si la miniatura le robara el foco, el Ctrl+V que viene detrás iría a parar a
//! ella. A cambio no recibe teclado, que no necesita: todo se hace con el ratón.
//!
//! **Lo que se arrastra es un archivo de verdad.** Si la captura se guardó, el de la carpeta
//! de capturas; si solo se copió, una copia en el temporal con el mismo nombre que tendría
//! guardada (`winshotx-AAAAMMDD-HHMMSS.png`), que es lo que verá quien la reciba en el chat.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use image::RgbaImage;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl};

use crate::capture::Rect;
use crate::encode::png;
use crate::error::{AppError, Result};
use crate::state::AppState;

/// La etiqueta de la ventana. Va también en `capabilities/default.json`, o la ventana
/// nace muda (trampa 10).
pub const ETIQUETA: &str = "miniaturas";
/// El aviso a la ventana de que la pila ha cambiado.
pub const EVENTO: &str = "miniaturas-cambian";
/// Cuántas se apilan como mucho. La quinta empuja fuera a la más vieja: una columna de
/// miniaturas que sube media pantalla ya no es una esquina.
const MAXIMO: usize = 4;

/// Las medidas de la ventana, en píxeles lógicos. Son las mismas que usa
/// `src/components/miniatura/Miniaturas.tsx`: si cambian aquí, cambian allí.
pub const ANCHO_CARTA: f64 = 240.0;
pub const ALTO_CARTA: f64 = 150.0;
pub const HUECO: f64 = 8.0;
pub const BORDE: f64 = 8.0;
/// Lo que se separa de la esquina de la pantalla, por encima de la barra de tareas.
const MARGEN_ESQUINA: f64 = 16.0;

struct Miniatura {
    id: u64,
    /// El archivo que se arrastra, se copia y se ancla: el guardado o la copia del temporal.
    archivo: PathBuf,
    /// La imagen pequeña que se enseña, en el temporal (el protocolo `asset:` no llega a la
    /// carpeta de capturas del usuario, y una captura 4K en una carta de 240 píxeles es
    /// gastar memoria para nada).
    vista: PathBuf,
    /// Si el archivo ya está en la carpeta de capturas.
    guardada: bool,
    /// Dónde estaba en la pantalla, para anclarla encima de su sitio.
    region: Rect,
}

/// Lo que ve la ventana de cada miniatura. Sin rutas que se puedan pedir de vuelta: las
/// órdenes reciben el número, y la ruta la pone Rust.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniaturaVista {
    pub id: u64,
    pub vista: String,
    pub guardada: bool,
}

/// La pila entera, y cuánto dura cada una. Va junta para que la ventana no tenga que
/// pedir los ajustes por su cuenta.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pila {
    pub lista: Vec<MiniaturaVista>,
    /// Segundos que se queda cada una. Cero es hasta que se cierre.
    pub segundos: u32,
}

static PILA: Mutex<Vec<Miniatura>> = Mutex::new(Vec::new());
static SIGUIENTE: AtomicU64 = AtomicU64::new(1);

/// Después de copiar o guardar una foto: prepara su miniatura y la enseña.
///
/// Todo va en un hilo aparte. Escribir el PNG de una captura grande son decenas de
/// milisegundos, y el overlay ya se ha cerrado: quien capturó está pegando, no esperando.
pub fn despues_de_capturar(app: &AppHandle, imagen: &RgbaImage, region: Rect, guardada: Option<PathBuf>) {
    let estado = app.state::<AppState>();
    let (activa, segundos) = {
        let ajustes = estado.settings.read();
        (ajustes.miniatura, ajustes.miniatura_segundos)
    };
    if !activa {
        return;
    }
    let app = app.clone();
    let imagen = imagen.clone();
    std::thread::spawn(move || {
        if let Err(e) = preparar(&app, &imagen, region, guardada) {
            eprintln!("[winshotx] la miniatura no ha salido: {e}");
            return;
        }
        ensennar(&app, segundos);
    });
}

fn carpeta(app: &AppHandle) -> PathBuf {
    app.state::<AppState>().temp_root.join("miniaturas")
}

fn preparar(app: &AppHandle, imagen: &RgbaImage, region: Rect, guardada: Option<PathBuf>) -> Result<()> {
    let dir = carpeta(app);
    std::fs::create_dir_all(&dir)?;
    limpiar_viejas(&dir);
    let id = SIGUIENTE.fetch_add(1, Ordering::Relaxed);
    let (archivo, esta_guardada) = match guardada {
        Some(ruta) => (ruta, true),
        None => {
            let ruta = crate::archivos::destino(&dir, "png")?;
            png::save(imagen, &ruta, imagen.width(), imagen.height())?;
            (ruta, false)
        }
    };
    let vista = dir.join(format!("vista-{id}.png"));
    let (w, h) = medida_de_la_vista(imagen.width(), imagen.height());
    let pequena = image::imageops::thumbnail(imagen, w, h);
    png::save_fast(&pequena, &vista)?;

    let mut pila = PILA.lock();
    pila.push(Miniatura {
        id,
        archivo,
        vista,
        guardada: esta_guardada,
        region,
    });
    while pila.len() > MAXIMO {
        let fuera = pila.remove(0);
        let _ = std::fs::remove_file(&fuera.vista);
    }
    Ok(())
}

/// Lo que mide la imagen de la carta: a doble resolución de la carta, para que se vea
/// nítida en una pantalla al 200 %, y sin agrandar nunca una captura pequeña.
fn medida_de_la_vista(ancho: u32, alto: u32) -> (u32, u32) {
    let (tope_w, tope_h) = ((ANCHO_CARTA * 2.0) as f32, (ALTO_CARTA * 2.0) as f32);
    let escala = (tope_w / ancho.max(1) as f32).min(tope_h / alto.max(1) as f32).min(1.0);
    (
        ((ancho as f32 * escala).round() as u32).max(1),
        ((alto as f32 * escala).round() as u32).max(1),
    )
}

/// Borra lo que lleve más de un día en la carpeta del temporal.
///
/// No se borra al cerrar cada miniatura a propósito: la app donde se soltó puede leer el
/// archivo un rato después (un correo que adjunta al enviar), y borrarlo al momento le
/// dejaría un adjunto roto. Un día es tiempo de sobra.
fn limpiar_viejas(dir: &std::path::Path) {
    let Ok(entradas) = std::fs::read_dir(dir) else { return };
    let ahora = std::time::SystemTime::now();
    for entrada in entradas.flatten() {
        let vieja = entrada
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|cuando| ahora.duration_since(cuando).ok())
            .is_some_and(|edad| edad.as_secs() > 24 * 3600);
        if vieja {
            let _ = std::fs::remove_file(entrada.path());
        }
    }
}

fn vista_de_la_pila(segundos: u32) -> Pila {
    Pila {
        lista: PILA
            .lock()
            .iter()
            .map(|m| MiniaturaVista {
                id: m.id,
                vista: m.vista.to_string_lossy().to_string(),
                guardada: m.guardada,
            })
            .collect(),
        segundos,
    }
}

fn segundos(app: &AppHandle) -> u32 {
    app.state::<AppState>().settings.read().miniatura_segundos
}

/// Lo que mide la ventana con `cuantas` miniaturas, en píxeles lógicos.
pub fn medida(cuantas: usize) -> (f64, f64) {
    let n = cuantas.max(1) as f64;
    (
        ANCHO_CARTA + BORDE * 2.0,
        ALTO_CARTA * n + HUECO * (n - 1.0) + BORDE * 2.0,
    )
}

/// Dónde va la ventana: abajo a la derecha del área de trabajo (sin la barra de tareas),
/// en píxeles físicos. Aparte para probarlo sin pantallas.
pub fn sitio(trabajo: (i32, i32, i32, i32), ancho: u32, alto: u32, escala: f64) -> (i32, i32) {
    let (izq, arr, der, aba) = trabajo;
    let margen = (MARGEN_ESQUINA * escala).round() as i32;
    // Si la pila es más alta que la pantalla, se ve la de abajo (la última) y no la de arriba.
    let x = (der - margen - ancho as i32).max(izq);
    let y = (aba - margen - alto as i32).max(arr);
    (x, y)
}

/// El área de trabajo y el escalado de la pantalla del ratón: es donde se acaba de capturar
/// y donde se está mirando.
#[cfg(windows)]
fn pantalla_del_raton() -> Option<((i32, i32, i32, i32), f64)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    unsafe {
        let mut punto = POINT::default();
        GetCursorPos(&mut punto).ok()?;
        let monitor = MonitorFromPoint(punto, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        let r = info.rcWork;
        Some(((r.left, r.top, r.right, r.bottom), dpi_x as f64 / 96.0))
    }
}

#[cfg(not(windows))]
fn pantalla_del_raton() -> Option<((i32, i32, i32, i32), f64)> {
    None
}

/// Crea la ventana la primera vez, y las siguientes la devuelve tal cual.
fn ventana(app: &AppHandle) -> Result<tauri::WebviewWindow> {
    if let Some(existente) = app.get_webview_window(ETIQUETA) {
        return Ok(existente);
    }
    let window = tauri::WebviewWindowBuilder::new(app, ETIQUETA, WebviewUrl::App("miniatura.html".into()))
        // Los mismos argumentos del navegador que todas: con otros, WebView2 levanta un
        // navegador aparte para esta ventana (ver `NAVEGADOR_ARGS`).
        .additional_browser_args(crate::windows_mgr::NAVEGADOR_ARGS)
        .title("winshotx")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(true)
        .focused(false)
        .visible(false)
        .build()?;
    crate::platform::window_style::rounded_corners(&window);
    crate::platform::window_style::never_focus(&window);
    Ok(window)
}

/// Coloca la ventana para lo que haya en la pila, avisa a la página y la enseña. Con la
/// pila vacía, la esconde.
fn ensennar(app: &AppHandle, segundos: u32) {
    let cuantas = PILA.lock().len();
    let Ok(window) = ventana(app) else { return };
    let _ = app.emit_to(ETIQUETA, EVENTO, vista_de_la_pila(segundos));
    if cuantas == 0 {
        // Se enseña por fuera de Tauri (`colocar_y_ensennar`), así que el `hide()` de Tauri
        // sola la cree ya escondida y no hace nada: el recuadro vacío se quedaba en pantalla.
        crate::platform::window_style::esconder(&window);
        return;
    }
    let (trabajo, escala) = pantalla_del_raton().unwrap_or(((0, 0, 1920, 1040), 1.0));
    let (ancho, alto) = medida(cuantas);
    let (ancho, alto) = ((ancho * escala).round() as u32, (alto * escala).round() as u32);
    let _ = window.set_size(PhysicalSize::new(ancho, alto));
    let (x, y) = sitio(trabajo, ancho, alto, escala);
    // Sin activarla: `show()` le daría el foco a la ventana, y el foco es de la app donde
    // se va a pegar.
    if !crate::platform::window_style::colocar_y_ensennar(&window, x, y) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
        let _ = window.show();
    }
}

fn quitar(app: &AppHandle, id: u64) {
    PILA.lock().retain(|m| m.id != id);
    ensennar(app, segundos(app));
}

/// El archivo y el sitio de una miniatura, o un error si ya se cerró.
fn de(id: u64) -> Result<(PathBuf, bool, Rect)> {
    PILA.lock()
        .iter()
        .find(|m| m.id == id)
        .map(|m| (m.archivo.clone(), m.guardada, m.region))
        .ok_or_else(|| AppError::Msg("esa miniatura ya se ha cerrado".into()))
}

// --- Las órdenes de la ventana ---

#[tauri::command]
pub async fn miniaturas_lista(app: AppHandle) -> Pila {
    vista_de_la_pila(segundos(&app))
}

#[tauri::command]
pub async fn miniatura_cerrar(app: AppHandle, id: u64) {
    quitar(&app, id);
}

#[tauri::command]
pub async fn miniatura_copiar(id: u64) -> Result<()> {
    let (archivo, _, _) = de(id)?;
    let imagen = image::open(&archivo)?.to_rgba8();
    let bytes = std::fs::read(&archivo)?;
    crate::platform::clipboard::copy_image(&imagen, &bytes)
}

/// La guarda en la carpeta de capturas. Si ya estaba guardada no hace otra copia: la
/// enseña en el Explorador, que es lo que se busca al pulsar «Guardar» sobre algo guardado.
#[tauri::command]
pub async fn miniatura_guardar(app: AppHandle, id: u64) -> Result<String> {
    let (archivo, guardada, _) = de(id)?;
    if guardada {
        crate::platform::reveal(&archivo)?;
        return Ok(archivo.to_string_lossy().to_string());
    }
    let carpeta = PathBuf::from(app.state::<AppState>().settings.read().save_directory.clone());
    let destino = crate::archivos::destino(&carpeta, "png")?;
    std::fs::copy(&archivo, &destino)?;
    if let Some(m) = PILA.lock().iter_mut().find(|m| m.id == id) {
        m.archivo = destino.clone();
        m.guardada = true;
    }
    let _ = app.emit_to(ETIQUETA, EVENTO, vista_de_la_pila(segundos(&app)));
    Ok(destino.to_string_lossy().to_string())
}

/// La abre en el editor y se quita de la esquina: ya está delante, en grande.
#[tauri::command]
pub async fn miniatura_editar(app: AppHandle, id: u64) -> Result<()> {
    let (archivo, _, region) = de(id)?;
    let imagen = image::open(&archivo)?.to_rgba8();
    let sesion = crate::recorder::session_from_image(&app, &imagen, region_de(&imagen, region))?;
    crate::windows_mgr::open_editor(&app, &sesion.id)?;
    quitar(&app, id);
    Ok(())
}

/// La deja flotando encima de todo, en el sitio donde se capturó.
#[tauri::command]
pub async fn miniatura_anclar(app: AppHandle, id: u64) -> Result<()> {
    let (archivo, _, region) = de(id)?;
    // Las ancladas viven en su carpeta del temporal, que es la única de la que sus órdenes
    // aceptan rutas (`ruta_de_anclada`): se copia allí.
    let pins = app.state::<AppState>().temp_root.join("pins");
    std::fs::create_dir_all(&pins)?;
    let anclada = pins.join(format!("pin-{}.png", chrono::Local::now().format("%Y%m%d-%H%M%S%3f")));
    std::fs::copy(&archivo, &anclada)?;
    let (ancho, alto) = image::image_dimensions(&anclada)?;
    let sitio = centrada(region, ancho, alto);
    crate::windows_mgr::open_pin(&app, sitio, &anclada)?;
    quitar(&app, id);
    Ok(())
}

/// Arrastra el archivo de la miniatura a donde se suelte. Se llama con el botón ya pulsado.
///
/// El arrastre de OLE tiene que salir del hilo de la ventana (el principal), y no vuelve
/// hasta que se suelta el ratón: por eso se manda allí y esta orden contesta en el acto.
/// Si alguien lo recoge, la miniatura se va, que ya ha llegado a donde iba.
#[tauri::command]
pub async fn miniatura_arrastrar(app: AppHandle, id: u64) -> Result<()> {
    let (archivo, _, _) = de(id)?;
    let window = ventana(&app)?;
    let hwnd = window.hwnd().map_err(|e| AppError::Msg(e.to_string()))?.0 as isize;
    let otra = app.clone();
    app.run_on_main_thread(move || {
        match crate::platform::arrastrar::arrastrar(hwnd, &archivo) {
            Ok(true) => quitar(&otra, id),
            Ok(false) => {}
            Err(e) => eprintln!("[winshotx] {e}"),
        }
    })
    .map_err(|e| AppError::Msg(e.to_string()))?;
    Ok(())
}

/// El sitio de la captura si la imagen ha crecido (con el fondo de E1 lleva aire
/// alrededor): mismo centro, el tamaño de la imagen. Así la anclada cae encima de donde
/// estaba, con su marco por fuera, en vez de encogida dentro del recorte.
fn centrada(region: Rect, ancho: u32, alto: u32) -> Rect {
    let (cx, cy) = (region.x + region.width as i32 / 2, region.y + region.height as i32 / 2);
    Rect {
        x: cx - ancho as i32 / 2,
        y: cy - alto as i32 / 2,
        width: ancho,
        height: alto,
    }
}

fn region_de(imagen: &RgbaImage, region: Rect) -> Rect {
    centrada(region, imagen.width(), imagen.height())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La ventana mide lo que miden sus cartas: ni un hueco vacío debajo ni una cortada.
    #[test]
    fn la_ventana_crece_con_la_pila() {
        assert_eq!(medida(1), (256.0, 166.0));
        assert_eq!(medida(3), (256.0, 150.0 * 3.0 + 16.0 + 16.0));
        // Con la pila vacía mide lo de una, para no pedirle a Windows una ventana de cero.
        assert_eq!(medida(0), medida(1));
    }

    /// Abajo a la derecha del área de trabajo, a 16 píxeles lógicos de la esquina, también
    /// en una pantalla a la izquierda de la principal (coordenadas negativas).
    #[test]
    fn va_abajo_a_la_derecha_de_su_pantalla() {
        assert_eq!(sitio((0, 0, 1920, 1040), 256, 166, 1.0), (1920 - 16 - 256, 1040 - 16 - 166));
        assert_eq!(sitio((-2560, 0, 0, 1400), 384, 249, 1.5), (-24 - 384, 1400 - 24 - 249));
    }

    /// Una pila más alta que la pantalla se pega arriba, no se sale.
    #[test]
    fn una_pila_enorme_no_se_sale_por_arriba() {
        let (_, y) = sitio((0, 0, 800, 600), 256, 900, 1.0);
        assert_eq!(y, 0);
    }

    #[test]
    fn la_vista_es_pequena_y_no_agranda_nada() {
        assert_eq!(medida_de_la_vista(3840, 2160), (480, 270));
        assert_eq!(medida_de_la_vista(200, 100), (200, 100));
        assert_eq!(medida_de_la_vista(100, 2000), (15, 300));
    }

    /// La anclada de una captura con fondo cae con el mismo centro que el recorte.
    #[test]
    fn la_anclada_con_fondo_mantiene_su_centro() {
        let recorte = Rect { x: 100, y: 200, width: 400, height: 300 };
        let sitio = centrada(recorte, 464, 364);
        assert_eq!((sitio.x, sitio.y, sitio.width, sitio.height), (68, 168, 464, 364));
    }
}
