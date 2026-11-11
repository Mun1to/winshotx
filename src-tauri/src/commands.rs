use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::capture::{self, MonitorInfo, Rect, WindowRect};
use crate::encode::{ffmpeg, png};
use crate::error::{AppError, Result};
use crate::exporter::{self, ExportRequest, ExportResult};
use crate::record;
use crate::recorder::{self, RecordOptions, SessionInfo};
use crate::settings::Settings;
use crate::state::AppState;
use crate::windows_mgr::{self, OverlayIntent};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayPayload {
    monitor: MonitorInfo,
    /// De que disparo del atajo es este payload. El overlay lo devuelve en `overlay_listo`
    /// cuando tiene la imagen pintada, y asi un aviso que llegue tarde (de una captura que
    /// ya se cancelo) no ensenna una ventana que nadie ha pedido.
    generation: u64,
    windows: Vec<WindowRect>,
    settings: Settings,
    /// Si se abrio para capturar o para grabar. El overlay es el mismo.
    intent: OverlayIntent,
    /// Que numero de pantalla es esta, empezando por 1, y cuantas hay. Con varias
    /// pantallas hay que poder decir "esta", y para eso hay que poder nombrarlas.
    screen_number: usize,
    screen_count: usize,
    /// La ultima region capturada, en coordenadas del escritorio virtual, o nada si
    /// todavia no se ha capturado ninguna desde que se abrio la app.
    last_region: Option<Rect>,
}

