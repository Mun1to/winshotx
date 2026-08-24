use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use parking_lot::{Mutex, RwLock};

use crate::capture::{Freeze, Rect};
use crate::error::Result;
use crate::record::SessionData;
use crate::settings::Settings;
use crate::windows_mgr::OverlayIntent;
use tauri_plugin_global_shortcut::Shortcut;

/// Todo lo que hay que compartir con el hilo de captura mientras se graba.
pub struct RecordingState {
    #[allow(dead_code)]
    pub session_id: String,
    #[allow(dead_code)]
    pub region: Rect,
    pub started: Instant,
    pub stop: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
    pub paused_ms: Arc<AtomicU64>,
    pub pause_started: Mutex<Option<Instant>>,
    pub frames: Arc<AtomicU64>,
    pub bytes: Arc<AtomicU64>,
    #[allow(dead_code)]
    pub cancelled: Arc<AtomicBool>,
    #[cfg(windows)]
    pub control: Option<crate::record::win::Control>,
    pub writer: Option<JoinHandle<Result<SessionData>>>,
}

impl RecordingState {
    pub fn elapsed_ms(&self) -> u64 {
        (self.started.elapsed().as_millis() as u64)
            .saturating_sub(self.paused_ms.load(std::sync::atomic::Ordering::Relaxed))
    }
}

pub struct AppState {
    pub settings: RwLock<Settings>,
    /// Pantallas congeladas de la captura en curso, una por monitor.
    pub freezes: RwLock<Vec<Freeze>>,
    pub sessions: RwLock<HashMap<String, SessionData>>,
    pub shortcuts: RwLock<crate::hotkeys::ShortcutStatus>,
    /// Los atajos que hemos pedido al sistema, para poder soltarlos uno a uno.
    /// Sin esta lista hay que fiarse de `unregister_all`, y no se puede: ver hotkeys.rs.
    pub registered: RwLock<Vec<Shortcut>>,
    /// Con que se ha abierto el overlay que hay en pantalla. El mismo overlay sirve
    /// para capturar y para grabar, y sin esto no podria saber si al soltar el raton
    /// toca copiar la imagen o empezar a grabar.
    pub intent: RwLock<OverlayIntent>,
    pub recording: Mutex<Option<RecordingState>>,
    pub temp_root: PathBuf,
}

impl AppState {
    pub fn new(settings: Settings, temp_root: PathBuf) -> Self {
        Self {
            settings: RwLock::new(settings),
            freezes: RwLock::new(Vec::new()),
            sessions: RwLock::new(HashMap::new()),
            shortcuts: RwLock::new(crate::hotkeys::ShortcutStatus::default()),
            registered: RwLock::new(Vec::new()),
            intent: RwLock::new(OverlayIntent::Capture),
            recording: Mutex::new(None),
            temp_root,
        }
    }

    pub fn freeze_dir(&self) -> PathBuf {
        self.temp_root.join("freeze")
    }

    pub fn session_dir(&self, id: &str) -> PathBuf {
        self.temp_root.join("sessions").join(id)
    }

    pub fn is_recording(&self) -> bool {
        self.recording.lock().is_some()
    }
}
