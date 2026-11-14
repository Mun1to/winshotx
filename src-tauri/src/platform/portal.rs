//! Lo de xdg-desktop-portal que no es de una sola funcion: presentarse al portal con el
//! nombre de la app, y los atajos globales de Wayland.
//!
//! **El orden importa.** El portal apunta quien es cada conexion de D-Bus la primera vez que
//! le habla, y desde ahi ya no deja cambiarlo («Registered too late»). `ashpd` usa una sola
//! conexion para todo el proceso, asi que `registrar_la_app` va antes que cualquier otra
//! llamada a un portal: la foto de Wayland la espera, y los atajos tambien.

use std::sync::{LazyLock, OnceLock};

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
use ashpd::desktop::Session;
use futures_channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures_util::future::{select, Either};
use futures_util::StreamExt;
use tauri::async_runtime::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;
use crate::windows_mgr::{self, OverlayIntent};

/// El identificador de la app, el de `tauri.conf.json`, que es tambien el nombre con que GTK
/// presenta las ventanas (`lib.rs`, `set_prgname`). El portal solo acepta el registro si
/// encuentra un `.desktop` con este nombre: el .deb y el .rpm lo instalan como la entrada del
/// menu (`packaging/linux/com.munir.winshotx.desktop`), y el AppImage deja uno escondido en la
/// carpeta del usuario (`dejar_el_desktop_del_appimage`).
pub const ID_DE_LA_APP: &str = "com.munir.winshotx";

/// Si el portal ha aceptado ya el registro: `None` mientras no se ha intentado.
static REGISTRADA: LazyLock<Mutex<Option<bool>>> = LazyLock::new(|| Mutex::new(None));

/// Se presenta al portal como `com.munir.winshotx`, una vez por proceso, y devuelve si lo ha
/// aceptado.
///
/// Sin esto, una app instalada sin Flatpak le llega al portal sin nombre. Las capturas
/// funcionan igual (el permiso queda a nombre de «todas las apps sin sandbox»), pero los
/// atajos globales no: el portal los rechaza con «An app id is required» (lo comprueba
/// `xdg-desktop-portal` 1.22, `src/global-shortcuts.c`). El registro existe desde la 1.19; en
/// uno mas viejo falla y se sigue sin nombre.
pub async fn registrar_la_app() -> bool {
    let mut registrada = REGISTRADA.lock().await;
    if let Some(valor) = *registrada {
        return valor;
    }
    let recien_dejado = dejar_el_desktop_del_appimage();
    let mut resultado = presentarse().await;
    // El portal vigila la carpeta de los `.desktop`, pero tarda un poco en ver uno nuevo. Un
    // registro que falla no deja apuntado nada, asi que se puede repetir.
    if recien_dejado {
        for _ in 0..3 {
            let sin_el_desktop = resultado
                .as_ref()
                .is_err_and(|error| error.to_string().contains("App info not found"));
            if !sin_el_desktop {
                break;
            }
            let _ = tauri::async_runtime::spawn_blocking(|| {
                std::thread::sleep(std::time::Duration::from_secs(1))
            })
            .await;
            resultado = presentarse().await;
        }
    }
    let valor = match resultado {
        Ok(()) => {
            eprintln!("[portal] registrada como {ID_DE_LA_APP}");
            true
        }
        Err(error) => {
            eprintln!("[portal] sin registrar ({error}): los atajos de Wayland no se pueden pedir");
            false
        }
    };
    *registrada = Some(valor);
    valor
}

async fn presentarse() -> ashpd::Result<()> {
    ashpd::register_host_app(ID_DE_LA_APP.parse::<ashpd::AppID>()?).await
}

/// El AppImage no instala nada, asi que el `.desktop` que pide el registro no esta en ningun
/// sitio. Se deja uno escondido (`NoDisplay`, no sale en el menu) en
/// `~/.local/share/applications`, apuntando al AppImage, si no hay ya uno de la app. Devuelve
/// si lo ha dejado ahora.
///
/// El portal ve un `.desktop` nuevo aunque la carpeta no existiera cuando arranco (medido en
/// el laboratorio de S21: a los 6 s ya lo encontraba), pero no al instante: de ahi los
/// reintentos de `registrar_la_app`. Cuanto tarda exactamente no se ha medido.
fn dejar_el_desktop_del_appimage() -> bool {
    let Some(appimage) = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from) else {
        return false;
    };
    let Some(casa) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        return false;
    };
    let datos_usuario = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .filter(|ruta| ruta.is_absolute())
        .unwrap_or_else(|| casa.join(".local/share"));
    let datos_sistema = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|valor| !valor.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let nombre = format!("{ID_DE_LA_APP}.desktop");
    let ya_hay = std::iter::once(datos_usuario.clone())
        .chain(datos_sistema.split(':').map(std::path::PathBuf::from))
        .any(|carpeta| carpeta.join("applications").join(&nombre).is_file());
    if ya_hay {
        return false;
    }
    let carpeta = datos_usuario.join("applications");
    let texto = desktop_escondido(&appimage.to_string_lossy());
    match std::fs::create_dir_all(&carpeta).and_then(|()| std::fs::write(carpeta.join(&nombre), texto)) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("[portal] no se ha podido dejar el .desktop del AppImage: {error}");
            false
        }
    }
}