/// Construye el payload de una pantalla. La usan tanto el comando `overlay_bootstrap`
/// (primer montaje de la ventana) como `windows_mgr::open_overlays` (para mandarlo ya
/// hecho en el evento cuando se reutiliza una ventana, y ahorrarse la vuelta de invoke).
pub fn build_overlay_payload(state: &AppState, monitor_id: u32) -> Result<OverlayPayload> {
    let freezes = state.freezes.read();
    let freeze = freezes
        .iter()
        .find(|f| f.monitor.id == monitor_id)
        .ok_or_else(|| AppError::Msg(format!("el monitor {monitor_id} no tiene captura congelada")))?;

    Ok(OverlayPayload {
        monitor: freeze.monitor.clone(),
        generation: windows_mgr::generacion_actual(),
        windows: capture::window_rects(),
        settings: state.settings.read().clone(),
        intent: *state.intent.read(),
        // El id es el indice del monitor en la enumeracion del sistema, asi que el numero
        // que se pinta en cada pantalla es siempre el mismo entre disparo y disparo, aunque
        // las capturas lleguen en dos tandas (primero la del raton) y `freezes` no este
        // completo todavia.
        screen_number: freeze.monitor.id as usize + 1,
        screen_count: capture::monitors().map(|m| m.len()).unwrap_or(freezes.len()).max(1),
        last_region: *state.last_region.read(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameDto {
    index: u32,
    timestamp_ms: u64,
    duration_ms: u32,
    thumb_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StillResult {
    path: Option<String>,
    copied: bool,
    width: u32,
    height: u32,
}

#[tauri::command]
pub async fn overlay_bootstrap(state: State<'_, AppState>, monitor_id: u32) -> Result<OverlayPayload> {
    build_overlay_payload(&state, monitor_id)
}

/// La pantalla congelada de un monitor, en PNG, para el overlay.
///
/// Es lo que mas pesaba de todo el camino del atajo: llevar los 8 MB en crudo a cada
/// ventana tardaba 300-400 ms con tres pantallas (medido el 5 de septiembre de 2026), y en
/// PNG rapido son dos o tres megabytes que el navegador decodifica nativo. Se comprime al
/// congelar, una vez, en `capture::freeze_all`.
#[tauri::command]
pub async fn freeze_png(
    state: State<'_, AppState>,
    monitor_id: u32,
) -> Result<tauri::ipc::Response> {
    let png = {
        let freezes = state.freezes.read();
        freezes
            .iter()
            .find(|f| f.monitor.id == monitor_id)
            .map(|f| f.png.clone())
            .ok_or_else(|| AppError::Msg(format!("el monitor {monitor_id} no tiene captura congelada")))?
    };
    Ok(tauri::ipc::Response::new((*png).clone()))
}

/// El overlay de ese monitor ya tiene su imagen pintada: que se ensenne.
///
/// Las ventanas se quedan aparcadas fuera de las pantallas hasta este aviso, asi que nadie
/// ve nunca la pantalla de «preparando la captura»: lo que aparece, aparece ya pintado.
#[tauri::command]
pub async fn overlay_listo(app: AppHandle, monitor_id: u32, generation: u64) {
    windows_mgr::overlay_listo(&app, monitor_id, generation);
}

/// Respaldo del overlay: la pantalla congelada sin comprimir, como BMP, si el PNG fallara.
/// El overlay ocupa la pantalla entera y es opaco: sin una segunda via se quedaria en negro
/// tapando el escritorio.
#[tauri::command]
pub async fn freeze_bytes(
    state: State<'_, AppState>,
    monitor_id: u32,
) -> Result<tauri::ipc::Response> {
    let image = {
        let freezes = state.freezes.read();
        freezes
            .iter()
            .find(|f| f.monitor.id == monitor_id)
            .map(|f| f.image.clone())
            .ok_or_else(|| AppError::Msg(format!("el monitor {monitor_id} no tiene captura congelada")))?
    };
    Ok(tauri::ipc::Response::new(capture::bmp_bytes(&image)))
}

#[tauri::command]
pub async fn capture_still(app: AppHandle, region: Rect, action: String) -> Result<StillResult> {
    let image = {
        let state = app.state::<AppState>();
        let freezes = state.freezes.read();
        capture::crop_from_freeze(&freezes, region)?
    };
    // Se recuerda solo si el recorte ha salido bien: repetir una region que fallo no
    // repetiria nada. Y solo aqui, no en `capture_all_screens`, porque "la ultima region"
    // que tiene sentido repetir es un recorte, no el escritorio entero.
    *app.state::<AppState>().last_region.write() = Some(region);
    entregar(&app, image, region, &action)
}

/// Se lleva las pantallas de golpe, todas en una imagen, cada una en su sitio real.
///
/// No pasa por `capture_still` porque ahi la region decide **de que pantalla** se recorta,
/// y aqui no hay una pantalla: hay todas. La region que sale es el escritorio virtual
/// entero, y es la que se guarda en la sesion si se abre el editor.
#[tauri::command]
pub async fn capture_all_screens(app: AppHandle, action: String) -> Result<StillResult> {
    let (image, region) = {
        let state = app.state::<AppState>();
        let freezes = state.freezes.read();
        capture::stitch_all(&freezes)?
    };
    entregar(&app, image, region, &action)
}

/// Que se hace con una imagen ya recortada: copiarla, guardarla o abrirla en el editor.
/// Lo comparten la captura de una region y la de todas las pantallas, que solo se
/// diferencian en como consiguen la imagen.
fn entregar(
    app: &AppHandle,
    image: image::RgbaImage,
    region: Rect,
    action: &str,
) -> Result<StillResult> {
    let state = app.state::<AppState>();
    let (width, height) = (image.width(), image.height());
    let (copy_after, con_sonido) = {
        let ajustes = state.settings.read();
        (ajustes.copy_after_capture, ajustes.play_sound)
    };
    // El clic va aqui y no en cada rama: suena por haber capturado, se haga lo que se haga
    // despues con la imagen. Y va antes de escribir el archivo para que no lo retrase.
    if con_sonido {
        crate::platform::sonido::obturador();
    }

    let mut result = StillResult {
        path: None,
        copied: false,
        width,
        height,
    };

    match action {
        "copy" => {
            let bytes = png::to_bytes(&image)?;
            crate::platform::clipboard::copy_image(&image, &bytes)?;
            result.copied = true;
        }
        "save" => {
            let directory = PathBuf::from(state.settings.read().save_directory.clone());
            // El nombre lo pone `archivos`, que ademas comprueba que no exista ya: dos
            // capturas guardadas dentro del mismo segundo dejaban una sola.
            let path = crate::archivos::destino(&directory, "png")?;
            png::save(&image, &path, width, height)?;
            if copy_after {
                let bytes = png::to_bytes(&image)?;
                result.copied = crate::platform::clipboard::copy_image(&image, &bytes).is_ok();
            }
            result.path = Some(path.to_string_lossy().to_string());
        }
        // Guardar donde diga el usuario, con el dialogo de Windows: para la captura que va
        // a un proyecto concreto y no a la carpeta de siempre. Los overlays tapan la
        // pantalla entera y van encima de todo, asi que el dialogo saldria detras: se
        // cierran antes, que la imagen ya esta recortada. Si cancela, no pasa nada.
        "save_as" => {
            let directory = PathBuf::from(state.settings.read().save_directory.clone());
            windows_mgr::close_overlays(app);
            let Some(path) = elegir_donde_guardar(app, &directory, "png") else {
                return Ok(result);
            };
            png::save(&image, &path, width, height)?;
            if copy_after {
                let bytes = png::to_bytes(&image)?;
                result.copied = crate::platform::clipboard::copy_image(&image, &bytes).is_ok();
            }
            result.path = Some(path.to_string_lossy().to_string());
        }
        "edit" => {
            let session = recorder::session_from_image(app, &image, region)?;
            windows_mgr::close_overlays(app);
            windows_mgr::open_editor(app, &session.id)?;
            return Ok(result);
        }
        // Copiar el TEXTO en vez de la imagen. Se lee con el motor que trae Windows, asi
        // que no engorda el instalador ni sale de la maquina.
        "text" => {
            let bytes = png::to_bytes(&image)?;
            let texto = crate::platform::ocr::leer_texto(&bytes)?;
            if texto.is_empty() {
                return Err(AppError::Msg(
                    "No he encontrado texto en esa captura.".into(),
                ));
            }
            crate::platform::clipboard::copy_text(&texto)?;
            result.copied = true;
        }
        // Dejar la captura flotando encima de todo. El PNG va a la carpeta temporal y no
        // a la de capturas del usuario: anclar es mirar algo un rato, no guardarlo, y
        // llenarle la carpeta de recortes que no ha pedido seria un mal negocio.
        "pin" => {
            let dir = state.temp_root.join("pins");
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!(
                "pin-{}.png",
                chrono::Local::now().format("%Y%m%d-%H%M%S%3f")
            ));
            png::save(&image, &path, width, height)?;
            windows_mgr::close_overlays(app);
            windows_mgr::open_pin(app, region, &path)?;
            result.path = Some(path.to_string_lossy().to_string());
            return Ok(result);
        }
        other => return Err(AppError::Msg(format!("acción desconocida: {other}"))),
    }

    windows_mgr::close_overlays(app);
    Ok(result)
}

/// Comprueba que una ruta cae de verdad dentro de la carpeta de ancladas.
///
/// Se limita a esa carpeta a proposito: sin esto, cualquier ventana podria pedir que se
/// copiara, se guardara o se leyera un archivo cualquiera del disco.
///
/// **Y no basta con mirar el principio de la ruta.** `...\pins\..\..\otra\cosa.png`
/// empieza por la carpeta buena y acaba en cualquier sitio, asi que un `..` en medio se
/// rechaza entero. No se llama a `canonicalize`, que ademas exigiria que el archivo ya
/// existiera: un `..` en una ruta que la propia aplicacion acaba de escribir solo puede
/// venir de alguien intentandolo.
fn dentro_de_las_ancladas(path: &std::path::Path, pins: &std::path::Path) -> bool {
    !path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
        && path.starts_with(pins)
}

/// La ruta que manda una ventana anclada, comprobada.
fn ruta_de_anclada(app: &AppHandle, path: String) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    let pins = app.state::<AppState>().temp_root.join("pins");
    if !dentro_de_las_ancladas(&path, &pins) {
        return Err(AppError::Msg("esa imagen no es una captura anclada".into()));
    }
    Ok(path)
}

#[cfg(test)]
mod pruebas_de_ancladas {
    use super::dentro_de_las_ancladas;
    use std::path::Path;

    const PINS: &str = r"C:\Users\a\AppData\Local\Temp\winshotx\pins";

    #[test]
    fn una_anclada_de_verdad_pasa() {
        assert!(dentro_de_las_ancladas(
            Path::new(&format!(r"{PINS}\pin-20260827-101500123.png")),
            Path::new(PINS)
        ));
    }

    #[test]
    fn un_archivo_de_otra_carpeta_no() {
        assert!(!dentro_de_las_ancladas(
            Path::new(r"C:\Users\a\Documents\privado.png"),
            Path::new(PINS)
        ));
    }

    #[test]
    fn y_tampoco_uno_que_se_sale_con_dos_puntos() {
        // Empieza por la carpeta buena y acaba fuera: es el caso que `starts_with` a
        // secas no ve, y por el que se mira componente a componente.
        assert!(!dentro_de_las_ancladas(
            Path::new(&format!(r"{PINS}\..\..\..\.ssh\id_rsa")),
            Path::new(PINS)
        ));
    }

    #[test]
    fn una_carpeta_que_solo_empieza_igual_no_cuela() {
        // `pins-viejos` empieza por `pins` como texto, pero es otra carpeta. Se compara
        // por componentes y no por letras, asi que este caso sale bien solo.
        assert!(!dentro_de_las_ancladas(
            Path::new(&format!(r"{PINS}-viejos\pin-1.png")),
            Path::new(PINS)
        ));
    }
}

/// Copia al portapapeles la imagen de una captura anclada.
///
/// La ventana anclada tiene la imagen delante, pero solo como pixeles pintados: para
/// llevarla al portapapeles en PNG y en DIB hay que volver a leer el archivo, que es lo
/// que hace esto.
#[tauri::command]
pub async fn copy_pinned(app: AppHandle, path: String) -> Result<()> {
    let path = ruta_de_anclada(&app, path)?;
    let image = image::open(&path)?.to_rgba8();
    let bytes = std::fs::read(&path)?;
    crate::platform::clipboard::copy_image(&image, &bytes)?;
    Ok(())
}

/// Guarda en la carpeta de capturas una que estaba anclada.
///
/// El PNG de una anclada vive en el temporal y se borra al arrancar, porque anclar es
/// mirar algo un rato. Pero a veces, mirandola, resulta que se queria guardar: sin esto
/// habia que copiarla y pegarla en otro programa para conservarla.
///
/// Se copia el archivo tal cual en vez de recodificarlo: es el mismo PNG que se acaba de
/// escribir, y volver a comprimirlo solo gastaria tiempo para dar el mismo resultado.
#[tauri::command]
pub async fn save_pinned(app: AppHandle, path: String) -> Result<String> {
    let origen = ruta_de_anclada(&app, path)?;
    let directory = PathBuf::from(app.state::<AppState>().settings.read().save_directory.clone());
    let destino = crate::archivos::destino(&directory, "png")?;
    std::fs::copy(&origen, &destino)?;
    Ok(destino.to_string_lossy().to_string())
}

/// Copia al portapapeles el texto de una captura anclada, con el motor de Windows.
///
/// Es la misma tecla `T` de la barra de captura: la razon para anclar algo suele ser
/// tener delante un dato que hay que escribir en otro sitio, y ese es justo el caso en el
/// que copiarlo a mano se equivoca.
#[tauri::command]
pub async fn pinned_text(app: AppHandle, path: String) -> Result<()> {
    let path = ruta_de_anclada(&app, path)?;
    let bytes = std::fs::read(&path)?;
    let texto = crate::platform::ocr::leer_texto(&bytes)?;
    if texto.is_empty() {
        return Err(AppError::Msg(
            "No he encontrado texto en esa captura.".into(),
        ));
    }
    crate::platform::clipboard::copy_text(&texto)?;
    Ok(())
}

/// Copia un color al portapapeles, en `#rrggbb`.
///
/// La lupa de la seleccion ya ensenna el color que hay debajo del cursor; esto es lo que
/// faltaba para poder llevarselo. Se comprueba el formato aqui y no solo en la interfaz:
/// un comando que acepta cualquier cadena es un comando para escribir cualquier cosa en
/// el portapapeles de quien tenga winshotx abierto.
#[tauri::command]
pub async fn copy_color(color: String) -> Result<()> {
    if !es_un_color(&color) {
        return Err(AppError::Msg("eso no es un color".into()));
    }
    crate::platform::clipboard::copy_text(&color)?;
    Ok(())
}

/// Un `#rrggbb` y nada mas: almohadilla y seis digitos hexadecimales.
fn es_un_color(texto: &str) -> bool {
    texto.len() == 7
        && texto.starts_with('#')
        && texto[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod pruebas_de_color {
    use super::es_un_color;

    #[test]
    fn un_color_de_verdad_pasa() {
        assert!(es_un_color("#0a9bff"));
        assert!(es_un_color("#FFFFFF"));
    }

    #[test]
    fn cualquier_otra_cosa_no() {
        // Es lo unico que separa este comando de uno que escribe lo que sea en el
        // portapapeles de quien tenga la aplicacion abierta.
        for malo in ["0a9bff", "#0a9bf", "#0a9bfff", "", "#zzzzzz", "rm -rf /"] {
            assert!(!es_un_color(malo), "{malo} no tendria que pasar");
        }
    }
}

/// Si las fotos van por la captura moderna o por la de reserva. Lo preguntan los ajustes
/// para avisar, porque la diferencia se ve en la foto y no hay nada que el usuario pueda
/// tocar: en un Windows sin ella, las ventanas aceleradas salen con trozos en negro.
#[tauri::command]
pub async fn captura_moderna() -> bool {
    crate::capture::moderna_disponible()
}

#[tauri::command]
pub async fn cancel_capture(app: AppHandle) {
    windows_mgr::close_overlays(&app);
}

/// Una marca del cronometro puesta desde el frontend. Sin `--crono` no hace nada.
#[tauri::command]
pub async fn crono_marca(etapa: String) {
    crate::crono::marca(&etapa);
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    region: Rect,
    options: RecordOptions,
) -> Result<SessionInfo> {
    recorder::start(&app, region, options)
}

/// Vuelve enseguida: lo que tarda (miniaturas, editor o archivo) pasa por detras, y la
/// barra se entera por el ultimo tick, que llega con `saving`.
#[tauri::command]
pub async fn stop_recording(app: AppHandle) -> Result<()> {
    recorder::stop(&app)
}

#[tauri::command]
pub async fn pause_recording(app: AppHandle, paused: bool) -> Result<()> {
    recorder::set_paused(&app, paused)
}

#[tauri::command]
pub async fn cancel_recording(app: AppHandle) -> Result<()> {
    recorder::cancel(&app)
}

fn session_of(app: &AppHandle, session_id: &str) -> Result<record::SessionData> {
    let state = app.state::<AppState>();
    let sessions = state.sessions.read();
    sessions
        .get(session_id)
        .cloned()
        .ok_or_else(|| AppError::UnknownSession(session_id.to_string()))
}

#[tauri::command]
pub async fn session_info(app: AppHandle, session_id: String) -> Result<SessionInfo> {
    Ok(SessionInfo::from(&session_of(&app, &session_id)?))
}

#[tauri::command]
pub async fn session_frames(app: AppHandle, session_id: String) -> Result<Vec<FrameDto>> {
    Ok(session_of(&app, &session_id)?
        .frames
        .into_iter()
        .map(|frame| FrameDto {
            index: frame.index,
            timestamp_ms: frame.timestamp_ms,
            duration_ms: frame.duration_ms,
            thumb_path: frame.thumb_path,
        })
        .collect())
}

/// Lo que se anoto al grabar y de lo que sale el estudio: clics, atajos y el rastro del
/// raton. La vista previa del editor lo dibuja en vivo encima del video, para que el zoom,
/// los aros y el puntero se vean ANTES de exportar y no solo en el archivo.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioDto {
    pub clics: Vec<crate::encode::zoom::Clic>,
    pub teclas: Vec<crate::record::teclas::Atajo>,
    /// `[ms, x, y]` por fotograma, en pixeles de la region grabada.
    pub cursor: Vec<(u64, i32, i32)>,
    /// `[ms, forma]` cada vez que el puntero cambio de forma: 0 flecha, 1 texto, 2 mano.
    pub formas: Vec<(u64, u8)>,
    /// Las imagenes de verdad de los punteros, como PNG en la carpeta de la sesion.
    pub punteros: Vec<record::PunteroGrabado>,
    /// `[ms, id]` cada vez que cambio el puntero puesto. `4294967295` es uno ilegible.
    pub cambios_puntero: Vec<(u64, u32)>,
    /// Donde se pulsó la tecla de zoom al grabar: cada una sale como un zoom en la pista.
    pub marcas_zoom: Vec<crate::contrato::MarcaZoom>,
    /// La webcam, si se grabó.
    pub camara: Option<crate::contrato::CamaraGrabada>,
}

#[tauri::command]
pub async fn session_studio(app: AppHandle, session_id: String) -> Result<StudioDto> {
    let session = session_of(&app, &session_id)?;
    Ok(StudioDto {
        clics: session.clics,
        teclas: session.teclas,
        cursor: session.cursor,
        formas: session.formas,
        punteros: session.punteros,
        cambios_puntero: session.cambios_puntero,
        marcas_zoom: session.marcas_zoom,
        camara: session.camara,
    })
}

/// La camara del zoom en cada fotograma, con ese zoom y ese recorte: la misma que usara la
/// exportacion, para que la vista previa ensenne exactamente lo que va a salir.
#[tauri::command]
pub async fn session_camera(
    app: AppHandle,
    session_id: String,
    zoom: f32,
    crop: Option<crate::encode::recorte::Recorte>,
    zoom_speed: Option<f32>,
) -> Result<Vec<crate::exporter::MuestraCamara>> {
    let session = session_of(&app, &session_id)?;
    Ok(crate::exporter::muestras_de_camara(
        &session,
        zoom,
        crop,
        zoom_speed.unwrap_or(1.0),
    ))
}

#[tauri::command]
pub async fn frame_image(app: AppHandle, session_id: String, index: usize) -> Result<String> {
    let session = session_of(&app, &session_id)?;
    let directory = session.dir.join("full");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{index:06}.png"));
    if !path.exists() {
        let image = record::read_frame(&session, index)?;
        // Comprimido lo minimo: es una vista previa temporal que se pide en cada
        // fotograma al que se salta con el raton, y con la compresion de fabrica de
        // `image` cada salto costaba 180 ms a 1080p. Ver `png::save_fast`.
        png::save_fast(&image, &path)?;
    }
    Ok(path.to_string_lossy().to_string())
}

/// Lo que se ha tapado y con que marcas, para poder ensennarlo y para poder contarlo.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatosTapados {
    pub marcas: Vec<crate::encode::anotacion::Anotacion>,
    /// Cuantos de cada clase, en el orden en que se encontraron: «2 correos, 1 telefono».
    pub resumen: Vec<(String, usize)>,
}

/// Cuanto se agranda la caja de una palabra antes de taparla, en veces su alto.
///
/// El rectangulo que da el lector es el de las letras, y las letras tienen colas y
/// acentos que se salen de el. Un mosaico que deja asomar la mitad de una «j» no tapa
/// nada, y uno que se pasa dos pixeles no molesta a nadie.
const MARGEN: f32 = 0.18;

/// Busca datos personales en un fotograma y devuelve las marcas que los taparian.
///
/// **No cambia nada por su cuenta.** Devuelve las marcas y las annade el editor, que es
/// quien sabe lo que el usuario ya habia dibujado y quien puede deshacerlas con Ctrl+Z.
/// Tapar algo a la fuerza en la imagen seria lo unico de la aplicacion que no se puede
/// deshacer, porque el mosaico borra los pixeles de verdad.
#[tauri::command]
pub async fn tapar_datos(app: AppHandle, session_id: String, index: usize) -> Result<DatosTapados> {
    let session = session_of(&app, &session_id)?;
    let image = record::read_frame(&session, index)?;
    let (ancho, alto) = (image.width() as f32, image.height() as f32);
    let bytes = png::to_bytes(&image)?;

    let lineas = crate::platform::ocr::leer_palabras(&bytes)?;
    let mut marcas = Vec::new();
    let mut cuenta: Vec<(String, usize)> = Vec::new();
    for linea in &lineas {
        for hallazgo in crate::sensibles::buscar_en_linea(linea) {
            let Some(marca) = caja_de(linea, &hallazgo.palabras, ancho, alto) else {
                continue;
            };
            marcas.push(marca);
            let nombre = hallazgo.clase.nombre().to_string();
            match cuenta.iter_mut().find(|(n, _)| *n == nombre) {
                Some((_, cuantos)) => *cuantos += 1,
                None => cuenta.push((nombre, 1)),
            }
        }
    }
    Ok(DatosTapados {
        marcas,
        resumen: cuenta,
    })
}

/// Una captura que ya está en el disco, para poder volver a ella sin repetirla.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturaReciente {
    pub path: String,
    pub nombre: String,
    /// Hace cuántos segundos se guardó. El frontend lo escribe como «hace 3 min».
    pub hace_segundos: u64,
}

/// Las últimas capturas guardadas, de la más nueva a la más vieja.
///
/// **El historial no se guarda en ningún sitio: se lee de la carpeta.** Una lista aparte
/// sería otro archivo de estado que mantener, que se desincroniza en cuanto alguien borra
/// o mueve una captura por fuera, y que habría que limpiar. Lo que hay en la carpeta ES lo
/// que hay, y si el usuario borra un archivo desaparece de aquí sin que nadie haga nada.
///
/// Lo que solo se copió al portapapeles y nunca se guardó no sale, y es correcto que no
/// salga: no existe en ninguna parte.
#[tauri::command]
pub async fn ultimas_capturas(app: AppHandle, cuantas: usize) -> Result<Vec<CapturaReciente>> {
    let directory = PathBuf::from(app.state::<AppState>().settings.read().save_directory.clone());
    let Ok(entradas) = std::fs::read_dir(&directory) else {
        // Que la carpeta no exista todavía no es un error: es que no se ha guardado nada.
        return Ok(Vec::new());
    };
    let ahora = std::time::SystemTime::now();
    let mut fotos: Vec<(std::time::SystemTime, PathBuf)> = entradas
        .flatten()
        .filter_map(|e| {
            let ruta = e.path();
            let extension = ruta.extension()?.to_string_lossy().to_lowercase();
            if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp") {
                return None;
            }
            Some((e.metadata().ok()?.modified().ok()?, ruta))
        })
        .collect();
    fotos.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(fotos
        .into_iter()
        .take(cuantas.clamp(1, 20))
        .map(|(cuando, ruta)| CapturaReciente {
            nombre: ruta
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            hace_segundos: ahora
                .duration_since(cuando)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            path: ruta.to_string_lossy().to_string(),
        })
        .collect())
}

/// Vuelve a dejar en el portapapeles una captura que ya estaba guardada.
#[tauri::command]
pub async fn copiar_captura(path: String) -> Result<()> {
    let image = image::open(&path)?.to_rgba8();
    let bytes = png::to_bytes(&image)?;
    crate::platform::clipboard::copy_image(&image, &bytes)
}

/// Copia al portapapeles el texto que hay en el fotograma que se está viendo.
///
/// La tecla `T` ya hacía esto **en la captura**, antes de llegar al editor, y ahí se
/// quedaba: una vez dentro, para copiar un trozo de texto de la imagen había que salir,
/// capturar otra vez el mismo sitio y pulsar `T`. Y en una grabación no había forma
/// ninguna, porque de un vídeo no se sale a capturar el fotograma que acabas de ver.
#[tauri::command]
pub async fn copiar_texto(app: AppHandle, session_id: String, index: usize) -> Result<String> {
    let session = session_of(&app, &session_id)?;
    let image = record::read_frame(&session, index)?;
    let bytes = png::to_bytes(&image)?;
    let texto = crate::platform::ocr::leer_texto(&bytes)?;
    if texto.trim().is_empty() {
        return Err(AppError::Msg("No he encontrado texto en esta imagen.".into()));
    }
    crate::platform::clipboard::copy_text(&texto)?;
    Ok(texto)
}

/// La marca que tapa esas palabras: su caja envolvente, con margen, de 0 a 1.
fn caja_de(
    linea: &[crate::sensibles::Palabra],
    cuales: &[usize],
    ancho: f32,
    alto: f32,
) -> Option<crate::encode::anotacion::Anotacion> {
    if ancho <= 0.0 || alto <= 0.0 {
        return None;
    }
    let partes: Vec<&crate::sensibles::Palabra> = cuales.iter().filter_map(|i| linea.get(*i)).collect();
    let primera = partes.first()?;
    let mut x1 = primera.x;
    let mut y1 = primera.y;
    let mut x2 = primera.x + primera.ancho;
    let mut y2 = primera.y + primera.alto;
    for p in partes.iter().skip(1) {
        x1 = x1.min(p.x);
        y1 = y1.min(p.y);
        x2 = x2.max(p.x + p.ancho);
        y2 = y2.max(p.y + p.alto);
    }
    let margen = (y2 - y1) * MARGEN;
    Some(crate::encode::anotacion::Anotacion {
        kind: "blur".into(),
        x1: ((x1 - margen) / ancho).clamp(0.0, 1.0),
        y1: ((y1 - margen) / alto).clamp(0.0, 1.0),
        x2: ((x2 + margen) / ancho).clamp(0.0, 1.0),
        y2: ((y2 + margen) / alto).clamp(0.0, 1.0),
        color: String::new(),
        text: String::new(),
    })
}

#[tauri::command]
pub async fn export_media(app: AppHandle, request: ExportRequest) -> Result<ExportResult> {
    exporter::export(&app, request)
}

#[tauri::command]
pub async fn ffmpeg_available() -> bool {
    // Lanzar un proceso puede tardar; por eso vive fuera del hilo de la interfaz.
    ffmpeg::available()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    bytes: u64,
    sessions: u32,
}

/// Cuanto ocupa el cache de sesiones: es lo unico que crece solo en el disco.
#[tauri::command]
pub async fn cache_stats(app: AppHandle) -> Result<CacheStats> {
    let root = app.state::<AppState>().temp_root.join("sessions");
    let mut bytes = 0u64;
    let mut sessions = 0u32;
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            sessions += 1;
            if let Ok(files) = std::fs::read_dir(entry.path()) {
                for file in files.flatten() {
                    bytes += file.metadata().map(|m| m.len()).unwrap_or(0);
                    if let Ok(inner) = std::fs::read_dir(file.path()) {
                        for thumb in inner.flatten() {
                            bytes += thumb.metadata().map(|m| m.len()).unwrap_or(0);
                        }
                    }
                }
            }
        }
    }
    Ok(CacheStats { bytes, sessions })
}

