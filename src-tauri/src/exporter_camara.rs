//! Las pruebas de la webcam en el Estudio (S8), por `escribir`: el mismo camino que «Guardar».
//!
//! El `camara.mp4` se fabrica con el mismo escritor que usa la grabación de verdad
//! (`record::camara::Escritor`), con imágenes NV12 hechas aquí: es lo que daría una cámara,
//! sin encender ninguna. Así se prueba también que lo que escribe S2 se lee aquí.
//!
//! La cámara de mentira tiene la mitad izquierda roja el primer segundo y azul el segundo, y
//! la derecha siempre verde: recortada a la izquierda no tiene que verse ni un verde, y la
//! cara tiene que cambiar de color a su tiempo, también con la pantalla quieta.

use super::*;
use crate::contrato::{CamaraGrabada, Forma, MarcoCamara};
use crate::encode::camara_estudio;
use crate::record::FrameCache;

const ANCHO: u32 = 320;
const ALTO: u32 = 180;
const GRIS: [u8; 3] = [128, 128, 128];

/// Lo que mide la cámara de mentira: H.264 la quiere en múltiplos de 16.
const CAM: (u32, u32) = (320, 240);

/// Una sesión de verdad en el disco: una pantalla gris quieta que dura `total_ms`.
fn sesion(nombre: &str, total_ms: u64) -> SessionData {
    let dir = std::env::temp_dir().join(format!("winshotx-webcam-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cache = FrameCache::new(&dir).expect("cache");
    let gris: Vec<u8> = (0..ANCHO * ALTO)
        .flat_map(|_| [GRIS[0], GRIS[1], GRIS[2], 255])
        .collect();
    cache.push_rgba(&gris, ANCHO, ALTO, 0).unwrap();
    let mut session = super::tests::session_with(&[]);
    session.frames = cache.finish(total_ms, 30).expect("cerrar la cache");
    session.dir = dir;
    session.width = ANCHO;
    session.height = ALTO;
    session.region = crate::capture::Rect {
        x: 0,
        y: 0,
        width: ANCHO,
        height: ALTO,
    };
    session.fps = 30;
    session
}

/// Escribe `camara.mp4` en la carpeta de la sesión con `segundos` de imagen y la apunta en
/// la sesión, con ese desfase. Sin poder escribirlo la prueba falla: una prueba que se
/// salta en silencio parece verde y no ha mirado nada.
#[cfg(windows)]
fn con_camara(session: &mut SessionData, segundos: u32, desfase_ms: i64) -> bool {
    let ruta = session.dir.join("camara.mp4");
    std::thread::spawn(move || escribir_camara(&ruta, CAM, segundos))
        .join()
        .expect("el hilo de la cámara se ha caído")
        .expect("Media Foundation no ha podido escribir el camara.mp4 de mentira");
    session.camara = Some(CamaraGrabada {
        archivo: "camara.mp4".into(),
        ancho: CAM.0,
        alto: CAM.1,
        desfase_ms,
    });
    true
}

/// Los tres colores de la cámara de mentira, en YUV (BT.601, rango de vídeo).
const ROJO_YUV: [u8; 3] = [81, 90, 240];
const AZUL_YUV: [u8; 3] = [41, 240, 110];
const VERDE_YUV: [u8; 3] = [145, 54, 34];

/// La imagen `n` de la cámara (a 30 por segundo), en NV12: la mitad izquierda roja el primer
/// segundo y azul después, la derecha verde.
#[cfg(windows)]
fn nv12(camara: (u32, u32), n: u32) -> Vec<u8> {
    let (w, h) = (camara.0 as usize, camara.1 as usize);
    let izquierda = if n < 30 { ROJO_YUV } else { AZUL_YUV };
    let color = |x: usize| if x < w / 2 { izquierda } else { VERDE_YUV };
    let mut datos = vec![0u8; w * h * 3 / 2];
    for y in 0..h {
        for x in 0..w {
            datos[y * w + x] = color(x)[0];
        }
    }
    for y in 0..h / 2 {
        for x in 0..w / 2 {
            let i = w * h + y * w + x * 2;
            let c = color(x * 2);
            datos[i] = c[1];
            datos[i + 1] = c[2];
        }
    }
    datos
}

/// El `camara.mp4` de mentira, por el mismo `Escritor` que la grabación de verdad.
#[cfg(windows)]
fn escribir_camara(ruta: &Path, camara: (u32, u32), segundos: u32) -> Result<()> {
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    const FOTO: i64 = 333_333;
    let empaquetar = |a: u32, b: u32| (u64::from(a) << 32) | u64::from(b);
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }?;
    let resultado = (|| -> Result<()> {
        let tipo = unsafe { MFCreateMediaType() }?;
        unsafe {
            tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            tipo.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
            tipo.SetUINT64(&MF_MT_FRAME_SIZE, empaquetar(camara.0, camara.1))?;
            tipo.SetUINT64(&MF_MT_FRAME_RATE, empaquetar(30, 1))?;
            tipo.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            tipo.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, empaquetar(1, 1))?;
        }
        let escritor = crate::record::camara::Escritor::nuevo(ruta, &tipo)?;
        for n in 0..segundos * 30 {
            let datos = nv12(camara, n);
            let muestra = unsafe {
                let bufer = MFCreateMemoryBuffer(datos.len() as u32)?;
                let mut p = std::ptr::null_mut();
                bufer.Lock(&mut p, None, None)?;
                std::ptr::copy_nonoverlapping(datos.as_ptr(), p, datos.len());
                bufer.Unlock()?;
                bufer.SetCurrentLength(datos.len() as u32)?;
                let muestra = MFCreateSample()?;
                muestra.AddBuffer(&bufer)?;
                muestra.SetSampleTime(i64::from(n) * FOTO)?;
                muestra.SetSampleDuration(FOTO)?;
                muestra
            };
            escritor.escribir(&muestra)?;
        }
        escritor.cerrar()
    })();
    unsafe {
        let _ = MFShutdown();
        if com {
            CoUninitialize();
        }
    }
    resultado
}

