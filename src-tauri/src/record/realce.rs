//! Marcar los clics sobre la grabación, para que se vea dónde se está pulsando.
//!
//! Es lo que separa una grabación de un tutorial que se entiende: sin esto, el puntero se
//! mueve y de pronto pasan cosas, y quien mira no sabe si hubo un clic, un doble clic o
//! una tecla. Un aro que aparece donde se pulsó y se va apagando lo cuenta sin palabras.
//!
//! **El aro se dibuja en los fotogramas guardados, no en la pantalla.** Pintarlo encima
//! del escritorio querría decir una ventana más, transparente, que se cuela en cualquier
//! otra captura y que hay que mover a la velocidad del ratón. Aquí se pinta solo donde
//! importa: dentro del vídeo.
//!
//! Cómo se sabe que hay un clic: preguntando por el estado del botón en cada fotograma, no
//! con un enganche global del ratón. Un enganche mal hecho **le cuelga el escritorio a
//! quien lo tenga puesto**, y a treinta fotogramas por segundo se pregunta cada 33 ms, que
//! es menos de lo que dura un clic humano.

use image::{Rgba, RgbaImage};

/// Cuánto dura el aro desde que se pulsa, en milisegundos.
pub const DURACION_MS: u64 = 420;

/// Un clic que todavía se está viendo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clic {
    /// Dónde se pulsó, en coordenadas del escritorio virtual.
    pub x: i32,
    pub y: i32,
    /// Cuándo, contando desde que empezó la grabación.
    pub ms: u64,
    /// El botón derecho se pinta de otro color: no es lo mismo abrir un menú que pulsar.
    pub derecho: bool,
}

/// El radio del aro en un momento dado. Crece al principio y se queda.
///
/// Crece porque un aro que aparece ya del tamaño final se lee como un adorno; uno que se
/// abre desde el punto donde se pulsó se lee como «aquí ha pasado algo».
fn radio(avance: f32) -> f32 {
    let crecida = (avance * 3.0).min(1.0);
    6.0 + 16.0 * crecida
}

/// Lo transparente que está el aro. Entero al principio, apagándose al final.
fn opacidad(avance: f32) -> f32 {
    if avance < 0.35 {
        0.85
    } else {
        0.85 * (1.0 - (avance - 0.35) / 0.65)
    }
}

/// Pinta sobre el fotograma los clics que todavía se ven en ese instante.
///
/// La región es la parte del escritorio que se está grabando: los clics vienen en
/// coordenadas del escritorio virtual y hay que restarle su origen, que **puede ser
/// negativo** si se graba en un monitor a la izquierda del principal.
pub fn pintar(
    imagen: &mut RgbaImage,
    clics: &[Clic],
    region_x: i32,
    region_y: i32,
    ahora_ms: u64,
) {
    for clic in clics {
        let Some(edad) = ahora_ms.checked_sub(clic.ms) else {
            continue;
        };
        if edad >= DURACION_MS {
            continue;
        }
        let avance = edad as f32 / DURACION_MS as f32;
        aro(
            imagen,
            clic.x - region_x,
            clic.y - region_y,
            radio(avance),
            opacidad(avance),
            clic.derecho,
        );
    }
}

/// Grosor del trazo del aro, en píxeles.
const GROSOR: f32 = 3.0;

/// Un aro de color, con el borde suavizado para que no salgan los escalones.
fn aro(imagen: &mut RgbaImage, cx: i32, cy: i32, radio: f32, opacidad: f32, derecho: bool) {
    if opacidad <= 0.0 {
        return;
    }
    let (ancho, alto) = imagen.dimensions();
    let borde = radio + GROSOR;
    // Solo se recorre el cuadrado que ocupa el aro, no la imagen entera: a 30 fotogramas
    // por segundo, barrer dos millones de píxeles por cada clic no sale a cuenta.
    let x0 = (cx as f32 - borde).floor().max(0.0) as u32;
    let y0 = (cy as f32 - borde).floor().max(0.0) as u32;
    let x1 = ((cx as f32 + borde).ceil() as i64).clamp(0, ancho as i64) as u32;
    let y1 = ((cy as f32 + borde).ceil() as i64).clamp(0, alto as i64) as u32;

    let color = if derecho {
        [255u8, 176, 32]
    } else {
        [10u8, 155, 255]
    };

    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 - cx as f32;
            let dy = y as f32 - cy as f32;
            let distancia = (dx * dx + dy * dy).sqrt();
            // Lo cerca que está del trazo, de 0 (fuera) a 1 (justo encima).
            let cerca = 1.0 - ((distancia - radio).abs() / (GROSOR / 2.0)).min(1.0);
            if cerca <= 0.0 {
                continue;
            }
            let alfa = cerca * opacidad;
            let pixel = imagen.get_pixel_mut(x, y);
            for (canal, nuevo) in pixel.0.iter_mut().zip(color).take(3) {
                *canal = (*canal as f32 * (1.0 - alfa) + nuevo as f32 * alfa).round() as u8;
            }
        }
    }
    let _ = Rgba([0u8, 0, 0, 0]);
}

