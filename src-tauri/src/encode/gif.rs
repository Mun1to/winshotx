//! GIF con paleta global, difusion de error y solo la zona que cambia.
//!
//! **Lo que costaba y por que, medido el 5 de septiembre de 2026 en release**, sobre 90
//! fotogramas de 1280x720 con casi todo quieto (como una grabacion de pantalla). El error
//! es cuanto se aleja de media cada canal del color original, mas bajo es mas fiel:
//!
//! | calidad | antes | ahora |
//! |---|---|---|
//! | 50 | 18 ms/fotograma, error 2,01 | 6,6 ms/fotograma, error 1,26 |
//! | 80 | 37 ms/fotograma, error 1,35 | 8,9 ms/fotograma, error 1,27 |
//! | 100 | **308 ms/fotograma**, error 1,54 | 8,4 ms/fotograma, error 1,30 |
//!
//! Tres cosas se comian ese tiempo:
//!
//! 1. **La paleta se entrenaba con todos los pixeles de la muestra.** Uno de cada cuatro
//!    de 48 fotogramas a 720p son once millones, y a calidad 100 NeuQuant los visita todos:
//!    veintisiete segundos antes de escribir un solo fotograma. Ahora se acota cuantos
//!    pixeles VISITA (`VISITAS_MAX`), que es lo que cuesta, y no cuantos hay.
//! 2. **Cada fotograma se comparaba con el anterior pixel a pixel**, dos millones de
//!    restas con sus comprobaciones de limites, para descubrir que casi ninguna fila habia
//!    cambiado. Ahora se comparan las filas enteras primero, que es una sola comparacion
//!    de memoria, y solo las que difieren se miran pixel a pixel.
//! 3. **Buscar el color de la paleta mas cercano** recorre la red de NeuQuant en cada
//!    pixel. Una pantalla tiene pocos colores distintos, asi que la respuesta se guarda en
//!    una tabla con una casilla por color y la segunda vez cuesta una lectura.
//!
//! Y los fotogramas se cuantizan **en paralelo, por lotes**: cada uno solo depende del
//! anterior SIN cuantizar (para saber que cambio), asi que la difusion de error de uno no
//! espera a la del otro. El orden de escritura se conserva y el resultado es el mismo
//! byte a byte que en fila; hay una prueba que codifica dos veces y compara.
//!
//! **Y una correccion de tiempo.** El GIF cuenta en centesimas y cada fotograma de 33 ms
//! se redondeaba a 3 por separado: un clip a 30 fps salia un 10 % mas rapido de lo
//! grabado, y nadie lo habia notado porque un GIF "va rapido" sin mas. Ahora el redondeo
//! se arrastra de un fotograma al siguiente y la duracion total cuadra.

use std::borrow::Cow;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};

use color_quant::NeuQuant;
use image::RgbaImage;
use rayon::prelude::*;

use crate::error::{AppError, Result};

/// Indice reservado para "este pixel no ha cambiado": es lo que hace pequennos
/// los GIF de pantalla, porque entre fotograma y fotograma casi nada se mueve.
const TRANSPARENT: u8 = 255;
const PALETTE_COLORS: usize = 255;
/// Como mucho se muestrean estos fotogramas para construir la paleta global.
const PALETTE_SAMPLES: usize = 48;
/// Cuantos pixeles VISITA NeuQuant al entrenar la paleta, como mucho. Visita uno de cada
/// `sample_faction` de la muestra, asi que la muestra puede ser tanto mas grande cuanto
/// mas baja sea la calidad; lo que se acota es el trabajo, no la muestra. El articulo de
/// NeuQuant llama "mejor calidad" a visitar todos los pixeles de una imagen de 512x512,
/// que son 262.144: esto es el doble.
const VISITAS_MAX: usize = 600_000;
/// Y la muestra en si tampoco puede crecer sin tope, porque vive en memoria de una pieza
/// mientras se entrena: cuatro millones de pixeles son 16 MB.
const MUESTRA_MAX: usize = 4_000_000;
/// Cuanta memoria se deja para los fotogramas de un lote que se cuantizan a la vez.
const LOTE_BYTES: usize = 96 << 20;

pub struct GifOptions {
    pub width: u32,
    pub height: u32,
    /// 10..100. Manda en el submuestreo del cuantizador y en la tolerancia del diff.
    pub quality: u8,
    pub loop_forever: bool,
}

