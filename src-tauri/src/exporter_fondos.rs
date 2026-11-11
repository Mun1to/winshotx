//! Los fondos propios (S3) de punta a punta: por `escribir`, que es el camino de «Guardar»,
//! y mirando el archivo que se lleva el usuario.
//!
//! Vive en su propio archivo, colgado de `exporter.rs` con un `#[path]`, para que estas
//! pruebas no se mezclen con las de la línea de tiempo, que se escriben a la vez en otra rama.

use super::*;
use crate::contrato::FondoElegido;
use crate::record::FrameCache;

const ANCHO: u32 = 96;
const ALTO: u32 = 64;
const GRIS: [u8; 3] = [128, 128, 128];

/// Una sesión de verdad en una carpeta temporal: UN fotograma gris que dura `total_ms`.
/// Una pantalla quieta, que es justo donde un fondo animado tiene que seguir moviéndose.
fn sesion_quieta(nombre: &str, total_ms: u64) -> SessionData {
    let dir = std::env::temp_dir().join(format!("winshotx-s3-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cache = FrameCache::new(&dir).expect("cache");
    let gris: Vec<u8> = (0..ANCHO * ALTO).flat_map(|_| [GRIS[0], GRIS[1], GRIS[2], 255]).collect();
    cache.push_rgba(&gris, ANCHO, ALTO, 0).unwrap();
    let mut session = super::tests::session_with(&[]);
    session.frames = cache.finish(total_ms, 30).expect("cerrar la cache");
    session.dir = dir;
    session.width = ANCHO;
    session.height = ALTO;
    session.region = crate::capture::Rect { x: 0, y: 0, width: ANCHO, height: ALTO };
    session.fps = 30;
    session
}

fn peticion(session: &SessionData, formato: &str, fondo: FondoElegido) -> ExportRequest {
    let mut peticion = crate::recorder::peticion_por_defecto(session);
    peticion.format = formato.into();
    peticion.fps = 20;
    peticion.cursor = 0.0;
    peticion.audio = false;
    peticion.margin = 10;
    peticion.background = "blanco".into();
    peticion.shadow = false;
    peticion.corners = 0;
    peticion.edicion.fondo = Some(fondo);
    peticion
}

/// Los fotogramas del GIF ya montados, como los ve quien lo abre.
fn fotogramas_del_gif(ruta: &Path) -> Vec<(u16, image::RgbaImage)> {
    let mut opciones = ::gif::DecodeOptions::new();
    opciones.set_color_output(::gif::ColorOutput::RGBA);
    let mut lector = opciones.read_info(std::fs::File::open(ruta).expect("abrir el GIF")).expect("GIF ilegible");
    let (ancho, alto) = (u32::from(lector.width()), u32::from(lector.height()));
    let mut lienzo = image::RgbaImage::new(ancho, alto);
    let mut salen = Vec::new();
    while let Some(f) = lector.read_next_frame().expect("fotograma roto") {
        for y in 0..u32::from(f.height) {
            for x in 0..u32::from(f.width) {
                let i = ((y * u32::from(f.width) + x) * 4) as usize;
                let p = &f.buffer[i..i + 4];
                if p[3] > 0 {
                    lienzo.put_pixel(u32::from(f.left) + x, u32::from(f.top) + y, image::Rgba([p[0], p[1], p[2], 255]));
                }
            }
        }
        salen.push((f.delay, lienzo.clone()));
    }
    salen
}

fn parecido(a: [u8; 3], b: [u8; 3], margen: i32) -> bool {
    (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= margen)
}

fn rgb(p: &image::Rgba<u8>) -> [u8; 3] {
    [p.0[0], p.0[1], p.0[2]]
}

/// B3: un color de la paleta llega al GIF tal cual (con lo que redondea la paleta del GIF).
#[test]
fn un_color_propio_sale_en_el_gif() {
    let session = sesion_quieta("color", 500);
    let destino = session.dir.join("color.gif");
    let pedida = peticion(&session, "gif", FondoElegido::Color { rgb: [12, 200, 99] });
    escribir(&session, &pedida, &destino, &|_, _, _| {}).expect("exportar");
    let salen = fotogramas_del_gif(&destino);
    let _ = std::fs::remove_dir_all(&session.dir);
    let (_, primero) = &salen[0];
    assert_eq!(primero.dimensions(), (ANCHO + 20, ALTO + 20), "el marco crece por fuera");
    assert!(parecido(rgb(primero.get_pixel(1, 1)), [12, 200, 99], 12), "el aire es {:?}", primero.get_pixel(1, 1));
    assert!(parecido(rgb(primero.get_pixel(50, 40)), GRIS, 12), "y la grabación sigue en medio");
}

/// B3: el degradado sale del color elegido y acaba en su segundo color.
#[test]
fn un_degradado_propio_sale_en_el_png() {
    let session = sesion_quieta("degradado", 300);
    let destino = session.dir.join("degradado.png");
    let elegido = [220, 40, 40];
    let mut pedida = peticion(&session, "png", FondoElegido::Degradado { rgb: elegido });
    pedida.margin = 40;
    escribir(&session, &pedida, &destino, &|_, _, _| {}).expect("exportar");
    let imagen = image::open(&destino).expect("releer el PNG").to_rgba8();
    let _ = std::fs::remove_dir_all(&session.dir);
    let (a, b) = imagen.dimensions();
    assert!(parecido(rgb(imagen.get_pixel(0, 0)), elegido, 2), "arriba a la izquierda, el color elegido");
    let fin = rgb(imagen.get_pixel(a - 1, b - 1));
    assert!(parecido(fin, marco::segundo_color(elegido), 4), "abajo a la derecha {fin:?}");
}

/// B1: una imagen propia en el PNG, recortada para llenar.
#[test]
fn una_imagen_propia_sale_en_el_png() {
    let session = sesion_quieta("imagen", 300);
    let ruta = session.dir.join("mi-fondo.png");
    image::RgbaImage::from_fn(300, 100, |x, _| {
        if x < 150 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) }
    })
    .save(&ruta)
    .unwrap();
    let destino = session.dir.join("imagen.png");
    let pedida = peticion(&session, "png", FondoElegido::Imagen { ruta: ruta.to_string_lossy().into() });
    escribir(&session, &pedida, &destino, &|_, _, _| {}).expect("exportar");
    let imagen = image::open(&destino).expect("releer el PNG").to_rgba8();
    let _ = std::fs::remove_dir_all(&session.dir);
    assert_eq!(rgb(imagen.get_pixel(2, 40)), [255, 0, 0], "la izquierda del fondo, roja");
    assert_eq!(rgb(imagen.get_pixel(ANCHO + 17, 40)), [0, 0, 255], "la derecha, azul");
}

