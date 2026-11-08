//! Poner la captura sobre un fondo, con aire alrededor y una sombra debajo.
//!
//! Es lo que separa un pantallazo de algo que se puede colgar en una publicación sin que
//! parezca un recorte pegado: **cuánto aire, de qué fondo, si lleva sombra, y cuánto se
//! redondean las esquinas.** Las esquinas entraron el 8 de noviembre de 2026, cuando Munir
//! pidió el estudio «parecido a los competidores»: Screen Studio, Cap y CleanShot las
//! traen todos, y sin ellas el vídeo enmarcado parecía un recorte pegado sobre un fondo.
//! Ángulos, reflejos y perspectivas siguen fuera: eso ya es un editor de imagen.
//!
//! Vive aparte del exportador y sin tocar disco para poder comprobarlo con números: cada
//! prueba mira píxeles concretos de una imagen pequeña.

use image::{Rgba, RgbaImage};

/// De qué color es el aire de alrededor.
///
/// Son unos pocos fondos elegidos, no un selector de color: con la rueda entera, la
/// decisión pasa de «cuál de estos cinco» a «cuál de dieciséis millones», que es
/// exactamente el tipo de pregunta que hace que alguien cierre el panel sin exportar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fondo {
    Blanco,
    Negro,
    Gris,
    /// Azul a violeta, en diagonal.
    Atardecer,
    /// Verde a azul, en diagonal.
    Menta,
    /// Pizarra casi negra, para que el color lo pongan la captura y no el fondo.
    Noche,
    /// Celeste a azul marino.
    Oceano,
    /// Rosa a ámbar, el cálido.
    Coral,
}

impl Fondo {
    /// Lee el nombre que manda el panel del editor. Lo que no reconoce sale blanco, que es
    /// el fondo más neutro: un nombre nuevo no debe reventar una exportación.
    pub fn desde(nombre: &str) -> Self {
        match nombre {
            "negro" => Fondo::Negro,
            "gris" => Fondo::Gris,
            "atardecer" => Fondo::Atardecer,
            "menta" => Fondo::Menta,
            "noche" => Fondo::Noche,
            "oceano" => Fondo::Oceano,
            "coral" => Fondo::Coral,
            _ => Fondo::Blanco,
        }
    }

