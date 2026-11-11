//! Tapar con mosaico lo que hay que tapar de un vídeo, solo mientras se ve (D4).
//!
//! Es el mismo mosaico que tapa un correo en una captura (`anotacion`, la marca `blur`),
//! pero con tiempo: cada `Tapa` dice dónde, de 0 a 1 sobre la **imagen grabada**, y desde
//! cuándo hasta cuándo, en milisegundos de la grabación. Va sobre la imagen grabada y antes
//! del zoom a propósito: el dato está en la pantalla que se grabó, y si la cámara se acerca
//! o se aleja, el mosaico tiene que irse con él, no quedarse quieto en el vídeo exportado.
//!
//! **Un mosaico y no un desenfoque**, por lo mismo que en las capturas: el mosaico tira los
//! píxeles de verdad, y un desenfoque suave se puede deshacer con paciencia.
//!
//! Quién decide qué se tapa no es esto: aquí solo se pinta lo que manda el editor, que es
//! lo que el usuario ha visto en la línea de tiempo. Nunca se tapa nada en silencio.

use image::RgbaImage;

use crate::contrato::Tapa;
use crate::encode::anotacion::{self, Anotacion};

/// Si esa tapa está puesta en ese instante: desde su principio, sin incluir su final, y
/// con un rectángulo que se pueda pintar.
pub fn puesta(tapa: &Tapa, ms: u64) -> bool {
    ms >= tapa.desde_ms && ms < tapa.hasta_ms && valida(tapa)
}

/// Un rectángulo con tamaño y con números de verdad. Lo demás no tapa nada.
fn valida(tapa: &Tapa) -> bool {
    [tapa.x, tapa.y, tapa.ancho, tapa.alto].iter().all(|v| v.is_finite()) && tapa.ancho > 0.0 && tapa.alto > 0.0
}

/// Pone el mosaico de las tapas que tocan en ese instante sobre la imagen grabada.
///
/// Sin ninguna puesta no toca la imagen ni reserva nada: un vídeo sin tapas exporta
/// exactamente igual que antes, y al mismo ritmo.
pub fn pintar(imagen: &mut RgbaImage, tapas: &[Tapa], ms: u64) {
    pintar_estas(imagen, tapas.iter().filter(|t| puesta(t, ms)));
}

/// Lo mismo para un fotograma suelto que está en pantalla de `desde_ms` a `hasta_ms` (una
/// foto PNG o JPG sacada de un vídeo): lleva tapado todo lo que se tape en algún momento
/// de ese rato. Una pantalla quieta es un fotograma de varios segundos, y una tapa que
/// empieza a la mitad tiene que salir en su foto.
pub fn pintar_durante(imagen: &mut RgbaImage, tapas: &[Tapa], desde_ms: u64, hasta_ms: u64) {
    pintar_estas(
        imagen,
        tapas.iter().filter(|t| valida(t) && t.desde_ms < hasta_ms.max(desde_ms + 1) && t.hasta_ms > desde_ms),
    );
}

fn pintar_estas<'a>(imagen: &mut RgbaImage, tapas: impl Iterator<Item = &'a Tapa>) {
    let marcas: Vec<Anotacion> = tapas
        .map(|t| Anotacion {
            kind: "blur".into(),
            x1: t.x,
            y1: t.y,
            x2: t.x + t.ancho,
            y2: t.y + t.alto,
            ..Default::default()
        })
        .collect();
    if !marcas.is_empty() {
        anotacion::pintar(imagen, &marcas);
    }
}

