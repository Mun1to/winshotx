//! S25 (marco): la captura dentro de una ventana de Windows 11 o de un portátil.
//!
//! Es lo que hace que un pantallazo de un trozo de pantalla parezca sacado de una ventana de
//! verdad, o de un ordenador encima de una mesa, sin abrir un editor de imagen. **Todo se
//! dibuja aquí, a mano y con cuentas**: la barra de título con sus tres botones, el bisel del
//! portátil, su cámara y su base. Nada se copia del arte de nadie.
//!
//! **La captura no se encoge.** La carcasa crece por fuera, como el aire del fondo: quien pide
//! 1920 x 1080 tiene su captura a 1920 x 1080 dentro de la ventana, píxel a píxel, y el archivo
//! mide eso más la barra, el bisel y el aire.
//!
//! Lo que mide cada pieza sale de `geometria`, que tiene su espejo en `src/lib/carcasa.ts` con
//! las MISMAS cuentas: la vista previa del editor dibuja la carcasa en SVG con esas medidas, y
//! las pruebas de los dos lados comprueban los mismos números. Si cambia una, cambia la otra.
//!
//! Solo se pone con fondo (con aire alrededor): una ventana con esquinas redondeadas y sombra
//! necesita algo detrás. Lo decide `Marco::carcasa_puesta`.

use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// Lo que envuelve la captura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Carcasa {
    /// Una ventana de Windows 11: barra de título con minimizar, maximizar y cerrar, esquinas
    /// redondeadas y su borde fino. Clara u oscura como las ventanas del sistema: lo decide
    /// quien pide (el editor, con el tema de winshotx; las capturas, con `platform::tema`).
    Ventana {
        #[serde(default)]
        oscura: bool,
    },
    /// Un portátil abierto visto de frente: la pantalla con su bisel y su cámara, y la base.
    Portatil,
}

/// Lo que se elige y se guarda (en los ajustes de las capturas y en un estilo): sin el tema,
/// que se pone al pintar con `con_tema`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CarcasaElegida {
    Ventana,
    Portatil,
}

impl CarcasaElegida {
    pub fn con_tema(self, oscura: bool) -> Carcasa {
        match self {
            CarcasaElegida::Ventana => Carcasa::Ventana { oscura },
            CarcasaElegida::Portatil => Carcasa::Portatil,
        }
    }

    /// La de un nombre guardado: `ventana` o `portatil`. Cualquier otra cosa, ninguna.
    pub fn de_nombre(nombre: &str) -> Option<Self> {
        match nombre {
            "ventana" => Some(CarcasaElegida::Ventana),
            "portatil" => Some(CarcasaElegida::Portatil),
            _ => None,
        }
    }
}

/// Lee la carcasa de unos ajustes sin romperlos: una que esta versión no conoce (la guardó
/// una más nueva) cuenta como ninguna, en vez de tirar todos los ajustes por un campo.
pub fn leer_elegida<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<CarcasaElegida>, D::Error> {
    let valor = Option::<serde_json::Value>::deserialize(d)?;
    Ok(valor.as_ref().and_then(|v| v.as_str()).and_then(CarcasaElegida::de_nombre))
}

/// Un rectángulo con cada esquina redondeada a su radio, en coordenadas de la pieza.
///
/// Los radios van en el orden de las agujas del reloj desde arriba a la izquierda: arriba a la
/// izquierda, arriba a la derecha, abajo a la derecha y abajo a la izquierda.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Forma {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    pub radios: [f32; 4],
}

impl Forma {
    pub fn recta(x: f32, y: f32, ancho: f32, alto: f32, radio: f32) -> Self {
        Self { x, y, ancho, alto, radios: [radio; 4] }
    }

