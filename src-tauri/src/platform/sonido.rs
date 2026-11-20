//! El clic de obturador, que suena al capturar si el ajuste esta encendido.
//!
//! **El WAV va dentro del binario** (`include_bytes!`), no al lado del ejecutable: son 12 KB
//! y asi no hay un archivo suelto que alguien pueda borrar, ni una ruta que resolver en
//! tiempo de ejecucion. El instalador pasa de 2,48 a 2,49 MB.
//!
//! Y suena con `PlaySoundW`, que es lo que trae Windows: reproducir un sonido corto no
//! justifica meter un motor de audio con sus hilos y sus dependencias. A cambio, **solo
//! sabe leer WAV PCM**, y por eso la prueba de aqui abajo comprueba la cabecera: un mp3
//! renombrado a `.wav` no sonaria, y no se enteraria nadie hasta que alguien capturase.
//!
//! **En Linux suena con el GStreamer del sistema** (`en_linux::tocar`), el mismo que ya
//! escribe el MP4: `autoaudiosink` elige la salida que haya, sea PipeWire o PulseAudio. Son
//! los mismos dos sonidos, sin crate nuevo.

/// El sonido, ya recortado a los tres golpes de un obturador de reflex.
const CLIC: &[u8] = include_bytes!("../../assets/obturador.wav");

/// Suena el obturador, sin esperar a que termine.
///
/// No devuelve error a proposito: que no suene un clic no puede estropear una captura, y
/// una captura que falla porque el audio esta ocupado seria mucho peor que el silencio.
pub fn obturador() {
    #[cfg(windows)]
    {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
        use windows::core::PCWSTR;

        // SND_MEMORY hace que el primer parametro sea un puntero a los bytes del WAV y no
        // una ruta. El buffer es `'static`, asi que sigue vivo mientras suena en su hilo.
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(CLIC.as_ptr() as *const u16),
                None,
                // NODEFAULT: si el WAV no se puede tocar, silencio. Sin esto Windows suelta
                // su pitido de sistema, que es peor que no sonar nada.
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }
    #[cfg(target_os = "linux")]
    {
        // En su hilo: tocar espera a que el sonido acabe, y capturar no espera a nadie.
        let _ = std::thread::Builder::new().name("obturador".into()).spawn(|| {
            let sonado = crate::encode::wav::leer(CLIC)
                .map_err(|error| error.to_string())
                .and_then(|(muestras, frecuencia)| en_linux::tocar(&muestras, frecuencia));
            if let Err(error) = sonado {
                eprintln!("[sonido] el obturador no suena: {error}");
            }
        });
    }
}

/// S45 (Linux): sonar con el GStreamer del sistema.
#[cfg(target_os = "linux")]
mod en_linux {
    use gst::prelude::*;
    use gstreamer as gst;
    use gstreamer_app as gst_app;

    use crate::encode::gst as motor;

    /// Lo que se espera, ademas de lo que dura el sonido, a que la salida diga que ha acabado.
    /// Con un servidor de sonido que no contesta, esto es todo lo que se pierde.
    const DE_MAS_MS: u64 = 2_000;