/// El `.desktop` escondido del AppImage. La ruta va entre comillas, como pide la
/// especificacion de las entradas de escritorio para un `Exec` con espacios.
fn desktop_escondido(ejecutable: &str) -> String {
    let ruta = ejecutable.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "[Desktop Entry]\nType=Application\nName=winshotx\nExec=\"{ruta}\"\nIcon=winshotx\nNoDisplay=true\nTerminal=false\n"
    )
}

/// Un atajo que se le pide al escritorio: el nombre para nosotros, lo que ensenna el
/// escritorio al preguntar, y la tecla que proponemos con el formato del portal. La tecla es
/// solo una sugerencia: la decide la persona en el dialogo del escritorio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peticion {
    pub id: &'static str,
    pub descripcion: String,
    pub tecla: Option<String>,
}

/// Por donde le llegan las peticiones a la tarea que habla con el portal.
static CANAL: OnceLock<UnboundedSender<Vec<Peticion>>> = OnceLock::new();

/// Le pide al escritorio estos atajos, en lugar de los de la vez anterior. Vuelve enseguida:
/// el escritorio puede preguntar (un dialogo de GNOME), y lo que quede puesto se apunta en
/// `AppState::shortcuts` cuando conteste.
pub fn pedir_atajos(app: &AppHandle, peticiones: Vec<Peticion>) {
    let canal = CANAL.get_or_init(|| {
        let (enviar, recibir) = unbounded();
        tauri::async_runtime::spawn(atender(app.clone(), recibir));
        enviar
    });
    let _ = canal.unbounded_send(peticiones);
}

/// La tarea de los atajos: una sesion del portal viva mientras la app lo este (si la sesion se
/// cierra, el escritorio suelta las teclas), que se rehace cuando cambian los atajos.
async fn atender(app: AppHandle, mut peticiones: UnboundedReceiver<Vec<Peticion>>) {
    if !registrar_la_app().await {
        anotar(&app, &[]);
        return;
    }
    let portal = match GlobalShortcuts::new().await {
        Ok(portal) => portal,
        Err(error) => {
            eprintln!("[atajo] el escritorio no tiene el portal de atajos: {error}");
            anotar(&app, &[]);
            return;
        }
    };
    let mut pulsados = match portal.receive_activated().await {
        Ok(senales) => Box::pin(senales),
        Err(error) => {
            eprintln!("[atajo] no se pueden escuchar los atajos del portal: {error}");
            anotar(&app, &[]);
            return;
        }
    };
    let mut sesion: Option<Session<'_, GlobalShortcuts<'_>>> = None;
    let mut pedidos: Vec<Peticion> = Vec::new();
    loop {
        match select(peticiones.next(), pulsados.next()).await {
            Either::Left((Some(mut nuevos), _)) => {
                // Cada ajuste que se toca vuelve a pedir los atajos: vale el ultimo.
                while let Ok(mas) = peticiones.try_recv() {
                    nuevos = mas;
                }
                if sesion.is_some() && nuevos == pedidos {
                    continue;
                }
                if let Some(vieja) = sesion.take() {
                    let _ = vieja.close().await;
                }
                pedidos = nuevos;
                match atar(&portal, &pedidos).await {
                    Ok((nueva, atados)) => {
                        sesion = Some(nueva);
                        anotar(&app, &atados);
                    }
                    Err(error) => {
                        eprintln!("[atajo] el escritorio no ha puesto los atajos: {error}");
                        anotar(&app, &[]);
                    }
                }
            }
            // El portal manda la senal solo a quien abrio la sesion, y la sesion vieja se cierra
            // antes de abrir otra: lo que llega es nuestro y de ahora.
            Either::Right((Some(senal), _)) => {
                if sesion.is_some() {
                    // El permiso de traer una ventana al frente, que el escritorio da con cada
                    // pulsacion (GNOME lo manda). Ver `pedir_permiso_con_la_ventana`.
                    *ULTIMO_TOKEN.lock() = senal
                        .options()
                        .get("activation_token")
                        .and_then(|valor| String::try_from(valor.clone()).ok());
                    disparar(&app, senal.shortcut_id());
                }
            }
            Either::Left((None, _)) | Either::Right((None, _)) => break,
        }
    }
    if let Some(vieja) = sesion.take() {
        let _ = vieja.close().await;
    }
}

