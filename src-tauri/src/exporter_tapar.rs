//! Tapar datos en un vídeo, probado por `escribir`: el mismo camino que «Guardar» (S9, D4).
//!
//! Las que corren siempre van a GIF y a PNG, que son Rust entero. La de la factura pasa por
//! el lector de texto de Windows, el codificador de MP4 de Windows y ffmpeg para abrir lo
//! que sale, así que va con `--ignored`:
//!
//!     cargo test --release --lib la_factura_en_un_video -- --ignored --nocapture
//!     cargo test --release --lib medir_tapar_un_minuto -- --ignored --nocapture

use super::*;
use crate::contrato::{ModoZoom, Tapa, TramoZoom};
use crate::datos_en_video::pruebas::sesion;

const ANCHO: u32 = 160;
const ALTO: u32 = 100;
/// Donde va el «dato»: un trozo de ruido en blanco y negro, que se reconoce píxel a píxel.
const DATO: (u32, u32, u32, u32) = (60, 40, 40, 20);

/// Ruido en blanco y negro dentro del dato y blanco alrededor. Sin grises a propósito: el
/// GIF tiene esos dos colores exactos en la paleta, así que lo que no se tapa sale idéntico
/// píxel a píxel, y lo tapado (grises del mosaico, o su tramado) sale distinto en la mitad.
fn pantalla(semilla: u32) -> image::RgbaImage {
    let mut estado = semilla.wrapping_mul(2_654_435_761).max(1);
    image::RgbaImage::from_fn(ANCHO, ALTO, |x, y| {
        let (dx, dy, w, h) = DATO;
        if x >= dx && x < dx + w && y >= dy && y < dy + h {
            estado ^= estado << 13;
            estado ^= estado >> 17;
            estado ^= estado << 5;
            if estado & 1 == 0 { image::Rgba([0, 0, 0, 255]) } else { image::Rgba([255, 255, 255, 255]) }
        } else {
            image::Rgba([255, 255, 255, 255])
        }
    })
}

fn tapa_del_dato(desde_ms: u64, hasta_ms: u64) -> Tapa {
    let (x, y, w, h) = DATO;
    Tapa {
        desde_ms,
        hasta_ms,
        x: x as f32 / ANCHO as f32,
        y: y as f32 / ALTO as f32,
        ancho: w as f32 / ANCHO as f32,
        alto: h as f32 / ALTO as f32,
    }
}

fn peticion(session: &SessionData, formato: &str) -> ExportRequest {
    let mut p = crate::recorder::peticion_por_defecto(session);
    p.format = formato.into();
    p.fps = 20;
    p.cursor = 0.0;
    p.audio = false;
    p
}

