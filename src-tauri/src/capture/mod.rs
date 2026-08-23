use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

/// Rectangulo en pixeles fisicos del escritorio virtual.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn center(&self) -> (i32, i32) {
        (
            self.x + (self.width / 2) as i32,
            self.y + (self.height / 2) as i32,
        )
    }

    /// Los codecs de video exigen dimensiones pares.
    pub fn to_even(mut self) -> Self {
        self.width = (self.width / 2) * 2;
        self.height = (self.height / 2) * 2;
        self.width = self.width.max(2);
        self.height = self.height.max(2);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub id: u32,
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub is_primary: bool,
}

impl MonitorInfo {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x + self.width as i32
            && y < self.y + self.height as i32
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRect {
    pub title: String,
    pub rect: Rect,
}

/// Pantalla congelada de un monitor: es el fondo sobre el que se selecciona.
#[derive(Debug, Clone)]
pub struct Freeze {
    pub monitor: MonitorInfo,
    pub path: PathBuf,
}

/// Captura todos los monitores de una vez y deja los PNG en disco.
/// Congelar la imagen evita pelearse con ventanas transparentes y da lupa exacta.
pub fn freeze_all(dir: &Path) -> Result<Vec<Freeze>> {
    std::fs::create_dir_all(dir)?;
    let monitors = xcap::Monitor::all().map_err(|e| AppError::Msg(e.to_string()))?;
    let mut out = Vec::with_capacity(monitors.len());

    for (index, monitor) in monitors.iter().enumerate() {
        let info = MonitorInfo {
            id: index as u32,
            label: monitor.name().unwrap_or_else(|_| format!("Monitor {}", index + 1)),
            x: monitor.x().map_err(|e| AppError::Msg(e.to_string()))?,
            y: monitor.y().map_err(|e| AppError::Msg(e.to_string()))?,
            width: monitor.width().map_err(|e| AppError::Msg(e.to_string()))?,
            height: monitor.height().map_err(|e| AppError::Msg(e.to_string()))?,
            scale: monitor.scale_factor().unwrap_or(1.0),
            is_primary: monitor.is_primary().unwrap_or(false),
        };
        let image = monitor
            .capture_image()
            .map_err(|e| AppError::Msg(e.to_string()))?;
        let path = dir.join(format!("freeze-{index}.png"));
        image.save(&path)?;
        out.push(Freeze {
            monitor: info,
            path,
        });
    }

    if out.is_empty() {
        return Err(AppError::Msg("no se ha detectado ningún monitor".into()));
    }
    Ok(out)
}

/// Rectangulos de las ventanas visibles, para el ajuste automatico del overlay.
pub fn window_rects() -> Vec<WindowRect> {
    let Ok(windows) = xcap::Window::all() else {
        return Vec::new();
    };
    // Las ventanas del propio overlay tapan la pantalla entera y se enumeran como
    // cualquier otra: sin este filtro, el ajuste automatico se ofreceria a si mismo.
    let own_pid = std::process::id();
    windows
        .iter()
        .filter(|w| w.pid().map(|pid| pid != own_pid).unwrap_or(true))
        .filter(|w| !w.is_minimized().unwrap_or(true))
        .filter_map(|w| {
            let title = w.title().unwrap_or_default();
            let width = w.width().ok()?;
            let height = w.height().ok()?;
            if width < 40 || height < 40 {
                return None;
            }
            Some(WindowRect {
                title,
                rect: Rect {
                    x: w.x().ok()?,
                    y: w.y().ok()?,
                    width,
                    height,
                },
            })
        })
        .collect()
}

/// Recorta la region pedida del monitor que la contiene, usando la imagen ya congelada.
pub fn crop_from_freeze(freezes: &[Freeze], region: Rect) -> Result<image::RgbaImage> {
    let (cx, cy) = region.center();
    let freeze = freezes
        .iter()
        .find(|f| f.monitor.contains(cx, cy))
        .or_else(|| freezes.first())
        .ok_or_else(|| AppError::Msg("no hay pantalla congelada".into()))?;

    let image = image::open(&freeze.path)?.to_rgba8();
    let local_x = (region.x - freeze.monitor.x).max(0) as u32;
    let local_y = (region.y - freeze.monitor.y).max(0) as u32;
    let width = region.width.min(image.width().saturating_sub(local_x));
    let height = region.height.min(image.height().saturating_sub(local_y));
    if width == 0 || height == 0 {
        return Err(AppError::Msg("la selección queda fuera de la pantalla".into()));
    }
    Ok(image::imageops::crop_imm(&image, local_x, local_y, width, height).to_image())
}
