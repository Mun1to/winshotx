//! Los fotogramas en X11: se le pide al servidor la imagen de la region, por memoria
//! compartida (MIT-SHM), tantas veces por segundo como fotogramas se quieran.
//!
//! `xcap`, que es lo que congela la pantalla para una foto, pide la imagen por el socket y la
//! convierte pixel a pixel: para una foto vale, para treinta por segundo no. Con MIT-SHM el
//! servidor escribe la imagen en un trozo de memoria que comparte con nosotros y no viaja
//! nada. Si el servidor no lo tiene (una pantalla remota), se pide por el socket, mas lento.
//!
//! X11 no avisa de cuando cambia la pantalla, asi que se mira a ritmo fijo y lo que sale
//! igual que la vez anterior no se manda: una pantalla quieta no llena el disco ni hace
//! trabajar a nadie.
//!
//! El raton no sale en esa imagen. Se pregunta aparte, sesenta veces por segundo, con XFixes,
//! que da donde esta y que dibujo tiene.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::shm::{ConnectionExt as _, Seg};
use x11rb::protocol::xfixes::ConnectionExt as _;
use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat, ImageOrder, Window};
use x11rb::rust_connection::RustConnection;

use super::{bgrx_a_rgba, puntero_de_argb, EnMarcha, Fotograma, Marcha, Puntero, Rastro, Reciclados};
use crate::capture::Rect;
use crate::error::{AppError, Result};

fn fallo(que: &str, error: impl std::fmt::Display) -> AppError {
    AppError::Msg(format!("{que}: {error}"))
}

/// Un trozo de memoria compartido con el servidor X, donde escribe cada imagen.
struct Segmento {
    id: Seg,
    direccion: *mut u8,
    largo: usize,
}

// SAFETY: la direccion es de un segmento de memoria que es nuestro hasta el `Drop`, y solo
// lo usa el hilo que captura.
unsafe impl Send for Segmento {}

impl Segmento {
    fn crear(conexion: &RustConnection, largo: usize) -> Result<Self> {
        // SAFETY: llamadas de System V sin punteros nuestros. El segmento se marca para borrar
        // en cuanto esta pegado en los dos lados, asi que no queda huerfano si el proceso muere.
        unsafe {
            let shmid = libc::shmget(libc::IPC_PRIVATE, largo, libc::IPC_CREAT | 0o600);
            if shmid < 0 {
                return Err(fallo("no se ha podido reservar la memoria compartida", std::io::Error::last_os_error()));
            }
            let direccion = libc::shmat(shmid, std::ptr::null(), 0);
            if direccion as isize == -1 {
                let error = std::io::Error::last_os_error();
                libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut());
                return Err(fallo("no se ha podido abrir la memoria compartida", error));
            }
            let pegado = conexion
                .generate_id()
                .map_err(|e| fallo("el servidor X no da identificadores", e))
                .and_then(|id| {
                    conexion
                        .shm_attach(id, shmid as u32, false)
                        .map_err(|e| fallo("el servidor X no coge la memoria compartida", e))?
                        .check()
                        .map_err(|e| fallo("el servidor X no coge la memoria compartida", e))?;
                    Ok(id)
                });
            libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut());
            match pegado {
                Ok(id) => Ok(Self { id, direccion: direccion.cast(), largo }),
                Err(error) => {
                    libc::shmdt(direccion);
                    Err(error)
                }
            }
        }
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: `direccion` apunta a `largo` bytes vivos hasta el `Drop`.
        unsafe { std::slice::from_raw_parts(self.direccion, self.largo) }
    }
}

impl Drop for Segmento {
    fn drop(&mut self) {
        // SAFETY: es la direccion que devolvio `shmat`.
        unsafe { libc::shmdt(self.direccion.cast()) };
    }
}

/// La parte de la region que cae dentro de una pantalla de ese tamanno, con medidas pares:
/// pedirle al servidor un pixel de fuera es un error (`BadMatch`), no una imagen mas pequenna.
pub(crate) fn dentro_de_la_pantalla(region: Rect, ancho: u32, alto: u32) -> Option<Rect> {
    let dentro = lo_que_cae_dentro(region, ancho, alto)?;
    Some(Rect { width: dentro.width & !1, height: dentro.height & !1, ..dentro })
}

/// Lo mismo sin redondear a pares, que es cosa del video: una foto mide lo que mida.
fn lo_que_cae_dentro(region: Rect, ancho: u32, alto: u32) -> Option<Rect> {
    let x0 = region.x.max(0);
    let y0 = region.y.max(0);
    let x1 = (region.x + region.width as i32).min(ancho as i32);
    let y1 = (region.y + region.height as i32).min(alto as i32);
    if x1 - x0 < 2 || y1 - y0 < 2 {
        return None;
    }
    Some(Rect { x: x0, y: y0, width: (x1 - x0) as u32, height: (y1 - y0) as u32 })
}

