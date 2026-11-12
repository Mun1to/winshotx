//! S11 (pistas): las pruebas del sonido del PC y la voz por separado, al guardar.
//!
//! Las sesiones se fabrican con el mismo escritor de WAV que usa la grabación de verdad
//! (`record::pistas::EscritorWav`), así que lo que escribe la grabación es lo que se lee aquí.

use super::*;
use crate::contrato::{PistasGrabadas, VolumenDePista};
use crate::record::pistas::{EscritorWav, ARCHIVO_MICROFONO, ARCHIVO_PC};
use crate::record::{AudioInfo, FrameCache};

const INFO: AudioInfo = AudioInfo {
    channels: 2,
    sample_rate: 48_000,
};
/// Lo que ocupa un milisegundo a 48 kHz en estéreo de 16 bits.
const POR_MS: usize = 192;

fn pcm(muestras: impl IntoIterator<Item = i16>) -> Vec<u8> {
    muestras.into_iter().flat_map(|m| m.to_le_bytes()).collect()
}

fn muestras(datos: &[u8]) -> Vec<i16> {
    datos
        .chunks_exact(2)
        .map(|m| i16::from_le_bytes([m[0], m[1]]))
        .collect()
}

fn escribir_wav(ruta: &Path, datos: &[u8]) {
    let mut wav = EscritorWav::crear(ruta, INFO).unwrap();
    wav.escribir(datos);
    assert_eq!(wav.cerrar(), datos.len() as u64);
}

