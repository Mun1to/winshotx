use std::borrow::Cow;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use color_quant::NeuQuant;
use image::RgbaImage;

use crate::error::{AppError, Result};

/// Indice reservado para "este pixel no ha cambiado": es lo que hace pequennos
/// los GIF de pantalla, porque entre fotograma y fotograma casi nada se mueve.
const TRANSPARENT: u8 = 255;
const PALETTE_COLORS: usize = 255;
/// Como mucho se muestrean estos fotogramas para construir la paleta global.
const PALETTE_SAMPLES: usize = 48;

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
    let mut sample: Vec<u8> = Vec::new();
    for (n, index) in indices.iter().step_by(step).enumerate() {
        let frame = scaled(loader(*index)?, width, height);
        // Un pixel de cada cuatro basta para que NeuQuant vea la distribucion real.
        for pixel in frame.as_raw().chunks_exact(4).step_by(4) {
            sample.extend_from_slice(pixel);
        }
        progress("palette", n, PALETTE_SAMPLES);
    }
    let quantizer = NeuQuant::new(options.sample_faction(), PALETTE_COLORS, &sample);
    drop(sample);

    let color_map = quantizer.color_map_rgb();
    let mut palette = color_map.clone();
    palette.resize(256 * 3, 0);

    let file = BufWriter::with_capacity(1 << 20, File::create(path)?);
    let mut encoder = gif::Encoder::new(file, width as u16, height as u16, &palette)?;
    if options.loop_forever {
        encoder.set_repeat(gif::Repeat::Infinite)?;
    }

    // 2) Cada fotograma se cuantiza con difusion de error y se recorta al area que cambio.
    let tolerance = options.tolerance();
    let mut previous: Option<RgbaImage> = None;
    let mut carry_delay = 0u32;

    for (position, index) in indices.iter().enumerate() {
        progress("encoding", position, total);
        let frame = scaled(loader(*index)?, width, height);
        let delay_ms = delays_ms.get(position).copied().unwrap_or(40) + carry_delay;
        let first = previous.is_none();

        let (buffer, rect) = quantize_frame(
            &frame,
            previous.as_ref(),
            &quantizer,
            &color_map,
            tolerance,
        );
        let Some((left, top, w, h)) = rect else {
            // Nada ha cambiado: el retardo se acumula en el fotograma siguiente.
            carry_delay = delay_ms;
            previous = Some(frame);
            continue;
        };
        carry_delay = 0;

        let mut gif_frame = gif::Frame::default();
        gif_frame.left = left as u16;
        gif_frame.top = top as u16;
        gif_frame.width = w as u16;
        gif_frame.height = h as u16;
        gif_frame.buffer = Cow::Owned(buffer);
        gif_frame.delay = ((delay_ms + 5) / 10).clamp(2, 65_535) as u16;
        gif_frame.dispose = gif::DisposalMethod::Keep;
        gif_frame.transparent = if first { None } else { Some(TRANSPARENT) };
        encoder.write_frame(&gif_frame)?;
        previous = Some(frame);
    }

    progress("done", total, total);
    Ok(())
}

fn scaled(image: RgbaImage, width: u32, height: u32) -> RgbaImage {
    if image.width() == width && image.height() == height {
        return image;
    }
    image::imageops::resize(&image, width, height, image::imageops::FilterType::Lanczos3)
}

/// Rectangulo del area que ha cambiado: left, top, ancho, alto.
type Region = Option<(u32, u32, u32, u32)>;

