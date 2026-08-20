use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub capture_shortcut: String,
    pub record_shortcut: String,
    pub save_directory: String,
    pub copy_after_capture: bool,
    pub open_editor_after_recording: bool,
    pub capture_cursor: bool,
    pub record_audio: bool,
    pub fps: u32,
    pub play_sound: bool,
    pub show_magnifier: bool,
    pub start_with_windows: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            capture_shortcut: "CmdOrCtrl+Shift+2".into(),
            record_shortcut: "CmdOrCtrl+Shift+5".into(),
            save_directory: default_save_dir(),
            copy_after_capture: true,
            open_editor_after_recording: true,
            capture_cursor: true,
            record_audio: false,
            fps: 30,
            play_sound: false,
            show_magnifier: true,
            start_with_windows: false,
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
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<()> {
    let path = config_path(app)?;
    std::fs::write(path, serde_json::to_string_pretty(settings)?)?;
    Ok(())
}
