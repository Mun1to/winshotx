use std::path::Path;

use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, RgbaImage};

use crate::error::Result;

/// El filtro que usa el codificador PNG, y es lo unico que de verdad costaba tiempo aqui.
///
/// El de fabrica es `Adaptive`, que prueba los cinco filtros en cada fila y se queda con
/// el que mejor comprime. Medido el 26 de agosto de 2026 en release, sobre una imagen de
/// 1920x1200 parecida a una captura:
///
/// | ajuste             | tiempo | tamano  |
/// |--------------------|--------|---------|
/// | Default + Adaptive | 182 ms |  177 KB |
/// | Default + Up       |  64 ms |  169 KB |
/// | Fast    + Up       |  28 ms | 3111 KB |
///
/// `Up` sale mas rapido **y** mas pequeno que `Adaptive`: no hay nada que sopesar. Tiene
/// sentido con capturas de pantalla, donde una fila se parece muchisimo a la de arriba.
const FILTRO: PngFilter = PngFilter::Up;

fn codificar(image: &RgbaImage, compresion: CompressionType) -> Result<Vec<u8>> {
    let mut salida = Vec::new();
    PngEncoder::new_with_quality(&mut salida, compresion, FILTRO).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgba8,
    )?;
    Ok(salida)
}

/// Guarda el recorte como PNG, escalandolo antes si el usuario cambio las dimensiones.
pub fn save(image: &RgbaImage, path: &Path, width: u32, height: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // En disco el tamano si importa, asi que se comprime de verdad. Con `Up` eso cuesta
    // 64 ms en vez de los 182 que costaba, y encima el archivo sale mas pequeno.
    let bytes = if image.width() == width && image.height() == height {
        codificar(image, CompressionType::Default)?
    } else {
        let scaled = image::imageops::resize(
            image,
            width.max(1),
            height.max(1),
            image::imageops::FilterType::Lanczos3,
        );
        codificar(&scaled, CompressionType::Default)?
    };
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Guarda un PNG comprimido lo minimo, para archivos que se miran una vez y se tiran.
///
/// Es el fotograma grande que el editor pide al saltar con el raton por la tira cuando
/// no hay video de vista previa. Cada salto es un archivo nuevo, asi que lo que importa
/// es que salga ya: 28 ms contra los 180 de la compresion de fabrica a 1920x1200.
pub fn save_fast(image: &RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, codificar(image, CompressionType::Fast)?)?;
    Ok(())
}

/// Bytes PNG en memoria: es lo que se mete en el portapapeles.
///
/// Aqui se comprime lo minimo (`Fast`), y no es una concesion: al portapapeles el tamano
/// le da igual porque nadie lo guarda, se pega y se olvida. Ademas `copy_image` mete al
/// lado un CF_DIB **sin comprimir** de 9 MB para las aplicaciones que no entienden PNG,
/// asi que ahorrar bytes en esta mitad no ahorra nada. Copiar una pantalla entera pasa de
/// 182 ms a 28.
pub fn to_bytes(image: &RgbaImage) -> Result<Vec<u8>> {
    codificar(image, CompressionType::Fast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Algo parecido a una captura de verdad: fondo liso, franjas y ruido suave. Un color
    /// plano comprimiria al instante y daria un numero que no significa nada.
    fn captura_falsa(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_fn(ancho, alto, |x, y| {
            let franja = (y / 28) % 2 == 0;
            let base = if franja { 246 } else { 255 };
            let ventana = x > ancho / 6 && x < ancho * 5 / 6 && y > alto / 8 && y < alto * 7 / 8;
            let texto = ventana && (x / 3 + y / 11) % 17 == 0;
            let ruido = (x
                .wrapping_mul(2654435761)
                .wrapping_add(y.wrapping_mul(40503))
                % 7) as u8;
            if texto {
                image::Rgba([32, 34, 38, 255])
            } else if ventana {
                image::Rgba([base - ruido, base - ruido, base, 255])
            } else {
                image::Rgba([18 + ruido, 22 + ruido, 30, 255])
            }
        })
    }

    /// Munir, el 26 de agosto de 2026: «Lo importante es que no tarde al hacer capturas y
    /// guardarlas». Nunca se habia medido. Esto pone el numero y lo deja fijado.
    #[test]
    fn guardar_una_pantalla_entera_no_puede_tardar_una_eternidad() {
        let imagen = captura_falsa(1920, 1200);
        let dir = std::env::temp_dir().join("winshotx-test-guardar");
        std::fs::create_dir_all(&dir).unwrap();
        let destino = dir.join("medida.png");

        let t = Instant::now();
        let bytes = to_bytes(&imagen).unwrap();
        let ms_portapapeles = t.elapsed().as_millis();

        let t = Instant::now();
        save(&imagen, &destino, 1920, 1200).unwrap();
        let ms_disco = t.elapsed().as_millis();

        // El PNG ya no lo escribe `image::save`, se codifica a mano y se vuelca: hay que
        // comprobar que lo que queda en disco se puede volver a abrir y es lo que era.
        let releido = image::open(&destino).expect("el PNG guardado no se puede volver a abrir");
        assert_eq!((releido.width(), releido.height()), (1920, 1200));
        let releido = releido.to_rgba8();
        assert_eq!(
            releido.get_pixel(960, 600),
            imagen.get_pixel(960, 600),
            "el PNG guardado no tiene los mismos pixeles"
        );

        let en_disco = std::fs::metadata(&destino).unwrap().len();
        eprintln!(
            "[guardar] 1920x1200 -> portapapeles {ms_portapapeles} ms ({} KB), disco {ms_disco} ms ({} KB)",
            bytes.len() / 1024,
            en_disco / 1024
        );

        // El techo solo se exige en release. En debug esto mismo tarda unos 1.400 ms, asi
        // que un limite util aqui haria fallar `cargo test` siempre, por como se compila y
        // no por como esta escrito el codigo. Medido en release el 26-08-2026: 28 ms para
        // el portapapeles y 64 para el disco; el techo deja sitio para maquinas mas lentas
        // pero se pone rojo si alguien vuelve a poner el filtro Adaptive, que costaba 182.
        #[cfg(not(debug_assertions))]
        {
            assert!(
                ms_portapapeles < 120,
                "codificar el PNG para el portapapeles ha tardado {ms_portapapeles} ms"
            );
            assert!(ms_disco < 160, "guardar el PNG ha tardado {ms_disco} ms");
        }
    }
}
