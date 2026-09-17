//! Estirar un fotograma, rápido.
//!
//! El zoom recorta un trozo y hay que llevarlo al tamaño del vídeo, así que **cada
//! fotograma pasa por aquí**. Con `image::imageops::resize` eso costaba 64 ms por
//! fotograma, o sea casi dos minutos por un vídeo de un minuto, y en la compilación de
//! desarrollo cuarenta minutos: la barra de guardar parecía colgada.
//!
//! Medido el 27 de agosto de 2026 sobre 640x400 → 1280x800, en release:
//!
//! | qué | tiempo por fotograma |
//! |---|---:|
//! | `image` con Lanczos3 | 62 ms |
//! | `image` con Triangle | 53 ms |
//! | **esto** | **2 ms** |
//!
//! Y con `opt-level = "s"`, que es como se compila todo aquí: se probó a subirlo a 3 y el
//! instalador engordaba 360 KB para ganar un tercio, mientras que esto va igual de rápido
//! sin tocar el perfil. La velocidad estaba en el algoritmo, no en el compilador.
//!
//! Que `Nearest` costara lo mismo que `Lanczos3` fue la pista: el coste no está en el
//! filtro, está en el camino genérico que recorre `image` para cualquier tipo de píxel.
//! Aquí solo hay un caso (RGBA de 8 bits) y se puede ir derecho.
//!
//! **Es bilineal, y para ampliar eso es lo correcto.** Lanczos3 sirve para REDUCIR, donde
//! hay que promediar muchos píxeles en uno; ampliando, sus lóbulos negativos dejan halos
//! alrededor de los bordes con contraste. Para reducir se sigue usando el de `image`.

use image::RgbaImage;
use rayon::prelude::*;

use crate::record::delta::Parche;

/// El punto fijo con el que se recorre: 16 bits de parte entera y 16 de fracción.
///
/// Con flotantes hay una conversión por píxel y por canal, que a un millón de píxeles se
/// nota. Con enteros la interpolación son dos multiplicaciones y un desplazamiento.
const FRAC: u32 = 16;
const UNO: u32 = 1 << FRAC;

/// La tabla de una dimensión: para cada píxel de salida, de qué dos de entrada sale y con
/// cuánto peso. Se calcula una vez por eje en vez de una vez por píxel.
fn tabla(origen: u32, destino: u32) -> Vec<(u32, u32)> {
    let paso = ((origen as u64) << FRAC) / destino.max(1) as u64;
    (0..destino)
        .map(|i| {
            // El centro del píxel de salida, llevado al sistema de la entrada. El medio
            // píxel es lo que evita que la imagen se desplace media muestra al ampliar.
            let centro = ((i as u64 * 2 + 1) * paso / 2).saturating_sub(UNO as u64 / 2);
            let entero = (centro >> FRAC) as u32;
            let entero = entero.min(origen.saturating_sub(1));
            ((centro & (UNO as u64 - 1)) as u32, entero)
        })
        .map(|(frac, entero)| (entero, frac))
        .collect()
}

/// Estira la imagen al tamaño pedido, con interpolación bilineal.
///
/// Si ya mide lo pedido devuelve una copia sin tocar nada: es el caso normal cuando no hay
/// zoom, y no tiene sentido pagar una interpolación para no cambiar nada.
pub fn ampliar(origen: &RgbaImage, ancho: u32, alto: u32) -> RgbaImage {
    if origen.dimensions() == (ancho, alto) {
        return origen.clone();
    }
    let (ow, oh) = origen.dimensions();
    if ow == 0 || oh == 0 || ancho == 0 || alto == 0 {
        return RgbaImage::new(ancho.max(1), alto.max(1));
    }

    let columnas = tabla(ow, ancho);
    let filas = tabla(oh, alto);
    let dentro = origen.as_raw();
    let mut salida = vec![0u8; (ancho as usize) * (alto as usize) * 4];

    // Cada fila de salida solo lee la entrada y escribe su propio trozo, así que se
    // reparten entre los núcleos sin coordinar nada. Es donde está el resto del tiempo.
    salida
        .par_chunks_mut(ancho as usize * 4)
        .zip(filas.par_iter())
        .for_each(|(fila_salida, &(fy, pesoy))| {
            let fy2 = (fy + 1).min(oh - 1);
            let arriba = fy as usize * ow as usize * 4;
            let abajo = fy2 as usize * ow as usize * 4;
            for (x, &(fx, pesox)) in columnas.iter().enumerate() {
                let fx2 = (fx + 1).min(ow - 1);
                let (a, b) = (arriba + fx as usize * 4, arriba + fx2 as usize * 4);
                let (c, d) = (abajo + fx as usize * 4, abajo + fx2 as usize * 4);
                let salida_i = x * 4;
                for canal in 0..4 {
                    // Primero se mezclan en horizontal las dos filas, después entre ellas.
                    let ab = mezcla(dentro[a + canal], dentro[b + canal], pesox);
                    let cd = mezcla(dentro[c + canal], dentro[d + canal], pesox);
                    fila_salida[salida_i + canal] = mezcla(ab, cd, pesoy);
                }
            }
        });
    RgbaImage::from_raw(ancho, alto, salida).unwrap_or_else(|| RgbaImage::new(ancho, alto))
}