/// Se queda solo con los clics que aún se pueden ver, para que la lista no crezca sin fin.
///
/// Una grabación de diez minutos son miles de clics, y recorrerlos todos en cada fotograma
/// para descartar los de hace nueve minutos es trabajo tirado treinta veces por segundo.
pub fn olvidar_viejos(clics: &mut Vec<Clic>, ahora_ms: u64) {
    clics.retain(|c| ahora_ms.saturating_sub(c.ms) < DURACION_MS);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lienzo(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_pixel(ancho, alto, Rgba([0, 0, 0, 255]))
    }

    /// Cuántos píxeles ha tocado el aro.
    fn pintados(imagen: &RgbaImage) -> usize {
        imagen
            .pixels()
            .filter(|p| p.0[0] != 0 || p.0[1] != 0 || p.0[2] != 0)
            .count()
    }

    #[test]
    fn un_clic_recien_hecho_deja_marca() {
        let mut imagen = lienzo(120, 120);
        let clic = Clic {
            x: 60,
            y: 60,
            ms: 100,
            derecho: false,
        };
        pintar(&mut imagen, &[clic], 0, 0, 110);
        assert!(pintados(&imagen) > 0, "no ha pintado nada");
    }

    #[test]
    fn un_clic_ya_pasado_no_se_pinta() {
        let mut imagen = lienzo(120, 120);
        let clic = Clic {
            x: 60,
            y: 60,
            ms: 0,
            derecho: false,
        };
        pintar(&mut imagen, &[clic], 0, 0, DURACION_MS + 1);
        assert_eq!(pintados(&imagen), 0);
    }

    #[test]
    fn el_aro_se_va_apagando() {
        let temprano = {
            let mut imagen = lienzo(120, 120);
            pintar(
                &mut imagen,
                &[Clic {
                    x: 60,
                    y: 60,
                    ms: 0,
                    derecho: false,
                }],
                0,
                0,
                20,
            );
            imagen.pixels().map(|p| p.0[2] as u64).sum::<u64>()
        };
        let tarde = {
            let mut imagen = lienzo(120, 120);
            pintar(
                &mut imagen,
                &[Clic {
                    x: 60,
                    y: 60,
                    ms: 0,
                    derecho: false,
                }],
                0,
                0,
                DURACION_MS - 20,
            );
            imagen.pixels().map(|p| p.0[2] as u64).sum::<u64>()
        };
        assert!(tarde < temprano, "al final tendría que estar más apagado");
    }

    /// La trampa de siempre: grabando en el monitor de la izquierda, la región empieza en
    /// una coordenada negativa. Restarla es sumar, y un `max(0)` pondría el aro en la
    /// esquina en vez de donde se pulsó.
    #[test]
    fn en_un_monitor_de_coordenadas_negativas_el_aro_cae_donde_toca() {
        let mut imagen = lienzo(200, 200);
        // Se graba una región que empieza en (-1000, 100) del escritorio virtual, y se
        // pulsa en (-900, 200): eso son 100, 100 dentro de la grabación.
        let clic = Clic {
            x: -900,
            y: 200,
            ms: 0,
            derecho: false,
        };
        pintar(&mut imagen, &[clic], -1000, 100, 10);
        // El centro del aro está hueco (es un aro), así que se mira un punto del trazo.
        let radio_ahora = radio(10.0 / DURACION_MS as f32).round() as u32;
        let tocado = imagen.get_pixel(100 + radio_ahora, 100);
        assert!(tocado.0[2] > 0, "el aro no ha caído en 100,100");
    }

    #[test]
    fn un_clic_fuera_de_la_region_no_revienta_ni_pinta_en_el_borde() {
        let mut imagen = lienzo(100, 100);
        // Muy a la izquierda de lo que se graba: ni un píxel del aro entra.
        let clic = Clic {
            x: -500,
            y: 50,
            ms: 0,
            derecho: false,
        };
        pintar(&mut imagen, &[clic], 0, 0, 10);
        assert_eq!(pintados(&imagen), 0);
    }

    #[test]
    fn un_clic_a_medio_salir_por_el_borde_pinta_solo_lo_que_entra() {
        let mut imagen = lienzo(100, 100);
        let clic = Clic {
            x: 2,
            y: 50,
            ms: 0,
            derecho: false,
        };
        pintar(&mut imagen, &[clic], 0, 0, 10);
        assert!(pintados(&imagen) > 0, "algo tenía que entrar");
    }

    #[test]
    fn el_boton_derecho_se_pinta_de_otro_color() {
        let mut izquierdo = lienzo(120, 120);
        let mut derecho = lienzo(120, 120);
        let base = Clic {
            x: 60,
            y: 60,
            ms: 0,
            derecho: false,
        };
        pintar(&mut izquierdo, &[base], 0, 0, 10);
        pintar(
            &mut derecho,
            &[Clic {
                derecho: true,
                ..base
            }],
            0,
            0,
            10,
        );
        let azul = |i: &RgbaImage| i.pixels().map(|p| p.0[2] as u64).sum::<u64>();
        let rojo = |i: &RgbaImage| i.pixels().map(|p| p.0[0] as u64).sum::<u64>();
        assert!(azul(&izquierdo) > azul(&derecho), "el izquierdo tira a azul");
        assert!(rojo(&derecho) > rojo(&izquierdo), "y el derecho a naranja");
    }

    /// Saca una tira con el aro en cinco momentos, para mirarlo con los ojos.
    /// `cargo test --lib ver_el_aro -- --ignored --nocapture`
    #[test]
    #[ignore = "no comprueba nada: deja un PNG para mirar"]
    fn ver_el_aro() {
        let paso = 150u32;
        let mut tira = RgbaImage::from_pixel(paso * 5, paso, Rgba([46, 48, 56, 255]));
        // Un fondo con rejilla, para ver que el aro deja pasar lo de debajo.
        for (x, y, p) in tira.enumerate_pixels_mut() {
            if x % 25 == 0 || y % 25 == 0 {
                *p = Rgba([64, 66, 76, 255]);
            }
        }
        for (i, edad) in [0u64, 60, 150, 280, 400].iter().enumerate() {
            let centro = paso as i32 * i as i32 + paso as i32 / 2;
            pintar(
                &mut tira,
                &[Clic {
                    x: centro,
                    y: paso as i32 / 2,
                    ms: 0,
                    derecho: i == 4,
                }],
                0,
                0,
                *edad,
            );
        }
        let ruta = std::env::temp_dir().join("winshotx-aro.png");
        let (a, b) = tira.dimensions();
        crate::encode::png::save(&tira, &ruta, a, b).expect("guardar");
        println!("{}", ruta.display());
    }

    #[test]
    fn los_clics_viejos_se_tiran_de_la_lista() {
        let mut clics = vec![
            Clic {
                x: 0,
                y: 0,
                ms: 0,
                derecho: false,
            },
            Clic {
                x: 0,
                y: 0,
                ms: 5_000,
                derecho: false,
            },
        ];
        olvidar_viejos(&mut clics, 5_010);
        assert_eq!(clics.len(), 1, "solo se queda el reciente");
        assert_eq!(clics[0].ms, 5_000);
    }

    /// Un fotograma con marca de tiempo anterior al clic (puede pasar con el reloj de la
    /// captura) no debe restar por debajo de cero y dar un número gigante.
    #[test]
    fn un_clic_del_futuro_no_da_la_vuelta_al_contador() {
        let mut imagen = lienzo(120, 120);
        let clic = Clic {
            x: 60,
            y: 60,
            ms: 900,
            derecho: false,
        };
        pintar(&mut imagen, &[clic], 0, 0, 100);
        assert_eq!(pintados(&imagen), 0);
    }
}
