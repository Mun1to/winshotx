//! El puntero del ratón, dibujado al exportar.
//!
//! Windows entrega el cursor cocido dentro de la captura y del tamaño que tenga puesto el
//! sistema, que en un monitor grande son treinta píxeles y en un vídeo compartido no se ve.
//! Dibujarlo aquí permite hacerlo **más grande** sin que se pixele, porque no se amplía una
//! imagen: se dibuja otra vez con la forma a otra escala.
//!
//! **Es la flecha estándar de Windows, no la que tenga puesta cada uno.** Quien use un
//! puntero personalizado verá otro distinto en el vídeo, y por eso esto no se enciende solo.
//!
//! La forma es la de siempre: un triángulo alargado con la punta arriba a la izquierda y
//! una cola que sale hacia abajo a la derecha. Relleno negro y borde blanco, que es lo que
//! la hace visible tanto sobre claro como sobre oscuro.

use image::RgbaImage;

/// La flecha, en un sistema de 0 a 1 donde 1 es el alto del cursor.
///
/// Sale de medir el puntero de Windows: mide 12 de ancho por 20 de alto, la muesca de la
/// cola está a media altura y la cola baja hasta abajo del todo.
const FLECHA: [(f32, f32); 7] = [
    (0.00, 0.00),
    (0.00, 0.82),
    (0.22, 0.63),
    (0.38, 1.00),
    (0.55, 0.93),
    (0.39, 0.57),
    (0.62, 0.57),
];

/// Si un punto cae dentro del polígono, contando cruces con una semirrecta.
fn dentro(px: f32, py: f32, puntos: &[(f32, f32)]) -> bool {
    let mut dentro = false;
    let mut j = puntos.len() - 1;
    for i in 0..puntos.len() {
        let (xi, yi) = puntos[i];
        let (xj, yj) = puntos[j];
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            dentro = !dentro;
        }
        j = i;
    }
    dentro
}

/// La distancia de un punto al borde del polígono, para poder pintar el contorno.
fn al_borde(px: f32, py: f32, puntos: &[(f32, f32)]) -> f32 {
    let mut minima = f32::MAX;
    let mut j = puntos.len() - 1;
    for i in 0..puntos.len() {
        let (xi, yi) = puntos[i];
        let (xj, yj) = puntos[j];
        let (dx, dy) = (xj - xi, yj - yi);
        let largo = dx * dx + dy * dy;
        let t = if largo <= f32::EPSILON {
            0.0
        } else {
            (((px - xi) * dx + (py - yi) * dy) / largo).clamp(0.0, 1.0)
        };
        let (cx, cy) = (xi + t * dx, yi + t * dy);
        minima = minima.min(((px - cx).powi(2) + (py - cy).powi(2)).sqrt());
        j = i;
    }
    minima
}

/// Mezcla un color sobre el fotograma con esa opacidad.
fn mezclar(imagen: &mut RgbaImage, x: i32, y: i32, color: [u8; 3], alfa: f32) {
    if alfa <= 0.0 || x < 0 || y < 0 || x >= imagen.width() as i32 || y >= imagen.height() as i32 {
        return;
    }
    let p = imagen.get_pixel_mut(x as u32, y as u32);
    let a = alfa.clamp(0.0, 1.0);
    for i in 0..3 {
        p.0[i] = (p.0[i] as f32 * (1.0 - a) + color[i] as f32 * a).round() as u8;
    }
}

