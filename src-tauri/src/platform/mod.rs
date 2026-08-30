//! Lo que solo sabe hacer cada sistema: abrir el explorador, tocar el portapapeles,
//! esconder los iconos del escritorio.
//!
//! **Los `return` explicitos dentro de los bloques `cfg` se quedan**, aunque clippy los vea
//! de mas. Cada funcion de aqui tiene dos cuerpos, uno por sistema, y sin el `return` el que
//! se compila funciona solo porque resulta ser el ultimo: basta con que alguien anada una
//! linea detras para que la funcion devuelva otra cosa sin que nadie lo note.
#![allow(clippy::needless_return)]

pub mod autostart;
pub mod clipboard;
pub mod desktop_icons;
pub mod ocr;
pub mod snipping;
pub mod sonido;
pub mod window_style;

use std::path::Path;

/// Abre el explorador con el archivo ya seleccionado.
pub fn reveal(path: &Path) -> crate::error::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .creation_flags(0x0800_0000)
            .arg("/select,")
            .arg(path)
            .spawn()?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(crate::error::AppError::Unsupported)
    }
}

/// Abre la pantalla de Windows donde se desinstalan las aplicaciones.
///
/// Es hasta donde puede llegar winshotx con la Herramienta de Recortes: quitarla es cosa
/// del usuario, en su Configuracion, y se recupera desde la Store cuando quiera. Una
/// herramienta de captura no desinstala aplicaciones del sistema por su cuenta.
pub fn abrir_aplicaciones_de_windows() -> crate::error::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .creation_flags(0x0800_0000)
            .arg("ms-settings:appsfeatures")
            .spawn()?;
        return Ok(());
    }
    #[cfg(not(windows))]
    Err(crate::error::AppError::Unsupported)
}

/// Abre una carpeta en el explorador; si no existe todavia, la crea antes.
pub fn open_folder(path: &Path) -> crate::error::Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .creation_flags(0x0800_0000)
            .arg(path)
            .spawn()?;
        return Ok(());
    }
    #[cfg(not(windows))]
    Err(crate::error::AppError::Unsupported)
}

/// Abre una direccion en el navegador que tenga puesto el usuario.
///
/// Va por `explorer` como el resto de este archivo, y no por el plugin de Tauri, para que
/// la ventana no necesite ningun permiso nuevo: quien decide que direcciones existen es
/// `crate::enlaces`, en Rust, y no la parte que se pinta.
pub fn abrir_url(url: &str) -> crate::error::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .creation_flags(0x0800_0000)
            .arg(url)
            .spawn()?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err(crate::error::AppError::Unsupported)
    }
}