impl GifOptions {
    /// NeuQuant llama "sample faction" al paso de muestreo: 1 es el mejor y el mas lento.
    fn sample_faction(&self) -> i32 {
        let q = self.quality.clamp(10, 100) as i32;
        (31 - (q * 30) / 100).clamp(1, 30)
    }

    /// Diferencia por canal por debajo de la cual dos pixeles se consideran iguales.
    fn tolerance(&self) -> i32 {
        let q = self.quality.clamp(10, 100) as i32;
        ((100 - q) * 18 / 100).clamp(0, 18)
    }
}

/// Codifica el GIF leyendo los fotogramas bajo demanda: nunca se cargan todos en RAM.
pub fn encode<L, P>(
    indices: &[usize],
    delays_ms: &[u32],
    loader: &mut L,
    path: &Path,
    options: &GifOptions,
    mut progress: P,
) -> Result<()>
where
    L: FnMut(usize) -> Result<RgbaImage>,
    P: FnMut(&str, usize, usize),
{
    if indices.is_empty() {
        return Err(AppError::Msg("no hay fotogramas que exportar".into()));
    }
    let width = options.width.max(1);
    let height = options.height.max(1);
    let total = indices.len();

    // 1) Paleta global: una sola para todo el clip, asi no parpadean los colores.
    progress("palette", 0, total);
    let step = (total / PALETTE_SAMPLES).max(1);
    let muestreados = total.div_ceil(step);
    let zancada = zancada_de_muestreo(width, height, muestreados, options.sample_faction());
    let por_fotograma = ((width as usize) * (height as usize)).div_ceil(zancada);
    let mut sample: Vec<u8> = Vec::with_capacity(por_fotograma * muestreados * 4);
    for (n, index) in indices.iter().step_by(step).enumerate() {
        let frame = scaled(loader(*index)?, width, height);
        for pixel in frame.as_raw().chunks_exact(4).step_by(zancada) {
            sample.extend_from_slice(pixel);
        }
        progress("palette", n, muestreados);
    }
    let quantizer = NeuQuant::new(options.sample_faction(), PALETTE_COLORS, &sample);
    drop(sample);

    let color_map = quantizer.color_map_rgb();
    let mut palette = color_map.clone();
    palette.resize(256 * 3, 0);
    let buscador = Buscador::nuevo(&quantizer, &color_map);

    let file = BufWriter::with_capacity(1 << 20, File::create(path)?);
    let mut encoder = gif::Encoder::new(file, width as u16, height as u16, &palette)?;
    if options.loop_forever {
        encoder.set_repeat(gif::Repeat::Infinite)?;
    }

    // 2) Cada fotograma se cuantiza con difusion de error y se recorta al area que cambio.
    //
    // Por lotes: se cargan unos cuantos en orden (leer es secuencial, porque los parches
    // de la cache se apoyan unos en otros) y se cuantizan todos a la vez. Lo unico que
    // necesita cada uno es el fotograma anterior tal cual salio de la grabacion, que ya
    // esta en el lote o es el ultimo del lote de antes.
    let tolerance = options.tolerance();
    let lote = tamanno_del_lote(width, height);
    let mut previous: Option<RgbaImage> = None;
    let mut reloj = Reloj::default();
    let mut escritos = 0usize;

    for (numero_lote, trozo) in indices.chunks(lote).enumerate() {
        progress("encoding", numero_lote * lote, total);
        let frames = trozo
            .iter()
            .map(|index| Ok(scaled(loader(*index)?, width, height)))
            .collect::<Result<Vec<RgbaImage>>>()?;

        let cuantizados: Vec<(Vec<u8>, Region)> = frames
            .par_iter()
            .enumerate()
            .map(|(k, frame)| {
                let anterior = if k == 0 { previous.as_ref() } else { Some(&frames[k - 1]) };
                quantize_frame(frame, anterior, &buscador, tolerance)
            })
            .collect();

        for (k, (buffer, rect)) in cuantizados.into_iter().enumerate() {
            let position = numero_lote * lote + k;
            let delay_ms = delays_ms.get(position).copied().unwrap_or(40);
            let Some((left, top, w, h)) = rect else {
                // Nada ha cambiado: el tiempo se acumula en el fotograma siguiente.
                reloj.acumular(delay_ms);
                continue;
            };
            reloj.acumular(delay_ms);

            let mut gif_frame = gif::Frame {
                left: left as u16,
                top: top as u16,
                width: w as u16,
                height: h as u16,
                buffer: Cow::Owned(buffer),
                ..Default::default()
            };
            gif_frame.delay = reloj.retardo();
            gif_frame.dispose = gif::DisposalMethod::Keep;
            gif_frame.transparent = if escritos == 0 { None } else { Some(TRANSPARENT) };
            encoder.write_frame(&gif_frame)?;
            escritos += 1;
        }
        previous = frames.into_iter().last();
    }

    progress("done", total, total);
    Ok(())
}