/// B5: con la pantalla quieta, el GIF exportado tiene que seguir moviendo el fondo, y
/// volver a empezar cuando el fondo se acaba. Sin el `!animado` de `pasos_de`, la pantalla
/// quieta se juntaba en UN fotograma y el fondo salía congelado.
#[test]
fn un_fondo_animado_se_mueve_en_el_gif_aunque_la_pantalla_este_quieta() {
    let session = sesion_quieta("animado", 1_000);
    let fondo = session.dir.join("fondo.gif");
    // Rojo 100 ms y azul 100 ms (el azul solo en la mitad izquierda, como un GIF de verdad).
    crate::encode::fondo_video::pruebas::gif_de_prueba(&fondo, &[[255, 0, 0], [0, 0, 255]], 16, 16, 10);
    let destino = session.dir.join("animado.gif");
    let pedida = peticion(&session, "gif", FondoElegido::Video { ruta: fondo.to_string_lossy().into() });
    escribir(&session, &pedida, &destino, &|_, _, _| {}).expect("exportar");
    let salen = fotogramas_del_gif(&destino);
    let _ = std::fs::remove_dir_all(&session.dir);
    // El GIF solo escribe los fotogramas que cambian, así que cada uno es un cambio de
    // fondo: rojo, azul, rojo, azul... Cinco vueltas de 200 ms en un segundo.
    let rojo = |c: [u8; 3]| c[0] > 200 && c[1] < 60 && c[2] < 60;
    let azul = |c: [u8; 3]| c[2] > 200 && c[0] < 60 && c[1] < 60;
    let colores: Vec<[u8; 3]> = salen.iter().map(|(_, imagen)| rgb(imagen.get_pixel(1, 1))).collect();
    assert!(colores.iter().all(|&c| rojo(c) || azul(c)), "el aire tiene colores que no son del fondo: {colores:?}");
    let cambios = colores.windows(2).filter(|par| rojo(par[0]) != rojo(par[1])).count();
    assert!(cambios >= 7, "el fondo solo cambió {cambios} veces en un segundo: {colores:?}");
    assert!(rojo(colores[0]), "empieza por el primer fotograma del fondo");
    let dura: u32 = salen.iter().map(|(d, _)| u32::from(*d) * 10).sum();
    assert!((900..=1_100).contains(&dura), "el GIF dura {dura} ms y la grabación 1000");
    // Y la grabación sigue en medio, encima del fondo.
    assert!(parecido(rgb(salen[0].1.get_pixel(50, 40)), GRIS, 12));
}

