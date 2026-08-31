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
/// Como es un monitor: donde esta, cuanto mide y como se llama.
fn describir(index: usize, monitor: &xcap::Monitor) -> Result<MonitorInfo> {
    Ok(MonitorInfo {
        id: index as u32,
        label: monitor
            .name()
            .unwrap_or_else(|_| format!("Monitor {}", index + 1)),
        x: monitor.x().map_err(|e| AppError::Msg(e.to_string()))?,
        y: monitor.y().map_err(|e| AppError::Msg(e.to_string()))?,
        width: monitor.width().map_err(|e| AppError::Msg(e.to_string()))?,
        height: monitor.height().map_err(|e| AppError::Msg(e.to_string()))?,
        scale: monitor.scale_factor().unwrap_or(1.0),
        is_primary: monitor.is_primary().unwrap_or(false),
    })
}

/// Las pantallas que hay, sin fotografiarlas.
///
/// `freeze_all` tambien las enumera, pero de paso captura cada una, que es lo caro. Quien
/// solo quiere saber donde estan (el anillo de los ultimos segundos, para elegir cual
/// vigila) no tiene por que pagar eso.
pub fn monitors() -> Result<Vec<MonitorInfo>> {
    xcap::Monitor::all()
        .map_err(|e| AppError::Msg(e.to_string()))?
        .iter()
        .enumerate()
        .map(|(index, monitor)| describir(index, monitor))
        .collect()
}

