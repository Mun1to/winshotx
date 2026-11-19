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

/// Lo que se le dice a quien tiene la copia de la Microsoft Store y pide una de las dos teclas.
pub const SOLO_FUERA_DE_LA_STORE: &str = "La versión de la Microsoft Store no puede quitarle esta tecla a Windows: Windows no ve los cambios que hace en sus ajustes. Para usarla, instala winshotx desde winshotx.com o con winget.";

/// Si Windows ve lo que winshotx escribe en el registro del usuario.
///
/// Desde la Microsoft Store, NO. Un paquete MSIX de escritorio guarda todo lo que escribe en
/// HKEY_CURRENT_USER en una copia privada suya: «All writes under HKCU are copied on write to a
/// private per-user, per-app location» (Microsoft, «Understanding how packaged desktop apps run
/// on Windows», apartado Registry). La app se lee a sí misma y cree que lo ha cambiado, pero el
/// Explorador lee el registro de verdad y no ve ni la S de `DisabledHotkeys` ni el 0 de Impr Pant.
/// Así que reiniciarlo no suelta nada y la tecla sigue abriendo la Herramienta de Recortes.
///
/// Medido el 19 de noviembre de 2026 en Windows 11 25H2 (26200), con un paquete de prueba con
/// el mismo manifiesto que el de la Store: escribe «sin error», se lee desde dentro y desde fuera
/// no está; y en Windows Sandbox, reiniciado el Explorador, `Win+Mayús+S` sigue siendo suya.
/// Saltárselo pide la capacidad restringida `unvirtualizedResources`, que Microsoft dice que es
/// para juegos suyos y de sus socios.
pub fn windows_ve_el_registro() -> bool {
    !crate::platform::empaquetado::es_msix()
}

/// Para cortar antes de tocar nada cuando lo que se va a escribir no lo vería Windows.
///
/// Se prueba aparte de `windows_ve_el_registro` porque eso depende de cómo se ha instalado
/// la copia que corre la prueba.
pub fn comprobar_que_windows_lo_ve(ve_el_registro: bool) -> crate::error::Result<()> {
    if ve_el_registro {
        Ok(())
    } else {
        Err(crate::error::AppError::Msg(SOLO_FUERA_DE_LA_STORE.into()))
    }
}

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
    use std::time::Duration;
    use windows::Win32::UI::WindowsAndMessaging::GetShellWindow;

    let ventana = || unsafe { GetShellWindow().0 as isize };
    let antes = ventana();

    std::process::Command::new("taskkill")
        .creation_flags(0x0800_0000)
        .args(["/F", "/IM", "explorer.exe"])
        .output()?;

    esperar_shell_nuevo(
        antes,
        ventana,
        || {
            std::process::Command::new("explorer.exe")
                .creation_flags(0x0800_0000)
                .spawn()?;
            Ok(())
        },
        Duration::from_secs(5),
        Duration::from_millis(250),
    )
}