/// Dibuja el puntero con la punta en `(x, y)`.
///
/// `alto` es lo que mide de arriba abajo en píxeles del fotograma; el ancho sale de la
/// forma. El borde blanco tiene un grosor proporcional, para que a tamaño grande no se
/// quede como un pelo.
pub fn pintar(imagen: &mut RgbaImage, x: i32, y: i32, alto: f32) {
    if alto < 4.0 {
        return;
    }
    let borde = (alto * 0.055).max(1.0);
    // La caja que hay que recorrer: la forma más el borde, que sobresale por fuera.
    let ancho = alto * 0.62;
    let margen = borde + 1.0;
    let desde_x = (x as f32 - margen).floor() as i32;
    let hasta_x = (x as f32 + ancho + margen).ceil() as i32;
    let desde_y = (y as f32 - margen).floor() as i32;
    let hasta_y = (y as f32 + alto + margen).ceil() as i32;

    for py in desde_y..=hasta_y {
        for px in desde_x..=hasta_x {
            // El punto, llevado al sistema de la forma.
            let u = (px as f32 + 0.5 - x as f32) / alto;
            let v = (py as f32 + 0.5 - y as f32) / alto;
            let d = al_borde(u, v, &FLECHA) * alto;
            if dentro(u, v, &FLECHA) {
                // Dentro: negro, y blanco pegado al borde por la parte de adentro.
                if d < borde * 0.5 {
                    mezclar(imagen, px, py, [255, 255, 255], (borde * 0.5 - d).min(1.0));
                } else {
                    mezclar(imagen, px, py, [16, 16, 16], 1.0);
                }
            } else if d < borde {
                // Fuera pero cerca: el borde blanco, que se apaga medio píxel más allá.
                mezclar(imagen, px, py, [255, 255, 255], (borde - d).min(1.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lienzo() -> RgbaImage {
        RgbaImage::from_pixel(120, 120, image::Rgba([120, 120, 120, 255]))
    }

    fn tocados(imagen: &RgbaImage) -> usize {
        imagen
            .pixels()
            .filter(|p| p.0 != [120, 120, 120, 255])
            .count()
    }

    #[test]
    fn dibuja_algo_donde_se_le_dice() {
        let mut i = lienzo();
        pintar(&mut i, 20, 20, 30.0);
        assert!(tocados(&i) > 100, "no ha dibujado casi nada");
        // La punta esta en el punto que se pidio, asi que justo ahi hay cursor.
        assert_ne!(*i.get_pixel(21, 25), image::Rgba([120, 120, 120, 255]));
        // Y a la izquierda de la punta no hay nada, porque la flecha va hacia la derecha.
        assert_eq!(*i.get_pixel(5, 60), image::Rgba([120, 120, 120, 255]));
    }

    #[test]
    fn mas_grande_ocupa_mas() {
        // Es la razon entera de dibujarlo aqui: poder agrandarlo sin pixelar nada.
        let (mut a, mut b) = (lienzo(), lienzo());
        pintar(&mut a, 20, 20, 20.0);
        pintar(&mut b, 20, 20, 60.0);
        assert!(
            tocados(&b) > tocados(&a) * 4,
            "al triple de alto tendria que ocupar bastante mas: {} vs {}",
            tocados(&a),
            tocados(&b)
        );
    }

    #[test]
    fn tiene_borde_blanco_y_relleno_oscuro() {
        // Sin el borde, sobre un fondo negro el cursor desaparece.
        let mut i = lienzo();
        pintar(&mut i, 30, 30, 40.0);
        let claros = i.pixels().filter(|p| p.0[0] > 200).count();
        let oscuros = i.pixels().filter(|p| p.0[0] < 60).count();
        assert!(claros > 20, "no hay borde blanco");
        assert!(oscuros > 100, "no hay relleno oscuro");
    }

    #[test]
    fn en_el_borde_de_la_imagen_no_se_sale_ni_revienta() {
        let mut i = lienzo();
        pintar(&mut i, 118, 118, 40.0);
        pintar(&mut i, -10, -10, 40.0);
        assert_eq!(i.width(), 120);
    }

    #[test]
    fn un_cursor_diminuto_no_se_dibuja() {
        // Por debajo de cuatro pixeles es una mancha, y una mancha en mitad del video
        // parece un fallo del codificador.
        let mut i = lienzo();
        pintar(&mut i, 40, 40, 2.0);
        assert_eq!(tocados(&i), 0);
    }

    /// No comprueba nada: deja un PNG para mirar la forma con los ojos.
    #[test]
    #[ignore]
    fn ver_el_cursor() {
        let mut i = RgbaImage::from_pixel(260, 140, image::Rgba([200, 205, 215, 255]));
        for (n, alto) in [18.0f32, 28.0, 44.0, 70.0].iter().enumerate() {
            pintar(&mut i, 20 + n as i32 * 60, 30, *alto);
        }
        let destino = std::env::temp_dir().join("winshotx-cursor.png");
        i.save(&destino).unwrap();
        eprintln!("[cursor] mira {}", destino.display());
    }
}