    /// Lo lejos que está el punto del borde: negativo dentro, positivo fuera. Es la distancia
    /// de siempre a un rectángulo redondeado, con el radio de la esquina que le toca a ese
    /// cuarto del rectángulo.
    pub fn distancia(&self, px: f32, py: f32) -> f32 {
        let (mx, my) = (self.ancho / 2.0, self.alto / 2.0);
        let (dx, dy) = (px - (self.x + mx), py - (self.y + my));
        let r = match (dx > 0.0, dy > 0.0) {
            (false, false) => self.radios[0],
            (true, false) => self.radios[1],
            (true, true) => self.radios[2],
            (false, true) => self.radios[3],
        }
        .min(mx)
        .min(my)
        .max(0.0);
        let qx = dx.abs() - mx + r;
        let qy = dy.abs() - my + r;
        qx.max(qy).min(0.0) + (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() - r
    }

    /// Cuánto de un píxel (por su centro) cae dentro, con el borde suavizado: 1 dentro, 0
    /// fuera y lo de en medio en el borde.
    pub fn cubre(&self, x: u32, y: u32) -> f32 {
        (0.5 - self.distancia(x as f32 + 0.5, y as f32 + 0.5)).clamp(0.0, 1.0)
    }
}

/// Lo que mide una carcasa alrededor de una captura de ese tamaño, en píxeles del archivo.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometria {
    /// La pieza entera: la carcasa con la captura dentro, sin el aire del fondo.
    pub ancho: u32,
    pub alto: u32,
    /// Dónde va la esquina de arriba a la izquierda de la captura dentro de la pieza.
    pub x: u32,
    pub y: u32,
    /// Los radios de las esquinas de la captura, en el orden de `Forma::radios`.
    pub radios: [f32; 4],
    /// Lo que echa sombra sobre el fondo: el contorno de la pieza, en una o más formas.
    pub sombra: Vec<Forma>,
}

/// El lado largo con el que se mide una ventana a «100 %». Una captura de pantalla completa a
/// 1920 lleva la barra de 32 px de Windows; una de 3840 (una 4K al 200 %), la de 64. Una más
/// pequeña que eso es un trozo de pantalla, y lleva la barra de 100 %, como la ventana real.
const LADO_DE_REFERENCIA: f32 = 1920.0;

/// Las medidas de Windows 11 a 100 %: la barra de título, cada botón, el dibujo de dentro, su
/// trazo y el radio de las esquinas de la ventana.
const BARRA: f32 = 32.0;
const BOTON: f32 = 46.0;
const GLIFO: f32 = 10.0;
const TRAZO: f32 = 1.0;
const RADIO_VENTANA: f32 = 8.0;

/// La escala de la ventana para una captura de ese tamaño: 1 hasta 1920 de lado largo, y de
/// ahí crece con la captura, como crecería con el escalado de Windows.
pub fn escala(ancho: u32, alto: u32) -> f32 {
    (ancho.max(alto) as f32 / LADO_DE_REFERENCIA).max(1.0)
}

/// La unidad del portátil: un 1 % del lado largo de la pantalla, y nunca menos que el de una
/// de 640. Por debajo, el bisel de una captura pequeña sería un hilo y no se leería portátil.
pub fn unidad_del_portatil(ancho: u32, alto: u32) -> f32 {
    ancho.max(alto).max(640) as f32 / 100.0
}

/// Las medidas del portátil, en unidades (ver `unidad_del_portatil`).
const BISEL_LADO: f32 = 2.4;
const BISEL_ARRIBA: f32 = 3.0;
const BISEL_ABAJO: f32 = 3.0;
const RADIO_TAPA: f32 = 2.0;
const CANTO: f32 = 0.22;
const VUELO: f32 = 4.5;
const ALTO_BASE: f32 = 1.5;
const MUESCA: f32 = 14.0;
const CAMARA: f32 = 0.32;

/// El borde fino de la ventana, en píxeles enteros para que no salga borroso.
fn borde(s: f32) -> u32 {
    (s.round() as u32).max(1)
}

/// Un número de unidades en píxeles enteros, nunca menos de uno.
fn px(unidades: f32, u: f32) -> u32 {
    ((unidades * u).round() as u32).max(1)
}

