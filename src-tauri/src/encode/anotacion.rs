//! Las cinco cosas que se pueden dibujar encima de una captura antes de exportarla.
//!
//! Flecha, rectángulo, texto, resaltado y difuminado. **Cinco y ni una más**: el editor de
//! imagen no es el producto, y cada herramienta que se añade es una fila más en una barra
//! que hoy se lee de un vistazo. Lo que hace falta para señalar algo o para tapar un dato
//! está aquí; lo demás es otro programa.
//!
//! **Las coordenadas van de 0 a 1, no en píxeles.** Quien dibuja lo hace sobre una vista
//! previa que casi nunca mide lo que va a medir el archivo, y el mismo dibujo tiene que
//! valer si después se exporta al 50 % o con el doble de ancho. Guardarlas en píxeles
//! obligaría a recalcularlas cada vez que alguien toca «Dimensiones», y bastaría con
//! olvidarse una para que una flecha señalara al sitio equivocado.

use image::{Rgba, RgbaImage};
use serde::Deserialize;

/// Una marca sobre la captura, con sus dos esquinas en tanto por uno.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anotacion {
    /// `arrow`, `box`, `text`, `highlight` o `blur`.
    pub kind: String,
    /// Desde dónde y hasta dónde, de 0 a 1 sobre el ancho y el alto de la imagen.
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    /// El color, en `#rrggbb`. El difuminado y el resaltado lo ignoran.
    #[serde(default)]
    pub color: String,
    /// Lo que dice, si es un texto.
    #[serde(default)]
    pub text: String,
}

impl Anotacion {
    /// Las dos esquinas en píxeles de esta imagen, ya ordenadas y dentro del lienzo.
    fn caja(&self, ancho: u32, alto: u32) -> (i32, i32, i32, i32) {
        let px = |v: f32, tope: u32| ((v * tope as f32).round() as i32).clamp(0, tope as i32 - 1);
        let (a, b) = (px(self.x1, ancho), px(self.x2, ancho));
        let (c, d) = (px(self.y1, alto), px(self.y2, alto));
        (a.min(b), c.min(d), a.max(b), c.max(d))
    }

    fn rgb(&self) -> [u8; 3] {
        color_de(&self.color)
    }
}

/// Lee un `#rrggbb`. Lo que no se entienda sale rojo, que es el color de señalar algo.
fn color_de(texto: &str) -> [u8; 3] {
    let limpio = texto.trim_start_matches('#');
    if limpio.len() != 6 {
        return [239, 68, 68];
    }
    let leer = |i: usize| u8::from_str_radix(&limpio[i..i + 2], 16).ok();
    match (leer(0), leer(2), leer(4)) {
        (Some(r), Some(g), Some(b)) => [r, g, b],
        _ => [239, 68, 68],
    }
}

/// Grosor del trazo, en píxeles, sobre una imagen de este ancho.
///
/// Crece con la imagen: un trazo de 3 px sobre una captura de 3.000 de ancho no se ve, y
/// uno de 12 sobre una de 200 la tapa entera.
///
/// El suelo son tres píxeles y no dos porque con dos **no se ve**. Se probó primero con
/// `ancho / 320` y un mínimo de dos, y sobre una captura de 720 el rectángulo de señalar
/// salía tan fino que había que buscarlo. Una marca que hay que buscar no marca nada.
fn grosor(ancho: u32) -> i32 {
    ((ancho as f32 / 400.0).round() as i32).clamp(3, 9)
}

/// Pinta todas las anotaciones sobre la imagen, en el orden en que se hicieron.
pub fn pintar(imagen: &mut RgbaImage, anotaciones: &[Anotacion]) {
    for anotacion in anotaciones {
        let (ancho, alto) = imagen.dimensions();
        let (x1, y1, x2, y2) = anotacion.caja(ancho, alto);
        match anotacion.kind.as_str() {
            "box" => marco(imagen, x1, y1, x2, y2, anotacion.rgb()),
            "arrow" => flecha(imagen, &anotacion.caja_sin_ordenar(ancho, alto), anotacion.rgb()),
            "highlight" => resaltar(imagen, x1, y1, x2, y2, anotacion.rgb()),
            "blur" => difuminar(imagen, x1, y1, x2, y2),
            "text" => texto(imagen, x1, y1, &anotacion.text, anotacion.rgb()),
            _ => {}
        }
    }
}

