pub mod archivos;
pub mod capture;
pub(crate) mod commands;
pub mod encode;
pub mod error;
mod exporter;
mod hotkeys;
mod platform;
pub mod record;
mod recorder;
mod replay;
mod settings;
mod state;
mod textos;
mod tray;
mod windows_mgr;

use std::time::{Duration, SystemTime};

use tauri::{Manager, RunEvent, WindowEvent};

use crate::state::AppState;

/// Se lo manda la bandeja a la ventana de ajustes para que mire si hay version nueva.
pub const EVENT_CHECK_UPDATE: &str = "winshotx://check-update";

/// Y este avisa de que la ventana vuelve a estar a la vista, para refrescar lo que
/// se haya quedado viejo mientras estaba escondida.
pub const EVENT_SETTINGS_SHOWN: &str = "winshotx://settings-shown";

/// El overlay se reutiliza entre capturas (ver windows_mgr::open_overlays): esto le dice
/// a una ventana que YA estaba montada que hay una captura nueva que cargar, porque no va
/// a recibir un remontaje que dispare su arranque solo.
pub const EVENT_OVERLAY_SHOW: &str = "winshotx://overlay-show";

/// Cuantos segundos le quedan al temporizador. Lo recibe la ventanita de la cuenta atras,
/// que a partir de ahi baja sola de segundo en segundo: Rust solo dice el numero de
/// partida y avisa con un cero cuando se acabo.
pub const EVENT_COUNTDOWN: &str = "winshotx://countdown";

/// El video de vista previa de una sesion ya esta escrito, y con el llega el play.
///
/// Lo manda el anillo de los ultimos segundos, que abre el editor antes de tener el video
/// para no hacer esperar doce segundos a quien acaba de pulsar la tecla. Lleva el id de la
/// sesion: el editor mira si es la suya y la vuelve a pedir.
pub const EVENT_SESSION_PREVIEW: &str = "winshotx://session-preview";

/// El numero de una pantalla, ensennado en esa pantalla un par de segundos.
///
/// Lo usa el ajuste que elige que pantalla vigila el anillo: «la 2» no dice nada si no se
/// sabe cual es la 2. Comparte ventana con la cuenta atras, y un cero la apaga.
pub const EVENT_SCREEN_NUMBER: &str = "winshotx://screen-number";

/// Las capturas ancladas viven en ventanas, y las ventanas no sobreviven a cerrar la app.
///
/// Sus PNG si sobrevivirian, uno por cada vez que alguien ancla algo, hasta llenar el
/// disco de recortes que ya nadie puede volver a ver. Se vacia la carpeta entera al
/// arrancar, que es el unico momento en el que se sabe seguro que ninguno esta en uso.
fn purge_pins(root: &std::path::Path) {
    let _ = std::fs::remove_dir_all(root.join("pins"));
}

/// Y lo que dejo el anillo de los ultimos segundos, que puede ser mucho.
///
/// Se borra al arrancar y no al salir a proposito: si la app se cierra a lo bruto o se
/// cuelga, nadie llega a limpiar, y ahi dentro hay cientos de megabytes que ya no sirven
/// para nada. El arranque es el unico momento en el que se sabe que no los usa nadie.
fn purge_replay(root: &std::path::Path) {
    let _ = std::fs::remove_dir_all(root.join("replay"));
}

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
        // Tiene que ir la primera: si ya hay un winshotx vivo, esta instancia se
        // cierra y le pasa el testigo, en vez de robarle los atajos globales.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = windows_mgr::show_settings(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let mut config = settings::load(&handle);

            // Actualizar desde los propios ajustes acaba en `relaunch()`, y winshotx vuelve
            // a arrancar en la bandeja: la version nueva entra pero no se ve nada, ni
            // siquiera que ha pasado algo. Si la version guardada no es la de ahora, este
            // arranque viene de una actualizacion, y da igual por donde haya entrado
            // (los ajustes, winget o reinstalar a mano).
            //
            // `onboarded` separa actualizar de estrenar: recien instalada la ventana ya se
            // abre sola con la bienvenida, y ahi no hay ninguna novedad que anunciar.
            let version = handle.package_info().version.to_string();
            let recien_actualizado = settings::viene_de_actualizar(
                config.last_version.as_deref(),
                &version,
                config.onboarded,
            );
            if config.last_version.as_deref() != Some(version.as_str()) {
                config.last_version = Some(version);
                let _ = settings::save(&handle, &config);
            }

            let temp_root = std::env::temp_dir().join("winshotx");
            std::fs::create_dir_all(temp_root.join("sessions"))?;
            purge_old_sessions(&temp_root);
            purge_pins(&temp_root);
            purge_replay(&temp_root);

            app.manage(AppState::new(config.clone(), temp_root, recien_actualizado));

            // Si el usuario nos dio la tecla Impr Pant hay que comprobarlo en cada
            // arranque: una actualizacion de Windows o un paseo por Configuracion se la
            // devuelven a la Herramienta de Recortes sin avisar, y entonces el atajo se
            // registra igual pero no llega ni una pulsacion.
            #[cfg(windows)]
            if config.print_screen_capture {
                let _ = platform::snipping::write(0);

            }

            // Y la S fuera de los atajos del escritorio, solo si se pidio esa opcion
            // aparte: es la que cuesta perder Win+S.
            #[cfg(windows)]
            if config.take_win_shift_s {
                let actuales = platform::snipping::read_disabled_hotkeys().unwrap_or_default();
                if !actuales.to_uppercase().contains('S') {
                    let _ =
                        platform::snipping::write_disabled_hotkeys(Some(&format!("{actuales}S")));
                }
            }

            hotkeys::register(&handle, &config);
            tray::build(&handle)?;

            // El anillo de los ultimos segundos vuelve solo si se dejo encendido: es un
            // ajuste que se pone una vez y se espera encontrar puesto. Si no arranca, se
            // dice por consola y la app sigue: quedarse sin winshotx por esto seria peor.
            if config.replay_enabled {
                if let Err(error) = replay::start(&handle) {
                    eprintln!("[replay] no se ha podido encender al arrancar: {error}");
                }
            }
            // No hay ninguna ventana visible esperando en este momento (la app arranca
            // en la bandeja), asi que crear las ventanas overlay ahora, ocultas, no lo
            // nota nadie: la primera captura del dia encuentra el pool ya listo.
            windows_mgr::precrear_overlays(&handle);

            // La primera vez se abre sola con la bienvenida: recien instalada, la app
            // vive en la bandeja y sin esto no habria nada que mirar. Y al actualizar
            // igual, que es el mismo problema: si no, se reinicia y no se ve nada.
            if !config.onboarded
                || recien_actualizado
                || std::env::args().any(|arg| arg == "--settings")
            {
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
            commands::freeze_bytes,
            commands::capture_still,
            commands::capture_all_screens,
            commands::cancel_capture,
            commands::copy_color,
            commands::copy_pinned,
            commands::save_pinned,
            commands::pinned_text,
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
            commands::just_updated,
            commands::cache_stats,
            commands::clear_cache,
            commands::shortcut_status,
            commands::list_screens,
            commands::show_screen_number,
            commands::replay_status,
            commands::replay_save,
            commands::print_screen_state,
            commands::use_print_screen,
            commands::use_win_shift_s,
            commands::restart_shell,
            commands::open_folder,
            commands::open_windows_apps,
            commands::remove_snipping_tool,
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