fn quantize_frame(
    frame: &RgbaImage,
    previous: Option<&RgbaImage>,
    quantizer: &NeuQuant,
    color_map: &[u8],
    tolerance: i32,
) -> (Vec<u8>, Region) {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let current = frame.as_raw();

    // Mascara de cambios calculada sobre el color original, antes de cuantizar:
    // compararlos despues del dithering daria falsos cambios por el error acumulado.
    let mut changed = vec![true; width * height];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (usize::MAX, usize::MAX, 0usize, 0usize);
    let mut any = false;

    match previous {
        None => {
            min_x = 0;
            min_y = 0;
            max_x = width - 1;
            max_y = height - 1;
            any = true;
        }
        Some(prev) if prev.dimensions() == frame.dimensions() => {
            let old = prev.as_raw();
            for y in 0..height {
                for x in 0..width {
                    let i = (y * width + x) * 4;
                    let diff = (current[i] as i32 - old[i] as i32).abs()
                        + (current[i + 1] as i32 - old[i + 1] as i32).abs()
                        + (current[i + 2] as i32 - old[i + 2] as i32).abs();
                    let is_changed = diff > tolerance;
                    changed[y * width + x] = is_changed;
                    if is_changed {
                        any = true;
                        min_x = min_x.min(x);
                        min_y = min_y.min(y);
                        max_x = max_x.max(x);
                        max_y = max_y.max(y);
                    }
                }
            }
        }
        Some(_) => {
            min_x = 0;
            min_y = 0;
            max_x = width - 1;
            max_y = height - 1;
            any = true;
        }
    }

    if !any {
        return (Vec::new(), None);
    }

    let region_w = max_x - min_x + 1;
    let region_h = max_y - min_y + 1;
    let mut out = vec![TRANSPARENT; region_w * region_h];

    // Floyd-Steinberg sobre el area recortada, difundiendo solo entre pixeles vivos.
    let mut errors = vec![0i32; region_w * region_h * 3];
    for y in 0..region_h {
        for x in 0..region_w {
            if !changed[(y + min_y) * width + (x + min_x)] {
                continue;
            }
            let src = ((y + min_y) * width + (x + min_x)) * 4;
            let e = (y * region_w + x) * 3;
            let r = (current[src] as i32 + errors[e]).clamp(0, 255);
            let g = (current[src + 1] as i32 + errors[e + 1]).clamp(0, 255);
            let b = (current[src + 2] as i32 + errors[e + 2]).clamp(0, 255);
            let index = quantizer.index_of(&[r as u8, g as u8, b as u8, 255]);
            out[y * region_w + x] = index as u8;

            let base = index * 3;
            let err = [
                r - color_map[base] as i32,
                g - color_map[base + 1] as i32,
                b - color_map[base + 2] as i32,
            ];
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

    #[test]
    fn frames_identicos_no_generan_region() {
        let frame = solid(8, 8, [10, 20, 30, 255]);
        let quantizer = NeuQuant::new(1, PALETTE_COLORS, frame.as_raw());
        let map = quantizer.color_map_rgb();
        let (_, region) = quantize_frame(&frame, Some(&frame), &quantizer, &map, 0);
        assert!(region.is_none(), "sin cambios no deberia escribirse nada");
    }

    #[test]
    fn solo_se_escribe_el_area_que_cambia() {
        let previous = solid(16, 16, [0, 0, 0, 255]);
        let mut frame = previous.clone();
        frame.put_pixel(10, 12, Rgba([255, 255, 255, 255]));
        let quantizer = NeuQuant::new(1, PALETTE_COLORS, frame.as_raw());
        let map = quantizer.color_map_rgb();
        let (buffer, region) = quantize_frame(&frame, Some(&previous), &quantizer, &map, 0);
        assert_eq!(region, Some((10, 12, 1, 1)));
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn el_primer_fotograma_cubre_todo() {
        let frame = solid(4, 4, [200, 100, 50, 255]);
        let quantizer = NeuQuant::new(1, PALETTE_COLORS, frame.as_raw());
        let map = quantizer.color_map_rgb();
        let (buffer, region) = quantize_frame(&frame, None, &quantizer, &map, 0);
        assert_eq!(region, Some((0, 0, 4, 4)));
        assert_eq!(buffer.len(), 16);
        assert!(buffer.iter().all(|&i| i != TRANSPARENT));
    }
}