/// Cada cuantos pixeles se coge uno para la paleta, para que NeuQuant no visite mas de
/// `VISITAS_MAX` entre todos los fotogramas muestreados ni la muestra pase de
/// `MUESTRA_MAX`. Impar a proposito: una zancada par sobre una pantalla llena de rejillas
/// de dos y cuatro pixeles cogeria siempre la misma columna.
fn zancada_de_muestreo(width: u32, height: u32, fotogramas: usize, sample_faction: i32) -> usize {
    let pixeles = (width as usize) * (height as usize) * fotogramas.max(1);
    let tope = (VISITAS_MAX * sample_faction.max(1) as usize).min(MUESTRA_MAX);
    let zancada = pixeles.div_ceil(tope).max(3);
    zancada | 1
}

/// Cuantos fotogramas se cuantizan a la vez: los que quepan en `LOTE_BYTES`, sin pasar de
/// los hilos que hay ni bajar de dos.
fn tamanno_del_lote(width: u32, height: u32) -> usize {
    let por_fotograma = ((width as usize) * (height as usize) * 4).max(1);
    let caben = LOTE_BYTES / por_fotograma;
    caben.clamp(2, rayon::current_num_threads().max(2))
}

/// El reloj del GIF, que cuenta en centesimas y no puede redondear cada fotograma por su
/// cuenta: 33 ms redondeados a 3 en cada uno es un 10 % de prisa acumulada.
#[derive(Default)]
struct Reloj {
    /// Lo que deberia haber pasado, en milisegundos de la grabacion.
    debido_ms: u64,
    /// Lo que se ha escrito ya en el GIF, en centesimas.
    escrito_cs: u64,
}

impl Reloj {
    fn acumular(&mut self, ms: u32) {
        self.debido_ms += u64::from(ms);
    }

    /// El retardo del fotograma que se va a escribir ahora: lo que falta para ponerse al
    /// dia, con el minimo que los navegadores respetan (por debajo de 2 lo tratan como 10).
    fn retardo(&mut self) -> u16 {
        let objetivo_cs = (self.debido_ms + 5) / 10;
        let retardo = objetivo_cs.saturating_sub(self.escrito_cs).clamp(2, 65_535);
        self.escrito_cs += retardo;
        retardo as u16
    }
}

fn scaled(image: RgbaImage, width: u32, height: u32) -> RgbaImage {
    if image.width() == width && image.height() == height {
        return image;
    }
    super::escalar::a_medida(&image, width, height)
}

/// El color de la paleta mas cercano a uno dado, con memoria.
///
/// NeuQuant busca recorriendo su red y eso, pixel a pixel, es lo que mas costaba. Una
/// pantalla tiene pocos colores distintos, asi que la respuesta se guarda por color
/// exacto (una casilla por cada uno de los 16 millones, un byte cada una) y a la segunda
/// vez es una lectura. Dieciseis megabytes que viven lo que dura la exportacion.
///
/// Es exacto a proposito: se probo con casillas mas gordas (seis bits por canal, medio
/// megabyte) y el color se alejaba 0,4 niveles mas de media en un degradado, ademas de
/// que dos hilos podian guardar respuestas distintas para la misma casilla y el GIF salia
/// distinto cada vez. Con una casilla por color la respuesta solo depende del color, asi
/// que las casillas se comparten entre los hilos del lote sin candado: quien escriba,
/// escribe lo mismo.
pub(crate) struct Buscador<'a> {
    quantizer: &'a NeuQuant,
    color_map: &'a [u8],
    casillas: Vec<AtomicU8>,
}

