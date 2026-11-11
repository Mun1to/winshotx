//! El fondo de las capturas sueltas (E1 del plan de CleanShot X).
//!
//! Lo que el Estudio del editor hace con un vídeo (ponerlo sobre un fondo, con aire, sombra
//! y esquinas redondeadas) aplicado a cada foto al copiarla o guardarla, sin pasar por el
//! editor. Es para quien cuelga capturas en redes, en una documentación o en un tuit: con
//! esto puesto, todas salen ya presentables.
//!
//! **Quien pinta es `encode::marco`**, el mismo del Estudio, con los mismos fondos de
//! nombre: una captura y un vídeo con el fondo «Atardecer» tienen que ser el mismo
//! atardecer. Aquí solo se decide cuánto aire y cuánta esquina le tocan a una foto.
//!
//! **Pocas perillas a propósito.** Se elige el fondo y cuánto aire; la sombra y las
//! esquinas van siempre, porque son lo que hace que la captura parezca colocada encima y no
//! pegada. Cada interruptor de más es una fila más en los ajustes, y «cuanto más simple,
//! mejor».

use image::RgbaImage;
use serde::{Deserialize, Serialize};

use super::marco::{self, Fondo, Marco};

/// Lo que se guarda en los ajustes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarcoCaptura {
    /// Cuánto aire alrededor, en tantos por ciento del lado corto de la captura. **Cero es
    /// sin fondo**, que es como viene: nadie se encuentra un marco que no ha pedido.
    pub aire: u32,
    /// El fondo, por su nombre: los mismos del Estudio (`blanco`, `atardecer`...).
    pub fondo: String,
}

impl Default for MarcoCaptura {
    fn default() -> Self {
        Self {
            aire: 0,
            fondo: "atardecer".into(),
        }
    }
}

/// El aire que ofrece la interfaz: poco, normal y mucho. Va aquí para que la prueba de los
/// ajustes y la de la ventana usen los mismos números.
pub const AIRES: [u32; 3] = [4, 8, 14];

impl MarcoCaptura {
    /// El marco que le toca a una captura de ese tamaño, o nada si está apagado.
    ///
    /// **En tanto por ciento y no en píxeles**, a diferencia del Estudio: un vídeo se exporta
    /// a un tamaño que se elige, pero las capturas van de un botón de 200 píxeles a una
    /// pantalla 4K, y 48 píxeles de aire son un océano en la primera y un hilo en la segunda.
    /// Se mide contra el lado corto para que una captura muy apaisada no acabe con más aire
    /// que imagen.
    pub fn para(&self, ancho: u32, alto: u32) -> Option<Marco> {
        if self.aire == 0 || ancho == 0 || alto == 0 {
            return None;
        }
        let corto = ancho.min(alto) as f32;
        let margen = (corto * self.aire.min(40) as f32 / 100.0).round().clamp(8.0, 600.0) as u32;
        // Unas esquinas de un 2 % del lado corto: se notan en una captura grande sin
        // comerse los botones de una pequeña.
        let esquinas = (corto * 0.02).round().clamp(4.0, 32.0) as u32;
        Some(Marco {
            margen,
            fondo: Fondo::desde(&self.fondo),
            sombra: true,
            esquinas,
        })
    }

    /// La captura montada sobre su fondo, o nada si el marco está apagado. Devolver nada en
    /// vez de una copia es lo que hace que el caso de siempre no pague un solo píxel.
    pub fn aplicar(&self, imagen: &RgbaImage) -> Option<RgbaImage> {
        let marco = self.para(imagen.width(), imagen.height())?;
        Some(marco::poner(imagen, marco))
    }