impl Anotacion {
    /// Igual que `caja`, pero sin ordenar: la flecha necesita saber de dónde sale y adónde
    /// va, y ordenar las esquinas le daría siempre la vuelta.
    fn caja_sin_ordenar(&self, ancho: u32, alto: u32) -> (i32, i32, i32, i32) {
        let px = |v: f32, tope: u32| ((v * tope as f32).round() as i32).clamp(0, tope as i32 - 1);
        (
            px(self.x1, ancho),
            px(self.y1, alto),
            px(self.x2, ancho),
            px(self.y2, alto),
        )
    }
}

/// Un punto de color, mezclado con lo que hubiera.
fn punto(imagen: &mut RgbaImage, x: i32, y: i32, color: [u8; 3], alfa: f32) {
    let (ancho, alto) = imagen.dimensions();
    if x < 0 || y < 0 || x >= ancho as i32 || y >= alto as i32 || alfa <= 0.0 {
        return;
    }
    let pixel = imagen.get_pixel_mut(x as u32, y as u32);
    for (canal, nuevo) in pixel.0.iter_mut().zip(color).take(3) {
        *canal = (*canal as f32 * (1.0 - alfa) + nuevo as f32 * alfa).round() as u8;
    }
    pixel.0[3] = 255;
}

/// Un disco relleno, que es como se engorda un trazo sin que se vean los escalones.
///
/// El radio va en coma flotante porque los trazos finos son medios píxeles: un trazo de 3
/// tiene radio 1,5. Y el alfa se mide **hasta medio píxel por fuera** del radio, no hasta
/// el radio justo. Con la versión anterior, un disco de radio 1 pintaba UN píxel: a
/// distancia 1 el borde valía exactamente cero y no se dibujaba nada. El rectángulo de
/// señalar salía tan fino que había que buscarlo, y las pruebas seguían verdes porque solo
/// contaban píxeles tocados, no lo gordos que eran.
fn disco(imagen: &mut RgbaImage, cx: i32, cy: i32, radio: f32, color: [u8; 3]) {
    let alcance = radio.ceil() as i32 + 1;
    for dy in -alcance..=alcance {
        for dx in -alcance..=alcance {
            let distancia = ((dx * dx + dy * dy) as f32).sqrt();
            let borde = (radio + 0.5 - distancia).clamp(0.0, 1.0);
            if borde > 0.0 {
                punto(imagen, cx + dx, cy + dy, color, borde);
            }
        }
    }
}

/// Una línea gorda, dibujada como una fila de discos.
fn linea(imagen: &mut RgbaImage, x1: i32, y1: i32, x2: i32, y2: i32, radio: f32, color: [u8; 3]) {
    let pasos = (x2 - x1).abs().max((y2 - y1).abs()).max(1);
    for i in 0..=pasos {
        let t = i as f32 / pasos as f32;
        let x = x1 as f32 + (x2 - x1) as f32 * t;
        let y = y1 as f32 + (y2 - y1) as f32 * t;
        disco(imagen, x.round() as i32, y.round() as i32, radio, color);
    }
}

/// Un rectángulo hueco: se señala lo de dentro sin taparlo.
fn marco(imagen: &mut RgbaImage, x1: i32, y1: i32, x2: i32, y2: i32, color: [u8; 3]) {
    let radio = grosor(imagen.width()) as f32 / 2.0;
    linea(imagen, x1, y1, x2, y1, radio, color);
    linea(imagen, x2, y1, x2, y2, radio, color);
    linea(imagen, x2, y2, x1, y2, radio, color);
    linea(imagen, x1, y2, x1, y1, radio, color);
}

