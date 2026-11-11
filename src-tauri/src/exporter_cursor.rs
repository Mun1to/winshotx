//! El cursor y los clics (S7) de punta a punta: por `escribir`, que es el camino de
//! «Guardar», y mirando el archivo que se lleva el usuario.
//!
//! Vive en su propio archivo, colgado de `exporter.rs` con un `#[path]`, como las pruebas de
//! los fondos: así no se mezclan con las de la línea de tiempo ni con las de otras ramas.
//!
//! - C1, el suavizado: «poco» es lo de antes byte a byte, y «mucho» suaviza más y se ciñe al clic.
//! - C2, esconder el puntero al escribir: se va con lo escrito y vuelve después.
//! - C4, la onda: sale por fuera del aro.
//! - A4, la estela: copias del puntero detrás, y la cámara que se mueve deja estela.
//! - A5, acercarse al escribir: con la pista sin tocar se acerca al cursor de texto, y la
//!   pista que lo enseña exporta lo mismo.
//! - C3, el sonido de clic, en un MP4 de verdad (a mano: necesita el codificador de Windows).

use super::la_linea_de_tiempo::{liso, mitades, sesion, ALTO, ANCHO};
use super::*;
use crate::contrato::{Suavizado, TramoEscritura};

const FONDO: [u8; 3] = [235, 235, 235];

/// La petición de las pruebas: un GIF a 20 fotogramas por segundo, con el puntero de 12.
fn peticion(session: &SessionData) -> ExportRequest {
    let mut p = crate::recorder::peticion_por_defecto(session);
    p.format = "gif".into();
    p.fps = 20;
    p.cursor = 12.0;
    p.audio = false;
    p
}

/// Exporta a GIF por `escribir` y devuelve sus bytes y cada fotograma con el instante en
/// que empieza y acaba.
fn gif(session: &SessionData, p: &ExportRequest, nombre: &str) -> (Vec<u8>, Vec<(u64, u64, image::RgbaImage)>) {
    let destino = session.dir.join(format!("{nombre}.gif"));
    escribir(session, p, &destino, &|_, _, _| {}).expect("exportar a GIF ha fallado");
    let bytes = std::fs::read(&destino).unwrap();
    let mut t = 0u64;
    let fotos = super::exportar_de_verdad::fotogramas_del_gif(&destino)
        .into_iter()
        .map(|(retardo, imagen)| {
            let empieza = t;
            t += u64::from(retardo) * 10;
            (empieza, t, imagen)
        })
        .collect();
    (bytes, fotos)
}

fn en(gif: &[(u64, u64, image::RgbaImage)], ms: u64) -> &image::RgbaImage {
    &gif.iter().find(|(d, h, _)| ms >= *d && ms < *h).unwrap_or_else(|| gif.last().unwrap()).2
}

/// Los píxeles del puntero (la flecha es oscura) en una imagen de fondo claro.
fn oscuros(imagen: &image::RgbaImage) -> Vec<(u32, u32)> {
    imagen
        .enumerate_pixels()
        .filter(|(_, _, p)| p.0[0] < 90 && p.0[1] < 90 && p.0[2] < 90)
        .map(|(x, y, _)| (x, y))
        .collect()
}

/// Dónde está la punta de la flecha: su píxel oscuro más a la izquierda.
fn punta(imagen: &image::RgbaImage) -> Option<u32> {
    oscuros(imagen).iter().map(|(x, _)| *x).min()
}

/// Los que no son el fondo, ni la flecha ni su borde blanco: copias a medias o color.
fn tocados(imagen: &image::RgbaImage) -> usize {
    imagen.pixels().filter(|p| (0..3).any(|i| (p.0[i] as i32 - FONDO[i] as i32).abs() > 12)).count()
}

fn clic(ms: u64, x: i32, y: i32) -> crate::encode::zoom::Clic {
    crate::encode::zoom::Clic { ms, x, y, derecho: false }
}

