//! El zoom que se acerca solo a donde se hizo clic.
//!
//! Es lo que hace que una grabación de pantalla se entienda: en un monitor de 1920 el botón
//! que se pulsa mide veinte píxeles, y quien mira el vídeo no sabe dónde mirar. Acercarse a
//! ese punto durante un segundo y volver es la diferencia entre un vídeo y un tutorial.
//!
//! **Todo se calcula al exportar, sobre los fotogramas que ya están en disco.** Durante la
//! grabación solo se anota dónde y cuándo se pulsó. De ahí salen tres cosas:
//!
//! 1. **Cero megabytes de instalador y cero milisegundos de arranque.** Es aritmética.
//! 2. **Se puede cambiar de idea después de grabar**: subir el zoom, bajarlo o quitarlo no
//!    obliga a repetir la grabación.
//! 3. No hace falta ningún enganche de teclado ni de ratón.
//!
//! La cámara nunca se sale de la imagen: al pulsar en una esquina se acerca a esa esquina,
//! no a un trozo de negro que no existe.

use serde::{Deserialize, Serialize};

/// Un clic, en píxeles de la región grabada y milisegundos desde el principio.
///
/// Es lo único que se anota mientras se graba, y de aquí sale todo el estudio: el zoom se
/// acerca a estos puntos y los aros se dibujan encima de ellos. Doce bytes y un booleano.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Clic {
    pub ms: u64,
    pub x: i32,
    pub y: i32,
    /// El botón derecho va de otro color: abrir un menú no es pulsar un botón. Al zoom le
    /// da igual cuál fue, pero el aro que se dibuja después sí lo necesita.
    #[serde(default)]
    pub derecho: bool,
}

/// Cómo se comporta el zoom. Lo elige quien exporta, no quien graba.
#[derive(Debug, Clone, Copy)]
pub struct Ajustes {
    /// Cuánto se acerca. 2,0 es el doble de grande.
    pub escala: f32,
    /// Lo que tarda en acercarse y en alejarse.
    pub transicion_ms: u64,
    /// Lo que se queda quieto después del último clic del grupo.
    pub quieto_ms: u64,
    /// Cuánto pasado del ratón se promedia para seguirlo.
    ///
    /// Más alto es más suave y más retrasado; más bajo, más pegado y más nervioso. Medio
    /// segundo deja la cámara acompañando sin dar tirones.
    pub seguir_ms: u64,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            escala: 1.8,
            transicion_ms: 450,
            quieto_ms: 1200,
            seguir_ms: 500,
        }
    }
}

/// Un rato en el que la cámara está acercada a un sitio.
///
/// Sale de agrupar los clics cercanos en el tiempo: acercarse y alejarse por cada clic de un
/// doble clic, o mientras alguien rellena un formulario, marea más que ayuda.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tramo {
    /// Cuándo empieza a acercarse.
    pub desde_ms: u64,
    /// Cuándo termina de alejarse.
    pub hasta_ms: u64,
    /// El primer clic del grupo, que es cuando ya tiene que estar cerca.
    pub cerca_ms: u64,
    /// El último clic del grupo: hasta aquí se queda quieto.
    pub fin_cerca_ms: u64,
    pub x: i32,
    pub y: i32,
}

/// Dónde mira la cámara en un instante.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camara {
    pub x: i32,
    pub y: i32,
    /// 1,0 es la imagen entera.
    pub escala: f32,
}

impl Camara {
    /// La imagen entera, sin acercarse a nada.
    pub fn entera(ancho: u32, alto: u32) -> Self {
        Self {
            x: ancho as i32 / 2,
            y: alto as i32 / 2,
            escala: 1.0,
        }
    }

    /// El trozo que hay que recortar: `(x, y, ancho, alto)`, siempre dentro de la imagen.
    ///
    /// **El centro se corrige antes de recortar, no después.** Un clic pegado al borde
    /// derecho pide una ventana que se sale, y lo correcto es enseñar la esquina de verdad,
    /// no un trozo con la mitad en negro.
    pub fn recorte(&self, ancho: u32, alto: u32) -> (u32, u32, u32, u32) {
        let escala = self.escala.max(1.0);
        let w = ((ancho as f32 / escala).round() as u32).clamp(2, ancho);
        let h = ((alto as f32 / escala).round() as u32).clamp(2, alto);
        let x = (self.x - w as i32 / 2).clamp(0, (ancho - w) as i32) as u32;
        let y = (self.y - h as i32 / 2).clamp(0, (alto - h) as i32) as u32;
        (x, y, w, h)
    }