    /// Toca un sonido corto (un canal, de -1 a 1) por la salida de sonido del sistema y
    /// vuelve cuando ha acabado de sonar. `autoaudiosink` elige por donde: `pulsesink` donde
    /// haya PulseAudio o el PipeWire que contesta por el, y si no ALSA. Sin ninguna salida
    /// (o sin el plugin), devuelve el porque y no suena nada: quien llama sigue igual.
    pub(super) fn tocar(muestras: &[f32], frecuencia: u32) -> std::result::Result<(), String> {
        let mal = |error: crate::error::AppError| error.to_string();
        let tuberia = motor::monta(&format!(
            "appsrc name=a format=time caps=audio/x-raw,format=F32LE,rate={frecuencia},channels=1,layout=interleaved \
             ! audioconvert ! audioresample ! autoaudiosink"
        ))
        .map_err(mal)?;
        let fuente: gst_app::AppSrc = motor::pieza(&tuberia, "a").map_err(mal)?;
        let bus = tuberia.bus().ok_or("la tubería no tiene por dónde avisar")?;
        let dura_ms = muestras.len() as u64 * 1000 / u64::from(frecuencia.max(1));
        let antes = std::time::Instant::now();
        let sonar = || -> std::result::Result<(), String> {
            if tuberia.set_state(gst::State::Playing).is_err() {
                return Err(motor::error_de(&bus).unwrap_or_else(|| "no hay por dónde sonar".into()));
            }
            let bytes: Vec<u8> = muestras.iter().flat_map(|m| m.to_le_bytes()).collect();
            let mut bufer = gst::Buffer::from_slice(bytes);
            if let Some(b) = bufer.get_mut() {
                b.set_pts(gst::ClockTime::ZERO);
                b.set_duration(gst::ClockTime::from_mseconds(dura_ms.max(1)));
            }
            fuente.push_buffer(bufer).map_err(|_| "la salida de sonido no acepta nada".to_string())?;
            let _ = fuente.end_of_stream();
            // El final llega cuando la salida ha sacado la ultima muestra, no cuando se le da.
            let fin = bus.timed_pop_filtered(
                gst::ClockTime::from_mseconds(dura_ms + DE_MAS_MS),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            );
            match fin {
                Some(mensaje) => match mensaje.view() {
                    gst::MessageView::Error(e) => Err(e.error().to_string()),
                    _ => Ok(()),
                },
                None => Err("la salida de sonido no ha acabado a tiempo".into()),
            }
        };
        let sonado = sonar();
        let _ = tuberia.set_state(gst::State::Null);
        if sonado.is_ok() {
            eprintln!("[sonido] {dura_ms} ms de sonido, sonados en {} ms", antes.elapsed().as_millis());
        }
        sonado
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Que lo que se empaqueta sea de verdad lo unico que `PlaySoundW` sabe tocar.
    ///
    /// Munir, el 30 de agosto de 2026: *«y no suena ningun sonido xd»*. Entonces era que
    /// nadie leia el ajuste; ahora que si se lee, el riesgo se muda al archivo.
    #[test]
    fn el_clic_es_un_wav_pcm_que_windows_sabe_tocar() {
        assert_eq!(&CLIC[0..4], b"RIFF", "no empieza por RIFF");
        assert_eq!(&CLIC[8..12], b"WAVE", "no es un WAVE");
        assert_eq!(&CLIC[12..16], b"fmt ", "no trae el bloque de formato");

        let leer16 = |i: usize| u16::from_le_bytes([CLIC[i], CLIC[i + 1]]);
        let leer32 = |i: usize| u32::from_le_bytes([CLIC[i], CLIC[i + 1], CLIC[i + 2], CLIC[i + 3]]);

        assert_eq!(leer16(20), 1, "no es PCM sin comprimir");
        assert_eq!(leer16(22), 1, "tiene que ser mono");
        assert_eq!(leer32(24), 44_100, "tiene que ir a 44.100 Hz");
        assert_eq!(leer16(34), 16, "tiene que ser de 16 bits");

        // Y que siga siendo un clic y no una cancion: al capturar, el sonido tiene que
        // haber acabado antes de que el usuario mire el resultado.
        let ms = (CLIC.len() as u64 - 44) * 1000 / (44_100 * 2);
        assert!((80..=300).contains(&ms), "dura {ms} ms, y eso no es un clic");
    }

    /// Y que suene de verdad. Va con `--ignored` porque **hace ruido en el equipo**.
    #[test]
    #[ignore = "suena por los altavoces de quien lo corra"]
    fn suena() {
        obturador();
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
}

// S10 (sonidos): el sonido al empezar a grabar.

/// Lo que se espera después de que acabe el sonido de empezar, antes de abrir la captura de
/// sonido. `PlaySoundW` vuelve cuando Windows ha mezclado lo último, y la captura del sistema
/// graba lo que sale por los altavoces: con este margen, la cola no entra en el vídeo.
pub const MARGEN_AL_EMPEZAR_MS: u64 = 60;

/// La marimba que eligió Munito el 12 de noviembre de 2026 (`fuerte()` de la quinta tanda),
/// de -1 a 1, en mono y a esa frecuencia: dos golpes de madera suave, un la (220 Hz) y un re
/// (294 Hz) a los 90 ms, que se apagan en 0,11 s, sin eco, con el pico en 0,95. Dura 0,69 s.
///
/// Antes, la de la primera tanda (la campanita) le gustó pero la quería «más profesional y
/// sin ser tan aguda»; la marimba con eco, «rápida pero sin eco y más alto el sonido».
pub fn marimba(frecuencia: u32) -> Vec<f32> {
    let sr = f64::from(frecuencia.max(8_000));
    // Cada parcial: (múltiplo de la nota, fuerza, cuánto antes se apaga). Lo agudo cae antes.
    const SUAVE: [(f64, f64, f64); 3] = [(1.0, 1.0, 1.0), (3.93, 0.10, 0.18), (9.2, 0.02, 0.08)];
    let tono = |f: f64, dura: f64, tau: f64, t0: f64| -> (usize, Vec<f64>) {
        let i = (t0 * sr) as usize;
        let n = (sr * (t0 + dura)) as usize;
        let golpe = (0..n.saturating_sub(i))
            .map(|k| {
                let t = k as f64 / sr;
                let s: f64 = SUAVE
                    .iter()
                    .map(|&(m, a, caida)| a * (std::f64::consts::TAU * f * m * t).sin() * (-t / (tau * caida)).exp())
                    .sum();
                // 5 ms de ataque: sin él, el golpe empezaría con un chasquido.
                s * (t / 0.005).min(1.0)
            })
            .collect();
        (i, golpe)
    };
    let notas = [tono(220.0, 0.5, 0.11, 0.0), tono(294.0, 0.6, 0.11 * 1.15, 0.09)];
    let largo = notas.iter().map(|(i, g)| i + g.len()).max().unwrap_or(0);
    let mut salida = vec![0.0f64; largo];
    for (i, golpe) in &notas {
        for (k, m) in golpe.iter().enumerate() {
            salida[i + k] += m;
        }
    }
    let pico = salida.iter().fold(0.0f64, |p, x| p.max(x.abs()));
    salida.into_iter().map(|x| if pico > 0.0 { (x / pico * 0.95) as f32 } else { 0.0 }).collect()
}

/// Lo que se hace con el sonido de empezar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlEmpezar {
    /// El ajuste está apagado.
    Callado,
    /// Suena mientras arranca: sin sonido que grabar no hay nada que lo pueda coger.
    SinEsperar,
    /// Suena entero, y se espera a que acabe antes de abrir nada que lo pueda grabar: el del
    /// sistema por la captura de los altavoces, y el micro porque lo oye por ellos.
    Esperando,
}

pub fn al_empezar(encendido: bool, graba_sonido: bool) -> AlEmpezar {
    match (encendido, graba_sonido) {
        (false, _) => AlEmpezar::Callado,
        (true, false) => AlEmpezar::SinEsperar,
        (true, true) => AlEmpezar::Esperando,
    }
}

/// El WAV de la marimba, hecho una vez: `PlaySoundW` lo lee de la memoria mientras suena en
/// su hilo, así que tiene que seguir vivo hasta que acabe. En Linux la marimba se toca de sus
/// muestras, sin pasar por un WAV: allí esto solo lo usan las pruebas.
#[cfg(any(windows, test))]
fn wav_de_la_marimba() -> &'static [u8] {
    static WAV: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    WAV.get_or_init(|| crate::encode::wav::escribir_mono16(&marimba(48_000), 48_000))
}

