use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::Result;
use crate::state::AppState;
use crate::windows_mgr::{self, OverlayIntent};

/// La app vive en la bandeja: no hay ventana principal hasta que se piden los ajustes.
///
/// **En Windows, sin menu del sistema.** El del boton derecho lo dibuja winshotx
/// (`tray_menu.rs`), porque uno de Windows no sabe ensennar un interruptor, ni el atajo de
/// cada cosa, ni decir que version esta puesta. Aqui solo se escuchan los clics del icono.
///
/// **En Linux, con el menu del sistema y nada mas.** Alli el icono es un AppIndicator, y un
/// AppIndicator no le cuenta a nadie que le han hecho clic: el escritorio abre su menu y ya
/// esta. Lo dice Tauri en `tray/mod.rs` («Linux: Unsupported. The event is not emitted even
/// though the icon is shown»), y avisa de otra cosa peor: sin menu puede que el icono ni se
/// vea. Asi que en Linux el menu es la unica puerta, y lleva lo mismo que el de Windows.
pub fn build(app: &AppHandle) -> Result<()> {
    let mut builder = TrayIconBuilder::with_id("winshotx")
        .tooltip("winshotx")
        .on_tray_icon_event(|tray, event| {
            let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            else {
                return;
            };
            match button {
                // Izquierdo: lo que se viene a hacer nueve de cada diez veces.
                MouseButton::Left => {
                    let _ = windows_mgr::open_overlays(tray.app_handle(), OverlayIntent::Capture);
                }
                // Derecho: el menu, anclado justo donde esta el icono.
                MouseButton::Right => {
                    crate::tray_menu::alternar(
                        tray.app_handle(),
                        (position.x as i32, position.y as i32),
                    );
                }
                MouseButton::Middle => {}
            }
        });

    #[cfg(target_os = "linux")]
    {
        builder = builder
            .menu(&linux::menu(app)?)
            .show_menu_on_left_click(true)
            .on_menu_event(|app, evento| ejecutar(app, evento.id().as_ref()));
    }

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Lo que cambia en el menu cuando se tocan los ajustes (el idioma). En Windows el menu se
/// dibuja de nuevo cada vez que se abre y no hay nada que hacer.
pub fn al_cambiar_los_ajustes(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    if let (Some(icono), Ok(menu)) = (app.tray_by_id("winshotx"), linux::menu(app)) {
        let _ = icono.set_menu(Some(menu));
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app;
}

/// El idioma de un `LANG` (o `LC_ALL`, `LC_MESSAGES`): `es_ES.UTF-8` es espannol. Lo mismo que
/// hace la interfaz con el idioma del navegador (`src/lib/i18n.ts`).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn idioma_de(valor: &str) -> &'static str {
    let valor = valor.to_lowercase();
    if valor.starts_with("es") {
        "es"
    } else if valor.starts_with("zh") {
        "zh"
    } else {
        "en"
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::{AppHandle, Manager, Wry};

    use crate::settings::Language;
    use crate::state::AppState;

    /// Los textos son los mismos del menu dibujado de Windows, con sus traducciones
    /// (`src/lib/textos-en.ts` y `textos-zh.ts`).
    struct Textos {
        capturar: &'static str,
        carpeta: &'static str,
        ajustes: &'static str,
        actualizar: &'static str,
        salir: &'static str,
    }

    fn textos(idioma: &str) -> Textos {
        match idioma {
            "es" => Textos {
                capturar: "Capturar región",
                carpeta: "Abrir la carpeta",
                ajustes: "Ajustes",
                actualizar: "Buscar actualizaciones",
                salir: "Salir",
            },
            "zh" => Textos {
                capturar: "区域截图",
                carpeta: "打开截图文件夹",
                ajustes: "设置",
                actualizar: "检查更新",
                salir: "退出",
            },
            _ => Textos {
                capturar: "Capture a region",
                carpeta: "Open the shots folder",
                ajustes: "Settings",
                actualizar: "Check for updates",
                salir: "Quit",
            },
        }
    }

    fn idioma(app: &AppHandle) -> &'static str {
        match app.state::<AppState>().settings.read().language {
            Language::Es => "es",
            Language::En => "en",
            Language::Zh => "zh",
            Language::Sistema => ["LC_ALL", "LC_MESSAGES", "LANG"]
                .into_iter()
                .filter_map(|variable| std::env::var(variable).ok())
                .find(|valor| !valor.is_empty())
                .map(|valor| super::idioma_de(&valor))
                .unwrap_or("en"),
        }
    }

    /// Grabar no esta todavia: en Linux `recorder::start` contesta que no se puede (Fase 2
    /// del plan de Linux). Una entrada que no hace nada al pulsarla es peor que no tenerla.
    pub fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
        let t = textos(idioma(app));
        Menu::with_items(
            app,
            &[
                &MenuItem::with_id(app, "capture", t.capturar, true, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(app, "folder", t.carpeta, true, None::<&str>)?,
                &MenuItem::with_id(app, "settings", t.ajustes, true, None::<&str>)?,
                &MenuItem::with_id(app, "update", t.actualizar, true, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(app, "quit", t.salir, true, None::<&str>)?,
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::idioma_de;

    #[test]
    fn el_idioma_del_sistema_sale_del_lang() {
        assert_eq!(idioma_de("es_ES.UTF-8"), "es");
        assert_eq!(idioma_de("es_MX"), "es");
        assert_eq!(idioma_de("zh_CN.UTF-8"), "zh");
        assert_eq!(idioma_de("en_US.UTF-8"), "en");
        assert_eq!(idioma_de("C.UTF-8"), "en");
        assert_eq!(idioma_de("fr_FR.UTF-8"), "en");
    }
}

/// Lo que hace cada entrada del menu, en un solo sitio.
///
/// Lo llaman el menu dibujado (`tray_menu.rs`, por el comando `tray_menu_action`) y el
/// menu del sistema, que sigue existiendo de reserva. Estaba escrito dentro del manejador
/// del menu nativo, y dejarlo ahi habria significado dos listas de acciones que se
/// separarian al primer cambio.
pub fn ejecutar(app: &AppHandle, id: &str) {
    match id {
            "capture" => {
                let _ = windows_mgr::open_overlays(app, OverlayIntent::Capture);
            }
            "record" => {
                if app.state::<AppState>().is_recording() {
                    let _ = crate::recorder::stop(app);
                } else {
                    let _ = windows_mgr::open_overlays(app, OverlayIntent::Record);
                }
            }
            "replay" => {
                if let Err(error) = crate::replay::save(app) {
                    eprintln!("no se han podido guardar los ultimos segundos: {error}");
                }
            }
            // Donde acaban las capturas guardadas. Sin esto habia que abrir los ajustes
            // para llegar a una carpeta que se visita todos los dias.
            "folder" => {
                let carpeta = app.state::<AppState>().settings.read().save_directory.clone();
                let _ = std::fs::create_dir_all(&carpeta);
                let _ = crate::platform::open_folder(&std::path::PathBuf::from(carpeta));
            }
            "settings" => {
                let _ = windows_mgr::show_settings(app);
            }
            // La comprobacion vive en la ventana de ajustes: desde la bandeja se
            // abre y se le dice que mire, para no tener dos caminos distintos.
            "update" => {
                let _ = windows_mgr::show_settings(app);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit(crate::EVENT_CHECK_UPDATE, ());
                }
            }
            "quit" => app.exit(0),
            _ => {}
    }
}