    /// Lo mismo, pero como recorte de 0 a 1.
    ///
    /// Asi el zoom entra por el camino que ya existe: recortar la imagen, volver a medir
    /// las anotaciones sobre el trozo y escalar. Una flecha dibujada encima entra y sale
    /// de cuadro con la camara, que es lo que tiene que pasar.
    pub fn como_recorte(&self, ancho: u32, alto: u32) -> super::recorte::Recorte {
        let (x, y, w, h) = self.recorte(ancho, alto);
        super::recorte::Recorte {
            x1: x as f32 / ancho as f32,
            y1: y as f32 / alto as f32,
            x2: (x + w) as f32 / ancho as f32,
            y2: (y + h) as f32 / alto as f32,
        }
    }
}

/// Agrupa los clics en tramos de zoom.
///
/// Dos clics entran en el mismo tramo si el segundo llega antes de que el primero haya
/// terminado de estar quieto. El centro es el del PRIMER clic del grupo y no la media: la
/// media de dos esquinas opuestas cae en el medio de la pantalla, que es donde no ha pasado
/// nada.
pub fn tramos(clics: &[Clic], ajustes: &Ajustes) -> Vec<Tramo> {
    let mut salida: Vec<Tramo> = Vec::new();
    for clic in clics {
        match salida.last_mut() {
            // Sigue dentro del anterior: se alarga en vez de abrir otro.
            Some(ultimo) if clic.ms <= ultimo.fin_cerca_ms + ajustes.quieto_ms => {
                ultimo.fin_cerca_ms = clic.ms;
                ultimo.hasta_ms = clic.ms + ajustes.quieto_ms + ajustes.transicion_ms;
            }
            _ => salida.push(Tramo {
                desde_ms: clic.ms.saturating_sub(ajustes.transicion_ms),
                cerca_ms: clic.ms,
                fin_cerca_ms: clic.ms,
                hasta_ms: clic.ms + ajustes.quieto_ms + ajustes.transicion_ms,
                x: clic.x,
                y: clic.y,
            }),
        }
    }
    salida
}

/// Una rampa que empieza y acaba parada, de 0 a 1.
///
/// Con una recta se nota el tirón al empezar y al terminar, y el zoom parece un salto en vez
/// de un movimiento. Es la misma curva que usa cualquier animación que se ve bien.
fn suave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Dónde mira la cámara en ese milisegundo, **siguiendo al ratón**.
///
/// Con la cámara acercada, quedarse clavada en el punto del clic deja al ratón saliéndose
/// del cuadro en cuanto se mueve un poco. Siguiéndolo, la vista acompaña a lo que se está
/// haciendo, que es lo que hace que se entienda.
///
/// **La cámara va por detrás.** El centro no salta a donde está el ratón, se acerca a él un
/// poco en cada fotograma: así un movimiento nervioso no zarandea la imagen, y un
/// movimiento largo se sigue igual. Sin ese retraso, el vídeo marea.
pub fn siguiendo(
    tramos: &[Tramo],
    rastro: &[(u64, i32, i32)],
    ms: u64,
    ancho: u32,
    alto: u32,
    ajustes: &Ajustes,
) -> Camara {
    let base = camara(tramos, ms, ancho, alto, ajustes);
    if base.escala <= 1.001 || rastro.is_empty() {
        return base;
    }
    // El ratón, suavizado: la media de donde ha estado en el último tramo de tiempo, con
    // más peso en lo reciente. Es lo que convierte un temblor en un movimiento.
    let desde = ms.saturating_sub(ajustes.seguir_ms);
    let (mut sx, mut sy, mut peso_total) = (0.0f32, 0.0f32, 0.0f32);
    for (t, x, y) in rastro.iter().filter(|(t, _, _)| *t >= desde && *t <= ms) {
        // De 0 en el punto más antiguo a 1 en el más reciente.
        let cercania = (*t - desde) as f32 / ajustes.seguir_ms.max(1) as f32;
        let peso = 0.15 + cercania * cercania;
        sx += *x as f32 * peso;
        sy += *y as f32 * peso;
        peso_total += peso;
    }
    if peso_total <= 0.0 {
        return base;
    }
    Camara {
        x: (sx / peso_total).round() as i32,
        y: (sy / peso_total).round() as i32,
        escala: base.escala,
    }
}

