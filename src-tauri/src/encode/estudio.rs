//! Los aros de los clics y la pastilla de los atajos, dibujados **al exportar**.
//!
//! Antes se pintaban sobre los fotogramas mientras se grababa, y eso tenía dos problemas.
//!
//! El primero es que quedaban cocidos: había que decidir antes de grabar si se querían, y
//! si te arrepentías, a grabar otra vez. El segundo aparece con el zoom: un aro cocido se
//! amplía con la imagen, así que al acercarse la cámara salía gordo y borroso. Dibujándolo
//! aquí sale del mismo tamaño y nítido, esté la cámara donde esté.
//!
//! Lo que se guarda al grabar es una lista de puntos y de textos. Todo lo demás es
//! aritmética sobre fotogramas que ya están en disco.

use image::RgbaImage;

use crate::encode::recorte::Recorte;
use crate::encode::zoom::Clic;
use crate::record::{pastilla, realce, teclas::Atajo};

/// Qué se dibuja encima. Lo elige quien exporta.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ajustes {
    /// Un aro donde se pulsó.
    pub clics: bool,
    /// Una pastilla abajo con el atajo que se acaba de pulsar.
    pub teclas: bool,
}

impl Ajustes {
    pub fn hay_algo(&self) -> bool {
        self.clics || self.teclas
    }
}

/// Lleva un punto de la región grabada al fotograma que se va a escribir.
///
/// Por el mismo camino que las anotaciones: cada recorte lo vuelve a medir, y al final se
/// pasa al tamaño de salida. Sin esto, un aro dibujado con el zoom puesto caería donde
/// estaba el clic en la pantalla, no donde está ahora en el cuadro.
///
/// Devuelve `None` si el punto se quedó fuera de algún recorte: un clic que no se ve no
/// tiene aro.
fn colocar(
    x: i32,
    y: i32,
    origen: (u32, u32),
    recortes: &[Recorte],
    destino: (u32, u32),
) -> Option<(i32, i32)> {
    let (mut u, mut v) = (
        x as f32 / origen.0.max(1) as f32,
        y as f32 / origen.1.max(1) as f32,
    );
    for r in recortes {
        let (x1, y1, x2, y2) = (
            r.x1.min(r.x2),
            r.y1.min(r.y2),
            r.x1.max(r.x2),
            r.y1.max(r.y2),
        );
        u = (u - x1) / (x2 - x1).max(f32::EPSILON);
        v = (v - y1) / (y2 - y1).max(f32::EPSILON);
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            return None;
        }
    }
    Some((
        (u * destino.0 as f32).round() as i32,
        (v * destino.1 as f32).round() as i32,
    ))
}

/// Dibuja sobre el fotograma lo que toque en ese instante.
///
/// `origen` es el tamaño de la región grabada, que es en cuyo sistema están los clics.
/// `recortes` son los que se aplicaron antes de escalar, en orden.
#[allow(clippy::too_many_arguments)]
pub fn pintar(
    imagen: &mut RgbaImage,
    ms: u64,
    clics: &[Clic],
    atajos: &[Atajo],
    origen: (u32, u32),
    recortes: &[Recorte],
    ajustes: &Ajustes,
    pastillas: &mut pastilla::Cache,
) {
    let destino = (imagen.width(), imagen.height());

    if ajustes.clics {
        // Solo los que todavía se ven: el aro dura menos de medio segundo.
        let visibles: Vec<realce::Clic> = clics
            .iter()
            .filter(|c| ms >= c.ms && ms - c.ms < realce::DURACION_MS)
            .filter_map(|c| {
                colocar(c.x, c.y, origen, recortes, destino).map(|(x, y)| realce::Clic {
                    x,
                    y,
                    ms: c.ms,
                    derecho: c.derecho,
                })
            })
            .collect();
        if !visibles.is_empty() {
            // Los puntos ya vienen en coordenadas del fotograma, así que no hay que
            // restarle ninguna región: eso ya lo hizo `colocar`.
            realce::pintar(imagen, &visibles, 0, 0, ms);
        }
    }

    if ajustes.teclas {
        // El último atajo que sigue vivo. Dos a la vez se taparían el uno al otro.
        if let Some(a) = atajos
            .iter()
            .filter(|a| ms >= a.ms && ms - a.ms < teclas_duracion())
            .next_back()
        {
            let opaca = pastilla::opacidad(ms - a.ms, teclas_duracion());
            if opaca > 0.0 {
                if let Some(dibujo) = pastillas.pastilla(&a.texto) {
                    pastilla::pegar(imagen, dibujo, opaca);
                }
            }
        }
    }
}

