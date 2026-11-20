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

#[cfg(test)]
mod banco_anillo;

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
#[derive(Debug, Default, Clone)]
pub struct Rastro {
    pub cursor: Vec<(u64, i32, i32)>,
    /// Las imagenes de los punteros que hubo, en el orden en que aparecieron.
    pub punteros: Vec<Puntero>,
    /// Cual de ellas estaba puesta y desde cuando.
    pub cambios_puntero: Vec<(u64, u32)>,
}

/// El rastro mientras se graba, a la vista de quien no puede esperar al final: «Grabar siempre
/// lo ultimo» lo copia al rescatar y le va tirando lo viejo (`Rastro::podar`).
pub type RastroVivo = Arc<parking_lot::Mutex<Rastro>>;

impl Rastro {
    /// El numero de ese puntero: el de uno igual que ya estuviera, o el suyo, recien apuntado.
    ///
    /// Se mira la imagen y no quien la puso: los programas crean el mismo puntero una y otra
    /// vez, y un rastro que corre toda la tarde no puede guardar una copia por cada vez.
    pub fn puntero(&mut self, caliente: (u32, u32), imagen: RgbaImage) -> u32 {
        let ya = self.punteros.iter().position(|p| p.caliente == caliente && p.imagen == imagen);
        ya.unwrap_or_else(|| {
            self.punteros.push(Puntero { caliente, imagen });
            self.punteros.len() - 1
        }) as u32
    }

    /// Tira lo anterior a `corte_ms`. De lo de antes se queda con el ultimo punto y el ultimo
    /// cambio de puntero, que son los que valen en ese instante: un raton que lleva un rato
    /// quieto sigue estando en algun sitio. Las imagenes no se tiran: son pocas (`puntero`).
    pub fn podar(&mut self, corte_ms: u64) {
        fn podar<T>(lista: &mut Vec<T>, corte_ms: u64, ms: impl Fn(&T) -> u64) {
            // Van en el orden en que pasaron.
            let de_antes = lista.partition_point(|apunte| ms(apunte) < corte_ms);
            lista.drain(..de_antes.saturating_sub(1));
        }
        podar(&mut self.cursor, corte_ms, |punto| punto.0);
        podar(&mut self.cambios_puntero, corte_ms, |cambio| cambio.0);
    }
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

/// Lo mismo para una pantalla ENTERA, que es lo que pide «Grabar siempre lo ultimo». Devuelve
/// tambien la pantalla que se graba de verdad, que puede no ser la pedida: en Wayland la
/// elige la persona en la pregunta del escritorio, y `pantalla` solo es la que se prefiere si
/// ha dejado varias. En X11 es esa.
pub fn preparar_pantalla(
    pantalla: crate::capture::MonitorInfo,
    fps: u32,
    opciones: Opciones,
) -> Result<(Preparada, crate::capture::MonitorInfo)> {
    if crate::platform::xdg::es_wayland() {
        let (preparada, grabada) = wayland::preparar_pantalla(pantalla.id, fps, &opciones.ficha, opciones.al_cortarse)?;
        return Ok((Preparada::Wayland(preparada), grabada));
    }
    let region = Rect { x: pantalla.x, y: pantalla.y, width: pantalla.width, height: pantalla.height };
    Ok((Preparada::X11(x11::preparar(region.to_even(), fps)?), pantalla))
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
    pub(crate) raton: Option<JoinHandle<()>>,
    /// Donde va apuntando el hilo del raton.
    pub(crate) rastro: RastroVivo,
}

impl EnMarcha {
    /// El rastro tal y como va, para leerlo y podarlo sin esperar al final.
    pub fn rastro_vivo(&self) -> RastroVivo {
        self.rastro.clone()
    }

    /// Espera a que los hilos acaben (ya se les ha dicho que paren) y devuelve el rastro.
    pub fn terminar(mut self) -> Rastro {
        for hilo in [self.imagen.take(), self.raton.take()].into_iter().flatten() {
            let _ = hilo.join();
        }
        std::mem::take(&mut *self.rastro.lock())
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

    /// Un rastro que corre toda la tarde: lo viejo se tira, pero se sigue sabiendo donde
    /// estaba el raton y que puntero llevaba en el instante del corte.
    #[test]
    fn al_podar_el_rastro_queda_lo_que_valia_en_el_corte() {
        let flecha = RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]));
        let mano = RgbaImage::from_pixel(2, 2, image::Rgba([9, 9, 9, 255]));
        let mut rastro = Rastro::default();
        assert_eq!(rastro.puntero((0, 0), flecha.clone()), 0);
        assert_eq!(rastro.puntero((1, 1), mano.clone()), 1);
        // El mismo dibujo otra vez no es otro puntero, aunque lo haya puesto otro programa.
        assert_eq!(rastro.puntero((0, 0), flecha.clone()), 0);
        assert_eq!(rastro.puntero((1, 0), flecha), 2, "con otro punto caliente si es otro");
        assert_eq!(rastro.punteros.len(), 3);

        rastro.cursor = vec![(0, 1, 1), (100, 2, 2), (5_000, 3, 3), (9_000, 4, 4)];
        rastro.cambios_puntero = vec![(0, 0), (50, 1), (8_000, 0)];
        rastro.podar(6_000);
        assert_eq!(rastro.cursor, vec![(5_000, 3, 3), (9_000, 4, 4)]);
        assert_eq!(rastro.cambios_puntero, vec![(50, 1), (8_000, 0)]);
        assert_eq!(rastro.punteros.len(), 3, "las imagenes se quedan: los cambios las nombran por su numero");

        // Sin nada despues del corte queda lo ultimo, y podar otra vez no lo pierde.
        rastro.podar(60_000);
        rastro.podar(120_000);
        assert_eq!(rastro.cursor, vec![(9_000, 4, 4)]);
        assert_eq!(rastro.cambios_puntero, vec![(8_000, 0)]);
        Rastro::default().podar(10);
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
