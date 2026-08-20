//! Pruebas del motor de verdad: captura la pantalla de esta maquina, escribe el
//! cache de fotogramas y exporta GIF y MP4 reales. Si algo de esto falla, la app
//! no funciona por mucho que la interfaz se vea bien.

use std::path::PathBuf;

use image::{Rgba, RgbaImage};
use winshotx_lib::capture::{self, Rect};
use winshotx_lib::encode::{gif as gif_encoder, mp4};
use winshotx_lib::record::{self, FrameCache};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("winshotx-tests").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("no se ha podido crear el directorio de pruebas");
    dir
}

/// Un cuadrado que se desplaza sobre un degradado: cambia poco entre fotogramas,
/// que es justo el caso que tiene que aprovechar el GIF por diferencias.
fn synthetic_frame(index: u32, width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::from_fn(width, height, |x, y| {
        Rgba([(x * 255 / width) as u8, (y * 255 / height) as u8, 90, 255])
    });
    let left = 4 + index * 3;
    for y in 10..30u32 {
        for x in left..left + 20 {
            if x < width && y < height {
                image.put_pixel(x, y, Rgba([250, 250, 250, 255]));
            }
        }
    }
    image
}

#[test]
fn el_cache_deduplica_y_devuelve_los_fotogramas_intactos() {
    let dir = scratch("cache");
    let mut cache = FrameCache::new(&dir).unwrap();
    let (width, height) = (120u32, 80u32);

    let first = synthetic_frame(0, width, height);
    assert!(cache
        .push_rgba(first.as_raw(), width, height, 0)
        .unwrap());
    // El mismo fotograma otra vez no debe ocupar ni un byte mas.
    let bytes_before = cache.bytes_written();
    assert!(!cache
        .push_rgba(first.as_raw(), width, height, 33)
        .unwrap());
    assert_eq!(cache.bytes_written(), bytes_before);

    let second = synthetic_frame(1, width, height);
    assert!(cache
        .push_rgba(second.as_raw(), width, height, 66)
        .unwrap());

    let frames = cache.finish(100, 30).unwrap();
    assert_eq!(frames.len(), 2);
    assert!(frames.iter().all(|f| f.duration_ms >= 10));

    let session = record::SessionData {
        id: "cache".into(),
        dir: dir.clone(),
        region: Rect {
            x: 0,
            y: 0,
            width,
            height,
        },
        fps: 30,
        format: "gif".into(),
        has_audio: false,
        width,
        height,
        mp4_path: None,
        frames,
    };

    let restored = record::read_frame(&session, 1).unwrap();
    assert_eq!(restored.dimensions(), (width, height));
    assert_eq!(restored.as_raw(), second.as_raw(), "QOI no es sin perdida");
}

#[test]
fn exporta_un_gif_valido_y_mas_pequenno_que_los_fotogramas_crudos() {
    let dir = scratch("gif");
    let (width, height) = (160u32, 120u32);
    let frames: Vec<RgbaImage> = (0..12).map(|i| synthetic_frame(i, width, height)).collect();
    let indices: Vec<usize> = (0..frames.len()).collect();
    let delays = vec![40u32; frames.len()];
    let path = dir.join("salida.gif");

    let mut loader = |index: usize| Ok(frames[index].clone());
    gif_encoder::encode(
        &indices,
        &delays,
        &mut loader,
        &path,
        &gif_encoder::GifOptions {
            width,
            height,
            quality: 85,
            loop_forever: true,
        },
        |_stage, _done, _total| {},
    )
    .expect("el codificador de GIF ha fallado");

    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"GIF89a"), "cabecera de GIF invalida");
    let crudo = (width * height * 4 * frames.len() as u32) as usize;
    assert!(bytes.len() < crudo / 4, "el GIF no comprime nada");

    // Se relee con el decodificador para asegurar que no es un archivo roto.
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::Indexed);
    let mut decoder = options
        .read_info(std::fs::File::open(&path).unwrap())
        .expect("el GIF no se puede decodificar");
    let mut count = 0;
    while decoder.read_next_frame().unwrap().is_some() {
        count += 1;
    }
    assert!(count >= 2, "se esperaban varios fotogramas, hay {count}");
}

#[cfg(windows)]
#[test]
fn exporta_un_mp4_que_media_foundation_acepta() {
    let dir = scratch("mp4");
    let (width, height) = (160u32, 120u32);
    let frames: Vec<RgbaImage> = (0..15).map(|i| synthetic_frame(i, width, height)).collect();
    let indices: Vec<usize> = (0..frames.len()).collect();
    let delays = vec![33u32; frames.len()];
    let path = dir.join("salida.mp4");

    let mut loader = |index: usize| Ok(frames[index].clone());
    mp4::encode(
        &indices,
        &delays,
        &mut loader,
        &path,
        &mp4::Mp4Options {
            width,
            height,
            fps: 30,
            quality: 70,
        },
        |_stage, _done, _total| {},
    )
    .expect("el codificador de MP4 ha fallado");

    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() > 1024, "el MP4 esta vacio");
    assert_eq!(&bytes[4..8], b"ftyp", "no parece un contenedor MP4");
}

#[cfg(windows)]
#[test]
fn congela_los_monitores_reales_y_recorta_la_region_pedida() {
    let dir = scratch("freeze");
    let freezes = capture::freeze_all(&dir).expect("no se ha podido capturar la pantalla");
    assert!(!freezes.is_empty(), "no se ha detectado ningun monitor");

    let monitor = &freezes[0].monitor;
    assert!(monitor.width > 0 && monitor.height > 0);
    assert!(freezes[0].path.exists(), "el PNG congelado no se ha escrito");

    let region = Rect {
        x: monitor.x + 10,
        y: monitor.y + 10,
        width: 100,
        height: 60,
    };
    let recorte = capture::crop_from_freeze(&freezes, region).expect("el recorte ha fallado");
    assert_eq!(recorte.dimensions(), (100, 60));
}

