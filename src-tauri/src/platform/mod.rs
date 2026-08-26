pub mod autostart;
pub mod clipboard;
pub mod desktop_icons;
pub mod snipping;
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