/// Un rastro de ratón: `(ms, x, y)` cada 16 ms, como lo apunta la grabación.
fn rastro(hasta_ms: u64, sitio: impl Fn(u64) -> (i32, i32)) -> Vec<(u64, i32, i32)> {
    (0..=hasta_ms / 16).map(|k| {
        let ms = k * 16;
        let (x, y) = sitio(ms);
        (ms, x, y)
    }).collect()
}

/// C1: «poco» es exactamente el movimiento suave de siempre, y «nada», sin él. Con los
/// campos nuevos apagados el vídeo sale igual, byte a byte.
#[test]
fn poco_es_el_movimiento_suave_de_antes_byte_a_byte() {
    let mut session = sesion("s7-poco", &[(0, liso(FONDO))], 2_000);
    session.cursor = rastro(2_000, |ms| (10 + (ms / 20) as i32, 40));
    let mut antes = peticion(&session);
    antes.smooth_cursor = true;
    let mut ahora = peticion(&session);
    ahora.edicion.suavizado = Some(Suavizado::Poco);
    assert_eq!(gif(&session, &antes, "antes").0, gif(&session, &ahora, "ahora").0);

    antes.smooth_cursor = false;
    ahora.smooth_cursor = true;
    ahora.edicion.suavizado = Some(Suavizado::Nada);
    assert_eq!(gif(&session, &antes, "sin").0, gif(&session, &ahora, "nada").0, "«nada» manda sobre el interruptor");

    // Y todo lo nuevo a cero es lo de antes.
    let mut apagado = peticion(&session);
    apagado.edicion.sonido_clic = 0.0;
    apagado.edicion.estela = 0.0;
    apagado.edicion.onda_clic = false;
    apagado.edicion.esconder_al_escribir = false;
    apagado.edicion.zoom_al_escribir = false;
    assert_eq!(gif(&session, &antes, "otra").0, gif(&session, &apagado, "apagado").0);
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// C1: «mucho» se queda más atrás en un salto, pero en el instante de un clic el puntero
/// está justo donde se pulsó, y «poco» no.
#[test]
fn mucho_suaviza_mas_y_en_el_clic_esta_en_su_sitio() {
    let mut session = sesion("s7-mucho", &[(0, liso(FONDO))], 3_000);
    // Hacia los 1,5 s el ratón salta de 20 a 90 (la muestra de 1.488 ms ya está en 90) y
    // pulsa al llegar.
    session.cursor = rastro(3_000, |ms| (if ms < 1_488 { 20 } else { 90 }, 40));
    session.clics = vec![clic(1_500, 90, 40)];
    let mut poco = peticion(&session);
    poco.edicion.suavizado = Some(Suavizado::Poco);
    let mut mucho = peticion(&session);
    mucho.edicion.suavizado = Some(Suavizado::Mucho);
    let (_, con_poco) = gif(&session, &poco, "poco");
    let (_, con_mucho) = gif(&session, &mucho, "mucho");
    let _ = std::fs::remove_dir_all(&session.dir);

    let p = punta(en(&con_poco, 1_500)).unwrap();
    let m = punta(en(&con_mucho, 1_500)).unwrap();
    assert!((88..=92).contains(&m), "con «mucho», en el clic está en el clic: {m}");
    assert!(p < 80, "con «poco» va por detrás del salto: {p}");
}

/// C2: con lo escrito el puntero se va, con un fundido, y vuelve al acabar. La pantalla
/// está quieta y el ratón también: sin contar el fundido, sería un solo fotograma.
#[test]
fn al_escribir_el_puntero_se_va_y_vuelve() {
    let mut session = sesion("s7-escribir", &[(0, liso(FONDO))], 4_000);
    session.cursor = rastro(4_000, |_| (60, 30));
    session.escritura = vec![TramoEscritura { desde_ms: 1_000, hasta_ms: 2_500, caret: None }];
    let mut p = peticion(&session);
    p.edicion.esconder_al_escribir = true;
    let (_, fotos) = gif(&session, &p, "escribir");
    let mut sin = peticion(&session);
    sin.edicion.esconder_al_escribir = false;
    let (_, sin_esconder) = gif(&session, &sin, "sin");
    let _ = std::fs::remove_dir_all(&session.dir);

    assert!(!oscuros(en(&fotos, 500)).is_empty(), "antes de escribir se ve");
    assert!(oscuros(en(&fotos, 1_800)).is_empty(), "escribiendo no se ve");
    assert!(!oscuros(en(&fotos, 3_500)).is_empty(), "y al acabar vuelve");
    assert!(!oscuros(en(&sin_esconder, 1_800)).is_empty(), "sin pedirlo, no se esconde");
    // El fundido son fotogramas propios, no un corte de golpe: a los 100 ms de empezar a
    // escribir queda un tercio de puntero, que se ve pero ya no es la flecha oscura.
    let medio = en(&fotos, 1_100);
    assert!(tocados(medio) > 0 && oscuros(medio).is_empty(), "a medio irse, a medias");
}

/// C4: la onda sale por fuera del aro, y sin pedirla no hay nada ahí.
#[test]
fn la_onda_sale_por_fuera_del_aro() {
    let mut session = sesion("s7-onda", &[(0, liso(FONDO))], 2_000);
    session.clics = vec![clic(1_000, 60, 40)];
    let mut p = peticion(&session);
    p.cursor = 0.0;
    p.clicks = true;
    let (_, sin) = gif(&session, &p, "sin");
    p.edicion.onda_clic = true;
    let (_, con) = gif(&session, &p, "con");
    let _ = std::fs::remove_dir_all(&session.dir);

    let fuera = |img: &image::RgbaImage| {
        let r = crate::record::realce::radio_maximo(ALTO) + 4.0;
        img.enumerate_pixels()
            .filter(|(x, y, p)| {
                let d = ((*x as f32 - 60.0).powi(2) + (*y as f32 - 40.0).powi(2)).sqrt();
                d > r && p.0[2] > p.0[0] + 20
            })
            .count()
    };
    assert_eq!(fuera(en(&sin, 1_250)), 0, "el aro solo no pasa de su radio");
    assert!(fuera(en(&con, 1_250)) > 20, "la onda sí: {}", fuera(en(&con, 1_250)));
    assert_eq!(fuera(en(&con, 1_800)), 0, "y se acaba");
}

/// A4: con estela, detrás del puntero que corre quedan copias suyas; quieto, ninguna.
#[test]
fn la_estela_deja_copias_detras_del_puntero() {
    let mut session = sesion("s7-estela", &[(0, liso(FONDO))], 2_000);
    // Corre hacia la derecha a 60 píxeles por segundo... del vídeo de 120: rápido.
    session.cursor = rastro(2_000, |ms| (10 + (ms * 6 / 100).min(100) as i32, 40));
    let mut p = peticion(&session);
    let (_, sin) = gif(&session, &p, "sin");
    p.edicion.estela = 1.0;
    let (_, con) = gif(&session, &p, "con");
    let _ = std::fs::remove_dir_all(&session.dir);

    let corriendo = 1_000;
    assert!(
        tocados(en(&con, corriendo)) > tocados(en(&sin, corriendo)) + 10,
        "con estela hay más puntero: {} vs {}",
        tocados(en(&con, corriendo)),
        tocados(en(&sin, corriendo))
    );
    assert_eq!(punta(en(&con, corriendo)), punta(en(&sin, corriendo)), "el puntero sigue en su sitio");
    // Parado (a los 1,9 s ya llegó), la estela no deja nada.
    assert_eq!(tocados(en(&con, 1_950)), tocados(en(&sin, 1_950)), "parado, sin estela");
}

/// A4: la cámara que se acerca deja estela en el borde rojo y azul; quieta encima, no.
#[test]
fn la_camara_que_se_mueve_deja_estela_en_el_gif() {
    let session = sesion("s7-camara", &[(0, mitades())], 3_000);
    let mut p = peticion(&session);
    p.cursor = 0.0;
    p.edicion.zooms = Some(vec![super::la_linea_de_tiempo::zoom_fijo(500, 2_500, 0.3, 0.5)]);
    let (_, sin) = gif(&session, &p, "sin");
    p.edicion.estela = 1.0;
    let (_, con) = gif(&session, &p, "con");
    let _ = std::fs::remove_dir_all(&session.dir);

    // Lo que no es ni rojo ni azul: la mezcla de los dos que deja el borde al moverse.
    let mezcla = |img: &image::RgbaImage| {
        img.pixels().filter(|p| p.0[0] > 70 && p.0[0] < 180 && p.0[2] > 70 && p.0[2] < 180).count()
    };
    let acercandose = 750;
    assert!(
        mezcla(en(&con, acercandose)) > mezcla(en(&sin, acercandose)) + 20,
        "acercándose, el borde se emborrona: {} vs {}",
        mezcla(en(&con, acercandose)),
        mezcla(en(&sin, acercandose))
    );
    assert_eq!(mezcla(en(&con, 1_500)), mezcla(en(&sin, 1_500)), "quieta encima, igual que sin estela");
}

/// A5: con la pista sin tocar, se acerca a donde se escribe (el cursor de texto, en la mitad
/// roja), y la pista que lo enseña, exportada tal cual, da el mismo GIF.
#[test]
fn acercarse_al_escribir_con_la_pista_sin_tocar_y_tocada() {
    let mut session = sesion("s7-acercarse", &[(0, mitades())], 4_000);
    session.cursor = rastro(4_000, |_| (100, 40)); // El ratón, en lo azul.
    session.escritura = vec![TramoEscritura { desde_ms: 1_000, hasta_ms: 2_000, caret: Some((30, 40)) }];
    let mut p = peticion(&session);
    p.cursor = 0.0;
    let (_, sin) = gif(&session, &p, "sin");
    p.edicion.zoom_al_escribir = true;
    let (solo, con) = gif(&session, &p, "con");
    p.edicion.zooms = Some(zooms_con_escritura(&session, p.zoom, p.crop, p.zoom_speed));
    let (pista, _) = gif(&session, &p, "pista");
    let _ = std::fs::remove_dir_all(&session.dir);

    let rojo = |img: &image::RgbaImage| {
        img.pixels().filter(|p| p.0[0] > 150 && p.0[2] < 100).count() as f32 / (ANCHO * ALTO) as f32
    };
    assert!((rojo(en(&sin, 1_500)) - 0.5).abs() < 0.05, "sin pedirlo no se acerca");
    assert!(rojo(en(&con, 1_500)) > 0.95, "escribiendo se acerca al cursor de texto: {}", rojo(en(&con, 1_500)));
    assert!((rojo(en(&con, 3_800)) - 0.5).abs() < 0.05, "y después se aleja");
    assert_eq!(solo, pista, "la pista sin tocar y la misma pista tocada sacan el mismo GIF");
}

/// C3 en un MP4 de verdad: una grabación SIN sonido, con un clic en un trozo que se queda y
/// otro en uno cortado. Sale con pista de sonido, el clic que se queda suena donde cae en el
/// vídeo, y el cortado no suena.
#[test]
#[ignore = "necesita el codificador de Windows y ffmpeg: se corre a mano"]
fn el_clic_suena_en_el_mp4_aunque_la_grabacion_no_tenga_sonido() {
    use crate::contrato::TramoVelocidad;
    use std::process::Command;

    let mut session = sesion("s7-sonido", &[(0, liso(FONDO))], 4_000);
    assert!(session.audio.is_none());
    session.clics = vec![clic(1_000, 60, 40), clic(3_000, 60, 40)];
    let mut p = crate::recorder::peticion_por_defecto(&session);
    p.format = "mp4".into();
    p.cursor = 0.0;
    p.audio = false;
    // Se queda de 0 a 2 s y de 3,5 a 4 s: el clic de 1 s cae en el 1 s del vídeo, y el de
    // 3 s está en lo cortado, así que no suena. El vídeo dura 2,5 s.
    p.edicion.sonido_clic = 0.4;
    p.edicion.tramos = Some(vec![
        TramoVelocidad { desde_ms: 0, hasta_ms: 2_000, velocidad: 1.0 },
        TramoVelocidad { desde_ms: 3_500, hasta_ms: 4_000, velocidad: 1.0 },
    ]);
    let destino = session.dir.join("sonido.mp4");
    escribir(&session, &p, &destino, &|_, _, _| {}).expect("exportar a MP4");

    // El sonido en crudo, a 1 kHz de muestras de energía: una por milisegundo.
    let crudo = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&destino)
        .args(["-vn", "-ac", "1", "-ar", "48000", "-f", "s16le", "-"])
        .output()
        .expect("ffmpeg")
        .stdout;
    let muestras: Vec<i16> = crudo.chunks_exact(2).map(|m| i16::from_le_bytes([m[0], m[1]])).collect();
    let pico = |desde_ms: usize, hasta_ms: usize| {
        muestras
            .get(desde_ms * 48..(hasta_ms * 48).min(muestras.len()))
            .map_or(0, |s| s.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0))
    };
    let duracion = muestras.len() as f64 / 48_000.0;
    eprintln!(
        "[s7] sonido {duracion:.3} s; pico 0-900 ms {}, 990-1100 ms {}, 1200-2490 ms {}",
        pico(0, 900),
        pico(990, 1_100),
        pico(1_200, 2_490)
    );
    let _ = std::fs::remove_dir_all(&session.dir);
    assert!((duracion - 2.5).abs() < 0.1, "el sonido dura lo que el vídeo: {duracion}");
    assert!(pico(990, 1_100) > 3_000, "en el clic suena");
    assert!(pico(0, 900) < 300, "antes, silencio");
    assert!(pico(1_200, 2_490) < 300, "el clic cortado no suena");
}

