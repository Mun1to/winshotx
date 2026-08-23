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
use crate::windows_mgr;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayPayload {
    monitor: MonitorInfo,
    freeze_path: String,
    windows: Vec<WindowRect>,
    settings: Settings,
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
    let freezes = state.freezes.read();
    let freeze = freezes
        .iter()
        .find(|f| f.monitor.id == monitor_id)
        .ok_or_else(|| AppError::Msg(format!("el monitor {monitor_id} no tiene captura congelada")))?;

    Ok(OverlayPayload {
        monitor: freeze.monitor.clone(),
        freeze_path: freeze.path.to_string_lossy().to_string(),
        windows: capture::window_rects(),
        settings: state.settings.read().clone(),
    })
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
    let state = app.state::<AppState>();
    let image = {
        let freezes = state.freezes.read();
        capture::crop_from_freeze(&freezes, region)?
    };
    let (width, height) = (image.width(), image.height());
    let copy_after = state.settings.read().copy_after_capture;

    let mut result = StillResult {
        path: None,
        copied: false,
        width,
        height,
    };

    match action.as_str() {
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
            let session = recorder::session_from_image(&app, &image, region)?;
            windows_mgr::close_overlays(&app);
            windows_mgr::open_editor(&app, &session.id)?;
            return Ok(result);
        }
        other => return Err(AppError::Msg(format!("acción desconocida: {other}"))),
    }

    windows_mgr::close_overlays(&app);
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

#[tauri::command]
pub async fn open_folder(path: String) -> Result<()> {
    crate::platform::open_folder(&PathBuf::from(path))
}

#[tauri::command]
pub async fn quit_app(app: AppHandle) {
    app.exit(0);
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

#[tauri::command]
pub async fn discard_session(app: AppHandle, session_id: String) -> Result<()> {
    let state = app.state::<AppState>();
    if let Some(session) = state.sessions.write().remove(&session_id) {
        let _ = std::fs::remove_dir_all(&session.dir);
    }
    Ok(())
}
