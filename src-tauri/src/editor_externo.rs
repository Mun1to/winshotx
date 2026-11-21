use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::Result;

const ARCHIVO: &str = "{file}";
const EDITOR_INVALIDO: &str = "Elige un ejecutable válido en los ajustes de grabación.";

pub fn preparar(ejecutable: Option<&str>) -> Result<Command> {
    let ruta = Path::new(ejecutable.unwrap_or_default());
    if !ruta.is_absolute() || !ruta.is_file() {
        return Err(EDITOR_INVALIDO.into());
    }
    // En Windows, Command puede invocar cmd.exe implícitamente para .bat y .cmd.
    #[cfg(windows)]
    if !ruta
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Err(EDITOR_INVALIDO.into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if ruta.metadata()?.permissions().mode() & 0o111 == 0 {
            return Err(EDITOR_INVALIDO.into());
        }
    }
    let mut comando = Command::new(ruta);
    comando
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(comando)
}

// Una línea es un argumento completo, incluso si contiene espacios o comillas.
// La ruta se sustituye en el argumento antes de pasarlo al proceso, sin shell.
fn argumentos(plantilla: &str, archivo: &str) -> Vec<String> {
    let mut args: Vec<_> = plantilla
        .lines()
        .filter(|linea| !linea.trim().is_empty())
        .map(|linea| linea.replace(ARCHIVO, archivo))
        .collect();
    if !plantilla.contains(ARCHIVO) {
        args.push(archivo.into());
    }
    args
}

pub fn abrir(mut comando: Command, plantilla: &str, archivo: &str) -> Result<()> {
    let ruta = std::path::absolute(archivo)?;
    let proceso = comando
        .args(argumentos(plantilla, &ruta.to_string_lossy()))
        .spawn()?;
    // En Unix hay que recoger la salida aunque el editor siga abierto minutos.
    #[cfg(unix)]
    std::thread::spawn(move || {
        let mut proceso = proceso;
        let _ = proceso.wait();
    });
    #[cfg(not(unix))]
    drop(proceso);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sustituye_la_ruta_sin_partir_argumentos_ni_interpretar_comillas() {
        let ruta = r"C:\Mis vídeos\demo & corte.mp4";
        assert_eq!(
            argumentos(
                "--nombre=Mi corte\r\n--input={file}\r\ncomillas \"tal cual\"",
                ruta
            ),
            [
                "--nombre=Mi corte",
                &format!("--input={ruta}"),
                "comillas \"tal cual\""
            ]
        );
    }

    #[test]
    fn anade_el_archivo_si_no_hay_marcador() {
        let ruta = r"C:\Mis vídeos\demo & corte.mp4";
        assert_eq!(
            argumentos(" \r\n--sendto-task\ncut\n", ruta),
            ["--sendto-task", "cut", ruta]
        );
        assert_eq!(argumentos("", ruta), [ruta]);
    }

    #[test]
    fn rechaza_programas_ausentes_o_relativos() {
        assert!(preparar(None).is_err());
        assert!(preparar(Some("editor.exe")).is_err());
    }
}
