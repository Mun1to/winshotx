//! De punta a punta, con el Tesseract de verdad: se pinta una pantalla con las letras del
//! sistema y se lee por las mismas puertas que usa la app. Necesitan `tesseract` instalado
//! (la CI de Linux lo instala). Las dos mitades en verde por separado no son la función hecha:
//! entre las dos hay una imagen de verdad y un lector que parte las palabras donde quiere.

use image::{Rgba, RgbaImage};

use super::*;
use crate::sensibles::Clase;

const CLARO: [u8; 3] = [246, 246, 247];
const TINTA: [u8; 3] = [28, 28, 30];
/// El alto de celda de la letra de una interfaz sin escalar: 14 píxeles.
const LETRA: i32 = 14;

/// Renglones pintados con las letras del sistema: `(x, y, texto)` con la esquina de arriba a
/// la izquierda de cada uno, y `letra` de alto de celda.
pub(crate) fn pintar(
    lineas: &[(i32, i32, &str)],
    ancho: u32,
    alto: u32,
    letra: i32,
    fondo: [u8; 3],
    tinta: [u8; 3],
) -> RgbaImage {
    let mut imagen = RgbaImage::from_pixel(ancho, alto, Rgba([fondo[0], fondo[1], fondo[2], 255]));
    for (x0, y0, texto) in lineas {
        let renglon = crate::record::letras::componer(texto, letra).expect("el sistema no tiene ninguna fuente");
        renglon.pintar(|x, y, cobertura| {
            let (x, y) = (x0 + x, y0 + y);
            if x < 0 || y < 0 || x >= ancho as i32 || y >= alto as i32 {
                return;
            }
            let pixel = imagen.get_pixel_mut(x as u32, y as u32);
            for canal in 0..3 {
                let antes = pixel.0[canal] as f32;
                pixel.0[canal] = (antes + (tinta[canal] as f32 - antes) * cobertura.clamp(0.0, 1.0)).round() as u8;
            }
        });
    }
    imagen
}

fn motor() -> Motor {
    Motor::nuevo().expect("para estas pruebas hace falta Tesseract instalado")
}

/// Una lectura de verdad cada vez. El Tesseract de Debian reparte cada lectura en varios
/// hilos, y nueve a la vez en los cuatro nucleos de la CI pasaban del plazo de 60 s (20-nov-2026,
/// dos vueltas seguidas). La app lee de una en una.
fn turno() -> parking_lot::MutexGuard<'static, ()> {
    static TURNO: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    TURNO.lock()
}

#[test]
fn lee_la_letra_de_una_interfaz_y_cada_renglon_sale_en_su_linea() {
    let _turno = turno();
    let imagen = pintar(
        &[(24, 20, "Grabar la pantalla entera"), (24, 60, "Abrir el editor al terminar")],
        420,
        110,
        LETRA,
        CLARO,
        TINTA,
    );
    let texto = texto_de(&motor().leer(&imagen).expect("leer"));
    println!("ha leído: {texto:?}");
    let (arriba, abajo) = texto.split_once('\n').expect("los dos renglones han salido pegados");
    assert!(arriba.contains("Grabar") && arriba.contains("pantalla") && arriba.contains("entera"), "arriba: {arriba:?}");
    assert!(abajo.contains("Abrir") && abajo.contains("editor") && abajo.contains("terminar"), "abajo: {abajo:?}");
}

/// La puerta que usa la tecla T: entra un PNG y sale su texto.
#[test]
fn la_puerta_de_copiar_el_texto_lee_un_png() {
    let _turno = turno();
    let imagen = pintar(&[(24, 24, "winshotx lee este texto")], 420, 80, 18, CLARO, TINTA);
    let png = crate::encode::png::to_bytes(&imagen).expect("el PNG de prueba");
    let texto = crate::platform::ocr::leer_texto(&png).expect("leer").to_lowercase();
    assert!(texto.contains("winshotx") && texto.contains("texto"), "ha leído {texto:?}");
}

