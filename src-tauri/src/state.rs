use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    pub started: Instant,
    pub stop: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
    pub paused_ms: Arc<AtomicU64>,
    pub pause_started: Mutex<Option<Instant>>,
    pub frames: Arc<AtomicU64>,
    pub bytes: Arc<AtomicU64>,
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
    /// El anillo de los ultimos segundos, si esta encendido. Va aparte de `recording`
    /// porque puede convivir con una grabacion normal: son dos capturas distintas y quien
    /// esta grabando un tutorial no tiene por que perder lo que el anillo ya tenia.
    pub replay: Mutex<Option<crate::replay::ReplayState>>,
    /// La ultima region capturada, en coordenadas del escritorio virtual. Sobrevive al
    /// cierre del overlay a proposito: repetir una captura solo sirve si se acuerda de la
    /// vez anterior, que fue otro disparo del atajo.
    pub last_region: RwLock<Option<Rect>>,
    pub temp_root: PathBuf,
    /// Si hay una captura de pantalla en curso ahora mismo (congelando o guardando).
    /// Sin este candado, pulsar el atajo dos veces seguidas muy rapido lanzaba dos
    /// `freeze_all` a la vez, y la captura de pantalla (GDI) no tolera bien que dos
    /// disparos capturen al mismo tiempo: se cruzaban entre si y todo salia mas lento.
    ///
    /// Va en un `Arc` y no suelto porque con el temporizador la captura deja de caber en
    /// una sola llamada: el candado se coge al pulsar el atajo y no se suelta hasta que
    /// terminan los tres o cinco segundos, ya en otro hilo. Prestado no podria viajar.
    capturando: Arc<AtomicBool>,
    /// Si este arranque viene de una actualizacion. Se enciende una vez en el arranque y
    /// lo apaga la interfaz al leerlo: es una noticia que se da una sola vez, no un estado.
    recien_actualizado: AtomicBool,
}

/// Se suelta el candado de `capturando` solo al destruirse, asi que un `?` que sale a
/// medio camino de `open_overlays` no lo deja puesto para siempre.
pub struct CandadoCaptura(Arc<AtomicBool>);

impl Drop for CandadoCaptura {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl AppState {
    pub fn new(settings: Settings, temp_root: PathBuf, recien_actualizado: bool) -> Self {
        Self {
            recien_actualizado: AtomicBool::new(recien_actualizado),
            settings: RwLock::new(settings),
            freezes: RwLock::new(Vec::new()),
            sessions: RwLock::new(HashMap::new()),
            shortcuts: RwLock::new(crate::hotkeys::ShortcutStatus::default()),
            registered: RwLock::new(Vec::new()),
            intent: RwLock::new(OverlayIntent::Capture),
            recording: Mutex::new(None),
            replay: Mutex::new(None),
            last_region: RwLock::new(None),
            temp_root,
            capturando: Arc::new(AtomicBool::new(false)),
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

    /// `None` si ya habia una captura en curso: quien lo pide no debe seguir.
    pub fn intentar_capturar(&self) -> Option<CandadoCaptura> {
        self.capturando
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| CandadoCaptura(Arc::clone(&self.capturando)))
    }

    /// Cierto solo la primera vez que se pregunta, y solo si este arranque vino de una
    /// actualizacion. Se consume al leerlo para que reabrir los ajustes mas tarde no
    /// vuelva a anunciar algo que ya se conto.
    pub fn consumir_recien_actualizado(&self) -> bool {
        self.recien_actualizado.swap(false, Ordering::Relaxed)
    }
}
