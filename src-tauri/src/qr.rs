//! Leer los códigos QR que haya en una captura (E5 del plan de CleanShot X).
//!
//! Es la hermana de «Copiar el texto»: se elige el trozo de pantalla donde está el código y
//! lo que dice acaba en el portapapeles, sin sacar el móvil. Sirve para el enlace de una
//! presentación, la wifi de un cartel o el código de una factura.
//!
//! **Por qué `quircs` y no otro.** Windows no trae un lector de QR para imágenes (el suyo
//! es para escáneres y cámaras), así que hace falta un crate. Se midieron dos, con el mismo
//! perfil de compilación que la app, el 11 de noviembre de 2026:
//!
//! | crate          | .exe      | comprimido |
//! |----------------|-----------|------------|
//! | `quircs` 0.10  | +33 KB    | +15 KB     |
//! | `rqrr` 0.10    | +178 KB   | +86 KB     |
//!
//! Los dos son el mismo lector de C (quirc) pasado a Rust, y los dos leyeron los códigos de
//! prueba. Se queda el que pesa cinco veces menos. Licencia MIT.

use image::{GrayImage, RgbaImage};

/// Lo que dicen todos los códigos QR de la imagen, sin repetir, en el orden en que salen.
///
/// Prueba tres veces antes de rendirse, porque una captura no es una foto de un QR:
///
/// 1. tal cual;
/// 2. con los colores al revés, para los QR claros sobre fondo oscuro de los temas
///    oscuros, que el lector no reconoce porque busca las tres esquinas negras;
/// 3. al doble de tamaño, para los QR pequeños de una web a poca resolución, donde cada
///    cuadradito mide uno o dos píxeles y el lector no llega a ver las esquinas.
///
/// El texto que no sea UTF-8 válido se lee igual, cambiando lo que no se entiende por «�»:
/// mejor un enlace con un carácter raro que no copiar nada.
pub fn leer(imagen: &RgbaImage) -> Vec<String> {
    let gris = image::imageops::grayscale(imagen);
    let mut encontrados = leer_gris(&gris);
    if encontrados.is_empty() {
        let mut al_reves = gris.clone();
        image::imageops::invert(&mut al_reves);
        encontrados = leer_gris(&al_reves);
    }
    // Solo si la captura es pequeña: duplicar una de 4K son 33 millones de píxeles para
    // buscar un código que, a ese tamaño, ya se habría visto en la primera pasada.
    if encontrados.is_empty() && gris.width() * gris.height() <= 1600 * 1200 {
        let doble = image::imageops::resize(
            &gris,
            gris.width() * 2,
            gris.height() * 2,
            image::imageops::FilterType::Nearest,
        );
        encontrados = leer_gris(&doble);
    }
    encontrados
}

fn leer_gris(gris: &GrayImage) -> Vec<String> {
    let mut lector = quircs::Quirc::default();
    let mut textos: Vec<String> = Vec::new();
    for codigo in lector.identify(gris.width() as usize, gris.height() as usize, gris.as_raw()) {
        let Ok(codigo) = codigo else { continue };
        let Ok(datos) = codigo.decode() else { continue };
        let texto = String::from_utf8_lossy(&datos.payload).into_owned();
        if !texto.is_empty() && !textos.contains(&texto) {
            textos.push(texto);
        }
    }
    textos
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use image::{imageops, Rgba};

    /// Un QR de verdad (hecho con el crate `qrcode`, fuera del repo), a un píxel por
    /// cuadradito: cada prueba lo pone del tamaño que necesita.
    fn qr(bytes: &[u8]) -> RgbaImage {
        image::load_from_memory(bytes).unwrap().to_rgba8()
    }
    const ENLACE: &[u8] = include_bytes!("qr/qr-enlace.png");
    const TEXTO: &[u8] = include_bytes!("qr/qr-texto.png");

    /// Una pantalla de mentira: barra arriba, un panel y unas líneas de texto, con los QR
    /// puestos encima a `lado` píxeles por cuadradito.
    fn pantalla(codigos: &[(&[u8], u32, u32)], lado: u32) -> RgbaImage {
        let mut fondo = RgbaImage::from_fn(900, 560, |x, y| {
            if y < 36 {
                Rgba([38, 40, 48, 255])
            } else if x < 200 {
                Rgba([240, 240, 243, 255])
            } else if y % 28 < 9 && x < 200 + (y * 7) % 500 {
                Rgba([70, 72, 80, 255])
            } else {
                Rgba([252, 252, 252, 255])
            }
        });
        for (bytes, x, y) in codigos {
            let codigo = qr(bytes);
            let grande = imageops::resize(
                &codigo,
                codigo.width() * lado,
                codigo.height() * lado,
                imageops::FilterType::Nearest,
            );
            imageops::overlay(&mut fondo, &grande, *x as i64, *y as i64);
        }
        fondo
    }

    #[test]
    fn lee_el_enlace_de_un_qr_dentro_de_una_captura() {
        let captura = pantalla(&[(ENLACE, 420, 120)], 4);
        assert_eq!(leer(&captura), vec!["https://winshotx.com/?desde=qr".to_string()]);
    }

    /// Tildes, eñes y chino: el QR guarda bytes, y se leen como UTF-8.
    #[test]
    fn el_texto_sale_con_sus_tildes() {
        let captura = pantalla(&[(TEXTO, 300, 200)], 4);
        assert_eq!(leer(&captura), vec!["¿Qué tal? Ñandú, 你好".to_string()]);
    }

    #[test]
    fn dos_codigos_salen_los_dos_y_uno_repetido_una_vez() {
        let captura = pantalla(&[(ENLACE, 220, 60), (TEXTO, 620, 60), (ENLACE, 420, 330)], 4);
        let leidos = leer(&captura);
        assert_eq!(leidos.len(), 2, "{leidos:?}");
        assert!(leidos.contains(&"https://winshotx.com/?desde=qr".to_string()));
        assert!(leidos.contains(&"¿Qué tal? Ñandú, 你好".to_string()));
    }

    /// Un QR claro sobre fondo oscuro, como lo pintan muchas webs en modo oscuro.
    #[test]
    fn lee_un_qr_en_negativo() {
        let mut captura = pantalla(&[(ENLACE, 420, 120)], 4);
        imageops::invert(&mut captura);
        assert_eq!(leer(&captura), vec!["https://winshotx.com/?desde=qr".to_string()]);
    }

    /// Un QR diminuto, a un píxel por cuadradito, en una captura pequeña.
    #[test]
    fn lee_uno_de_un_pixel_por_cuadradito() {
        let captura = pantalla(&[(ENLACE, 420, 120)], 1);
        assert_eq!(leer(&captura), vec!["https://winshotx.com/?desde=qr".to_string()]);
    }

    #[test]
    fn sin_qr_no_se_inventa_nada() {
        assert!(leer(&pantalla(&[], 4)).is_empty());
    }
}
