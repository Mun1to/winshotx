//! WAV de ida y vuelta, en Rust a pelo (S10, los sonidos).
//!
//! Lo usan el sonido de clic propio, que se guarda en la carpeta de la app como un WAV
//! nuestro (48 kHz, mono, 16 bits), y el sonido al empezar a grabar, que `PlaySoundW` solo
//! sabe tocar si es un WAV PCM. Leer un WAV no necesita a Windows: así se prueba en
//! cualquier máquina y no depende de qué codificadores tenga instalados.
//!
//! Se lee lo que de verdad se encuentra: PCM de 8, 16, 24 y 32 bits, coma flotante de 32 y
//! 64, y el formato «extensible» que escriben casi todos los editores de sonido. Varios
//! canales se juntan en uno, que es lo que suena en un clic.

use crate::error::{AppError, Result};

/// Un WAV PCM de 16 bits y un canal con esas muestras, de -1 a 1. Lo que se pasa de ahí se
/// queda en el tope en vez de dar la vuelta.
pub fn escribir_mono16(muestras: &[f32], frecuencia: u32) -> Vec<u8> {
    let datos = (muestras.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + datos as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + datos).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // un canal
    wav.extend_from_slice(&frecuencia.to_le_bytes());
    wav.extend_from_slice(&(frecuencia * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&datos.to_le_bytes());
    for &m in muestras {
        let m = if m.is_finite() { m.clamp(-1.0, 1.0) } else { 0.0 };
        wav.extend_from_slice(&((m * i16::MAX as f32).round() as i16).to_le_bytes());
    }
    wav
}

/// Las muestras de un WAV, ya en un canal y de -1 a 1, y a qué frecuencia van.
///
/// Si el bloque de datos dice que mide más de lo que hay (un archivo cortado, o leído solo
/// por el principio), se queda con lo que hay.
pub fn leer(bytes: &[u8]) -> Result<(Vec<f32>, u32)> {
    let no_es = || AppError::Msg("ese archivo no es un WAV que se pueda leer".into());
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(no_es());
    }
    let u16_en = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_en = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let mut formato: Option<(u16, u16, u32, u16)> = None;
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let largo = u32_en(i + 4) as usize;
        let cuerpo = i + 8;
        if id == b"fmt " && cuerpo + 16 <= bytes.len() {
            let mut codigo = u16_en(cuerpo);
            let canales = u16_en(cuerpo + 2);
            let frecuencia = u32_en(cuerpo + 4);
            let bits = u16_en(cuerpo + 14);
            // El «extensible» lleva el formato de verdad en los dos primeros bytes de su GUID.
            if codigo == 0xFFFE && largo >= 40 && cuerpo + 26 <= bytes.len() {
                codigo = u16_en(cuerpo + 24);
            }
            formato = Some((codigo, canales, frecuencia, bits));
        } else if id == b"data" {
            let (codigo, canales, frecuencia, bits) = formato.ok_or_else(no_es)?;
            let fin = cuerpo.saturating_add(largo).min(bytes.len());
            let muestras = a_mono(&bytes[cuerpo..fin], codigo, canales, bits)?;
            if frecuencia == 0 {
                return Err(no_es());
            }
            return Ok((muestras, frecuencia));
        }
        // Los bloques de largo impar llevan un byte de relleno.
        i = cuerpo.saturating_add(largo).saturating_add(largo & 1);
    }
    Err(no_es())
}

