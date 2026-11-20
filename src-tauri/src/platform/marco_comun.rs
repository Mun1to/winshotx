//! La figura del marco de grabar y su dibujo: lo que no depende del sistema.
//!
//! Los colores, las medidas a cada escala, las cuatro tiras que rodean la region, que lados
//! van por dentro y el dibujo de cada tira pixel a pixel. Cada sistema pone las ventanas:
//! `marco.rs` en Windows, que es donde se cuenta que es el marco y por que es asi, y
//! `marco_x11.rs` en Linux.

use crate::capture::Rect;

/// Rojo mientras se graba: es el color que todo el mundo lee como «grabando», y el mismo del
/// punto de la barra (`bg-red-500`).
const ROJO: Color = Color(0xEF, 0x44, 0x44);
/// El azul de winshotx (`#0a9bff`, el del logo y la web).
const AZUL: Color = Color(0x0A, 0x9B, 0xFF);
const BLANCO: Color = Color(0xFF, 0xFF, 0xFF);
const NEGRO: Color = Color(0, 0, 0);
/// Ambar en pausa, el mismo del punto de la barra en pausa.
const EN_PAUSA: Color = Color(0xF5, 0x9E, 0x0B);

/// Como se ve el marco. Munito, 16-nov: «no me gusta, revísalo, el diseño rojo»; entre las
/// variaciones (`ver_las_variaciones_del_marco`) eligió la 2, el azul con luz.
// Solo se usa uno a la vez (`ESTILO`): los otros viven para las fotos de las variaciones.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estilo {
    /// 0. El de la 0.4.1 a la 0.5.0: linea roja y una sombra negra corta.
    Rojo,
    /// 1. La misma linea y la misma sombra, en el azul de winshotx.
    Azul,
    /// 2. La linea azul con su luz alrededor en vez de sombra.
    AzulConLuz,
    /// 3. Una linea blanca con la luz azul de winshotx por fuera: se lee sobre lo oscuro por
    /// la linea y sobre lo claro por la luz.
    BlancoConLuz,
}

/// El marco que se ve al grabar: el azul de winshotx con su luz (Munito, 16-nov, «2. Azul con
/// luz»). En pausa, la linea y la luz pasan a ambar.
pub const ESTILO: Estilo = Estilo::AzulConLuz;

/// Lo que despega la linea del fondo, por fuera: un color, lo fuerte que es pegado a la linea
/// (se apaga hacia fuera hasta cero) y lo que mide a 100 %.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Halo {
    color: Color,
    fuerza: f32,
    ancho: f64,
}

impl Estilo {
    #[cfg_attr(not(test), allow(dead_code))]
    pub const TODOS: [Estilo; 4] = [Estilo::Rojo, Estilo::Azul, Estilo::AzulConLuz, Estilo::BlancoConLuz];

    /// El color de la linea, grabando o en pausa.
    pub(crate) fn linea(self, pausado: bool) -> Color {
        match (self, pausado) {
            (Estilo::BlancoConLuz, _) => BLANCO,
            (_, true) => EN_PAUSA,
            (Estilo::Rojo, false) => ROJO,
            (Estilo::Azul | Estilo::AzulConLuz, false) => AZUL,
        }
    }

    /// El halo de fuera. La sombra es negra y solo oscurece; la luz es del color del estado
    /// (azul grabando, ambar en pausa), y en el blanco es la que dice si esta en pausa.
    fn halo(self, pausado: bool) -> Halo {
        let estado = if pausado { EN_PAUSA } else { AZUL };
        match self {
            Estilo::Rojo | Estilo::Azul => Halo { color: NEGRO, fuerza: 0.30, ancho: 5.0 },
            Estilo::AzulConLuz => Halo { color: estado, fuerza: 0.55, ancho: 9.0 },
            Estilo::BlancoConLuz => Halo { color: estado, fuerza: 0.75, ancho: 9.0 },
        }
    }
}

/// Un color de la linea, en rojo, verde y azul.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Color(pub(crate) u8, pub(crate) u8, pub(crate) u8);

/// Lo que mide el marco en pixeles de la pantalla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Medidas {
    /// Lo que se separa la linea de la region. Sin hueco, la esquina de dentro tendria que ir
    /// en angulo recto: una esquina redonda pegada a la region se meteria en ella.
    pub(crate) hueco: i32,
    /// La linea de color.
    pub(crate) linea: i32,
    /// El radio de la esquina de dentro de la linea (la de fuera es este mas la linea).
    radio: i32,
    /// El halo, por fuera de la linea.
    halo: i32,
}

