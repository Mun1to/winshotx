//! Grabar la pantalla en Linux: de donde salen los fotogramas y el rastro del raton.
//!
//! Hay dos caminos y no se parecen. En X11 se le pide la imagen al servidor, las veces por
//! segundo que haga falta (`x11`). En Wayland una app no puede leer la pantalla: se le pide
//! al escritorio por el portal `ScreenCast`, que pregunta a la persona que pantalla comparte
//! y la manda por PipeWire (`wayland`). Lo que sale de los dos es lo mismo: fotogramas ya
//! recortados a la region, en RGBA, con el reloj de la grabacion, que van a la cache QOI de
//! siempre (`record::FrameCache`).
//!
//! **Va en dos pasos, `preparar` y `grabar`,** porque en Wayland lo primero puede tardar lo
//! que tarde la persona en contestar al escritorio, y el reloj de la grabacion no puede
//! empezar a contar hasta que hay imagen.
//!
//! **El raton no va cocido en los fotogramas,** igual que en Windows: se apunta por donde va
//! y que imagen tiene (`Rastro`), y lo dibuja el exportador al tamanno que se pida.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use image::RgbaImage;

use crate::capture::Rect;
use crate::error::Result;

pub mod camara;
pub mod entrada;
pub mod sonido;
pub mod wayland;
pub mod x11;

/// Un fotograma recien capturado, ya recortado a la region y en RGBA.
pub struct Fotograma {
    pub rgba: Vec<u8>,
    pub ts_ms: u64,
}

/// Lo que comparten quien graba y quien captura: cuando parar, la pausa y el reloj.
#[derive(Clone)]
pub struct Marcha {
    pub stop: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
    /// Milisegundos acumulados en pausa, para que el reloj no cuente ese tiempo.
    pub paused_ms: Arc<AtomicU64>,
    /// El cero del reloj: el mismo para los fotogramas, el raton y la barra.
    pub start: Instant,
}

impl Marcha {
    /// Lo que lleva grabado en este instante, sin las pausas ya cerradas.
    pub fn ms(&self) -> u64 {
        (self.start.elapsed().as_millis() as u64).saturating_sub(self.paused_ms.load(Ordering::Relaxed))
    }

    pub fn parada(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub fn en_pausa(&self) -> bool {
        self.pause.load(Ordering::Relaxed)
    }
}

/// Bufers de fotograma que van y vuelven entre la captura y quien los guarda, para no
/// reservar ocho megabytes nuevos en cada fotograma (lo mismo que `win::Reciclados`).
#[derive(Clone, Default)]
pub struct Reciclados(Arc<parking_lot::Mutex<Vec<Vec<u8>>>>);

impl Reciclados {
    const TOPE: usize = 4;

    pub fn coger(&self) -> Vec<u8> {
        self.0.lock().pop().unwrap_or_default()
    }

    pub fn devolver(&self, bufer: Vec<u8>) {
        let mut guardados = self.0.lock();
        if guardados.len() < Self::TOPE {
            guardados.push(bufer);
        }
    }
}

/// La imagen de un puntero, con su punto caliente en pixeles de la propia imagen.
#[derive(Debug, Clone)]
pub struct Puntero {
    pub caliente: (u32, u32),
    pub imagen: RgbaImage,
}

/// Por donde fue el raton y que imagen tenia, ya en coordenadas de la region grabada.
#[derive(Debug, Default)]
pub struct Rastro {
    pub cursor: Vec<(u64, i32, i32)>,
    /// Las imagenes de los punteros que hubo, en el orden en que aparecieron.
    pub punteros: Vec<Puntero>,
    /// Cual de ellas estaba puesta y desde cuando.
    pub cambios_puntero: Vec<(u64, u32)>,
}

/// La captura con todo abierto, a falta de que empiece a contar el reloj.
pub enum Preparada {
    X11(x11::Preparada),
    Wayland(wayland::Preparada),
}

/// Lo que solo hace falta en Wayland.
pub struct Opciones {
    /// El archivo donde se guarda el permiso del escritorio, para que no vuelva a preguntar.
    pub ficha: PathBuf,
    /// Lo que hay que hacer si el escritorio corta la grabacion por su cuenta. Se llama desde
    /// el hilo de la captura: no puede esperar a que ese hilo acabe.
    pub al_cortarse: Box<dyn Fn() + Send>,
}

/// Abre lo que haga falta para grabar esa region (en pixeles del escritorio). En Wayland
/// puede tardar lo que tarde la persona en contestar al escritorio.
pub fn preparar(region: Rect, fps: u32, opciones: Opciones) -> Result<Preparada> {
    if crate::platform::xdg::es_wayland() {
        return Ok(Preparada::Wayland(wayland::preparar(region, fps, &opciones.ficha, opciones.al_cortarse)?));
    }
    Ok(Preparada::X11(x11::preparar(region, fps)?))
}

impl Preparada {
    /// La region que se va a grabar de verdad: la pedida, metida dentro de la pantalla.
    pub fn region(&self) -> Rect {
        match self {
            Preparada::X11(p) => p.region(),
            Preparada::Wayland(p) => p.region(),
        }
    }

