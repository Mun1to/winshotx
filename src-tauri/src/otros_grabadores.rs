//! Los otros programas que graban la pantalla todo el rato, como el anillo de winshotx.
//!
//! Munir, 11-nov-2026: *«un botón de inhabilitar capturador de NVIDIA, porque consume todo
//! el rato y no es necesario si tienes winshotx ya»*. La Repetición instantánea de NVIDIA y
//! la grabación en segundo plano de la Barra de juego de Windows hacen lo mismo que «Grabar
//! siempre lo último»: tener dos a la vez es pagar dos veces por lo mismo.
//!
//! **No se apagan desde aquí, se avisa.** Los ajustes de NVIDIA no son públicos (en el
//! registro son valores binarios sin nombre), y escribirlos a ciegas es romperle el programa
//! a alguien en la próxima actualización suya. Lo que sí se puede saber:
//!
//! - **La Barra de juego**, con certeza: Windows lo guarda en el registro, y su página de
//!   ajustes se abre con una dirección oficial (`ms-settings:gaming-gamedvr`).
//! - **NVIDIA**, solo si su panel está en marcha (`NVIDIA Overlay.exe`). Si la repetición
//!   está encendida o no, no se ve desde fuera, así que el aviso lo dice con un «si».

use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OtrosGrabadores {
    /// El panel de NVIDIA está en marcha: puede tener la Repetición instantánea puesta.
    pub nvidia: bool,
    /// La Barra de juego de Windows graba en segundo plano.
    pub barra_de_juego: bool,
}

/// Los procesos de NVIDIA que dicen que su panel (y con él su grabador) está en marcha.
const DE_NVIDIA: [&str; 2] = ["nvidia overlay.exe", "nvsphelper64.exe"];

/// Si alguno de esos nombres de proceso es de NVIDIA. Aparte para probarlo sin procesos.
fn hay_nvidia<'a>(procesos: impl IntoIterator<Item = &'a str>) -> bool {
    procesos
        .into_iter()
        .any(|p| DE_NVIDIA.contains(&p.to_ascii_lowercase().as_str()))
}

/// La Barra de juego graba en segundo plano solo con las dos cosas: la captura encendida
/// (si no está el valor, lo está: es lo de fábrica) y «grabar lo que ha pasado» puesto.
fn barra_grabando(captura: Option<u32>, en_segundo_plano: Option<u32>) -> bool {
    captura.unwrap_or(1) != 0 && en_segundo_plano == Some(1)
}

#[cfg(windows)]
fn procesos() -> Vec<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };

    let mut nombres = Vec::new();
    unsafe {
        let Ok(foto) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return nombres;
        };
        let mut entrada = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(foto, &mut entrada).is_ok() {
            loop {
                let largo = entrada.szExeFile.iter().position(|&c| c == 0).unwrap_or(entrada.szExeFile.len());
                nombres.push(String::from_utf16_lossy(&entrada.szExeFile[..largo]));
                if Process32NextW(foto, &mut entrada).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(foto);
    }
    nombres
}

#[cfg(windows)]
fn dword(clave: &str, valor: &str) -> Option<u32> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

    let mut dato = 0u32;
    let mut largo = std::mem::size_of::<u32>() as u32;
    let hecho = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(clave),
            &HSTRING::from(valor),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut dato).cast()),
            Some(&mut largo),
        )
    };
    hecho.is_ok().then_some(dato)
}

#[cfg(windows)]
pub fn mirar() -> OtrosGrabadores {
    let nombres = procesos();
    let clave = r"Software\Microsoft\Windows\CurrentVersion\GameDVR";
    OtrosGrabadores {
        nvidia: hay_nvidia(nombres.iter().map(String::as_str)),
        barra_de_juego: barra_grabando(
            dword(clave, "AppCaptureEnabled"),
            dword(clave, "HistoricalCaptureEnabled"),
        ),
    }
}

#[cfg(not(windows))]
pub fn mirar() -> OtrosGrabadores {
    OtrosGrabadores::default()
}

#[tauri::command]
pub async fn otros_grabadores() -> OtrosGrabadores {
    // Recorrer los procesos son unos milisegundos, pero fuera del hilo de las órdenes.
    tauri::async_runtime::spawn_blocking(mirar).await.unwrap_or_default()
}

/// Abre la página de Windows donde se apaga la grabación en segundo plano de la Barra de
/// juego. La dirección es fija: desde la ventana no llega nada que se pueda abrir.
#[tauri::command]
pub async fn abrir_capturas_de_windows() -> crate::error::Result<()> {
    crate::platform::abrir_url("ms-settings:gaming-gamedvr")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidia_se_ve_por_su_panel_o_por_su_grabador() {
        assert!(hay_nvidia(["explorer.exe", "NVIDIA Overlay.exe"]));
        assert!(hay_nvidia(["nvsphelper64.exe"]));
        assert!(!hay_nvidia(["explorer.exe", "nvcontainer.exe", "winshotx.exe"]));
    }

    #[test]
    fn la_barra_de_juego_solo_cuenta_si_graba_en_segundo_plano() {
        assert!(barra_grabando(Some(1), Some(1)));
        assert!(barra_grabando(None, Some(1)), "sin el valor, la captura está encendida de fábrica");
        assert!(!barra_grabando(Some(0), Some(1)), "con la captura apagada no graba nada");
        assert!(!barra_grabando(Some(1), Some(0)));
        assert!(!barra_grabando(None, None), "lo de fábrica es no grabar en segundo plano");
    }

    /// En esta máquina, de verdad: que mirar no falle ni se cuelgue. Lo que devuelva depende
    /// de lo que haya instalado, así que solo se imprime.
    #[test]
    fn mirar_en_esta_maquina() {
        let visto = mirar();
        eprintln!("otros grabadores: {visto:?}");
    }
}
