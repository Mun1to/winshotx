use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::ExtendedColorType;
use image::ImageEncoder;
use image::codecs::bmp::BmpEncoder;
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

/// Captura todos los monitores de una vez y deja los BMP en disco.
/// Congelar la imagen evita pelearse con ventanas transparentes y da lupa exacta.
///
/// Capturar es estrictamente secuencial: GDI (lo que usa `xcap` en Windows) no tolera
/// bien que varios hilos capturen pantalla a la vez, y paralelizarlo fue justo lo que
/// colgo la app la primera vez que se intento (ver la memoria del proyecto). Pero una vez
/// que cada imagen ya esta en memoria, guardarla a disco no comparte nada entre monitores
/// (cada uno escribe su propio archivo), asi que eso si se reparte en hilos: mientras se
/// captura el monitor 2, el monitor 1 ya se puede estar escribiendo en paralelo.
pub fn freeze_all(dir: &Path) -> Result<Vec<Freeze>> {
    std::fs::create_dir_all(dir)?;
    let monitors = xcap::Monitor::all().map_err(|e| AppError::Msg(e.to_string()))?;

    let mut capturas = Vec::with_capacity(monitors.len());
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
        capturas.push((index, info, image));
    }

    if capturas.is_empty() {
        return Err(AppError::Msg("no se ha detectado ningún monitor".into()));
    }

    std::thread::scope(|scope| -> Result<Vec<Freeze>> {
        let handles: Vec<_> = capturas
            .into_iter()
            .map(|(index, info, image)| {
                let dir = dir.to_path_buf();
                scope.spawn(move || -> Result<Freeze> {
                    // BMP en vez de PNG: sin compresion, casi sin coste de CPU al escribir
                    // ni al decodificar en el navegador (PNG comprimido, aunque "rapido",
                    // seguia costando varios cientos de ms por monitor entre escribirlo y
                    // luego decodificarlo en cada ventana). El navegador lo entiende nativo
                    // en <img>/createImageBitmap. El canal alfa de una captura de escritorio
                    // siempre es opaco, asi que da igual si algun decodificador lo ignora.
                    let path = dir.join(format!("freeze-{index}.bmp"));
                    let mut writer = BufWriter::new(std::fs::File::create(&path)?);
                    BmpEncoder::new(&mut writer).write_image(
                        image.as_raw(),
                        image.width(),
                        image.height(),
                        ExtendedColorType::Rgba8,
                    )?;
                    Ok(Freeze {
                        monitor: info,
                        path,
                    })
                })
            })
            .collect();

        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| AppError::Msg("un hilo de guardado ha fallado".into()))?
            })
            .collect()
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Escribe una pantalla congelada de un solo color, como las que deja `freeze_all`.
    /// El color es la firma: si el recorte sale de otra pantalla, se ve en el pixel.
    fn pantalla(dir: &Path, id: u32, x: i32, color: [u8; 3]) -> Freeze {
        let (ancho, alto) = (1920u32, 1080u32);
        let imagen = image::RgbaImage::from_pixel(
            ancho,
            alto,
            image::Rgba([color[0], color[1], color[2], 255]),
        );
        let path = dir.join(format!("freeze-{id}.bmp"));
        imagen.save(&path).expect("no se ha podido escribir la pantalla de prueba");
        Freeze {
            monitor: MonitorInfo {
                id,
                label: format!("PRUEBA-{id}"),
                x,
                y: 0,
                width: ancho,
                height: alto,
                scale: 1.0,
                is_primary: id == 0,
            },
            path,
        }
    }

    /// Tres monitores de 1920 en fila, como los de Munir: rojo, verde y azul.
    fn tres_pantallas(nombre: &str) -> (PathBuf, Vec<Freeze>) {
        let dir = std::env::temp_dir().join(format!("winshotx-test-{nombre}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("no se ha podido crear el directorio de prueba");
        let freezes = vec![
            pantalla(&dir, 0, 0, [255, 0, 0]),
            pantalla(&dir, 1, 1920, [0, 255, 0]),
            pantalla(&dir, 2, 3840, [0, 0, 255]),
        ];
        (dir, freezes)
    }

    fn primer_pixel(imagen: &image::RgbaImage) -> [u8; 3] {
        let p = imagen.get_pixel(0, 0).0;
        [p[0], p[1], p[2]]
    }

    #[test]
    fn recorta_de_la_pantalla_donde_cae_la_seleccion() {
        let (_dir, freezes) = tres_pantallas("donde-cae");

        // Coordenadas del escritorio virtual, que es lo que manda `toPhysical`.
        let en_la_primera = crop_from_freeze(&freezes, Rect { x: 10, y: 10, width: 100, height: 100 }).unwrap();
        assert_eq!(primer_pixel(&en_la_primera), [255, 0, 0], "la primera pantalla es roja");

        let en_la_segunda = crop_from_freeze(&freezes, Rect { x: 1930, y: 10, width: 100, height: 100 }).unwrap();
        assert_eq!(primer_pixel(&en_la_segunda), [0, 255, 0], "la segunda pantalla es verde");

        let en_la_tercera = crop_from_freeze(&freezes, Rect { x: 3850, y: 10, width: 100, height: 100 }).unwrap();
        assert_eq!(primer_pixel(&en_la_tercera), [0, 0, 255], "la tercera pantalla es azul");
    }

    /// EL BUG. Una seleccion que no cae en ninguna pantalla no puede devolver la primera
    /// en silencio: eso es exactamente lo que se ve como "mi pantalla principal duplicada
    /// en las otras", porque cualquier error de coordenadas rio arriba acaba aqui.
    #[test]
    fn una_seleccion_fuera_de_todo_falla_en_vez_de_devolver_la_principal() {
        let (_dir, freezes) = tres_pantallas("fuera-de-todo");

        let resultado = crop_from_freeze(
            &freezes,
            Rect { x: 10_000, y: 10_000, width: 100, height: 100 },
        );

        assert!(
            resultado.is_err(),
            "una seleccion fuera de toda pantalla devolvio una imagen en vez de fallar; \
             si el pixel es rojo, ha caido en la pantalla principal: {:?}",
            resultado.map(|i| primer_pixel(&i))
        );
    }
}