/// A4: lo que cuesta la estela de la cámara, que es la cara: un tutorial 1080p de 20 s con un
/// clic cada dos segundos y el zoom puesto, así que la cámara se está moviendo casi siempre.
///
///     ESTELA=1 cargo test --release --lib medir_la_estela_de_la_camara -- --ignored --nocapture
#[test]
#[ignore = "mide, no comprueba: pide release y el codificador de Windows"]
fn medir_la_estela_de_la_camara() {
    use crate::record::FrameCache;
    let (ancho, alto) = (1920u32, 1080u32);
    let dir = std::env::temp_dir().join(format!("winshotx-s7-estela-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cache = FrameCache::new(&dir).expect("cache");
    let pagina: Vec<u8> = (0..ancho * alto)
        .flat_map(|i| {
            let (x, y) = (i % ancho, i / ancho);
            let v = if (x / 120 + y / 80) % 2 == 0 { 235 } else { 215 };
            [v, v, v, 255]
        })
        .collect();
    cache.push_rgba(&pagina, ancho, alto, 0).unwrap();
    let mut session = super::tests::session_with(&[]);
    session.frames = cache.finish(20_000, 30).expect("cerrar la cache");
    session.dir = dir.clone();
    session.width = ancho;
    session.height = alto;
    session.region = crate::capture::Rect { x: 0, y: 0, width: ancho, height: alto };
    session.fps = 30;
    session.cursor = rastro(20_000, |ms| (200 + ((ms % 4_000) * 1_400 / 4_000) as i32, 540));
    // Un clic cada dos segundos, cada vez en una punta: la cámara va de un lado a otro.
    session.clics = (0..10).map(|k| clic(1_000 + k * 2_000, if k % 2 == 0 { 300 } else { 1_600 }, 540)).collect();
    let mut p = crate::recorder::peticion_por_defecto(&session);
    p.format = "mp4".into();
    p.audio = false;
    p.zoom = 2.0;
    p.edicion.estela = std::env::var("ESTELA").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let destino = dir.join("estela.mp4");
    let empieza = std::time::Instant::now();
    escribir(&session, &p, &destino, &|_, _, _| {}).expect("exportar");
    let tardo = empieza.elapsed().as_millis();
    let _ = std::fs::remove_dir_all(&dir);
    println!("tutorial 1080p de 20 s con zoom a los clics, estela {}: {tardo} ms", p.edicion.estela);
}
