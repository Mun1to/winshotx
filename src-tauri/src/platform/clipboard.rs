use std::path::Path;

use image::RgbaImage;

use crate::error::Result;

/// Cabecera BITMAPINFOHEADER + pixeles, que es lo que Windows llama CF_DIB.
/// Se compone sobre blanco porque muchas apps ignoran el alfa de un DIB de 32 bits.
#[cfg(windows)]
fn build_dib(image: &RgbaImage) -> Vec<u8> {
    let width = image.width() as i32;
    let height = image.height() as i32;
    let row_size = (width as usize) * 3;
    let padded_row = (row_size + 3) & !3;
    let pixel_bytes = padded_row * height as usize;

    let mut out = Vec::with_capacity(40 + pixel_bytes);
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes()); // positivo: filas de abajo arriba
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&24u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes()); // 72 ppp
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    let source = image.as_raw();
    for y in (0..height as usize).rev() {
        let mut written = 0;
        for x in 0..width as usize {
            let i = (y * width as usize + x) * 4;
            let alpha = source[i + 3] as u32;
            let blend = |c: u8| -> u8 {
                ((c as u32 * alpha + 255 * (255 - alpha)) / 255).min(255) as u8
            };
            out.push(blend(source[i + 2]));
            out.push(blend(source[i + 1]));
            out.push(blend(source[i]));
            written += 3;
        }
        while written < padded_row {
            out.push(0);
            written += 1;
        }
    }
    out
}

/// Copia la imagen al portapapeles en dos formatos: PNG con alfa para las apps
/// modernas y CF_DIB para todo lo demas (Paint, Office, chats antiguos).
#[cfg(windows)]
pub fn copy_image(image: &RgbaImage, png_bytes: &[u8]) -> Result<()> {
    use clipboard_win::{formats, raw, register_format, Clipboard};

    let _guard = Clipboard::new_attempts(10)
        .map_err(|e| crate::error::AppError::Msg(format!("portapapeles ocupado: {e}")))?;
    raw::empty().map_err(|e| crate::error::AppError::Msg(e.to_string()))?;

    if let Some(format) = register_format("PNG") {
        raw::set_without_clear(format.get(), png_bytes)
            .map_err(|e| crate::error::AppError::Msg(e.to_string()))?;
    }
    let dib = build_dib(image);
    raw::set_without_clear(formats::CF_DIB, &dib)
        .map_err(|e| crate::error::AppError::Msg(e.to_string()))?;
    Ok(())
}

/// Copia el archivo como tal (CF_HDROP) para poder pegarlo en Slack, Discord
/// o el explorador: un GIF o un MP4 no caben en el portapapeles como imagen.
#[cfg(windows)]
pub fn copy_files(paths: &[&Path]) -> Result<()> {
    use clipboard_win::{formats, Setter};

    let list: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let _guard = clipboard_win::Clipboard::new_attempts(10)
        .map_err(|e| crate::error::AppError::Msg(format!("portapapeles ocupado: {e}")))?;
    clipboard_win::raw::empty().map_err(|e| crate::error::AppError::Msg(e.to_string()))?;
    formats::FileList
        .write_clipboard(&list)
        .map_err(|e| crate::error::AppError::Msg(e.to_string()))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn copy_image(_image: &RgbaImage, _png_bytes: &[u8]) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(not(windows))]
pub fn copy_files(_paths: &[&Path]) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}