/// El Tesseract 4 de Ubuntu 22.04 no lee letra clara sobre fondo oscuro: por eso se le da la
/// vuelta antes. Tiene que leer lo mismo que en claro.
#[test]
fn la_letra_clara_sobre_fondo_oscuro_se_lee_igual() {
    let _turno = turno();
    let lineas = [(24, 20, "Fotogramas por segundo"), (24, 60, "Sonido al empezar")];
    let oscura = pintar(&lineas, 420, 110, LETRA, [17, 17, 19], [232, 232, 234]);
    let texto = texto_de(&motor().leer(&oscura).expect("leer"));
    println!("ha leído: {texto:?}");
    for palabra in ["Fotogramas", "segundo", "Sonido", "empezar"] {
        assert!(texto.contains(palabra), "falta «{palabra}» en {texto:?}");
    }
}

/// Con el idioma del sistema instalado, las tildes salen. Sin él se pierden: por eso la orden
/// que se le dice a quien no tiene Tesseract lleva también el paquete de su idioma.
#[test]
fn con_el_espannol_instalado_salen_las_tildes() {
    let _turno = turno();
    let salida = std::process::Command::new("tesseract").arg("--list-langs").output().expect("tesseract");
    if !idiomas_instalados(&String::from_utf8_lossy(&salida.stdout)).iter().any(|i| i == "spa") {
        eprintln!("sin el paquete del español no se puede comprobar: no miro nada");
        return;
    }
    let imagen = pintar(&[(24, 20, "La cámara y el micrófono de la sesión")], 520, 70, LETRA, CLARO, TINTA);
    let texto = texto_de(&Motor::con_idiomas("spa+eng").leer(&imagen).expect("leer"));
    for palabra in ["cámara", "micrófono", "sesión"] {
        assert!(texto.contains(palabra), "falta «{palabra}» en {texto:?}");
    }
}

/// Con el español delante, el correo sale entero y las tildes también: el correo lo pone la
/// pasada en inglés (con `spa`, Tesseract lee la arroba como una «O») y lo demás, la otra.
#[test]
fn con_el_espannol_delante_el_correo_sale_entero() {
    let _turno = turno();
    let salida = std::process::Command::new("tesseract").arg("--list-langs").output().expect("tesseract");
    if !idiomas_instalados(&String::from_utf8_lossy(&salida.stdout)).iter().any(|i| i == "spa") {
        eprintln!("sin el paquete del español no se puede comprobar: no miro nada");
        return;
    }
    let imagen = pintar(
        &[(16, 14, "El camión llega mañana"), (16, 50, "Correo: ana.lopez@ejemplo.com")],
        340,
        100,
        15,
        [255, 255, 255],
        [17, 17, 17],
    );
    let lineas = Motor::con_idiomas("spa+eng").leer(&imagen).expect("leer");
    let texto = texto_de(&lineas);
    assert!(texto.contains("ana.lopez@ejemplo.com"), "el correo no sale entero en {texto:?}");
    assert!(texto.contains("camión") && texto.contains("mañana"), "faltan las tildes en {texto:?}");
    let datos: Vec<_> = lineas.iter().flat_map(|l| crate::sensibles::buscar_en_linea(l)).collect();
    assert!(datos.iter().any(|d| d.clase == Clase::Correo), "el detector no ve el correo en {texto:?}");
}

/// El camino entero de «Tapar datos»: una pantalla con un correo, un DNI y un teléfono, leída
/// por la puerta de la app, y el detector sobre lo leído.
#[test]
fn el_camino_entero_de_tapar_datos() {
    let _turno = turno();
    let imagen = pintar(
        &[
            (24, 20, "Factura 2026-104 por 1.234,56 euros"),
            (24, 60, "Contacto: munir@example.com"),
            (24, 100, "DNI 12345678Z telefono 600123456"),
        ],
        520,
        150,
        16,
        CLARO,
        TINTA,
    );
    let png = crate::encode::png::to_bytes(&imagen).expect("el PNG de prueba");
    let lineas = crate::platform::ocr::leer_palabras(&png).expect("leer");
    let mut clases = Vec::new();
    for linea in &lineas {
        println!("  {:?}", linea.iter().map(|p| p.texto.as_str()).collect::<Vec<_>>().join(" "));
        for hallazgo in crate::sensibles::buscar_en_linea(linea) {
            // La caja de lo encontrado cae dentro de la imagen.
            for &i in &hallazgo.palabras {
                let p = &linea[i];
                assert!(p.x >= 0.0 && p.y >= 0.0 && p.x + p.ancho <= 521.0 && p.y + p.alto <= 151.0, "caja fuera: {p:?}");
            }
            clases.push(hallazgo.clase);
        }
    }
    assert!(clases.contains(&Clase::Correo), "no ha encontrado el correo: {clases:?}");
    assert!(clases.len() >= 2, "de tres datos ha encontrado {}: {clases:?}", clases.len());
    assert!(clases.len() <= 4, "ha encontrado datos de más: {clases:?}");
}