fn teclas_duracion() -> u64 {
    crate::record::teclas::DURACION_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entera() -> Vec<Recorte> {
        Vec::new()
    }

    fn mitad_derecha() -> Vec<Recorte> {
        vec![Recorte {
            x1: 0.5,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        }]
    }

    #[test]
    fn sin_recortes_ni_escalado_el_punto_se_queda_donde_estaba() {
        let p = colocar(100, 50, (400, 300), &entera(), (400, 300));
        assert_eq!(p, Some((100, 50)));
    }

    #[test]
    fn al_exportar_al_doble_el_punto_va_al_doble() {
        // El fotograma se escala, y el aro tiene que seguir encima de lo que se pulso.
        let p = colocar(100, 50, (400, 300), &entera(), (800, 600));
        assert_eq!(p, Some((200, 100)));
    }

    #[test]
    fn con_un_recorte_el_punto_se_mide_sobre_el_trozo() {
        // El clic en 300 de una captura de 400 es el 100 de la mitad derecha.
        let p = colocar(300, 150, (400, 300), &mitad_derecha(), (200, 300));
        assert_eq!(p, Some((100, 150)));
    }

    #[test]
    fn un_punto_que_se_quedo_fuera_no_se_dibuja() {
        // Un clic que no se ve no tiene aro. Pegarlo al borde lo pondria donde nadie pulso.
        assert_eq!(colocar(20, 150, (400, 300), &mitad_derecha(), (200, 300)), None);
    }

    #[test]
    fn los_recortes_se_encadenan_igual_que_al_recortar_la_imagen() {
        // El del usuario y despues el de la camara del zoom, que se mide sobre el primero.
        let camara = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 0.5,
            y2: 1.0,
        };
        let mut recortes = mitad_derecha();
        recortes.push(camara);
        // El clic en 250 cae en el 50 de la mitad derecha, que es el 50 de su mitad
        // izquierda: justo el centro de un trozo de 100 de ancho.
        let p = colocar(250, 150, (400, 300), &recortes, (100, 300));
        assert_eq!(p, Some((50, 150)));
    }

    #[test]
    fn un_aro_solo_se_dibuja_mientras_dura() {
        let clic = Clic {
            ms: 1000,
            x: 200,
            y: 150,
            derecho: false,
        };
        let ajustes = Ajustes {
            clics: true,
            teclas: false,
        };
        let mut cache = pastilla::Cache::default();
        let mut tocado = |ms: u64| {
            let mut imagen = RgbaImage::from_pixel(400, 300, image::Rgba([0, 0, 0, 255]));
            pintar(
                &mut imagen,
                ms,
                std::slice::from_ref(&clic),
                &[],
                (400, 300),
                &entera(),
                &ajustes,
                &mut cache,
            );
            imagen.pixels().any(|p| p.0 != [0, 0, 0, 255])
        };
        assert!(tocado(1000), "en el momento del clic tendria que verse");
        assert!(
            !tocado(1000 + realce::DURACION_MS + 1),
            "medio segundo despues ya no tendria que quedar nada"
        );
        assert!(!tocado(500), "antes del clic no puede haber aro");
    }

    #[test]
    fn apagado_no_dibuja_nada() {
        let clic = Clic {
            ms: 1000,
            x: 200,
            y: 150,
            derecho: false,
        };
        let mut imagen = RgbaImage::from_pixel(400, 300, image::Rgba([0, 0, 0, 255]));
        let mut cache = pastilla::Cache::default();
        pintar(
            &mut imagen,
            1000,
            std::slice::from_ref(&clic),
            &[],
            (400, 300),
            &entera(),
            &Ajustes::default(),
            &mut cache,
        );
        assert!(imagen.pixels().all(|p| p.0 == [0, 0, 0, 255]));
    }

    #[test]
    fn el_aro_no_se_agranda_con_el_zoom() {
        // Es la razon de dibujarlo aqui y no al grabar: cocido, la camara lo ampliaba y
        // salia gordo y borroso. Dibujado ahora, mide lo mismo con zoom y sin el.
        let clic = Clic {
            ms: 1000,
            x: 200,
            y: 150,
            derecho: false,
        };
        let ajustes = Ajustes {
            clics: true,
            teclas: false,
        };
        let mut cache = pastilla::Cache::default();
        let mut cuantos = |recortes: &[Recorte]| {
            let mut imagen = RgbaImage::from_pixel(400, 300, image::Rgba([0, 0, 0, 255]));
            pintar(
                &mut imagen,
                1000,
                std::slice::from_ref(&clic),
                &[],
                (400, 300),
                recortes,
                &ajustes,
                &mut cache,
            );
            imagen.pixels().filter(|p| p.0 != [0, 0, 0, 255]).count()
        };
        let sin_zoom = cuantos(&entera());
        let con_zoom = cuantos(&[Recorte {
            x1: 0.25,
            y1: 0.25,
            x2: 0.75,
            y2: 0.75,
        }]);
        assert_eq!(
            sin_zoom, con_zoom,
            "el aro cambia de tamanno con la camara: {sin_zoom} vs {con_zoom}"
        );
    }
}