/// Borra las sesiones guardadas, menos la que se este grabando ahora mismo.
#[tauri::command]
pub async fn clear_cache(app: AppHandle) -> Result<CacheStats> {
    let state = app.state::<AppState>();
    let root = state.temp_root.join("sessions");
    // El editor lee los fotogramas del disco segun los pide: borrarlos con la
    // ventana abierta la deja mostrando una sesion que ya no existe.
    if app
        .webview_windows()
        .keys()
        .any(|label| label.starts_with(windows_mgr::EDITOR_LABEL))
    {
        return Err(AppError::Msg(
            "cierra el editor antes de vaciar la caché".into(),
        ));
    }
    if state.is_recording() {
        return Err(AppError::Msg(
            "hay una grabación en curso; párala antes de vaciar la caché".into(),
        ));
    }
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)?;
    state.sessions.write().clear();
    cache_stats(app.clone()).await
}

#[tauri::command]
pub async fn shortcut_status(state: State<'_, AppState>) -> Result<crate::hotkeys::ShortcutStatus> {
    Ok(*state.shortcuts.read())
}

/// Las pantallas que hay, para poder elegir cual vigila el anillo.
///
/// No captura nada: solo dice cuantas son, como se llaman y cual es la principal.
#[tauri::command]
pub async fn list_screens() -> Result<Vec<crate::capture::MonitorInfo>> {
    crate::capture::monitors()
}