pub struct Preparada {
    conexion: RustConnection,
    raiz: Window,
    region: Rect,
    fps: u32,
    /// `None` si el servidor no tiene MIT-SHM: se pide por el socket.
    segmento: Option<Segmento>,
}

pub fn preparar(region: Rect, fps: u32) -> Result<Preparada> {
    preparar_con(region, fps, true)
}

/// Lo mismo para fotos sueltas (`Preparada::foto`), que es lo que pide la captura con scroll:
/// la zona tal cual, sin redondearla a medidas pares.
pub fn preparar_fotos(region: Rect) -> Result<Preparada> {
    preparar_con(region, 1, false)
}

fn preparar_con(region: Rect, fps: u32, pares: bool) -> Result<Preparada> {
    let (conexion, numero) = RustConnection::connect(None).map_err(|e| fallo("no se puede hablar con el servidor X", e))?;
    let instalacion = conexion.setup();
    let pantalla = instalacion
        .roots
        .get(numero)
        .ok_or_else(|| AppError::Msg("el servidor X no tiene esa pantalla".into()))?;
    let raiz = pantalla.root;
    // Lo que se lee son cuatro bytes por pixel en el orden azul, verde, rojo: es lo que dan
    // todas las pantallas de 24 bits de un PC. Otra cosa (10 bits por color, 16 bits) saldria
    // con los colores cambiados, y es mejor decirlo que grabarlo mal.
    let visual = pantalla
        .allowed_depths
        .iter()
        .flat_map(|d| d.visuals.iter())
        .find(|v| v.visual_id == pantalla.root_visual);
    let bits = instalacion
        .pixmap_formats
        .iter()
        .find(|f| f.depth == pantalla.root_depth)
        .map(|f| f.bits_per_pixel);
    let conocido = visual.is_some_and(|v| v.red_mask == 0x00FF_0000 && v.green_mask == 0x0000_FF00 && v.blue_mask == 0x0000_00FF)
        && bits == Some(32)
        && instalacion.image_byte_order == ImageOrder::LSB_FIRST;
    if !conocido {
        return Err(AppError::Msg(format!(
            "esta pantalla usa un formato de color que winshotx todavía no sabe grabar ({} bits)",
            pantalla.root_depth
        )));
    }
    let (ancho, alto) = (u32::from(pantalla.width_in_pixels), u32::from(pantalla.height_in_pixels));
    let region = if pares { dentro_de_la_pantalla(region, ancho, alto) } else { lo_que_cae_dentro(region, ancho, alto) }
        .ok_or_else(|| AppError::Msg("la zona que se quiere grabar no está en la pantalla".into()))?;

    let largo = region.width as usize * region.height as usize * 4;
    let con_shm = conexion.shm_query_version().ok().and_then(|c| c.reply().ok()).is_some();
    let segmento = if con_shm {
        match Segmento::crear(&conexion, largo) {
            Ok(segmento) => Some(segmento),
            Err(error) => {
                eprintln!("[grabar] sin memoria compartida con el servidor X ({error}): se pide por el socket");
                None
            }
        }
    } else {
        eprintln!("[grabar] el servidor X no tiene MIT-SHM: se pide por el socket");
        None
    };
    Ok(Preparada { conexion, raiz, region, fps, segmento })
}

impl Preparada {
    pub fn region(&self) -> Rect {
        self.region
    }

    /// Una foto de la zona ahora mismo, ya en RGBA. El raton no sale en ella.
    pub fn foto(&self) -> Result<image::RgbaImage> {
        const QUE: &str = "el servidor X no ha dado la imagen";
        let r = self.region;
        let (x, y, ancho, alto) = (r.x as i16, r.y as i16, r.width as u16, r.height as u16);
        let largo = r.width as usize * r.height as usize * 4;
        let mut rgba = vec![0u8; largo];
        match self.segmento.as_ref() {
            Some(s) => {
                self.conexion
                    .shm_get_image(self.raiz, x, y, ancho, alto, !0, ImageFormat::Z_PIXMAP.into(), s.id, 0)
                    .map_err(|e| fallo(QUE, e))?
                    .reply()
                    .map_err(|e| fallo(QUE, e))?;
                bgrx_a_rgba(&s.bytes()[..largo], &mut rgba);
            }
            None => {
                let respuesta = self
                    .conexion
                    .get_image(ImageFormat::Z_PIXMAP, self.raiz, x, y, ancho, alto, !0)
                    .map_err(|e| fallo(QUE, e))?
                    .reply()
                    .map_err(|e| fallo(QUE, e))?;
                if respuesta.data.len() < largo {
                    return Err(AppError::Msg(format!("{QUE}: ha llegado más pequeña de lo pedido")));
                }
                bgrx_a_rgba(&respuesta.data[..largo], &mut rgba);
            }
        }
        image::RgbaImage::from_raw(r.width, r.height, rgba).ok_or_else(|| AppError::Msg(format!("{QUE}: no tiene las medidas pedidas")))
    }