/// Hasta donde puede llegar el radio con un hueco dado sin que la esquina de la region asome
/// por dentro de la linea: el pico de la region queda a `raiz(2) * (radio - hueco)` del centro
/// de la curva, y tiene que caber en el radio. Sale `radio <= hueco * raiz(2) / (raiz(2) - 1)`.
const RADIO_POR_HUECO: f64 = std::f64::consts::SQRT_2 / (std::f64::consts::SQRT_2 - 1.0);

impl Medidas {
    /// A 100 %: dos pixeles de hueco, dos de linea, esquina de seis por dentro (ocho por
    /// fuera, la de las ventanas de Windows 11) y el halo de su estilo (cinco de sombra). Crecen
    /// con la escala de la pantalla y se redondean a pixeles enteros: una linea de 2,5 pixeles
    /// no existe, se ve borrosa.
    pub(crate) fn a_escala(escala: f64, estilo: Estilo) -> Self {
        let e = if escala.is_finite() && escala > 0.0 { escala } else { 1.0 };
        let px = |v: f64, minimo: f64| (v * e).round().max(minimo) as i32;
        let hueco = px(2.0, 1.0);
        Self {
            hueco,
            linea: px(2.0, 1.0),
            radio: px(6.0, 0.0).min((hueco as f64 * RADIO_POR_HUECO).floor() as i32),
            halo: px(estilo.halo(false).ancho, 2.0),
        }
    }

    /// Lo que sale el marco por fuera de la region, por cada lado que va por fuera.
    pub(crate) fn grosor(self) -> i32 {
        self.hueco + self.linea + self.halo
    }
}

/// La figura del marco alrededor de una region: el borde de dentro de la linea, el radio de
/// cada esquina y las cuatro tiras (ventanas) que la cubren.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Forma {
    region: Rect,
    medidas: Medidas,
    /// El borde de dentro de la linea, contado desde la esquina de arriba a la izquierda de la
    /// region (para no hacer cuentas finas con coordenadas grandes: las pruebas viven en
    /// -20000): izquierda, arriba, derecha y abajo.
    contorno: [f32; 4],
    /// El radio de cada esquina del contorno: arriba a la izquierda, arriba a la derecha,
    /// abajo a la izquierda y abajo a la derecha.
    radios: [f32; 4],
    /// Las cuatro tiras, en la pantalla: arriba, abajo, izquierda y derecha. Arriba y abajo
    /// cubren tambien las esquinas.
    pub(crate) tiras: [Rect; 4],
}

impl Forma {
    /// El marco de esa region, con cada lado por fuera o, si `dentro` lo dice (arriba, abajo,
    /// izquierda, derecha), por dentro y pegado al borde.
    ///
    /// Una esquina con algun lado por dentro va en angulo recto: redonda, su curva se meteria
    /// en la region por donde no hay tira que la pinte, y la linea saldria cortada.
    pub(crate) fn nueva(region: Rect, medidas: Medidas, dentro: [bool; 4]) -> Self {
        let (w, h) = (region.width as i32, region.height as i32);
        let Medidas { hueco, linea, radio, .. } = medidas;
        let g = medidas.grosor();
        let [arriba, abajo, izquierda, derecha] = dentro;
        // De cada lado, por orden: el borde de dentro de la linea, lo de fuera del todo del
        // marco y el borde del agujero que dejan las tiras en medio. Por fuera, el agujero es
        // la region; por dentro, la region menos la linea.
        let lado = |por_dentro: bool, borde: i32, hacia_fuera: i32| {
            if por_dentro {
                (borde - hacia_fuera * linea, borde, borde - hacia_fuera * linea)
            } else {
                (borde + hacia_fuera * hueco, borde + hacia_fuera * g, borde)
            }
        };
        let (c_arr, o_arr, m_arr) = lado(arriba, 0, -1);
        let (c_aba, o_aba, m_aba) = lado(abajo, h, 1);
        let (c_izq, o_izq, m_izq) = lado(izquierda, 0, -1);
        let (c_der, o_der, m_der) = lado(derecha, w, 1);
        let tira = |x1: i32, y1: i32, x2: i32, y2: i32| Rect {
            x: region.x + x1,
            y: region.y + y1,
            width: (x2 - x1).max(1) as u32,
            height: (y2 - y1).max(1) as u32,
        };
        let redonda = |a: bool, b: bool| if a || b { 0.0 } else { radio as f32 };
        Self {
            region,
            medidas,
            contorno: [c_izq as f32, c_arr as f32, c_der as f32, c_aba as f32],
            radios: [
                redonda(arriba, izquierda),
                redonda(arriba, derecha),
                redonda(abajo, izquierda),
                redonda(abajo, derecha),
            ],
            tiras: [
                tira(o_izq, o_arr, o_der, m_arr),
                tira(o_izq, m_aba, o_der, o_aba),
                tira(o_izq, m_arr, m_izq, m_aba),
                tira(m_der, m_arr, o_der, m_aba),
            ],
        }
    }