    /// El color de un punto del lienzo. `t` va de 0 en la esquina de arriba a la izquierda
    /// a 1 en la de abajo a la derecha, que es por donde corre el degradado.
    fn color(self, t: f32) -> Rgba<u8> {
        let mezcla = |a: [u8; 3], b: [u8; 3]| {
            Rgba([
                (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
                (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
                (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
                255,
            ])
        };
        match self {
            Fondo::Blanco => Rgba([255, 255, 255, 255]),
            Fondo::Negro => Rgba([18, 18, 20, 255]),
            Fondo::Gris => Rgba([232, 232, 234, 255]),
            Fondo::Atardecer => mezcla([88, 116, 245], [168, 85, 217]),
            Fondo::Menta => mezcla([34, 197, 152], [56, 132, 232]),
            Fondo::Noche => mezcla([30, 41, 59], [15, 23, 42]),
            Fondo::Oceano => mezcla([14, 165, 233], [30, 64, 175]),
            Fondo::Coral => mezcla([251, 113, 133], [245, 158, 11]),
        }
    }
}

/// Lo que se le pide al marco.
#[derive(Debug, Clone, Copy)]
pub struct Marco {
    /// Píxeles de aire por cada lado. Cero significa que no hay marco y no se toca nada.
    pub margen: u32,
    pub fondo: Fondo,
    pub sombra: bool,
    /// El radio de las esquinas de la captura, en píxeles. Solo se nota con aire alrededor:
    /// sin fondo detrás no hay nada que asome por la esquina.
    pub esquinas: u32,
}

impl Marco {
    /// El tamaño que tendrá la imagen con el marco puesto.
    ///
    /// **Los dos lados salen pares.** H.264 no acepta un ancho o un alto impar, y una
    /// exportación que muere en el codificador después de recorrer trescientos fotogramas
    /// es la peor forma de enterarse.
    pub fn medida(&self, ancho: u32, alto: u32) -> (u32, u32) {
        if self.margen == 0 {
            return (ancho, alto);
        }
        let par = |n: u32| n + (n % 2);
        (par(ancho + self.margen * 2), par(alto + self.margen * 2))
    }
}

/// Cuánto se desplaza la sombra hacia abajo, en tantos por ciento del margen.
const SOMBRA_CAIDA: f32 = 0.22;
/// Y cuánto se difumina, también en tantos por ciento del margen.
const SOMBRA_BORROSA: f32 = 0.45;

/// Devuelve la captura montada sobre su fondo.
///
/// Sin margen devuelve la imagen tal cual, sin copiarla dos veces: el caso de siempre no
/// debe pagar por una función que casi nadie enciende.
pub fn poner(imagen: &RgbaImage, marco: Marco) -> RgbaImage {
    if marco.margen == 0 {
        return imagen.clone();
    }
    let (ancho_f, alto_f) = imagen.dimensions();
    // El fondo con su sombra es el mismo en todos los fotogramas de una exportación: se
    // pinta una vez y cada fotograma lo copia. Pintarlo píxel a píxel en cada uno (un
    // degradado y una sombra sobre tres millones de píxeles a 1080p) era lo que más costaba
    // de exportar un vídeo con marco.
    let mut lienzo = (*base(marco, ancho_f, alto_f)).clone();
    let ancho = lienzo.width() as usize;

    // La captura va centrada. Si el redondeo a par ha dejado un píxel de más, se lo queda
    // el lado derecho y el de abajo: repartir medio píxel no existe.
    let izquierda = marco.margen as usize;
    let arriba = marco.margen as usize;

    // Nunca más que media captura: con más, las dos esquinas de un lado se pisarían.
    let radio = marco.esquinas.min(ancho_f.min(alto_f) / 2) as f32;
    let banda = radio.ceil() as u32;
    // Una captura de pantalla es opaca entera, y entonces una fila que no toca ninguna
    // esquina se copia de golpe: `sobre` dejaría el píxel de la captura tal cual.
    let opaca = imagen.as_raw().chunks_exact(4).all(|p| p[3] == 255);
    let origen = imagen.as_raw();
    let fila = ancho_f as usize * 4;
    let destino: &mut [u8] = &mut lienzo;
    for y in 0..alto_f {
        let de = &origen[y as usize * fila..(y as usize + 1) * fila];
        let inicio = ((y as usize + arriba) * ancho + izquierda) * 4;
        let en_una_esquina = radio > 0.0 && (y < banda || y >= alto_f.saturating_sub(banda));
        if opaca && !en_una_esquina {
            destino[inicio..inicio + fila].copy_from_slice(de);
            continue;
        }
        for x in 0..ancho_f {
            let i = x as usize * 4;
            let j = inicio + i;
            let arriba_px = Rgba([de[i], de[i + 1], de[i + 2], de[i + 3]]);
            let debajo = Rgba([destino[j], destino[j + 1], destino[j + 2], destino[j + 3]]);
            let encima = sobre(arriba_px, debajo);
            // Lo que cae fuera del arco de una esquina deja ver el fondo, con el borde
            // suavizado: un píxel cubierto a medias se mezcla a medias, o sale dentado.
            let cubre = cobertura(x, y, ancho_f, alto_f, radio);
            let sale = if cubre >= 1.0 { encima } else { entre(debajo, encima, cubre) };
            destino[j..j + 4].copy_from_slice(&sale.0);
        }
    }
    lienzo
}

/// Lo que identifica un fondo ya pintado: el tamaño de la captura y todo lo del marco.
type Clave = (u32, u32, u32, Fondo, bool, u32);

/// El fondo con su sombra para una captura de ese tamaño, pintado la primera vez que se
/// pide y guardado para los siguientes fotogramas. Uno solo: se exporta un vídeo cada vez.
fn base(marco: Marco, ancho_f: u32, alto_f: u32) -> std::sync::Arc<RgbaImage> {
    static GUARDADO: std::sync::Mutex<Option<(Clave, std::sync::Arc<RgbaImage>)>> =
        std::sync::Mutex::new(None);
    let clave = (ancho_f, alto_f, marco.margen, marco.fondo, marco.sombra, marco.esquinas);
    if let Ok(guardado) = GUARDADO.lock() {
        if let Some((k, lienzo)) = guardado.as_ref() {
            if *k == clave {
                return lienzo.clone();
            }
        }
    }
    let lienzo = std::sync::Arc::new(pintar_base(marco, ancho_f, alto_f));
    if let Ok(mut guardado) = GUARDADO.lock() {
        *guardado = Some((clave, lienzo.clone()));
    }
    lienzo
}

/// El lienzo entero con el degradado del fondo y la sombra de la captura, sin la captura.
fn pintar_base(marco: Marco, ancho_f: u32, alto_f: u32) -> RgbaImage {
    let (ancho, alto) = marco.medida(ancho_f, alto_f);
    let mut lienzo = RgbaImage::new(ancho, alto);
    // El degradado corre en diagonal, así que el punto de cada píxel es lo lejos que está
    // de la esquina de arriba a la izquierda medido sobre esa diagonal.
    let diagonal = (ancho + alto).max(1) as f32;
    for (x, y, pixel) in lienzo.enumerate_pixels_mut() {
        *pixel = marco.fondo.color((x + y) as f32 / diagonal);
    }
    if marco.sombra {
        let radio = marco.esquinas.min(ancho_f.min(alto_f) / 2) as f32;
        pintar_sombra(&mut lienzo, marco.margen, marco.margen, ancho_f, alto_f, marco.margen, radio);
    }
    lienzo
}

/// Cuánto de un píxel de la captura queda dentro de sus esquinas redondeadas: 1 en todo lo
/// que no es esquina, 0 fuera del arco, y lo de en medio en el borde.
fn cobertura(x: u32, y: u32, ancho: u32, alto: u32, radio: f32) -> f32 {
    if radio <= 0.0 {
        return 1.0;
    }
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let (w, h) = (ancho as f32, alto as f32);
    let cx = if px < radio {
        radio
    } else if px > w - radio {
        w - radio
    } else {
        return 1.0;
    };
    let cy = if py < radio {
        radio
    } else if py > h - radio {
        h - radio
    } else {
        return 1.0;
    };
    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    (radio - d + 0.5).clamp(0.0, 1.0)
}

/// De `a` hacia `b` en `t` (de 0 a 1), canal a canal.
fn entre(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    let c = |i: usize| (a.0[i] as f32 + (b.0[i] as f32 - a.0[i] as f32) * t).round() as u8;
    Rgba([c(0), c(1), c(2), 255])
}

/// Una mancha oscura debajo de la captura, desplazada y con los bordes difuminados.
///
/// No es un desenfoque de verdad: es una rampa lineal en los bordes del rectángulo. A
/// simple vista se distingue poco de un desenfoque gaussiano cuando la mancha está detrás
/// de la imagen, y cuesta una fracción de lo que cuesta el gaussiano en cada uno de los
/// trescientos fotogramas de un vídeo.
fn pintar_sombra(
    lienzo: &mut RgbaImage,
    izquierda: u32,
    arriba: u32,
    ancho_f: u32,
    alto_f: u32,
    margen: u32,
    radio: f32,
) {
    let caida = (margen as f32 * SOMBRA_CAIDA).round() as i32;
    let borrosa = (margen as f32 * SOMBRA_BORROSA).max(1.0);
    let (ancho, alto) = lienzo.dimensions();

    let x0 = izquierda as i32;
    let y0 = arriba as i32 + caida;
    let x1 = x0 + ancho_f as i32;
    let y1 = y0 + alto_f as i32;

    for y in 0..alto as i32 {
        for x in 0..ancho as i32 {
            // Lo lejos que está el píxel del rectángulo de la sombra, por fuera. Con las
            // esquinas redondeadas la sombra también lo es: se mide al rectángulo encogido
            // el radio, y se le resta el radio.
            let fuera_x = ((x0 as f32 + radio) - x as f32).max(x as f32 - (x1 - 1) as f32 + radio).max(0.0);
            let fuera_y = ((y0 as f32 + radio) - y as f32).max(y as f32 - (y1 - 1) as f32 + radio).max(0.0);
            let distancia = ((fuera_x * fuera_x + fuera_y * fuera_y).sqrt() - radio).max(0.0);
            if distancia >= borrosa {
                continue;
            }
            let fuerza = (1.0 - distancia / borrosa) * 0.42;
            let pixel = lienzo.get_pixel_mut(x as u32, y as u32);
            *pixel = sobre(Rgba([0, 0, 0, (fuerza * 255.0) as u8]), *pixel);
        }
    }
}

/// Pone un color encima de otro, respetando la transparencia del de arriba.
fn sobre(arriba: Rgba<u8>, abajo: Rgba<u8>) -> Rgba<u8> {
    let a = arriba.0[3] as u32;
    if a == 255 {
        return arriba;
    }
    if a == 0 {
        return abajo;
    }
    let mezcla = |i: usize| ((arriba.0[i] as u32 * a + abajo.0[i] as u32 * (255 - a)) / 255) as u8;
    Rgba([mezcla(0), mezcla(1), mezcla(2), 255])
}

/// Escala la captura al tamanno pedido y le pone el marco encima.
///
/// Va en este orden y no al reves: primero se lleva la captura al tamanno que eligio el
/// usuario en «Dimensiones», y despues se le anade el aire alrededor. Enmarcar antes y
/// escalar despues encogeria tambien el margen, y un margen de 40 px acabaria siendo de 12
/// en cuanto alguien exportara al 30 %.
///
/// El codificador recibe la imagen ya del tamanno final, asi que no la vuelve a escalar:
/// eso lo comprueba su propio `if`, y por eso enmarcar no cuesta un reescalado de mas.
pub fn enmarcar(imagen: RgbaImage, ancho: u32, alto: u32, marco: Marco) -> RgbaImage {
    let escalada = if imagen.dimensions() == (ancho, alto) {
        imagen
    } else {
        super::escalar::a_medida(&imagen, ancho, alto)
    };
    poner(&escalada, marco)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cuadro(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_pixel(ancho, alto, Rgba([255, 0, 0, 255]))
    }

    #[test]
    fn sin_margen_la_imagen_sale_igual_que_entro() {
        let dentro = cuadro(40, 30);
        let fuera = poner(
            &dentro,
            Marco {
                margen: 0,
                fondo: Fondo::Atardecer,
                sombra: true,
                esquinas: 0,
            },
        );
        assert_eq!(fuera.dimensions(), (40, 30));
        assert_eq!(fuera.as_raw(), dentro.as_raw());
    }

    #[test]
    fn el_margen_crece_por_los_cuatro_lados() {
        let fuera = poner(
            &cuadro(40, 30),
            Marco {
                margen: 10,
                fondo: Fondo::Blanco,
                sombra: false,
                esquinas: 0,
            },
        );
        assert_eq!(fuera.dimensions(), (60, 50));
    }

    #[test]
    fn la_captura_queda_centrada_y_entera() {
        let marco = Marco {
            margen: 8,
            fondo: Fondo::Blanco,
            sombra: false,
            esquinas: 0,
        };
        let fuera = poner(&cuadro(20, 20), marco);
        // Las cuatro esquinas de la captura, y ni un píxel más allá.
        assert_eq!(*fuera.get_pixel(8, 8), Rgba([255, 0, 0, 255]));
        assert_eq!(*fuera.get_pixel(27, 27), Rgba([255, 0, 0, 255]));
        assert_eq!(*fuera.get_pixel(7, 8), Rgba([255, 255, 255, 255]));
        assert_eq!(*fuera.get_pixel(28, 27), Rgba([255, 255, 255, 255]));
    }

    /// H.264 no acepta lados impares, y enterarse en el codificador después de trescientos
    /// fotogramas es la peor forma de enterarse.
    #[test]
    fn los_dos_lados_salen_siempre_pares() {
        for (ancho, alto) in [(41, 31), (40, 31), (41, 30), (40, 30)] {
            let marco = Marco {
                margen: 7,
                fondo: Fondo::Negro,
                sombra: false,
                esquinas: 0,
            };
            let (a, b) = marco.medida(ancho, alto);
            assert_eq!(a % 2, 0, "el ancho {a} es impar (venía de {ancho})");
            assert_eq!(b % 2, 0, "el alto {b} es impar (venía de {alto})");
            assert_eq!(poner(&cuadro(ancho, alto), marco).dimensions(), (a, b));
        }
    }

    #[test]
    fn el_fondo_liso_es_el_mismo_color_en_las_cuatro_esquinas() {
        let fuera = poner(
            &cuadro(10, 10),
            Marco {
                margen: 6,
                fondo: Fondo::Negro,
                sombra: false,
                esquinas: 0,
            },
        );
        let (ancho, alto) = fuera.dimensions();
        let esquina = *fuera.get_pixel(0, 0);
        assert_eq!(*fuera.get_pixel(ancho - 1, 0), esquina);
        assert_eq!(*fuera.get_pixel(0, alto - 1), esquina);
        assert_eq!(*fuera.get_pixel(ancho - 1, alto - 1), esquina);
    }

    #[test]
    fn el_degradado_cambia_de_una_esquina_a_la_otra() {
        let fuera = poner(
            &cuadro(10, 10),
            Marco {
                margen: 20,
                fondo: Fondo::Atardecer,
                sombra: false,
                esquinas: 0,
            },
        );
        let (ancho, alto) = fuera.dimensions();
        let arriba = *fuera.get_pixel(0, 0);
        let abajo = *fuera.get_pixel(ancho - 1, alto - 1);
        assert_ne!(arriba, abajo, "el degradado ha salido plano");
    }

    #[test]
    fn la_sombra_oscurece_debajo_de_la_captura_y_no_encima() {
        let marco = Marco {
            margen: 20,
            fondo: Fondo::Blanco,
            sombra: true,
            esquinas: 0,
        };
        let fuera = poner(&cuadro(40, 40), marco);
        let claro = |p: Rgba<u8>| p.0[0] as u32 + p.0[1] as u32 + p.0[2] as u32;
        // Justo debajo del borde de abajo hay sombra; a la misma distancia por arriba, no.
        let debajo = claro(*fuera.get_pixel(30, 62));
        let encima = claro(*fuera.get_pixel(30, 2));
        assert!(
            debajo < encima,
            "debajo tendría que estar más oscuro: {debajo} contra {encima}"
        );
    }

    #[test]
    fn sin_sombra_el_fondo_se_queda_limpio() {
        let fuera = poner(
            &cuadro(40, 40),
            Marco {
                margen: 20,
                fondo: Fondo::Blanco,
                sombra: false,
                esquinas: 0,
            },
        );
        assert_eq!(*fuera.get_pixel(30, 62), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn una_captura_con_transparencia_deja_ver_el_fondo_por_debajo() {
        // Pasa de verdad: la captura de todas las pantallas deja transparentes los huecos
        // entre monitores desalineados.
        let mut medio = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 0]));
        medio.put_pixel(10, 10, Rgba([255, 0, 0, 255]));
        let fuera = poner(
            &medio,
            Marco {
                margen: 4,
                fondo: Fondo::Negro,
                sombra: false,
                esquinas: 0,
            },
        );
        assert_eq!(*fuera.get_pixel(14, 14), Rgba([255, 0, 0, 255]));
        // Y donde la captura era transparente se ve el fondo, no un agujero negro.
        assert_eq!(*fuera.get_pixel(6, 6), Fondo::Negro.color(0.0));
    }

    /// El orden importa: escalar y luego enmarcar deja el margen del tamanno que se pidio.
    /// Al reves, exportar al 25 % dejaria un margen de 10 px donde se pidieron 40.
    #[test]
    fn el_margen_no_encoge_al_exportar_mas_pequenno() {
        let marco = Marco {
            margen: 40,
            fondo: Fondo::Blanco,
            sombra: false,
            esquinas: 0,
        };
        // Una captura de 400 x 400 exportada a 100 x 100, con 40 px de aire.
        let fuera = enmarcar(cuadro(400, 400), 100, 100, marco);
        assert_eq!(fuera.dimensions(), (180, 180));
        // El borde de la captura sigue estando a 40 px del borde del lienzo.
        assert_eq!(*fuera.get_pixel(39, 90), Rgba([255, 255, 255, 255]));
        assert_eq!(*fuera.get_pixel(40, 90), Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn enmarcar_sin_margen_solo_escala() {
        let fuera = enmarcar(
            cuadro(400, 400),
            100,
            100,
            Marco {
                margen: 0,
                fondo: Fondo::Blanco,
                sombra: false,
                esquinas: 0,
            },
        );
        assert_eq!(fuera.dimensions(), (100, 100));
    }

    #[test]
    fn un_nombre_de_fondo_desconocido_no_revienta_la_exportacion() {
        assert_eq!(Fondo::desde("lo que sea"), Fondo::Blanco);
        assert_eq!(Fondo::desde("menta"), Fondo::Menta);
    }

    /// Saca un muestrario de los cinco fondos para mirarlo con los ojos.
    /// `cargo test --lib ver_los_fondos -- --ignored --nocapture`
    #[test]
    #[ignore = "no comprueba nada: deja PNG para mirar"]
    fn ver_los_fondos() {
        let mut captura = RgbaImage::from_pixel(240, 150, Rgba([28, 30, 36, 255]));
        // Unas rayas claras, para que se vea donde acaba la captura y empieza el fondo.
        for y in 20..130 {
            for x in 20..220 {
                if (x + y) % 24 < 12 {
                    captura.put_pixel(x, y, Rgba([120, 170, 240, 255]));
                }
            }
        }
        let carpeta = std::env::temp_dir().join("winshotx-muestrario");
        std::fs::create_dir_all(&carpeta).expect("la carpeta");
        for (nombre, fondo) in [
            ("blanco", Fondo::Blanco),
            ("negro", Fondo::Negro),
            ("gris", Fondo::Gris),
            ("atardecer", Fondo::Atardecer),
            ("menta", Fondo::Menta),
        ] {
            let marco = Marco {
                margen: 44,
                fondo,
                sombra: true,
                esquinas: 0,
            };
            let fuera = poner(&captura, marco);
            let ruta = carpeta.join(format!("{nombre}.png"));
            let (a, b) = fuera.dimensions();
            crate::encode::png::save(&fuera, &ruta, a, b).expect("guardar");
            println!("{}", ruta.display());
        }
    }

    /// El camino entero hasta el archivo que se lleva el usuario.
    ///
    /// Que la funcion devuelva la imagen buena no basta: el PNG lo escribe `png::save`, que
    /// tiene su propio reescalado, y si se le pasara el tamanno de antes de enmarcar
    /// volveria a encoger la imagen y el marco desapareceria en el archivo aunque las once
    /// pruebas de arriba siguieran verdes. Es exactamente lo que paso con el audio: dos
    /// extremos en verde y el archivo del usuario sin sonido.
    #[test]
    fn el_png_guardado_en_disco_sale_con_el_marco_puesto() {
        let marco = Marco {
            margen: 12,
            fondo: Fondo::Negro,
            sombra: false,
            esquinas: 0,
        };
        let enmarcada = enmarcar(cuadro(60, 40), 60, 40, marco);
        let (ancho, alto) = marco.medida(60, 40);

        let ruta = std::env::temp_dir().join(format!(
            "winshotx-marco-{}.png",
            std::process::id()
        ));
        crate::encode::png::save(&enmarcada, &ruta, ancho, alto).expect("guardar el PNG");

        let releida = image::open(&ruta).expect("releer el PNG").to_rgba8();
        let _ = std::fs::remove_file(&ruta);

        assert_eq!(releida.dimensions(), (84, 64), "el marco no llego al archivo");
        // La esquina es fondo, y el centro sigue siendo la captura.
        assert_eq!(releida.get_pixel(0, 0).0[0], 18);
        assert_eq!(*releida.get_pixel(42, 32), Rgba([255, 0, 0, 255]));
    }
}

/// Las esquinas redondeadas y los fondos nuevos del estudio.
#[cfg(test)]
mod esquinas {
    use super::*;

    const ROJO: Rgba<u8> = Rgba([220, 30, 30, 255]);

    fn redondeada(esquinas: u32, sombra: bool) -> RgbaImage {
        poner(
            &RgbaImage::from_pixel(40, 30, ROJO),
            Marco { margen: 10, fondo: Fondo::Blanco, sombra, esquinas },
        )
    }

    #[test]
    fn la_esquina_deja_ver_el_fondo_y_el_resto_de_la_captura_sigue_entero() {
        let lienzo = redondeada(8, false);
        assert_eq!(*lienzo.get_pixel(10, 10), Rgba([255, 255, 255, 255]), "la esquina tiene que ser fondo");
        assert_eq!(*lienzo.get_pixel(30, 25), ROJO, "el centro es la captura");
        assert_eq!(*lienzo.get_pixel(19, 10), ROJO, "el borde de arriba, fuera de la esquina, tambien");
    }

    #[test]
    fn el_borde_del_arco_sale_suavizado_y_no_dentado() {
        let lienzo = redondeada(8, false);
        // Algun pixel del arco queda a medias entre el rojo y el blanco.
        let a_medias = (10..18).flat_map(|x| (10..18).map(move |y| (x, y))).any(|(x, y)| {
            let p = lienzo.get_pixel(x, y);
            p.0[1] > 40 && p.0[1] < 230
        });
        assert!(a_medias, "el arco sale dentado");
    }

    #[test]
    fn sin_esquinas_la_esquina_es_captura_como_siempre() {
        assert_eq!(*redondeada(0, false).get_pixel(10, 10), ROJO);
    }

    #[test]
    fn la_sombra_sigue_la_esquina_redondeada() {
        // Justo fuera de la esquina de abajo a la derecha, en diagonal: con el rectangulo
        // recto ahi habria sombra fuerte; con la esquina redondeada, mas suave.
        let recta = redondeada(0, true);
        let curva = redondeada(12, true);
        let (x, y) = (50, 42);
        assert!(curva.get_pixel(x, y).0[0] > recta.get_pixel(x, y).0[0], "la sombra no sigue la curva");
    }

    #[test]
    fn los_fondos_nuevos_tienen_nombre() {
        assert_eq!(Fondo::desde("noche"), Fondo::Noche);
        assert_eq!(Fondo::desde("oceano"), Fondo::Oceano);
        assert_eq!(Fondo::desde("coral"), Fondo::Coral);
        assert_eq!(Fondo::desde("no-existe"), Fondo::Blanco);
    }
}
