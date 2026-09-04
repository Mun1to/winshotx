//! Arranque con el sistema. Suelta, en Windows es una entrada en el registro del
//! usuario: nada de tareas programadas ni permisos de administrador.
//!
//! Instalada desde la Microsoft Store hay que ir por otro lado. Dentro de un paquete
//! MSIX el registro esta virtualizado, asi que la entrada se escribe sin error ninguno
//! y el sistema no la lee jamas: la app dice que arranca sola y no arranca. Ahi manda
//! `StartupTask`, que es la puerta que Windows deja abierta a las apps empaquetadas y
//! ademas sale en el Administrador de tareas para que el usuario pueda quitarla.

use crate::error::Result;

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const VALUE_NAME: &str = "winshotx";
/// El mismo identificador que lleva `windows.startupTask` en el AppxManifest.
#[cfg(windows)]
const TAREA_MSIX: &str = "winshotxAutoStart";

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
pub fn set(enabled: bool) -> Result<()> {
    if crate::platform::empaquetado::es_msix() {
        return set_msix(enabled);
    }
    set_registro(enabled)
}

/// La via de la Store. `RequestEnableAsync` puede devolver que no sin fallar: si el
/// usuario lo apago desde el Administrador de tareas, manda el, y hay que decirlo en vez
/// de dejar el interruptor encendido mintiendo.
#[cfg(windows)]
fn set_msix(enabled: bool) -> Result<()> {
    use windows::core::HSTRING;
    use windows::ApplicationModel::{StartupTask, StartupTaskState};

    let fallo = |que: &str| crate::error::AppError::Msg(format!("arranque automático: {que}"));

    let tarea = esperar(
        StartupTask::GetAsync(&HSTRING::from(TAREA_MSIX))
            .map_err(|_| fallo("Windows no encuentra la tarea del paquete"))?,
    )
    .map_err(|_| fallo("Windows no encuentra la tarea del paquete"))?;

    if !enabled {
        tarea.Disable().map_err(|_| fallo("no se ha podido quitar"))?;
        return Ok(());
    }

    let estado = esperar(
        tarea
            .RequestEnableAsync()
            .map_err(|_| fallo("no se ha podido pedir"))?,
    )
    .map_err(|_| fallo("no se ha podido pedir"))?;

    match estado {
        StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => Ok(()),
        StartupTaskState::DisabledByUser => Err(fallo(
            "lo has desactivado en el Administrador de tareas, actívalo ahí",
        )),
        StartupTaskState::DisabledByPolicy => {
            Err(fallo("lo tiene bloqueado la política del equipo"))
        }
        _ => Err(fallo("Windows lo ha rechazado")),
    }
}

/// Espera a que termine una operacion de WinRT y devuelve su resultado.
///
/// El `.join()` que haria esto de una linea vive en un trait privado de `windows-future`,
/// asi que se hace a mano: se cuelga un aviso de «ya termine» y se espera en un canal. Si
/// la operacion ya venia terminada, el aviso salta en el acto y el canal ya trae el valor.
///
/// **Con tope de tiempo, y no por si acaso:** `RequestEnableAsync` puede pararse a
/// preguntarle algo al usuario, y esto corre en el hilo que atiende los ajustes. Sin tope,
/// una respuesta que no llega deja la ventana colgada para siempre.
#[cfg(windows)]
fn esperar<T>(op: windows_future::IAsyncOperation<T>) -> windows::core::Result<T>
where
    T: windows::core::RuntimeType + 'static,
{
    use std::sync::mpsc;
    use std::time::Duration;
    use windows::core::{Error, HRESULT};
    use windows_future::AsyncOperationCompletedHandler;

    // Por el canal solo viaja un aviso de «ya esta», no el resultado: el objeto de WinRT
    // se queda en este hilo y se recoge aqui con `GetResults`. Mandarlo por el canal
    // obligaria a que fuese `Send`, que es una promesa que estos objetos no siempre hacen.
    let (tx, rx) = mpsc::channel::<()>();
    op.SetCompleted(&AsyncOperationCompletedHandler::<T>::new(move |_quien, _estado| {
        let _ = tx.send(());
        Ok(())
    }))?;

    // RPC_E_TIMEOUT: es lo que dice Windows cuando algo suyo no contesta a tiempo.
    if rx.recv_timeout(Duration::from_secs(30)).is_err() {
        return Err(Error::from(HRESULT(0x8001011F_u32 as i32)));
    }
    op.GetResults()
}

