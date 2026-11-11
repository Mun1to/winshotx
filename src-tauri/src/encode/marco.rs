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
/// Los de nombre son unos pocos elegidos, no un selector de color: con la rueda entera, la
/// decisión pasa de «cuál de estos cinco» a «cuál de dieciséis millones», que es
/// exactamente el tipo de pregunta que hace que alguien cierre el panel sin exportar. Por
/// eso los propios (un color cualquiera, una imagen, el escritorio, un vídeo) van detrás,
/// en el panel, y aquí llegan ya resueltos por `Fondo::elegido`.
///
/// Sigue siendo `Copy`: una imagen o un vídeo van por su número en una tabla (`Lamina`), no
/// dentro del valor, para que el `Marco` se pueda pasar a cada fotograma sin copiar nada.
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
    /// Un color liso cualquiera, de la paleta (B3).
    Color([u8; 3]),
    /// Un degradado que sale de ese color hacia `segundo_color` (B3).
    Degradado([u8; 3]),
    /// Una imagen del disco, recortada para llenar (B1, y B2 con la del escritorio).
    Imagen(Lamina),
    /// Un vídeo de fondo ya importado, en ese instante del vídeo exportado (B5).
    Video { video: Lamina, ms: u64 },
}

/// El número de una imagen o un vídeo en la tabla de rutas de este módulo.
pub type Lamina = u32;

/// Las rutas de las imágenes y los vídeos de fondo que se han pedido, por orden de llegada,
/// con la hora en que se escribió cada archivo. Solo crece con cada archivo distinto, y una
/// sesión de la app usa unos pocos.
static RUTAS: std::sync::Mutex<Vec<(std::path::PathBuf, Option<std::time::SystemTime>)>> =
    std::sync::Mutex::new(Vec::new());

/// El número de esa ruta. **Con la hora del archivo**: el fondo de escritorio de Windows es
/// siempre el mismo archivo (`TranscodedWallpaper`) aunque cambie la imagen, y con la ruta
/// sola la caché de `base` seguiría pintando la de antes.
fn registrar(ruta: &std::path::Path) -> Lamina {
    let hora = std::fs::metadata(ruta).and_then(|m| m.modified()).ok();
    let mut rutas = RUTAS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = rutas.iter().position(|(r, h)| r == ruta && *h == hora) {
        return i as Lamina;
    }
    rutas.push((ruta.to_path_buf(), hora));
    (rutas.len() - 1) as Lamina
}

fn ruta_de(lamina: Lamina) -> Option<std::path::PathBuf> {
    RUTAS.lock().ok()?.get(lamina as usize).map(|(r, _)| r.clone())
}

/// El otro extremo del degradado de un color: el tono girado 40 grados y la luz al 82 %.
///
/// Así sale de un color cualquiera un degradado parecido a los de nombre (de azul a violeta,
/// de verde a azul) en vez de uno de ese color a negro, que se ve sucio. Va en `f64` y con
/// las mismas cuentas que `segundoColor` de `src/lib/fondos.ts`: la vista previa tiene que
/// pintar el mismo degradado que sale en el archivo, y las pruebas de los dos lados
/// comprueban los mismos números.
pub fn segundo_color(rgb: [u8; 3]) -> [u8; 3] {
    let [r, g, b] = rgb.map(|c| f64::from(c) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, s) = if d == 0.0 {
        (0.0, 0.0)
    } else {
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s)
    };
    let h = (h + 40.0) % 360.0;
    let l = l * 0.82;
    // De HSL a RGB, la fórmula de siempre.
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|v| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// Si se puede abrir esa imagen, mirando solo su cabecera: decodificarla entera aquí sería
/// pagar dos veces lo que luego paga `pintar_base`.
fn se_puede_abrir(ruta: &std::path::Path) -> bool {
    image::ImageReader::open(ruta)
        .and_then(|r| r.with_guessed_format())
        .ok()
        .and_then(|r| r.into_dimensions().ok())
        .is_some()
}