/// Ensenna el numero de esa pantalla, en esa pantalla, un par de segundos.
#[tauri::command]
pub async fn show_screen_number(app: AppHandle, screen: u32) -> Result<()> {
    windows_mgr::mostrar_numero_de_pantalla(&app, screen);
    Ok(())
}

/// Como va el anillo de los ultimos segundos: si corre, que pantalla vigila y cuanto lleva
/// grabado. La interfaz lo pregunta al abrirse; despues se entera por el evento.
#[tauri::command]
pub async fn replay_status(app: AppHandle) -> Result<crate::replay::ReplayStatus> {
    Ok(crate::replay::status(&app))
}

/// Quedarse con lo ultimo que paso. Es lo mismo que hace la tecla.
#[tauri::command]
pub async fn replay_save(app: AppHandle) -> Result<()> {
    crate::replay::save(&app)
}

/// Lleva al usuario a la lista de aplicaciones de Windows, que es donde se quita la
/// Herramienta de Recortes si quiere que Win+Mayus+S deje de abrirla.
#[tauri::command]
pub async fn open_windows_apps() -> Result<()> {
    crate::platform::abrir_aplicaciones_de_windows()
}

/// Quita la Herramienta de Recortes de este usuario. Devuelve si habia algo que quitar.
///
/// Solo se llega aqui pulsando dos veces a proposito en los ajustes. Vuelve desde la
/// Microsoft Store, no hace falta ser administrador y no le afecta a nadie mas del equipo.
#[tauri::command]
pub async fn remove_snipping_tool() -> Result<bool> {
    crate::platform::snipping::uninstall_snipping_tool()
}

