//! El sonido de los clics, mezclado en el audio al exportar (C3).
//!
//! **Hecho con unas cuantas muestras, aquí mismo.** Un sonido grabado lleva su licencia y
//! sus bytes: este no lleva ninguna de las dos cosas, sale igual en todas las máquinas y se
//! puede probar con un `assert`. Es lo que suena un ratón: un golpe seco al pulsar y otro
//! más flojo al soltar, cada uno con un chasquido corto, un tono que se apaga enseguida y un
//! poco de cuerpo por debajo.
//!
//! Suena donde suena la imagen: un clic dentro de un trozo cortado no suena, y dentro de un
//! trozo acelerado tampoco, porque esos trozos van en silencio (decisión de la línea de
//! tiempo, `exporter::pista_de_trozos`). Y si la grabación no tiene sonido, se crea la pista:
//! quien pone el sonido de clic quiere oírlo.

use crate::encode::mp4::Pista;
use crate::encode::zoom::Clic;

/// La frecuencia de la pista que se crea cuando la grabación no trae sonido.
pub const FRECUENCIA: u32 = 48_000;
/// Hasta dónde llega el clic con el volumen a tope, sobre el máximo de la pista: deja aire
/// para la voz que suene a la vez.
pub const PICO: f32 = 0.6;
/// Lo que separa el golpe de pulsar del de soltar.
pub const SUELTA_MS: f32 = 65.0;

