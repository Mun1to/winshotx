//! La lupa: amplía un trozo del vídeo encima de él, sin mover la vista (A6).
//!
//! El zoom acerca la cámara entera; la lupa deja el vídeo como está y pone encima una
//! lente con lo de debajo más grande. Sirve para lo pequeño que hay que leer (un menú, una
//! cifra) sin perder de vista dónde está.
//!
//! Se pinta sobre el fotograma ya recortado, acercado y escalado, y antes del marco: así
//! mide lo que mide en el vídeo que se lleva quien lo guarda, y lo que hay debajo (el
//! puntero dibujado incluido) sale ampliado, como en una lupa de verdad.

use image::{Rgba, RgbaImage};

use super::recorte::Recorte;
use crate::contrato::Forma;

/// Una lupa en un fotograma, ya en píxeles de ese fotograma.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lente {
    /// Lo que se amplía: el centro de lo que se mira.
    pub cx: f32,
    pub cy: f32,
    pub forma: Forma,
    /// Con un borde blanco alrededor.
    pub marco: bool,
    /// Lo que mide la lente.
    pub ancho: f32,
    pub alto: f32,
    /// Cuánto amplía: 2 es el doble.
    pub aumento: f32,
}

/// Lo alto que es un rectángulo respecto a su ancho: el de una tarjeta, que es lo que
/// cabe al ampliar una línea de texto o una fila de botones.
const PROPORCION_RECTANGULO: f32 = 0.6;

/// Lo que mide la lente en un fotograma de `ancho` por `alto`, con su tamaño de 0 a 1
/// sobre el ancho. El círculo y el cuadrado son tan altos como anchos; los rectángulos,
/// más bajos. Nunca más grande que el fotograma.
pub fn medidas(forma: Forma, tam: f32, ancho: u32, alto: u32) -> (f32, f32) {
    let tam = if tam.is_finite() { tam.clamp(0.05, 1.0) } else { 0.3 };
    let (w, h) = (ancho as f32, alto as f32);
    let lado = tam * w;
    match forma {
        Forma::Circulo | Forma::Cuadrado => {
            let l = lado.min(h);
            (l, l)
        }
        Forma::Rectangulo | Forma::Redondeado => (lado.min(w), (lado * PROPORCION_RECTANGULO).min(h)),
    }
}

/// Lleva un punto de la imagen grabada entera (de 0 a 1) al fotograma, pasando por los
/// recortes en el orden en que se aplicaron. `None` si se quedó fuera de lo que se ve.
pub fn colocar(u: f32, v: f32, recortes: &[Recorte]) -> Option<(f32, f32)> {
    let (mut u, mut v) = (u, v);
    for r in recortes {
        let (x1, y1, x2, y2) = (r.x1.min(r.x2), r.y1.min(r.y2), r.x1.max(r.x2), r.y1.max(r.y2));
        u = (u - x1) / (x2 - x1).max(f32::EPSILON);
        v = (v - y1) / (y2 - y1).max(f32::EPSILON);
    }
    ((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v)).then_some((u, v))
}

/// La distancia con signo al borde de una caja de esquinas redondeadas centrada en el
/// origen: negativa dentro, positiva fuera. Con el radio igual a la mitad del lado es un
/// círculo; con radio cero, un rectángulo.
fn distancia(x: f32, y: f32, medio_ancho: f32, medio_alto: f32, radio: f32) -> f32 {
    let qx = x.abs() - (medio_ancho - radio);
    let qy = y.abs() - (medio_alto - radio);
    let fuera = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    fuera + qx.max(qy).min(0.0) - radio
}

/// El color de `imagen` en un punto que no cae en un píxel entero, mezclando los cuatro
/// de alrededor. Fuera de la imagen vale el borde.
fn bilineal(imagen: &RgbaImage, x: f32, y: f32) -> [f32; 4] {
    let (w, h) = (imagen.width() as i64, imagen.height() as i64);
    let x = x.clamp(0.0, (w - 1).max(0) as f32);
    let y = y.clamp(0.0, (h - 1).max(0) as f32);
    let (x0, y0) = (x.floor() as i64, y.floor() as i64);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let p = |x: i64, y: i64| imagen.get_pixel(x as u32, y as u32).0;
    let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
    let mut salida = [0.0; 4];
    for i in 0..4 {
        let arriba = a[i] as f32 + (b[i] as f32 - a[i] as f32) * fx;
        let abajo = c[i] as f32 + (d[i] as f32 - c[i] as f32) * fx;
        salida[i] = arriba + (abajo - arriba) * fy;
    }
    salida
}

