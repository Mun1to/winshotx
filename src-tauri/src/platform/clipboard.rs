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

    // Fila a fila sobre memoria ya reservada, y sin mezclar cuando el pixel es opaco, que
    // en una captura de pantalla son todos: la version que empujaba byte a byte y mezclaba
    // siempre costaba mas que codificar el PNG que va al lado.
    let source = image.as_raw();
    let fila_origen = width as usize * 4;
    let cabecera = out.len();
    out.resize(cabecera + pixel_bytes, 0);
    let cuerpo = &mut out[cabecera..];
    for (n, destino) in cuerpo.chunks_exact_mut(padded_row).enumerate() {
        let y = height as usize - 1 - n;
        let origen = &source[y * fila_origen..(y + 1) * fila_origen];
        for (d, s) in destino[..row_size].chunks_exact_mut(3).zip(origen.chunks_exact(4)) {
            if s[3] == 255 {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
            } else {
                let alpha = s[3] as u32;
                let blend = |c: u8| -> u8 { ((c as u32 * alpha + 255 * (255 - alpha)) / 255).min(255) as u8 };
                d[0] = blend(s[2]);
                d[1] = blend(s[1]);
                d[2] = blend(s[0]);
            }
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

/// Copia el archivo de dos maneras a la vez: **como archivo y como su ruta escrita**.
///
/// Un GIF o un MP4 no caben en el portapapeles como imagen, asi que se pega el archivo
/// (CF_HDROP), que es lo que entienden el explorador, Slack, Discord o WhatsApp. Pero un
/// campo de texto no sabe nada de archivos y ahi no aparecia NADA: Munir, el 29 de agosto
/// de 2026, *«sigue sin copiarse el video o la direccion del video al portapapeles»*,
/// con la app diciendo «Copiado» al mismo tiempo, porque copiar habia ido bien.
///
/// Con los dos formatos puestos, cada sitio coge el que sabe usar: donde se pueden soltar
/// archivos se pega el video, y en cualquier caja de texto se pega su ruta. Windows los
/// ofrece en el orden en que se ponen, y el archivo va primero a proposito.
#[cfg(windows)]
pub fn copy_files(paths: &[&Path]) -> Result<()> {
    use clipboard_win::{options, raw};

    let list: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let rutas = list.join("\r\n");
    let _guard = clipboard_win::Clipboard::new_attempts(10)
        .map_err(|e| crate::error::AppError::Msg(format!("portapapeles ocupado: {e}")))?;
    raw::empty().map_err(|e| crate::error::AppError::Msg(e.to_string()))?;
    raw::set_file_list(&list).map_err(|e| crate::error::AppError::Msg(e.to_string()))?;
    // **`NoClear` y no la version normal.** `set_string` a secas vacia el portapapeles
    // antes de escribir, asi que borraba el archivo que se acababa de poner y volviamos a
    // tener un solo formato. Se ve en el codigo de `clipboard_win`: `set_file_list` usa
    // `NoClear` y `set_string` usa `DoClear`.
    raw::set_string_with(&rutas, options::NoClear)
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

/// En Linux, el PNG tal cual, con su alfa: es el formato que piden las apps de Linux al pegar
/// una imagen (GIMP, LibreOffice, los chats, el navegador). No hace falta un segundo formato
/// como el DIB de Windows.
#[cfg(target_os = "linux")]
pub fn copy_image(_image: &RgbaImage, png_bytes: &[u8]) -> Result<()> {
    linux::poner(vec![("image/png", png_bytes.to_vec())])
}

#[cfg(target_os = "linux")]
pub fn copy_text(texto: &str) -> Result<()> {
    linux::poner(linux::como_texto(texto))
}

/// Lo mismo que en Windows: el archivo y su ruta escrita a la vez. El archivo va de las dos
/// maneras que se piden en Linux, la de GNOME (`x-special/gnome-copied-files`, la que usan
/// Archivos, Nemo y Caja) y la de todos (`text/uri-list`: Dolphin, Thunar, los navegadores), y
/// la ruta como texto para cualquier caja de texto.
#[cfg(target_os = "linux")]
pub fn copy_files(paths: &[&Path]) -> Result<()> {
    let uris: Vec<String> = paths
        .iter()
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .filter_map(|p| ashpd::url::Url::from_file_path(p).ok())
        .map(|u| u.to_string())
        .collect();
    if uris.is_empty() {
        return Err(crate::error::AppError::Msg("no hay ningún archivo que copiar".into()));
    }
    let rutas: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
    let mut formatos = vec![
        ("x-special/gnome-copied-files", format!("copy\n{}", uris.join("\n")).into_bytes()),
        ("text/uri-list", format!("{}\r\n", uris.join("\r\n")).into_bytes()),
    ];
    formatos.extend(linux::como_texto(&rutas.join("\n")));
    linux::poner(formatos)
}

/// El portapapeles de Linux, por GTK.
///
/// GTK ya esta dentro (es lo que pinta las ventanas de Tauri) y habla las dos cosas que hay:
/// X11 y Wayland, sin ventanas propias ni conexiones aparte. Y deja ofrecer varios formatos
/// a la vez, que es lo que hace falta para pegar un video como archivo en un sitio y como
/// ruta en otro.
///
/// Dos cosas de Linux que aqui no se ven:
/// - **Lo copiado lo sirve winshotx mientras vive.** En Linux el portapapeles no guarda nada:
///   cuando alguien pega, se le pide a quien copio. Por eso se le dice a GTK que lo puede
///   guardar un gestor del portapapeles si lo hay, y si no, sigue sirviendose desde la
///   bandeja, que no se cierra.
/// - **GTK solo se toca desde su hilo**, el principal. Las ordenes llegan desde otros, asi
///   que se le manda el trabajo y se espera la respuesta con un plazo.
#[cfg(target_os = "linux")]
mod linux {
    use crate::error::{AppError, Result};

    /// Un texto en los nombres de formato que piden las apps de Linux, de los nuevos a los
    /// de X11 de toda la vida.
    pub fn como_texto(texto: &str) -> Vec<(&'static str, Vec<u8>)> {
        ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT"]
            .into_iter()
            .map(|formato| (formato, texto.as_bytes().to_vec()))
            .collect()
    }

    pub fn poner(formatos: Vec<(&'static str, Vec<u8>)>) -> Result<()> {
        let (listo, hecho) = std::sync::mpsc::channel();
        gtk::glib::MainContext::default().invoke(move || {
            let _ = listo.send(poner_en_gtk(formatos));
        });
        hecho
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| AppError::Msg("el portapapeles no contesta".into()))?
    }

    fn poner_en_gtk(formatos: Vec<(&'static str, Vec<u8>)>) -> Result<()> {
        let portapapeles = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        let entradas: Vec<gtk::TargetEntry> = formatos
            .iter()
            .enumerate()
            .map(|(numero, (nombre, _))| {
                gtk::TargetEntry::new(nombre, gtk::TargetFlags::empty(), numero as u32)
            })
            .collect();
        let puesto = portapapeles.set_with_data(&entradas, move |_, seleccion, numero| {
            if let Some((_, datos)) = formatos.get(numero as usize) {
                seleccion.set(&seleccion.target(), 8, datos);
            }
        });
        if !puesto {
            return Err(AppError::Msg("el sistema no ha dejado usar el portapapeles".into()));
        }
        // Sin lista, todos los formatos: si hay un gestor del portapapeles, que se los quede.
        // gtk-rs no envuelve esta funcion, asi que va directa a la de C.
        unsafe {
            use gtk::glib::translate::ToGlibPtr;
            gtk::ffi::gtk_clipboard_set_can_store(portapapeles.to_glib_none().0, std::ptr::null(), 0);
        }
        Ok(())
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn copy_image(_image: &RgbaImage, _png_bytes: &[u8]) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn copy_text(_texto: &str) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(not(any(windows, target_os = "linux")))]
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

        // Y la ruta como texto, que es lo unico que entiende una caja de texto. Sin esto,
        // pegar un video fuera del explorador o de un chat no hacia absolutamente nada.
        let mut texto = String::new();
        formats::Unicode
            .read_clipboard(&mut texto)
            .expect("el portapapeles tendria que llevar tambien la ruta escrita");
        assert_eq!(texto, leidos[0], "el texto no es la ruta del archivo");
        eprintln!("[portapapeles] dentro hay: {} y su ruta escrita", leidos[0]);
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