/// Suena la marimba de empezar a grabar, si toca. Con `Esperando` no vuelve hasta que ha
/// acabado (unos 0,75 s), y eso es todo lo que se retrasa el arranque. Como el obturador, no
/// devuelve error: que no suene no puede impedir grabar.
pub fn sonar_al_empezar(cuando: AlEmpezar) {
    if cuando == AlEmpezar::Callado {
        return;
    }
    #[cfg(windows)]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
        let espera = if cuando == AlEmpezar::Esperando { SND_SYNC } else { SND_ASYNC };
        let wav = wav_de_la_marimba();
        unsafe {
            let _ = PlaySoundW(PCWSTR(wav.as_ptr() as *const u16), None, SND_MEMORY | SND_NODEFAULT | espera);
        }
        if cuando == AlEmpezar::Esperando {
            std::thread::sleep(std::time::Duration::from_millis(MARGEN_AL_EMPEZAR_MS));
        }
    }
    #[cfg(target_os = "linux")]
    {
        let sonar = || {
            if let Err(error) = en_linux::tocar(&marimba(48_000), 48_000) {
                eprintln!("[sonido] la marimba no suena: {error}");
            }
        };
        if cuando == AlEmpezar::Esperando {
            // `tocar` vuelve cuando la salida ha sacado la ultima muestra: con el margen de
            // siempre, la cola no entra en lo que se grabe del altavoz.
            sonar();
            std::thread::sleep(std::time::Duration::from_millis(MARGEN_AL_EMPEZAR_MS));
        } else {
            let _ = std::thread::Builder::new().name("marimba".into()).spawn(sonar);
        }
    }
}