/// La cámara en círculo, con solo la mitad izquierda de su imagen, en el centro de la
/// mitad derecha del vídeo: 128 px de lado (el 40 % de 320), con el centro en (224, 90).
fn marco_de_prueba() -> MarcoCamara {
    MarcoCamara {
        forma: Forma::Circulo,
        recorte: Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 0.5,
            y2: 1.0,
        },
        tam: 0.4,
        x: 0.7,
        y: 0.5,
        borde: false,
        redondeo: None,
    }
}

fn peticion(session: &SessionData, formato: &str, marco: Option<MarcoCamara>) -> ExportRequest {
    let mut p = crate::recorder::peticion_por_defecto(session);
    p.format = formato.into();
    p.fps = 20;
    p.cursor = 0.0;
    p.audio = false;
    p.edicion.camara = marco;
    p
}

/// Exporta a GIF y devuelve cada fotograma con el instante del vídeo en que empieza y acaba.
fn gif(
    session: &SessionData,
    peticion: &ExportRequest,
    nombre: &str,
) -> Vec<(u64, u64, image::RgbaImage)> {
    let destino = session.dir.join(format!("{nombre}.gif"));
    escribir(session, peticion, &destino, &|_, _, _| {}).expect("exportar a GIF ha fallado");
    let mut t = 0u64;
    super::exportar_de_verdad::fotogramas_del_gif(&destino)
        .into_iter()
        .map(|(retardo, imagen)| {
            let empieza = t;
            t += u64::from(retardo) * 10;
            (empieza, t, imagen)
        })
        .collect()
}

/// El fotograma que se ve en ese instante del GIF.
fn en(gif: &[(u64, u64, image::RgbaImage)], ms: u64) -> &image::RgbaImage {
    &gif.iter()
        .find(|(d, h, _)| ms >= *d && ms < *h)
        .unwrap_or_else(|| gif.last().unwrap())
        .2
}

