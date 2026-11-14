//! Lo que en Linux hace el escritorio por nosotros: la carpeta de imagenes del usuario, abrir
//! una carpeta o una direccion con el programa que tenga puesto, y ensennar un archivo
//! marcado en su gestor de archivos.
//!
//! Lo que es leer texto (`imagenes_de_user_dirs`) se compila en todos los sistemas para que
//! sus pruebas corran tambien en Windows; lo demas solo existe en Linux.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::path::{Path, PathBuf};

/// La carpeta de imagenes del usuario, que en Linux no se llama igual en todas partes: en
/// un escritorio en espannol es `~/Imágenes`, y quien lo decide es `xdg-user-dirs`, que lo
/// apunta en `~/.config/user-dirs.dirs`. Si no hay nada apuntado, `~/Pictures`, que es lo
/// que dice la especificacion para cuando falta el archivo.
///
/// No hace falta que exista: `archivos::destino` la crea al guardar la primera captura.
#[cfg(target_os = "linux")]
pub fn carpeta_de_imagenes() -> Option<PathBuf> {
    let casa = std::env::var_os("HOME").map(PathBuf::from).filter(|p| p.is_absolute())?;
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| casa.join(".config"));
    std::fs::read_to_string(config.join("user-dirs.dirs"))
        .ok()
        .and_then(|texto| imagenes_de_user_dirs(&texto, &casa))
        .or_else(|| Some(casa.join("Pictures")))
}

/// Saca `XDG_PICTURES_DIR` del texto de un `user-dirs.dirs`.
///
/// El archivo es de shell, pero `xdg-user-dirs-update` solo escribe dos formas, y la
/// especificacion no admite otras: `"$HOME/algo"` o una ruta absoluta entre comillas, con
/// `\` delante de las comillas, el dolar y la propia barra si el nombre los lleva. Una
/// carpeta puesta a `"$HOME/"` a secas esta desactivada, y entonces no se usa.
pub fn imagenes_de_user_dirs(texto: &str, casa: &Path) -> Option<PathBuf> {
    for linea in texto.lines() {
        let linea = linea.trim();
        let Some(valor) = linea.strip_prefix("XDG_PICTURES_DIR=") else {
            continue;
        };
        let valor = valor.trim();
        let valor = valor
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(valor);
        let valor = quitar_escapes(valor);
        if let Some(resto) = valor.strip_prefix("$HOME") {
            let resto = resto.trim_start_matches('/');
            if resto.is_empty() {
                return None;
            }
            return Some(casa.join(resto));
        }
        if valor.starts_with('/') {
            return Some(PathBuf::from(valor));
        }
        // Cualquier otra cosa (una variable que no es `$HOME`, una ruta relativa) no la
        // escribe nadie y no se puede adivinar: mejor la de siempre que una inventada.
        return None;
    }
    None
}

fn quitar_escapes(valor: &str) -> String {
    let mut salida = String::with_capacity(valor.len());
    let mut caracteres = valor.chars();
    while let Some(c) = caracteres.next() {
        if c == '\\' {
            if let Some(siguiente) = caracteres.next() {
                salida.push(siguiente);
            }
        } else {
            salida.push(c);
        }
    }
    salida
}

/// Si las ventanas de winshotx van por Wayland (y no por X11 ni por XWayland).
///
/// Lo que cuenta es lo que ha elegido GTK al arrancar, no el escritorio: con
/// `GDK_BACKEND=x11` una sesion de Wayland pinta por XWayland y ahi si valen las cosas de
/// X11. Lo apunta [`anotar_la_sesion`] desde el hilo de GTK; antes de eso (las pruebas, por
/// ejemplo) se adivina por las variables, como hace GTK.
#[cfg(target_os = "linux")]
pub fn es_wayland() -> bool {
    match SESION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let backend = std::env::var("GDK_BACKEND").unwrap_or_default();
            let hay_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
            hay_wayland && !backend.trim_start().starts_with("x11")
        }
    }
}

