//! Quedarse con un trozo de la captura antes de exportarla.
//!
//! El editor ya recortaba el TIEMPO con los marcadores A y B. Esto es la otra mitad:
//! recortar el ESPACIO. En una foto se puede volver a capturar y no pasa nada, pero una
//! grabación de tres minutos con el encuadre torcido no se repite: o se recorta, o se tira.
//!
//! **Las coordenadas van de 0 a 1**, como las anotaciones y por lo mismo: quien arrastra
//! el marco lo hace sobre una vista previa que casi nunca mide lo que mide el archivo.
//!
//! El recorte va **antes de escalar**, que es el primer paso de todo lo demás: quien pide
//! un recorte de 400 px de ancho y luego exporta a 800 quiere ese trozo al doble, no la
//! captura entera. Recortar después de escalar daría el mismo trozo pero a otro tamaño.

use image::RgbaImage;
use serde::Deserialize;

use super::anotacion::Anotacion;

/// El trozo que se queda, con sus dos esquinas en tanto por uno.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recorte {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Recorte {
    /// Las dos esquinas ordenadas y metidas dentro de [0, 1].
    fn ordenado(&self) -> (f32, f32, f32, f32) {
        (
            self.x1.min(self.x2).clamp(0.0, 1.0),
            self.y1.min(self.y2).clamp(0.0, 1.0),
            self.x1.max(self.x2).clamp(0.0, 1.0),
            self.y1.max(self.y2).clamp(0.0, 1.0),
        )
    }

    /// Si de verdad recorta algo. Un marco de cero píxeles, o uno que abarca la imagen
    /// entera, no es un recorte: es trabajo para no cambiar nada.
    pub fn recorta_algo(&self, ancho: u32, alto: u32) -> bool {
        let (x, y, w, h) = self.en_pixeles(ancho, alto);
        (x, y, w, h) != (0, 0, ancho, alto)
    }

    /// Dónde y cuánto, en píxeles de esta imagen: `(x, y, ancho, alto)`.
    ///
    /// Nunca devuelve un lado de cero ni un rectángulo que se sale: un arrastre de un
    /// píxel no puede dejar una imagen vacía que reviente al codificador.
    pub fn en_pixeles(&self, ancho: u32, alto: u32) -> (u32, u32, u32, u32) {
        let (x1, y1, x2, y2) = self.ordenado();
        let x = (x1 * ancho as f32).round() as u32;
        let y = (y1 * alto as f32).round() as u32;
        let x = x.min(ancho.saturating_sub(1));
        let y = y.min(alto.saturating_sub(1));
        let w = ((x2 * ancho as f32).round() as u32).saturating_sub(x).max(1);
        let h = ((y2 * alto as f32).round() as u32).saturating_sub(y).max(1);
        (x, y, w.min(ancho - x), h.min(alto - y))
    }

    /// El trozo, como imagen suya.
    pub fn aplicar(&self, imagen: &RgbaImage) -> RgbaImage {
        let (x, y, w, h) = self.en_pixeles(imagen.width(), imagen.height());
        image::imageops::crop_imm(imagen, x, y, w, h).to_image()
    }

    /// La misma marca, pero medida sobre el trozo en vez de sobre la captura entera.
    ///
    /// Sin esto, una flecha en el centro de la captura seguiría diciendo «0,5» y acabaría
    /// en el centro del RECORTE, que es otro sitio. Lo que caiga fuera se sale de [0, 1] y
    /// lo recorta quien pinta, que ya sabe hacerlo.
    pub fn reencuadrar(&self, marca: &Anotacion) -> Anotacion {
        let (x1, y1, x2, y2) = self.ordenado();
        let (ancho, alto) = ((x2 - x1).max(f32::EPSILON), (y2 - y1).max(f32::EPSILON));
        Anotacion {
            x1: (marca.x1 - x1) / ancho,
            y1: (marca.y1 - y1) / alto,
            x2: (marca.x2 - x1) / ancho,
            y2: (marca.y2 - y1) / alto,
            kind: marca.kind.clone(),
            color: marca.color.clone(),
            text: marca.text.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imagen(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_fn(ancho, alto, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 0, 255])
        })
    }

    fn marca(x1: f32, y1: f32, x2: f32, y2: f32) -> Anotacion {
        Anotacion {
            kind: "box".into(),
            x1,
            y1,
            x2,
            y2,
            color: "#ef4444".into(),
            text: String::new(),
        }
    }

    #[test]
    fn la_mitad_derecha_es_la_mitad_derecha() {
        let r = Recorte {
            x1: 0.5,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        assert_eq!(r.en_pixeles(400, 300), (200, 0, 200, 300));
    }

    #[test]
    fn el_trozo_lleva_los_pixeles_que_tenia_ahi_la_captura() {
        // No basta con que mida lo pedido: tiene que ser ESE trozo y no otro.
        let fuente = imagen(400, 300);
        let r = Recorte {
            x1: 0.25,
            y1: 0.5,
            x2: 0.75,
            y2: 1.0,
        };
        let trozo = r.aplicar(&fuente);
        assert_eq!(trozo.dimensions(), (200, 150));
        assert_eq!(trozo.get_pixel(0, 0), fuente.get_pixel(100, 150));
        assert_eq!(trozo.get_pixel(199, 149), fuente.get_pixel(299, 299));
    }

    #[test]
    fn las_esquinas_al_reves_dan_el_mismo_trozo() {
        // Arrastrar de derecha a izquierda es tan normal como al reves.
        let derecho = Recorte {
            x1: 0.2,
            y1: 0.2,
            x2: 0.8,
            y2: 0.8,
        };
        let del_reves = Recorte {
            x1: 0.8,
            y1: 0.8,
            x2: 0.2,
            y2: 0.2,
        };
        assert_eq!(derecho.en_pixeles(500, 500), del_reves.en_pixeles(500, 500));
    }

    #[test]
    fn un_arrastre_de_nada_no_deja_una_imagen_vacia() {
        // Un lado de cero revienta al codificador de video mucho mas tarde, y para
        // entonces ya no se sabe de donde venia.
        let r = Recorte {
            x1: 0.5,
            y1: 0.5,
            x2: 0.5,
            y2: 0.5,
        };
        let (_, _, w, h) = r.en_pixeles(400, 300);
        assert!(w >= 1 && h >= 1, "ha salido un lado de cero: {w}x{h}");
        assert_eq!(r.aplicar(&imagen(400, 300)).dimensions(), (1, 1));
    }

    #[test]
    fn uno_que_se_sale_por_los_bordes_se_queda_dentro() {
        let r = Recorte {
            x1: -0.5,
            y1: -0.5,
            x2: 1.5,
            y2: 1.5,
        };
        assert_eq!(r.en_pixeles(400, 300), (0, 0, 400, 300));
    }

    #[test]
    fn el_marco_que_abarca_la_captura_entera_no_recorta_nada() {
        // Se usa para no hacer trabajo de mas: recortar la imagen entera es copiarla.
        let entero = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        assert!(!entero.recorta_algo(400, 300));
        let medio = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 0.5,
            y2: 1.0,
        };
        assert!(medio.recorta_algo(400, 300));
    }

    #[test]
    fn una_marca_en_el_centro_sigue_en_el_centro_del_trozo() {
        // El caso que lo explica todo: recortar la mitad derecha deja el centro de la
        // captura pegado al borde izquierdo del trozo, no en su centro.
        let r = Recorte {
            x1: 0.5,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let reencuadrada = r.reencuadrar(&marca(0.5, 0.5, 0.75, 0.6));
        assert!((reencuadrada.x1 - 0.0).abs() < 1e-5);
        assert!((reencuadrada.x2 - 0.5).abs() < 1e-5);
        // El alto no se toca, porque el recorte no toca el alto.
        assert!((reencuadrada.y1 - 0.5).abs() < 1e-5);
    }

    #[test]
    fn una_marca_que_cae_fuera_del_trozo_sale_del_rango_en_vez_de_pegarse_al_borde() {
        // Pegarla al borde la dejaria pintada en un sitio donde nadie la puso. Que se
        // salga de [0, 1] es lo correcto: quien pinta ya recorta al lienzo.
        let r = Recorte {
            x1: 0.5,
            y1: 0.5,
            x2: 1.0,
            y2: 1.0,
        };
        let fuera = r.reencuadrar(&marca(0.1, 0.1, 0.2, 0.2));
        assert!(fuera.x2 < 0.0, "tendria que quedar a la izquierda del trozo");
    }

    #[test]
    fn reencuadrar_conserva_lo_que_no_son_coordenadas() {
        let r = Recorte {
            x1: 0.1,
            y1: 0.1,
            x2: 0.9,
            y2: 0.9,
        };
        let mut original = marca(0.2, 0.2, 0.3, 0.3);
        original.kind = "text".into();
        original.text = "mira esto".into();
        let salida = r.reencuadrar(&original);
        assert_eq!(salida.kind, "text");
        assert_eq!(salida.text, "mira esto");
        assert_eq!(salida.color, "#ef4444");
    }

    #[test]
    fn sin_recorte_de_verdad_la_marca_se_queda_donde_estaba() {
        let entero = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let salida = entero.reencuadrar(&marca(0.3, 0.4, 0.6, 0.7));
        assert!((salida.x1 - 0.3).abs() < 1e-5);
        assert!((salida.y2 - 0.7).abs() < 1e-5);
    }
}