/// Lo que mide la carcasa alrededor de una captura de `ancho` x `alto`.
pub fn geometria(carcasa: Carcasa, ancho: u32, alto: u32) -> Geometria {
    match carcasa {
        Carcasa::Ventana { .. } => {
            let s = escala(ancho, alto);
            let b = borde(s);
            let barra = (BARRA * s).round() as u32;
            let radio = (RADIO_VENTANA * s).round();
            let (pieza_ancho, pieza_alto) = (ancho + 2 * b, alto + barra + 2 * b);
            // La captura lleva redondeadas las dos esquinas de abajo, a lo que deja el borde:
            // las de arriba quedan debajo de la barra.
            let dentro = (radio - b as f32).max(0.0);
            Geometria {
                ancho: pieza_ancho,
                alto: pieza_alto,
                x: b,
                y: b + barra,
                radios: [0.0, 0.0, dentro, dentro],
                sombra: vec![Forma::recta(0.0, 0.0, pieza_ancho as f32, pieza_alto as f32, radio)],
            }
        }
        Carcasa::Portatil => {
            let u = unidad_del_portatil(ancho, alto);
            let (lado, arriba, abajo) = (px(BISEL_LADO, u), px(BISEL_ARRIBA, u), px(BISEL_ABAJO, u));
            let (tapa_ancho, tapa_alto) = (ancho + 2 * lado, alto + arriba + abajo);
            let vuelo = px(VUELO, u);
            let alto_base = px(ALTO_BASE, u);
            let (pieza_ancho, pieza_alto) = (tapa_ancho + 2 * vuelo, tapa_alto + alto_base);
            let base = forma_de_la_base(pieza_ancho, tapa_alto, alto_base);
            Geometria {
                ancho: pieza_ancho,
                alto: pieza_alto,
                x: vuelo + lado,
                y: arriba,
                radios: [0.0; 4],
                sombra: vec![forma_de_la_tapa(vuelo, tapa_ancho, tapa_alto, u), base],
            }
        }
    }
}

/// La tapa del portátil: la pantalla con su bisel, redondeada arriba y casi recta abajo, donde
/// apoya en la base.
fn forma_de_la_tapa(vuelo: u32, ancho: u32, alto: u32, u: f32) -> Forma {
    let r = (RADIO_TAPA * u).round();
    Forma { x: vuelo as f32, y: 0.0, ancho: ancho as f32, alto: alto as f32, radios: [r, r, r * 0.25, r * 0.25] }
}

/// La base: una losa más ancha que la tapa, recta arriba y con el canto de delante redondeado.
fn forma_de_la_base(ancho: u32, y: u32, alto: u32) -> Forma {
    let r = (alto as f32 * 0.7).round();
    Forma { x: 0.0, y: y as f32, ancho: ancho as f32, alto: alto as f32, radios: [0.0, 0.0, r, r] }
}

/// Los colores de la ventana: la barra, sus dibujos y el borde, en claro y en oscuro. Son los
/// de Windows 11 sin Mica: la barra lisa que se ve con la transparencia apagada.
pub fn colores_de_la_ventana(oscura: bool) -> ([u8; 3], [u8; 3], [u8; 3]) {
    if oscura {
        ([32, 32, 32], [255, 255, 255], [58, 58, 58])
    } else {
        ([243, 243, 243], [26, 26, 26], [204, 204, 204])
    }
}

/// Los colores del portátil: el canto de la tapa, el bisel, la cámara, la base (de arriba a
/// abajo) y la muesca para abrirlo.
pub const CANTO_COLOR: [u8; 3] = [92, 94, 100];
pub const BISEL_COLOR: [u8; 3] = [12, 12, 14];
pub const CAMARA_COLOR: [u8; 3] = [38, 40, 46];
pub const BASE_ARRIBA: [u8; 3] = [226, 228, 232];
pub const BASE_ABAJO: [u8; 3] = [150, 153, 160];
pub const MUESCA_COLOR: [u8; 3] = [132, 135, 142];

