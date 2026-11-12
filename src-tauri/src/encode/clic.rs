//! El sonido de los clics, mezclado en el audio al exportar (C3).
//!
//! **Hecho con unas cuantas muestras, aquí mismo.** Un sonido grabado lleva su licencia y
//! sus bytes: estos no llevan ninguna de las dos cosas, salen igual en todas las máquinas y
//! se pueden probar con un `assert`. Son los tres que eligió Munito el 12 de noviembre de
//! 2026 de entre cinco (el de antes, con un pitido agudo encima, «no me gusta»): el «Ratón
//! suave» de fábrica, el «Ratón grave» y el «Toque». Y además, uno propio del usuario, que
//! se copia a la carpeta de la app (`sonidos.rs`) y se lee de ahí.
//!
//! Suena donde suena la imagen: un clic dentro de un trozo cortado no suena, y dentro de un
//! trozo acelerado tampoco, porque esos trozos van en silencio (decisión de la línea de
//! tiempo, `exporter::pista_de_trozos`). Y si la grabación no tiene sonido, se crea la pista:
//! quien pone el sonido de clic quiere oírlo.
//!
//! La vista previa del editor suena con estas mismas muestras: se las pide a Rust
//! (`sonidos::muestras_de_clic`) en vez de tener otra copia de las fórmulas en el editor.

use std::path::Path;

use crate::contrato::ClicElegido;
use crate::encode::mp4::Pista;
use crate::encode::zoom::Clic;

/// La frecuencia de la pista que se crea cuando la grabación no trae sonido.
pub const FRECUENCIA: u32 = 48_000;
/// Hasta dónde llega el clic con el volumen a tope, sobre el máximo de la pista: deja aire
/// para la voz que suene a la vez.
pub const PICO: f32 = 0.6;

/// El clic elegido, de -1 a 1, en mono y a esa frecuencia, con el pico en 1.
///
/// Sin elegir (`None`) es el «Ratón suave». Un propio que no se puede leer (se borró la
/// carpeta de la app, o el archivo está roto) suena como el suave, y se dice en el registro:
/// quedarse sin vídeo por un sonido de clic sería mucho peor.
pub fn sonido(elegido: Option<&ClicElegido>, frecuencia: u32) -> Vec<f32> {
    let frecuencia = frecuencia.max(8_000);
    let sr = f64::from(frecuencia);
    let mut salida = match elegido {
        None | Some(ClicElegido::Suave) => suave(sr),
        Some(ClicElegido::Grave) => grave(sr),
        Some(ClicElegido::Toque) => toque(sr),
        Some(ClicElegido::Propio { ruta }) => match propio(Path::new(ruta), frecuencia) {
            Ok(muestras) => muestras,
            Err(error) => {
                eprintln!("[winshotx] el sonido de clic propio no se puede leer ({error}): suena el suave");
                suave(sr)
            }
        },
    };
    normalizar(&mut salida);
    salida
}

/// Un sonido propio ya guardado por `sonidos::importar_sonido_de_clic`, a esa frecuencia.
/// Allí ya se dejó sin el silencio del principio y con su largo máximo.
pub fn propio(ruta: &Path, frecuencia: u32) -> crate::error::Result<Vec<f32>> {
    let (muestras, de) = crate::encode::wav::leer(&std::fs::read(ruta)?)?;
    if !muestras.iter().any(|m| *m != 0.0) {
        return Err("el sonido está en silencio".into());
    }
    Ok(crate::encode::wav::a_frecuencia(&muestras, de, frecuencia))
}

fn normalizar(salida: &mut [f32]) {
    let maximo = salida.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if maximo > 0.0 {
        for x in salida {
            *x /= maximo;
        }
    }
}

/// Ruido de siempre el mismo: con la misma semilla, el mismo clic en todas las máquinas. Las
/// fórmulas que oyó Munito usaban el de numpy; lo que se oye es la forma (los filtros, las
/// envolventes y los tonos), no qué ruido en concreto.
struct Ruido(u32);

impl Ruido {
    fn nuevo() -> Self {
        Self(0x2545_F491)
    }