/// Mezcla dos valores con un peso en punto fijo. `peso` a cero devuelve el primero.
#[inline(always)]
fn mezcla(uno: u8, otro: u8, peso: u32) -> u8 {
    let a = uno as u32 * (UNO - peso);
    let b = otro as u32 * peso;
    ((a + b) >> FRAC) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rampa(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_fn(ancho, alto, |x, y| {
            image::Rgba([(x * 4) as u8, (y * 4) as u8, 128, 255])
        })
    }

    #[test]
    fn el_mismo_tamanno_no_cambia_ni_un_pixel() {
        // Es el caso de siempre: sin zoom, el fotograma ya mide lo que se pide.
        let uno = rampa(40, 30);
        let otro = ampliar(&uno, 40, 30);
        assert_eq!(uno.as_raw(), otro.as_raw());
    }

    #[test]
    fn sale_del_tamanno_pedido() {
        assert_eq!(ampliar(&rampa(40, 30), 120, 90).dimensions(), (120, 90));
        assert_eq!(ampliar(&rampa(40, 30), 20, 15).dimensions(), (20, 15));
    }

    #[test]
    fn las_esquinas_se_quedan_donde_estaban() {
        // Si la imagen se desplazara medio pixel, el zoom se notaria temblando.
        let uno = rampa(40, 30);
        let doble = ampliar(&uno, 80, 60);
        assert_eq!(doble.get_pixel(0, 0), uno.get_pixel(0, 0));
        assert_eq!(doble.get_pixel(79, 59), uno.get_pixel(39, 29));
    }

    #[test]
    fn un_color_plano_sigue_plano() {
        // Cualquier error en los pesos se ve aqui de inmediato: un borde o una raya.
        let plano = RgbaImage::from_pixel(20, 20, image::Rgba([90, 140, 200, 255]));
        let grande = ampliar(&plano, 100, 100);
        assert!(
            grande.pixels().all(|p| p.0 == [90, 140, 200, 255]),
            "un color plano ha salido con manchas"
        );
    }

    #[test]
    fn interpola_de_verdad_en_vez_de_repetir_pixeles() {
        // Con vecino mas cercano saldrian escalones; con bilineal, valores intermedios.
        let mut uno = RgbaImage::from_pixel(2, 1, image::Rgba([0, 0, 0, 255]));
        uno.put_pixel(1, 0, image::Rgba([200, 200, 200, 255]));
        let ancho = ampliar(&uno, 8, 1);
        let medios = (0..8)
            .map(|x| ancho.get_pixel(x, 0).0[0])
            .filter(|v| *v > 10 && *v < 190)
            .count();
        assert!(medios >= 2, "no ha interpolado: {medios} valores intermedios");
    }

    #[test]
    fn se_parece_a_lo_que_hace_image() {
        // No tiene que dar lo mismo bit a bit (son filtros distintos), pero si la misma
        // imagen: si se pareciera poco, es que las coordenadas estan mal.
        let uno = rampa(64, 48);
        let mio = ampliar(&uno, 128, 96);
        let suyo = image::imageops::resize(&uno, 128, 96, image::imageops::FilterType::Triangle);
        let peor = mio
            .pixels()
            .zip(suyo.pixels())
            .map(|(a, b)| {
                (0..3)
                    .map(|i| (a.0[i] as i32 - b.0[i] as i32).abs())
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0);
        assert!(peor <= 8, "se aleja demasiado de image: {peor} de diferencia");
    }

    #[test]
    fn una_imagen_de_un_pixel_no_revienta() {
        let uno = RgbaImage::from_pixel(1, 1, image::Rgba([10, 20, 30, 255]));
        let grande = ampliar(&uno, 50, 50);
        assert_eq!(grande.dimensions(), (50, 50));
        assert_eq!(*grande.get_pixel(25, 25), image::Rgba([10, 20, 30, 255]));
    }

    /// Lo que cuesta, comparado con lo que se usaba antes.
    #[test]
    #[ignore]
    fn medir_lo_que_cuesta_escalar() {
        use std::time::Instant;
        let trozo = rampa(640, 400);
        let veces = 20;

        let t = Instant::now();
        for _ in 0..veces {
            std::hint::black_box(ampliar(&trozo, 1280, 800));
        }
        let mio = t.elapsed().as_secs_f64() * 1000.0 / veces as f64;

        let t = Instant::now();
        for _ in 0..veces {
            std::hint::black_box(image::imageops::resize(
                &trozo,
                1280,
                800,
                image::imageops::FilterType::Lanczos3,
            ));
        }
        let suyo = t.elapsed().as_secs_f64() * 1000.0 / veces as f64;

        eprintln!("[escalar] 640x400 -> 1280x800: esto {mio:.1} ms, image/Lanczos3 {suyo:.1} ms");
    }
}

/// Reduce la imagen promediando, para cuando el destino es MÁS PEQUEÑO que el origen.
///
/// `ampliar` interpola entre dos píxeles, que es lo correcto al estirar y lo incorrecto al
/// encoger: al reducir a la mitad se mira uno de cada dos y el resultado sale con dientes
/// de sierra. Aquí cada píxel de salida es la media de todos los de su rectángulo, que es
/// lo mismo que hace un filtro de caja.
///
/// Existe porque el anillo de los últimos segundos puede grabar a 720p desde una pantalla
/// de 1080p, y eso son quince reducciones por segundo durante horas: `image::imageops`
/// tarda 53 ms en una de estas y aquí no se puede pagar eso ni una vez.
pub fn reducir(origen: &RgbaImage, ancho: u32, alto: u32) -> RgbaImage {
    let (ow, oh) = origen.dimensions();
    if (ow, oh) == (ancho, alto) || ancho == 0 || alto == 0 || ow == 0 || oh == 0 {
        return origen.clone();
    }
    let reduccion = Reduccion::nueva(ow, oh, ancho, alto);
    let mut salida = vec![0u8; (ancho as usize) * (alto as usize) * 4];
    reduccion.reducir_zona(origen.as_raw(), reduccion.entera(), &mut salida);
    RgbaImage::from_raw(ancho, alto, salida).unwrap_or_else(|| RgbaImage::new(ancho, alto))
}

/// Una reduccion preparada: las tablas de reparto de los dos ejes, calculadas una vez.
///
/// El anillo de los ultimos segundos reduce el mismo tamanno al mismo destino toda la
/// tarde, y en cada fotograma casi nunca cambia mas que el cursor. Con esto se reduce
/// **solo la zona de salida a la que le afecta lo que cambio**, y se pega sobre el
/// fotograma reducido anterior: el resto de pixeles de salida dependen de pixeles de
/// entrada que no se han movido, asi que valen tal cual. Medido el 17 de septiembre de
/// 2026: reducir 1080p a 720p entero son 10-12 ms de CPU por fotograma; con un cursor
/// moviendose, la zona afectada son unos cientos de pixeles.
pub struct Reduccion {
    origen: (u32, u32),
    destino: (u32, u32),
    columnas: Vec<(u32, Vec<u32>)>,
    filas: Vec<(u32, Vec<u32>)>,
}

impl Reduccion {
    pub fn nueva(ow: u32, oh: u32, ancho: u32, alto: u32) -> Self {
        Self {
            origen: (ow, oh),
            destino: (ancho, alto),
            columnas: reparto(ow, ancho),
            filas: reparto(oh, alto),
        }
    }

    /// El fotograma de salida entero, como zona.
    pub fn entera(&self) -> Parche {
        Parche {
            x: 0,
            y: 0,
            width: self.destino.0,
            height: self.destino.1,
        }
    }

    /// La zona de SALIDA que depende de esa zona de ENTRADA: todo pixel de salida que
    /// tenga dentro de su rectangulo al menos un pixel de entrada que cambio.
    ///
    /// Es lo unico que hace correcto reducir por zonas: un pixel de salida que se queda
    /// fuera de esta zona solo mira pixeles de entrada iguales a los de antes, asi que su
    /// valor de antes sigue siendo el bueno.
    pub fn zona_afectada(&self, cambio: Parche) -> Parche {
        let (x1, x2) = tramo_afectado(&self.columnas, cambio.x, cambio.x + cambio.width);
        let (y1, y2) = tramo_afectado(&self.filas, cambio.y, cambio.y + cambio.height);
        Parche {
            x: x1,
            y: y1,
            width: x2.saturating_sub(x1),
            height: y2.saturating_sub(y1),
        }
    }

    /// Reduce solo esa zona de salida, leyendo del fotograma de entrada entero, y la
    /// escribe en su sitio de `salida`. `entrada` mide el origen y `salida` el destino,
    /// las dos en RGBA.
    pub fn reducir_zona(&self, entrada: &[u8], zona: Parche, salida: &mut [u8]) {
        let (ow, _) = self.origen;
        let (ancho, alto) = self.destino;
        let fila_out = ancho as usize * 4;
        if entrada.len() < (self.origen.0 as usize) * (self.origen.1 as usize) * 4
            || salida.len() < fila_out * alto as usize
        {
            return;
        }
        let x_desde = zona.x.min(ancho) as usize;
        let x_hasta = (zona.x + zona.width).min(ancho) as usize;
        let y_desde = zona.y.min(alto) as usize;
        let y_hasta = (zona.y + zona.height).min(alto) as usize;
        if x_desde >= x_hasta || y_desde >= y_hasta {
            return;
        }
        let columnas = &self.columnas[x_desde..x_hasta];
        let filas = &self.filas;

        salida
            .par_chunks_mut(fila_out)
            .enumerate()
            .skip(y_desde)
            .take(y_hasta - y_desde)
            .for_each(|(y, destino)| {
                let (y0, pesos_y) = &filas[y];
                for (x, (x0, pesos_x)) in (x_desde..).zip(columnas.iter()) {
                    let mut suma = [0u64; 4];
                    for (dy, py) in pesos_y.iter().enumerate() {
                        let fila = ((*y0 as usize + dy) * ow as usize + *x0 as usize) * 4;
                        for (dx, px) in pesos_x.iter().enumerate() {
                            let p = fila + dx * 4;
                            let peso = u64::from(*py) * u64::from(*px);
                            suma[0] += peso * u64::from(entrada[p]);
                            suma[1] += peso * u64::from(entrada[p + 1]);
                            suma[2] += peso * u64::from(entrada[p + 2]);
                            suma[3] += peso * u64::from(entrada[p + 3]);
                        }
                    }
                    let d = x * 4;
                    for canal in 0..4 {
                        // Los pesos de cada eje suman UNO, así que el total es UNO al cuadrado.
                        destino[d + canal] =
                            (((suma[canal] + (TOTAL / 2)) / TOTAL).min(255)) as u8;
                    }
                }
            });
    }
}

/// Que pixeles de salida (de un eje) tocan el tramo `[desde, hasta)` de entrada. Las
/// tablas van en orden, asi que basta con el primero que llega y el ultimo que empieza
/// antes del final.
fn tramo_afectado(tabla: &[(u32, Vec<u32>)], desde: u32, hasta: u32) -> (u32, u32) {
    let primero = tabla
        .iter()
        .position(|(inicio, pesos)| inicio + pesos.len() as u32 > desde)
        .unwrap_or(tabla.len());
    let ultimo = tabla
        .iter()
        .rposition(|(inicio, _)| *inicio < hasta)
        .map(|i| i + 1)
        .unwrap_or(0);
    (primero as u32, ultimo.max(primero) as u32)
}

/// Lo que suman los pesos de los dos ejes juntos.
const TOTAL: u64 = (UNO as u64) * (UNO as u64);

/// Encoge la imagen un numero entero de veces, promediando cada bloque de `factor` por
/// `factor` pixeles. Es exacto (un filtro de caja con bloques enteros) y muy barato,
/// porque cada pixel de entrada se lee una vez y las sumas caben en un entero corto.
///
/// Existe para las miniaturas: bajar de 1920x1080 a 142x80 con `reducir` directamente
/// cuesta unos diez milisegundos por fotograma, porque cada pixel de salida promedia
/// catorce por catorce con pesos fraccionarios. Encogiendo antes seis veces con esto, lo
/// que le llega a `reducir` son 320x180 y el total baja a un par de milisegundos. Al
/// parar una grabacion de un minuto eso es la diferencia entre esperar y no esperar.
///
/// Lo que sobra por la derecha y por abajo cuando el lado no es multiplo del factor se
/// descarta: como mucho `factor - 1` pixeles, que en una miniatura no los ve nadie.
pub fn prereducir(origen: &RgbaImage, factor: u32) -> RgbaImage {
    let (ow, oh) = origen.dimensions();
    if factor <= 1 || ow < factor || oh < factor {
        return origen.clone();
    }
    let (ancho, alto) = (ow / factor, oh / factor);
    let f = factor as usize;
    let entrada = origen.as_raw();
    let fila_in = ow as usize * 4;
    let cuantos = (f * f) as u32;
    let mitad = cuantos / 2;

    let mut salida = vec![0u8; (ancho as usize) * (alto as usize) * 4];
    salida
        .par_chunks_mut(ancho as usize * 4)
        .enumerate()
        .for_each(|(y, destino)| {
            // Primero se suman las `f` filas del bloque columna a columna, y despues se
            // agrupan las columnas de `f` en `f`: asi cada byte de entrada se lee una vez.
            let mut columnas = vec![0u32; ancho as usize * f * 4];
            for dy in 0..f {
                let fila = &entrada[(y * f + dy) * fila_in..(y * f + dy) * fila_in + columnas.len()];
                for (acumulado, byte) in columnas.iter_mut().zip(fila) {
                    *acumulado += u32::from(*byte);
                }
            }
            for (x, pixel) in destino.chunks_exact_mut(4).enumerate() {
                let mut suma = [0u32; 4];
                for bloque in columnas[x * f * 4..(x + 1) * f * 4].chunks_exact(4) {
                    for canal in 0..4 {
                        suma[canal] += bloque[canal];
                    }
                }
                for canal in 0..4 {
                    pixel[canal] = ((suma[canal] + mitad) / cuantos) as u8;
                }
            }
        });
    RgbaImage::from_raw(ancho, alto, salida).unwrap_or_else(|| RgbaImage::new(ancho, alto))
}

/// Una miniatura de ese alto, encogiendo primero a lo bruto y despues fino.
///
/// El factor entero se elige para que a `reducir` le llegue algo entre dos y cuatro veces
/// el tamanno final: mas grande y se pierde el ahorro, mas pequenno y el promedio fino ya
/// no tiene con que trabajar.
pub fn miniatura(origen: &RgbaImage, ancho: u32, alto: u32) -> RgbaImage {
    let (ow, oh) = origen.dimensions();
    let ratio = (ow / ancho.max(1)).min(oh / alto.max(1));
    let factor = (ratio / 2).max(1);
    if factor <= 1 {
        return reducir(origen, ancho, alto);
    }
    reducir(&prereducir(origen, factor), ancho, alto)
}

/// Para cada píxel de salida, desde qué píxel del origen empieza y con cuánto entra cada
/// uno. Se calcula una vez por eje y vale para todas las filas.
///
/// **Los límites son fraccionarios a propósito.** Con límites enteros, bajar de 1890 a 1280
/// promediaba unas veces un píxel y otras dos, según dónde cayera la división: una reja de
/// un píxel salía a rayas gordas y el texto de una captura con unas letras más finas que
/// otras. Aquí cada píxel de entrada entra por la parte que solapa, que es lo que hace un
/// filtro de área de verdad.
fn reparto(origen: u32, destino: u32) -> Vec<(u32, Vec<u32>)> {
    let escala = f64::from(origen) / f64::from(destino.max(1));
    (0..destino)
        .map(|i| {
            let a = f64::from(i) * escala;
            let b = (a + escala).min(f64::from(origen));
            let primero = (a.floor() as u32).min(origen.saturating_sub(1));
            let ultimo = (b.ceil() as u32).clamp(primero + 1, origen);
            let mut pesos: Vec<u32> = (primero..ultimo)
                .map(|j| {
                    let solapa = (b.min(f64::from(j) + 1.0) - a.max(f64::from(j))).max(0.0);
                    (solapa / (b - a).max(f64::EPSILON) * f64::from(UNO)).round() as u32
                })
                .collect();
            // Que sumen exactamente UNO: el redondeo se lo lleva el peso más gordo, que es
            // donde menos se nota. Sin esto la imagen se iría aclarando u oscureciendo.
            let suma: i64 = pesos.iter().map(|w| i64::from(*w)).sum();
            if let Some(mayor) = pesos.iter_mut().max_by_key(|w| **w) {
                *mayor = (i64::from(*mayor) + i64::from(UNO) - suma).max(0) as u32;
            }
            (primero, pesos)
        })
        .collect()
}

/// Lleva la imagen al tamaño pedido por el camino que toque, sin pasar nunca por `image`.
///
/// Es la puerta única de todo lo que escala un fotograma: ampliar interpola, reducir
/// promedia por área, y si un lado crece mientras el otro encoge se hace en dos pasos.
///
/// Existe porque exportar hacía esto mismo con `image::imageops::resize`, que en un
/// fotograma de 1890x1052 cuesta **64 ms**; aquí son **2 ms**. Munir exportó una grabación
/// de 50 segundos a 1280 y tardó dos minutos: 64 de esos 92 ms por fotograma eran esta
/// línea. Medido el 29 de agosto de 2026.
pub fn a_medida(origen: &RgbaImage, ancho: u32, alto: u32) -> RgbaImage {
    let (ow, oh) = origen.dimensions();
    if (ow, oh) == (ancho, alto) {
        return origen.clone();
    }
    if ancho >= ow && alto >= oh {
        return ampliar(origen, ancho, alto);
    }
    if ancho <= ow && alto <= oh {
        return reducir(origen, ancho, alto);
    }
    // Un lado crece y el otro encoge, que es lo que pasa al cambiar la proporción. Primero
    // se encoge, que deja menos píxeles que estirar después.
    let intermedio = reducir(origen, ancho.min(ow), alto.min(oh));
    ampliar(&intermedio, ancho, alto)
}

/// El tamaño al que se graba, dado el alto que se pidió.
///
/// Devuelve `None` cuando no hay que tocar nada: el alto pedido es cero (nativo) o la
/// pantalla ya mide eso o menos. Los dos lados salen PARES porque H.264 no acepta un lado
/// impar, y enterarse de eso en el codificador es enterarse tarde.
pub fn medida_para(ancho: u32, alto: u32, alto_pedido: u32) -> Option<(u32, u32)> {
    if alto_pedido == 0 || alto == 0 || alto_pedido >= alto {
        return None;
    }
    let nuevo_ancho = ((u64::from(ancho) * u64::from(alto_pedido) / u64::from(alto)) as u32).max(2);
    Some((nuevo_ancho / 2 * 2, alto_pedido / 2 * 2))
}

#[cfg(test)]
mod reducir_tests {
    use super::*;

    /// Un color plano reducido sigue siendo ese color: si el promedio estuviera mal, se
    /// vería aquí antes que en ningún sitio.
    #[test]
    fn un_color_plano_sobrevive_a_la_reduccion() {
        let origen = RgbaImage::from_pixel(1920, 1080, image::Rgba([40, 90, 200, 255]));
        let pequenna = reducir(&origen, 1280, 720);
        assert_eq!(pequenna.dimensions(), (1280, 720));
        assert_eq!(pequenna.get_pixel(640, 360).0, [40, 90, 200, 255]);
        assert_eq!(pequenna.get_pixel(0, 0).0, [40, 90, 200, 255]);
    }

    /// Y lo que de verdad separa promediar de saltarse píxeles: un tablero de ajedrez de
    /// un píxel reducido a la mitad tiene que salir GRIS, no blanco ni negro.
    #[test]
    fn promedia_de_verdad_en_vez_de_saltarse_pixeles() {
        let mut origen = RgbaImage::new(200, 200);
        for (x, y, pixel) in origen.enumerate_pixels_mut() {
            let claro = (x + y) % 2 == 0;
            *pixel = image::Rgba(if claro { [255, 255, 255, 255] } else { [0, 0, 0, 255] });
        }
        let media = reducir(&origen, 100, 100);
        let centro = media.get_pixel(50, 50).0;
        assert!(
            (120..=135).contains(&centro[0]),
            "un ajedrez reducido tiene que salir gris, y salió {centro:?}"
        );
    }

    /// Reducir 1890 a 1280 no es quedarse unas veces con un pixel y otras con dos.
    ///
    /// Munir grababa a 1890x1052 y exportaba a 1280: con los limites en numeros enteros,
    /// cada pixel de salida promediaba **uno o dos** de entrada segun donde cayera, asi que
    /// una reja de un pixel salia a rayas gordas y el texto de la captura con unas letras
    /// mas finas que otras. Con el reparto por area cada pixel de salida se lleva su parte
    /// proporcional, y la reja sale gris pareja.
    ///
    /// Los numeros: un pixel de salida cubre 1,477 de entrada, asi que como mucho puede
    /// irse a 172 o a 82, o sea 45 del gris. Antes se iba a 255 y a 0, que son 128.
    #[test]
    fn una_reja_fina_no_sale_a_rayas_gordas() {
        let mut origen = RgbaImage::new(1890, 8);
        for (x, _y, pixel) in origen.enumerate_pixels_mut() {
            let v = if x % 2 == 0 { 255 } else { 0 };
            *pixel = image::Rgba([v, v, v, 255]);
        }
        let pequenna = reducir(&origen, 1280, 8);
        let peor = (0..1280)
            .map(|x| (i32::from(pequenna.get_pixel(x, 4).0[0]) - 128).abs())
            .max()
            .expect("hay columnas");
        assert!(
            peor <= 50,
            "la reja sale a rayas: el pixel mas lejos del gris se va {peor}"
        );
    }

    /// El caso raro: un lado crece y el otro encoge. Pasa al cambiar la proporción en el
    /// panel, y antes lo resolvía `image`; ahora hay que atenderlo aquí.
    #[test]
    fn a_medida_atiende_los_tres_caminos() {
        let origen = RgbaImage::from_pixel(400, 300, image::Rgba([10, 20, 30, 255]));
        assert_eq!(a_medida(&origen, 400, 300).dimensions(), (400, 300));
        assert_eq!(a_medida(&origen, 800, 600).dimensions(), (800, 600));
        assert_eq!(a_medida(&origen, 200, 150).dimensions(), (200, 150));
        // Ancho abajo, alto arriba: el que se le atragantaba al if de antes.
        let mixta = a_medida(&origen, 200, 600);
        assert_eq!(mixta.dimensions(), (200, 600));
        assert_eq!(mixta.get_pixel(100, 300).0, [10, 20, 30, 255]);
    }

    /// Reducir solo la zona afectada y pegarla da EXACTAMENTE lo mismo que reducir el
    /// fotograma entero. Es lo que permite que el anillo reducido cueste segun lo que se
    /// mueve y no segun lo que mide la pantalla.
    #[test]
    fn reducir_por_zonas_da_lo_mismo_que_entero() {
        let mut origen = RgbaImage::new(640, 360);
        let mut s: u32 = 0x1234_5678;
        for pixel in origen.pixels_mut() {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *pixel = image::Rgba([(s >> 24) as u8, (s >> 16) as u8, (s >> 8) as u8, 255]);
        }
        let reduccion = Reduccion::nueva(640, 360, 427, 240);
        let mut escalado = vec![0u8; 427 * 240 * 4];
        reduccion.reducir_zona(origen.as_raw(), reduccion.entera(), &mut escalado);
        assert_eq!(escalado, reducir(&origen, 427, 240).into_raw());

        // Cambia un cuadrado de la entrada (un cursor), en tres sitios distintos, incluido
        // el borde de abajo a la derecha que es donde las tablas se acaban.
        for (cx, cy) in [(100u32, 50u32), (0, 0), (640 - 20, 360 - 20)] {
            for y in cy..cy + 20 {
                for x in cx..cx + 20 {
                    origen.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
                }
            }
            let cambio = Parche { x: cx, y: cy, width: 20, height: 20 };
            let zona = reduccion.zona_afectada(cambio);
            assert!(zona.width > 0 && zona.height > 0, "la zona afectada no puede estar vacia");
            reduccion.reducir_zona(origen.as_raw(), zona, &mut escalado);
            assert_eq!(
                escalado,
                reducir(&origen, 427, 240).into_raw(),
                "tras cambiar en ({cx}, {cy}) el fotograma por zonas no coincide con el entero"
            );
        }
    }

    /// La zona afectada cubre todos los pixeles de salida que miran al cambio, y ninguno
    /// mas de la cuenta: con reduccion 2:1, cambiar el pixel 10 afecta al pixel 5 y al 4
    /// no; cambiar del 10 al 13 afecta al 5 y al 6.
    #[test]
    fn la_zona_afectada_cubre_justo_lo_que_mira_al_cambio() {
        let reduccion = Reduccion::nueva(100, 100, 50, 50);
        let zona = reduccion.zona_afectada(Parche { x: 10, y: 10, width: 1, height: 1 });
        assert_eq!((zona.x, zona.y, zona.width, zona.height), (5, 5, 1, 1));
        let zona = reduccion.zona_afectada(Parche { x: 10, y: 20, width: 4, height: 2 });
        assert_eq!((zona.x, zona.y, zona.width, zona.height), (5, 10, 2, 1));
        // Un cambio que se sale por el final se queda en el ultimo pixel de salida.
        let zona = reduccion.zona_afectada(Parche { x: 98, y: 98, width: 10, height: 10 });
        assert_eq!((zona.x, zona.y, zona.width, zona.height), (49, 49, 1, 1));
        // Y el fotograma entero afecta al fotograma entero.
        let zona = reduccion.zona_afectada(Parche { x: 0, y: 0, width: 100, height: 100 });
        assert_eq!((zona.x, zona.y, zona.width, zona.height), (0, 0, 50, 50));
    }

    #[test]
    fn la_medida_sale_par_y_respeta_la_proporcion() {
        assert_eq!(medida_para(1920, 1080, 720), Some((1280, 720)));
        // 2560x1440 a 720 son 1280x720 justos; 1366x768 a 720 sale impar y se corta.
        assert_eq!(medida_para(1366, 768, 720), Some((1280, 720)));
        // Nada que hacer: se pidió lo nativo, o la pantalla ya es más pequeña.
        assert_eq!(medida_para(1920, 1080, 0), None);
        assert_eq!(medida_para(1280, 720, 1080), None);
    }
}

#[cfg(test)]
mod medir_reduccion {
    use super::*;
    use std::time::Instant;

    /// Lo que cuesta bajar una pantalla de 1080p a 720p, que es lo que hace el anillo de
    /// los ultimos segundos quince veces por segundo durante horas.
    ///
    /// `cargo test --release --lib medir_reducir -- --ignored --nocapture`
    #[test]
    #[ignore = "es una medicion, no una comprobacion"]
    fn medir_reducir() {
        let ruta = std::env::temp_dir().join("winshotx-replay-pantalla/ultimo.png");
        let origen = match image::open(&ruta) {
            Ok(imagen) => imagen.to_rgba8(),
            Err(_) => RgbaImage::from_pixel(1920, 1080, image::Rgba([90, 120, 200, 255])),
        };
        println!("desde {}x{}", origen.width(), origen.height());

        for alto in [1080u32, 720] {
            let Some((w, h)) = medida_para(origen.width(), origen.height(), alto) else {
                println!("{alto}p: nada que hacer, ya mide eso o menos");
                continue;
            };
            let ahora = Instant::now();
            let pequenna = reducir(&origen, w, h);
            let propio = ahora.elapsed().as_micros();

            let ahora = Instant::now();
            let _ = image::imageops::resize(&origen, w, h, image::imageops::FilterType::Triangle);
            let del_crate = ahora.elapsed().as_micros();

            let qoi = qoi::encode_to_vec(pequenna.as_raw(), w, h).unwrap();
            println!(
                "{alto}p ({w}x{h}): esto {} ms · image {} ms · el fotograma ocupa {} KB",
                propio / 1000,
                del_crate / 1000,
                qoi.len() / 1024
            );
        }
    }

    #[test]
    fn prereducir_promedia_bloques_enteros() {
        // Bloques de 2x2 con valores 0, 0, 100, 100 -> media 50; y un canal distinto.
        let mut img = RgbaImage::from_pixel(4, 2, image::Rgba([0, 10, 200, 255]));
        for x in 0..4 {
            img.put_pixel(x, 1, image::Rgba([100, 10, 200, 255]));
        }
        let chica = prereducir(&img, 2);
        assert_eq!(chica.dimensions(), (2, 1));
        assert_eq!(*chica.get_pixel(0, 0), image::Rgba([50, 10, 200, 255]));
        assert_eq!(*chica.get_pixel(1, 0), image::Rgba([50, 10, 200, 255]));
    }

    #[test]
    fn prereducir_descarta_lo_que_no_llena_un_bloque() {
        let img = RgbaImage::from_fn(11, 7, |x, y| image::Rgba([x as u8, y as u8, 7, 255]));
        assert_eq!(prereducir(&img, 3).dimensions(), (3, 2));
        // Factor uno o imagen mas pequenna que el bloque: se devuelve tal cual.
        assert_eq!(prereducir(&img, 1).dimensions(), (11, 7));
        assert_eq!(prereducir(&img, 20).dimensions(), (11, 7));
    }

    /// Una miniatura hecha en dos pasos tiene que parecerse a la que sale de un solo paso
    /// fino: si se alejara, es que el encogido bruto esta mal alineado.
    #[test]
    fn la_miniatura_en_dos_pasos_se_parece_a_la_de_uno() {
        let grande = RgbaImage::from_fn(640, 360, |x, y| {
            image::Rgba([(x / 3) as u8, (y / 2) as u8, ((x + y) / 5) as u8, 255])
        });
        let fina = reducir(&grande, 142, 80);
        let rapida = miniatura(&grande, 142, 80);
        assert_eq!(rapida.dimensions(), (142, 80));
        let peor = fina
            .pixels()
            .zip(rapida.pixels())
            .map(|(a, b)| (0..3).map(|i| (a.0[i] as i32 - b.0[i] as i32).abs()).max().unwrap_or(0))
            .max()
            .unwrap_or(0);
        assert!(peor <= 6, "la miniatura rapida se aleja {peor} niveles de la fina");
    }
}
