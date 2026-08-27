use std::path::Path;

use image::RgbaImage;

use crate::error::{AppError, Result};

pub struct Mp4Options {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// 10..100; se traduce a bitrate en funcion de la resolucion y el ritmo.
    pub quality: u8,
}

impl Mp4Options {
    pub fn bitrate(&self) -> u32 {
        let pixels_per_second = self.width as f32 * self.height as f32 * self.fps.max(1) as f32;
        let factor = 0.04 + (self.quality.clamp(10, 100) as f32 / 100.0) * 0.16;
        (pixels_per_second * factor).clamp(400_000.0, 60_000_000.0) as u32
    }
}

/// Media Foundation espera BGRA y las filas de abajo arriba.
pub fn rgba_to_bgra_bottom_up(image: &RgbaImage) -> Vec<u8> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let source = image.as_raw();
    let mut out = vec![0u8; width * height * 4];
    for y in 0..height {
        let src_row = y * width * 4;
        let dst_row = (height - 1 - y) * width * 4;
        for x in 0..width {
            let s = src_row + x * 4;
            let d = dst_row + x * 4;
            out[d] = source[s + 2];
            out[d + 1] = source[s + 1];
            out[d + 2] = source[s];
            out[d + 3] = source[s + 3];
        }
    }
    out
}

#[cfg(windows)]
/// El sonido que acompanna al video, ya recortado al tramo que se exporta.
pub struct Pista {
    pub channels: u16,
    pub sample_rate: u32,
    /// PCM de 16 bits con los canales intercalados, que es lo que quiere el codificador.
    pub datos: Vec<u8>,
}

impl Pista {
    fn bloque(&self) -> usize {
        usize::from(self.channels) * 2
    }

    /// Cuantos bytes de sonido corresponden a ese trozo de tiempo, sin cortar una muestra
    /// por la mitad.
    fn bytes_de(&self, ms: u32) -> usize {
        let bloque = self.bloque().max(1);
        let bytes = u64::from(ms) * u64::from(self.sample_rate) * bloque as u64 / 1000;
        (bytes as usize) / bloque * bloque
    }
}

pub fn encode<L, P>(
    indices: &[usize],
    delays_ms: &[u32],
    loader: &mut L,
    path: &Path,
    options: &Mp4Options,
    audio: Option<Pista>,
    mut progress: P,
) -> Result<()>
where
    L: FnMut(usize) -> Result<RgbaImage>,
    P: FnMut(&str, usize, usize),
{
    use windows_capture::encoder::{
        AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
        VideoSettingsSubType,
    };

    if indices.is_empty() {
        return Err(AppError::Msg("no hay fotogramas que exportar".into()));
    }
    // Las dimensiones impares rompen a H.264.
    let width = (options.width / 2) * 2;
    let height = (options.height / 2) * 2;

    let mut encoder = VideoEncoder::new(
        VideoSettingsBuilder::new(width, height)
            .sub_type(VideoSettingsSubType::H264)
            .frame_rate(options.fps.max(1))
            .bitrate(options.bitrate()),
        match audio.as_ref() {
            Some(pista) => AudioSettingsBuilder::default()
                .channel_count(u32::from(pista.channels))
                .sample_rate(pista.sample_rate)
                .bit_per_sample(16)
                .disabled(false),
            None => AudioSettingsBuilder::default().disabled(true),
        },
        ContainerSettingsBuilder::default(),
        path,
    )
    .map_err(|e| AppError::Msg(e.to_string()))?;

    let total = indices.len();
    // Media Foundation cuenta el tiempo en unidades de 100 nanosegundos.
    let mut timestamp: i64 = 0;
    let mut enviado: usize = 0;
    for (position, index) in indices.iter().enumerate() {
        progress("encoding", position, total);
        let frame = loader(*index)?;
        let frame = if frame.width() == width && frame.height() == height {
            frame
        } else {
            image::imageops::resize(
                &frame,
                width,
                height,
                image::imageops::FilterType::Lanczos3,
            )
        };
        let buffer = rgba_to_bgra_bottom_up(&frame);
        encoder
            .send_frame_buffer(&buffer, timestamp)
            .map_err(|e| AppError::Msg(e.to_string()))?;
        let dura = delays_ms.get(position).copied().unwrap_or(33);
        timestamp += dura as i64 * 10_000;

        // El sonido va detras de cada fotograma, en el trozo que le toca por duracion. De
        // una tacada al final tambien valdria (el codificador coloca el audio contando
        // muestras, no por la marca de tiempo), pero llenaria su cola de golpe.
        if let Some(pista) = audio.as_ref() {
            let hasta = (enviado + pista.bytes_de(dura)).min(pista.datos.len());
            if hasta > enviado {
                encoder
                    .send_audio_buffer(&pista.datos[enviado..hasta], 0)
                    .map_err(|e| AppError::Msg(e.to_string()))?;
                enviado = hasta;
            }
        }
    }

    // Y lo que quede sonando cuando ya no hay mas fotogramas.
    if let Some(pista) = audio.as_ref() {
        if enviado < pista.datos.len() {
            encoder
                .send_audio_buffer(&pista.datos[enviado..], 0)
                .map_err(|e| AppError::Msg(e.to_string()))?;
        }
    }

    encoder.finish().map_err(|e| AppError::Msg(e.to_string()))?;
    progress("done", total, total);
    Ok(())
}

#[cfg(not(windows))]
pub fn encode<L, P>(
    _indices: &[usize],
    _delays_ms: &[u32],
    _loader: &mut L,
    _path: &Path,
    _options: &Mp4Options,
    _audio: Option<Pista>,
    _progress: P,
) -> Result<()>
where
    L: FnMut(usize) -> Result<RgbaImage>,
    P: FnMut(&str, usize, usize),
{
    Err(AppError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn la_conversion_invierte_filas_y_canales() {
        let mut image = RgbaImage::from_pixel(1, 2, Rgba([0, 0, 0, 255]));
        image.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
        image.put_pixel(0, 1, Rgba([40, 50, 60, 255]));
        let out = rgba_to_bgra_bottom_up(&image);
        // La ultima fila del original tiene que quedar la primera, y en BGRA.
        assert_eq!(&out[0..4], &[60, 50, 40, 255]);
        assert_eq!(&out[4..8], &[30, 20, 10, 255]);
    }

    #[test]
    fn el_bitrate_sube_con_la_calidad() {
        let base = Mp4Options {
            width: 1280,
            height: 720,
            fps: 30,
            quality: 20,
        };
        let alta = Mp4Options {
            quality: 100,
            ..base
        };
        assert!(alta.bitrate() > base.bitrate());
    }
}