    /// La imagen que se entrega para esa acción de la barra de captura.
    ///
    /// **Solo copiar y guardar llevan el fondo**, que es lo que se lleva el usuario tal
    /// cual. Editar no, porque el editor tiene su propio Estudio y el fondo saldría dos
    /// veces; anclar tampoco, porque la anclada tiene que caer exactamente encima de su
    /// recorte; y leer el texto o un QR miran lo que hay en la pantalla, no el adorno.
    pub fn para_la_accion(&self, imagen: RgbaImage, accion: &str) -> RgbaImage {
        if !matches!(accion, "copy" | "save" | "save_as") {
            return imagen;
        }
        self.aplicar(&imagen).unwrap_or(imagen)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use image::Rgba;

    fn captura(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_pixel(ancho, alto, Rgba([255, 0, 0, 255]))
    }

    #[test]
    fn de_serie_no_pone_nada() {
        let ajuste = MarcoCaptura::default();
        assert!(ajuste.aplicar(&captura(300, 200)).is_none());
    }

    /// Unos ajustes de antes de que existiera esto se leen sin marco.
    #[test]
    fn unos_ajustes_sin_el_campo_lo_traen_apagado() {
        let leido: MarcoCaptura = serde_json::from_str("{}").unwrap();
        assert_eq!(leido.aire, 0);
    }

    /// La captura queda en medio, entera, con aire de su color alrededor.
    #[test]
    fn la_captura_va_en_medio_y_el_aire_lleva_el_fondo() {
        let ajuste = MarcoCaptura {
            aire: 8,
            fondo: "negro".into(),
        };
        let salida = ajuste.aplicar(&captura(400, 200)).unwrap();
        // 8 % de 200 son 16 píxeles por lado.
        assert_eq!(salida.dimensions(), (432, 232));
        assert_eq!(salida.get_pixel(216, 116).0, [255, 0, 0, 255], "el centro es la captura");
        assert_eq!(salida.get_pixel(2, 2).0, [18, 18, 20, 255], "la esquina es el fondo negro");
    }

    /// El aire crece con la captura: no son los mismos píxeles para un botón que para una
    /// pantalla entera.
    #[test]
    fn el_aire_se_mide_contra_el_lado_corto() {
        let ajuste = MarcoCaptura {
            aire: 8,
            fondo: "blanco".into(),
        };
        let pequena = ajuste.para(300, 120).unwrap().margen;
        let grande = ajuste.para(3840, 2160).unwrap().margen;
        assert_eq!(pequena, 10);
        assert_eq!(grande, 173);
        // Y nunca tan poco que no se vea.
        assert_eq!(ajuste.para(40, 40).unwrap().margen, 8);
    }

    /// Lleva sombra y esquinas, que es lo que lo hace parecer colocado y no pegado: abajo
    /// de la captura, sobre fondo blanco, el aire está más oscuro que arriba.
    #[test]
    fn lleva_su_sombra_y_sus_esquinas() {
        let ajuste = MarcoCaptura {
            aire: 14,
            fondo: "blanco".into(),
        };
        let marco = ajuste.para(400, 300).unwrap();
        assert!(marco.sombra);
        assert!(marco.esquinas >= 4);
        let salida = ajuste.aplicar(&captura(400, 300)).unwrap();
        let m = marco.margen;
        let debajo = salida.get_pixel(m + 200, m + 300 + m / 3).0[0];
        let encima = salida.get_pixel(m + 200, m / 3).0[0];
        assert!(debajo < encima, "la sombra va debajo: {debajo} contra {encima}");
        // La esquina de la captura deja ver el fondo: está redondeada.
        assert_ne!(salida.get_pixel(m, m).0, [255, 0, 0, 255]);
    }

    /// Copiar y guardar salen con fondo; editar, anclar y leer, tal cual.
    #[test]
    fn solo_copiar_y_guardar_llevan_el_fondo() {
        let ajuste = MarcoCaptura {
            aire: 8,
            fondo: "menta".into(),
        };
        for accion in ["copy", "save", "save_as"] {
            let salida = ajuste.para_la_accion(captura(200, 100), accion);
            assert_eq!(salida.dimensions(), (216, 116), "«{accion}» tenía que llevar el fondo");
        }
        for accion in ["edit", "pin", "text", "qr"] {
            let salida = ajuste.para_la_accion(captura(200, 100), accion);
            assert_eq!(salida.dimensions(), (200, 100), "«{accion}» no lleva fondo");
        }
    }

    /// Los fondos son los del Estudio: un nombre que no existe sale blanco, no revienta.
    #[test]
    fn un_fondo_raro_sale_blanco() {
        let ajuste = MarcoCaptura {
            aire: 4,
            fondo: "fucsia".into(),
        };
        assert_eq!(ajuste.para(100, 100).unwrap().fondo, Fondo::Blanco);
    }
}