/// La caja de una palabra tiene que caer donde está pintada, en píxeles de la imagen y no de
/// la copia agrandada que lee Tesseract: con esa caja se tapa.
#[test]
fn la_caja_de_una_palabra_cae_donde_esta_pintada() {
    let _turno = turno();
    let (x0, y0, letra) = (200, 90, 20);
    let imagen = pintar(&[(x0, y0, "winshotx")], 480, 200, letra, CLARO, TINTA);
    let ancho_pintado = crate::record::letras::componer("winshotx", letra).unwrap().ancho as f32;
    let lineas = motor().leer(&imagen).expect("leer");
    let palabra = lineas
        .iter()
        .flatten()
        .find(|p| p.texto.to_lowercase().contains("winshotx"))
        .expect("no ha leído la palabra");
    assert!((palabra.x - x0 as f32).abs() <= 4.0, "empieza en {} y está pintada en {x0}", palabra.x);
    assert!((palabra.ancho - ancho_pintado).abs() <= 8.0, "mide {} y está pintada con {ancho_pintado}", palabra.ancho);
    assert!(
        palabra.y >= y0 as f32 - 2.0 && palabra.y + palabra.alto <= (y0 + letra) as f32 + 4.0,
        "en vertical: {palabra:?}"
    );
}

#[test]
fn una_imagen_sin_texto_no_da_nada() {
    let _turno = turno();
    let blanca = RgbaImage::from_pixel(300, 160, Rgba([255, 255, 255, 255]));
    assert!(motor().leer(&blanca).expect("leer").is_empty());
    let png = crate::encode::png::to_bytes(&blanca).expect("el PNG de prueba");
    assert_eq!(crate::platform::ocr::leer_texto(&png).expect("leer"), "");
}

/// Una captura con scroll de nueve mil píxeles se lee a tiras. Cada renglón sale una vez,
/// también el que cae donde se solapan dos tiras, y a su altura de verdad.
#[test]
fn una_imagen_muy_alta_se_lee_a_tiras_sin_repetir_ni_perder_renglones() {
    let _turno = turno();
    let renglones = [(60, "primero arriba"), (3_930, "segundo corte"), (5_000, "tercero medio"), (8_940, "cuarto abajo")];
    let lineas: Vec<(i32, i32, &str)> = renglones.iter().map(|(y, t)| (24, *y, *t)).collect();
    let imagen = pintar(&lineas, 600, 9_000, 22, CLARO, TINTA);
    let leidas = motor().leer(&imagen).expect("leer");
    let texto = texto_de(&leidas).to_lowercase();
    println!("ha leído: {texto:?}");
    for (y, frase) in renglones {
        let clave = frase.split(' ').next().unwrap();
        assert_eq!(texto.matches(clave).count(), 1, "«{clave}» tiene que salir una vez en {texto:?}");
        let linea = leidas
            .iter()
            .find(|l| l.iter().any(|p| p.texto.to_lowercase().contains(clave)))
            .unwrap();
        assert!((linea[0].y - y as f32).abs() <= 12.0, "«{clave}» está pintado en {y} y sale en {}", linea[0].y);
    }
    let orden: Vec<usize> = ["primero", "segundo", "tercero", "cuarto"].iter().map(|c| texto.find(c).unwrap()).collect();
    assert!(orden.windows(2).all(|par| par[0] < par[1]), "no salen de arriba abajo: {texto:?}");
}