#[tauri::command]
pub async fn open_folder(path: String) -> Result<()> {
    crate::platform::open_folder(&PathBuf::from(path))
}

/// Abre uno de los cuatro sitios de winshotx en el navegador del usuario.
///
/// La direccion llega desde la ventana, asi que se comprueba contra la lista de
/// `crate::enlaces` antes de tocar nada: es el unico comando que saca a alguien de la
/// aplicacion, y sin la lista una cadena cualquiera acabaria en `explorer`.
#[tauri::command]
pub async fn open_url(url: String) -> Result<()> {
    if !crate::enlaces::permitido(&url) {
        return Err(crate::error::AppError::Msg(format!(
            "esa dirección no es de winshotx: {url}"
        )));
    }
    crate::platform::abrir_url(&url)
}

#[tauri::command]
pub async fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintScreenState {
    /// winshotx ha pedido la tecla.
    enabled: bool,
    /// Y Windows se la ha dado de verdad. Lo segundo no se deduce de lo primero.
    active: bool,
    /// La Herramienta de Recortes la sigue teniendo asignada en el registro. Cuando el
    /// valor no existe se da por asignada, que es como viene Windows 11 de fabrica.
    taken_by_windows: bool,
}

fn print_screen_now(state: &AppState) -> PrintScreenState {
    PrintScreenState {
        enabled: state.settings.read().print_screen_capture,
        active: state.shortcuts.read().print_screen,
        taken_by_windows: crate::platform::snipping::read().unwrap_or(1) == 1,
    }
}