/// 0 sin mirar, 1 X11, 2 Wayland.
#[cfg(target_os = "linux")]
static SESION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Mira con que ha abierto GTK la pantalla. Tiene que llamarse desde el hilo principal, con
/// GTK ya arrancado (el `setup` de Tauri).
#[cfg(target_os = "linux")]
pub fn anotar_la_sesion() {
    use gtk::glib::prelude::*;
    let Some(pantalla) = gtk::gdk::Display::default() else {
        return;
    };
    let wayland = pantalla.type_().name().contains("Wayland");
    SESION.store(if wayland { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
    eprintln!("[sesion] GTK pinta por {}", if wayland { "Wayland" } else { "X11" });
}

/// Hace algo en el hilo de GTK y espera lo que devuelva.
///
/// GTK solo se toca desde el hilo principal, y Tauri tiene ese hilo cogido mientras corre su
/// bucle (`tao` lo hace dentro de `with_thread_default`). `invoke` lo deja en su cola, o lo
/// hace en el acto si ya se esta en ese hilo, asi que llamarlo desde el principal no se
/// queda esperando a si mismo.
#[cfg(target_os = "linux")]
pub fn en_el_hilo_de_gtk<T: Send + 'static>(
    trabajo: impl FnOnce() -> T + Send + 'static,
) -> crate::error::Result<T> {
    let (listo, hecho) = std::sync::mpsc::channel();
    gtk::glib::MainContext::default().invoke(move || {
        let _ = listo.send(trabajo());
    });
    hecho
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| crate::error::AppError::Msg("GTK no contesta".into()))
}

/// Abre una carpeta, un archivo o una direccion con lo que tenga puesto el usuario.
///
/// `xdg-open` es lo que trae cualquier escritorio (es de `xdg-utils`), y decide el
/// programa con las mismas reglas que el gestor de archivos. Se le espera en otro hilo
/// para que no se quede un proceso zombi por cada carpeta abierta, y no se espera aqui: un
/// navegador que tarda en arrancar no tiene por que dejar colgado el menu.
#[cfg(target_os = "linux")]
pub fn abrir(que: &std::ffi::OsStr) -> std::io::Result<()> {
    let mut orden = std::process::Command::new("xdg-open");
    orden
        .arg(que)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    sin_el_entorno_del_appimage(&mut orden);
    let mut hijo = orden.spawn()?;
    std::thread::spawn(move || {
        let _ = hijo.wait();
    });
    Ok(())
}

/// Ensenna el archivo marcado dentro de su carpeta, como el «Mostrar en la carpeta» de
/// Windows.
///
/// Se le pide al gestor de archivos por D-Bus (`org.freedesktop.FileManager1.ShowItems`,
/// que tienen Archivos de GNOME, Dolphin, Nemo, Caja y Thunar). Si no hay ninguno que lo
/// atienda, se abre la carpeta sin marcar nada, que es mejor que no hacer nada.
///
/// La llamada va en su propio hilo: arrancar el gestor por D-Bus puede tardar un segundo, y
/// quien pregunta suele ser una orden de la interfaz que no tiene por que esperar.
#[cfg(target_os = "linux")]
pub fn ensennar(archivo: &Path) -> std::io::Result<()> {
    let archivo = archivo.to_path_buf();
    let carpeta = archivo
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| archivo.clone());
    std::thread::spawn(move || {
        let Ok(uri) = ashpd::url::Url::from_file_path(&archivo) else {
            let _ = abrir(carpeta.as_os_str());
            return;
        };
        let resultado = tauri::async_runtime::block_on(async move {
            let conexion = ashpd::zbus::Connection::session().await?;
            conexion
                .call_method(
                    Some("org.freedesktop.FileManager1"),
                    "/org/freedesktop/FileManager1",
                    Some("org.freedesktop.FileManager1"),
                    "ShowItems",
                    &(vec![uri.as_str()], ""),
                )
                .await?;
            Ok::<(), ashpd::zbus::Error>(())
        });
        if let Err(error) = resultado {
            eprintln!("[xdg] el gestor de archivos no ha contestado ({error}): se abre la carpeta");
            let _ = abrir(carpeta.as_os_str());
        }
    });
    Ok(())
}