    /// El radio de la esquina que toca a ese punto (contado desde la region).
    fn radio_en(&self, x: f32, y: f32) -> f32 {
        let [x1, y1, x2, y2] = self.contorno;
        let (derecha, abajo) = (x >= (x1 + x2) / 2.0, y >= (y1 + y2) / 2.0);
        self.radios[usize::from(derecha) + 2 * usize::from(abajo)]
    }

    /// Lo mismo que otra forma salvo el sitio: basta con mover las ventanas, sin repintarlas.
    // Solo Windows sigue a una ventana que se mueve.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn misma_figura(&self, otra: &Forma) -> bool {
        let tamanno = |f: &Forma| f.tiras.map(|t| (t.width, t.height));
        self.medidas == otra.medidas
            && self.contorno == otra.contorno
            && self.radios == otra.radios
            && tamanno(self) == tamanno(otra)
    }
}

/// La distancia de un punto al borde de dentro de la linea, en pixeles: negativa por dentro
/// (la region y el hueco) y positiva por fuera. El punto va contado desde la esquina de arriba
/// a la izquierda de la region.
fn distancia(forma: &Forma, x: f32, y: f32) -> f32 {
    let [x1, y1, x2, y2] = forma.contorno;
    let r = forma.radio_en(x, y);
    let qx = (x - (x1 + x2) / 2.0).abs() - (x2 - x1) / 2.0 + r;
    let qy = (y - (y1 + y2) / 2.0).abs() - (y2 - y1) / 2.0 + r;
    if r <= 0.0 {
        // Una esquina en angulo recto lo es tambien por fuera de la linea: con la distancia de
        // siempre, la linea doblaria la esquina redonda y dejaria su pico sin pintar.
        return qx.max(qy);
    }
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

/// Cuanto de un pixel es linea, de 0 a 1. El pixel va contado desde la esquina de arriba a la
/// izquierda de la region.
///
/// En los tramos rectos, el hueco y la linea son pixeles enteros y sus bordes caen justo
/// entre dos pixeles: la linea sale nitida, sin medio pixel borroso a ningun lado. En las
/// esquinas redondas cada pixel se mira en 4x4 puntos y la curva sale suave, sin escalones.
fn cubre_la_linea(forma: &Forma, x: f32, y: f32) -> f32 {
    let linea = forma.medidas.linea as f32;
    let [x1, y1, x2, y2] = forma.contorno;
    let r = forma.radio_en(x + 0.5, y + 0.5);
    // Cerca de una esquina redonda: donde empieza su curva, con un pixel de margen.
    let en_curva = r > 0.0
        && (x + 0.5 < x1 + r + 1.0 || x + 0.5 > x2 - r - 1.0)
        && (y + 0.5 < y1 + r + 1.0 || y + 0.5 > y2 - r - 1.0);
    if !en_curva {
        // Tramo recto: el centro del pixel decide, y la linea sale de pixeles enteros.
        let d = distancia(forma, x + 0.5, y + 0.5);
        return if (0.0..=linea).contains(&d) { 1.0 } else { 0.0 };
    }
    let mut dentro = 0u32;
    for my in 0..4 {
        for mx in 0..4 {
            let (sx, sy) = (x + (mx as f32 + 0.5) / 4.0, y + (my as f32 + 0.5) / 4.0);
            if (0.0..=linea).contains(&distancia(forma, sx, sy)) {
                dentro += 1;
            }
        }
    }
    dentro as f32 / 16.0
}

/// Que pixeles de una tira son linea (medio pixel o mas), fila a fila. Es para quien no puede
/// pintar con transparencia y recorta la ventana a la forma de la linea (X11 sin compositor):
/// ahi no hay halo ni borde suave, solo la linea con sus esquinas.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn linea_de_tira(forma: &Forma, tira: Rect) -> Vec<bool> {
    let (dx, dy) = (tira.x - forma.region.x, tira.y - forma.region.y);
    let mut marcados = Vec::with_capacity(tira.width as usize * tira.height as usize);
    for fila in 0..tira.height as i32 {
        for col in 0..tira.width as i32 {
            marcados.push(cubre_la_linea(forma, (dx + col) as f32, (dy + fila) as f32) >= 0.5);
        }
    }
    marcados
}