fn a_mono(datos: &[u8], codigo: u16, canales: u16, bits: u16) -> Result<Vec<f32>> {
    let canales = usize::from(canales.max(1));
    let ancho = usize::from(bits / 8);
    let una: fn(&[u8]) -> f32 = match (codigo, bits) {
        (1, 8) => |b| (f32::from(b[0]) - 128.0) / 128.0,
        (1, 16) => |b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0,
        (1, 24) => |b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
        (1, 32) => |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
        (3, 32) => |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        (3, 64) => |b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as f32,
        _ => {
            return Err(AppError::Msg(format!(
                "ese WAV va comprimido (formato {codigo}, {bits} bits). Prueba a guardarlo como WAV PCM o MP3."
            )))
        }
    };
    let bloque = canales * ancho;
    Ok(datos
        .chunks_exact(bloque)
        .map(|b| {
            let suma: f32 = b.chunks_exact(ancho).map(una).sum();
            let m = suma / canales as f32;
            if m.is_finite() {
                m.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        })
        .collect())
}

/// Las mismas muestras a otra frecuencia, uniendo con rectas. Para un clic de menos de un
/// segundo no hace falta más: lo que se pierde por arriba de 20 kHz no se oye.
pub fn a_frecuencia(muestras: &[f32], de: u32, a: u32) -> Vec<f32> {
    if de == a || de == 0 || a == 0 || muestras.is_empty() {
        return muestras.to_vec();
    }
    let paso = f64::from(de) / f64::from(a);
    let largo = ((muestras.len() as f64) / paso).floor().max(1.0) as usize;
    (0..largo)
        .map(|k| {
            let x = k as f64 * paso;
            let i = x.floor() as usize;
            let f = (x - i as f64) as f32;
            let a = muestras[i.min(muestras.len() - 1)];
            let b = muestras[(i + 1).min(muestras.len() - 1)];
            a + (b - a) * f
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un WAV a mano: cabecera de 16 bytes de formato y los datos tal cual.
    fn wav(codigo: u16, canales: u16, frecuencia: u32, bits: u16, datos: &[u8]) -> Vec<u8> {
        let mut w = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&codigo.to_le_bytes());
        w.extend_from_slice(&canales.to_le_bytes());
        w.extend_from_slice(&frecuencia.to_le_bytes());
        w.extend_from_slice(&(frecuencia * u32::from(canales) * u32::from(bits / 8)).to_le_bytes());
        w.extend_from_slice(&(canales * bits / 8).to_le_bytes());
        w.extend_from_slice(&bits.to_le_bytes());
        // Un bloque que no es de sonido por en medio, como los que dejan los editores.
        w.extend_from_slice(b"LIST\x03\0\0\0abc\0");
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(datos.len() as u32).to_le_bytes());
        w.extend_from_slice(datos);
        w
    }

    #[test]
    fn lo_que_se_escribe_se_lee_igual() {
        let muestras = [0.0, 0.5, -0.5, 1.0, -1.0, 2.0];
        let bytes = escribir_mono16(&muestras, 48_000);
        assert_eq!(bytes.len(), 44 + 12);
        let (leido, frecuencia) = leer(&bytes).unwrap();
        assert_eq!(frecuencia, 48_000);
        assert_eq!(leido.len(), 6);
        for (a, b) in leido.iter().zip([0.0, 0.5, -0.5, 1.0, -1.0, 1.0]) {
            assert!((a - b).abs() < 1e-3, "{a} frente a {b}");
        }
    }

    #[test]
    fn lee_los_formatos_que_se_encuentran_y_junta_los_canales() {
        // Estéreo de 16 bits: medio arriba en un canal y nada en el otro es un cuarto.
        let estereo = [16_384i16, 0, -32_768, -32_768].iter().flat_map(|m| m.to_le_bytes()).collect::<Vec<_>>();
        let (m, f) = leer(&wav(1, 2, 44_100, 16, &estereo)).unwrap();
        assert_eq!(f, 44_100);
        assert!((m[0] - 0.25).abs() < 1e-3 && (m[1] + 1.0).abs() < 1e-3, "{m:?}");
        // 8 bits sin signo, 24 y 32 enteros, y coma flotante de 32 y 64.
        assert!((leer(&wav(1, 1, 8_000, 8, &[255, 0, 128])).unwrap().0[2]).abs() < 1e-3);
        let (m24, _) = leer(&wav(1, 1, 8_000, 24, &[0, 0, 0x40, 0, 0, 0xC0])).unwrap();
        assert!((m24[0] - 0.5).abs() < 1e-3 && (m24[1] + 0.5).abs() < 1e-3, "{m24:?}");
        let (m32, _) = leer(&wav(1, 1, 8_000, 32, &(i32::MIN / 2).to_le_bytes())).unwrap();
        assert!((m32[0] + 0.5).abs() < 1e-3);
        let (f32s, _) = leer(&wav(3, 1, 8_000, 32, &0.75f32.to_le_bytes())).unwrap();
        assert!((f32s[0] - 0.75).abs() < 1e-6);
        let (f64s, _) = leer(&wav(3, 1, 8_000, 64, &(-0.25f64).to_le_bytes())).unwrap();
        assert!((f64s[0] + 0.25).abs() < 1e-6);
    }

    #[test]
    fn un_archivo_cortado_se_lee_hasta_donde_llega() {
        let mut bytes = escribir_mono16(&[0.5; 100], 48_000);
        bytes.truncate(44 + 20);
        assert_eq!(leer(&bytes).unwrap().0.len(), 10);
    }

    #[test]
    fn lo_que_no_es_un_wav_de_sonido_se_dice() {
        assert!(leer(b"ID3\x04 esto es un mp3").is_err());
        assert!(leer(&[]).is_err());
        // ADPCM (2): un WAV, pero comprimido.
        let error = leer(&wav(2, 1, 8_000, 4, &[0; 8])).unwrap_err().to_string();
        assert!(error.contains("comprimido"), "{error}");
    }

    #[test]
    fn cambiar_de_frecuencia_mantiene_lo_que_dura() {
        let rampa: Vec<f32> = (0..441).map(|i| i as f32 / 441.0).collect();
        let a48 = a_frecuencia(&rampa, 44_100, 48_000);
        assert_eq!(a48.len(), 480, "diez milisegundos siguen siendo diez");
        assert!((a48[240] - 0.5).abs() < 0.01, "a mitad, la mitad: {}", a48[240]);
        assert_eq!(a_frecuencia(&rampa, 48_000, 48_000), rampa);
    }
}
