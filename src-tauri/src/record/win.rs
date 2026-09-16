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
    /// El cero del reloj de los fotogramas. Lo crea quien arranca la grabacion y lo
    /// comparte con el hilo que muestrea el raton: asi el rastro y la imagen van en el
    /// mismo tiempo. Antes cada uno tenia su `Instant::now()` y se separaban unos
    /// milisegundos que no se podian corregir.
    pub start: Instant,
}

pub struct RegionCapture {
    flags: CaptureFlags,
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

        let elapsed = now.duration_since(self.flags.start).as_millis() as u64;
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc::channel;
    use std::time::Duration;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::CreateSolidBrush;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, ShowWindow,
        SW_SHOWNOACTIVATE, WNDCLASSW,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    const CLASE: &[u16] = &[
        b'w' as u16, b's' as u16, b'x' as u16, b'-' as u16, b'p' as u16, b'r' as u16,
        b'u' as u16, b'e' as u16, b'b' as u16, b'a' as u16, 0,
    ];

    /// Lo unico que hace esta ventana es dejarse pintar del color de su clase.
    unsafe extern "system" fn procedimiento(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }

    /// Un rectangulo magenta, encima de todo y que no se puede pulsar, en la esquina de la
    /// pantalla principal. El magenta no sale en ningun escritorio por accidente.
    fn ventana_magenta(region: Rect) -> isize {
        let instancia = unsafe { GetModuleHandleW(None) }.expect("sin instancia");
        let clase = WNDCLASSW {
            hInstance: instancia.into(),
            lpszClassName: PCWSTR(CLASE.as_ptr()),
            lpfnWndProc: Some(procedimiento),
            hbrBackground: unsafe { CreateSolidBrush(COLORREF(0x00FF_00FF)) },
            ..Default::default()
        };
        let _ = unsafe { RegisterClassW(&clase) };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                PCWSTR(CLASE.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                region.x,
                region.y,
                region.width as i32,
                region.height as i32,
                None,
                None,
                Some(instancia.into()),
                None,
            )
        }
        .expect("no se ha podido abrir la ventana de prueba");
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        hwnd.0 as isize
    }

    /// Graba la region medio segundo y dice si el ultimo fotograma lleva magenta.
    fn sale_el_magenta(region: Rect) -> bool {
        let (sender, receiver) = channel::<CapturedFrame>();
        let stop = Arc::new(AtomicBool::new(false));
        let control = start(
            region,
            (0, 0),
            false,
            30,
            CaptureFlags {
                sender,
                crop: (0, 0, 0, 0),
                stop: stop.clone(),
                pause: Arc::new(AtomicBool::new(false)),
                paused_ms: Arc::new(AtomicU64::new(0)),
                min_interval_ms: 0,
                start: Instant::now(),
            },
        )
        .expect("no se ha podido arrancar la captura");

        // El ULTIMO de medio segundo, no el primero: los primeros fotogramas pueden venir
        // de antes de que el compositor haya pintado el cambio.
        let hasta = Instant::now() + Duration::from_millis(600);
        let mut ultimo = None;
        while Instant::now() < hasta {
            match receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) => ultimo = Some(frame),
                Err(_) => break,
            }
        }
        stop.store(true, Ordering::Relaxed);
        let _ = control.stop();

        let frame = ultimo.expect("la captura no ha dado ni un fotograma");
        // BGRA: magenta es azul al maximo, verde a cero y rojo al maximo.
        frame
            .bgra
            .chunks_exact(4)
            .any(|p| p[0] > 200 && p[1] < 60 && p[2] > 200)
    }

    /// La prueba de que el arreglo sirve de algo: una ventana marcada como fuera de la
    /// captura NO sale en lo que graba winshotx, y la misma ventana sin marcar SI sale.
    ///
    /// Esta ignorada porque abre una ventana en la pantalla de quien la corra y graba
    /// medio segundo de esa esquina: no es para una maquina de integracion, es para
    /// comprobar a mano que Windows sigue cumpliendo.
    ///
    ///     cargo test --lib una_ventana_excluida -- --ignored --nocapture
    #[test]
    #[ignore = "abre una ventana en la pantalla y graba un trozo: se corre a mano"]
    fn una_ventana_excluida_no_sale_en_lo_que_se_graba() {
        let region = Rect { x: 0, y: 0, width: 160, height: 120 };
        let ventana = ventana_magenta(region);
        std::thread::sleep(Duration::from_millis(300));

        assert!(
            sale_el_magenta(region),
            "sin excluirla tiene que salir, o esta prueba no prueba nada"
        );

        assert!(
            crate::platform::window_style::fuera_de_la_captura_hwnd(ventana, true),
            "Windows no ha aceptado sacarla de la captura"
        );
        std::thread::sleep(Duration::from_millis(300));
        let sigue = sale_el_magenta(region);

        unsafe {
            let _ = DestroyWindow(HWND(ventana as *mut core::ffi::c_void));
        }
        assert!(!sigue, "la ventana excluida se sigue colando en la grabacion");
    }
}