    /// El raton viene ya dibujado dentro de los fotogramas (los escritorios de Wayland que no
    /// saben mandarlo aparte): el exportador no tiene que dibujarlo otra vez.
    pub fn raton_cocido(&self) -> bool {
        match self {
            Preparada::X11(_) => false,
            Preparada::Wayland(p) => p.raton_cocido(),
        }
    }

    /// Empieza a mandar fotogramas por `salida`, con el reloj de `marcha`.
    pub fn grabar(self, marcha: Marcha, salida: Sender<Fotograma>, reciclados: Reciclados) -> EnMarcha {
        match self {
            Preparada::X11(p) => p.grabar(marcha, salida, reciclados),
            Preparada::Wayland(p) => p.grabar(marcha, salida, reciclados),
        }
    }
}

/// La captura en marcha. Para sola cuando `Marcha::stop` se pone a cierto.
pub struct EnMarcha {
    pub(crate) imagen: Option<JoinHandle<()>>,
    pub(crate) raton: Option<JoinHandle<Rastro>>,
}

impl EnMarcha {
    /// Espera a que los hilos acaben (ya se les ha dicho que paren) y devuelve el rastro.
    pub fn terminar(mut self) -> Rastro {
        if let Some(hilo) = self.imagen.take() {
            let _ = hilo.join();
        }
        self.raton.take().and_then(|hilo| hilo.join().ok()).unwrap_or_default()
    }
}

/// Pasa una fila de pixeles BGRX (como los da X11 y casi siempre PipeWire) a RGBA opaco.
#[inline]
pub(crate) fn bgrx_a_rgba(origen: &[u8], destino: &mut [u8]) {
    for (o, d) in origen.chunks_exact(4).zip(destino.chunks_exact_mut(4)) {
        let v = u32::from_le_bytes([o[0], o[1], o[2], o[3]]);
        let girado = (v & 0x0000_FF00) | ((v & 0x0000_00FF) << 16) | ((v >> 16) & 0x0000_00FF) | 0xFF00_0000;
        d.copy_from_slice(&girado.to_le_bytes());
    }
}

/// La imagen de un puntero tal y como la dan X11 y PipeWire (ARGB con el color ya
/// multiplicado por la transparencia, un `u32` por pixel) a RGBA sin multiplicar, que es
/// lo que guarda un PNG.
pub(crate) fn puntero_de_argb(pixeles: &[u32], ancho: u32, alto: u32) -> Option<RgbaImage> {
    if ancho == 0 || alto == 0 || pixeles.len() < (ancho as usize) * (alto as usize) {
        return None;
    }
    let mut rgba = Vec::with_capacity(pixeles.len() * 4);
    for &p in &pixeles[..(ancho as usize) * (alto as usize)] {
        let a = (p >> 24) & 0xFF;
        let quitar = |canal: u32| -> u8 {
            if a == 0 {
                0
            } else {
                ((canal * 255 + a / 2) / a).min(255) as u8
            }
        };
        rgba.extend_from_slice(&[quitar((p >> 16) & 0xFF), quitar((p >> 8) & 0xFF), quitar(p & 0xFF), a as u8]);
    }
    RgbaImage::from_raw(ancho, alto, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_azul_y_el_rojo_cambian_de_sitio_y_queda_opaco() {
        let mut destino = [0u8; 8];
        bgrx_a_rgba(&[10, 20, 30, 0, 1, 2, 3, 77], &mut destino);
        assert_eq!(destino, [30, 20, 10, 255, 3, 2, 1, 255]);
    }

    #[test]
    fn el_puntero_sale_sin_multiplicar_por_su_transparencia() {
        // Blanco a media transparencia: X11 lo manda como 0x80808080.
        let imagen = puntero_de_argb(&[0x8080_8080, 0xFF00_00FF, 0], 3, 1).expect("tres pixeles");
        assert_eq!(imagen.get_pixel(0, 0).0, [255, 255, 255, 128]);
        assert_eq!(imagen.get_pixel(1, 0).0, [0, 0, 255, 255]);
        assert_eq!(imagen.get_pixel(2, 0).0, [0, 0, 0, 0]);
    }

    #[test]
    fn un_puntero_sin_pixeles_no_es_un_puntero() {
        assert!(puntero_de_argb(&[], 0, 0).is_none());
        assert!(puntero_de_argb(&[1, 2], 2, 2).is_none());
    }

    #[test]
    fn el_reloj_descuenta_las_pausas() {
        let marcha = Marcha {
            stop: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
            paused_ms: Arc::new(AtomicU64::new(u64::MAX)),
            start: Instant::now(),
        };
        assert_eq!(marcha.ms(), 0, "nunca por debajo de cero");
    }
}