#[tauri::command]
pub async fn print_screen_state(state: State<'_, AppState>) -> Result<PrintScreenState> {
    Ok(print_screen_now(&state))
}

/// Le quita la tecla Impr Pant a la Herramienta de Recortes y se la da a winshotx,
/// o la devuelve. Las dos cosas van juntas a proposito: apagar el ajuste de Windows
/// sin registrar el atajo deja la tecla muerta, y registrarlo sin apagar el ajuste
/// deja un atajo que nunca se dispara.
#[tauri::command]
pub async fn use_print_screen(app: AppHandle, enabled: bool) -> Result<PrintScreenState> {
    use crate::platform::snipping;

    let state = app.state::<AppState>();
    let mut settings = state.settings.read().clone();

    if enabled {
        // Solo la primera vez: si ya estaba activo, lo guardado es el 0 que pusimos
        // nosotros y machacarlo seria perder el valor original del usuario.
        if !settings.print_screen_capture {
            settings.snipping_key_restore = snipping::read();
            settings.disabled_hotkeys_restore = snipping::read_disabled_hotkeys();
        }
        snipping::write(0)?;
        settings.print_screen_capture = true;
    } else {
        // Se deja el registro como estaba, incluido el caso de que no hubiera valor.
        match settings.snipping_key_restore.take() {
            Some(previo) => snipping::write(previo)?,
            None => snipping::remove()?,
        }
        settings.print_screen_capture = false;
    }

    *state.settings.write() = settings.clone();
    crate::settings::save(&app, &settings)?;
    crate::hotkeys::register(&app, &settings);
    Ok(print_screen_now(&state))
}

/// Le quita `Win+Mayus+S` al escritorio, o se la devuelve.
///
/// Esto es aparte de todo lo demas porque es lo unico de winshotx que le quita algo al
/// usuario: la lista de atajos de Windows va por LETRA, no por combinacion, asi que apagar
/// la S para que `Win+Mayus+S` no abra el recorte apaga tambien `Win+S`, la busqueda. No hay
/// forma de afinar mas, y nadie deberia pagar eso sin haberlo pedido.
///
/// Ninguna de las dos cosas surte efecto hasta que el escritorio vuelve a leer esa lista,
/// cosa que solo hace al arrancar. Para eso esta `restart_shell`, que se lo hace leer en dos
/// segundos sin obligar a nadie a cerrar sesion.
#[tauri::command]
pub async fn use_win_shift_s(app: AppHandle, enabled: bool) -> Result<bool> {
    use crate::platform::snipping;

    let state = app.state::<AppState>();
    let mut settings = state.settings.read().clone();

    if enabled {
        if !settings.take_win_shift_s {
            settings.disabled_hotkeys_restore = snipping::read_disabled_hotkeys();
        }
        // Se anade a lo que hubiera en vez de sustituirlo: quien tuviera otras letras
        // apagadas las conserva.
        let previas = settings.disabled_hotkeys_restore.clone().unwrap_or_default();
        if !previas.to_uppercase().contains('S') {
            snipping::write_disabled_hotkeys(Some(&format!("{previas}S")))?;
        }
        settings.take_win_shift_s = true;
    } else {
        let previas = settings.disabled_hotkeys_restore.take();
        snipping::write_disabled_hotkeys(previas.as_deref())?;
        settings.take_win_shift_s = false;
    }

    *state.settings.write() = settings.clone();
    crate::settings::save(&app, &settings)?;
    crate::hotkeys::register(&app, &settings);
    Ok(settings.take_win_shift_s)
}

