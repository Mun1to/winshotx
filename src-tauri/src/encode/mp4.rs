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
///
/// Fila a fila y con trozos de cuatro bytes, sin indices calculados a mano: asi el
/// compilador no tiene que comprobar los limites en cada canal de cada pixel, que a dos
/// millones de pixeles por fotograma se notaba.
pub fn rgba_to_bgra_bottom_up(image: &RgbaImage) -> Vec<u8> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let fila = width * 4;
    let source = image.as_raw();
    let mut out = vec![0u8; fila * height];
    for (y, destino) in out.chunks_exact_mut(fila).enumerate() {
        let origen = &source[(height - 1 - y) * fila..(height - y) * fila];
        for (d, s) in destino.chunks_exact_mut(4).zip(origen.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
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
    // El ultimo que se mando, para poder mandarlo otra vez al final. Ver abajo.
    let mut ultimo: Option<Vec<u8>> = None;
    for (position, index) in indices.iter().enumerate() {
        progress("encoding", position, total);
        let frame = loader(*index)?;
        // Normalmente ya viene del tamanno pedido y esto no hace nada. Cuando no, escala
        // por el camino de casa: con `image` esta linea sola costaba 64 ms por fotograma.
        let frame = if frame.width() == width && frame.height() == height {
            frame
        } else {
            crate::encode::escalar::a_medida(&frame, width, height)
        };
        let buffer = rgba_to_bgra_bottom_up(&frame);
        encoder
            .send_frame_buffer(&buffer, timestamp)
            .map_err(|e| AppError::Msg(e.to_string()))?;
        let dura = delays_ms.get(position).copied().unwrap_or(33);
        timestamp += dura as i64 * 10_000;
        ultimo = Some(buffer);

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

    // El ultimo fotograma se manda DOS veces, la segunda en el instante en que acaba.
    //
    // Media Foundation solo sabe cuando empieza cada fotograma; el ultimo lo termina donde
    // manda la velocidad declarada, o sea a los 33 milisegundos, sin importar lo que
    // durara de verdad. Con mil fotogramas eso se pierde en el redondeo, pero una pantalla
    // quieta se guarda en UN fotograma que dura tres segundos: el video salia de 66
    // milisegundos, o sea vacio. Se ve al rescatar los ultimos segundos de algo parado, y
    // tambien al exportar una grabacion en la que no se movio nada.
    //
    // Mandarlo otra vez en `timestamp` (que ya ha avanzado su duracion) le da a Media
    // Foundation el final de verdad. Es un fotograma identico al anterior, asi que al
    // comprimirlo no ocupa practicamente nada.
    if let Some(buffer) = ultimo {
        encoder
            .send_frame_buffer(&buffer, timestamp)
            .map_err(|e| AppError::Msg(e.to_string()))?;
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