/// La carcasa ya pintada, del tamaño de la pieza y con transparencia alrededor, para ponerla
/// encima del fondo. Lo que queda debajo de la captura (y la tapa entera del todo) va
/// transparente: lo pisa la captura en cada fotograma y pintarlo sería trabajo tirado.
pub struct Capa {
    pub imagen: RgbaImage,
}

impl Capa {
    pub fn nueva(carcasa: Carcasa, ancho_f: u32, alto_f: u32) -> Self {
        let g = geometria(carcasa, ancho_f, alto_f);
        let mut imagen = RgbaImage::new(g.ancho, g.alto);
        match carcasa {
            Carcasa::Ventana { oscura } => pintar_ventana(&mut imagen, &g, oscura, ancho_f, alto_f),
            Carcasa::Portatil => pintar_portatil(&mut imagen, &g, ancho_f, alto_f),
        }
        // Lo que tapa la captura entera no se pinta: quien pone la capa se lo salta.
        let hueco = Forma { x: g.x as f32, y: g.y as f32, ancho: ancho_f as f32, alto: alto_f as f32, radios: g.radios };
        for y in g.y..g.y + alto_f {
            for x in g.x..g.x + ancho_f {
                if hueco.cubre(x, y) >= 1.0 {
                    imagen.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                }
            }
        }
        Self { imagen }
    }

    /// La pone sobre el lienzo con su esquina en `(x, y)`.
    pub fn poner(&self, lienzo: &mut RgbaImage, x: u32, y: u32) {
        let (ancho, alto) = self.imagen.dimensions();
        let lw = lienzo.width() as usize;
        let lh = lienzo.height();
        let origen = self.imagen.as_raw();
        let destino: &mut [u8] = lienzo;
        for fila in 0..alto {
            if y + fila >= lh {
                break;
            }
            for col in 0..ancho {
                let i = ((fila * ancho + col) * 4) as usize;
                let a = origen[i + 3];
                if a == 0 {
                    continue;
                }
                let j = (((y + fila) as usize) * lw + (x + col) as usize) * 4;
                if j + 4 > destino.len() {
                    continue;
                }
                if a == 255 {
                    destino[j..j + 3].copy_from_slice(&origen[i..i + 3]);
                    destino[j + 3] = 255;
                    continue;
                }
                let a = u32::from(a);
                for c in 0..3 {
                    destino[j + c] = ((u32::from(origen[i + c]) * a + u32::from(destino[j + c]) * (255 - a)) / 255) as u8;
                }
                destino[j + 3] = 255;
            }
        }
    }
}

/// Pinta una forma de un color encima de lo que haya, con el borde suavizado. `color` puede
/// cambiar con la altura dentro de la forma (de 0 arriba a 1 abajo), para el degradado de la base.
fn rellenar(imagen: &mut RgbaImage, forma: &Forma, color: &dyn Fn(f32) -> [u8; 3]) {
    let (iw, ih) = imagen.dimensions();
    let x0 = forma.x.floor().max(0.0) as u32;
    let y0 = forma.y.floor().max(0.0) as u32;
    let x1 = ((forma.x + forma.ancho).ceil() as u32).min(iw);
    let y1 = ((forma.y + forma.alto).ceil() as u32).min(ih);
    for y in y0..y1 {
        let t = ((y as f32 + 0.5 - forma.y) / forma.alto.max(1.0)).clamp(0.0, 1.0);
        let c = color(t);
        for x in x0..x1 {
            let cubre = forma.cubre(x, y);
            if cubre > 0.0 {
                encima(imagen, x, y, c, cubre);
            }
        }
    }
}