/// Una sesion nueva con estos atajos. Si la persona cancela el dialogo, la sesion se cierra y
/// no queda ninguno.
async fn atar<'a>(
    portal: &GlobalShortcuts<'a>,
    pedidos: &[Peticion],
) -> ashpd::Result<(Session<'a, GlobalShortcuts<'a>>, Vec<Shortcut>)> {
    let sesion = portal.create_session().await?;
    let nuevos: Vec<NewShortcut> = pedidos
        .iter()
        .map(|p| NewShortcut::new(p.id, p.descripcion.as_str()).preferred_trigger(p.tecla.as_deref()))
        .collect();
    let respuesta = match portal.bind_shortcuts(&sesion, &nuevos, None).await {
        Ok(peticion) => peticion.response(),
        Err(error) => Err(error),
    };
    match respuesta {
        Ok(atados) => Ok((sesion, atados.shortcuts().to_vec())),
        Err(error) => {
            let _ = sesion.close().await;
            Err(error)
        }
    }
}

/// Apunta que atajos han quedado puestos de verdad. Uno que el escritorio devuelve sin tecla
/// (la persona lo dejo en blanco en el dialogo) no cuenta: no hay nada que pulsar.
fn anotar(app: &AppHandle, atados: &[Shortcut]) {
    let con_tecla = |id: &str| {
        atados
            .iter()
            .find(|s| s.id() == id)
            .filter(|s| !s.trigger_description().is_empty())
    };
    let captura = con_tecla("capture");
    let editor = con_tecla("edit");
    let state = app.state::<AppState>();
    let estado = {
        let mut estado = state.shortcuts.write();
        estado.capture = captura.is_some();
        estado.edit = editor.is_some();
        *estado
    };
    // La bienvenida y los ajustes preguntan el estado al abrirse, y el escritorio puede
    // contestar despues (cuando la persona acepta su dialogo).
    let _ = app.emit(crate::EVENT_SHORTCUTS_CHANGED, estado);
    eprintln!(
        "[atajo] portal: captura={} editor={}",
        captura.map(|s| s.trigger_description()).unwrap_or("-"),
        editor.map(|s| s.trigger_description()).unwrap_or("-"),
    );
}

/// El token de activacion de la ultima tecla del portal, sin gastar.
static ULTIMO_TOKEN: parking_lot::Mutex<Option<String>> = parking_lot::Mutex::new(None);

/// La foto del portal ha fallado al pulsar un atajo. Lo normal es que falte el permiso de
/// capturas: el portal pregunta la primera vez, pero GNOME solo deja preguntar a la app que
/// tiene el foco («Only the focused app is allowed to show a system access dialog», medido en
/// el laboratorio de S21), y quien pulsa un atajo esta en otra app.
///
/// Se trae la ventana de winshotx al frente con el token de esa pulsacion y se pide el permiso
/// desde ahi: sale el dialogo del escritorio encima de nuestra ventana, y la siguiente pulsacion
/// ya captura. Una sola vez por sesion: si la persona dice que no, no se le insiste.
pub fn pedir_permiso_con_la_ventana(app: &AppHandle) {
    static YA_PEDIDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if YA_PEDIDO.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let token = ULTIMO_TOKEN.lock().take();
    let dentro = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let (Some(token), Some(ventana)) = (token, dentro.get_webview_window("main")) {
            use gtk::prelude::GtkWindowExt;
            if let Ok(gtk) = ventana.gtk_window() {
                gtk.set_startup_id(&token);
            }
        }
        let _ = windows_mgr::show_settings(&dentro);
        std::thread::spawn(|| {
            // Que el escritorio haya dado el foco a la ventana antes de preguntar.
            std::thread::sleep(std::time::Duration::from_millis(700));
            match crate::capture::pedir_permiso_de_capturas() {
                Ok(()) => eprintln!("[portal] permiso de capturas: concedido"),
                Err(error) => eprintln!("[portal] permiso de capturas: {error}"),
            }
        });
    });
}

/// Lo mismo que hacen las teclas del plugin en X11 y en Windows (`hotkeys::on_capture`).
fn disparar(app: &AppHandle, id: &str) {
    let intencion = match id {
        "capture" => OverlayIntent::Capture,
        "edit" => OverlayIntent::Edit,
        _ => return,
    };
    let dentro = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Err(error) = windows_mgr::open_overlays(&dentro, intencion) {
            eprintln!("no se ha podido abrir el overlay: {error}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::desktop_escondido;

    #[test]
    fn el_desktop_del_appimage_va_escondido_y_con_la_ruta_entre_comillas() {
        let texto = desktop_escondido("/home/ana/Mis cosas/winshotx.AppImage");
        assert!(texto.starts_with("[Desktop Entry]\n"));
        assert!(texto.contains("\nExec=\"/home/ana/Mis cosas/winshotx.AppImage\"\n"));
        assert!(texto.contains("\nNoDisplay=true\n"));
    }

    #[test]
    fn las_comillas_y_las_barras_de_la_ruta_se_escapan() {
        let texto = desktop_escondido("/tmp/a\"b\\c.AppImage");
        assert!(texto.contains("Exec=\"/tmp/a\\\"b\\\\c.AppImage\"\n"));
    }
}