/// Un MP4 de prueba hecho con el codificador de casa: `fotogramas` a 30 por segundo, cada uno
/// de un color que va girando, para que el fondo cambie de verdad.
#[cfg(windows)]
fn video_de_prueba(ruta: &Path, ancho: u32, alto: u32, fotogramas: usize) {
    let indices: Vec<usize> = (0..fotogramas).collect();
    let retardos = vec![33u32; fotogramas];
    let mut cargar = |i: usize| -> Result<image::RgbaImage> {
        Ok(image::RgbaImage::from_fn(ancho, alto, |x, y| {
            let v = ((x + y) / 8 + i as u32 * 6) % 256;
            if i % 30 < 15 { image::Rgba([230, v as u8, 30, 255]) } else { image::Rgba([30, v as u8, 230, 255]) }
        }))
    };
    mp4::encode(
        &indices,
        &retardos,
        &mut cargar,
        ruta,
        &mp4::Mp4Options { width: ancho, height: alto, fps: 30, quality: 60 },
        None,
        |_, _, _| {},
    )
    .expect("hacer el vídeo de prueba");
}

/// B5 entero con Windows: importar un MP4 (Media Foundation lo lee, nuestro codificador lo
/// escribe), exportar con él de fondo, y leer el MP4 exportado para ver que el fondo cambia.
///
///     cargo test --lib exporter::pruebas_de_fondos -- --ignored --nocapture
#[test]
#[cfg(windows)]
#[ignore = "necesita el codificador y el lector de Windows: se corre a mano"]
fn un_video_importado_se_mueve_en_el_mp4() {
    let session = sesion_quieta("mp4-animado", 2_000);
    let origen = session.dir.join("origen.mp4");
    video_de_prueba(&origen, 320, 180, 60);
    let importado = crate::encode::fondo_video::importar(
        &origen,
        &session.dir.join("fondos").join("video.mp4"),
        &std::sync::atomic::AtomicBool::new(false),
        &mut |_| {},
    )
    .expect("importar");
    assert!(Path::new(&importado.vista).is_file(), "falta la vista del fondo");
    assert_eq!((importado.ancho, importado.alto), (320, 180));
    assert!((1_900..=2_100).contains(&importado.dura_ms), "el fondo dura {} ms", importado.dura_ms);

    let destino = session.dir.join("animado.mp4");
    let mut pedida = peticion(&session, "mp4", FondoElegido::Video { ruta: importado.ruta.clone() });
    pedida.fps = 30;
    escribir(&session, &pedida, &destino, &|_, _, _| {}).expect("exportar");

    // El MP4 exportado, leído con el mismo lector: el aire empieza naranja y a los 0,5 s es azul.
    let mut lector = crate::encode::fondo_video::abrir(&destino).expect("abrir el MP4 exportado");
    let mut colores = Vec::new();
    while let Some(c) = lector.siguiente().expect("leer el MP4") {
        colores.push((c.desde_ms, rgb(c.imagen.get_pixel(2, 2)), c.imagen.dimensions()));
    }
    let _ = std::fs::remove_dir_all(&session.dir);
    assert!(colores.len() >= 50, "una pantalla quieta salió en {} fotogramas", colores.len());
    assert_eq!(colores[0].2, (ANCHO + 20, ALTO + 20));
    let naranja = |c: [u8; 3]| c[0] > 170 && c[2] < 90;
    let azul = |c: [u8; 3]| c[2] > 170 && c[0] < 90;
    assert!(naranja(colores[2].1), "al empezar, el fondo naranja: {:?}", colores[2]);
    let medio = colores.iter().find(|(ms, ..)| *ms >= 700).expect("un fotograma a los 0,7 s");
    assert!(azul(medio.1), "a los 0,7 s, azul: {medio:?}");
}