    fn muestras(&mut self, cuantas: usize) -> Vec<f64> {
        (0..cuantas)
            .map(|_| {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 17;
                self.0 ^= self.0 << 5;
                (f64::from(self.0) / f64::from(u32::MAX)) * 2.0 - 1.0
            })
            .collect()
    }
}

fn env(t: f64, tau: f64) -> f64 {
    (-t / tau).exp()
}

fn seno(f: f64, t: f64) -> f64 {
    (std::f64::consts::TAU * f * t).sin()
}

/// Cada muestra menos la anterior: el ruido sin graves (el `np.diff` con un cero delante).
fn sin_graves(r: &[f64]) -> Vec<f64> {
    (0..r.len()).map(|k| r[k] - if k > 0 { r[k - 1] } else { 0.0 }).collect()
}

/// Un golpe que empieza en `t0` y llega hasta el final, sumado en `salida`. `forma` recibe el
/// ruido de ese golpe ya sin graves, el número de muestra desde que empieza y su tiempo.
fn golpe(salida: &mut [f64], sr: f64, ruido: &mut Ruido, t0: f64, forma: impl Fn(&[f64], usize, f64) -> f64) {
    let i = (t0 * sr) as usize;
    let m = salida.len().saturating_sub(i);
    let hp = sin_graves(&ruido.muestras(m));
    for k in 0..m {
        salida[i + k] += forma(&hp, k, k as f64 / sr);
    }
}

fn a_f32(salida: Vec<f64>) -> Vec<f32> {
    salida.into_iter().map(|x| x as f32).collect()
}

/// Ratón suave: un chasquido seco filtrado, sin pitido, y el de soltar a los 72 ms al 45 %.
fn suave(sr: f64) -> Vec<f32> {
    let mut salida = vec![0.0; (sr * 0.12) as usize];
    let mut ruido = Ruido::nuevo();
    let forma = |fuerza: f64| {
        move |hp: &[f64], k: usize, t: f64| {
            // Sin lo más agudo: la media de cuatro, centrada como `np.convolve(.., "same")`.
            let en = |j: isize| usize::try_from(j).ok().and_then(|j| hp.get(j)).copied().unwrap_or(0.0);
            let k = k as isize;
            let lp = (en(k + 1) + en(k) + en(k - 1) + en(k - 2)) / 4.0;
            fuerza * (lp * env(t, 0.0025) + 0.25 * seno(900.0, t) * env(t, 0.004))
        }
    };
    golpe(&mut salida, sr, &mut ruido, 0.0, forma(1.0));
    golpe(&mut salida, sr, &mut ruido, 0.072, forma(0.45));
    a_f32(salida)
}

/// Ratón grave: más cuerpo (260 Hz), y el de soltar a los 60 ms al 40 %.
fn grave(sr: f64) -> Vec<f32> {
    let mut salida = vec![0.0; (sr * 0.12) as usize];
    let mut ruido = Ruido::nuevo();
    let forma = |fuerza: f64| {
        move |hp: &[f64], k: usize, t: f64| {
            fuerza
                * (0.5 * hp[k] * env(t, 0.0012)
                    + 0.9 * seno(260.0, t) * env(t, 0.012)
                    + 0.35 * seno(1_150.0, t) * env(t, 0.003))
        }
    };
    golpe(&mut salida, sr, &mut ruido, 0.0, forma(1.0));
    golpe(&mut salida, sr, &mut ruido, 0.06, forma(0.4));
    a_f32(salida)
}

