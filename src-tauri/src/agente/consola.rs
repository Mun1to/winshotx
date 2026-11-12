//! La salida de `winshotx.exe agent` cuando se escribe a mano en una consola.
//!
//! En release el exe es de subsistema Windows (`main.rs`), así que Windows no le da consola.
//! Con la salida en una tubería (Git Bash, PowerShell capturando, cualquier agente) da igual:
//! hereda las de quien lo lanza y `println!` funciona. Pero en una consola interactiva no hay
//! salida estándar y lo escrito se perdería: entonces, y solo entonces, se engancha a la consola
//! de quien lo llamó (`AttachConsole(ATTACH_PARENT_PROCESS)`) y abre su salida.
//!
//! **Lo que no tiene arreglo:** la consola no espera a un programa de subsistema Windows, así
//! que en `cmd` la respuesta sale DESPUÉS del siguiente símbolo del sistema. Se lee igual; para
//! esperarlo, se pasa por una tubería (`winshotx agent status | more`).

/// Si falta la salida estándar, coge la de la consola de quien lo ha llamado (y la de errores
/// y la entrada, si también faltan). No toca las que ya vienen puestas.
pub fn preparar() {
    #[cfg(windows)]
    unsafe {
        use windows::core::w;
        use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE};
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        use windows::Win32::System::Console::{
            AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_HANDLE,
            STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        };

        let falta = |cual: STD_HANDLE| GetStdHandle(cual).map_or(true, |h: HANDLE| h.is_invalid() || h.0.is_null());
        let (sin_salida, sin_errores, sin_entrada) =
            (falta(STD_OUTPUT_HANDLE), falta(STD_ERROR_HANDLE), falta(STD_INPUT_HANDLE));
        // Solo si falta la salida: con ella puesta (una tubería) es que lo ha lanzado un
        // programa, y engancharse a su consola por la de errores le ensuciaría la pantalla
        // (un cliente de MCP en una terminal, por ejemplo).
        if !sin_salida {
            return;
        }
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            // Nadie con consola detrás (lo ha lanzado otra app sin tuberías): no hay a dónde
            // escribir, y el código de salida sigue diciendo si fue bien.
            return;
        }
        let abrir = |nombre: windows::core::PCWSTR| {
            CreateFileW(
                nombre,
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        };
        if sin_salida || sin_errores {
            if let Ok(salida) = abrir(w!("CONOUT$")) {
                if sin_salida {
                    let _ = SetStdHandle(STD_OUTPUT_HANDLE, salida);
                }
                if sin_errores {
                    let _ = SetStdHandle(STD_ERROR_HANDLE, salida);
                }
            }
        }
        if sin_entrada {
            if let Ok(entrada) = abrir(w!("CONIN$")) {
                let _ = SetStdHandle(STD_INPUT_HANDLE, entrada);
            }
        }
    }
}