/// El clic, de -1 a 1 y en mono, a esa frecuencia. Dura algo más de una décima.
pub fn sonido(frecuencia: u32) -> Vec<f32> {
    let sr = frecuencia.max(8_000) as f32;
    let largo = (sr * 0.11) as usize;
    let mut salida = vec![0.0f32; largo];
    // Ruido de siempre el mismo: con la misma semilla, el mismo clic en todas las máquinas.
    let mut semilla: u32 = 0x2545_F491;
    let ruido: Vec<f32> = (0..largo)
        .map(|_| {
            semilla ^= semilla << 13;
            semilla ^= semilla >> 17;
            semilla ^= semilla << 5;
            (semilla as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect();
    let mut golpe = |empieza_s: f32, fuerza: f32, tono: f32| {
        let inicio = (empieza_s * sr) as usize;
        let tau = std::f32::consts::TAU;
        for (i, muestra) in salida.iter_mut().enumerate().skip(inicio) {
            let t = (i - inicio) as f32 / sr;
            // El chasquido: ruido sin graves (cada muestra menos la anterior), muy corto.
            let chasquido = (ruido[i] - ruido[i.saturating_sub(1)]) * 0.5 * (-t / 0.0018).exp();
            let tono_alto = (tau * tono * t).sin() * (-t / 0.006).exp()
                + 0.5 * (tau * tono * 1.9 * t).sin() * (-t / 0.003).exp();
            let cuerpo = 0.35 * (tau * 170.0 * t).sin() * (-t / 0.010).exp();
            *muestra += fuerza * (chasquido + 0.6 * tono_alto + cuerpo);
        }
    };
    golpe(0.0, 1.0, 2_100.0);
    golpe(SUELTA_MS / 1000.0, 0.55, 2_600.0);
    let maximo = salida.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if maximo > 0.0 {
        for x in &mut salida {
            *x /= maximo;
        }
    }
    salida
}

/// Dónde cae cada clic en el vídeo exportado, en milisegundos desde su principio.
///
/// `trozos` son los que se quedan, cada uno `(desde, hasta, velocidad)` en el reloj de la
/// grabación, y `duraciones` lo que dura cada uno en el vídeo (`Pasos::duraciones`). Solo
/// suenan los clics de un trozo a su velocidad de verdad.
pub fn instantes(clics: &[Clic], trozos: &[(u64, u64, f32)], duraciones: &[u64]) -> Vec<u64> {
    let mut salen = Vec::new();
    let mut hecho = 0u64;
    for (&(desde, hasta, velocidad), &dura) in trozos.iter().zip(duraciones) {
        if (velocidad - 1.0).abs() < 0.001 {
            salen.extend(
                clics
                    .iter()
                    .filter(|c| c.ms >= desde && c.ms < hasta && c.ms - desde < dura)
                    .map(|c| hecho + (c.ms - desde)),
            );
        }
        hecho += dura;
    }
    salen.sort_unstable();
    salen
}

/// La pista con el sonido de los clics encima, a ese volumen (de 0 a 1).
///
/// Con el volumen a cero, o sin ningún clic que suene, devuelve `voz` tal cual: el vídeo sale
/// igual que sin esto, byte a byte. Sin `voz` (la grabación no trae sonido, o se quitó), crea
/// una pista en silencio de lo que dura el vídeo y pone los clics en ella.
pub fn mezclar(
    voz: Option<Pista>,
    clics: &[Clic],
    trozos: &[(u64, u64, f32)],
    duraciones: &[u64],
    volumen: f32,
) -> Option<Pista> {
    let volumen = if volumen.is_finite() { volumen.clamp(0.0, 1.0) } else { 0.0 };
    if volumen <= 0.0 {
        return voz;
    }
    let cuando = instantes(clics, trozos, duraciones);
    if cuando.is_empty() {
        return voz;
    }
    let mut pista = voz.unwrap_or_else(|| {
        let total_ms: u64 = duraciones.iter().sum();
        let muestras = total_ms * u64::from(FRECUENCIA) / 1000;
        Pista {
            channels: 2,
            sample_rate: FRECUENCIA,
            datos: vec![0u8; (muestras * 4) as usize],
        }
    });
    let canales = usize::from(pista.channels.max(1));
    let bloque = canales * 2;
    let muestras_total = pista.datos.len() / bloque;
    let clic = sonido(pista.sample_rate);
    let fuerza = volumen * PICO * i16::MAX as f32;
    for ms in cuando {
        let inicio = (ms * u64::from(pista.sample_rate) / 1000) as usize;
        for (k, s) in clic.iter().enumerate() {
            let i = inicio + k;
            if i >= muestras_total {
                break;
            }
            let suma = (s * fuerza).round();
            for c in 0..canales {
                let at = i * bloque + c * 2;
                let antes = i16::from_le_bytes([pista.datos[at], pista.datos[at + 1]]) as f32;
                let ahora = (antes + suma).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                pista.datos[at..at + 2].copy_from_slice(&ahora.to_le_bytes());
            }
        }
    }
    Some(pista)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn clic(ms: u64) -> Clic {
        Clic { ms, x: 0, y: 0, derecho: false }
    }

    /// La energía de una pista entre dos instantes, en el canal izquierdo.
    fn energia(p: &Pista, desde_ms: u64, hasta_ms: u64) -> f64 {
        let bloque = usize::from(p.channels) * 2;
        let a = (desde_ms * u64::from(p.sample_rate) / 1000) as usize;
        let b = ((hasta_ms * u64::from(p.sample_rate) / 1000) as usize).min(p.datos.len() / bloque);
        (a..b)
            .map(|i| {
                let v = i16::from_le_bytes([p.datos[i * bloque], p.datos[i * bloque + 1]]) as f64;
                v * v
            })
            .sum()
    }

    fn pico(s: &[f32]) -> f32 {
        s.iter().fold(0.0f32, |m, x| m.max(x.abs()))
    }

    #[test]
    fn el_clic_es_corto_suena_y_sale_siempre_igual() {
        let a = sonido(48_000);
        assert_eq!(a.len(), 5_280, "dura 110 ms");
        assert!((pico(&a) - 1.0).abs() < 1e-6);
        assert_eq!(a, sonido(48_000), "el mismo clic en todas las máquinas");
        // Lo fuerte está al principio: el golpe de pulsar. De 40 a 60 ms ya casi no queda nada.
        let tarde = pico(&a[48 * 40..48 * 60]);
        assert!(tarde < 0.05, "a los 40 ms sigue sonando: {tarde}");
        // Y el de soltar, a los 65 ms, se oye pero más flojo.
        let suelta = pico(&a[48 * 65..48 * 80]);
        assert!(suelta > 0.3 && suelta < 0.9, "el de soltar: {suelta}");
    }

    #[test]
    fn a_volumen_cero_la_voz_no_cambia_ni_un_byte() {
        let voz = || Pista { channels: 2, sample_rate: 44_100, datos: vec![7u8; 44_100 * 4] };
        let sale = mezclar(Some(voz()), &[clic(100)], &[(0, 1_000, 1.0)], &[1_000], 0.0).unwrap();
        assert_eq!(sale.datos, voz().datos);
        assert!(mezclar(None, &[clic(100)], &[(0, 1_000, 1.0)], &[1_000], 0.0).is_none());
        // Ni sin clics que suenen.
        let sin = mezclar(Some(voz()), &[], &[(0, 1_000, 1.0)], &[1_000], 0.5).unwrap();
        assert_eq!(sin.datos, voz().datos);
    }

    #[test]
    fn sin_sonido_grabado_se_crea_la_pista_con_el_clic_en_su_sitio() {
        let p = mezclar(None, &[clic(500)], &[(0, 2_000, 1.0)], &[2_000], 0.3).unwrap();
        assert_eq!((p.channels, p.sample_rate), (2, FRECUENCIA));
        assert_eq!(p.datos.len(), 2 * 48_000 * 4, "dura lo que el vídeo");
        assert_eq!(energia(&p, 0, 490), 0.0, "antes del clic, silencio");
        assert!(energia(&p, 500, 530) > 0.0, "en el clic suena");
        assert_eq!(energia(&p, 700, 2_000), 0.0, "y después se calla");
        // Nace bajo: a 0,3 el pico no pasa de 0,3 · 0,6 del máximo.
        let pico = p
            .datos
            .chunks_exact(2)
            .map(|m| i16::from_le_bytes([m[0], m[1]]).unsigned_abs())
            .max()
            .unwrap();
        assert!(f32::from(pico) <= 0.3 * PICO * 32_767.0 + 1.0, "pico {pico}");
    }

    #[test]
    fn un_clic_cortado_o_acelerado_no_suena_y_los_demas_se_corren() {
        // Se queda de 0 a 1 s (clic a 0,5 s), se corta de 1 a 3 s (clic a 2 s), de 3 a 5 s va
        // al doble (clic a 4 s, dura 1 s en el vídeo), y de 5 a 6 s a su velocidad (clic a 5,2 s).
        let clics = [clic(500), clic(2_000), clic(4_000), clic(5_200)];
        let trozos = [(0, 1_000, 1.0), (3_000, 5_000, 2.0), (5_000, 6_000, 1.0)];
        let duraciones = [1_000, 1_000, 1_000];
        assert_eq!(instantes(&clics, &trozos, &duraciones), vec![500, 2_200]);
    }

    #[test]
    fn se_suma_a_la_voz_sin_dar_la_vuelta() {
        // Una voz al tope: el clic satura en vez de pasarse al otro lado, que sonaría a chasquido.
        let voz = Pista { channels: 1, sample_rate: 44_100, datos: i16::MAX.to_le_bytes().repeat(44_100) };
        let p = mezclar(Some(voz), &[clic(100)], &[(0, 1_000, 1.0)], &[1_000], 1.0).unwrap();
        assert_eq!(p.datos.len(), 44_100 * 2, "la voz no cambia de largo");
        let minimo = p.datos.chunks_exact(2).map(|m| i16::from_le_bytes([m[0], m[1]])).min().unwrap();
        assert!(minimo > 0, "ninguna muestra ha dado la vuelta: {minimo}");
    }
}