impl Fondo {
    /// El fondo de una petición: el propio (`edicion.fondo`) si viene, y si no el de nombre.
    ///
    /// Lo que no se puede usar (una imagen que ya no está, un escritorio sin imagen ni color)
    /// cae al de nombre en vez de reventar la exportación: el panel ya comprobó el archivo
    /// al elegirlo, y lo único que puede pasar entre medias es que alguien lo borre.
    pub fn elegido(nombre: &str, elegido: Option<&crate::contrato::FondoElegido>) -> Self {
        use crate::contrato::FondoElegido;
        let respaldo = Fondo::desde(nombre);
        match elegido {
            None => respaldo,
            Some(FondoElegido::Color { rgb }) => Fondo::Color(*rgb),
            Some(FondoElegido::Degradado { rgb }) => Fondo::Degradado(*rgb),
            Some(FondoElegido::Imagen { ruta }) => {
                let ruta = std::path::Path::new(ruta);
                if se_puede_abrir(ruta) {
                    Fondo::Imagen(registrar(ruta))
                } else {
                    respaldo
                }
            }
            Some(FondoElegido::Escritorio) => match crate::platform::escritorio::actual() {
                Some(crate::platform::escritorio::FondoDeEscritorio::Imagen(ruta)) if se_puede_abrir(&ruta) => {
                    Fondo::Imagen(registrar(&ruta))
                }
                Some(crate::platform::escritorio::FondoDeEscritorio::Color(rgb)) => Fondo::Color(rgb),
                _ => respaldo,
            },
            Some(FondoElegido::Video { ruta }) => {
                let ruta = std::path::Path::new(ruta);
                if ruta.is_file() {
                    Fondo::Video { video: registrar(ruta), ms: 0 }
                } else {
                    respaldo
                }
            }
        }
    }

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
            Fondo::Color([r, g, b]) => Rgba([r, g, b, 255]),
            Fondo::Degradado(rgb) => mezcla(rgb, segundo_color(rgb)),
            // Los de archivo no tienen un color por punto: los pinta `pintar_base` o el hilo
            // del fondo animado. Si llega a pedirse, negro, que es lo que hay detrás.
            Fondo::Imagen(_) | Fondo::Video { .. } => Rgba([0, 0, 0, 255]),
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

    /// Si el fondo cambia con el tiempo: entonces cada fotograma lleva el suyo, y dos
    /// instantes iguales de la grabación ya no se ven iguales en el vídeo.
    pub fn animado(&self) -> bool {
        self.margen > 0 && matches!(self.fondo, Fondo::Video { .. })
    }

    /// El mismo marco, con el fondo animado en ese instante del vídeo exportado.
    pub fn en(self, ms: u64) -> Marco {
        match self.fondo {
            Fondo::Video { video, .. } => Marco { fondo: Fondo::Video { video, ms }, ..self },
            _ => self,
        }
    }

