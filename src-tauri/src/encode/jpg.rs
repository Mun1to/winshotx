use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbImage, RgbaImage};

use crate::error::Result;

/// El blanco sobre el que se apoya lo transparente.
///
/// JPEG no guarda transparencia, asi que hay que decidir que hay detras. Los huecos de una
/// captura salen de dos sitios y los dos piden blanco: los espacios entre monitores
/// desalineados, y las esquinas redondeadas de una ventana. Dejarlos en negro seria
/// pintar un marco negro alrededor de cada ventana capturada.
const DETRAS: [u8; 3] = [255, 255, 255];

/// Quita el alfa apoyando la imagen sobre blanco.
///
/// Se mezcla en vez de tirar el canal: un pixel a medio camino (el borde suavizado de una
/// esquina redondeada) con el alfa tirado saldria del color pleno y dejaria un diente de
/// sierra donde antes habia una curva limpia.
fn sobre_blanco(image: &RgbaImage) -> RgbImage {
    RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let p = image.get_pixel(x, y).0;
        let a = p[3] as u32;
        image::Rgb([
            ((p[0] as u32 * a + DETRAS[0] as u32 * (255 - a)) / 255) as u8,
            ((p[1] as u32 * a + DETRAS[1] as u32 * (255 - a)) / 255) as u8,
            ((p[2] as u32 * a + DETRAS[2] as u32 * (255 - a)) / 255) as u8,
        ])
    })
}

/// Guarda el recorte como JPEG, escalandolo antes si el usuario cambio las dimensiones.
///
/// La calidad es la misma barra que ya gobierna el GIF y el MP4, para no inventar un
/// numero mas. De 1 a 100, donde 100 no es «sin perdida»: JPEG siempre pierde algo.
pub fn save(image: &RgbaImage, path: &Path, width: u32, height: u32, calidad: u8) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let escalada;
    let fuente = if image.width() == width && image.height() == height {
        image
    } else {
        escalada = image::imageops::resize(
            image,
            width.max(1),
            height.max(1),
            image::imageops::FilterType::Lanczos3,
        );
        &escalada
    };
    let plana = sobre_blanco(fuente);
    let mut salida = Vec::new();
    JpegEncoder::new_with_quality(&mut salida, calidad.clamp(1, 100)).write_image(
        plana.as_raw(),
        plana.width(),
        plana.height(),
        ExtendedColorType::Rgb8,
    )?;
    std::fs::write(path, salida)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imagen(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_fn(ancho, alto, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
        })
    }

    fn temporal(nombre: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("winshotx-test-jpg");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(nombre)
    }

    #[test]
    fn el_archivo_guardado_se_vuelve_a_abrir_y_mide_lo_pedido() {
        // La leccion del audio: comprobar el archivo que se lleva el usuario, no solo que
        // la funcion devolvio Ok.
        let destino = temporal("normal.jpg");
        save(&imagen(320, 200), &destino, 320, 200, 85).unwrap();
        let releido = image::open(&destino).expect("el JPEG guardado no se puede abrir");
        assert_eq!((releido.width(), releido.height()), (320, 200));
    }

    #[test]
    fn escalar_deja_el_archivo_del_tamanno_pedido() {
        let destino = temporal("escalado.jpg");
        save(&imagen(400, 300), &destino, 200, 150, 85).unwrap();
        let releido = image::open(&destino).unwrap();
        assert_eq!((releido.width(), releido.height()), (200, 150));
    }

    #[test]
    fn lo_transparente_sale_blanco_y_no_negro() {
        // Es el caso de la captura de todas las pantallas: los huecos entre monitores
        // desalineados van transparentes, y en JPEG hay que decidir que hay detras.
        let mut con_hueco = imagen(40, 40);
        for x in 0..10 {
            for y in 0..10 {
                con_hueco.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            }
        }
        let destino = temporal("hueco.jpg");
        save(&con_hueco, &destino, 40, 40, 95).unwrap();
        let releido = image::open(&destino).unwrap().to_rgb8();
        let p = releido.get_pixel(4, 4).0;
        assert!(
            p[0] > 240 && p[1] > 240 && p[2] > 240,
            "el hueco transparente ha salido {p:?} en vez de blanco"
        );
    }

    #[test]
    fn un_borde_a_medio_alfa_se_mezcla_en_vez_de_saltar() {
        // Tirar el canal alfa en vez de mezclarlo dejaria este pixel del color pleno, y
        // con el un diente de sierra en cada esquina redondeada de ventana.
        let mut medio = RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 128]));
        medio.put_pixel(0, 0, image::Rgba([0, 0, 0, 128]));
        let plana = sobre_blanco(&medio);
        let p = plana.get_pixel(4, 4).0;
        assert!(
            (120..=135).contains(&p[0]),
            "medio alfa sobre blanco tendria que quedar a media luz, y ha quedado {p:?}"
        );
    }

    #[test]
    fn menos_calidad_pesa_menos() {
        // Es la razon entera de que exista este formato: una captura que hay que mandar
        // por correo. Si la barra de calidad no cambiara el peso, no serviria de nada.
        let alta = temporal("alta.jpg");
        let baja = temporal("baja.jpg");
        let fuente = imagen(600, 400);
        save(&fuente, &alta, 600, 400, 95).unwrap();
        save(&fuente, &baja, 600, 400, 30).unwrap();
        let (a, b) = (
            std::fs::metadata(&alta).unwrap().len(),
            std::fs::metadata(&baja).unwrap().len(),
        );
        assert!(b < a, "calidad 30 ({b} bytes) no pesa menos que calidad 95 ({a} bytes)");
    }

    #[test]
    fn una_calidad_imposible_no_revienta_el_codificador() {
        // El valor llega del frontend y `JpegEncoder` entra en panico con un 0.
        let destino = temporal("cero.jpg");
        save(&imagen(20, 20), &destino, 20, 20, 0).unwrap();
        assert!(image::open(&destino).is_ok());
    }
}
