pub mod capture;
mod commands;
pub mod encode;
pub mod error;
mod exporter;
mod hotkeys;
mod platform;
pub mod record;
mod recorder;
mod settings;
mod state;
mod tray;
mod windows_mgr;

use std::time::{Duration, SystemTime};

use tauri::{Manager, RunEvent, WindowEvent};

use crate::state::AppState;

/// Las sesiones son cache: si llevan un dia en el disco, ya no le importan a nadie.
fn purge_old_sessions(root: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(root.join("sessions")) else {
        return;
    };
    let limit = Duration::from_secs(60 * 60 * 24);
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|modified| {
                SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default()
                    > limit
            })
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let config = settings::load(&handle);

            let temp_root = std::env::temp_dir().join("winshotx");
            std::fs::create_dir_all(temp_root.join("sessions"))?;
            purge_old_sessions(&temp_root);

            app.manage(AppState::new(config.clone(), temp_root));
            hotkeys::register(&handle, &config);
            tray::build(&handle)?;

            if std::env::args().any(|arg| arg == "--settings") {
                windows_mgr::show_settings(&handle)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Cerrar los ajustes no cierra la app: sigue viva en la bandeja.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::overlay_bootstrap,
            commands::capture_still,
            commands::cancel_capture,
            commands::start_recording,
            commands::stop_recording,
            commands::pause_recording,
            commands::cancel_recording,
            commands::session_info,
            commands::session_frames,
            commands::frame_image,
            commands::export_media,
            commands::ffmpeg_available,
            commands::get_settings,
            commands::set_settings,
            commands::pick_directory,
            commands::reveal_in_explorer,
            commands::discard_session,
            commands::cache_stats,
            commands::clear_cache,
            commands::shortcut_status,
            commands::open_folder,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("no se ha podido construir la aplicacion")
        .run(|_app, event| {
            if let RunEvent::ExitRequested { api, code, .. } = event {
                // Sin ventanas abiertas la app sigue en la bandeja, salvo que se pida salir.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