/// Reinicia el Explorador para que el escritorio relea la lista de teclas apagadas, y
/// vuelve a pedir los atajos con la tecla ya libre.
///
/// Las dos partes tienen que ir juntas: al reiniciar el shell la tecla queda suelta, y si
/// nadie la pide en ese momento no la tiene ni Windows ni winshotx, o sea que deja de hacer
/// nada. Devuelve el estado de los atajos para poder decir en la interfaz si se consiguio.
#[tauri::command]
pub async fn restart_shell(app: AppHandle) -> Result<crate::hotkeys::ShortcutStatus> {
    crate::platform::snipping::restart_shell()?;

    let settings = app.state::<AppState>().settings.read().clone();
    Ok(crate::hotkeys::register(&app, &settings))
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    Ok(state.settings.read().clone())
}

#[tauri::command]
pub async fn set_settings(app: AppHandle, settings: Settings) -> Result<Settings> {
    let state = app.state::<AppState>();
    let previous = state.settings.read().clone();
    *state.settings.write() = settings.clone();
    crate::settings::save(&app, &settings)?;

    // El anillo de los ultimos segundos se reengancha aqui. Cambiar los segundos tambien
    // lo reinicia: la ventana se fija al abrirlo y no se puede estirar sobre la marcha sin
    // tirar lo que ya hay dentro, que es justo lo que nadie quiere que pase.
    let mut settings = settings;
    let cambia_como_graba = previous.replay_seconds != settings.replay_seconds
        || previous.replay_screen != settings.replay_screen
        || previous.replay_fps != settings.replay_fps;
    if previous.replay_enabled != settings.replay_enabled
        || (settings.replay_enabled && cambia_como_graba)
    {
        crate::replay::stop(&app)?;
        if settings.replay_enabled {
            if let Err(error) = crate::replay::start(&app) {
                // Si no se ha podido encender, el ajuste no puede quedarse diciendo que si:
                // el interruptor tiene que contar lo que esta pasando de verdad.
                settings.replay_enabled = false;
                state.settings.write().replay_enabled = false;
                crate::settings::save(&app, &settings)?;
                crate::hotkeys::register(&app, &settings);
                return Err(error);
            }
        }
    }

    // Se registra siempre, aunque la combinacion no haya cambiado: es la unica forma
    // de reintentar cuando el atajo estaba cogido por otra aplicacion y ya se ha
    // cerrado. `register` empieza desregistrando todo, asi que repetirlo no molesta.
    crate::hotkeys::register(&app, &settings);
    if previous.start_with_windows != settings.start_with_windows {
        crate::platform::autostart::set(settings.start_with_windows)?;
    }
    Ok(settings)
}

/// Lo que el menu de la bandeja necesita saber para pintarse.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuState {
    pub version: String,
    /// Si hay una grabacion en marcha: entonces la entrada de grabar la para.
    pub recording: bool,
    /// Si el anillo de los ultimos segundos esta encendido.
    pub replay: bool,
    /// Los tres atajos, para ensennarlos al lado de su entrada como haria un menu bueno.
    pub capture_shortcut: String,
    pub record_shortcut: String,
    pub replay_shortcut: String,
}

#[tauri::command]
pub async fn tray_menu_state(app: AppHandle) -> Result<TrayMenuState> {
    let state = app.state::<AppState>();
    let settings = state.settings.read().clone();
    // El guardia del anillo se suelta AQUI y no dentro del `Ok(...)`: dentro seguiria
    // vivo al acabar el bloque, cuando `state` ya no esta, y eso no compila.
    let anillo = state.replay.lock().is_some();
    Ok(TrayMenuState {
        version: app.package_info().version.to_string(),
        recording: state.is_recording(),
        replay: anillo,
        capture_shortcut: settings.capture_shortcut,
        record_shortcut: settings.record_shortcut,
        replay_shortcut: settings.replay_shortcut,
    })
}

/// Una entrada del menu, pulsada.
///
/// Va al mismo sitio que iban las entradas del menu del sistema: `tray::ejecutar`. El menu
/// se esconde ANTES de hacer nada, porque casi todo lo que hay dentro abre una ventana o
/// congela la pantalla, y hacerlo con el menu todavia puesto lo mete en la foto.
#[tauri::command]
pub async fn tray_menu_action(app: AppHandle, action: String) -> Result<()> {
    crate::tray_menu::esconder(&app);
    crate::tray::ejecutar(&app, &action);
    Ok(())
}

/// El alto que ha medido el menu, para que la ventana se ajuste a su contenido.
#[tauri::command]
pub async fn resize_tray_menu(app: AppHandle, height: f64) -> Result<()> {
    crate::tray_menu::redimensionar(&app, height);
    Ok(())
}

#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|folder| folder.to_string())
}

/// El dialogo de «Guardar como» de Windows, para elegir carpeta Y nombre de una vez.
///
/// Arranca en la carpeta que se le diga (la de capturas, si no hay otra) y con el nombre
/// que le habria puesto `archivos`, para que aceptar sin tocar nada de lo mismo que guardar
/// a secas. Devuelve la ruta entera, o nada si se cancelo. La extension la asegura quien
/// escribe el archivo: el dialogo deja borrarla.
fn elegir_donde_guardar(app: &AppHandle, carpeta: &std::path::Path, extension: &str) -> Option<PathBuf> {
    use tauri_plugin_dialog::DialogExt;
    let sugerido = crate::archivos::destino(carpeta, extension).ok()?;
    let nombre = sugerido.file_name()?.to_string_lossy().to_string();
    let elegido = app
        .dialog()
        .file()
        .set_directory(carpeta)
        .set_file_name(&nombre)
        .add_filter(extension.to_uppercase(), &[extension])
        .blocking_save_file()?;
    elegido.into_path().ok()
}