/// La prueba que de verdad importa: Windows Graphics Capture entregando
/// fotogramas ya recortados a la region, con el tamanno exacto que se pidio.
#[cfg(windows)]
#[test]
fn la_grabacion_en_vivo_entrega_fotogramas_recortados() {
    use std::sync::atomic::{AtomicBool, AtomicU64};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use winshotx_lib::record::win::{self, CaptureFlags};

    let freezes = capture::freeze_all(&scratch("live")).expect("sin monitores");
    let monitor = freezes[0].monitor.clone();
    let region = Rect {
        x: monitor.x + 80,
        y: monitor.y + 80,
        width: 200,
        height: 120,
    };

    let (sender, receiver) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let control = win::start(
        region,
        (monitor.x, monitor.y),
        false,
        15,
        CaptureFlags {
            sender,
            crop: (0, 0, 0, 0),
            stop: stop.clone(),
            pause: Arc::new(AtomicBool::new(false)),
            paused_ms: Arc::new(AtomicU64::new(0)),
            min_interval_ms: 0,
        },
    )
    .expect("no se ha podido iniciar la captura");

    let deadline = Instant::now() + Duration::from_secs(6);
    let mut received = 0;
    while Instant::now() < deadline && received < 2 {
        if let Ok(frame) = receiver.recv_timeout(Duration::from_millis(400)) {
            assert_eq!(
                frame.bgra.len(),
                (region.width * region.height * 4) as usize,
                "el recorte no tiene el tamanno pedido"
            );
            received += 1;
        }
    }

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = control.stop();
    assert!(received > 0, "no ha llegado ningun fotograma en 6 segundos");
}

/// De la pantalla al archivo: graba un segundo de escritorio, escribe el cache,
/// genera las miniaturas y exporta GIF y MP4. Es el recorrido completo del motor.
#[cfg(windows)]
#[test]
fn de_la_pantalla_al_gif_y_al_mp4() {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use winshotx_lib::record::win::{self, CaptureFlags};

    let dir = scratch("extremo-a-extremo");
    let freezes = capture::freeze_all(&dir.join("freeze")).expect("sin monitores");
    let monitor = freezes[0].monitor.clone();
    let region = Rect {
        x: monitor.x + 40,
        y: monitor.y + 40,
        width: 240,
        height: 160,
    };

    let (sender, receiver) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let control = win::start(
        region,
        (monitor.x, monitor.y),
        false,
        20,
        CaptureFlags {
            sender,
            crop: (0, 0, 0, 0),
            stop: stop.clone(),
            pause: Arc::new(AtomicBool::new(false)),
            paused_ms: Arc::new(AtomicU64::new(0)),
            min_interval_ms: 0,
        },
    )
    .expect("no se ha podido iniciar la captura");

    let mut cache = FrameCache::new(&dir).unwrap();
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut last_ts = 0;
    while Instant::now() < deadline && cache.frame_count() < 4 {
        let Ok(frame) = receiver.recv_timeout(Duration::from_millis(500)) else {
            continue;
        };
        let mut rgba = frame.bgra.clone();
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        last_ts = frame.ts_ms;
        cache
            .push_rgba(&rgba, region.width, region.height, frame.ts_ms)
            .unwrap();
    }
    stop.store(true, Ordering::Relaxed);
    let _ = control.stop();

    let frames = cache.finish(last_ts + 50, 20).unwrap();
    assert!(!frames.is_empty(), "no se ha cacheado ningun fotograma");

    let mut session = record::SessionData {
        id: "e2e".into(),
        dir: dir.clone(),
        region,
        fps: 20,
        format: "gif".into(),
        has_audio: false,
        width: region.width,
        height: region.height,
        mp4_path: None,
        frames,
    };
    record::generate_thumbnails(&mut session).expect("las miniaturas han fallado");
    assert!(
        PathBuf::from(&session.frames[0].thumb_path).exists(),
        "la miniatura no se ha escrito"
    );
    session.persist().unwrap();

    let indices: Vec<usize> = (0..session.frames.len()).collect();
    let delays: Vec<u32> = session.frames.iter().map(|f| f.duration_ms).collect();

    let gif_path = dir.join("clip.gif");
    let mut loader = |index: usize| record::read_frame(&session, index);
    gif_encoder::encode(
        &indices,
        &delays,
        &mut loader,
        &gif_path,
        &gif_encoder::GifOptions {
            width: region.width / 2,
            height: region.height / 2,
            quality: 70,
            loop_forever: true,
        },
        |_, _, _| {},
    )
    .expect("no se ha podido exportar el GIF");
    assert!(std::fs::read(&gif_path).unwrap().starts_with(b"GIF89a"));

    let mp4_path = dir.join("clip.mp4");
    let mut loader = |index: usize| record::read_frame(&session, index);
    mp4::encode(
        &indices,
        &delays,
        &mut loader,
        &mp4_path,
        &mp4::Mp4Options {
            width: region.width,
            height: region.height,
            fps: 20,
            quality: 70,
        },
        |_, _, _| {},
    )
    .expect("no se ha podido exportar el MP4");
    assert_eq!(&std::fs::read(&mp4_path).unwrap()[4..8], b"ftyp");
}
