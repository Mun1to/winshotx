use std::path::Path;

use image::RgbaImage;

use crate::error::Result;

/// Guarda el recorte como PNG, escalandolo antes si el usuario cambio las dimensiones.
pub fn save(image: &RgbaImage, path: &Path, width: u32, height: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if image.width() == width && image.height() == height {
        image.save(path)?;
    } else {
        let scaled = image::imageops::resize(
            image,
            width.max(1),
            height.max(1),
            image::imageops::FilterType::Lanczos3,
        );
        scaled.save(path)?;
    }
    Ok(())
}

/// Bytes PNG en memoria: es lo que se mete en el portapapeles.
pub fn to_bytes(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    image.write_to(&mut buffer, image::ImageFormat::Png)?;
    Ok(buffer.into_inner())
}
