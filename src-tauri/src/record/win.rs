use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Instant;

use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor as WcMonitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use crate::capture::Rect;
use crate::error::{AppError, Result};

/// windows-capture identifica monitores por HMONITOR, asi que se lo pedimos a Win32.
fn monitor_at(x: i32, y: i32) -> Result<WcMonitor> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};

    let handle = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    if handle.is_invalid() {
        return Err(AppError::Msg("no hay monitor en esa posición".into()));
    }
    Ok(WcMonitor::from_raw_hmonitor(handle.0))
}

/// Un fotograma recien capturado, ya recortado a la region y en BGRA.
pub struct CapturedFrame {
    pub bgra: Vec<u8>,
    pub ts_ms: u64,
}

#[derive(Clone)]
pub struct CaptureFlags {
    pub sender: Sender<CapturedFrame>,
    /// Recorte dentro del monitor: x1, y1, x2, y2.
    pub crop: (u32, u32, u32, u32),
    pub stop: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
    /// Milisegundos acumulados en pausa, para que el reloj no cuente ese tiempo.
    pub paused_ms: Arc<AtomicU64>,
    pub min_interval_ms: u64,
}

pub struct RegionCapture {
    flags: CaptureFlags,
    start: Instant,
    last_emit: Option<Instant>,
    /// El apoyo que necesita el crate cuando la textura viene con relleno de fila. Se
    /// guarda entre fotogramas para no reservar ocho megabytes nuevos sesenta veces por
    /// segundo: la copia que se manda por el canal sigue siendo una, esta no cuenta.
    scratch: Vec<u8>,
}

impl GraphicsCaptureApiHandler for RegionCapture {
    type Flags = CaptureFlags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            flags: ctx.flags,
            start: Instant::now(),
            last_emit: None,
            scratch: Vec::new(),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> std::result::Result<(), Self::Error> {
        if self.flags.stop.load(Ordering::Relaxed) {
            control.stop();
            return Ok(());
        }
        if self.flags.pause.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Windows entrega fotogramas al ritmo del compositor; aqui se recorta al fps pedido.
        let now = Instant::now();
        if let Some(last) = self.last_emit {
            if now.duration_since(last).as_millis() < self.flags.min_interval_ms as u128 {
                return Ok(());
            }
        }
        self.last_emit = Some(now);

        let (x1, y1, x2, y2) = self.flags.crop;
        let buffer = frame.buffer_crop(x1, y1, x2, y2)?;
        // El crate necesita un Vec de apoyo por si la textura viene con relleno de fila.
        let data = buffer.as_nopadding_buffer(&mut self.scratch).to_vec();

        let elapsed = now.duration_since(self.start).as_millis() as u64;
        let ts_ms = elapsed.saturating_sub(self.flags.paused_ms.load(Ordering::Relaxed));
        let _ = self.flags.sender.send(CapturedFrame { bgra: data, ts_ms });
        Ok(())
    }

    fn on_closed(&mut self) -> std::result::Result<(), Self::Error> {
        self.flags.stop.store(true, Ordering::Relaxed);
        Ok(())
    }
}

pub type Control =
    windows_capture::capture::CaptureControl<RegionCapture, Box<dyn std::error::Error + Send + Sync>>;

/// Arranca la captura de la region sobre el monitor que la contiene.
/// `monitor_origin` viene de la enumeracion de xcap, que si expone coordenadas.
pub fn start(
    region: Rect,
    monitor_origin: (i32, i32),
    capture_cursor: bool,
    fps: u32,
    flags_base: CaptureFlags,
) -> Result<Control> {
    let (cx, cy) = region.center();
    let monitor = monitor_at(cx, cy)?;

    let x1 = (region.x - monitor_origin.0).max(0) as u32;
    let y1 = (region.y - monitor_origin.1).max(0) as u32;
    let flags = CaptureFlags {
        crop: (x1, y1, x1 + region.width, y1 + region.height),
        min_interval_ms: (1000 / fps.max(1)).saturating_sub(1) as u64,
        ..flags_base
    };

    let settings = Settings::new(
        monitor,
        if capture_cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        },
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    );

    RegionCapture::start_free_threaded(settings).map_err(|e| AppError::Msg(e.to_string()))
}