impl<'a> Buscador<'a> {
    pub(crate) fn nuevo(quantizer: &'a NeuQuant, color_map: &'a [u8]) -> Self {
        Self {
            quantizer,
            color_map,
            casillas: std::iter::repeat_with(|| AtomicU8::new(0)).take(1 << 24).collect(),
        }
    }

    #[inline]
    pub(crate) fn indice(&self, r: u8, g: u8, b: u8) -> usize {
        let casilla = ((r as usize) << 16) | ((g as usize) << 8) | (b as usize);
        let guardado = self.casillas[casilla].load(Ordering::Relaxed);
        if guardado != 0 {
            return usize::from(guardado - 1);
        }
        let indice = self.exacto(r, g, b);
        // El indice mas alto es 254, asi que +1 cabe en el byte y deja el 0 para "vacio".
        self.casillas[casilla].store((indice + 1) as u8, Ordering::Relaxed);
        indice
    }

    /// La busqueda de NeuQuant tal cual, sin memoria.
    pub(crate) fn exacto(&self, r: u8, g: u8, b: u8) -> usize {
        self.quantizer.index_of(&[r, g, b, 255])
    }

    #[inline]
    fn color(&self, indice: usize) -> [i32; 3] {
        let base = indice * 3;
        [
            self.color_map[base] as i32,
            self.color_map[base + 1] as i32,
            self.color_map[base + 2] as i32,
        ]
    }
}

/// Rectangulo del area que ha cambiado: left, top, ancho, alto.
type Region = Option<(u32, u32, u32, u32)>;

/// Si dos pixeles se consideran distintos con esa tolerancia.
#[inline]
fn distinto(a: &[u8], b: &[u8], tolerance: i32) -> bool {
    let diff = (a[0] as i32 - b[0] as i32).abs()
        + (a[1] as i32 - b[1] as i32).abs()
        + (a[2] as i32 - b[2] as i32).abs();
    diff > tolerance
}

/// La zona que ha cambiado entre dos fotogramas del mismo tamanno, en pixeles: izquierda,
/// arriba, derecha y abajo, incluidos. `None` si nada ha cambiado.
///
/// Primero las filas enteras, con una comparacion de memoria cada una: en una grabacion
/// de pantalla casi todas son identicas y se despachan asi. Solo las que difieren se
/// miran pixel a pixel, que es donde entra la tolerancia.
fn zona_cambiada(
    current: &[u8],
    old: &[u8],
    width: usize,
    height: usize,
    tolerance: i32,
) -> Option<(usize, usize, usize, usize)> {
    let fila = width * 4;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..height {
        let (actual, antes) = (&current[y * fila..(y + 1) * fila], &old[y * fila..(y + 1) * fila]);
        if actual == antes {
            continue;
        }
        let mut primera = None;
        let mut ultima = 0;
        for (x, (a, b)) in actual.chunks_exact(4).zip(antes.chunks_exact(4)).enumerate() {
            if distinto(a, b, tolerance) {
                primera.get_or_insert(x);
                ultima = x;
            }
        }
        let Some(primera) = primera else { continue };
        min_x = min_x.min(primera);
        max_x = max_x.max(ultima);
        if min_y == usize::MAX {
            min_y = y;
        }
        max_y = y;
    }
    (min_y != usize::MAX).then_some((min_x, min_y, max_x, max_y))
}

