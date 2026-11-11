//! El fondo de escritorio que tiene puesto Windows, para usarlo de fondo del estudio (B2).
//!
//! Suma cero bytes al instalador: es una pregunta a `user32`, que ya está cargada. Windows
//! contesta con la ruta de la imagen, o con nada cuando el fondo es un color liso, y
//! entonces el color sale de `GetSysColor`.

use std::path::PathBuf;

/// Lo que tiene puesto el escritorio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FondoDeEscritorio {
    /// Una imagen. Ojo: con el pase de diapositivas o con Windows Spotlight es
    /// `TranscodedWallpaper`, un JPEG sin extensión, así que se abre adivinando el formato.
    Imagen(PathBuf),
    /// Un color liso, sin imagen.
    Color([u8; 3]),
}

/// Pregunta a Windows qué fondo tiene el escritorio ahora mismo.
///
/// `None` si no contesta o la imagen que dice ya no está en el disco (un fondo puesto
/// desde una memoria USB que ya no está, por ejemplo).
pub fn actual() -> Option<FondoDeEscritorio> {
    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Gdi::{GetSysColor, COLOR_DESKTOP};
        use windows::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        };

        // MAX_PATH se queda corto con rutas largas: con 1024 caben de sobra y no cuesta nada.
        let mut buffer = vec![0u16; 1024];
        let leida = unsafe {
            SystemParametersInfoW(
                SPI_GETDESKWALLPAPER,
                buffer.len() as u32,
                Some(buffer.as_mut_ptr().cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        if leida.is_ok() {
            let largo = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            let ruta = String::from_utf16_lossy(&buffer[..largo]);
            let ruta = ruta.trim();
            if !ruta.is_empty() {
                let ruta = PathBuf::from(ruta);
                return ruta.is_file().then_some(FondoDeEscritorio::Imagen(ruta));
            }
        }
        // Sin imagen: el color liso del escritorio, que llega como 0x00BBGGRR.
        let color = unsafe { GetSysColor(COLOR_DESKTOP) };
        return Some(FondoDeEscritorio::Color([
            (color & 0xFF) as u8,
            ((color >> 8) & 0xFF) as u8,
            ((color >> 16) & 0xFF) as u8,
        ]));
    }
    #[cfg(not(windows))]
    {
        return None;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// En una máquina con Windows siempre hay algo puesto: una imagen que existe o un color.
    /// No se compara con nada concreto porque depende de la máquina que corre la prueba.
    #[test]
    #[cfg(windows)]
    fn windows_contesta_con_una_imagen_que_existe_o_con_un_color() {
        match actual() {
            Some(FondoDeEscritorio::Imagen(ruta)) => assert!(ruta.is_file(), "{ruta:?} no existe"),
            Some(FondoDeEscritorio::Color(_)) => {}
            None => panic!("Windows no ha contestado nada"),
        }
    }
}