/// Una flecha del primer punto al segundo, con la punta en el segundo.
fn flecha(imagen: &mut RgbaImage, caja: &(i32, i32, i32, i32), color: [u8; 3]) {
    let (x1, y1, x2, y2) = *caja;
    let radio = grosor(imagen.width()) as f32 / 2.0;
    linea(imagen, x1, y1, x2, y2, radio, color);

    // La punta: dos rayas hacia atrás, abiertas treinta grados a cada lado.
    let angulo = ((y2 - y1) as f32).atan2((x2 - x1) as f32);
    let largo = (grosor(imagen.width()) * 4).max(12) as f32;
    for lado in [-0.52f32, 0.52] {
        let a = angulo + std::f32::consts::PI + lado;
        let x = x2 as f32 + a.cos() * largo;
        let y = y2 as f32 + a.sin() * largo;
        linea(imagen, x2, y2, x.round() as i32, y.round() as i32, radio, color);
    }
}

/// Marcador translúcido: se ve lo de debajo, teñido.
fn resaltar(imagen: &mut RgbaImage, x1: i32, y1: i32, x2: i32, y2: i32, color: [u8; 3]) {
    for y in y1..=y2 {
        for x in x1..=x2 {
            punto(imagen, x, y, color, 0.32);
        }
    }
}

/// Cuánto lado tiene cada cuadrado del difuminado, según lo grande que sea la zona.
fn lado_del_mosaico(ancho: i32, alto: i32) -> i32 {
    (ancho.min(alto) / 8).clamp(6, 40)
}

/// Tapa una zona con cuadrados gordos del color medio de lo que había.
///
/// Es un mosaico y no un desenfoque a propósito: un desenfoque suave se puede deshacer con
/// suficiente paciencia y algo de software, y aquí se está tapando un correo, un DNI o una
/// clave. Un mosaico tira la información de verdad: lo que había dentro del cuadrado ya no
/// está en el archivo.
fn difuminar(imagen: &mut RgbaImage, x1: i32, y1: i32, x2: i32, y2: i32) {
    let lado = lado_del_mosaico(x2 - x1 + 1, y2 - y1 + 1);
    let mut y = y1;
    while y <= y2 {
        let mut x = x1;
        while x <= x2 {
            let hasta_x = (x + lado - 1).min(x2);
            let hasta_y = (y + lado - 1).min(y2);
            let (mut r, mut g, mut b, mut cuantos) = (0u32, 0u32, 0u32, 0u32);
            for yy in y..=hasta_y {
                for xx in x..=hasta_x {
                    let p = imagen.get_pixel(xx as u32, yy as u32);
                    r += p.0[0] as u32;
                    g += p.0[1] as u32;
                    b += p.0[2] as u32;
                    cuantos += 1;
                }
            }
            // Clippy pide `checked_div`, pero este `if` protege las TRES divisiones a la
            // vez y ademas envuelve el bucle que las usa. Con `checked_div` serian tres
            // comprobaciones para decir lo mismo.
            #[allow(clippy::manual_checked_ops)]
            if cuantos > 0 {
                let medio = Rgba([
                    (r / cuantos) as u8,
                    (g / cuantos) as u8,
                    (b / cuantos) as u8,
                    255,
                ]);
                for yy in y..=hasta_y {
                    for xx in x..=hasta_x {
                        imagen.put_pixel(xx as u32, yy as u32, medio);
                    }
                }
            }
            x += lado;
        }
        y += lado;
    }
}

