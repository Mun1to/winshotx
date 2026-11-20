//! Con que se para una grabacion en Linux, para decirlo ANTES de que empiece.
//!
//! **Para que sirve.** En Linux la barra de grabar no se puede sacar del video, asi que hay
//! grabaciones sin barra: todas las de Wayland, y en X11 las de pantalla entera. Quien graba
//! tiene que saber como parar, y tiene que saberlo antes: una vez grabando, cualquier cosa
//! que se le ensenne (un aviso del escritorio, una ventana) sale dentro del video. Asi que
//! va en la pantalla de elegir zona, que es lo ultimo que se ve antes de grabar y que nunca
//! sale en lo grabado.
//!
//! Aqui solo se junta con que cuenta ESTA persona: su tecla, la bandeja si este escritorio
//! la ensenna, y el boton de «dejar de compartir» del propio escritorio. La frase la escribe
//! la pagina (`src/lib/comoParar.ts`), en su idioma.

use std::sync::atomic::{AtomicBool, Ordering};

use ashpd::zbus;
use serde::Serialize;

use crate::state::AppState;

/// Con que se puede parar una grabacion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComoParar {
    /// La tecla de grabar, que es tambien la de parar, escrita para leerla («Ctrl+Shift+5»).
    /// Nada si este escritorio no ha puesto ninguna.
    pub atajo: Option<String>,
    /// Este escritorio ensenna la bandeja (el GNOME de fabrica no).
    pub bandeja: bool,
    /// El escritorio ensenna que se esta compartiendo la pantalla, con su boton de parar
    /// (GNOME y KDE en Wayland).
    pub escritorio: bool,
    /// Lo que se escribe en una consola para parar, para cuando no queda otra cosa.
    pub orden: String,
    /// La tecla que para una captura con scroll antes de que llegue al final, escrita para
    /// leerla. En X11 es Esc, que se coge mientras dura (`capture::scroll::linux`): la barrita
    /// de la captura solo sale si cabe fuera de la zona, asi que hay capturas sin ella. En
    /// Wayland no hay barra nunca ni se puede coger Esc: es la tecla de capturar, si el
    /// escritorio ha puesto una. Sin tecla quedan `escritorio` y `scroll_orden`.
    pub scroll: Option<String>,
    /// Lo que se escribe en una consola para pararla, para cuando no queda otra cosa.
    pub scroll_orden: String,
    /// Al empezar una captura con scroll, el escritorio va a preguntar si deja mover el
    /// puntero (Wayland, mientras no haya permiso guardado). Se avisa antes porque en esa
    /// pregunta viene apagado, y sin el la pagina no baja sola.
    pub scroll_pregunta: bool,
}

/// Lo que hay ahora mismo para parar. No espera a nadie: va en el camino de abrir la captura.
pub fn de_ahora(state: &AppState) -> ComoParar {
    let wayland = super::xdg::es_wayland();
    // La bandeja se mira otra vez para la proxima: alguien puede haberla encendido.
    mirar_la_bandeja();
    ComoParar {
        atajo: atajo_de_grabar(state, wayland),
        bandeja: BANDEJA.load(Ordering::Relaxed),
        escritorio: wayland && escritorio_con_boton_de_parar(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()),
        orden: format!("{} --record", super::xdg::orden_de_la_app()),
        scroll: tecla_del_scroll(wayland),
        scroll_orden: format!("{} --capture", super::xdg::orden_de_la_app()),
        scroll_pregunta: wayland && PREGUNTA_DEL_PUNTERO.load(Ordering::Relaxed),
    }
}

/// Si la proxima captura con scroll va a hacer que el escritorio pregunte por el puntero,
/// segun la ultima vez que se supo. Mientras no se sepa, que no: avisar de una pregunta que
/// no llega confunde mas que callar.
static PREGUNTA_DEL_PUNTERO: AtomicBool = AtomicBool::new(false);

/// Lo apunta quien lo sabe: el arranque y cada captura con scroll (`capture::scroll::wayland`).
pub fn apuntar_la_pregunta_del_puntero(preguntara: bool) {
    PREGUNTA_DEL_PUNTERO.store(preguntara, Ordering::Relaxed);
}

/// La tecla que para una captura con scroll: Esc donde se puede coger (X11), y en Wayland la
/// de capturar, que mientras baja una la termina (`capture::scroll::terminar_la_que_haya`).
fn tecla_del_scroll(wayland: bool) -> Option<String> {
    if !wayland {
        return Some("Esc".to_string());
    }
    let tecla = super::portal::teclas_del_escritorio()?.capture?;
    let leida = tecla.atajo.as_deref().map(atajo_legible);
    Some(leida.unwrap_or(tecla.texto))
}

