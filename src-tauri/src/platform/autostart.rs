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

#[cfg(not(windows))]
pub fn set(_enabled: bool) -> Result<()> {
    Err(crate::error::AppError::Unsupported)
}
