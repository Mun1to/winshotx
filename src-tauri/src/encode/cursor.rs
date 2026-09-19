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
    for (canal, nuevo) in p.0.iter_mut().zip(color).take(3) {
        *canal = (*canal as f32 * (1.0 - a) + nuevo as f32 * a).round() as u8;
    }
}

/// La barra de texto, la que sale encima de un campo donde se escribe: una I con sus dos
/// remates. Su punto caliente es el centro.
const BARRA_TEXTO: [(f32, f32); 12] = [
    (0.20, 0.00),
    (0.80, 0.00),
    (0.80, 0.10),
    (0.55, 0.10),
    (0.55, 0.90),
    (0.80, 0.90),
    (0.80, 1.00),
    (0.20, 1.00),
    (0.20, 0.90),
    (0.45, 0.90),
    (0.45, 0.10),
    (0.20, 0.10),
];

/// La manita de los enlaces: el índice arriba y los otros tres dedos recogidos, con el
/// pulgar a la izquierda. Su punto caliente es la yema del índice.
const MANO: [(f32, f32); 17] = [
    (0.30, 0.00),
    (0.44, 0.00),
    (0.44, 0.42),
    (0.58, 0.40),
    (0.58, 0.48),
    (0.72, 0.46),
    (0.72, 0.54),
    (0.84, 0.52),
    (0.84, 0.60),
    (0.82, 0.80),
    (0.70, 1.00),
    (0.30, 1.00),
    (0.10, 0.78),
    (0.00, 0.55),
    (0.06, 0.48),
    (0.18, 0.52),
    (0.30, 0.62),
];

/// Las tres formas que se distinguen al grabar. Se guardan como número en la sesión.
///
/// Solo estas tres porque son las que cambian lo que se entiende: sobre un campo de texto
/// sale la barra, sobre un enlace la manita, y en todo lo demás la flecha. Los cursores de
/// esperar, redimensionar y demás son un instante y se dibujan como flecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Forma {
    Flecha = 0,
    Texto = 1,
    Mano = 2,
}

impl Forma {
    /// Desde el número guardado. Un número desconocido es la flecha, que no falla nunca.
    pub fn desde(n: u8) -> Self {
        match n {
            1 => Self::Texto,
            2 => Self::Mano,
            _ => Self::Flecha,
        }
    }

    /// Los puntos de la forma, dónde está su punto caliente (de 0 a 1), y si va clara
    /// (blanca con borde oscuro, como la manita) o oscura (negra con borde blanco).
    fn dibujo(self) -> (&'static [(f32, f32)], (f32, f32), bool) {
        match self {
            Self::Flecha => (&FLECHA, (0.0, 0.0), false),
            Self::Texto => (&BARRA_TEXTO, (0.5, 0.5), false),
            Self::Mano => (&MANO, (0.37, 0.0), true),
        }
    }
}

/// Dibuja la flecha con la punta en `(x, y)`.
///
/// `alto` es lo que mide de arriba abajo en píxeles del fotograma; el ancho sale de la
/// forma. El borde blanco tiene un grosor proporcional, para que a tamaño grande no se
/// quede como un pelo.
pub fn pintar(imagen: &mut RgbaImage, x: i32, y: i32, alto: f32) {
    pintar_forma(imagen, x, y, alto, Forma::Flecha);
}