fn es_rojo(p: &image::Rgba<u8>) -> bool {
    p.0[0] > 150 && p.0[1] < 90 && p.0[2] < 90
}
fn es_azul(p: &image::Rgba<u8>) -> bool {
    p.0[2] > 150 && p.0[0] < 90 && p.0[1] < 90
}
fn es_verde(p: &image::Rgba<u8>) -> bool {
    p.0[1] > 150 && p.0[0] < 90 && p.0[2] < 90
}
fn es_gris(p: &image::Rgba<u8>) -> bool {
    p.0.iter().take(3).all(|&c| (110..=146).contains(&c))
}
fn es_blanco(p: &image::Rgba<u8>) -> bool {
    p.0.iter().take(3).all(|&c| c > 225)
}

/// El criterio de «hecho» en el GIF: la cámara sale en su sitio, en su forma, recortada, y
/// avanza con el vídeo aunque la pantalla esté quieta.
#[cfg(windows)]
#[test]
fn la_camara_sale_en_su_sitio_con_su_forma_y_avanza_en_el_gif() {
    let mut session = sesion("gif", 2_000);
    if !con_camara(&mut session, 2, 0) {
        return;
    }
    let salida = gif(
        &session,
        &peticion(&session, "gif", Some(marco_de_prueba())),
        "circulo",
    );
    let (primero, segundo) = (en(&salida, 500), en(&salida, 1_500));
    println!(
        "círculo: {} fotogramas; a 0,5 s el centro es {:?}, a 1,5 s {:?}",
        salida.len(),
        primero.get_pixel(224, 90).0,
        segundo.get_pixel(224, 90).0
    );
    // En su sitio y con su imagen de cada instante.
    assert!(
        es_rojo(primero.get_pixel(224, 90)),
        "a 0,5 s la cara es roja: {:?}",
        primero.get_pixel(224, 90)
    );
    assert!(es_rojo(primero.get_pixel(224, 40)) && es_rojo(primero.get_pixel(224, 140)));
    assert!(
        es_azul(segundo.get_pixel(224, 90)),
        "a 1,5 s la cara es azul: {:?}",
        segundo.get_pixel(224, 90)
    );
    // La forma: las esquinas de su caja (160..288 x 26..154) siguen siendo la pantalla.
    for (x, y) in [(164, 30), (284, 30), (164, 150), (284, 150)] {
        assert!(
            es_gris(primero.get_pixel(x, y)),
            "la esquina {x},{y} del círculo es pantalla: {:?}",
            primero.get_pixel(x, y)
        );
    }
    // Fuera no se toca nada.
    assert!(es_gris(primero.get_pixel(60, 90)) && es_gris(primero.get_pixel(300, 10)));
    // El recorte: la mitad verde de la cámara no sale.
    let verdes = primero.pixels().filter(|p| es_verde(p)).count();
    assert!(
        verdes < 20,
        "el verde estaba recortado y salen {verdes} píxeles"
    );
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// Un cuadrado con borde llega a las esquinas de su caja y lleva el filo blanco; un
/// rectángulo sin recortar enseña la cámara entera, con su mitad verde.
#[cfg(windows)]
#[test]
fn el_cuadrado_con_borde_y_el_rectangulo_entero() {
    let mut session = sesion("formas", 1_000);
    if !con_camara(&mut session, 1, 0) {
        return;
    }
    let cuadrado = MarcoCamara {
        forma: Forma::Cuadrado,
        borde: true,
        ..marco_de_prueba()
    };
    let salida = gif(
        &session,
        &peticion(&session, "gif", Some(cuadrado)),
        "cuadrado",
    );
    let f = en(&salida, 300);
    assert!(
        es_blanco(f.get_pixel(160, 90)),
        "con borde, el filo es blanco: {:?}",
        f.get_pixel(160, 90)
    );
    assert!(
        es_rojo(f.get_pixel(167, 33)),
        "el cuadrado llega a la esquina: {:?}",
        f.get_pixel(167, 33)
    );
    assert!(
        es_gris(f.get_pixel(156, 90)),
        "y por fuera, la pantalla: {:?}",
        f.get_pixel(156, 90)
    );

    // La cámara entera en un rectángulo de 4:3 (lo que mide la cámara): 128 x 96.
    let rectangulo = MarcoCamara {
        forma: Forma::Rectangulo,
        recorte: Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        },
        ..marco_de_prueba()
    };
    let salida = gif(
        &session,
        &peticion(&session, "gif", Some(rectangulo)),
        "rectangulo",
    );
    let f = en(&salida, 300);
    assert!(
        es_rojo(f.get_pixel(180, 90)),
        "la mitad izquierda, roja: {:?}",
        f.get_pixel(180, 90)
    );
    assert!(
        es_verde(f.get_pixel(270, 90)),
        "la derecha, verde: {:?}",
        f.get_pixel(270, 90)
    );
    assert!(
        es_gris(f.get_pixel(224, 38)),
        "un 4:3 de 96 de alto no llega a y=38: {:?}",
        f.get_pixel(224, 38)
    );
    assert!(es_rojo(f.get_pixel(180, 45)));
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// S13 (redondeo): el cuadrado con «Esquinas redondeadas» sale con sus esquinas en el
/// archivo. Al 25 % de 128 px el radio es 32: el píxel a 7 de cada borde, que en el
/// cuadrado de antes es cara, aquí es la pantalla; el centro y el medio del lado, cara.
#[cfg(windows)]
#[test]
fn el_cuadrado_con_las_esquinas_redondeadas() {
    let mut session = sesion("redondeo", 1_000);
    if !con_camara(&mut session, 1, 0) {
        return;
    }
    let redondeado = MarcoCamara {
        forma: Forma::Cuadrado,
        redondeo: Some(0.25),
        ..marco_de_prueba()
    };
    let salida = gif(
        &session,
        &peticion(&session, "gif", Some(redondeado)),
        "redondeo",
    );
    let f = en(&salida, 300);
    assert!(
        es_gris(f.get_pixel(167, 33)),
        "la esquina redondeada deja ver la pantalla: {:?}",
        f.get_pixel(167, 33)
    );
    assert!(
        es_rojo(f.get_pixel(224, 90)),
        "el centro, la cara: {:?}",
        f.get_pixel(224, 90)
    );
    assert!(
        es_rojo(f.get_pixel(163, 90)),
        "y el medio del lado llega al borde: {:?}",
        f.get_pixel(163, 90)
    );
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// El desfase de S2: la cámara arrancó 600 ms tarde, así que va 600 ms por detrás. Antes
/// de arrancar se ve su primera imagen.
#[cfg(windows)]
#[test]
fn la_camara_va_con_su_desfase() {
    let mut session = sesion("desfase", 2_500);
    if !con_camara(&mut session, 2, 600) {
        return;
    }
    let salida = gif(
        &session,
        &peticion(&session, "gif", Some(marco_de_prueba())),
        "desfase",
    );
    let color = |ms| *en(&salida, ms).get_pixel(224, 90);
    assert!(
        es_rojo(&color(200)),
        "antes de arrancar, su primera imagen: {:?}",
        color(200)
    );
    assert!(
        es_rojo(&color(1_450)),
        "a 1,45 s la cámara va por 0,85: roja, {:?}",
        color(1_450)
    );
    assert!(
        es_azul(&color(1_800)),
        "a 1,8 s va por 1,2: azul, {:?}",
        color(1_800)
    );
    // Y acabada la cámara, su última imagen.
    assert!(
        es_azul(&color(2_450)),
        "acabada, la última: {:?}",
        color(2_450)
    );
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// Con los cortes de la línea de tiempo: quitado el primer segundo, el vídeo empieza en el
/// segundo de la cámara que toca, el azul.
#[cfg(windows)]
#[test]
fn la_camara_salta_con_los_cortes() {
    let mut session = sesion("cortes", 2_000);
    if !con_camara(&mut session, 2, 0) {
        return;
    }
    let mut p = peticion(&session, "gif", Some(marco_de_prueba()));
    p.edicion.tramos = Some(vec![crate::contrato::TramoVelocidad {
        desde_ms: 1_100,
        hasta_ms: 2_000,
        velocidad: 1.0,
    }]);
    let salida = gif(&session, &p, "cortes");
    let f = en(&salida, 100);
    assert!(
        es_azul(f.get_pixel(224, 90)),
        "lo primero que se ve es el segundo azul: {:?}",
        f.get_pixel(224, 90)
    );
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// Sin `camara` en la petición, o sin cámara en la sesión, no cambia nada: el mismo GIF,
/// con la pantalla quieta en un solo fotograma, como antes.
#[cfg(windows)]
#[test]
fn sin_pedirla_o_sin_grabarla_no_cambia_nada() {
    let mut session = sesion("nada", 1_000);
    let antes = gif(&session, &peticion(&session, "gif", None), "antes");
    let sin_camara_grabada = gif(
        &session,
        &peticion(&session, "gif", Some(marco_de_prueba())),
        "sin-grabar",
    );
    if !con_camara(&mut session, 1, 0) {
        return;
    }
    let sin_pedirla = gif(&session, &peticion(&session, "gif", None), "sin-pedir");
    let _ = std::fs::remove_dir_all(&session.dir);
    for (nombre, otro) in [
        ("sin cámara grabada", &sin_camara_grabada),
        ("sin pedirla", &sin_pedirla),
    ] {
        assert_eq!(otro.len(), antes.len(), "{nombre}: otros fotogramas");
        for (a, b) in antes.iter().zip(otro.iter()) {
            assert_eq!((a.0, a.1), (b.0, b.1), "{nombre}: otros tiempos");
            assert!(a.2 == b.2, "{nombre}: otra imagen");
        }
    }
    assert_eq!(antes.len(), 1, "una pantalla quieta es un solo fotograma");
}

/// Si el archivo de la cámara está roto, el vídeo sale igual, sin ella.
#[test]
fn una_camara_rota_no_tira_la_exportacion() {
    let mut session = sesion("rota", 600);
    std::fs::write(session.dir.join("camara.mp4"), b"esto no es un mp4").unwrap();
    session.camara = Some(CamaraGrabada {
        archivo: "camara.mp4".into(),
        ancho: 640,
        alto: 480,
        desfase_ms: 0,
    });
    let salida = gif(
        &session,
        &peticion(&session, "gif", Some(marco_de_prueba())),
        "rota",
    );
    let _ = std::fs::remove_dir_all(&session.dir);
    assert!(
        salida.iter().all(|(_, _, f)| es_gris(f.get_pixel(224, 90))),
        "sin cámara, la pantalla"
    );
}

/// Una foto (PNG o JPG) de una grabación con cámara lleva la cara de su instante en su sitio,
/// como la vista previa parada ahí; sin pedirla, la pantalla tal cual.
#[cfg(windows)]
#[test]
fn la_foto_lleva_la_camara_de_su_instante() {
    let mut session = sesion("foto", 2_000);
    if !con_camara(&mut session, 2, 0) {
        return;
    }
    for formato in ["png", "jpg"] {
        let leer = |marco: Option<MarcoCamara>, nombre: &str| {
            let destino = session.dir.join(format!("{nombre}.{formato}"));
            escribir(&session, &peticion(&session, formato, marco), &destino, &|_, _, _| {})
                .expect("exportar la foto ha fallado");
            image::open(&destino).expect("leer la foto").to_rgba8()
        };
        let con = leer(Some(marco_de_prueba()), "con");
        let sin = leer(None, "sin");
        println!("{formato}: el centro de la cara es {:?}", con.get_pixel(224, 90).0);
        assert!(es_rojo(con.get_pixel(224, 90)), "{formato}: la cara de los 0 s es roja: {:?}", con.get_pixel(224, 90));
        assert!(es_gris(con.get_pixel(164, 30)), "{formato}: en círculo, la esquina de su caja es pantalla");
        assert!(es_gris(con.get_pixel(60, 90)), "{formato}: fuera de la cara, la pantalla");
        assert!(es_gris(sin.get_pixel(224, 90)), "{formato}: sin pedirla no sale");
    }
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// Lo que se guarda sin pasar por el editor lleva la cámara si se grabó, la de fábrica, y sin
/// ella nada cambia.
#[test]
fn guardar_sin_editor_lleva_la_camara_si_se_grabo() {
    let mut session = sesion("sin-editor", 500);
    assert!(crate::recorder::peticion_por_defecto(&session).edicion.camara.is_none());
    session.camara = Some(CamaraGrabada {
        archivo: "camara.mp4".into(),
        ancho: 640,
        alto: 480,
        desfase_ms: 0,
    });
    // `MarcoCamara` no se compara (es del contrato y lleva números con decimales): se mira
    // tal cual viaja.
    let json = |m: Option<MarcoCamara>| serde_json::to_value(m).unwrap();
    assert_eq!(
        json(crate::recorder::peticion_por_defecto(&session).edicion.camara),
        json(Some(camara_estudio::de_fabrica(ANCHO, ALTO)))
    );
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// Lo que se le da al editor: la ruta entera del archivo, y nada si no está.
#[test]
fn el_editor_recibe_la_ruta_solo_si_existe() {
    let mut session = sesion("editor", 500);
    session.camara = Some(CamaraGrabada {
        archivo: "camara.mp4".into(),
        ancho: 640,
        alto: 480,
        desfase_ms: 120,
    });
    assert_eq!(
        camara_estudio::de_la_sesion(&session),
        None,
        "sin archivo no hay cámara"
    );
    std::fs::write(session.dir.join("camara.mp4"), b"x").unwrap();
    let dada = camara_estudio::de_la_sesion(&session).expect("con archivo, sí");
    assert!(
        dada.ruta.ends_with("camara.mp4") && dada.desfase_ms == 120 && dada.primer_ms == 0,
        "{dada:?}"
    );
    // Un nombre con carpetas no saca la ruta de su sitio.
    session.camara.as_mut().unwrap().archivo = "..\\..\\camara.mp4".into();
    assert_eq!(
        camara_estudio::de_la_sesion(&session).unwrap().ruta,
        session.dir.join("camara.mp4").to_string_lossy()
    );
    let _ = std::fs::remove_dir_all(&session.dir);
}

/// El color medio de un cuadrado de 9 x 9 alrededor de un punto.
fn medio(imagen: &image::RgbaImage, cx: u32, cy: u32) -> [u8; 3] {
    let mut suma = [0u32; 3];
    for y in cy - 4..=cy + 4 {
        for x in cx - 4..=cx + 4 {
            for (s, v) in suma.iter_mut().zip(imagen.get_pixel(x, y).0) {
                *s += u32::from(v);
            }
        }
    }
    suma.map(|s| (s / 81) as u8)
}

/// El criterio de «hecho» en el MP4: el archivo exportado se abre con Media Foundation y la
/// cara está en su sitio, en círculo y a su tiempo.
#[cfg(windows)]
#[test]
#[ignore = "necesita el codificador de Windows: se corre a mano"]
fn la_camara_sale_en_el_mp4() {
    let mut session = sesion("mp4", 2_000);
    if !con_camara(&mut session, 2, 0) {
        return;
    }
    let p = peticion(&session, "mp4", Some(marco_de_prueba()));
    let destino = session.dir.join("camara.export.mp4");
    escribir(&session, &p, &destino, &|_, _, _| {}).expect("exportar a MP4 ha fallado");
    let mut lector =
        crate::encode::fondo_video::abrir(&destino).expect("el MP4 exportado no se abre");
    let mut vistos: Vec<(u64, image::RgbaImage)> = Vec::new();
    while let Some(c) = lector.siguiente().expect("fotograma roto") {
        vistos.push((c.desde_ms, c.imagen));
    }
    drop(lector);
    let _ = std::fs::remove_dir_all(&session.dir);
    let en_el = |ms: u64| {
        &vistos
            .iter()
            .rev()
            .find(|(d, _)| *d <= ms)
            .unwrap_or(&vistos[0])
            .1
    };
    let (a, b) = (en_el(500), en_el(1_500));
    assert_eq!(a.dimensions(), (ANCHO, ALTO));
    let (centro_a, centro_b) = (medio(a, 224, 90), medio(b, 224, 90));
    let (esquina, fuera) = (medio(a, 166, 32), medio(a, 60, 90));
    println!(
        "MP4: {} fotogramas; centro a 0,5 s {centro_a:?}, a 1,5 s {centro_b:?}; esquina de la caja {esquina:?}, fuera {fuera:?}",
        vistos.len()
    );
    let mas = |p: u8, q: u8| i32::from(p) > i32::from(q) + 80;
    assert!(
        mas(centro_a[0], centro_a[1]) && mas(centro_a[0], centro_a[2]),
        "a 0,5 s, rojo: {centro_a:?}"
    );
    assert!(
        mas(centro_b[2], centro_b[0]) && mas(centro_b[2], centro_b[1]),
        "a 1,5 s, azul: {centro_b:?}"
    );
    let gris = |c: [u8; 3]| c.iter().all(|&v| (100..=156).contains(&v));
    assert!(
        gris(esquina),
        "la esquina de la caja del círculo es pantalla: {esquina:?}"
    );
    assert!(gris(fuera), "fuera, la pantalla: {fuera:?}");
}

/// Lo que cuesta la cámara al exportar: el tutorial de 20 s a 1080p de `medir_un_tutorial`,
/// sin cámara y con una de 720p encima, en la misma pasada para que el ruido de la máquina
/// caiga igual en los dos.
///
///     cargo test --release --lib medir_un_tutorial_con_camara -- --ignored --nocapture
#[cfg(windows)]
#[test]
#[ignore = "mide, no comprueba: pide release y el codificador de Windows"]
fn medir_un_tutorial_con_camara() {
    let (ancho, alto) = (1920u32, 1080u32);
    let dir = std::env::temp_dir().join(format!("winshotx-tutorial-camara-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cache = FrameCache::new(&dir).expect("cache");
    let pagina: Vec<u8> = (0..ancho * alto)
        .flat_map(|i| {
            let (x, y) = (i % ancho, i / ancho);
            let v = if (x / 120 + y / 80) % 2 == 0 {
                235
            } else {
                215
            };
            [v, v, v, 255]
        })
        .collect();
    cache.push_rgba(&pagina, ancho, alto, 0).unwrap();
    let mut session = super::tests::session_with(&[]);
    session.frames = cache.finish(20_000, 30).expect("cerrar la cache");
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
    session.cursor = (0..1_250u64)
        .map(|k| {
            let t = k * 16;
            let x = if (t / 2_000) % 2 == 0 {
                200 + ((t % 2_000) as i32 * 1_400) / 2_000
            } else {
                1_600
            };
            (t, x, 540)
        })
        .collect();
    let ruta = dir.join("camara.mp4");
    let escrita = Instant::now();
    std::thread::spawn(move || escribir_camara(&ruta, (1280, 720), 20))
        .join()
        .unwrap()
        .expect("escribir la cámara");
    println!(
        "cámara de 720p y 20 s escrita en {} ms",
        escrita.elapsed().as_millis()
    );
    session.camara = Some(CamaraGrabada {
        archivo: "camara.mp4".into(),
        ancho: 1280,
        alto: 720,
        desfase_ms: 0,
    });

    let mut peticion = crate::recorder::peticion_por_defecto(&session);
    peticion.format = "mp4".into();
    peticion.audio = false;
    peticion.smooth_cursor = true;
    peticion.margin = 96;
    peticion.corners = 23;
    peticion.background = "noche".into();
    peticion.shadow = true;
    for (nombre, marco) in [
        ("sin cámara", None),
        ("con cámara", Some(camara_estudio::de_fabrica(2112, 1272))),
    ] {
        peticion.edicion.camara = marco;
        let pasos = pasos_de(&session, &peticion);
        let destino = dir.join("tutorial.mp4");
        let empieza = Instant::now();
        escribir(&session, &peticion, &destino, &|_, _, _| {}).expect("exportar");
        let tardo = empieza.elapsed().as_millis();
        let pesa = std::fs::metadata(&destino).map(|m| m.len()).unwrap_or(0);
        println!(
            "tutorial 1080p de 20 s, {nombre}: {} pasos, {tardo} ms en total, {:.1} ms por paso, {} KB",
            pasos.indices.len(),
            tardo as f64 / pasos.indices.len().max(1) as f64,
            pesa / 1024
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