#[cfg(test)]
mod pruebas_al_empezar {
    use super::*;

    #[test]
    fn la_marimba_es_un_wav_que_windows_sabe_tocar_y_dura_lo_que_se_espera() {
        let wav = wav_de_la_marimba();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        let (muestras, frecuencia) = crate::encode::wav::leer(wav).unwrap();
        assert_eq!(frecuencia, 48_000);
        assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1, "PCM sin comprimir");
        assert_eq!(muestras.len(), 33_120, "0,69 s: lo que se retrasa el arranque, más el margen");
        let pico = muestras.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((pico - 0.95).abs() < 0.002, "con el pico en 0,95: {pico}");
        // Arranca sin chasquido (5 ms de ataque) y acaba apagada.
        assert!(muestras[..48].iter().all(|m| m.abs() < 0.2), "el ataque");
        let cola = muestras[muestras.len() - 480..].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(cola < 0.02, "al final ya no suena: {cola}");
    }

    #[test]
    fn la_marimba_es_un_la_y_a_los_90_ms_entra_el_re() {
        let s = marimba(48_000);
        let nota = |desde_ms: usize, hasta_ms: usize, f: f64| {
            let trozo = &s[desde_ms * 48..hasta_ms * 48];
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (k, &m) in trozo.iter().enumerate() {
                let fase = std::f64::consts::TAU * f * k as f64 / 48_000.0;
                re += f64::from(m) * fase.cos();
                im -= f64::from(m) * fase.sin();
            }
            (re * re + im * im).sqrt() / trozo.len() as f64
        };
        assert!(nota(10, 80, 220.0) > 10.0 * nota(10, 80, 294.0), "al principio solo el la");
        assert!(nota(100, 170, 294.0) > nota(100, 170, 220.0), "a los 90 ms manda el re");
    }

    #[test]
    fn solo_se_espera_si_se_graba_sonido() {
        assert_eq!(al_empezar(false, true), AlEmpezar::Callado);
        assert_eq!(al_empezar(false, false), AlEmpezar::Callado);
        assert_eq!(al_empezar(true, false), AlEmpezar::SinEsperar);
        assert_eq!(al_empezar(true, true), AlEmpezar::Esperando);
    }

    /// Para oírla sin grabar nada: la deja en `%TEMP%\winshotx-clics\empezar.wav`.
    ///
    ///     cargo test --lib escribe_la_marimba_para_oirla -- --ignored --nocapture
    #[test]
    #[ignore = "escribe un archivo para oírlo: se corre a mano"]
    fn escribe_la_marimba_para_oirla() {
        let dir = std::env::temp_dir().join("winshotx-clics");
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("empezar.wav");
        std::fs::write(&ruta, wav_de_la_marimba()).unwrap();
        eprintln!("{}", ruta.display());
    }

    /// Y que suene de verdad. Va con `--ignored` porque **hace ruido en el equipo**.
    #[test]
    #[ignore = "suena por los altavoces de quien lo corra"]
    fn suena_al_empezar() {
        let antes = std::time::Instant::now();
        sonar_al_empezar(AlEmpezar::Esperando);
        eprintln!("[s10] la marimba ha tardado {} ms", antes.elapsed().as_millis());
    }
}