pub fn freeze_all(dir: &Path) -> Result<Vec<Freeze>> {
    std::fs::create_dir_all(dir)?;
    let monitors = xcap::Monitor::all().map_err(|e| AppError::Msg(e.to_string()))?;

    // Las pantallas se fotografian A LA VEZ, una por hilo.
    //
    // Estaban en fila, y con una pantalla eso da igual, pero con tres se paga tres veces:
    // en la maquina de Munir (1920x1080 + 1080x1920 + 1536x960) congelarlas costaba 150 ms
    // de los 270 que tarda el atajo entero, y el numero que la app anuncia y defiende son
    // 28 ms desde el atajo hasta ver la seleccion. Escribir los archivos ya se hacia en
    // paralelo aqui abajo; fotografiar, no.
    // Cada hilo se busca SU monitor en vez de recibirlo: `xcap::Monitor` lleva un puntero
    // crudo dentro (el identificador que da Windows), asi que no se puede mandar de un hilo
    // a otro. Volver a enumerar cuesta microsegundos, que al lado de fotografiar una
    // pantalla no es nada, y evita tener que prometerle al compilador algo que no se puede
    // comprobar sobre un tipo de otra biblioteca.
    let cuantos = monitors.len();
    let capturas = std::thread::scope(|scope| -> Result<Vec<_>> {
        let handles: Vec<_> = (0..cuantos)
            .map(|index| {
                scope.spawn(move || -> Result<(usize, MonitorInfo, image::RgbaImage)> {
                    let suyos = xcap::Monitor::all().map_err(|e| AppError::Msg(e.to_string()))?;
                    let monitor = suyos
                        .get(index)
                        .ok_or_else(|| AppError::Msg("una pantalla ha desaparecido".into()))?;
                    let info = describir(index, monitor)?;
                    let image = monitor
                        .capture_image()
                        .map_err(|e| AppError::Msg(e.to_string()))?;
                    Ok((index, info, image))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .map_err(|_| AppError::Msg("una pantalla ha fallado al congelarse".into()))?
            })
            .collect()
    })?;

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

/// El rectangulo que contiene todas las pantallas: el escritorio virtual entero.
/// Con monitores de distinto alto o mal alineados sobra sitio por algun lado, y ese
/// sobrante no pertenece a ninguna pantalla: ver `stitch_all`.
pub fn virtual_desktop(freezes: &[Freeze]) -> Option<Rect> {
    let primero = freezes.first()?;
    let (mut x0, mut y0) = (primero.monitor.x, primero.monitor.y);
    let (mut x1, mut y1) = (
        primero.monitor.x + primero.monitor.width as i32,
        primero.monitor.y + primero.monitor.height as i32,
    );
    for f in freezes.iter().skip(1) {
        x0 = x0.min(f.monitor.x);
        y0 = y0.min(f.monitor.y);
        x1 = x1.max(f.monitor.x + f.monitor.width as i32);
        y1 = y1.max(f.monitor.y + f.monitor.height as i32);
    }
    Some(Rect {
        x: x0,
        y: y0,
        width: (x1 - x0) as u32,
        height: (y1 - y0) as u32,
    })
}

/// Pega todas las pantallas congeladas en una sola imagen, cada una en su sitio real.
///
/// Los huecos que quedan entre monitores desalineados se dejan **transparentes**, no en
/// negro (decision D7). Conviene saber que pasa despues con esa transparencia, porque no
/// es igual en todas las salidas: el PNG guardado la conserva; el portapapeles la compone
/// sobre blanco, porque `platform::clipboard` construye el CF_DIB sobre blanco ya que
/// muchas aplicaciones ignoran el alfa; y el GIF y el MP4 no tienen alfa, asi que ahi los
/// huecos salen negros. No hay nada que arreglar en eso, solo que no sorprenda.
pub fn stitch_all(freezes: &[Freeze]) -> Result<(image::RgbaImage, Rect)> {
    let marco = virtual_desktop(freezes)
        .ok_or_else(|| AppError::Msg("no hay ninguna pantalla congelada".into()))?;

    let mut lienzo = image::RgbaImage::from_pixel(marco.width, marco.height, image::Rgba([0, 0, 0, 0]));

    for f in freezes {
        let trozo = image::open(&f.path)?.to_rgba8();
        let dx = f.monitor.x - marco.x;
        let dy = f.monitor.y - marco.y;
        image::imageops::overlay(&mut lienzo, &trozo, dx as i64, dy as i64);
    }

    Ok((lienzo, marco))
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
    /// Un monitor mas bajo que los otros deja un hueco debajo que no es de nadie.
    fn tres_pantallas_desalineadas(nombre: &str) -> (PathBuf, Vec<Freeze>) {
        let dir = std::env::temp_dir().join(format!("winshotx-test-{nombre}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("no se ha podido crear el directorio de prueba");
        let mut freezes = vec![
            pantalla(&dir, 0, 0, [255, 0, 0]),
            pantalla(&dir, 1, 1920, [0, 255, 0]),
        ];
        // La tercera es de 1920x720 en vez de 1920x1080: deja 360 px de hueco debajo.
        let bajita = image::RgbaImage::from_pixel(1920, 720, image::Rgba([0, 0, 255, 255]));
        let path = dir.join("freeze-2.bmp");
        bajita.save(&path).expect("no se ha podido escribir la pantalla baja");
        freezes.push(Freeze {
            monitor: MonitorInfo {
                id: 2,
                label: "PRUEBA-2".into(),
                x: 3840,
                y: 0,
                width: 1920,
                height: 720,
                scale: 1.0,
                is_primary: false,
            },
            path,
        });
        (dir, freezes)
    }

    #[test]
    fn el_escritorio_virtual_abarca_todas_las_pantallas() {
        let (_dir, freezes) = tres_pantallas("marco");
        let marco = virtual_desktop(&freezes).expect("hay tres pantallas");
        assert_eq!(marco.x, 0);
        assert_eq!(marco.y, 0);
        assert_eq!(marco.width, 5760, "tres monitores de 1920 en fila");
        assert_eq!(marco.height, 1080);
    }

    #[test]
    fn juntar_las_pantallas_deja_cada_una_en_su_sitio() {
        let (_dir, freezes) = tres_pantallas("juntar");
        let (lienzo, marco) = stitch_all(&freezes).unwrap();

        assert_eq!((lienzo.width(), lienzo.height()), (5760, 1080));
        assert_eq!(marco.width, 5760);

        // Un pixel del centro de cada tercio tiene que ser del color de esa pantalla.
        let color = |x: u32| { let p = lienzo.get_pixel(x, 540).0; [p[0], p[1], p[2], p[3]] };
        assert_eq!(color(960), [255, 0, 0, 255], "el primer tercio es la pantalla roja");
        assert_eq!(color(2880), [0, 255, 0, 255], "el segundo tercio es la verde");
        assert_eq!(color(4800), [0, 0, 255, 255], "el tercero es la azul");
    }

    /// D7: los huecos van transparentes, no negros. Si alguien lo cambia, que falle aqui
    /// y no en el portapapeles de alguien.
    #[test]
    fn el_hueco_entre_monitores_desalineados_queda_transparente() {
        let (_dir, freezes) = tres_pantallas_desalineadas("hueco");
        let (lienzo, _) = stitch_all(&freezes).unwrap();

        assert_eq!((lienzo.width(), lienzo.height()), (5760, 1080));

        // Dentro de la tercera pantalla, que solo llega hasta y=720.
        assert_eq!(lienzo.get_pixel(4800, 360).0[3], 255, "dentro de la pantalla, opaco");
        // Debajo de ella no hay pantalla ninguna.
        assert_eq!(lienzo.get_pixel(4800, 900).0[3], 0, "el hueco tiene que ser transparente");
    }

}