/// Texto sobre la captura, dibujado con la fuente del sistema.
#[cfg(windows)]
fn texto(imagen: &mut RgbaImage, x: i32, y: i32, texto: &str, color: [u8; 3]) {
    if texto.is_empty() {
        return;
    }
    // El tamaño de la letra sale del ancho de la imagen, igual que el grosor del trazo: un
    // texto de 22 px sobre una captura de 3.000 no se lee.
    let alto_letra = ((imagen.width() as f32 / 42.0).round() as i32).clamp(14, 72);
    let Some(dibujo) = crate::record::pastilla::escribir(texto, alto_letra, color) else {
        return;
    };
    let (ancho, alto) = imagen.dimensions();
    for (dx, dy, pixel) in dibujo.enumerate_pixels() {
        let alfa = pixel.0[3] as f32 / 255.0;
        if alfa <= 0.0 {
            continue;
        }
        let (px, py) = (x + dx as i32, y + dy as i32);
        if px < 0 || py < 0 || px >= ancho as i32 || py >= alto as i32 {
            continue;
        }
        punto(imagen, px, py, [pixel.0[0], pixel.0[1], pixel.0[2]], alfa);
    }
}

#[cfg(not(windows))]
fn texto(_imagen: &mut RgbaImage, _x: i32, _y: i32, _texto: &str, _color: [u8; 3]) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn lienzo(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_pixel(ancho, alto, Rgba([255, 255, 255, 255]))
    }

    fn marca(kind: &str, x1: f32, y1: f32, x2: f32, y2: f32) -> Anotacion {
        Anotacion {
            kind: kind.into(),
            x1,
            y1,
            x2,
            y2,
            color: "#ef4444".into(),
            text: String::new(),
        }
    }

    fn tocados(imagen: &RgbaImage) -> usize {
        imagen
            .pixels()
            .filter(|p| p.0 != [255, 255, 255, 255])
            .count()
    }

    #[test]
    fn el_color_se_lee_del_texto_y_lo_raro_sale_rojo() {
        assert_eq!(color_de("#0a9bff"), [10, 155, 255]);
        assert_eq!(color_de("0a9bff"), [10, 155, 255]);
        assert_eq!(color_de("azul"), [239, 68, 68]);
        assert_eq!(color_de(""), [239, 68, 68]);
    }

    /// Lo importante de las coordenadas en tanto por uno: el mismo dibujo vale a
    /// cualquier tamaño. Un rectángulo en el centro sigue en el centro al exportar al
    /// doble o a la mitad.
    #[test]
    fn la_misma_marca_cae_en_el_mismo_sitio_a_cualquier_tamanno() {
        let mitad = marca("box", 0.25, 0.25, 0.75, 0.75);
        for (ancho, alto) in [(100u32, 100u32), (400, 400), (1000, 1000)] {
            let (x1, y1, x2, y2) = mitad.caja(ancho, alto);
            assert_eq!(x1, (ancho as f32 * 0.25) as i32);
            assert_eq!(y2, (alto as f32 * 0.75) as i32);
            assert!(x2 > x1 && y2 > y1);
        }
    }

    #[test]
    fn las_esquinas_se_ordenan_solas_si_se_arrastro_hacia_atras() {
        // Dibujado de abajo a la derecha hacia arriba a la izquierda.
        let alreves = marca("box", 0.8, 0.9, 0.2, 0.1);
        let (x1, y1, x2, y2) = alreves.caja(100, 100);
        assert!(x1 < x2 && y1 < y2, "la caja tenía que salir ordenada");
    }

    #[test]
    fn un_rectangulo_pinta_el_borde_y_deja_el_centro_limpio() {
        let mut imagen = lienzo(200, 200);
        pintar(&mut imagen, &[marca("box", 0.2, 0.2, 0.8, 0.8)]);
        assert!(tocados(&imagen) > 0, "no ha pintado el borde");
        assert_eq!(
            *imagen.get_pixel(100, 100),
            Rgba([255, 255, 255, 255]),
            "el centro tiene que quedarse limpio: es un marco, no un relleno"
        );
    }

    #[test]
    fn el_resaltado_tinne_lo_de_dentro_sin_taparlo() {
        let mut imagen = lienzo(100, 100);
        pintar(&mut imagen, &[marca("highlight", 0.2, 0.2, 0.8, 0.8)]);
        let dentro = *imagen.get_pixel(50, 50);
        assert_ne!(dentro, Rgba([255, 255, 255, 255]), "tenía que teñirse");
        assert!(dentro.0[0] > 200, "pero se sigue viendo lo de debajo");
    }

    /// El difuminado es lo único de aquí que tapa datos de verdad, así que su prueba mira
    /// que la información se pierda: dos zonas distintas dentro del mismo cuadrado tienen
    /// que acabar del mismo color.
    #[test]
    fn el_difuminado_borra_de_verdad_lo_que_habia() {
        let mut imagen = lienzo(100, 100);
        // Un texto de mentira: rayas negras finas sobre blanco.
        for y in 40..60 {
            for x in 20..80 {
                if x % 4 < 2 {
                    imagen.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
        pintar(&mut imagen, &[marca("blur", 0.2, 0.4, 0.8, 0.6)]);
        let a = *imagen.get_pixel(21, 45);
        let b = *imagen.get_pixel(23, 45);
        assert_eq!(a, b, "dentro del mosaico ya no se distingue una raya de un hueco");
        assert_ne!(a, Rgba([0, 0, 0, 255]), "y no es negro puro: es la media");
    }

    #[test]
    fn el_mosaico_se_adapta_al_tamanno_de_la_zona() {
        // Una zona pequeña necesita cuadrados pequeños o se convierte en un solo bloque.
        assert!(lado_del_mosaico(40, 20) < lado_del_mosaico(800, 400));
        assert!(lado_del_mosaico(8, 8) >= 6, "nunca por debajo de seis");
        assert!(lado_del_mosaico(5000, 5000) <= 40, "ni por encima de cuarenta");
    }

    #[test]
    fn la_flecha_apunta_a_donde_se_solto_el_raton() {
        let mut imagen = lienzo(200, 200);
        // De arriba a la izquierda hasta abajo a la derecha.
        pintar(&mut imagen, &[marca("arrow", 0.1, 0.1, 0.9, 0.9)]);
        // La punta pinta mucho más que la cola: alrededor del destino hay más color.
        let cerca_del_destino = (160..=195)
            .flat_map(|y| (160..=195).map(move |x| (x, y)))
            .filter(|(x, y)| *imagen.get_pixel(*x, *y) != Rgba([255, 255, 255, 255]))
            .count();
        let cerca_del_origen = (5..=40)
            .flat_map(|y| (5..=40).map(move |x| (x, y)))
            .filter(|(x, y)| *imagen.get_pixel(*x, *y) != Rgba([255, 255, 255, 255]))
            .count();
        assert!(
            cerca_del_destino > cerca_del_origen,
            "la punta va en el destino: {cerca_del_destino} contra {cerca_del_origen}"
        );
    }

    #[test]
    fn una_marca_que_se_sale_del_lienzo_no_revienta() {
        let mut imagen = lienzo(50, 50);
        pintar(
            &mut imagen,
            &[
                marca("box", -0.5, -0.5, 1.5, 1.5),
                marca("arrow", 2.0, 2.0, -2.0, -2.0),
                marca("blur", 0.9, 0.9, 1.4, 1.4),
            ],
        );
        // Que llegue hasta aquí ya es la prueba: ninguna se ha salido del vector.
        assert!(tocados(&imagen) > 0);
    }

    #[test]
    fn una_clase_desconocida_se_ignora_en_vez_de_reventar() {
        let mut imagen = lienzo(50, 50);
        pintar(&mut imagen, &[marca("espiral", 0.1, 0.1, 0.9, 0.9)]);
        assert_eq!(tocados(&imagen), 0);
    }

    /// El trazo tiene que MEDIR lo que dice medir.
    ///
    /// Esta prueba existe porque las otras no vieron el fallo: contaban píxeles tocados y
    /// el rectángulo tocaba muchos, solo que de uno en uno. El disco de radio 1 pintaba un
    /// único píxel, porque el alfa se apagaba justo en el radio en vez de medio píxel más
    /// allá. En la imagen se veía a la primera; en los números, no.
    #[test]
    fn el_trazo_mide_de_verdad_lo_que_dice_el_grosor() {
        let mut imagen = lienzo(720, 420);
        pintar(&mut imagen, &[marca("box", 0.2, 0.2, 0.8, 0.8)]);
        // Se cuenta cuántas filas seguidas están pintadas en el borde de arriba.
        let x = 360;
        let y0 = (420.0 * 0.2) as u32;
        let gordo = (y0.saturating_sub(4)..y0 + 5)
            .filter(|y| *imagen.get_pixel(x, *y) != Rgba([255, 255, 255, 255]))
            .count();
        assert!(
            gordo >= grosor(720) as usize,
            "el trazo dice medir {} y mide {gordo}",
            grosor(720)
        );
    }

    #[test]
    fn el_trazo_engorda_con_la_imagen() {
        assert!(grosor(300) < grosor(1920));
        assert!(grosor(60) >= 3, "nunca más fino de tres píxeles: con dos no se ve");
        assert!(grosor(8000) <= 9, "ni más gordo de nueve");
    }

    /// Deja las cinco herramientas sobre una captura de mentira, para mirarlas.
    /// `cargo test --lib ver_las_anotaciones -- --ignored --nocapture`
    #[test]
    #[ignore = "no comprueba nada: deja un PNG para mirar"]
    fn ver_las_anotaciones() {
        let mut imagen = RgbaImage::new(720, 420);
        // Un escritorio de mentira: barra arriba, panel a la izquierda y texto simulado.
        for (x, y, p) in imagen.enumerate_pixels_mut() {
            *p = if y < 40 {
                Rgba([38, 40, 48, 255])
            } else if x < 180 {
                Rgba([30, 32, 38, 255])
            } else {
                Rgba([246, 246, 248, 255])
            };
        }
        for fila in 0..7 {
            let y0 = 90 + fila * 34;
            for y in y0..y0 + 12 {
                for x in 220..(240 + (fila * 61) % 380) {
                    imagen.put_pixel(x, y, Rgba([60, 64, 72, 255]));
                }
            }
        }
        pintar(
            &mut imagen,
            &[
                Anotacion {
                    kind: "highlight".into(),
                    x1: 0.29,
                    y1: 0.2,
                    x2: 0.62,
                    y2: 0.26,
                    color: "#fbbf24".into(),
                    text: String::new(),
                },
                Anotacion {
                    kind: "box".into(),
                    x1: 0.29,
                    y1: 0.36,
                    x2: 0.72,
                    y2: 0.46,
                    color: "#ef4444".into(),
                    text: String::new(),
                },
                Anotacion {
                    kind: "arrow".into(),
                    x1: 0.9,
                    y1: 0.78,
                    x2: 0.74,
                    y2: 0.45,
                    color: "#0a9bff".into(),
                    text: String::new(),
                },
                Anotacion {
                    kind: "blur".into(),
                    x1: 0.29,
                    y1: 0.62,
                    x2: 0.66,
                    y2: 0.7,
                    color: String::new(),
                    text: String::new(),
                },
                Anotacion {
                    kind: "text".into(),
                    x1: 0.3,
                    y1: 0.85,
                    x2: 0.3,
                    y2: 0.85,
                    color: "#ef4444".into(),
                    text: "esto es lo que falla".into(),
                },
            ],
        );
        let ruta = std::env::temp_dir().join("winshotx-anotaciones.png");
        let (a, b) = imagen.dimensions();
        crate::encode::png::save(&imagen, &ruta, a, b).expect("guardar");
        println!("{}", ruta.display());
    }

    #[test]
    fn sin_anotaciones_la_imagen_se_queda_como_estaba() {
        let mut imagen = lienzo(80, 80);
        pintar(&mut imagen, &[]);
        assert_eq!(tocados(&imagen), 0);
    }
}
