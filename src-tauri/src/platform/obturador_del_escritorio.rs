//! S45 (Linux): donde el escritorio ya toca SU obturador al capturar, el nuestro calla.
//!
//! **Que pasa.** En GNOME con Wayland la foto de una captura se pide al portal
//! (`capture::wayland`), el portal se la pide a GNOME Shell y el Shell, al hacerla, toca el
//! «screen-capture» de su tema de sonidos: 310 ms, medidos en el GNOME 50 del laboratorio, cada
//! vez que se abre una captura. winshotx no puede pedirle que no suene. Con «Sonido de
//! obturador» encendido sonaban dos por captura: el del escritorio al abrirla y el nuestro al
//! acabarla. Una captura, un obturador: en ese camino el nuestro no suena, y la fila de los
//! ajustes lo dice.
//!
//! **Solo cuando se sabe seguro.** Tienen que cumplirse las tres cosas: las ventanas van por
//! Wayland (asi que la foto ha pasado por el portal), el escritorio dice ser GNOME (que es por
//! lo que el portal elige a quien pedirle la foto) y el servicio de capturas del Shell esta en
//! el bus. Con cualquier duda el nuestro sigue sonando: mejor dos que ninguno. Y si en GNOME
//! estan apagados los sonidos de eventos (`org.gnome.desktop.sound event-sounds`), el suyo no
//! suena y el nuestro vuelve.
//!
//! Lo que no pasa por la foto del portal no cambia: el clic de «grabacion guardada» sigue
//! sonando en GNOME, porque ahi el escritorio no toca nada.

use std::sync::atomic::{AtomicBool, Ordering};

use ashpd::zbus;

/// El nombre con el que GNOME Shell atiende las fotos que le pide el portal.
const SERVICIO_DEL_SHELL: &str = "org.gnome.Shell.Screenshot";

/// Si el escritorio toca su obturador cuando saca la foto de una captura. `sus_sonidos_apagados`
/// solo se pregunta cuando lo demas ya dice que es GNOME con el portal.
pub(crate) fn lo_toca(
    wayland: bool,
    escritorio: &str,
    shell_de_gnome: bool,
    sus_sonidos_apagados: impl FnOnce() -> bool,
) -> bool {
    let dice_ser_gnome = escritorio.split(':').any(|parte| parte.trim().eq_ignore_ascii_case("gnome"));
    wayland && dice_ser_gnome && shell_de_gnome && !sus_sonidos_apagados()
}

/// Si el servicio de capturas de GNOME Shell esta en el bus, segun la ultima vez que se miro.
/// Mientras no se sepa, que no: el nuestro suena.
static SHELL_DE_GNOME: AtomicBool = AtomicBool::new(false);

/// Pregunta al bus por el servicio del Shell y lo deja apuntado. Vuelve enseguida: la respuesta
/// llega por detras, y vale para la siguiente captura.
pub fn mirar() {
    tauri::async_runtime::spawn(async {
        let Ok(conexion) = zbus::Connection::session().await else {
            return;
        };
        let hay = super::como_parar::esta_en_el_bus(&conexion, SERVICIO_DEL_SHELL).await;
        if SHELL_DE_GNOME.swap(hay, Ordering::Relaxed) != hay {
            eprintln!("[sonido] el servicio de capturas de GNOME Shell {} en el bus", if hay { "está" } else { "ya no está" });
        }
    });
}

/// GNOME con sus sonidos de eventos apagados a proposito. Solo es que si cuando se lee: sin el
/// esquema o sin la clave vale lo de fabrica, que es encendido.
fn sonidos_de_gnome_apagados() -> bool {
    use gtk::gio;
    use gtk::gio::prelude::*;

    // Sin mirar antes que el esquema y la clave existen, GLib aborta el programa entero.
    let esquema = gio::SettingsSchemaSource::default().and_then(|fuente| fuente.lookup("org.gnome.desktop.sound", true));
    let Some(esquema) = esquema else {
        return false;
    };
    if !esquema.has_key("event-sounds") {
        return false;
    }
    !gio::Settings::new_full(&esquema, None::<&gio::SettingsBackend>, None).boolean("event-sounds")
}

/// Si ahora mismo el escritorio toca su obturador al sacar la foto de una captura.
pub fn lo_toca_ahora() -> bool {
    lo_toca(
        super::xdg::es_wayland(),
        &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        SHELL_DE_GNOME.load(Ordering::Relaxed),
        sonidos_de_gnome_apagados,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_gnome_con_wayland_y_su_servicio_en_el_bus_el_nuestro_calla() {
        for escritorio in ["GNOME", "ubuntu:GNOME", "GNOME-Classic:GNOME", "zorin:gnome"] {
            assert!(lo_toca(true, escritorio, true, || false), "{escritorio}");
        }
    }

    /// Mejor dos que ninguno: con cualquier cosa que no cuadre, el nuestro sigue sonando.
    #[test]
    fn con_cualquier_duda_el_nuestro_sigue_sonando() {
        // En X11 la foto no pasa por el portal, aunque el escritorio sea GNOME.
        assert!(!lo_toca(false, "GNOME", true, || false));
        // Otros escritorios de Wayland no tocan nada al sacar la foto (o no se sabe).
        for otro in ["KDE", "sway", "Hyprland", "COSMIC", "X-Cinnamon", "GNOME-Flashback", ""] {
            assert!(!lo_toca(true, otro, true, || false), "{otro}");
        }
        // Dice ser GNOME, pero el Shell que toca el sonido no esta.
        assert!(!lo_toca(true, "GNOME", false, || false));
        assert!(!lo_toca(true, "Budgie:GNOME", false, || false));
    }

    #[test]
    fn con_los_sonidos_de_gnome_apagados_vuelve_el_nuestro() {
        assert!(!lo_toca(true, "GNOME", true, || true));
    }

    /// Los ajustes de GNOME no se leen fuera de GNOME: en otro escritorio puede no estar ni el
    /// esquema, y no hay por que tocarlo.
    #[test]
    fn fuera_de_gnome_no_se_leen_sus_ajustes() {
        let nunca = || -> bool { panic!("no hacia falta leer los ajustes de GNOME") };
        assert!(!lo_toca(false, "GNOME", true, nunca));
        assert!(!lo_toca(true, "KDE", true, nunca));
        assert!(!lo_toca(true, "GNOME", false, nunca));
    }

    /// Leer el ajuste de verdad no revienta, este o no el esquema de GNOME en la maquina.
    #[test]
    fn leer_los_sonidos_de_gnome_no_revienta() {
        let _ = sonidos_de_gnome_apagados();
        let _ = lo_toca_ahora();
    }
}