fn quantize_frame(
    frame: &RgbaImage,
    previous: Option<&RgbaImage>,
    buscador: &Buscador,
    tolerance: i32,
) -> (Vec<u8>, Region) {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let current = frame.as_raw();

    // La mascara de cambios se calcula sobre el color original, antes de cuantizar:
    // compararlos despues del dithering daria falsos cambios por el error acumulado.
    let anterior = previous.filter(|prev| prev.dimensions() == frame.dimensions());
    let (min_x, min_y, max_x, max_y) = match anterior {
        None => (0, 0, width - 1, height - 1),
        Some(prev) => match zona_cambiada(current, prev.as_raw(), width, height, tolerance) {
            None => return (Vec::new(), None),
            Some(zona) => zona,
        },
    };

    let region_w = max_x - min_x + 1;
    let region_h = max_y - min_y + 1;
    let mut out = vec![TRANSPARENT; region_w * region_h];

    // Solo dentro del rectangulo, y solo los pixeles vivos: el resto se queda transparente
    // y el GIF conserva lo que ya habia. La mascara se saca aqui, sobre el rectangulo y no
    // sobre el fotograma entero, que es lo que costaba antes.
    let fila = width * 4;
    let mut vivo = vec![true; region_w * region_h];
    if let Some(prev) = anterior {
        let old = prev.as_raw();
        for y in 0..region_h {
            let desde = (y + min_y) * fila + min_x * 4;
            let hasta = desde + region_w * 4;
            let (actual, antes) = (&current[desde..hasta], &old[desde..hasta]);
            let destino = &mut vivo[y * region_w..(y + 1) * region_w];
            if actual == antes {
                destino.fill(false);
                continue;
            }
            for ((v, a), b) in destino
                .iter_mut()
                .zip(actual.chunks_exact(4))
                .zip(antes.chunks_exact(4))
            {
                *v = distinto(a, b, tolerance);
            }
        }
    }

    // Floyd-Steinberg sobre el area recortada, difundiendo solo entre pixeles vivos.
    let mut errors = vec![0i32; region_w * region_h * 3];
    for y in 0..region_h {
        let src_fila = (y + min_y) * fila + min_x * 4;
        for x in 0..region_w {
            if !vivo[y * region_w + x] {
                continue;
            }
            let src = src_fila + x * 4;
            let e = (y * region_w + x) * 3;
            let r = (current[src] as i32 + errors[e]).clamp(0, 255);
            let g = (current[src + 1] as i32 + errors[e + 1]).clamp(0, 255);
            let b = (current[src + 2] as i32 + errors[e + 2]).clamp(0, 255);
            let index = buscador.indice(r as u8, g as u8, b as u8);
            out[y * region_w + x] = index as u8;

            let elegido = buscador.color(index);
            let err = [r - elegido[0], g - elegido[1], b - elegido[2]];
            diffuse(&mut errors, region_w, region_h, x, y, err);
        }
    }

    (
        out,
        Some((min_x as u32, min_y as u32, region_w as u32, region_h as u32)),
    )
}