/// Un color encima de un píxel con esa opacidad (de 0 a 1), sobre lo que ya hubiera pintado.
fn encima(imagen: &mut RgbaImage, x: u32, y: u32, color: [u8; 3], opacidad: f32) {
    let p = imagen.get_pixel_mut(x, y);
    let a_arriba = opacidad.clamp(0.0, 1.0);
    let a_abajo = f32::from(p.0[3]) / 255.0;
    let a = a_arriba + a_abajo * (1.0 - a_arriba);
    if a <= 0.0 {
        return;
    }
    for c in 0..3 {
        let v = (f32::from(color[c]) * a_arriba + f32::from(p.0[c]) * a_abajo * (1.0 - a_arriba)) / a;
        p.0[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    p.0[3] = (a * 255.0).round() as u8;
}

/// Lo que tapa un trazo recto de `a` a `b` con ese grosor, en un píxel: la distancia del centro
/// del píxel al segmento contra medio grosor, suavizada.
fn cubre_trazo(x: u32, y: u32, a: (f32, f32), b: (f32, f32), grosor: f32) -> f32 {
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let largo = dx * dx + dy * dy;
    let t = if largo > 0.0 { (((px - a.0) * dx + (py - a.1) * dy) / largo).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    (grosor / 2.0 - d + 0.5).clamp(0.0, 1.0)
}

/// Pinta trazos rectos dentro de una caja, el de más cobertura en cada píxel: en la X de
/// cerrar, el cruce de las dos rayas no sale más oscuro que el resto.
fn trazos(imagen: &mut RgbaImage, caja: (u32, u32, u32, u32), lineas: &[((f32, f32), (f32, f32))], grosor: f32, color: [u8; 3]) {
    let (x0, y0, x1, y1) = caja;
    for y in y0..y1.min(imagen.height()) {
        for x in x0..x1.min(imagen.width()) {
            let cubre = lineas.iter().map(|&(a, b)| cubre_trazo(x, y, a, b, grosor)).fold(0.0, f32::max);
            if cubre > 0.0 {
                encima(imagen, x, y, color, cubre);
            }
        }
    }
}

/// Los tres dibujos de la barra de Windows 11, de izquierda a derecha: minimizar, maximizar y
/// cerrar. Cada uno, en la caja de su botón, centrado y en píxeles enteros para que la raya de
/// un píxel salga nítida.
pub fn botones(g: &Geometria, ancho_f: u32, alto_f: u32) -> [(f32, f32, f32); 3] {
    let s = escala(ancho_f, alto_f);
    let b = borde(s) as f32;
    let barra = (BARRA * s).round();
    let boton = BOTON * s;
    let lado = (GLIFO * s).round();
    let derecha = g.ancho as f32 - b;
    let cy = b + barra / 2.0;
    let mut cajas = [(0.0, 0.0, 0.0); 3];
    for (i, caja) in cajas.iter_mut().enumerate() {
        // El primero (minimizar) es el más alejado de la esquina.
        let cx = derecha - boton * (3 - i) as f32 + boton / 2.0;
        *caja = ((cx - lado / 2.0).round(), (cy - lado / 2.0).round(), lado);
    }
    cajas
}

fn pintar_ventana(imagen: &mut RgbaImage, g: &Geometria, oscura: bool, ancho_f: u32, alto_f: u32) {
    let (barra_color, glifo_color, borde_color) = colores_de_la_ventana(oscura);
    let s = escala(ancho_f, alto_f);
    let b = borde(s) as f32;
    let barra = (BARRA * s).round();
    let radio = (RADIO_VENTANA * s).round();
    // El borde: toda la ventana de su color, y encima, metida lo que mide el borde, la barra.
    let fuera = Forma::recta(0.0, 0.0, g.ancho as f32, g.alto as f32, radio);
    rellenar(imagen, &fuera, &|_| borde_color);
    let dentro = (radio - b).max(0.0);
    let barra_forma = Forma { x: b, y: b, ancho: g.ancho as f32 - 2.0 * b, alto: barra, radios: [dentro, dentro, 0.0, 0.0] };
    rellenar(imagen, &barra_forma, &|_| barra_color);

    let grosor = (TRAZO * s).max(1.0);
    let medio = grosor / 2.0;
    let [min, max, cerrar] = botones(g, ancho_f, alto_f);
    let caja = |(x, y, lado): (f32, f32, f32)| {
        (x.floor().max(0.0) as u32, y.floor().max(0.0) as u32, (x + lado).ceil() as u32 + 1, (y + lado).ceil() as u32 + 1)
    };
    // Minimizar: una raya en medio, sobre el centro de una fila de píxeles.
    let (x, y, lado) = min;
    let fila = y + (lado / 2.0).floor() + medio;
    trazos(imagen, caja(min), &[((x, fila), (x + lado, fila))], grosor, glifo_color);
    // Maximizar: un cuadrado con el trazo por dentro de su caja.
    let (x, y, lado) = max;
    let (a, z) = (medio, lado - medio);
    let cuadrado = [
        ((x + a, y + a), (x + z, y + a)),
        ((x + z, y + a), (x + z, y + z)),
        ((x + z, y + z), (x + a, y + z)),
        ((x + a, y + z), (x + a, y + a)),
    ];
    trazos(imagen, caja(max), &cuadrado, grosor, glifo_color);
    // Cerrar: las dos diagonales.
    let (x, y, lado) = cerrar;
    let equis = [((x + a, y + a), (x + z, y + z)), ((x + z, y + a), (x + a, y + z))];
    trazos(imagen, caja(cerrar), &equis, grosor, glifo_color);
}

fn pintar_portatil(imagen: &mut RgbaImage, g: &Geometria, ancho_f: u32, alto_f: u32) {
    let u = unidad_del_portatil(ancho_f, alto_f);
    let tapa = g.sombra[0];
    let base = g.sombra[1];
    // La base primero: la tapa apoya encima.
    let degradado = |t: f32| {
        let mezcla = |i: usize| (f32::from(BASE_ARRIBA[i]) + (f32::from(BASE_ABAJO[i]) - f32::from(BASE_ARRIBA[i])) * t).round() as u8;
        [mezcla(0), mezcla(1), mezcla(2)]
    };
    rellenar(imagen, &base, &degradado);
    // La muesca para abrirlo, en medio del canto de arriba de la base.
    let muesca_ancho = (MUESCA * u).round().min(base.ancho * 0.4);
    let muesca_alto = (base.alto * 0.32).round().max(1.0);
    let muesca = Forma {
        x: (base.x + (base.ancho - muesca_ancho) / 2.0).round(),
        y: base.y,
        ancho: muesca_ancho,
        alto: muesca_alto,
        radios: [0.0, 0.0, muesca_alto, muesca_alto],
    };
    rellenar(imagen, &muesca, &|_| MUESCA_COLOR);
    // La tapa: el canto de aluminio y, metido lo que mide, el bisel negro.
    rellenar(imagen, &tapa, &|_| CANTO_COLOR);
    let canto = (CANTO * u).round().max(1.0);
    let bisel = Forma {
        x: tapa.x + canto,
        y: tapa.y + canto,
        ancho: tapa.ancho - 2.0 * canto,
        alto: tapa.alto - 2.0 * canto,
        radios: tapa.radios.map(|r| (r - canto).max(0.0)),
    };
    rellenar(imagen, &bisel, &|_| BISEL_COLOR);
    // La cámara, en medio del bisel de arriba.
    let r = (CAMARA * u).max(1.0);
    let (cx, cy) = (tapa.x + tapa.ancho / 2.0, g.y as f32 / 2.0);
    let camara = Forma::recta(cx - r, cy - r, 2.0 * r, 2.0 * r, r);
    rellenar(imagen, &camara, &|_| CAMARA_COLOR);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los números que comprueba también `src/lib/carcasa.test.ts`: si uno cambia, el otro.
    #[test]
    fn las_medidas_son_las_mismas_que_dibuja_el_editor() {
        let v = geometria(Carcasa::Ventana { oscura: false }, 1920, 1080);
        assert_eq!((v.ancho, v.alto, v.x, v.y), (1922, 1114, 1, 33));
        assert_eq!(v.radios, [0.0, 0.0, 7.0, 7.0]);
        let v = geometria(Carcasa::Ventana { oscura: true }, 3840, 2160);
        assert_eq!((v.ancho, v.alto, v.x, v.y), (3844, 2228, 2, 66));
        let v = geometria(Carcasa::Ventana { oscura: false }, 400, 300);
        assert_eq!((v.ancho, v.alto, v.x, v.y), (402, 334, 1, 33));
        let p = geometria(Carcasa::Portatil, 1920, 1080);
        assert_eq!((p.ancho, p.alto, p.x, p.y), (2184, 1225, 132, 58));
        assert_eq!(p.sombra[0], Forma { x: 86.0, y: 0.0, ancho: 2012.0, alto: 1196.0, radios: [38.0, 38.0, 9.5, 9.5] });
        assert_eq!(p.sombra[1], Forma { x: 0.0, y: 1196.0, ancho: 2184.0, alto: 29.0, radios: [0.0, 0.0, 20.0, 20.0] });
        let p = geometria(Carcasa::Portatil, 300, 200);
        assert_eq!((p.ancho, p.alto, p.x, p.y), (388, 248, 44, 19));
    }

    #[test]
    fn la_distancia_es_negativa_dentro_y_positiva_fuera() {
        let f = Forma::recta(10.0, 10.0, 100.0, 50.0, 8.0);
        assert!(f.distancia(60.0, 35.0) < 0.0);
        assert!(f.distancia(5.0, 35.0) > 0.0);
        // La esquina redondeada deja fuera la punta.
        assert!(f.distancia(10.5, 10.5) > 0.0);
        assert_eq!(f.cubre(60, 35), 1.0);
        assert_eq!(f.cubre(0, 0), 0.0);
    }

    fn color(imagen: &RgbaImage, x: u32, y: u32) -> [u8; 3] {
        let p = imagen.get_pixel(x, y);
        [p.0[0], p.0[1], p.0[2]]
    }

    /// La barra de la ventana clara es clara, la oscura es oscura, y la X de cerrar se ve
    /// encima: los dos colores, el de la barra y el del dibujo, están en su sitio.
    #[test]
    fn la_ventana_lleva_su_barra_y_sus_tres_botones() {
        for oscura in [false, true] {
            let (barra, glifo, borde) = colores_de_la_ventana(oscura);
            let capa = Capa::nueva(Carcasa::Ventana { oscura }, 800, 500);
            let g = geometria(Carcasa::Ventana { oscura }, 800, 500);
            let img = &capa.imagen;
            // La barra, a la izquierda, donde no hay botones.
            assert_eq!(color(img, 40, 10), barra);
            // El borde, en medio del lado izquierdo.
            assert_eq!(color(img, 0, 200), borde);
            // Los tres dibujos llevan tinta del color de los glifos.
            for (x, y, lado) in botones(&g, 800, 500) {
                let mut con_tinta = 0;
                for yy in y as u32..(y + lado) as u32 {
                    for xx in x as u32..(x + lado) as u32 {
                        if color(img, xx, yy) == glifo {
                            con_tinta += 1;
                        }
                    }
                }
                assert!(con_tinta >= 8, "un botón casi sin dibujo ({con_tinta} píxeles), oscura={oscura}");
            }
            // La raya de minimizar, nítida: una sola fila de tinta llena.
            let (x, y, lado) = botones(&g, 800, 500)[0];
            let filas_llenas = (y as u32..(y + lado) as u32 + 1)
                .filter(|&yy| (x as u32 + 1..(x + lado) as u32 - 1).all(|xx| color(img, xx, yy) == glifo))
                .count();
            assert_eq!(filas_llenas, 1, "la raya de minimizar sale borrosa o doble");
            // Debajo de la captura, nada: la pisa la captura.
            assert_eq!(img.get_pixel(g.x + 100, g.y + 100).0[3], 0);
            // Y la esquina de fuera, transparente: ahí se ve el fondo.
            assert_eq!(img.get_pixel(0, 0).0[3], 0);
        }
    }

    /// Los botones van a la derecha, en el orden de Windows: el de cerrar en la esquina.
    #[test]
    fn cerrar_va_en_la_esquina_y_minimizar_el_primero() {
        let g = geometria(Carcasa::Ventana { oscura: false }, 1920, 1080);
        let [min, max, cerrar] = botones(&g, 1920, 1080);
        assert!(min.0 < max.0 && max.0 < cerrar.0);
        // El centro del de cerrar, a medio botón (23 px) del borde de dentro.
        let centro = cerrar.0 + cerrar.2 / 2.0;
        assert!((g.ancho as f32 - 1.0 - 23.0 - centro).abs() <= 0.5, "cerrar centrado en {centro}");
        assert_eq!(cerrar.2, 10.0);
    }

    /// Las tres carcasas sobre una captura de verdad, para mirarlas: deja los PNG en la carpeta
    /// de `WINSHOTX_FOTOS`, pintados por el mismo `marco::poner` que usa «Guardar».
    ///
    ///     WINSHOTX_FOTOS=<carpeta> WINSHOTX_CAPTURA=<png> cargo test --lib ver_las_carcasas -- --ignored
    #[test]
    #[ignore = "deja fotos para mirarlas: se corre a mano"]
    fn ver_las_carcasas() {
        use super::super::marco::{poner, Fondo, Marco};
        let carpeta = std::path::PathBuf::from(std::env::var("WINSHOTX_FOTOS").expect("WINSHOTX_FOTOS"));
        let captura = image::open(std::env::var("WINSHOTX_CAPTURA").expect("WINSHOTX_CAPTURA")).expect("abrir").to_rgba8();
        for (nombre, carcasa) in [
            ("ventana-clara", Carcasa::Ventana { oscura: false }),
            ("ventana-oscura", Carcasa::Ventana { oscura: true }),
            ("portatil", Carcasa::Portatil),
        ] {
            let marco = Marco { margen: 42, fondo: Fondo::Atardecer, sombra: true, esquinas: 10, carcasa: Some(carcasa) };
            poner(&captura, marco).save(carpeta.join(format!("archivo-{nombre}.png"))).expect("guardar");
        }
    }

    #[test]
    fn el_portatil_lleva_bisel_camara_y_base() {
        let capa = Capa::nueva(Carcasa::Portatil, 1280, 800);
        let g = geometria(Carcasa::Portatil, 1280, 800);
        let img = &capa.imagen;
        let tapa = g.sombra[0];
        // El bisel, a la izquierda de la pantalla.
        assert_eq!(color(img, g.x - 10, g.y + 300), BISEL_COLOR);
        // La cámara, en medio del bisel de arriba.
        let cx = (tapa.x + tapa.ancho / 2.0) as u32;
        assert_eq!(color(img, cx, g.y / 2), CAMARA_COLOR);
        // La base asoma por los dos lados de la tapa, más ancha que ella.
        let fila_base = g.sombra[1].y as u32 + 2;
        assert_eq!(img.get_pixel(30, fila_base).0[3], 255, "la base no asoma a la izquierda");
        assert_eq!(img.get_pixel(g.ancho - 31, fila_base).0[3], 255, "ni a la derecha");
        // Encima de la base, a los lados de la tapa, no hay nada: ahí va el fondo.
        assert_eq!(img.get_pixel(10, 100).0[3], 0);
        // La pantalla, vacía: la pisa la captura.
        assert_eq!(img.get_pixel(g.x + 5, g.y + 5).0[3], 0);
    }
}
