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

/// Y aqui vive la lista de letras que el shell deja de atender junto a la tecla Windows.
///
/// Es lo unico que existe para que `Win+Mayus+S` deje de abrir la Herramienta de Recortes:
/// no valen ni un hook de teclado, ni una politica del sistema, ni redirigir el protocolo,
/// porque el shell atiende esa tecla antes que cualquier programa. Con la letra apuntada
/// aqui deja de atenderla y la tecla queda libre.
///
/// El precio hay que decirlo en la interfaz: la misma letra apaga tambien `Win+S`, que es
/// la busqueda de Windows, y nada de esto surte efecto hasta cerrar sesion.
#[cfg(windows)]
const HOTKEYS_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
#[cfg(windows)]
const HOTKEYS_VALUE: &str = "DisabledHotkeys";

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

/// Las letras que el shell tiene apuntadas ahora mismo, o None si no hay ninguna.
#[cfg(windows)]
pub fn read_disabled_hotkeys() -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
        REG_VALUE_TYPE,
    };

    let path = wide(HOTKEYS_PATH);
    let name = wide(HOTKEYS_VALUE);
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
        let mut datos = [0u16; 128];
        let mut size = (datos.len() * 2) as u32;
        let status = RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(datos.as_mut_ptr().cast::<u8>()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if status.is_err() {
            return None;
        }
        // El tamano viene en bytes e incluye el cero final del texto.
        let largo = (size as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&datos[..largo]))
    }
}

/// Escribe la lista de letras, o borra el valor entero cuando ya no queda ninguna.
#[cfg(windows)]
pub fn write_disabled_hotkeys(letras: Option<&str>) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_SZ,
    };

    let path = wide(HOTKEYS_PATH);
    let name = wide(HOTKEYS_VALUE);
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
                "no se ha podido abrir la configuración de atajos de Windows".into(),
            ));
        }
        let resultado = match letras.filter(|l| !l.is_empty()) {
            Some(letras) => {
                let datos = wide(letras);
                let bytes =
                    std::slice::from_raw_parts(datos.as_ptr().cast::<u8>(), datos.len() * 2);
                RegSetValueExW(key, PCWSTR(name.as_ptr()), None, REG_SZ, Some(bytes))
            }
            None => {
                let _ = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
                windows::Win32::Foundation::WIN32_ERROR(0)
            }
        };
        let _ = RegCloseKey(key);
        if resultado.is_err() {
            return Err(AppError::Msg(
                "no se han podido cambiar los atajos de Windows".into(),
            ));
        }
    }
    Ok(())
}

/// Desinstala la Herramienta de Recortes del usuario que la pide.
///
/// Es la unica forma de que no vuelva a abrirse por ningun camino, y por eso solo se hace
/// cuando alguien lo pulsa a proposito y confirma. No toca el sistema: el paquete se quita
/// de este usuario y vuelve desde la Microsoft Store cuando quiera, sin permisos de
/// administrador y sin que le afecte a nadie mas del equipo.
#[cfg(windows)]
pub fn uninstall_snipping_tool() -> Result<bool> {
    use std::os::windows::process::CommandExt;

    let salida = std::process::Command::new("powershell")
        .creation_flags(0x0800_0000) // sin ventana de consola
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$p = Get-AppxPackage -Name Microsoft.ScreenSketch; \
             if ($p) { $p | Remove-AppxPackage; 'quitada' } else { 'no estaba' }",
        ])
        .output()?;

    if !salida.status.success() {
        return Err(AppError::Msg(
            "Windows no ha dejado quitar la Herramienta de Recortes".into(),
        ));
    }
    // Devuelve si habia algo que quitar, para poder decirlo sin mentir.
    Ok(String::from_utf8_lossy(&salida.stdout).contains("quitada"))
}

/// Hace que el escritorio vuelva a leer `DisabledHotkeys`, sin cerrar sesion.
///
/// Esa lista se lee **una sola vez, al arrancar el shell**, asi que hasta ahora la unica
/// forma de que la letra apagada surtiera efecto era cerrar sesion y volver a entrar. Y
/// eso es pedirle a alguien que cierre todo lo que tiene abierto por un atajo: casi nadie
/// lo hace, y mientras tanto la tecla no es de nadie.
///
/// Reiniciar el Explorador consigue lo mismo en dos segundos. Se lleva por delante las
/// ventanas del Explorador de archivos que hubiera abiertas, nada mas: los programas
/// siguen donde estaban. Verificado en la maquina de Munir el 25 de agosto de 2026, donde
/// `RegisterHotKey` de `Win+Mayus+S` fallaba antes y pasaba a funcionar despues.
#[cfg(windows)]
pub fn restart_shell() -> Result<()> {
    use std::os::windows::process::CommandExt;
    use std::time::{Duration, Instant};
    use windows::Win32::UI::WindowsAndMessaging::GetShellWindow;

    let vivo = || unsafe { !GetShellWindow().0.is_null() };

    std::process::Command::new("taskkill")
        .creation_flags(0x0800_0000)
        .args(["/F", "/IM", "explorer.exe"])
        .output()?;

    // Windows relanza el shell solo, pero tarda lo suyo. Si en cinco segundos no ha
    // vuelto, se arranca a mano: a nadie se le puede dejar sin barra de tareas.
    let hasta = Instant::now() + Duration::from_secs(5);
    while Instant::now() < hasta {
        std::thread::sleep(Duration::from_millis(250));
        if vivo() {
            return Ok(());
        }
    }

    std::process::Command::new("explorer.exe")
        .creation_flags(0x0800_0000)
        .spawn()?;

    // Y aun asi se espera a verlo en pie antes de decir que salio bien.
    let hasta = Instant::now() + Duration::from_secs(5);
    while Instant::now() < hasta {
        std::thread::sleep(Duration::from_millis(250));
        if vivo() {
            return Ok(());
        }
    }
    Err(AppError::Msg(
        "el Explorador no ha vuelto solo; reinicia el equipo".into(),
    ))
}

#[cfg(not(windows))]
pub fn restart_shell() -> crate::error::Result<()> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(not(windows))]
pub fn uninstall_snipping_tool() -> crate::error::Result<bool> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(not(windows))]
pub fn read_disabled_hotkeys() -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn write_disabled_hotkeys(_letras: Option<&str>) -> crate::error::Result<()> {
    Err(crate::error::AppError::Unsupported)
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