/// Dibuja el puntero de esa forma, con su punto caliente en `(x, y)`.
pub fn pintar_forma(imagen: &mut RgbaImage, x: i32, y: i32, alto: f32, forma: Forma) {
    if alto < 4.0 {
        return;
    }
    let (puntos, (cu, cv), clara) = forma.dibujo();
    let (relleno, contorno) = if clara {
        ([250u8, 250, 250], [16u8, 16, 16])
    } else {
        ([16u8, 16, 16], [255u8, 255, 255])
    };
    let borde = (alto * 0.055).max(1.0);
    // La caja que hay que recorrer: la forma más el borde, que sobresale por fuera. La
    // forma va de 0 a 1 en cada eje, desplazada para que el punto caliente caiga en (x, y).
    let origen_x = x as f32 - cu * alto;
    let origen_y = y as f32 - cv * alto;
    let margen = borde + 1.0;
    let desde_x = (origen_x - margen).floor() as i32;
    let hasta_x = (origen_x + alto + margen).ceil() as i32;
    let desde_y = (origen_y - margen).floor() as i32;
    let hasta_y = (origen_y + alto + margen).ceil() as i32;

    for py in desde_y..=hasta_y {
        for px in desde_x..=hasta_x {
            // El punto, llevado al sistema de la forma.
            let u = (px as f32 + 0.5 - origen_x) / alto;
            let v = (py as f32 + 0.5 - origen_y) / alto;
            let d = al_borde(u, v, puntos) * alto;
            if dentro(u, v, puntos) {
                // Dentro: el relleno, y el contorno pegado al borde por la parte de adentro.
                if d < borde * 0.5 {
                    mezclar(imagen, px, py, contorno, (borde * 0.5 - d).min(1.0));
                } else {
                    mezclar(imagen, px, py, relleno, 1.0);
                }
            } else if d < borde {
                // Fuera pero cerca: el contorno, que se apaga medio píxel más allá.
                mezclar(imagen, px, py, contorno, (borde - d).min(1.0));
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

    /// La barra de texto va centrada en el punto: tiene que haber tinta a la izquierda y a
    /// la derecha del punto caliente, no solo hacia un lado como la flecha.
    #[test]
    fn la_barra_de_texto_va_centrada_en_el_punto() {
        let mut i = lienzo();
        pintar_forma(&mut i, 60, 60, 40.0, Forma::Texto);
        let fondo = image::Rgba([120, 120, 120, 255]);
        assert_ne!(*i.get_pixel(60, 60), fondo, "el centro de la I tiene que estar pintado");
        assert_ne!(*i.get_pixel(50, 42), fondo, "el remate de arriba llega a la izquierda");
        assert_ne!(*i.get_pixel(70, 42), fondo, "y a la derecha");
        assert_eq!(*i.get_pixel(60, 20), fondo, "por encima del remate no hay nada");
    }

    /// La manita va clara con borde oscuro, como la de Windows, y con la yema del indice
    /// en el punto caliente: por encima del punto no hay nada.
    #[test]
    fn la_manita_es_clara_y_apunta_con_el_indice() {
        let mut i = lienzo();
        pintar_forma(&mut i, 50, 30, 50.0, Forma::Mano);
        let claros = i.pixels().filter(|p| p.0[0] > 240).count();
        let oscuros = i.pixels().filter(|p| p.0[0] < 40).count();
        assert!(claros > 200, "la manita tiene que ser blanca por dentro: {claros}");
        assert!(oscuros > 30, "y llevar borde oscuro: {oscuros}");
        assert_eq!(*i.get_pixel(50, 20), image::Rgba([120, 120, 120, 255]));
    }

    #[test]
    fn un_numero_desconocido_es_la_flecha() {
        assert_eq!(Forma::desde(0), Forma::Flecha);
        assert_eq!(Forma::desde(1), Forma::Texto);
        assert_eq!(Forma::desde(2), Forma::Mano);
        assert_eq!(Forma::desde(77), Forma::Flecha);
    }

    /// No comprueba nada: deja un PNG para mirar las formas con los ojos.
    #[test]
    #[ignore]
    fn ver_el_cursor() {
        let mut i = RgbaImage::from_pixel(260, 260, image::Rgba([200, 205, 215, 255]));
        for (n, alto) in [18.0f32, 28.0, 44.0, 70.0].iter().enumerate() {
            pintar(&mut i, 20 + n as i32 * 60, 30, *alto);
            pintar_forma(&mut i, 40 + n as i32 * 60, 120, *alto, Forma::Texto);
            pintar_forma(&mut i, 20 + n as i32 * 60, 180, *alto, Forma::Mano);
        }
        let destino = std::env::temp_dir().join("winshotx-cursor.png");
        i.save(&destino).unwrap();
        eprintln!("[cursor] mira {}", destino.display());
    }
}