/// Dónde mira la cámara en ese milisegundo, sin mirar el ratón.
pub fn camara(tramos: &[Tramo], ms: u64, ancho: u32, alto: u32, ajustes: &Ajustes) -> Camara {
    let entera = Camara::entera(ancho, alto);
    let Some(tramo) = tramos
        .iter()
        .find(|t| ms >= t.desde_ms && ms <= t.hasta_ms)
    else {
        return entera;
    };

    let cerca = |cuanto: f32| Camara {
        x: tramo.x,
        y: tramo.y,
        escala: 1.0 + (ajustes.escala - 1.0) * cuanto,
    };

    if ms < tramo.cerca_ms {
        // Acercándose.
        let dentro = (ms - tramo.desde_ms) as f32;
        let total = (tramo.cerca_ms - tramo.desde_ms).max(1) as f32;
        cerca(suave(dentro / total))
    } else if ms <= tramo.fin_cerca_ms + ajustes.quieto_ms {
        // Quieto encima.
        cerca(1.0)
    } else {
        // Alejándose.
        let empieza = tramo.fin_cerca_ms + ajustes.quieto_ms;
        let dentro = (ms - empieza) as f32;
        let total = (tramo.hasta_ms - empieza).max(1) as f32;
        cerca(1.0 - suave(dentro / total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clic(ms: u64, x: i32, y: i32) -> Clic {
        Clic {
            ms,
            x,
            y,
            derecho: false,
        }
    }

    fn ajustes() -> Ajustes {
        Ajustes {
            escala: 2.0,
            transicion_ms: 400,
            quieto_ms: 1000,
            seguir_ms: 500,
        }
    }

    #[test]
    fn sin_clics_la_camara_no_se_mueve() {
        let c = camara(&[], 5000, 1920, 1080, &ajustes());
        assert_eq!(c.escala, 1.0);
        assert_eq!(c.recorte(1920, 1080), (0, 0, 1920, 1080));
    }

    #[test]
    fn un_clic_abre_un_tramo_que_empieza_antes_de_pulsar() {
        // Si el zoom empezara al pulsar, llegaria cerca cuando ya ha pasado lo importante.
        let t = tramos(&[clic(2000, 100, 100)], &ajustes());
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].desde_ms, 1600);
        assert_eq!(t[0].cerca_ms, 2000);
        assert_eq!(t[0].hasta_ms, 3400);
    }

    #[test]
    fn en_el_momento_del_clic_ya_esta_cerca_del_todo() {
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        assert_eq!(camara(&t, 2000, 1920, 1080, &a).escala, 2.0);
    }

    #[test]
    fn antes_del_tramo_y_despues_esta_entera() {
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        assert_eq!(camara(&t, 1000, 1920, 1080, &a).escala, 1.0);
        assert_eq!(camara(&t, 5000, 1920, 1080, &a).escala, 1.0);
    }

    #[test]
    fn a_media_transicion_esta_a_medio_camino() {
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        let mitad = camara(&t, 1800, 1920, 1080, &a).escala;
        assert!((mitad - 1.5).abs() < 0.01, "ha salido {mitad}");
    }

    #[test]
    fn el_movimiento_empieza_y_acaba_parado() {
        // Con una recta se nota el tiron. La curva tiene que moverse MENOS en los extremos
        // que en el centro, y eso es lo que se comprueba: no que valga 0,5 en la mitad,
        // que tambien lo cumple una recta.
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        let en = |ms| camara(&t, ms, 1920, 1080, &a).escala;
        let arranque = en(1640) - en(1600);
        let centro = en(1820) - en(1780);
        assert!(
            centro > arranque * 2.0,
            "el arranque ({arranque}) no es mas suave que el centro ({centro})"
        );
    }

    #[test]
    fn dos_clics_seguidos_son_un_solo_tramo() {
        // Acercarse y alejarse por cada clic de un doble clic marea mas que ayuda.
        let t = tramos(&[clic(2000, 100, 100), clic(2300, 120, 110)], &ajustes());
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].fin_cerca_ms, 2300);
    }

    #[test]
    fn y_se_queda_en_el_sitio_del_primero() {
        // La media de dos esquinas opuestas cae en el centro, que es donde no ha pasado nada.
        let t = tramos(&[clic(2000, 100, 100), clic(2300, 1800, 900)], &ajustes());
        assert_eq!((t[0].x, t[0].y), (100, 100));
    }

    #[test]
    fn dos_clics_lejos_en_el_tiempo_son_dos_tramos() {
        let t = tramos(&[clic(2000, 100, 100), clic(9000, 800, 400)], &ajustes());
        assert_eq!(t.len(), 2);
        assert_eq!((t[1].x, t[1].y), (800, 400));
    }

    #[test]
    fn entre_dos_tramos_se_vuelve_a_ver_la_pantalla_entera() {
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100), clic(9000, 800, 400)], &a);
        assert_eq!(camara(&t, 5500, 1920, 1080, &a).escala, 1.0);
    }

    #[test]
    fn el_recorte_al_doble_es_la_cuarta_parte_de_la_imagen() {
        let c = Camara {
            x: 960,
            y: 540,
            escala: 2.0,
        };
        assert_eq!(c.recorte(1920, 1080), (480, 270, 960, 540));
    }

    #[test]
    fn un_clic_en_la_esquina_no_saca_la_camara_de_la_imagen() {
        // Lo correcto es ensennar la esquina de verdad, no un trozo con la mitad en negro.
        let c = Camara {
            x: 5,
            y: 5,
            escala: 2.0,
        };
        assert_eq!(c.recorte(1920, 1080), (0, 0, 960, 540));
    }

    #[test]
    fn y_en_la_esquina_de_abajo_a_la_derecha_tampoco() {
        let c = Camara {
            x: 1915,
            y: 1075,
            escala: 2.0,
        };
        let (x, y, w, h) = c.recorte(1920, 1080);
        assert_eq!((x + w, y + h), (1920, 1080));
    }

    #[test]
    fn una_escala_por_debajo_de_uno_no_agranda_la_imagen() {
        // Pedir menos de 1 seria pedir un recorte mas grande que la imagen.
        let c = Camara {
            x: 960,
            y: 540,
            escala: 0.5,
        };
        assert_eq!(c.recorte(1920, 1080), (0, 0, 1920, 1080));
    }

    #[test]
    fn el_recorte_de_0_a_1_dice_lo_mismo_que_el_de_pixeles() {
        // El zoom entra por el mismo camino que el recorte del usuario, asi que los dos
        // tienen que decir exactamente lo mismo o la imagen saltaria al pasar de uno a otro.
        let c = Camara {
            x: 960,
            y: 540,
            escala: 2.0,
        };
        let r = c.como_recorte(1920, 1080);
        assert_eq!(r.en_pixeles(1920, 1080), c.recorte(1920, 1080));
    }

    #[test]
    fn sin_zoom_el_recorte_es_la_imagen_entera() {
        let r = Camara::entera(1920, 1080).como_recorte(1920, 1080);
        assert!(!r.recorta_algo(1920, 1080));
    }

    #[test]
    fn el_zoom_nunca_recorta_a_menos_de_dos_pixeles() {
        // H.264 no acepta un lado impar, y un lado de cero revienta al codificador.
        let c = Camara {
            x: 100,
            y: 100,
            escala: 5000.0,
        };
        let (_, _, w, h) = c.recorte(1920, 1080);
        assert!(w >= 2 && h >= 2, "{w}x{h}");
    }

    #[test]
    fn los_clics_de_un_arrastre_largo_no_dejan_la_camara_pegada_para_siempre() {
        // Un tramo se alarga con cada clic, pero termina: sin esto, alguien que hace clic
        // cada segundo durante un minuto tendria un minuto de zoom fijo sin poder salir.
        let a = ajustes();
        let clics: Vec<Clic> = (0..10).map(|i| clic(1000 + i * 500, 200, 200)).collect();
        let t = tramos(&clics, &a);
        assert_eq!(t.len(), 1);
        let ultimo = 1000 + 9 * 500;
        assert_eq!(t[0].hasta_ms, ultimo + a.quieto_ms + a.transicion_ms);
        assert_eq!(camara(&t, t[0].hasta_ms + 1, 1920, 1080, &a).escala, 1.0);
    }

    /// Un rastro de raton: una anotacion cada 33 ms, como al grabar a 30 fps.
    fn rastro(puntos: &[(u64, i32, i32)]) -> Vec<(u64, i32, i32)> {
        puntos.to_vec()
    }

    #[test]
    fn sin_zoom_el_raton_no_mueve_la_camara() {
        // Con la imagen entera no hay a donde seguir a nadie: se ve todo.
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        let r = rastro(&[(5000, 1800, 1000)]);
        assert_eq!(siguiendo(&t, &r, 5000, 1920, 1080, &a).escala, 1.0);
    }

    #[test]
    fn con_zoom_la_camara_se_va_hacia_donde_esta_el_raton() {
        // Quedarse clavada en el clic deja al raton fuera de cuadro en cuanto se mueve.
        let a = ajustes();
        let t = tramos(&[clic(1000, 200, 200)], &a);
        let r = rastro(&[
            (1000, 200, 200),
            (1200, 400, 300),
            (1400, 600, 400),
            (1500, 700, 450),
        ]);
        let c = siguiendo(&t, &r, 1500, 1920, 1080, &a);
        assert!(c.escala > 1.5, "tendria que seguir acercada");
        assert!(c.x > 300, "no ha seguido al raton: x = {}", c.x);
        assert!(c.y > 250, "no ha seguido al raton: y = {}", c.y);
    }

    #[test]
    fn pero_va_por_detras_y_no_pegada() {
        // Si saltara al punto exacto, un movimiento nervioso zarandearia la imagen.
        let a = ajustes();
        let t = tramos(&[clic(1000, 200, 200)], &a);
        let r = rastro(&[
            (1000, 200, 200),
            (1100, 300, 200),
            (1200, 400, 200),
            (1300, 500, 200),
            (1400, 900, 200),
        ]);
        let c = siguiendo(&t, &r, 1400, 1920, 1080, &a);
        assert!(
            c.x < 900,
            "la camara ha saltado encima del raton en vez de ir detras: {}",
            c.x
        );
        assert!(c.x > 300, "se ha quedado demasiado atras: {}", c.x);
    }

    #[test]
    fn un_temblor_no_zarandea_la_imagen() {
        // El raton va y viene alrededor de un punto: la camara tiene que quedarse quieta.
        let a = ajustes();
        let t = tramos(&[clic(1000, 500, 400)], &a);
        let mut puntos = Vec::new();
        for i in 0..20u64 {
            let vaiven = if i % 2 == 0 { 12 } else { -12 };
            puntos.push((1000 + i * 25, 500 + vaiven, 400 - vaiven));
        }
        let r = rastro(&puntos);
        let uno = siguiendo(&t, &r, 1400, 1920, 1080, &a);
        let otro = siguiendo(&t, &r, 1425, 1920, 1080, &a);
        assert!(
            (uno.x - otro.x).abs() <= 4,
            "la camara tiembla con el raton: {} y {}",
            uno.x,
            otro.x
        );
    }

    #[test]
    fn sin_rastro_se_queda_en_el_punto_del_clic() {
        // Las grabaciones de antes de esta version no tienen rastro anotado, y tienen que
        // seguir exportandose: se acercan al clic y ya, como hacian.
        let a = ajustes();
        let t = tramos(&[clic(1000, 200, 200)], &a);
        let c = siguiendo(&t, &[], 1000, 1920, 1080, &a);
        assert_eq!((c.x, c.y), (200, 200));
    }

    #[test]
    fn el_rastro_de_otro_momento_no_cuenta() {
        // Solo se promedia el pasado reciente. Con todo el rastro, la camara se iria a la
        // media de la grabacion entera, que es el centro de la pantalla.
        let a = ajustes();
        let t = tramos(&[clic(9000, 1500, 800)], &a);
        let mut puntos = vec![(100, 50, 50), (200, 60, 60), (300, 70, 70)];
        puntos.push((9000, 1500, 800));
        let c = siguiendo(&t, &rastro(&puntos), 9000, 1920, 1080, &a);
        assert!(c.x > 1200, "se ha ido al pasado antiguo: {}", c.x);
    }
}