/// «Guardar en…» del editor: el mismo dialogo, y el archivo elegido se le pasa despues a
/// `export_media` en `file`. Va aparte de exportar porque el dialogo bloquea, y el editor
/// quiere ensennar la barra de progreso solo cuando de verdad se esta escribiendo.
#[tauri::command]
pub async fn pick_save_file(
    app: AppHandle,
    extension: String,
    directory: Option<String>,
) -> Option<String> {
    let carpeta = directory
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| app.state::<AppState>().settings.read().save_directory.clone());
    elegir_donde_guardar(&app, &PathBuf::from(carpeta), &extension)
        .map(|ruta| ruta.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn reveal_in_explorer(path: String) -> Result<()> {
    crate::platform::reveal(&PathBuf::from(path))
}

/// Cierto una sola vez, si este arranque viene de actualizar. Lo pregunta la fila de
/// Actualizaciones al montarse, para decir que se ha actualizado en vez de quedarse
/// callada: los ajustes se abren solos despues de una actualizacion, y una ventana que
/// aparece sin motivo se lee como un fallo.
#[tauri::command]
pub async fn just_updated(app: AppHandle) -> bool {
    app.state::<AppState>().consumir_recien_actualizado()
}

/// Cierto si winshotx viene de la Microsoft Store. Lo pregunta la barra de abajo para
/// no ponerse a buscar versiones nuevas: dentro de un paquete MSIX la carpeta de la app
/// es de solo lectura, asi que el boton de actualizar no podria hacer nada aunque
/// encontrase algo, y de eso ya se encarga la Store sola.
#[tauri::command]
pub async fn is_store_build() -> bool {
    crate::platform::empaquetado::es_msix()
}

#[tauri::command]
pub async fn discard_session(app: AppHandle, session_id: String) -> Result<()> {
    let state = app.state::<AppState>();
    if let Some(session) = state.sessions.write().remove(&session_id) {
        let _ = std::fs::remove_dir_all(&session.dir);
    }
    Ok(())
}

/// El interruptor de la barra de arriba: si al soltar el recorte sale la barra de
/// acciones (copiar, guardar, editar, anclar) o la foto se copia sola.
///
/// Va aparte de `set_settings` a proposito, aunque escriba en los mismos ajustes. Aquel
/// desregistra y vuelve a registrar los tres atajos globales, mira si hay que reenganchar
/// el anillo y toca el arranque con Windows, y esto se pulsa **con el overlay abierto**,
/// que es el unico momento del dia en el que la maquina tiene prisa de verdad.
#[tauri::command]
pub async fn set_capture_flow(app: AppHandle, flow: crate::settings::CaptureFlow) -> Result<()> {
    let state = app.state::<AppState>();
    let settings = {
        let mut guard = state.settings.write();
        guard.capture_flow = flow;
        guard.clone()
    };
    crate::settings::save(&app, &settings)?;
    Ok(())
}

/// Los ajustes, abiertos desde la barra del overlay.
///
/// La captura se cierra primero: los ajustes son una ventana normal, y abrirla debajo de
/// tres overlays a pantalla completa y siempre encima es abrirla donde no se ve.
#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<()> {
    windows_mgr::close_overlays(&app);
    windows_mgr::show_settings(&app)
}

#[cfg(test)]
mod pruebas_de_tapar {
    use super::*;
    use crate::sensibles::Palabra;

    fn palabra(texto: &str, x: f32, y: f32, ancho: f32, alto: f32) -> Palabra {
        Palabra {
            texto: texto.into(),
            x,
            y,
            ancho,
            alto,
        }
    }

    /// La caja se mide **de 0 a 1 sobre la imagen**, que es como van todas las marcas.
    ///
    /// Es donde se cuela un fallo de coordenadas sin que nada falle: una marca en píxeles
    /// se ve bien en la vista previa de hoy y aparece en otro sitio en cuanto se exporta a
    /// otro tamaño.
    #[test]
    fn la_caja_sale_en_tanto_por_uno_sobre_la_imagen() {
        // Una palabra de 100 px de ancho en la mitad de una imagen de 1000 x 500.
        let linea = vec![palabra("hola@example.com", 400.0, 240.0, 200.0, 20.0)];
        let marca = caja_de(&linea, &[0], 1000.0, 500.0).expect("una marca");
        assert_eq!(marca.kind, "blur", "lo que tapa es el mosaico, no otra cosa");
        // El margen es el 18 % del alto de la palabra: 3,6 px arriba y abajo.
        assert!((marca.x1 - 0.3964).abs() < 0.001, "x1 ha salido {}", marca.x1);
        assert!((marca.x2 - 0.6036).abs() < 0.001, "x2 ha salido {}", marca.x2);
        assert!(marca.y1 < 0.48 && marca.y2 > 0.52, "la banda tiene que cubrir la palabra");
    }

    /// Varias palabras son UNA caja: una tarjeta en cuatro grupos se tapa de una pieza, no
    /// con cuatro mosaicos que dejan ver los huecos de en medio.
    #[test]
    fn varias_palabras_se_tapan_con_una_sola_caja() {
        let linea = vec![
            palabra("4539", 100.0, 50.0, 40.0, 10.0),
            palabra("5787", 150.0, 50.0, 40.0, 10.0),
            palabra("6362", 200.0, 50.0, 40.0, 10.0),
            palabra("1486", 250.0, 50.0, 40.0, 10.0),
        ];
        let marca = caja_de(&linea, &[0, 1, 2, 3], 400.0, 100.0).expect("una marca");
        assert!(marca.x1 < 0.25, "empieza en la primera cifra: {}", marca.x1);
        assert!(marca.x2 > 0.72, "y llega hasta la última: {}", marca.x2);
    }

    /// Lo que asoma por el borde se queda dentro: una marca fuera del lienzo no se pinta
    /// (ver `encode::anotacion`), así que un dato pegado al borde se quedaría sin tapar.
    #[test]
    fn una_palabra_pegada_al_borde_no_se_sale_de_la_imagen() {
        let linea = vec![palabra("600123456", 0.0, 0.0, 120.0, 20.0)];
        let marca = caja_de(&linea, &[0], 200.0, 100.0).expect("una marca");
        assert_eq!(marca.x1, 0.0, "el margen no la saca por la izquierda");
        assert_eq!(marca.y1, 0.0, "ni por arriba");
        assert!(marca.x2 <= 1.0 && marca.y2 <= 1.0);
    }

    #[test]
    fn sin_palabras_no_hay_marca() {
        assert!(caja_de(&[], &[], 100.0, 100.0).is_none());
        let linea = vec![palabra("hola", 0.0, 0.0, 10.0, 10.0)];
        assert!(caja_de(&linea, &[7], 100.0, 100.0).is_none(), "un índice que no existe");
        assert!(caja_de(&linea, &[0], 0.0, 100.0).is_none(), "una imagen sin ancho");
    }
}