/// Si en esos dos instantes están puestas las mismas tapas.
///
/// Lo pregunta el exportador antes de juntar dos pasos que repiten fotograma: una pantalla
/// quieta es UN fotograma de varios segundos, y si una tapa empieza a la mitad, juntarlo
/// todo dejaría el dato al aire hasta el fotograma siguiente.
pub fn igual(tapas: &[Tapa], a: u64, b: u64) -> bool {
    tapas.iter().all(|t| puesta(t, a) == puesta(t, b))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use image::Rgba;

    fn tapa(desde_ms: u64, hasta_ms: u64, x: f32, y: f32, ancho: f32, alto: f32) -> Tapa {
        Tapa { desde_ms, hasta_ms, x, y, ancho, alto }
    }

    /// Un tablero de rayas finas: cualquier mosaico encima se nota, porque deja cuadrados
    /// de un solo color donde había rayas de dos.
    fn rayas(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_fn(ancho, alto, |x, _| {
            if x % 2 == 0 { Rgba([0, 0, 0, 255]) } else { Rgba([255, 255, 255, 255]) }
        })
    }

    /// Cuántos píxeles de la zona son distintos de su vecino de la derecha: en las rayas,
    /// todos; bajo el mosaico, casi ninguno.
    fn cambios_en(imagen: &RgbaImage, x1: u32, y1: u32, x2: u32, y2: u32) -> usize {
        let mut cuantos = 0;
        for y in y1..y2 {
            for x in x1..x2.saturating_sub(1) {
                if imagen.get_pixel(x, y) != imagen.get_pixel(x + 1, y) {
                    cuantos += 1;
                }
            }
        }
        cuantos
    }

    #[test]
    fn tapa_solo_mientras_esta_puesta() {
        let tapas = [tapa(1_000, 2_000, 0.25, 0.25, 0.5, 0.5)];
        for (ms, tapado) in [(0, false), (999, false), (1_000, true), (1_999, true), (2_000, false)] {
            let mut imagen = rayas(80, 40);
            pintar(&mut imagen, &tapas, ms);
            let dentro = cambios_en(&imagen, 22, 12, 58, 28);
            if tapado {
                assert!(dentro < 36 * 16 / 4, "a los {ms} ms la zona tenía que ir tapada: {dentro} cambios");
            } else {
                assert_eq!(imagen, rayas(80, 40), "a los {ms} ms no se tapa nada");
            }
        }
    }

    /// Lo de fuera del rectángulo no se toca: tapar un correo no emborrona la línea de al lado.
    #[test]
    fn fuera_del_rectangulo_no_se_toca_nada() {
        let mut imagen = rayas(100, 50);
        pintar(&mut imagen, &[tapa(0, 10, 0.4, 0.4, 0.2, 0.2)], 5);
        let limpia = rayas(100, 50);
        for (x, y, p) in imagen.enumerate_pixels() {
            // El mosaico de las capturas cuenta el borde de abajo y el de la derecha como
            // dentro (`anotacion::difuminar`): un píxel de más, nunca uno de menos.
            let dentro = (40..=60).contains(&x) && (20..=30).contains(&y);
            if !dentro {
                assert_eq!(p, limpia.get_pixel(x, y), "se ha tocado ({x}, {y}), que estaba fuera");
            }
        }
        assert!(cambios_en(&imagen, 40, 20, 60, 30) < 20 * 10 / 3, "y lo de dentro sí va tapado");
    }

    /// Va de 0 a 1 sobre la imagen grabada: la misma tapa cae en el mismo sitio a cualquier
    /// tamaño de grabación.
    #[test]
    fn el_rectangulo_es_de_0_a_1_sobre_la_imagen() {
        for (ancho, alto) in [(200, 100), (1920, 1080)] {
            let mut imagen = rayas(ancho, alto);
            pintar(&mut imagen, &[tapa(0, 10, 0.5, 0.5, 0.25, 0.25)], 0);
            let (x1, y1) = (ancho / 2, alto / 2);
            let (x2, y2) = (x1 + ancho / 4, y1 + alto / 4);
            let total = ((x2 - x1 - 1) * (y2 - y1)) as usize;
            assert!(cambios_en(&imagen, x1 + 1, y1 + 1, x2 - 1, y2 - 1) < total / 3, "a {ancho} x {alto}");
            assert_eq!(cambios_en(&imagen, 0, 0, ancho, alto / 2 - 1), ((ancho - 1) * (alto / 2 - 1)) as usize);
        }
    }

    /// Una tapa sin tamaño, con números rotos o del revés no pinta nada ni revienta.
    #[test]
    fn una_tapa_rota_no_tapa_nada() {
        let rotas = [
            tapa(0, 10, 0.2, 0.2, 0.0, 0.3),
            tapa(0, 10, f32::NAN, 0.2, 0.3, 0.3),
            tapa(10, 0, 0.2, 0.2, 0.3, 0.3),
            tapa(0, 10, 0.2, 0.2, -0.1, 0.3),
        ];
        let mut imagen = rayas(60, 30);
        pintar(&mut imagen, &rotas, 5);
        assert_eq!(imagen, rayas(60, 30));
    }

    /// Lo que se sale de la imagen se tapa hasta el borde, no se pierde: un dato pegado al
    /// borde tiene que quedar tapado igual.
    #[test]
    fn lo_que_se_sale_por_el_borde_se_tapa_hasta_el_borde() {
        let mut imagen = rayas(60, 30);
        pintar(&mut imagen, &[tapa(0, 10, 0.8, 0.5, 0.5, 0.8)], 0);
        assert!(cambios_en(&imagen, 49, 16, 60, 30) < 11 * 14 / 3);
    }

    /// Una foto de un fotograma que dura de 1 a 4 s lleva la tapa que empieza a los 3 s, y
    /// no la que acabó antes de que saliera ese fotograma.
    #[test]
    fn una_foto_lleva_lo_que_se_tapa_mientras_esta_en_pantalla() {
        let tapas = [tapa(3_000, 5_000, 0.0, 0.0, 0.5, 1.0), tapa(0, 1_000, 0.5, 0.0, 0.5, 1.0)];
        let mut imagen = rayas(80, 40);
        pintar_durante(&mut imagen, &tapas, 1_000, 4_000);
        assert!(cambios_en(&imagen, 0, 0, 40, 40) < 39 * 40 / 3, "la de los 3 s va puesta");
        assert_eq!(cambios_en(&imagen, 41, 0, 80, 40), 38 * 40, "la que acabó al empezar este fotograma, no");
    }

    #[test]
    fn dos_instantes_son_iguales_si_tienen_las_mismas_tapas() {
        let tapas = [tapa(1_000, 2_000, 0.1, 0.1, 0.2, 0.2), tapa(1_500, 3_000, 0.5, 0.5, 0.2, 0.2)];
        assert!(igual(&tapas, 0, 999));
        assert!(igual(&tapas, 1_000, 1_499));
        assert!(!igual(&tapas, 999, 1_000), "empieza la primera");
        assert!(!igual(&tapas, 1_499, 1_500), "empieza la segunda");
        assert!(!igual(&tapas, 1_999, 2_000), "acaba la primera");
        assert!(igual(&[], 0, 10_000), "sin tapas, todo es igual");
    }
}
