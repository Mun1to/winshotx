//! La tecla Impr Pant, que en Windows 11 no esta libre.
//!
//! Windows la tiene asignada a la Herramienta de Recortes con un valor del registro
//! del usuario, el mismo que se ve en Configuracion > Accesibilidad > Teclado. Mientras
//! ese valor este puesto, RegisterHotKey no consigue la tecla y winshotx no se entera
//! de que se ha pulsado: el atajo se registra "bien" y luego no pasa nada.
//!
//! Por eso hay que apagarlo antes de pedir la tecla. Es HKEY_CURRENT_USER, asi que no
//! hace falta ser administrador y solo afecta a quien lo pide.

#[cfg(windows)]
use crate::error::{AppError, Result};

#[cfg(windows)]
const KEY_PATH: &str = r"Control Panel\Keyboard";
#[cfg(windows)]
const VALUE_NAME: &str = "PrintScreenKeyForSnippingEnabled";

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Lo que vale ahora mismo, o None si Windows nunca lo ha escrito.
#[cfg(windows)]
pub fn read() -> Option<u32> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
        REG_VALUE_TYPE,
    };

    let path = wide(KEY_PATH);
    let name = wide(VALUE_NAME);
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            Some(0),
            KEY_QUERY_VALUE,
            &mut key,
        )
        .is_err()
        {
            return None;
        }

        let mut kind = REG_VALUE_TYPE::default();
        let mut data = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(std::ptr::addr_of_mut!(data).cast::<u8>()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if status.is_ok() {
            Some(data)
        } else {
            None
        }
    }
}

/// Pone el valor. Con 0, la tecla queda libre para quien la pida primero.
#[cfg(windows)]
pub fn write(value: u32) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
        REG_DWORD,
    };

    let path = wide(KEY_PATH);
    let name = wide(VALUE_NAME);
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            Some(0),
            KEY_SET_VALUE,
            &mut key,
        )
        .is_err()
        {
            return Err(AppError::Msg(
                "no se ha podido abrir la configuración del teclado".into(),
            ));
        }
        let bytes = value.to_le_bytes();
        let status = RegSetValueExW(key, PCWSTR(name.as_ptr()), None, REG_DWORD, Some(&bytes));
        let _ = RegCloseKey(key);
        if status.is_err() {
            return Err(AppError::Msg(
                "no se ha podido cambiar la tecla Impr Pant".into(),
            ));
        }
    }
    Ok(())
}

/// Borra el valor: es lo que toca cuando antes no existia y hay que dejarlo igual.
#[cfg(windows)]
pub fn remove() -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
    };

    let path = wide(KEY_PATH);
    let name = wide(VALUE_NAME);
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            Some(0),
            KEY_SET_VALUE,
            &mut key,
        )
        .is_err()
        {
            return Ok(());
        }
        // Si no estaba, no hay nada que deshacer y tampoco es un error.
        let _ = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
        let _ = RegCloseKey(key);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn read() -> Option<u32> {
    None
}

#[cfg(not(windows))]
pub fn write(_value: u32) -> crate::error::Result<()> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(not(windows))]
pub fn remove() -> crate::error::Result<()> {
    Err(crate::error::AppError::Unsupported)
}