    pub fn grabar(self, marcha: Marcha, salida: Sender<Fotograma>, reciclados: Reciclados) -> EnMarcha {
        let region = self.region;
        let del_raton = marcha.clone();
        let imagen = std::thread::spawn(move || capturar(self, marcha, salida, reciclados));
        let raton = std::thread::spawn(move || seguir_al_raton(region, del_raton));
        EnMarcha { imagen: Some(imagen), raton: Some(raton) }
    }
}

/// Lo que se puede estar sin mirar la pantalla antes de dejarlo dicho en el registro.
const HUECO_RARO: Duration = Duration::from_millis(250);

/// El hilo de la imagen: una foto de la region cada `1/fps`, y fuera solo las que cambian.
fn capturar(preparada: Preparada, marcha: Marcha, salida: Sender<Fotograma>, reciclados: Reciclados) {
    let Preparada { conexion, raiz, region, fps, segmento } = preparada;
    let paso = Duration::from_micros(1_000_000 / u64::from(fps.max(1)));
    let (x, y) = (region.x as i16, region.y as i16);
    let (ancho, alto) = (region.width as u16, region.height as u16);
    let largo = region.width as usize * region.height as usize * 4;
    let mut previo: Vec<u8> = Vec::new();
    let mut por_el_socket: Vec<u8> = Vec::new();
    let mut siguiente = Instant::now();
    let mut fallos_seguidos = 0u32;
    let mut ultima_mirada = Instant::now();
    while !marcha.parada() {
        let ahora = Instant::now();
        if ahora < siguiente {
            std::thread::sleep(siguiente - ahora);
        }
        // Sin recuperar de golpe lo perdido: si una foto tarda mas que el paso, la siguiente
        // va un paso despues de ahora, no pegada a esta.
        siguiente = (siguiente + paso).max(Instant::now());
        if marcha.en_pausa() || marcha.parada() {
            ultima_mirada = Instant::now();
            continue;
        }
        let pedida = Instant::now();
        let leido = match segmento.as_ref() {
            Some(s) => conexion
                .shm_get_image(raiz, x, y, ancho, alto, !0, ImageFormat::Z_PIXMAP.into(), s.id, 0)
                .map_err(|e| e.to_string())
                .and_then(|c| c.reply().map_err(|e| e.to_string()))
                .map(|_| ()),
            None => conexion
                .get_image(ImageFormat::Z_PIXMAP, raiz, x, y, ancho, alto, !0)
                .map_err(|e| e.to_string())
                .and_then(|c| c.reply().map_err(|e| e.to_string()))
                .map(|respuesta| por_el_socket = respuesta.data),
        };
        if let Err(error) = leido {
            // Una foto que falla (la pantalla ha cambiado de tamanno a mitad) se salta; si no
            // sale ninguna en un rato, no se va a arreglar sola.
            fallos_seguidos += 1;
            if fallos_seguidos == 1 {
                eprintln!("[grabar] el servidor X no ha dado la imagen: {error}");
            }
            if fallos_seguidos > fps.max(1) * 3 {
                eprintln!("[grabar] el servidor X lleva tres segundos sin dar imagen: se deja de pedir");
                break;
            }
            continue;
        }
        fallos_seguidos = 0;
        // Un rato largo sin mirar la pantalla es, en el video, una imagen que dura de mas. Se
        // deja dicho con lo que tardo el servidor, para saber si fue el o este hilo.
        let hueco = ultima_mirada.elapsed();
        if hueco > HUECO_RARO {
            eprintln!(
                "[grabar] {} ms sin mirar la pantalla (el servidor X tardó {} ms en dar la imagen)",
                hueco.as_millis(),
                pedida.elapsed().as_millis()
            );
        }
        ultima_mirada = Instant::now();
        let ts_ms = marcha.ms();
        let crudo: &[u8] = match segmento.as_ref() {
            Some(s) => s.bytes(),
            None => &por_el_socket,
        };
        if crudo.len() < largo || crudo[..largo] == previo[..] {
            continue;
        }
        previo.clear();
        previo.extend_from_slice(&crudo[..largo]);
        let mut rgba = reciclados.coger();
        rgba.resize(largo, 0);
        bgrx_a_rgba(&previo, &mut rgba);
        if salida.send(Fotograma { rgba, ts_ms }).is_err() {
            break;
        }
    }
}

