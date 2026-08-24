use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::Result;

/// Que pasa justo despues de soltar el raton sobre la region elegida.
/// Son las dos formas de trabajar que pidio la gente: la que deja decidir y la que
/// no pregunta nada. Cualquier otra cosa se puede montar encima de estas dos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureFlow {
    /// Sale la barra flotante y el usuario elige: copiar, guardar, editar o grabar.
    Toolbar,
    /// La imagen se copia al portapapeles sola y el overlay se cierra. Cero clics.
    Instant,
}

impl Default for CaptureFlow {
    fn default() -> Self {
        Self::Toolbar
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub capture_shortcut: String,
    pub record_shortcut: String,
    pub save_directory: String,
    pub capture_flow: CaptureFlow,
    pub copy_after_capture: bool,
    pub open_editor_after_recording: bool,
    pub capture_cursor: bool,
    pub record_audio: bool,
    pub fps: u32,
    pub play_sound: bool,
    pub show_magnifier: bool,
    pub start_with_windows: bool,
    /// La tecla Impr Pant abre winshotx. Va aparte del atajo normal: se suma, no lo
    /// sustituye, asi que quien tenga el suyo puesto no lo pierde al activar esto.
    pub print_screen_capture: bool,
    /// Falso hasta que se termina la bienvenida. Es lo que decide si al abrir la
    /// ventana se ven los cuatro pasos o directamente los ajustes.
    pub onboarded: bool,
    /// Lo que valia `PrintScreenKeyForSnippingEnabled` antes de que le quitaramos la
    /// tecla a la Herramienta de Recortes. Al desactivarlo se devuelve tal cual: la
    /// maquina tiene que quedarse como estaba, no como nos venga bien.
    pub snipping_key_restore: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            capture_shortcut: "CmdOrCtrl+Shift+2".into(),
            record_shortcut: "CmdOrCtrl+Shift+5".into(),
            save_directory: default_save_dir(),
            capture_flow: CaptureFlow::Toolbar,
            copy_after_capture: true,
            open_editor_after_recording: true,
            capture_cursor: true,
            record_audio: false,
            fps: 30,
            play_sound: false,
            show_magnifier: true,
            start_with_windows: false,
            print_screen_capture: false,
            onboarded: false,
            snipping_key_restore: None,
        }
    }
}

fn default_save_dir() -> String {
    dirs_pictures()
        .map(|p| p.join("winshotx").to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string())
}

fn dirs_pictures() -> Option<PathBuf> {
    // En Windows basta con USERPROFILE: nada de arrastrar el crate dirs por esto.
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|home| home.join("Pictures"))
        .filter(|p| p.exists())
}

fn config_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_config_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    let Ok(path) = config_path(app) else {
        return Settings::default();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Settings::default();
    };
    let Ok(mut settings) = serde_json::from_str::<Settings>(&raw) else {
        return Settings::default();
    };

    // Quien ya tenia winshotx configurado no esta estrenandolo. Si el archivo viene de
    // una version anterior a la bienvenida no trae la clave, y sin esto le saldrian los
    // cuatro pasos a todo el mundo al actualizar.
    let decidido = serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|valor| valor.get("onboarded").cloned())
        .is_some();
    if !decidido {
        settings.onboarded = true;
    }
    settings
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<()> {
    let path = config_path(app)?;
    std::fs::write(path, serde_json::to_string_pretty(settings)?)?;
    Ok(())
}