    /// El marco del paso que viene después de estos: su instante es lo que duran todos los
    /// de antes. Es lo que le pasa el exportador a cada fotograma, y sin fondo animado no
    /// suma nada.
    pub fn tras(self, retardos: &[u32]) -> Marco {
        if !self.animado() {
            return self;
        }
        self.en(retardos.iter().map(|&r| u64::from(r)).sum())
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
    // de exportar un vídeo con marco. El animado no es el mismo, pero lo pinta otro hilo por
    // delante (`fondo_video::Animado`), y aquí también solo se copia.
    let fondo = match marco.fondo {
        Fondo::Video { video, ms } => base_animada(video, ms, marco, ancho_f, alto_f),
        _ => None,
    };
    let mut lienzo = match fondo {
        Some(animada) => (*animada).clone(),
        // Un vídeo que no se pudo leer sale sobre negro, que es lo que hay detrás.
        None if marco.animado() => (*base(Marco { fondo: Fondo::Negro, ..marco }, ancho_f, alto_f)).clone(),
        None => (*base(marco, ancho_f, alto_f)).clone(),
    };
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

/// Lo que identifica un fondo ya pintado: el tamaño de la captura y todo lo del marco. Con
/// una imagen, su número en `RUTAS`: dos imágenes distintas nunca comparten fondo.
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
    // Una imagen se recorta para llenar; si ya no se puede abrir, sale el fondo blanco de
    // siempre en vez de un agujero.
    let imagen = match marco.fondo {
        Fondo::Imagen(lamina) => ruta_de(lamina).and_then(|ruta| {
            image::ImageReader::open(&ruta).ok()?.with_guessed_format().ok()?.decode().ok()
        }),
        _ => None,
    };
    let mut lienzo = match (&imagen, marco.fondo) {
        (Some(imagen), _) => cubrir(&imagen.to_rgba8(), ancho, alto),
        (None, Fondo::Imagen(_)) => RgbaImage::from_pixel(ancho, alto, Rgba([255, 255, 255, 255])),
        _ => {
            let mut lienzo = RgbaImage::new(ancho, alto);
            // El degradado corre en diagonal, así que el punto de cada píxel es lo lejos que
            // está de la esquina de arriba a la izquierda medido sobre esa diagonal.
            let diagonal = (ancho + alto).max(1) as f32;
            for (x, y, pixel) in lienzo.enumerate_pixels_mut() {
                *pixel = marco.fondo.color((x + y) as f32 / diagonal);
            }
            lienzo
        }
    };
    if marco.sombra {
        aplicar_sombra(&mut lienzo, &mascara_de_sombra(marco, ancho_f, alto_f));
    }
    lienzo
}

/// Escala una imagen para que llene `ancho` x `alto` sin deformarse, recortando lo que sobra
/// por los lados o por arriba y abajo, a partes iguales. Es el «cubrir» de CSS
/// (`background-size: cover`), que es lo que pinta la vista previa.
pub fn cubrir(imagen: &RgbaImage, ancho: u32, alto: u32) -> RgbaImage {
    let (ow, oh) = imagen.dimensions();
    if (ow, oh) == (ancho, alto) || ow == 0 || oh == 0 {
        return super::escalar::a_medida(imagen, ancho.max(1), alto.max(1));
    }
    // El trozo del original que tiene la proporción del lienzo, lo más grande posible.
    let (cw, ch) = if u64::from(ow) * u64::from(alto) > u64::from(oh) * u64::from(ancho) {
        (((u64::from(oh) * u64::from(ancho)) / u64::from(alto.max(1))).max(1) as u32, oh)
    } else {
        (ow, ((u64::from(ow) * u64::from(alto)) / u64::from(ancho.max(1))).max(1) as u32)
    };
    let (cw, ch) = (cw.min(ow), ch.min(oh));
    let trozo = if (cw, ch) == (ow, oh) {
        imagen.clone()
    } else {
        image::imageops::crop_imm(imagen, (ow - cw) / 2, (oh - ch) / 2, cw, ch).to_image()
    };
    super::escalar::a_medida(&trozo, ancho, alto)
}

/// Lo que identifica un fondo animado ya abierto: el vídeo, el tamaño de la captura y el
/// marco. Con un `None` dentro, ese vídeo no se pudo leer y no se vuelve a intentar.
type ClaveAnimada = (Lamina, u32, u32, u32, bool, u32);

/// El fondo animado en ese instante, pintado por su hilo. `None` si el vídeo no se puede
/// leer: quien llama pinta negro.
fn base_animada(video: Lamina, ms: u64, marco: Marco, ancho_f: u32, alto_f: u32) -> Option<std::sync::Arc<RgbaImage>> {
    use super::fondo_video::Animado;
    static ABIERTO: std::sync::Mutex<Option<(ClaveAnimada, Option<Animado>)>> = std::sync::Mutex::new(None);
    let clave = (video, ancho_f, alto_f, marco.margen, marco.sombra, marco.esquinas);
    let mut abierto = ABIERTO.lock().unwrap_or_else(|e| e.into_inner());
    // Dos intentos: el hilo se cierra solo cuando lleva un rato sin uso, y entonces se
    // vuelve a abrir desde el instante que se pida.
    for _ in 0..2 {
        if abierto.as_ref().is_none_or(|(k, _)| *k != clave) {
            let ruta = ruta_de(video)?;
            let (ancho, alto) = marco.medida(ancho_f, alto_f);
            let mascara = marco.sombra.then(|| mascara_de_sombra(marco, ancho_f, alto_f));
            let vestir: super::fondo_video::Vestir = Box::new(move |lienzo: &mut RgbaImage| {
                if let Some(m) = &mascara {
                    aplicar_sombra(lienzo, m);
                }
            });
            *abierto = Some((clave, Some(Animado::abrir(ruta, ancho, alto, vestir))));
        }
        let (_, animado) = abierto.as_ref()?;
        match animado.as_ref()?.en(ms) {
            Ok(imagen) => return Some(imagen),
            // No se puede leer: se apunta para no reintentarlo en cada fotograma.
            Err(Some(_)) => {
                *abierto = Some((clave, None));
                return None;
            }
            Err(None) => *abierto = None,
        }
    }
    None
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

/// Una mancha oscura debajo de la captura, desplazada y con los bordes difuminados: cuánto
/// oscurece cada píxel del lienzo, de 0 a 255.
///
/// No es un desenfoque de verdad: es una rampa lineal en los bordes del rectángulo. A
/// simple vista se distingue poco de un desenfoque gaussiano cuando la mancha está detrás
/// de la imagen, y cuesta una fracción de lo que cuesta el gaussiano en cada uno de los
/// trescientos fotogramas de un vídeo.
///
/// Se saca aparte del pintado para que un fondo animado la calcule UNA vez y la ponga en
/// cada fotograma de su vídeo solo multiplicando: las raíces cuadradas de abajo, en tres
/// millones de píxeles, eran el grueso de pintar el fondo.
fn mascara_de_sombra(marco: Marco, ancho_f: u32, alto_f: u32) -> Vec<u8> {
    let (ancho, alto) = marco.medida(ancho_f, alto_f);
    let margen = marco.margen;
    let radio = marco.esquinas.min(ancho_f.min(alto_f) / 2) as f32;
    let caida = (margen as f32 * SOMBRA_CAIDA).round() as i32;
    let borrosa = (margen as f32 * SOMBRA_BORROSA).max(1.0);

    let x0 = margen as i32;
    let y0 = margen as i32 + caida;
    let x1 = x0 + ancho_f as i32;
    let y1 = y0 + alto_f as i32;

    let mut mascara = vec![0u8; ancho as usize * alto as usize];
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
            mascara[y as usize * ancho as usize + x as usize] = (fuerza * 255.0) as u8;
        }
    }
    mascara
}