#[cfg(windows)]
fn set_registro(enabled: bool) -> Result<()> {
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

/// Si el arranque con Windows apunta a otra copia de winshotx, se corrige.
///
/// **El fallo que arregla, encontrado el 4 de septiembre de 2026 en la maquina de Munir.**
/// Su registro de arranque decia `...\AppData\Local\winshotx\winshotx.exe`, que era la
/// **0.1.18**, mientras la instalada y registrada estaba en `C:\Apps\Random APPS\winshotx`
/// y era la 0.2.18. Encendia el ordenador, arrancaba la vieja, la vieja veia que habia
/// version nueva y le pedia actualizar; actualizaba, la nueva se reiniciaba en su sitio, y
/// al siguiente arranque de Windows volvia la vieja a pedirselo. Sus palabras: *«tienes que
/// estar todo el rato actualizando la app»*. Llevaba asi desde el 27 de agosto.
///
/// **De donde sale.** `set_registro` escribe `current_exe()` el dia que se pulsa el
/// interruptor, y nadie lo vuelve a mirar. Basta con reinstalar en otra carpeta (el
/// asistente de Tauri deja elegirla) para que esa ruta apunte a un ejecutable que ya no es
/// el que manda. No hace falta ni tocar el interruptor: no se entera.
///
/// Por eso esto se llama al arrancar y no al pulsar nada. Y por eso corrige tambien cuando
/// no hay entrada ninguna: si el ajuste dice que arranca sola, tiene que arrancar sola.
#[cfg(windows)]
pub fn revisar_ruta() {
    // Dentro de un MSIX el arranque va por `StartupTask` y el registro esta virtualizado:
    // lo que hubiera ahi no lo lee nadie, asi que tocarlo no arreglaria nada.
    if crate::platform::empaquetado::es_msix() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if !hay_que_corregir(leer_registro().as_deref(), &exe.to_string_lossy()) {
        return;
    }
    if let Err(error) = set_registro(true) {
        eprintln!("[autostart] no se ha podido corregir la ruta de arranque: {error}");
    }
}

#[cfg(not(windows))]
pub fn revisar_ruta() {}

/// Si lo que hay escrito en el arranque no es este ejecutable.
///
/// Aparte para poder probarla: lo de dentro son llamadas al registro de Windows, y lo que
/// aqui se puede equivocar es la comparacion de rutas, no la lectura.
///
/// Las comillas van y vienen (se escriben al guardar, y una entrada puesta a mano puede no
/// llevarlas) y Windows no distingue mayusculas de minusculas en las rutas, asi que las dos
/// cosas se normalizan antes de comparar.
#[cfg(windows)]
fn hay_que_corregir(registrado: Option<&str>, mio: &str) -> bool {
    let Some(registrado) = registrado else {
        // No hay entrada y el ajuste dice que si: hay que ponerla.
        return true;
    };
    let limpiar = |ruta: &str| ruta.trim().trim_matches('"').trim().to_lowercase();
    limpiar(registrado) != limpiar(mio)
}

/// Lo que hay escrito ahora mismo en el arranque, o nada si no hay entrada.
#[cfg(windows)]
fn leer_registro() -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
        REG_VALUE_TYPE,
    };

    let key_path = wide(RUN_KEY);
    let value_name = wide(VALUE_NAME);
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            Some(0),
            KEY_QUERY_VALUE,
            &mut key,
        )
        .is_err()
        {
            return None;
        }

        // Primera vuelta para que Windows diga cuanto ocupa, segunda para leerlo.
        let mut tipo = REG_VALUE_TYPE::default();
        let mut bytes: u32 = 0;
        let consulta = RegQueryValueExW(
            key,
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut tipo),
            None,
            Some(&mut bytes),
        );
        if consulta.is_err() || bytes == 0 {
            let _ = RegCloseKey(key);
            return None;
        }

        let mut datos = vec![0u8; bytes as usize];
        let leido = RegQueryValueExW(
            key,
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut tipo),
            Some(datos.as_mut_ptr()),
            Some(&mut bytes),
        );
        let _ = RegCloseKey(key);
        if leido.is_err() {
            return None;
        }

        // Llega como UTF-16 con su cero al final, que no forma parte del texto.
        let anchos: Vec<u16> = datos[..bytes as usize]
            .chunks_exact(2)
            .map(|par| u16::from_le_bytes([par[0], par[1]]))
            .take_while(|&c| c != 0)
            .collect();
        Some(String::from_utf16_lossy(&anchos))
    }
}

#[cfg(not(windows))]
pub fn set(_enabled: bool) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(all(test, windows))]
mod tests {
    use super::hay_que_corregir;

    const MIO: &str = r"C:\Apps\Random APPS\winshotx\winshotx.exe";

    /// El caso de Munir, tal cual estaba en su registro el 4 de septiembre de 2026.
    #[test]
    fn una_copia_vieja_en_otra_carpeta_se_corrige() {
        let vieja = r#""C:\Users\Muni\AppData\Local\winshotx\winshotx.exe""#;
        assert!(
            hay_que_corregir(Some(vieja), MIO),
            "arrancaba la 0.1.18 de otra carpeta y le pedia actualizar en cada encendido"
        );
    }

    #[test]
    fn la_ruta_buena_se_deja_en_paz() {
        let puesta = format!("\"{MIO}\"");
        assert!(!hay_que_corregir(Some(&puesta), MIO));
    }

    /// Se escriben con comillas, pero una entrada puesta a mano puede no llevarlas, y
    /// reescribir el registro en cada arranque por unas comillas seria tonto.
    #[test]
    fn las_comillas_no_cuentan() {
        assert!(!hay_que_corregir(Some(MIO), MIO));
    }

    /// Windows no distingue mayusculas en las rutas, asi que aqui tampoco.
    #[test]
    fn las_mayusculas_tampoco_cuentan() {
        assert!(!hay_que_corregir(Some(&MIO.to_uppercase()), MIO));
    }

    #[test]
    fn sin_entrada_ninguna_hay_que_ponerla() {
        // El ajuste dice que arranca sola; si no hay entrada, no arranca sola.
        assert!(hay_que_corregir(None, MIO));
    }
}