/// El dibujo de una tira: cada pixel segun lo lejos que este del borde de dentro de la linea.
/// Sale en BGRA premultiplicado, que es lo que pide `UpdateLayeredWindow` en Windows y una
/// ventana de 32 bits en X11.
pub(crate) fn pintar_tira(forma: &Forma, tira: Rect, estilo: Estilo, pausado: bool) -> Vec<u8> {
    let linea = forma.medidas.linea as f32;
    let ancho_halo = forma.medidas.halo.max(1) as f32;
    let (color, halo) = (estilo.linea(pausado), estilo.halo(pausado));
    let (dx, dy) = (tira.x - forma.region.x, tira.y - forma.region.y);
    let mut bgra = Vec::with_capacity(tira.width as usize * tira.height as usize * 4);
    for fila in 0..tira.height as i32 {
        for col in 0..tira.width as i32 {
            let (x, y) = ((dx + col) as f32, (dy + fila) as f32);
            let d = distancia(forma, x + 0.5, y + 0.5);
            let cubre = cubre_la_linea(forma, x, y);
            // El halo solo va por fuera de la linea: el hueco de dentro queda limpio.
            let fuera = if d > linea / 2.0 {
                let t = ((d - linea) / ancho_halo).clamp(0.0, 1.0);
                halo.fuerza * (1.0 - t) * (1.0 - t)
            } else {
                0.0
            };
            // La linea encima del halo, premultiplicado.
            let alfa = cubre + fuera * (1.0 - cubre);
            let canal = |l: u8, h: u8| {
                (l as f32 * cubre + h as f32 * fuera * (1.0 - cubre)).round().min(255.0) as u8
            };
            bgra.extend_from_slice(&[
                canal(color.2, halo.color.2),
                canal(color.1, halo.color.1),
                canal(color.0, halo.color.0),
                (alfa * 255.0).round() as u8,
            ]);
        }
    }
    bgra
}

/// Que lados del marco van por dentro de la region (arriba, abajo, izquierda, derecha): los
/// que, por fuera, caerian aunque sea un pixel fuera de las pantallas que toca la region, en
/// la de al lado o en ninguna.
///
/// Una region que no toca ninguna pantalla (las pruebas, en -20000) lo lleva todo por fuera.
pub(crate) fn lados_por_dentro(region: Rect, grosor: i32, pantallas: &[Rect]) -> [bool; 4] {
    let suyas: Vec<Rect> = pantallas.iter().copied().filter(|p| comun(p, &region) > 0).collect();
    if suyas.is_empty() {
        return [false; 4];
    }
    let (x, y, w, h) = (region.x, region.y, region.width, region.height);
    let g = grosor.max(1);
    let por_fuera = [
        Rect { x, y: y - g, width: w, height: g as u32 },
        Rect { x, y: y + h as i32, width: w, height: g as u32 },
        Rect { x: x - g, y, width: g as u32, height: h },
        Rect { x: x + w as i32, y, width: g as u32, height: h },
    ];
    por_fuera.map(|tira| {
        let cubierto: i64 = suyas.iter().map(|p| comun(p, &tira)).sum();
        cubierto < i64::from(tira.width) * i64::from(tira.height)
    })
}

/// Los pixeles que tienen en comun dos rectangulos.
fn comun(a: &Rect, b: &Rect) -> i64 {
    let ancho = (a.x + a.width as i32).min(b.x + b.width as i32) - a.x.max(b.x);
    let alto = (a.y + a.height as i32).min(b.y + b.height as i32) - a.y.max(b.y);
    if ancho <= 0 || alto <= 0 { 0 } else { i64::from(ancho) * i64::from(alto) }
}

#[cfg(test)]
#[path = "marco_comun_pruebas.rs"]
mod tests;