/// Espera a que el escritorio vuelva después de cerrar el Explorador, y dice si ha vuelto.
///
/// `ventana` es la del escritorio (`GetShellWindow`): 0 mientras no hay ninguno, y otra
/// distinta de `antes` cuando ha arrancado uno nuevo. `relanzar` se llama solo si el
/// escritorio se ha ido y no vuelve por su cuenta.
pub(crate) fn esperar_shell_nuevo(
    antes: isize,
    ventana: impl Fn() -> isize,
    relanzar: impl FnOnce() -> crate::error::Result<()>,
    plazo: std::time::Duration,
    paso: std::time::Duration,
) -> crate::error::Result<()> {
    // Hasta el 19 de noviembre bastaba con que hubiera un escritorio, y si `taskkill` no podía
    // con el Explorador (un PC con políticas) el de antes seguía ahí y esto decía que sí.
    let vivo = || {
        let ahora = ventana();
        ahora != 0 && ahora != antes
    };

    // Windows relanza el shell solo, pero tarda lo suyo. Si en cinco segundos no ha
    // vuelto, se arranca a mano: a nadie se le puede dejar sin barra de tareas.
    let hasta = std::time::Instant::now() + plazo;
    while std::time::Instant::now() < hasta {
        std::thread::sleep(paso);
        if vivo() {
            return Ok(());
        }
    }

    // El de antes sigue en pie: no se ha cerrado, así que no hay que abrir otro (saldría una
    // ventana de carpetas) y lo que se apuntó en el registro no lo ha leído nadie.
    if antes != 0 && ventana() == antes {
        return Err(crate::error::AppError::Msg(
            "Windows no ha dejado reiniciar el Explorador, así que la tecla sigue siendo suya. Cierra sesión y vuelve a entrar para que Windows la suelte.".into(),
        ));
    }

    relanzar()?;

    // Y aun asi se espera a verlo en pie antes de decir que salio bien.
    let hasta = std::time::Instant::now() + plazo;
    while std::time::Instant::now() < hasta {
        std::thread::sleep(paso);
        if vivo() {
            return Ok(());
        }
    }
    Err(crate::error::AppError::Msg(
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

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;
    use std::time::Duration;

    const PLAZO: Duration = Duration::from_millis(60);
    const PASO: Duration = Duration::from_millis(5);

    /// Lo que pasó en el PC de clase si estaba instalado desde la Store: winshotx se lee a sí
    /// mismo, dice que ya es suya y la tecla sigue abriendo la Herramienta de Recortes.
    #[test]
    fn desde_la_store_no_se_toca_ninguna_tecla() {
        let error = comprobar_que_windows_lo_ve(false).expect_err("en la Store tiene que decir que no");
        let texto = error.to_string();
        assert!(texto.contains("Microsoft Store"), "{texto}");
        assert!(texto.contains("winshotx.com"), "tiene que decir qué hacer: {texto}");
        assert!(comprobar_que_windows_lo_ve(true).is_ok());
    }

    /// Si `taskkill` no puede (un PC con políticas, un Explorador que no es nuestro), el
    /// escritorio de antes sigue ahí. Eso no es haberlo reiniciado, y tampoco se abre otro
    /// Explorador encima: sería una ventana de carpetas que nadie ha pedido.
    #[test]
    fn el_mismo_escritorio_no_cuenta_como_reiniciado() {
        let relanzado = Cell::new(false);
        let resultado = esperar_shell_nuevo(
            42,
            || 42,
            || {
                relanzado.set(true);
                Ok(())
            },
            PLAZO,
            PASO,
        );
        assert!(resultado.is_err(), "dijo que lo había reiniciado y seguía el de antes");
        assert!(!relanzado.get(), "abrió otro Explorador con el de antes todavía en pie");
    }

    #[test]
    fn vuelve_solo_con_otra_ventana() {
        let vueltas = Cell::new(0);
        let resultado = esperar_shell_nuevo(
            42,
            || {
                vueltas.set(vueltas.get() + 1);
                if vueltas.get() < 3 { 0 } else { 77 }
            },
            || panic!("no hacía falta relanzarlo"),
            PLAZO,
            PASO,
        );
        assert!(resultado.is_ok());
    }

    #[test]
    fn si_no_vuelve_solo_se_relanza() {
        let relanzado = Cell::new(false);
        let resultado = esperar_shell_nuevo(
            42,
            || if relanzado.get() { 77 } else { 0 },
            || {
                relanzado.set(true);
                Ok(())
            },
            PLAZO,
            PASO,
        );
        assert!(resultado.is_ok());
        assert!(relanzado.get());
    }
}

#[cfg(all(test, windows))]
mod tests {
    /// Reinicia el Explorador de VERDAD, asi que va ignorado: no puede correr en cada
    /// `cargo test` ni en CI. Se lanza a mano cuando se toca `restart_shell`, con
    /// `cargo test -- --ignored --nocapture reinicia_el_shell`.
    #[test]
    #[ignore]
    fn reinicia_el_shell() {
        use windows::Win32::UI::WindowsAndMessaging::GetShellWindow;
        let antes = unsafe { GetShellWindow() };
        assert!(!antes.0.is_null(), "no habia shell antes de empezar");

        super::restart_shell().expect("restart_shell ha fallado");

        let despues = unsafe { GetShellWindow() };
        assert!(!despues.0.is_null(), "el shell no ha vuelto");
        // Si la ventana es otra, es que de verdad ha arrancado un shell nuevo y no se ha
        // limitado a devolver el de antes sin hacer nada.
        assert_ne!(antes.0, despues.0, "el shell es el mismo: no se ha reiniciado");
        println!("shell reiniciado: {:?} -> {:?}", antes.0, despues.0);
    }
}