/// Toque: un solo «tic» de madera de 60 ms, sin golpe de soltar.
fn toque(sr: f64) -> Vec<f32> {
    let mut salida = vec![0.0; (sr * 0.06) as usize];
    let mut ruido = Ruido::nuevo();
    golpe(&mut salida, sr, &mut ruido, 0.0, |hp, k, t| {
        0.4 * hp[k] * env(t, 0.0008) + seno(1_750.0, t) * env(t, 0.0025) + 0.3 * seno(3_400.0, t) * env(t, 0.0015)
    });
    a_f32(salida)
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

/// La pista con el sonido de los clics encima, a ese volumen (de 0 a 1) y con el sonido
/// `elegido` (`None` es el de fábrica).
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
    elegido: Option<&ClicElegido>,
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
    let clic = sonido(elegido, pista.sample_rate);
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

    /// Lo que suena a una frecuencia, en un trozo: la energía de esa nota sola.
    fn nota(s: &[f32], sr: f64, f: f64) -> f64 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (k, &m) in s.iter().enumerate() {
            let fase = std::f64::consts::TAU * f * k as f64 / sr;
            re += f64::from(m) * fase.cos();
            im -= f64::from(m) * fase.sin();
        }
        (re * re + im * im) / s.len() as f64
    }

    fn de(elegido: ClicElegido) -> Vec<f32> {
        sonido(Some(&elegido), 48_000)
    }

    #[test]
    fn los_tres_de_serie_duran_lo_suyo_y_salen_siempre_igual() {
        for (elegido, largo) in [(ClicElegido::Suave, 5_760), (ClicElegido::Grave, 5_760), (ClicElegido::Toque, 2_880)] {
            let a = de(elegido.clone());
            assert_eq!(a.len(), largo, "{elegido:?}: 120 ms los ratones y 60 el toque");
            assert!((pico(&a) - 1.0).abs() < 1e-6, "{elegido:?} con el pico en 1");
            assert_eq!(a, de(elegido.clone()), "{elegido:?}: el mismo clic en todas las máquinas");
        }
        assert_eq!(sonido(None, 48_000), de(ClicElegido::Suave), "sin elegir, el suave");
        // A otra frecuencia dura lo mismo: se hace con sus muestras, no se estira.
        assert_eq!(sonido(None, 44_100).len(), 5_292);
    }

    #[test]
    fn los_ratones_sueltan_el_boton_y_el_toque_no() {
        let suave = de(ClicElegido::Suave);
        let callado = pico(&suave[48 * 40..48 * 71]);
        assert!(callado < 0.05, "el suave, entre pulsar y soltar: {callado}");
        let suelta = pico(&suave[48 * 72..48 * 82]);
        assert!(suelta > 0.2 && suelta < 0.7, "el suave suelta a los 72 ms, más flojo: {suelta}");

        let grave = de(ClicElegido::Grave);
        let callado = pico(&grave[48 * 45..48 * 59]);
        assert!(callado < 0.05, "el grave, entre pulsar y soltar: {callado}");
        let suelta = pico(&grave[48 * 60..48 * 75]);
        assert!(suelta > 0.2 && suelta < 0.7, "el grave suelta a los 60 ms, más flojo: {suelta}");

        let toque = de(ClicElegido::Toque);
        let cola = pico(&toque[48 * 15..]);
        assert!(cola < 0.02, "el toque es uno solo y se apaga enseguida: {cola}");
    }

    #[test]
    fn el_grave_tiene_cuerpo_y_el_toque_es_de_madera() {
        let primeros = |s: &[f32], ms: usize| s[..48 * ms].to_vec();
        let (suave, grave, toque) = (de(ClicElegido::Suave), de(ClicElegido::Grave), de(ClicElegido::Toque));
        let cuerpo = |s: &[f32]| nota(&primeros(s, 30), 48_000.0, 260.0);
        assert!(cuerpo(&grave) > 10.0 * cuerpo(&suave), "el grave suena a 260 Hz: {} frente a {}", cuerpo(&grave), cuerpo(&suave));
        let madera = |s: &[f32]| nota(&primeros(s, 10), 48_000.0, 1_750.0);
        assert!(madera(&toque) > 5.0 * madera(&grave), "el toque suena a 1750 Hz");
        assert!(madera(&toque) > 5.0 * madera(&suave), "y el suave no lleva ese tono");
    }

    #[test]
    fn uno_propio_suena_tal_cual_y_si_no_se_puede_leer_suena_el_suave() {
        let dir = std::env::temp_dir().join(format!("winshotx-clic-propio-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Un «la» de 200 ms a 44,1 kHz, a media altura.
        let la: Vec<f32> = (0..8_820).map(|k| 0.5 * (std::f32::consts::TAU * 440.0 * k as f32 / 44_100.0).sin()).collect();
        let ruta = dir.join("la.wav");
        std::fs::write(&ruta, crate::encode::wav::escribir_mono16(&la, 44_100)).unwrap();
        let propio = ClicElegido::Propio { ruta: ruta.to_string_lossy().into() };
        let suena = sonido(Some(&propio), 48_000);
        assert!((suena.len() as i64 - 9_600).abs() <= 1, "dura sus 200 ms a 48 kHz: {}", suena.len());
        assert!((pico(&suena) - 1.0).abs() < 1e-3, "con el pico en 1, como los de serie");
        assert!(nota(&suena, 48_000.0, 440.0) > 100.0 * nota(&suena, 48_000.0, 1_000.0), "es su «la»");

        let roto = ClicElegido::Propio { ruta: dir.join("no-esta.wav").to_string_lossy().into() };
        assert_eq!(sonido(Some(&roto), 48_000), de(ClicElegido::Suave));
        std::fs::write(dir.join("mal.wav"), b"esto no es un wav").unwrap();
        let malo = ClicElegido::Propio { ruta: dir.join("mal.wav").to_string_lossy().into() };
        assert_eq!(sonido(Some(&malo), 48_000), de(ClicElegido::Suave));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_pista_lleva_el_sonido_elegido() {
        let con = |elegido: Option<ClicElegido>| {
            mezclar(None, &[clic(500)], &[(0, 2_000, 1.0)], &[2_000], 0.5, elegido.as_ref()).unwrap()
        };
        let (suave, toque) = (con(None), con(Some(ClicElegido::Toque)));
        assert_ne!(suave.datos, toque.datos);
        assert!(energia(&suave, 572, 600) > 0.0, "el suave suelta a los 72 ms del clic");
        assert_eq!(energia(&toque, 561, 2_000), 0.0, "el toque dura 60 ms y no suelta");
        assert_eq!(con(Some(ClicElegido::Suave)).datos, suave.datos, "sin elegir es el suave");
    }

    /// Para oír lo que sale de aquí, sin abrir el editor: deja los tres en WAV en el temporal.
    ///
    ///     cargo test --lib escribe_los_clics_para_oirlos -- --ignored --nocapture
    #[test]
    #[ignore = "escribe archivos para oírlos: se corre a mano"]
    fn escribe_los_clics_para_oirlos() {
        let dir = std::env::temp_dir().join("winshotx-clics");
        std::fs::create_dir_all(&dir).unwrap();
        for (nombre, elegido) in [("suave", ClicElegido::Suave), ("grave", ClicElegido::Grave), ("toque", ClicElegido::Toque)] {
            let ruta = dir.join(format!("{nombre}.wav"));
            std::fs::write(&ruta, crate::encode::wav::escribir_mono16(&de(elegido), 48_000)).unwrap();
            eprintln!("{}", ruta.display());
        }
    }

    #[test]
    fn a_volumen_cero_la_voz_no_cambia_ni_un_byte() {
        let voz = || Pista { channels: 2, sample_rate: 44_100, datos: vec![7u8; 44_100 * 4] };
        let sale = mezclar(Some(voz()), &[clic(100)], &[(0, 1_000, 1.0)], &[1_000], 0.0, None).unwrap();
        assert_eq!(sale.datos, voz().datos);
        assert!(mezclar(None, &[clic(100)], &[(0, 1_000, 1.0)], &[1_000], 0.0, None).is_none());
        // Ni sin clics que suenen.
        let sin = mezclar(Some(voz()), &[], &[(0, 1_000, 1.0)], &[1_000], 0.5, None).unwrap();
        assert_eq!(sin.datos, voz().datos);
    }

    #[test]
    fn sin_sonido_grabado_se_crea_la_pista_con_el_clic_en_su_sitio() {
        let p = mezclar(None, &[clic(500)], &[(0, 2_000, 1.0)], &[2_000], 0.3, None).unwrap();
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
        let p = mezclar(Some(voz), &[clic(100)], &[(0, 1_000, 1.0)], &[1_000], 1.0, None).unwrap();
        assert_eq!(p.datos.len(), 44_100 * 2, "la voz no cambia de largo");
        let minimo = p.datos.chunks_exact(2).map(|m| i16::from_le_bytes([m[0], m[1]])).min().unwrap();
        assert!(minimo > 0, "ninguna muestra ha dado la vuelta: {minimo}");
    }
}