/// Un atajo de los ajustes (`CmdOrCtrl+Shift+Digit5`) como lo leeria una persona
/// (`Ctrl+Shift+5`).
pub fn atajo_legible(atajo: &str) -> String {
    atajo
        .split('+')
        .map(str::trim)
        .filter(|parte| !parte.is_empty())
        .map(|parte| match parte.to_ascii_lowercase().as_str() {
            "cmdorctrl" | "commandorcontrol" | "control" | "ctrl" => "Ctrl".to_string(),
            "super" | "meta" | "cmd" | "command" => "Super".to_string(),
            _ => parte
                .strip_prefix("Key")
                .or_else(|| parte.strip_prefix("Digit"))
                .filter(|resto| resto.len() == 1)
                .unwrap_or(parte)
                .to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// La tecla con la que se para: en Wayland la que ha puesto el escritorio, y fuera la de los
/// ajustes, si se ha podido coger.
fn atajo_de_grabar(state: &AppState, wayland: bool) -> Option<String> {
    if wayland {
        let tecla = super::portal::teclas_del_escritorio()?.record?;
        let leida = tecla.atajo.as_deref().map(atajo_legible);
        return Some(leida.unwrap_or(tecla.texto));
    }
    if !state.shortcuts.read().record {
        return None;
    }
    let atajo = state.settings.read().record_shortcut.clone();
    Some(atajo_legible(&atajo))
}

/// GNOME y KDE ensennan mientras se graba un aviso de pantalla compartida con su boton de
/// parar. Los de wlroots (Sway, Hyprland) no.
fn escritorio_con_boton_de_parar(escritorio: &str) -> bool {
    let escritorio = escritorio.to_ascii_lowercase();
    escritorio.contains("gnome") || escritorio.contains("kde")
}

/// Si alguien ensenna los iconos de bandeja en este escritorio, segun la ultima vez que se
/// miro. Mientras no se sepa, que no: nombrar una bandeja que no esta manda a la persona a
/// buscar un icono que no existe.
static BANDEJA: AtomicBool = AtomicBool::new(false);

/// Pregunta al escritorio si ensenna la bandeja y lo deja apuntado. Vuelve enseguida: la
/// respuesta llega por detras, y vale para la siguiente captura.
pub fn mirar_la_bandeja() {
    tauri::async_runtime::spawn(async {
        let Ok(conexion) = zbus::Connection::session().await else {
            return;
        };
        BANDEJA.store(hay_bandeja(&conexion).await, Ordering::Relaxed);
    });
}

async fn hay_bandeja(conexion: &zbus::Connection) -> bool {
    let respuesta = conexion
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &("org.kde.StatusNotifierWatcher",),
        )
        .await;
    let Ok(mensaje) = respuesta else {
        return false;
    };
    let cuerpo = mensaje.body();
    let hay: bool = cuerpo.deserialize().unwrap_or(false);
    hay
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_atajo_de_los_ajustes_se_escribe_para_leerlo() {
        assert_eq!(atajo_legible("CmdOrCtrl+Shift+Digit5"), "Ctrl+Shift+5");
        assert_eq!(atajo_legible("Super+Alt+KeyR"), "Super+Alt+R");
        assert_eq!(atajo_legible("Shift+PrintScreen"), "Shift+PrintScreen");
        assert_eq!(atajo_legible("F9"), "F9");
    }

    #[test]
    fn solo_gnome_y_kde_tienen_su_boton_de_dejar_de_compartir() {
        assert!(escritorio_con_boton_de_parar("GNOME"));
        assert!(escritorio_con_boton_de_parar("ubuntu:GNOME"));
        assert!(escritorio_con_boton_de_parar("KDE"));
        assert!(!escritorio_con_boton_de_parar("sway"));
        assert!(!escritorio_con_boton_de_parar("Hyprland"));
        assert!(!escritorio_con_boton_de_parar(""));
    }

    /// Lo que lee la pagina: los nombres de `ComoParar` en `src/lib/types.ts`.
    #[test]
    fn viaja_a_la_pagina_con_los_nombres_que_ella_espera() {
        let como = ComoParar {
            atajo: Some("Ctrl+Shift+5".into()),
            bandeja: true,
            escritorio: false,
            orden: "winshotx --record".into(),
            scroll: Some("Esc".into()),
            scroll_orden: "winshotx --capture".into(),
            scroll_pregunta: true,
        };
        assert_eq!(
            serde_json::to_value(&como).unwrap(),
            serde_json::json!({
                "atajo": "Ctrl+Shift+5",
                "bandeja": true,
                "escritorio": false,
                "orden": "winshotx --record",
                "scroll": "Esc",
                "scrollOrden": "winshotx --capture",
                "scrollPregunta": true
            })
        );
    }
}
