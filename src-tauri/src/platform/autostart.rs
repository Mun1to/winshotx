//! Arranque con el sistema. En Windows es una entrada en el registro del usuario:
//! nada de tareas programadas ni permisos de administrador.

use crate::error::Result;

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const VALUE_NAME: &str = "winshotx";

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
pub fn set(enabled: bool) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_SZ,
    };

    let exe = std::env::current_exe()?;
    let command = format!("\"{}\"", exe.to_string_lossy());
    let key_path = wide(RUN_KEY);
    let value_name = wide(VALUE_NAME);

    unsafe {
        let mut key = HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            Some(0),
            KEY_SET_VALUE,
            &mut key,
        );
        if status.is_err() {
            return Err(crate::error::AppError::Msg(
                "no se ha podido abrir el registro de arranque".into(),
            ));
        }

        let result = if enabled {
            let data = wide(&command);
            let bytes = std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2);
            RegSetValueExW(key, PCWSTR(value_name.as_ptr()), None, REG_SZ, Some(bytes))
        } else {
            // Si no estaba puesto, borrarlo no es un error que deba ver el usuario.
            let _ = RegDeleteValueW(key, PCWSTR(value_name.as_ptr()));
            windows::Win32::Foundation::WIN32_ERROR(0)
        };
        let _ = RegCloseKey(key);

        if result.is_err() {
            return Err(crate::error::AppError::Msg(
                "no se ha podido escribir el arranque automático".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set(_enabled: bool) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}
