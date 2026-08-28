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