/// Oscurece el lienzo con su máscara de sombra.
fn aplicar_sombra(lienzo: &mut RgbaImage, mascara: &[u8]) {
    for (pixel, &a) in lienzo.pixels_mut().zip(mascara) {
        if a != 0 {
            *pixel = sobre(Rgba([0, 0, 0, a]), *pixel);
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

/// Los fondos propios (S3): un color, su degradado, una imagen, el escritorio y un vídeo.
#[cfg(test)]
mod fondos_propios {
    use super::*;
    use crate::contrato::FondoElegido;

    const ROJO: Rgba<u8> = Rgba([220, 30, 30, 255]);

    fn con(fondo: Fondo, margen: u32) -> Marco {
        Marco { margen, fondo, sombra: false, esquinas: 0 }
    }

    /// Los mismos números que comprueba `src/lib/fondos.test.ts`: si uno cambia y el otro no,
    /// la vista previa enseña un degradado y el archivo sale con otro.
    #[test]
    fn el_segundo_color_es_el_mismo_que_calcula_el_editor() {
        assert_eq!(segundo_color([88, 116, 245]), [134, 31, 242]);
        assert_eq!(segundo_color([34, 197, 152]), [28, 109, 162]);
        assert_eq!(segundo_color([255, 255, 255]), [209, 209, 209]);
        assert_eq!(segundo_color([0, 0, 0]), [0, 0, 0]);
        assert_eq!(segundo_color([220, 40, 40]), [183, 132, 30]);
        assert_eq!(segundo_color([18, 18, 20]), [16, 15, 16]);
    }

    #[test]
    fn un_color_liso_es_ese_color_en_las_cuatro_esquinas() {
        let lienzo = poner(&RgbaImage::from_pixel(10, 10, ROJO), con(Fondo::Color([12, 200, 99]), 6));
        let (a, b) = lienzo.dimensions();
        for (x, y) in [(0, 0), (a - 1, 0), (0, b - 1), (a - 1, b - 1)] {
            assert_eq!(*lienzo.get_pixel(x, y), Rgba([12, 200, 99, 255]), "en {x},{y}");
        }
    }

    #[test]
    fn el_degradado_va_del_color_elegido_a_su_segundo_color() {
        let rgb = [34, 197, 152];
        let lienzo = poner(&RgbaImage::from_pixel(10, 10, ROJO), con(Fondo::Degradado(rgb), 40));
        let (a, b) = lienzo.dimensions();
        assert_eq!(lienzo.get_pixel(0, 0).0[..3], rgb);
        let fin = lienzo.get_pixel(a - 1, b - 1).0;
        let otro = segundo_color(rgb);
        for c in 0..3 {
            assert!((fin[c] as i32 - otro[c] as i32).abs() <= 3, "la esquina final {fin:?} no es {otro:?}");
        }
    }

    fn dir(nombre: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("winshotx-fondos-{nombre}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Una imagen apaisada de dos mitades, roja y azul, sobre un lienzo cuadrado: «cubrir»
    /// recorta los lados y deja la mitad roja a la izquierda y la azul a la derecha.
    #[test]
    fn una_imagen_se_recorta_para_llenar_sin_deformarse() {
        let dir = dir("imagen");
        let ruta = dir.join("dos-mitades.png");
        let imagen = RgbaImage::from_fn(200, 100, |x, _| {
            if x < 100 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 255, 255]) }
        });
        imagen.save(&ruta).unwrap();
        let fondo = Fondo::elegido("blanco", Some(&FondoElegido::Imagen { ruta: ruta.to_string_lossy().into() }));
        assert!(matches!(fondo, Fondo::Imagen(_)), "la imagen se tendría que poder usar: {fondo:?}");
        let lienzo = poner(&RgbaImage::from_pixel(20, 20, Rgba([0, 255, 0, 255])), con(fondo, 10));
        assert_eq!(lienzo.dimensions(), (40, 40));
        assert_eq!(lienzo.get_pixel(2, 20).0[..3], [255, 0, 0], "la izquierda, roja");
        assert_eq!(lienzo.get_pixel(37, 20).0[..3], [0, 0, 255], "la derecha, azul");
        assert_eq!(lienzo.get_pixel(20, 20).0[..3], [0, 255, 0], "y la captura encima");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_imagen_que_ya_no_esta_cae_al_fondo_de_nombre() {
        let fondo = Fondo::elegido("menta", Some(&FondoElegido::Imagen { ruta: "C:/no-existe/x.png".into() }));
        assert_eq!(fondo, Fondo::Menta);
        let fondo = Fondo::elegido("coral", Some(&FondoElegido::Video { ruta: "C:/no-existe/x.mp4".into() }));
        assert_eq!(fondo, Fondo::Coral);
        assert_eq!(Fondo::elegido("noche", None), Fondo::Noche);
    }

    /// El del escritorio sale de Windows: una imagen que se puede abrir o un color liso,
    /// pero nunca el de nombre si Windows contesta.
    #[test]
    #[cfg(windows)]
    fn el_escritorio_da_una_imagen_o_un_color() {
        let fondo = Fondo::elegido("blanco", Some(&FondoElegido::Escritorio));
        assert!(matches!(fondo, Fondo::Imagen(_) | Fondo::Color(_)), "ha salido {fondo:?}");
    }

    /// Lo que cambia la caché: la misma ruta con otro archivo dentro es otro fondo.
    #[test]
    fn la_misma_ruta_con_otra_imagen_dentro_es_otro_fondo() {
        let dir = dir("misma-ruta");
        let ruta = dir.join("fondo.png");
        RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255])).save(&ruta).unwrap();
        let antes = registrar(&ruta);
        std::thread::sleep(std::time::Duration::from_millis(30));
        RgbaImage::from_pixel(8, 8, Rgba([0, 0, 255, 255])).save(&ruta).unwrap();
        assert_ne!(registrar(&ruta), antes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn solo_el_video_con_aire_es_animado_y_su_instante_suma_los_pasos_de_antes() {
        let quieto = con(Fondo::Atardecer, 10);
        assert!(!quieto.animado());
        assert_eq!(quieto.tras(&[33, 33]).fondo, Fondo::Atardecer);
        let video = con(Fondo::Video { video: 7, ms: 0 }, 10);
        assert!(video.animado());
        assert_eq!(video.tras(&[33, 34, 33]).fondo, Fondo::Video { video: 7, ms: 100 });
        assert!(!con(Fondo::Video { video: 7, ms: 0 }, 0).animado(), "sin aire no hay fondo que ver");
    }

    /// Un fondo animado de punta a punta por `poner`: el fondo de cada instante es su
    /// fotograma, con la captura encima y la sombra puesta igual que en uno quieto.
    #[test]
    fn el_fondo_animado_cambia_con_el_instante() {
        let dir = dir("animado");
        let ruta = dir.join("fondo.gif");
        crate::encode::fondo_video::pruebas::gif_de_prueba(&ruta, &[[255, 0, 0], [0, 0, 255]], 16, 16, 10);
        let fondo = Fondo::elegido("blanco", Some(&FondoElegido::Video { ruta: ruta.to_string_lossy().into() }));
        let marco = Marco { sombra: true, ..con(fondo, 12) };
        let captura = RgbaImage::from_pixel(24, 24, Rgba([0, 255, 0, 255]));
        let a = poner(&captura, marco.en(0));
        let b = poner(&captura, marco.en(150));
        assert_eq!(a.get_pixel(1, 1).0[..3], [255, 0, 0], "a los 0 ms el fondo es el primer fotograma");
        assert_eq!(b.get_pixel(1, 1).0[..3], [0, 0, 255], "a los 150, el segundo");
        assert_eq!(a.get_pixel(24, 24).0[..3], [0, 255, 0], "la captura va encima");
        // La sombra: debajo de la captura, más oscuro que arriba a la misma distancia.
        let (debajo, encima) = (b.get_pixel(24, 38).0[2], b.get_pixel(24, 2).0[2]);
        assert!(debajo < encima, "sin sombra en el fondo animado: {debajo} contra {encima}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Un vídeo roto no revienta: sale sobre negro.
    #[test]
    fn un_video_que_no_se_puede_leer_sale_sobre_negro() {
        let dir = dir("roto");
        let ruta = dir.join("roto.gif");
        std::fs::write(&ruta, b"esto no es un gif").unwrap();
        let fondo = Fondo::elegido("blanco", Some(&FondoElegido::Video { ruta: ruta.to_string_lossy().into() }));
        let lienzo = poner(&RgbaImage::from_pixel(10, 10, ROJO), con(fondo, 6));
        assert_eq!(*lienzo.get_pixel(0, 0), Fondo::Negro.color(0.0));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