/// Cada cuanto se mira el raton: lo mismo que en Windows (`anotador::CADA`).
const CADA: Duration = Duration::from_millis(16);

/// El hilo del raton: donde esta y que imagen lleva, con el reloj de la grabacion.
///
/// Con su propia conexion: la de la imagen esta ocupada esperando cada foto, y compartirla
/// seria hacer esperar al raton a la pantalla.
fn seguir_al_raton(region: Rect, marcha: Marcha) -> Rastro {
    let mut rastro = Rastro::default();
    let Ok((conexion, numero)) = RustConnection::connect(None) else {
        return rastro;
    };
    let Some(raiz) = conexion.setup().roots.get(numero).map(|p| p.root) else {
        return rastro;
    };
    // XFixes da la imagen del puntero. Sin el (no deberia faltar en ningun servidor de este
    // siglo) queda la posicion, y el exportador dibuja su flecha.
    let con_xfixes = conexion.xfixes_query_version(4, 0).ok().and_then(|c| c.reply().ok()).is_some();
    let mut vistos: HashMap<u32, u32> = HashMap::new();
    let mut ultimo: Option<u32> = None;
    while !marcha.parada() {
        std::thread::sleep(CADA);
        if marcha.en_pausa() || marcha.parada() {
            continue;
        }
        let ms = marcha.ms();
        if con_xfixes {
            let Some(cursor) = conexion.xfixes_get_cursor_image().ok().and_then(|c| c.reply().ok()) else {
                continue;
            };
            rastro.cursor.push((ms, i32::from(cursor.x) - region.x, i32::from(cursor.y) - region.y));
            if ultimo != Some(cursor.cursor_serial) {
                ultimo = Some(cursor.cursor_serial);
                let id = match vistos.get(&cursor.cursor_serial) {
                    Some(id) => *id,
                    None => {
                        let id = match puntero_de_argb(&cursor.cursor_image, u32::from(cursor.width), u32::from(cursor.height)) {
                            Some(imagen) => {
                                rastro.punteros.push(Puntero {
                                    caliente: (u32::from(cursor.xhot), u32::from(cursor.yhot)),
                                    imagen,
                                });
                                (rastro.punteros.len() - 1) as u32
                            }
                            // Como en Windows: uno que no se puede leer se dibuja con la flecha.
                            None => u32::MAX,
                        };
                        vistos.insert(cursor.cursor_serial, id);
                        id
                    }
                };
                rastro.cambios_puntero.push((ms, id));
            }
        } else if let Some(raton) = conexion.query_pointer(raiz).ok().and_then(|c| c.reply().ok()) {
            rastro.cursor.push((ms, i32::from(raton.root_x) - region.x, i32::from(raton.root_y) - region.y));
        }
    }
    rastro
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect { x, y, width, height }
    }

    #[test]
    fn una_region_dentro_se_queda_como_esta() {
        let dentro = dentro_de_la_pantalla(r(100, 50, 640, 480), 1920, 1080).expect("cabe");
        assert_eq!((dentro.x, dentro.y, dentro.width, dentro.height), (100, 50, 640, 480));
    }

    #[test]
    fn lo_que_se_sale_se_recorta_y_queda_par() {
        let dentro = dentro_de_la_pantalla(r(-10, 1000, 301, 200), 1920, 1080).expect("toca la pantalla");
        assert_eq!((dentro.x, dentro.y), (0, 1000));
        assert_eq!((dentro.width, dentro.height), (290, 80));
    }

    #[test]
    fn para_una_foto_las_medidas_impares_se_quedan_como_estan() {
        let dentro = lo_que_cae_dentro(r(-10, 1000, 301, 200), 1920, 1080).expect("toca la pantalla");
        assert_eq!((dentro.x, dentro.y, dentro.width, dentro.height), (0, 1000, 291, 80));
        let dentro = lo_que_cae_dentro(r(7, 9, 351, 321), 1920, 1080).expect("cabe");
        assert_eq!((dentro.x, dentro.y, dentro.width, dentro.height), (7, 9, 351, 321));
    }

    #[test]
    fn una_region_fuera_de_la_pantalla_no_se_graba() {
        assert!(dentro_de_la_pantalla(r(2000, 0, 300, 200), 1920, 1080).is_none());
        assert!(dentro_de_la_pantalla(r(0, 0, 1, 200), 1920, 1080).is_none());
    }
}