/// Una sesión nueva con las dos pistas en disco: lo del PC y lo de la voz, como los deja
/// la grabación (en su WAV cada una, del mismo largo), y una pantalla gris quieta.
fn sesion(nombre: &str, total_ms: u64, pc: &[u8], voz: &[u8]) -> SessionData {
    let dir = std::env::temp_dir().join(format!("winshotx-pistas-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (ancho, alto) = (320u32, 180u32);
    let mut cache = FrameCache::new(&dir).expect("cache");
    let gris: Vec<u8> = (0..ancho * alto)
        .flat_map(|_| [128, 128, 128, 255])
        .collect();
    cache.push_rgba(&gris, ancho, alto, 0).unwrap();
    let mut session = super::tests::session_with(&[]);
    session.frames = cache.finish(total_ms, 30).expect("cerrar la cache");
    session.dir = dir.clone();
    session.width = ancho;
    session.height = alto;
    session.region = crate::capture::Rect {
        x: 0,
        y: 0,
        width: ancho,
        height: alto,
    };
    session.fps = 30;
    session.format = "video".into();
    session.has_audio = true;
    session.audio = Some(INFO);
    escribir_wav(&dir.join(ARCHIVO_PC), pc);
    escribir_wav(&dir.join(ARCHIVO_MICROFONO), voz);
    session.pistas = Some(PistasGrabadas {
        sistema: Some(ARCHIVO_PC.into()),
        microfono: Some(ARCHIVO_MICROFONO.into()),
    });
    session
}

fn peticion(session: &SessionData) -> ExportRequest {
    let mut p = crate::recorder::peticion_por_defecto(session);
    p.format = "mp4".into();
    p.cursor = 0.0;
    p.audio = true;
    p
}

fn pista(encendida: bool, volumen: f32) -> Option<VolumenDePista> {
    Some(VolumenDePista { encendida, volumen })
}

/// Lo que sale al guardar con las dos pistas: la suma, cada una a su volumen; la apagada
/// no suena; y sin decir nada del editor (lo guardado sin editor), las dos a `volume`.
#[test]
fn cada_pista_suena_a_su_volumen() {
    let total = 1_000;
    let session = sesion(
        "volumen",
        total,
        &pcm(vec![1_000; total as usize * POR_MS / 2]),
        &pcm(vec![400; total as usize * POR_MS / 2]),
    );
    let todo = [(0, total, 1.0)];
    let dura = [total];
    let suena =
        |p: &ExportRequest| sonido_de_trozos(&session, &todo, &dura, p).map(|p| muestras(&p.datos));

    let mut p = peticion(&session);
    let de_fabrica = suena(&p).expect("sin pista");
    assert_eq!(
        de_fabrica.len(),
        total as usize * POR_MS / 2,
        "dura lo que el vídeo"
    );
    assert!(
        de_fabrica.iter().all(|&m| m == 1_400),
        "sin tocar nada, las dos como se grabaron"
    );

    p.edicion.sonido_pc = pista(true, 1.0);
    p.edicion.microfono = pista(true, 0.5);
    assert!(
        suena(&p).unwrap().iter().all(|&m| m == 1_200),
        "la voz a la mitad"
    );

    p.edicion.microfono = pista(false, 1.0);
    assert!(
        suena(&p).unwrap().iter().all(|&m| m == 1_000),
        "sin la voz, solo el PC"
    );

    p.edicion.sonido_pc = pista(false, 1.0);
    p.edicion.microfono = pista(true, 2.0);
    assert!(
        suena(&p).unwrap().iter().all(|&m| m == 800),
        "sin el PC, solo la voz, al doble"
    );

    p.edicion.microfono = pista(false, 2.0);
    assert!(suena(&p).is_none(), "las dos apagadas: el vídeo sale mudo");

    p.edicion.sonido_pc = None;
    p.edicion.microfono = None;
    p.volume = 2.0;
    assert!(
        suena(&p).unwrap().iter().all(|&m| m == 2_800),
        "sin decir nada, las dos a `volume`"
    );
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// Con cortes y un trozo acelerado, las dos pistas se cortan por el mismo sitio: la voz no
/// se despega del PC ni de la imagen, y lo acelerado va en silencio en las dos.
#[test]
fn las_dos_pistas_se_cortan_juntas() {
    // Cada milisegundo lleva su número: el PC tal cual, la voz con 10.000 encima.
    let por_ms = |base: i16| {
        pcm((0..4_000i16).flat_map(move |ms| std::iter::repeat_n(base + ms % 250 + 1, POR_MS / 2)))
    };
    let session = sesion("cortes", 4_000, &por_ms(0), &por_ms(10_000));
    let trozos = [(0, 1_000, 1.0), (1_000, 3_000, 2.0), (3_500, 4_000, 1.0)];
    let duraciones = [1_000, 1_000, 500];
    let mut p = peticion(&session);
    p.edicion.sonido_pc = pista(true, 1.0);
    p.edicion.microfono = pista(true, 1.0);
    let sale = muestras(
        &sonido_de_trozos(&session, &trozos, &duraciones, &p)
            .expect("sin pista")
            .datos,
    );
    let _ = std::fs::remove_dir_all(&session.dir);

    let en = |ms: usize| sale[ms * POR_MS / 2];
    assert_eq!(sale.len(), 2_500 * POR_MS / 2, "dura lo que el vídeo");
    assert_eq!(
        en(999),
        10_000 + 2 * (999 % 250 + 1),
        "el primer trozo, las dos a la vez y en su sitio"
    );
    assert!(
        sale[1_000 * POR_MS / 2..2_000 * POR_MS / 2]
            .iter()
            .all(|&m| m == 0),
        "el acelerado, mudo en las dos"
    );
    assert_eq!(
        en(2_000),
        10_000 + 2 * (3_500 % 250 + 1),
        "y el de después, desde el 3,5 s en las dos"
    );
}

/// Una sesión de antes (todo mezclado en `audio.pcm`) se guarda exactamente igual que
/// hasta ahora, aunque el editor mandara volúmenes de pistas que ella no tiene.
#[test]
fn lo_grabado_antes_sale_como_siempre() {
    let mut session = sesion("vieja", 1_000, &[], &[]);
    session.pistas = None;
    let viejo: Vec<u8> = (0..1_000 * POR_MS).map(|i| ((i / 7) % 200) as u8).collect();
    std::fs::write(session.audio_path(), &viejo).unwrap();
    let mut p = peticion(&session);
    p.volume = 0.7;
    p.edicion.microfono = pista(false, 0.0);
    let ahora = sonido_de_trozos(&session, &[(0, 1_000, 1.0)], &[1_000], &p).expect("sin pista");
    let antes = pista_de_trozos(&session, &[(0, 1_000, 1.0)], &[1_000])
        .map(|q| con_volumen(q, 0.7))
        .unwrap();
    let _ = std::fs::remove_dir_all(&session.dir);
    assert_eq!(ahora.datos, antes.datos, "byte a byte lo de siempre");
}

/// Lo guardado sin editor (`recorder::peticion_por_defecto`) lleva las dos pistas tal
/// como se grabaron.
#[test]
fn sin_editor_salen_las_dos() {
    let session = sesion(
        "defecto",
        500,
        &pcm(vec![3; 500 * POR_MS / 2]),
        &pcm(vec![4; 500 * POR_MS / 2]),
    );
    let p = crate::recorder::peticion_por_defecto(&session);
    let sale = sonido_de_trozos(&session, &[(0, 500, 1.0)], &[500], &p).map(|q| muestras(&q.datos));
    let _ = std::fs::remove_dir_all(&session.dir);
    assert!(p.audio, "lo guardado sin editor lleva el sonido que haya");
    assert!(
        sale.expect("sin pista").iter().all(|&m| m == 7),
        "el PC y la voz, enteros"
    );
}

// --- De punta a punta, por `escribir` hasta el MP4 de verdad ---

fn hay_ffmpeg() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok()
}

/// Un tono de esa frecuencia, igual en los dos canales, de `segundos` segundos.
fn tono(hz: f32, amplitud: f32, segundos: f32) -> Vec<u8> {
    let n = (INFO.sample_rate as f32 * segundos) as usize;
    pcm((0..n).flat_map(|i| {
        let m = (amplitud
            * (std::f32::consts::TAU * hz * i as f32 / INFO.sample_rate as f32).sin()
            * 32_767.0) as i16;
        [m, m]
    }))
}

/// Cuánto suena esa frecuencia en el sonido del MP4, de 0 a 1 (Goertzel sobre el segundo
/// del medio, en mono: lejos del arranque del AAC y del final).
fn cuanto_suena(mp4: &Path, hz: f32) -> f32 {
    let salida = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(mp4)
        .args([
            "-map", "0:a", "-ac", "1", "-ar", "48000", "-f", "s16le", "-",
        ])
        .output()
        .expect("ffmpeg");
    let todo = muestras(&salida.stdout);
    assert!(
        todo.len() > 96_000,
        "el MP4 trae {} muestras de sonido y tenía que traer dos segundos",
        todo.len()
    );
    let trozo = &todo[todo.len() / 2 - 24_000..todo.len() / 2 + 24_000];
    let w = std::f32::consts::TAU * hz / 48_000.0;
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for &m in trozo {
        let s = m as f32 / 32_768.0 + 2.0 * w.cos() * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    let potencia = s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2;
    2.0 * potencia.sqrt() / trozo.len() as f32
}

/// Por el camino de «Guardar» hasta el MP4: el PC a 440 Hz y la voz a 1.000 Hz, cada uno en
/// su pista. Apagar la voz la quita del archivo y deja el PC; bajar el PC a la mitad lo deja
/// a la mitad sin tocar la voz. Es lo que no se podía hacer con todo mezclado al grabar.
///
/// `cargo test --lib el_mp4_lleva_cada_pista_a_su_volumen -- --ignored --nocapture`
#[test]
#[ignore = "necesita el codificador de Windows y ffmpeg: se corre a mano"]
fn el_mp4_lleva_cada_pista_a_su_volumen() {
    if !hay_ffmpeg() {
        eprintln!("sin ffmpeg en el PATH no se puede abrir el MP4: no compruebo nada");
        return;
    }
    let session = sesion(
        "mp4",
        3_000,
        &tono(440.0, 0.3, 3.0),
        &tono(1_000.0, 0.3, 3.0),
    );
    let guardar = |p: &ExportRequest, nombre: &str| {
        let destino = session.dir.join(format!("{nombre}.mp4"));
        escribir(&session, p, &destino, &|_, _, _| {}).expect("exportar a MP4 ha fallado");
        (
            cuanto_suena(&destino, 440.0),
            cuanto_suena(&destino, 1_000.0),
        )
    };
    let mut p = peticion(&session);
    let las_dos = guardar(&p, "las-dos");
    p.edicion.sonido_pc = pista(true, 1.0);
    p.edicion.microfono = pista(false, 1.0);
    let sin_voz = guardar(&p, "sin-voz");
    p.edicion.sonido_pc = pista(true, 0.5);
    p.edicion.microfono = pista(true, 1.0);
    let pc_a_la_mitad = guardar(&p, "pc-mitad");
    let _ = std::fs::remove_dir_all(&session.dir);
    println!("(PC a 440 Hz, voz a 1000 Hz): las dos {las_dos:.3?}, sin voz {sin_voz:.3?}, PC a la mitad {pc_a_la_mitad:.3?}");

    let cerca = |medido: f32, quiere: f32| (medido - quiere).abs() < 0.04;
    assert!(
        cerca(las_dos.0, 0.3) && cerca(las_dos.1, 0.3),
        "las dos tenían que sonar enteras: {las_dos:?}"
    );
    assert!(
        cerca(sin_voz.0, 0.3) && sin_voz.1 < 0.01,
        "sin voz tenía que quedar solo el PC: {sin_voz:?}"
    );
    assert!(
        cerca(pc_a_la_mitad.0, 0.15) && cerca(pc_a_la_mitad.1, 0.3),
        "el PC a la mitad y la voz entera: {pc_a_la_mitad:?}"
    );
}
