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
    freeze_path: String,
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
    let posicion = freezes
        .iter()
        .position(|f| f.monitor.id == monitor_id)
        .ok_or_else(|| AppError::Msg(format!("el monitor {monitor_id} no tiene captura congelada")))?;
    let freeze = &freezes[posicion];

    Ok(OverlayPayload {
        monitor: freeze.monitor.clone(),
        freeze_path: freeze.path.to_string_lossy().to_string(),
        windows: capture::window_rects(),
        settings: state.settings.read().clone(),
        intent: *state.intent.read(),
        // El orden de `freezes` es el de los monitores del sistema, asi que el numero que
        // se pinta en cada pantalla es siempre el mismo entre disparo y disparo.
        screen_number: posicion + 1,
        screen_count: freezes.len(),
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

/// Respaldo del overlay: el PNG congelado servido por el propio IPC.
/// El camino normal es el protocolo asset, pero si ese falla (CSP, ambito del
/// scope, ruta fuera de $TEMP) el overlay se quedaria en negro tapando la
/// pantalla entera. Con esto siempre hay una segunda via para pintar el fondo.
#[tauri::command]
pub async fn freeze_bytes(
    state: State<'_, AppState>,
    monitor_id: u32,
) -> Result<tauri::ipc::Response> {
    let path = {
        let freezes = state.freezes.read();
        freezes
            .iter()
            .find(|f| f.monitor.id == monitor_id)
            .map(|f| f.path.clone())
            .ok_or_else(|| AppError::Msg(format!("el monitor {monitor_id} no tiene captura congelada")))?
    };
    Ok(tauri::ipc::Response::new(std::fs::read(path)?))
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
    let copy_after = state.settings.read().copy_after_capture;

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
            std::fs::create_dir_all(&directory)?;
            let path = directory.join(format!(
                "winshotx-{}.png",
                chrono::Local::now().format("%Y%m%d-%H%M%S")
            ));
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
        other => return Err(AppError::Msg(format!("acción desconocida: {other}"))),
    }

    windows_mgr::close_overlays(app);
    Ok(result)
}

#[tauri::command]
pub async fn cancel_capture(app: AppHandle) {
    windows_mgr::close_overlays(&app);
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    region: Rect,
    options: RecordOptions,
) -> Result<SessionInfo> {
    recorder::start(&app, region, options)
}

#[tauri::command]
pub async fn stop_recording(app: AppHandle) -> Result<SessionInfo> {
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

#[tauri::command]
pub async fn frame_image(app: AppHandle, session_id: String, index: usize) -> Result<String> {
    let session = session_of(&app, &session_id)?;
    let directory = session.dir.join("full");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{index:06}.png"));
    if !path.exists() {
        let image = record::read_frame(&session, index)?;
        image.save(&path)?;
    }
    Ok(path.to_string_lossy().to_string())
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

    // Se registra siempre, aunque la combinacion no haya cambiado: es la unica forma
    // de reintentar cuando el atajo estaba cogido por otra aplicacion y ya se ha
    // cerrado. `register` empieza desregistrando todo, asi que repetirlo no molesta.
    crate::hotkeys::register(&app, &settings);
    if previous.start_with_windows != settings.start_with_windows {
        crate::platform::autostart::set(settings.start_with_windows)?;
    }
    Ok(settings)
}

#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|folder| folder.to_string())
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

#[tauri::command]
pub async fn discard_session(app: AppHandle, session_id: String) -> Result<()> {
    let state = app.state::<AppState>();
    if let Some(session) = state.sessions.write().remove(&session_id) {
        let _ = std::fs::remove_dir_all(&session.dir);
    }
    Ok(())
}