/// Cancelar a medias no deja nada: ni el `.parcial`, ni la vista, ni el MP4.
#[test]
#[cfg(windows)]
#[ignore = "necesita el codificador y el lector de Windows: se corre a mano"]
fn cancelar_la_importacion_a_medias_no_deja_archivos() {
    let dir = std::env::temp_dir().join(format!("winshotx-s3-cancelar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let origen = dir.join("largo.mp4");
    video_de_prueba(&origen, 640, 360, 150);
    let fondos = dir.join("fondos");
    let cancelar = std::sync::atomic::AtomicBool::new(false);
    let mut avisos = 0;
    let salida = crate::encode::fondo_video::importar(&origen, &fondos.join("video.mp4"), &cancelar, &mut |hecho| {
        avisos += 1;
        assert!((0.0..=1.0).contains(&hecho), "el progreso se sale: {hecho}");
        if avisos == 20 {
            cancelar.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });
    let quedan: Vec<_> = std::fs::read_dir(&fondos).map(|d| d.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    match salida {
        Err(e) => assert_eq!(e.to_string(), crate::encode::fondo_video::CANCELADO),
        Ok(_) => panic!("se canceló y terminó igual"),
    }
    assert!(quedan.is_empty(), "cancelar ha dejado {quedan:?}");
}

/// Lo que cuesta un fondo de vídeo: el tutorial de `medir_un_tutorial` (1080p, 20 s, puntero
/// suave, aire, esquinas y sombra) con un vídeo 1080p de fondo en vez de «noche».
///
///     cargo test --release --lib medir_un_tutorial_con_fondo_de_video -- --ignored --nocapture
#[test]
#[cfg(windows)]
#[ignore = "mide, no comprueba: pide release y el codificador de Windows"]
fn medir_un_tutorial_con_fondo_de_video() {
    use std::time::Instant;
    let (ancho, alto) = (1920u32, 1080u32);
    let dir = std::env::temp_dir().join(format!("winshotx-tutorial-fondo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let origen = dir.join("origen.mp4");
    video_de_prueba(&origen, ancho, alto, 150);
    let empieza = Instant::now();
    let importado = crate::encode::fondo_video::importar(
        &origen,
        &dir.join("fondos").join("video.mp4"),
        &std::sync::atomic::AtomicBool::new(false),
        &mut |_| {},
    )
    .expect("importar");
    let importar_ms = empieza.elapsed().as_millis();

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
    session.cursor = (0..1_250u64)
        .map(|k| {
            let t = k * 16;
            let fase = (t / 2_000) % 2;
            let x = if fase == 0 { 200 + ((t % 2_000) as i32 * 1_400) / 2_000 } else { 1_600 };
            (t, x, 540)
        })
        .collect();
    let mut peticion = crate::recorder::peticion_por_defecto(&session);
    peticion.format = "mp4".into();
    peticion.audio = false;
    peticion.smooth_cursor = true;
    peticion.margin = 96;
    peticion.corners = 23;
    peticion.background = "noche".into();
    peticion.shadow = true;
    peticion.edicion.fondo = Some(FondoElegido::Video { ruta: importado.ruta.clone() });
    let pasos = pasos_de(&session, &peticion);
    let destino = dir.join("tutorial.mp4");
    let empieza = Instant::now();
    escribir(&session, &peticion, &destino, &|_, _, _| {}).expect("exportar");
    let tardo = empieza.elapsed().as_millis();
    let pesa = std::fs::metadata(&destino).map(|m| m.len()).unwrap_or(0);
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "tutorial 1080p de 20 s con fondo de video: importar {importar_ms} ms; exportar {} pasos en {tardo} ms, {:.1} ms por paso, {} KB",
        pasos.indices.len(),
        tardo as f64 / pasos.indices.len().max(1) as f64,
        pesa / 1024
    );
}