/// Pinta la lente encima del fotograma.
///
/// La lente se centra en lo que amplía, salvo que no quepa: pegada a un borde se mete
/// dentro, y lo de dentro sigue siendo lo del punto, no lo del centro de la lente.
pub fn pintar(imagen: &mut RgbaImage, lente: &Lente) {
    let (w, h) = (imagen.width() as f32, imagen.height() as f32);
    let (lw, lh) = (lente.ancho.min(w), lente.alto.min(h));
    if lw < 4.0 || lh < 4.0 {
        return;
    }
    let aumento = if lente.aumento.is_finite() { lente.aumento.clamp(1.0, 8.0) } else { 2.0 };
    let (mx, my) = (lw / 2.0, lh / 2.0);
    let lx = lente.cx.clamp(mx, w - mx);
    let ly = lente.cy.clamp(my, h - my);
    let radio = match lente.forma {
        Forma::Circulo => mx.min(my),
        Forma::Cuadrado | Forma::Rectangulo => 0.0,
        Forma::Redondeado => mx.min(my) * 0.36,
    };
    let borde = if lente.marco { (w * 0.0028).clamp(2.0, 6.0) } else { 0.0 };

    // Lo de debajo se copia antes de pintar: la lente se lee a sí misma si no.
    let (fx0, fy0) = (
        (lente.cx - mx / aumento - 2.0).floor().clamp(0.0, w - 1.0) as u32,
        (lente.cy - my / aumento - 2.0).floor().clamp(0.0, h - 1.0) as u32,
    );
    let (fx1, fy1) = (
        (lente.cx + mx / aumento + 2.0).ceil().clamp(1.0, w) as u32,
        (lente.cy + my / aumento + 2.0).ceil().clamp(1.0, h) as u32,
    );
    if fx1 <= fx0 || fy1 <= fy0 {
        return;
    }
    let fuente = image::imageops::crop_imm(imagen, fx0, fy0, fx1 - fx0, fy1 - fy0).to_image();

    let x0 = (lx - mx - 2.0).floor().max(0.0) as u32;
    let y0 = (ly - my - 2.0).floor().max(0.0) as u32;
    let x1 = ((lx + mx + 2.0).ceil() as u32).min(imagen.width());
    let y1 = ((ly + my + 2.0).ceil() as u32).min(imagen.height());
    let mezclar = |a: [f32; 4], b: [f32; 4], t: f32| -> [f32; 4] {
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
    };
    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - lx;
            let dy = py as f32 + 0.5 - ly;
            let d = distancia(dx, dy, mx, my, radio);
            // Lo que cae dentro de la lente, con el borde suavizado medio píxel.
            let dentro = (0.5 - d).clamp(0.0, 1.0);
            // La raya oscura de fuera, un píxel, solo con marco: separa el borde blanco de
            // un fondo blanco.
            let raya = if lente.marco { ((1.5 - d).clamp(0.0, 1.0) - dentro).max(0.0) * 0.32 } else { 0.0 };
            if dentro <= 0.0 && raya <= 0.0 {
                continue;
            }
            let antes = imagen.get_pixel(px, py).0;
            let mut color = [antes[0] as f32, antes[1] as f32, antes[2] as f32, antes[3] as f32];
            if dentro > 0.0 {
                let sx = lente.cx + dx / aumento - fx0 as f32 - 0.5;
                let sy = lente.cy + dy / aumento - fy0 as f32 - 0.5;
                color = mezclar(color, bilineal(&fuente, sx, sy), dentro);
                if borde > 0.0 {
                    let interior = (0.5 - (d + borde)).clamp(0.0, 1.0);
                    color = mezclar(color, [255.0, 255.0, 255.0, 255.0], dentro - interior);
                }
            }
            if raya > 0.0 {
                color = mezclar(color, [0.0, 0.0, 0.0, 255.0], raya);
            }
            imagen.put_pixel(
                px,
                py,
                Rgba([
                    color[0].round().clamp(0.0, 255.0) as u8,
                    color[1].round().clamp(0.0, 255.0) as u8,
                    color[2].round().clamp(0.0, 255.0) as u8,
                    color[3].round().clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un fondo gris con un cuadrado rojo de 10 píxeles en el centro.
    fn con_cuadro() -> RgbaImage {
        RgbaImage::from_fn(200, 120, |x, y| {
            if (95..105).contains(&x) && (55..65).contains(&y) {
                Rgba([230, 20, 20, 255])
            } else {
                Rgba([120, 120, 120, 255])
            }
        })
    }

    fn rojo(p: &Rgba<u8>) -> bool {
        p.0[0] > 180 && p.0[1] < 80
    }

    fn lente(forma: Forma, marco: bool) -> Lente {
        Lente { cx: 100.0, cy: 60.0, forma, marco, ancho: 80.0, alto: 80.0, aumento: 3.0 }
    }

    /// Lo de dentro sale ampliado: el cuadrado de 10 pasa a medir 30 a lo ancho.
    #[test]
    fn amplia_lo_de_debajo() {
        let mut imagen = con_cuadro();
        pintar(&mut imagen, &lente(Forma::Cuadrado, false));
        let ancho_rojo = (0..200).filter(|&x| rojo(imagen.get_pixel(x, 60))).count();
        assert!((28..=32).contains(&ancho_rojo), "el rojo mide {ancho_rojo} y tenía que medir unos 30");
    }

    /// Fuera de la lente no se toca nada.
    #[test]
    fn fuera_no_toca_nada() {
        let original = con_cuadro();
        let mut imagen = original.clone();
        pintar(&mut imagen, &lente(Forma::Circulo, true));
        for (x, y) in [(0, 0), (199, 119), (40, 10), (160, 110)] {
            assert_eq!(imagen.get_pixel(x, y), original.get_pixel(x, y), "en {x},{y}");
        }
    }

    /// El círculo es redondo: la esquina de su caja se queda como estaba, el cuadrado no.
    #[test]
    fn el_circulo_deja_las_esquinas_y_el_cuadrado_no() {
        let mut circulo = con_cuadro();
        pintar(&mut circulo, &lente(Forma::Circulo, true));
        let mut cuadrado = con_cuadro();
        pintar(&mut cuadrado, &lente(Forma::Cuadrado, true));
        // La esquina de arriba a la izquierda de la caja de la lente, un píxel dentro.
        assert_eq!(circulo.get_pixel(61, 21), &Rgba([120, 120, 120, 255]), "el círculo no llega a la esquina");
        assert_eq!(cuadrado.get_pixel(61, 21), &Rgba([255, 255, 255, 255]), "el cuadrado sí, con su borde blanco");
    }

    /// Con marco hay un borde blanco; sin marco, lo ampliado llega hasta el filo.
    #[test]
    fn con_marco_y_sin_marco() {
        let mut con = con_cuadro();
        pintar(&mut con, &lente(Forma::Cuadrado, true));
        let mut sin = con_cuadro();
        pintar(&mut sin, &lente(Forma::Cuadrado, false));
        assert_eq!(con.get_pixel(61, 60), &Rgba([255, 255, 255, 255]));
        assert_eq!(sin.get_pixel(61, 60), &Rgba([120, 120, 120, 255]));
    }

    /// Pegada a un borde, la lente se mete dentro del fotograma, y lo de dentro sigue
    /// siendo lo del punto: el rojo de la esquina sale ampliado en la lente.
    #[test]
    fn pegada_al_borde_se_mete_dentro() {
        let mut imagen = RgbaImage::from_fn(200, 120, |x, y| {
            if x < 6 && y < 6 { Rgba([230, 20, 20, 255]) } else { Rgba([120, 120, 120, 255]) }
        });
        pintar(&mut imagen, &Lente { cx: 2.0, cy: 2.0, ..lente(Forma::Cuadrado, false) });
        let rojos = imagen.pixels().filter(|p| rojo(p)).count();
        assert!(rojos > 36 * 4, "el rojo de la esquina tenía que salir ampliado: {rojos} píxeles");
    }

    /// Los rectángulos son más bajos que anchos, y nada mide más que el fotograma.
    #[test]
    fn las_medidas_de_cada_forma() {
        assert_eq!(medidas(Forma::Circulo, 0.25, 1920, 1080), (480.0, 480.0));
        assert_eq!(medidas(Forma::Rectangulo, 0.25, 1920, 1080), (480.0, 288.0));
        assert_eq!(medidas(Forma::Cuadrado, 1.0, 1920, 1080), (1080.0, 1080.0));
    }

    /// Un punto en medio de un recorte cae en medio del fotograma; uno fuera no se ve.
    #[test]
    fn colocar_pasa_por_los_recortes() {
        let mitad_derecha = Recorte { x1: 0.5, y1: 0.0, x2: 1.0, y2: 1.0 };
        assert_eq!(colocar(0.75, 0.5, &[mitad_derecha]), Some((0.5, 0.5)));
        assert_eq!(colocar(0.25, 0.5, &[mitad_derecha]), None);
        assert_eq!(colocar(0.3, 0.4, &[]), Some((0.3, 0.4)));
    }
}
