//! TEMPORAL (S29): la captura y el cambio de color de la 0.4.8, traidos de git
//! (`git show 8962a32:src-tauri/src/record/win.rs`) para medir lo de antes y lo de ahora en
//! la misma maquina y con la misma escena. Se borra al terminar la medida.
#![cfg(all(test, windows))]

use std::sync::atomic::Ordering;
use std::time::Instant;

use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use super::win::{CaptureFlags, CapturedFrame};

pub fn bgra_a_rgba_en_sitio(pixeles: &mut [u8]) {
    for pixel in pixeles.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
}

pub struct CapturaVieja {
    flags: CaptureFlags,
    last_emit: Option<Instant>,
    scratch: Vec<u8>,
}

impl GraphicsCaptureApiHandler for CapturaVieja {
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
        let now = Instant::now();
        if let Some(last) = self.last_emit {
            if now.duration_since(last).as_millis() < self.flags.min_interval_ms as u128 {
                return Ok(());
            }
        }
        self.last_emit = Some(now);

        let (x1, y1, x2, y2) = self.flags.crop;
        let (ancho_real, alto_real) = (frame.width(), frame.height());
        let (hasta_x, hasta_y) = (x2.min(ancho_real), y2.min(alto_real));
        if x1 >= hasta_x || y1 >= hasta_y {
            return Ok(());
        }
        let buffer = frame.buffer_crop(x1, y1, hasta_x, hasta_y)?;
        let mut data = self.flags.reciclados.coger();
        let pedido = ((x2 - x1) as usize, (y2 - y1) as usize);
        let hay = ((hasta_x - x1) as usize, (hasta_y - y1) as usize);
        let sin_relleno = buffer.as_nopadding_buffer(&mut self.scratch);
        if hay == pedido {
            data.extend_from_slice(sin_relleno);
        } else {
            data.resize(pedido.0 * pedido.1 * 4, 0);
            for fila in 0..hay.1 {
                let desde = fila * hay.0 * 4;
                let hasta = desde + hay.0 * 4;
                let destino = fila * pedido.0 * 4;
                data[destino..destino + hay.0 * 4].copy_from_slice(&sin_relleno[desde..hasta]);
            }
        }

        let elapsed = now.duration_since(self.flags.start).as_millis() as u64;
        let ts_ms = elapsed.saturating_sub(self.flags.paused_ms.load(Ordering::Relaxed));
        let _ = self.flags.sender.send(CapturedFrame {
            bgra: data,
            ts_ms,
            zonas: None,
        });
        Ok(())
    }

    fn on_closed(&mut self) -> std::result::Result<(), Self::Error> {
        self.flags.stop.store(true, Ordering::Relaxed);
        Ok(())
    }
}

pub type Control =
    windows_capture::capture::CaptureControl<CapturaVieja, Box<dyn std::error::Error + Send + Sync>>;

/// Como `win::start` de la 0.4.8 (monitor).
pub fn start(region: crate::capture::Rect, monitor_origin: (i32, i32), capture_cursor: bool, fps: u32, flags_base: CaptureFlags) -> Control {
    let (cx, cy) = region.center();
    let monitor = super::win::monitor_at(cx, cy).expect("sin monitor");
    let x1 = (region.x - monitor_origin.0).max(0) as u32;
    let y1 = (region.y - monitor_origin.1).max(0) as u32;
    let flags = CaptureFlags {
        crop: (x1, y1, x1 + region.width, y1 + region.height),
        min_interval_ms: (1000 / fps.max(1)).saturating_sub(1) as u64,
        ..flags_base
    };
    let settings = Settings::new(
        monitor,
        if capture_cursor { CursorCaptureSettings::WithCursor } else { CursorCaptureSettings::WithoutCursor },
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    );
    CapturaVieja::start_free_threaded(settings).expect("no arranca la captura vieja")
}

/// Como `win::start_window` de la 0.4.8, para una ventana de `ancho` x `alto`.
pub fn start_window(hwnd: isize, ancho: u32, alto: u32, capture_cursor: bool, fps: u32, flags_base: CaptureFlags) -> Control {
    let flags = CaptureFlags {
        crop: (0, 0, ancho, alto),
        min_interval_ms: (1000 / fps.max(1)).saturating_sub(1) as u64,
        ..flags_base
    };
    let settings = Settings::new(
        windows_capture::window::Window::from_raw_hwnd(hwnd as *mut core::ffi::c_void),
        if capture_cursor { CursorCaptureSettings::WithCursor } else { CursorCaptureSettings::WithoutCursor },
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    );
    CapturaVieja::start_free_threaded(settings).expect("no arranca la captura vieja")
}