/// Lo que mete el AppImage en el entorno para que winshotx encuentre SUS bibliotecas, y que
/// no puede llegarle a otro programa: un navegador o un gestor de archivos arrancado con las
/// bibliotecas de dentro del AppImage revienta o sale roto (le pasa a cualquier AppImage que
/// abre enlaces). Fuera de un AppImage no se toca nada.
#[cfg(target_os = "linux")]
fn sin_el_entorno_del_appimage(orden: &mut std::process::Command) {
    let Some(dentro) = std::env::var_os("APPDIR") else {
        return;
    };
    if std::env::var_os("APPIMAGE").is_none() {
        return;
    }
    let dentro = PathBuf::from(dentro);
    for variable in [
        "LD_LIBRARY_PATH",
        "LD_PRELOAD",
        "GDK_PIXBUF_MODULE_FILE",
        "GDK_PIXBUF_MODULEDIR",
        "GIO_MODULE_DIR",
        "GIO_EXTRA_MODULES",
        "GSETTINGS_SCHEMA_DIR",
        "GST_PLUGIN_PATH",
        "GST_PLUGIN_PATH_1_0",
        "GST_PLUGIN_SYSTEM_PATH",
        "GST_PLUGIN_SYSTEM_PATH_1_0",
        "GST_PLUGIN_SCANNER",
        "GST_PLUGIN_SCANNER_1_0",
        "GTK_PATH",
        "GTK_DATA_PREFIX",
        "GTK_EXE_PREFIX",
        "GTK_IM_MODULE_FILE",
        "PYTHONHOME",
        "PYTHONPATH",
        "PERLLIB",
        "QT_PLUGIN_PATH",
    ] {
        orden.env_remove(variable);
    }
    // Las listas de carpetas se quedan, pero sin las de dentro del AppImage.
    for variable in ["PATH", "XDG_DATA_DIRS"] {
        if let Some(valor) = std::env::var_os(variable) {
            let fuera: Vec<PathBuf> = std::env::split_paths(&valor)
                .filter(|p| !p.starts_with(&dentro))
                .collect();
            if let Ok(junto) = std::env::join_paths(fuera) {
                orden.env(variable, junto);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn casa() -> PathBuf {
        PathBuf::from("/home/munito")
    }

    #[test]
    fn la_carpeta_de_un_escritorio_en_espannol_sale_con_su_nombre() {
        let texto = "# This file is written by xdg-user-dirs-update\n\
                     XDG_DESKTOP_DIR=\"$HOME/Escritorio\"\n\
                     XDG_PICTURES_DIR=\"$HOME/Imágenes\"\n";
        assert_eq!(imagenes_de_user_dirs(texto, &casa()), Some(casa().join("Imágenes")));
    }

    #[test]
    fn una_ruta_absoluta_se_respeta() {
        let texto = "XDG_PICTURES_DIR=\"/datos/fotos\"\n";
        assert_eq!(imagenes_de_user_dirs(texto, &casa()), Some(PathBuf::from("/datos/fotos")));
    }

    #[test]
    fn las_comillas_y_el_dolar_escapados_son_parte_del_nombre() {
        let texto = "XDG_PICTURES_DIR=\"$HOME/Mis \\\"fotos\\\" de 5\\$\"\n";
        assert_eq!(
            imagenes_de_user_dirs(texto, &casa()),
            Some(casa().join("Mis \"fotos\" de 5$"))
        );
    }

    #[test]
    fn una_carpeta_desactivada_o_sin_apuntar_no_inventa_nada() {
        assert_eq!(imagenes_de_user_dirs("XDG_PICTURES_DIR=\"$HOME/\"\n", &casa()), None);
        assert_eq!(imagenes_de_user_dirs("XDG_MUSIC_DIR=\"$HOME/Música\"\n", &casa()), None);
        assert_eq!(imagenes_de_user_dirs("XDG_PICTURES_DIR=\"$OTRA/fotos\"\n", &casa()), None);
        assert_eq!(imagenes_de_user_dirs("", &casa()), None);
    }

    #[test]
    fn un_comentario_no_cuenta() {
        let texto = "#XDG_PICTURES_DIR=\"$HOME/Vieja\"\nXDG_PICTURES_DIR=\"$HOME/Nueva\"\n";
        assert_eq!(imagenes_de_user_dirs(texto, &casa()), Some(casa().join("Nueva")));
    }
}