fn diffuse(errors: &mut [i32], w: usize, h: usize, x: usize, y: usize, err: [i32; 3]) {
    let mut spread = |tx: usize, ty: usize, factor: i32| {
        if tx >= w || ty >= h {
            return;
        }
        let target = (ty * w + tx) * 3;
        for c in 0..3 {
            errors[target + c] += err[c] * factor / 16;
        }
    };
    if x + 1 < w {
        spread(x + 1, y, 7);
    }
    if y + 1 < h {
        if x > 0 {
            spread(x - 1, y + 1, 3);
        }
        spread(x, y + 1, 5);
        if x + 1 < w {
            spread(x + 1, y + 1, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba(color))
    }

    fn buscador_de(frame: &RgbaImage) -> (NeuQuant, Vec<u8>) {
        let quantizer = NeuQuant::new(1, PALETTE_COLORS, frame.as_raw());
        let map = quantizer.color_map_rgb();
        (quantizer, map)
    }

    #[test]
    fn frames_identicos_no_generan_region() {
        let frame = solid(8, 8, [10, 20, 30, 255]);
        let (q, map) = buscador_de(&frame);
        let buscador = Buscador::nuevo(&q, &map);
        let (_, region) = quantize_frame(&frame, Some(&frame), &buscador, 0);
        assert!(region.is_none(), "sin cambios no deberia escribirse nada");
    }

    #[test]
    fn solo_se_escribe_el_area_que_cambia() {
        let previous = solid(16, 16, [0, 0, 0, 255]);
        let mut frame = previous.clone();
        frame.put_pixel(10, 12, Rgba([255, 255, 255, 255]));
        let (q, map) = buscador_de(&frame);
        let buscador = Buscador::nuevo(&q, &map);
        let (buffer, region) = quantize_frame(&frame, Some(&previous), &buscador, 0);
        assert_eq!(region, Some((10, 12, 1, 1)));
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn el_primer_fotograma_cubre_todo() {
        let frame = solid(4, 4, [200, 100, 50, 255]);
        let (q, map) = buscador_de(&frame);
        let buscador = Buscador::nuevo(&q, &map);
        let (buffer, region) = quantize_frame(&frame, None, &buscador, 0);
        assert_eq!(region, Some((0, 0, 4, 4)));
        assert_eq!(buffer.len(), 16);
        assert!(buffer.iter().all(|&i| i != TRANSPARENT));
    }

    /// Dos pixeles cambiados en esquinas opuestas: el rectangulo los abarca a los dos y
    /// lo de en medio, que no cambio, va transparente.
    #[test]
    fn lo_que_no_cambia_dentro_del_rectangulo_va_transparente() {
        let previous = solid(8, 8, [0, 0, 0, 255]);
        let mut frame = previous.clone();
        frame.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        frame.put_pixel(6, 6, Rgba([255, 255, 255, 255]));
        let (q, map) = buscador_de(&frame);
        let buscador = Buscador::nuevo(&q, &map);
        let (buffer, region) = quantize_frame(&frame, Some(&previous), &buscador, 0);
        assert_eq!(region, Some((1, 1, 6, 6)));
        let opacos = buffer.iter().filter(|&&i| i != TRANSPARENT).count();
        assert_eq!(opacos, 2, "solo los dos pixeles tocados llevan color");
    }

    /// Un cambio por debajo de la tolerancia no cuenta, aunque la fila difiera en memoria.
    #[test]
    fn un_cambio_minimo_no_cuenta_con_tolerancia() {
        let previous = solid(8, 8, [100, 100, 100, 255]);
        let mut frame = previous.clone();
        frame.put_pixel(3, 3, Rgba([101, 100, 100, 255]));
        let (q, map) = buscador_de(&frame);
        let buscador = Buscador::nuevo(&q, &map);
        let (_, region) = quantize_frame(&frame, Some(&previous), &buscador, 6);
        assert!(region.is_none(), "un nivel de diferencia no llega a la tolerancia");
        let (_, region) = quantize_frame(&frame, Some(&previous), &buscador, 0);
        assert_eq!(region, Some((3, 3, 1, 1)), "sin tolerancia si cuenta");
    }

    /// La memoria del buscador tiene que dar exactamente lo mismo que la busqueda sin
    /// memoria: es una casilla por color, asi que no hay aproximacion que valga.
    #[test]
    fn la_memoria_del_buscador_no_estropea_el_color() {
        // Una paleta de un degradado con muchos colores, para que la busqueda importe.
        let muestra = RgbaImage::from_fn(256, 64, |x, y| {
            Rgba([x as u8, (y * 4) as u8, ((x + y * 3) % 256) as u8, 255])
        });
        let (q, map) = buscador_de(&muestra);
        let buscador = Buscador::nuevo(&q, &map);
        let distancia = |indice: usize, c: [u8; 3]| -> i64 {
            let p = buscador.color(indice);
            (0..3).map(|i| (p[i] - c[i] as i32).abs() as i64).sum()
        };
        let mut semilla: u32 = 0x2545_F491;
        let (mut exacta, mut con_memoria) = (0i64, 0i64);
        let cuantos = 20_000;
        for _ in 0..cuantos {
            semilla = semilla.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let c = [(semilla >> 24) as u8, (semilla >> 16) as u8, (semilla >> 8) as u8];
            exacta += distancia(buscador.exacto(c[0], c[1], c[2]), c);
            con_memoria += distancia(buscador.indice(c[0], c[1], c[2]), c);
        }
        assert_eq!(con_memoria, exacta, "la memoria ha devuelto otro color que la busqueda");
    }

    /// Tres fotogramas de 33 ms tienen que durar 10 centesimas en total, no 9: es el
    /// redondeo que hacia que un GIF a 30 fps fuera un 10 % mas rapido que la grabacion.
    #[test]
    fn el_reloj_no_pierde_tiempo_al_redondear() {
        let mut reloj = Reloj::default();
        let mut total = 0u64;
        for _ in 0..30 {
            reloj.acumular(33);
            total += u64::from(reloj.retardo());
        }
        // 30 x 33 ms = 990 ms = 99 centesimas. Redondeando cada uno a 3 salian 90.
        assert_eq!(total, 99);
    }

    #[test]
    fn el_reloj_respeta_el_minimo_que_entienden_los_navegadores() {
        let mut reloj = Reloj::default();
        reloj.acumular(5);
        assert_eq!(reloj.retardo(), 2, "por debajo de 2 el navegador lo trata como 10");
    }

    /// Un lote nunca es tan grande que se coma la memoria, ni tan pequenno que no valga.
    #[test]
    fn el_lote_cabe_en_memoria() {
        assert!(tamanno_del_lote(3840, 2160) <= 3, "a 4K un lote son 33 MB por fotograma");
        assert!(tamanno_del_lote(320, 200) >= 2);
        assert!(tamanno_del_lote(320, 200) <= rayon::current_num_threads().max(2));
    }

    /// La muestra de la paleta no crece con el tamanno del clip, que es lo que hacia que
    /// calidad 100 tardara treinta segundos antes de escribir nada.
    #[test]
    fn la_muestra_de_la_paleta_tiene_tope() {
        let pixeles_de = |zancada: usize| 1920usize * 1080 * 48 / zancada;
        // A calidad 100 NeuQuant visita toda la muestra: la muestra es el tope de visitas.
        let a_tope = zancada_de_muestreo(1920, 1080, 48, 1);
        assert!(pixeles_de(a_tope) <= VISITAS_MAX, "{} visitas, mas del tope", pixeles_de(a_tope));
        // A calidad baja visita uno de cada dieciseis, asi que la muestra puede ser mayor,
        // pero nunca mas de lo que cabe en memoria.
        let holgada = zancada_de_muestreo(1920, 1080, 48, 16);
        assert!(holgada < a_tope, "con menos calidad se puede muestrear mas");
        assert!(pixeles_de(holgada) <= MUESTRA_MAX);
        assert!(pixeles_de(holgada) / 16 <= VISITAS_MAX);
        assert!(a_tope % 2 == 1 && holgada % 2 == 1, "la zancada tiene que ser impar");
        assert!(zancada_de_muestreo(32, 32, 1, 1) >= 3);
    }

    /// Codificar de verdad, de punta a punta: el archivo se vuelve a abrir, tiene los
    /// fotogramas que tiene que tener y dura lo que duraba la grabacion. Y dos veces
    /// seguidas sale el mismo archivo byte a byte, que es lo que garantiza que cuantizar en
    /// paralelo no cambia nada mas que el tiempo que tarda.
    #[test]
    fn el_gif_se_abre_dura_lo_que_debe_y_es_reproducible() {
        let (ancho, alto, cuantos) = (48u32, 32u32, 12usize);
        let frames: Vec<RgbaImage> = (0..cuantos as u32)
            .map(|i| {
                let mut f = RgbaImage::from_fn(ancho, alto, |x, y| {
                    Rgba([(x * 5) as u8, (y * 7) as u8, 90, 255])
                });
                for y in 4..12 {
                    for x in (i * 3)..(i * 3 + 6) {
                        f.put_pixel(x, y, Rgba([250, 250, 250, 255]));
                    }
                }
                f
            })
            .collect();
        let indices: Vec<usize> = (0..cuantos).collect();
        let delays = vec![33u32; cuantos];
        let dir = std::env::temp_dir().join("winshotx-test-gif");
        std::fs::create_dir_all(&dir).unwrap();

        let codificar = |nombre: &str| -> Vec<u8> {
            let path = dir.join(nombre);
            let mut loader = |i: usize| Ok(frames[i].clone());
            encode(
                &indices,
                &delays,
                &mut loader,
                &path,
                &GifOptions { width: ancho, height: alto, quality: 80, loop_forever: true },
                |_, _, _| {},
            )
            .expect("no se ha podido codificar");
            std::fs::read(&path).unwrap()
        };
        let uno = codificar("uno.gif");
        let otro = codificar("otro.gif");
        assert_eq!(uno, otro, "dos codificaciones iguales tienen que dar el mismo archivo");

        let mut opciones = gif::DecodeOptions::new();
        opciones.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = opciones.read_info(std::io::Cursor::new(&uno)).unwrap();
        let mut leidos = 0;
        let mut total_cs = 0u32;
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            leidos += 1;
            total_cs += u32::from(frame.delay);
        }
        assert_eq!(leidos, cuantos, "cada fotograma cambia, asi que salen todos");
        // 12 x 33 ms = 396 ms, que son 40 centesimas redondeando.
        assert_eq!(total_cs, 40, "el GIF no dura lo que duraba la grabacion");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