/// Exporta a GIF y devuelve cada fotograma con el rato del vídeo en que se ve.
fn gif(session: &SessionData, p: &ExportRequest, nombre: &str) -> Vec<(u64, u64, image::RgbaImage)> {
    let destino = session.dir.join(format!("{nombre}.gif"));
    escribir(session, p, &destino, &|_, _, _| {}).expect("exportar a GIF");
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

fn en(fotogramas: &[(u64, u64, image::RgbaImage)], ms: u64) -> &image::RgbaImage {
    &fotogramas.iter().find(|(d, h, _)| ms >= *d && ms < *h).expect("ese instante no está en el GIF").2
}

/// Qué parte de los píxeles de esa zona son iguales en las dos imágenes.
fn iguales(a: &image::RgbaImage, b: &image::RgbaImage, (x, y, w, h): (u32, u32, u32, u32)) -> f32 {
    let mut si = 0;
    for yy in y..y + h {
        for xx in x..x + w {
            if a.get_pixel(xx, yy) == b.get_pixel(xx, yy) {
                si += 1;
            }
        }
    }
    si as f32 / (w * h) as f32
}

/// Una pantalla quieta de tres segundos (UN fotograma) con la tapa de 1 a 2 s: el GIF sale
/// tapado justo en ese segundo, y el resto igual que sin tapa, píxel a píxel.
#[test]
fn tapa_solo_mientras_esta_puesta_aunque_la_pantalla_este_quieta() {
    let session = sesion("gif-quieto", &[(0, pantalla(1)), (3_000, pantalla(2))], 4_000);
    let sin = gif(&session, &peticion(&session, "gif"), "sin");
    let mut p = peticion(&session, "gif");
    p.edicion.tapar = vec![tapa_del_dato(1_000, 2_000)];
    let con = gif(&session, &p, "con");
    let _ = std::fs::remove_dir_all(&session.dir);
    for ms in [100, 900, 2_100, 2_900, 3_500] {
        assert_eq!(en(&con, ms), en(&sin, ms), "a los {ms} ms no hay nada tapado");
    }
    for ms in [1_050, 1_500, 1_950] {
        let parecido = iguales(en(&con, ms), en(&sin, ms), DATO);
        assert!(parecido < 0.75, "a los {ms} ms el dato tenía que ir tapado: {parecido:.2} igual");
        assert_eq!(iguales(en(&con, ms), en(&sin, ms), (0, 0, ANCHO, DATO.1)), 1.0, "lo de encima, intacto");
    }
}

/// Con zoom, el mosaico se va con el dato: la tapa es sobre la imagen grabada, y la cámara
/// acercada al dato enseña el dato tapado en el centro del vídeo.
#[test]
fn el_mosaico_sigue_al_dato_con_el_zoom() {
    let session = sesion("gif-zoom", &[(0, pantalla(3)), (2_000, pantalla(4))], 3_000);
    let (x, y, w, h) = DATO;
    let zoom = TramoZoom {
        desde_ms: 0,
        hasta_ms: 3_000,
        nivel: 2.0,
        modo: ModoZoom::Fijo,
        x: (x + w / 2) as f32 / ANCHO as f32,
        y: (y + h / 2) as f32 / ALTO as f32,
        instantaneo: true,
        lupa: None,
    };
    let mut p = peticion(&session, "gif");
    p.edicion.zooms = Some(vec![zoom]);
    let sin = gif(&session, &p, "sin");
    p.edicion.tapar = vec![tapa_del_dato(0, 3_000)];
    let con = gif(&session, &p, "con");
    let _ = std::fs::remove_dir_all(&session.dir);
    // Acercado al doble, el dato de 40 x 20 mide 80 x 40 en medio del vídeo de 160 x 100.
    let centro = (ANCHO / 2 - 36, ALTO / 2 - 16, 72, 32);
    let parecido = iguales(en(&con, 1_000), en(&sin, 1_000), centro);
    assert!(parecido < 0.75, "en el centro del zoom el dato tenía que ir tapado: {parecido:.2} igual");
}

/// Una foto PNG de un fotograma de vídeo lleva la tapa de su instante.
#[test]
fn una_foto_de_un_video_lleva_su_tapa() {
    let session = sesion("png", &[(0, pantalla(5)), (1_000, pantalla(6))], 2_000);
    let mut p = peticion(&session, "png");
    p.from = 1;
    p.to = 1;
    let sin_ruta = session.dir.join("sin.png");
    escribir(&session, &p, &sin_ruta, &|_, _, _| {}).unwrap();
    p.edicion.tapar = vec![tapa_del_dato(1_200, 1_400)];
    let con_ruta = session.dir.join("con.png");
    escribir(&session, &p, &con_ruta, &|_, _, _| {}).unwrap();
    // Y la tapa de otro rato no la toca.
    p.edicion.tapar = vec![tapa_del_dato(0, 500)];
    let otra_ruta = session.dir.join("otra.png");
    escribir(&session, &p, &otra_ruta, &|_, _, _| {}).unwrap();
    let abrir = |r: &Path| image::open(r).unwrap().to_rgba8();
    let (sin, con, otra) = (abrir(&sin_ruta), abrir(&con_ruta), abrir(&otra_ruta));
    let _ = std::fs::remove_dir_all(&session.dir);
    assert!(iguales(&con, &sin, DATO) < 0.75, "la foto tenía que llevar el dato tapado");
    assert_eq!(otra, sin, "una tapa de otro rato no se pone");
}

/// La prueba de la factura (`el_camino_entero_de_tapar_datos`), pero sobre un vídeo y
/// hasta el MP4: una pantalla con la factura, a los 1,0 s salen un correo, un DNI y un
/// teléfono, y a los 2,5 s se van. El lector de Windows los encuentra, `escribir` los tapa
/// en el MP4, y en el archivo que sale el lector ya no puede leerlos; la factura sí.
#[test]
#[ignore = "necesita el lector de texto y el codificador de Windows, y ffmpeg: se corre a mano"]
fn la_factura_en_un_video_sale_tapada_en_el_mp4() {
    use crate::datos_en_video::pruebas::pintar_texto;
    use std::process::Command;
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("sin ffmpeg no se puede abrir el MP4: no compruebo nada");
        return;
    }
    let (w, h) = (1280u32, 720u32);
    let factura = (40, 60, "Factura 2026-104 por 1.234,56 euros".to_string());
    let datos = [
        (40, 200, "Contacto: munir@example.com".to_string()),
        (40, 340, "DNI 12345678Z telefono 600123456".to_string()),
    ];
    let sin_datos = pintar_texto(&[factura.clone()], w, h, 48);
    let con_datos = pintar_texto(&[factura.clone(), datos[0].clone(), datos[1].clone()], w, h, 48);
    let despues = pintar_texto(&[factura.clone(), (40, 480, "Gracias por su compra".into())], w, h, 48);
    let session = sesion("factura", &[(0, sin_datos), (1_000, con_datos), (2_500, despues)], 3_500);

    let mut leer = crate::datos_en_video::lector_de_windows().expect("el lector de Windows");
    let hallado = crate::datos_en_video::buscar(&session, &mut leer, &mut |_, _| {}).expect("buscar");
    println!(
        "{} tapas en {} ms con {} lecturas: {:?} / {:?}",
        hallado.tapas.len(),
        hallado.ms,
        hallado.lecturas,
        hallado.resumen,
        hallado.tapas
    );
    assert!(hallado.tapas.len() >= 2, "de tres datos ha encontrado {}", hallado.tapas.len());
    for t in &hallado.tapas {
        assert!(t.desde_ms >= 1_000 && t.hasta_ms <= 2_500, "una tapa fuera del rato de los datos: {t:?}");
        assert!(t.y > 120.0 / h as f32, "ha tapado la factura, que no es un dato: {t:?}");
    }

    let mut p = peticion(&session, "mp4");
    p.fps = 10;
    let sin_ruta = session.dir.join("sin.mp4");
    escribir(&session, &p, &sin_ruta, &|_, _, _| {}).expect("MP4 sin tapas");
    p.edicion.tapar = hallado.tapas.clone();
    let con_ruta = session.dir.join("con.mp4");
    escribir(&session, &p, &con_ruta, &|_, _, _| {}).expect("MP4 con tapas");

    // Lo que se VE a cada décima del MP4, a tamaño completo.
    let fotogramas = |ruta: &Path| -> Vec<image::RgbaImage> {
        let crudo = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(ruta)
            .args(["-vf", "fps=10", "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .output()
            .expect("ffmpeg")
            .stdout;
        crudo
            .chunks_exact((w * h * 4) as usize)
            .map(|c| image::RgbaImage::from_raw(w, h, c.to_vec()).unwrap())
            .collect()
    };
    let (sin, con) = (fotogramas(&sin_ruta), fotogramas(&con_ruta));
    let _ = std::fs::remove_dir_all(&session.dir);
    let datos_en = |imagen: &image::RgbaImage| -> (Vec<crate::sensibles::Clase>, String) {
        let png = crate::encode::png::to_bytes(imagen).unwrap();
        let lineas = crate::platform::ocr::leer_palabras(&png).expect("el lector");
        let texto: Vec<String> =
            lineas.iter().map(|l| l.iter().map(|p| p.texto.as_str()).collect::<Vec<_>>().join(" ")).collect();
        let clases = lineas.iter().flat_map(|l| crate::sensibles::buscar_en_linea(l)).map(|d| d.clase).collect();
        (clases, texto.join(" / "))
    };
    for (decima, momento) in [(5, "antes"), (15, "con los datos"), (20, "con los datos"), (30, "después")] {
        let (en_sin, _) = datos_en(&sin[decima]);
        let (en_con, texto) = datos_en(&con[decima]);
        println!("a los {:.1} s ({momento}): sin tapas {:?}; con tapas {:?} y se lee «{texto}»", decima as f32 / 10.0, en_sin, en_con);
        assert!(en_con.is_empty(), "a los {:.1} s se lee un dato en el MP4 tapado: {en_con:?}", decima as f32 / 10.0);
        assert!(texto.to_lowercase().contains("factura"), "la factura tenía que seguir leyéndose: «{texto}»");
        if momento == "con los datos" {
            assert!(en_sin.len() >= 2, "el MP4 sin tapas tenía que enseñar los datos: {en_sin:?}");
        }
    }
    // Y lo que no tenía datos sale como sin tapas. No idéntico al píxel: el H.264 pierde, y
    // lo que sigue a unos fotogramas tapados se codifica a partir de ellos. Se mide la
    // diferencia media, en niveles de 0 a 255 por canal.
    for decima in [5, 30] {
        let (a, b) = (con[decima].as_raw(), sin[decima].as_raw());
        let media = a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as u64).sum::<u64>() as f64
            / a.len() as f64;
        println!("a los {:.1} s, con y sin tapas se diferencian en {media:.3} de media", decima as f32 / 10.0);
        assert!(media < 1.0, "a los {:.1} s el vídeo tapado no se parece al otro: {media:.3}", decima as f32 / 10.0);
    }
}

/// Lo que tarda buscar datos en un minuto de grabación 1080p, con el lector de Windows, en
/// una grabación que hace lo que hace una de verdad: quieta, escribiendo, con un scroll,
/// con el cursor de texto parpadeando y pasando páginas.
#[test]
#[ignore = "mide con el lector de Windows: se corre a mano, en release"]
fn medir_tapar_un_minuto() {
    use crate::datos_en_video::pruebas::pintar_texto;
    let (w, h) = (1920u32, 1080u32);
    let pagina = |n: usize, bajada: i32, escrito: &str, cursor: bool| {
        let mut lineas: Vec<(i32, i32, String)> = (0..22)
            .map(|i| {
                let texto = match (n + i) % 7 {
                    0 => format!("Pedido {} enviado a ana.garcia{}@example.com", 1_000 + n * 10 + i, i),
                    3 => format!("Llamar al 6{:08} antes del viernes", 12_345_678 + n * 7 + i),
                    _ => format!("Línea {i} de la página {n}: texto normal de una aplicación cualquiera"),
                };
                (60, 40 + i as i32 * 44 - bajada, texto)
            })
            .collect();
        lineas.push((60, 1_020, format!("{escrito}{}", if cursor { "|" } else { "" })));
        pintar_texto(&lineas, w, h, 22)
    };
    let mut fotogramas: Vec<(u64, image::RgbaImage)> = vec![(0, pagina(0, 0, "", false))];
    let frase = "Escribiendo: hola, mi correo es munir@example.com y mi teléfono 600123456";
    for (i, _) in frase.char_indices().skip(1).take(66) {
        // Con el cursor de texto detrás: un espacio solo no cambia la pantalla, el cursor sí.
        fotogramas.push((8_000 + fotogramas.len() as u64 * 150 - 150, pagina(0, 0, &frase[..i], true)));
    }
    let t = fotogramas.last().unwrap().0 + 400;
    for i in 1..=60u64 {
        fotogramas.push((t + i * 33, pagina(0, i as i32 * 8, frase, false)));
    }
    let t = fotogramas.last().unwrap().0 + 500;
    for i in 0..30u64 {
        fotogramas.push((t + i * 500, pagina(0, 480, frase, i % 2 == 0)));
    }
    let t = fotogramas.last().unwrap().0 + 500;
    for n in 1..=10usize {
        fotogramas.push((t + (n as u64 - 1) * 2_400, pagina(n, 0, "", false)));
    }
    let ultimo = fotogramas.last().unwrap().0;
    println!("{} fotogramas, el último a los {} ms", fotogramas.len(), ultimo);
    let session = sesion("minuto", &fotogramas, 60_000.max(ultimo + 1_000));
    drop(fotogramas);

    let mut leer = crate::datos_en_video::lector_de_windows().expect("el lector de Windows");
    let empieza = std::time::Instant::now();
    let hallado = crate::datos_en_video::buscar(&session, &mut leer, &mut |_, _| {}).expect("buscar");
    let total = empieza.elapsed();
    let _ = std::fs::remove_dir_all(&session.dir);
    println!(
        "un minuto 1080p: {:.1} s, {} lecturas para {} fotogramas, {} tapas, {:?}",
        total.as_secs_f32(),
        hallado.lecturas,
        hallado.fotogramas,
        hallado.tapas.len(),
        hallado.resumen
    );
    assert!(!hallado.tapas.is_empty());
}
