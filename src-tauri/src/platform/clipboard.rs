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

/// Copia texto plano, que es lo que deja el lector de texto de una captura.
#[cfg(windows)]
pub fn copy_text(texto: &str) -> Result<()> {
    use clipboard_win::{Clipboard, Setter, formats};

    let _guard = Clipboard::new_attempts(10)
        .map_err(|e| crate::error::AppError::Msg(format!("portapapeles ocupado: {e}")))?;
    clipboard_win::raw::empty().map_err(|e| crate::error::AppError::Msg(e.to_string()))?;
    formats::Unicode
        .write_clipboard(&texto)
        .map_err(|e| crate::error::AppError::Msg(e.to_string()))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn copy_image(_image: &RgbaImage, _png_bytes: &[u8]) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(not(windows))]
pub fn copy_text(_texto: &str) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(not(windows))]
pub fn copy_files(_paths: &[&Path]) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Que copiar un archivo lo deje de verdad en el portapapeles, y que Windows lo
    /// devuelva como archivo.
    ///
    /// Munir, el 28 de agosto de 2026: *«le das a copiar al portapapeles y no pasa
    /// absolutamente nada»*. Copiar un video se hacia a ciegas: el resultado se guardaba
    /// con un `.is_ok()` que se tragaba el error, asi que si el portapapeles estaba
    /// ocupado por otro programa no se enteraba nadie.
    ///
    /// Va con `--ignored` porque **le pisa el portapapeles a quien lo corra**.
    #[test]
    #[ignore]
    fn copiar_un_archivo_lo_deja_en_el_portapapeles() {
        use clipboard_win::{formats, Getter};

        let dir = std::env::temp_dir().join("winshotx-test-portapapeles");
        std::fs::create_dir_all(&dir).unwrap();
        let archivo = dir.join("video.mp4");
        std::fs::write(&archivo, b"no es un mp4 de verdad, da igual").unwrap();

        copy_files(&[&archivo]).expect("no se ha podido copiar");

        let _guard = clipboard_win::Clipboard::new_attempts(10).unwrap();
        let mut leidos: Vec<String> = Vec::new();
        formats::FileList
            .read_clipboard(&mut leidos)
            .expect("el portapapeles no devuelve una lista de archivos");

        assert_eq!(leidos.len(), 1, "ha dejado {} archivos", leidos.len());
        assert!(
            leidos[0].ends_with("video.mp4"),
            "ha dejado otra cosa: {}",
            leidos[0]
        );
        eprintln!("[portapapeles] dentro hay: {}", leidos[0]);
    }

    /// Y que el texto siga funcionando, que es lo que usa la tecla T.
    #[test]
    #[ignore]
    fn copiar_texto_lo_deja_en_el_portapapeles() {
        use clipboard_win::{formats, Getter};

        copy_text("winshotx lee texto").expect("no se ha podido copiar");
        let _guard = clipboard_win::Clipboard::new_attempts(10).unwrap();
        let mut leido = String::new();
        formats::Unicode.read_clipboard(&mut leido).unwrap();
        assert_eq!(leido, "winshotx lee texto");
    }
}
