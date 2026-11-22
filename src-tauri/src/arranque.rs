//! S22. Si winshotx se abre a mano, que se vean los ajustes.
//!
//! winshotx vive en la bandeja. Hasta la 0.4.7, abrirla a mano con la app cerrada (desde el
//! menú Inicio, un acceso directo o con doble clic) solo ponía el icono abajo a la derecha, y a
//! quien la abría le parecía que no había pasado nada. Pero el mismo `winshotx.exe` también
//! arranca solo con Windows, desde la tarea de inicio de la Store, desde un agente de IA o desde
//! un guion con `--capture`, y en esos casos una ventana en medio de la pantalla sobra. Aquí se
//! decide cuál de esos arranques es este, en una función pura para poder probarla entera.

/// La bandera de la entrada de arranque con Windows (ver `platform::autostart`).
#[cfg(any(windows, test))]
pub const AL_ENCENDER: &str = "--autostart";

/// Abre los ajustes siempre, venga de donde venga.
pub const AJUSTES: &str = "--settings";

/// Lo que dice Windows de cómo ha arrancado la copia de la Store.
///
/// La copia de la Store no puede llevar `--autostart`: su arranque con Windows es la
/// `StartupTask` del paquete, que lanza el exe sin argumentos. Lo que distingue un caso de otro
/// es el tipo de activación que da `AppInstance::GetActivatedEventArgs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum PorLaStore {
    /// No es la copia de la Store (o, en una segunda instancia, no se puede saber): mandan los
    /// argumentos.
    No,
    /// Desde el menú Inicio, un acceso directo o la propia Store.
    AMano,
    /// La tarea de inicio del paquete, al iniciar sesión.
    AlIniciarSesion,
    /// Windows no ha contestado, o ha dicho otra cosa.
    NoSeSabe,
}

/// Todo lo que cuenta para decidir, ya leído.
pub struct Arranque<'a> {
    /// Los argumentos, sin el nombre del programa.
    pub argumentos: &'a [String],
    pub onboarded: bool,
    pub recien_actualizado: bool,
    pub por_la_store: PorLaStore,
    /// Ver `agente::cliente::AvisoDeArranque`.
    pub lo_arranca_un_agente: bool,
}

/// Si este arranque tiene que abrir la ventana de ajustes.
pub fn ensenar_ajustes(arranque: &Arranque) -> bool {
    if arranque.argumentos.iter().any(|arg| arg == AJUSTES) {
        return true;
    }
    // Esto ya estaba antes de la 0.4.7 y sigue igual: la primera vez sale la bienvenida, y
    // después de actualizar salen las novedades, la haya arrancado quien la haya arrancado.
    if !arranque.onboarded || arranque.recien_actualizado {
        return true;
    }
    if arranque.lo_arranca_un_agente {
        return false;
    }
    // Quien la abre a mano no le pasa nada: el menú Inicio, el escritorio, la barra de tareas y
    // el doble clic arrancan el exe sin argumentos. Que llegue uno (`--autostart`, `--capture`,
    // `--cancel`, `--crono`...) quiere decir que la ha arrancado otro programa.
    if !arranque.argumentos.is_empty() {
        return false;
    }
    match arranque.por_la_store {
        PorLaStore::No | PorLaStore::AMano => true,
        // Si Windows no dice cómo la ha arrancado, se queda en la bandeja, como antes de la
        // 0.4.7. Equivocarse por este lado es lo de siempre; equivocarse por el otro sería
        // enseñar la ventana en cada inicio de sesión a quien la tiene puesta para que arranque
        // sola.
        PorLaStore::AlIniciarSesion | PorLaStore::NoSeSabe => false,
    }
}

/// Lo mismo para una segunda instancia, que solo le pasa sus argumentos a la que ya estaba
/// abierta: ni la bienvenida ni las novedades son cosa suya, y su tipo de activación no llega.
pub fn la_segunda_abre_los_ajustes(argumentos: &[String], lo_arranca_un_agente: bool) -> bool {
    ensenar_ajustes(&Arranque {
        argumentos,
        onboarded: true,
        recien_actualizado: false,
        por_la_store: PorLaStore::No,
        lo_arranca_un_agente,
    })
}

/// Cómo ha arrancado esta copia si es la de la Store. Fuera de un paquete, `No`.
///
/// Microsoft documenta `GetActivatedEventArgs` para las apps de escritorio empaquetadas desde
/// Windows 10 1809, con la tarea de inicio entre los tipos que devuelve («Get activation info
/// for packaged apps», learn.microsoft.com). El manifiesto pide 19041 como mínimo.
#[cfg(windows)]
pub fn por_la_store() -> PorLaStore {
    use windows::ApplicationModel::Activation::ActivationKind;
    use windows::ApplicationModel::AppInstance;

    if !crate::platform::empaquetado::es_msix() {
        return PorLaStore::No;
    }
    match AppInstance::GetActivatedEventArgs().and_then(|activacion| activacion.Kind()) {
        Ok(ActivationKind::Launch) => PorLaStore::AMano,
        Ok(ActivationKind::StartupTask) => PorLaStore::AlIniciarSesion,
        _ => PorLaStore::NoSeSabe,
    }
}

#[cfg(not(windows))]
pub fn por_la_store() -> PorLaStore {
    PorLaStore::No
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(lista: &[&str]) -> Vec<String> {
        lista.iter().map(|arg| arg.to_string()).collect()
    }

    /// Una copia ya estrenada y sin actualizar, que es el caso de todos los días.
    fn de_siempre(argumentos: &[String]) -> Arranque<'_> {
        Arranque {
            argumentos,
            onboarded: true,
            recien_actualizado: false,
            por_la_store: PorLaStore::No,
            lo_arranca_un_agente: false,
        }
    }

    /// Lo que pidió Munito el 14 de noviembre de 2026: «me gustaría que al abrir la app se viera así».
    #[test]
    fn abierta_a_mano_salen_los_ajustes() {
        assert!(ensenar_ajustes(&de_siempre(&[])));
    }

    #[test]
    fn al_encender_el_pc_se_queda_en_la_bandeja() {
        assert!(!ensenar_ajustes(&de_siempre(&args(&[AL_ENCENDER]))));
    }

    /// `--capture`, `--edit` y `--cancel` disparan o cierran una captura desde fuera; si llegan a la
    /// primera instancia, la captura es lo que se ha pedido, no los ajustes. Y `--crono` lo
    /// usa `scripts/cronometrar-atajo.mjs`, que mide con la pantalla libre.
    #[test]
    fn lo_que_llega_de_un_guion_no_abre_nada() {
        for bandera in ["--capture", "--edit", "--cancel", "--crono"] {
            assert!(!ensenar_ajustes(&de_siempre(&args(&[bandera]))), "{bandera}");
        }
    }

    #[test]
    fn con_settings_salen_siempre() {
        assert!(ensenar_ajustes(&de_siempre(&args(&[AJUSTES]))));
        assert!(ensenar_ajustes(&de_siempre(&args(&[AL_ENCENDER, AJUSTES]))));
        let mut por_un_agente = de_siempre(&[]);
        por_un_agente.lo_arranca_un_agente = true;
        let a_mano = args(&[AJUSTES]);
        por_un_agente.argumentos = &a_mano;
        assert!(ensenar_ajustes(&por_un_agente));
    }

    #[test]
    fn la_arranca_un_agente_y_no_se_ve_nada() {
        let mut arranque = de_siempre(&[]);
        arranque.lo_arranca_un_agente = true;
        assert!(!ensenar_ajustes(&arranque));
    }

    /// La bienvenida y las novedades ganan a todo, como antes de la 0.4.7: una actualización
    /// hecha con la app cerrada se anuncia aunque el siguiente arranque sea el de Windows.
    #[test]
    fn la_bienvenida_y_las_novedades_salen_igual() {
        let al_encender = args(&[AL_ENCENDER]);
        let mut estrenando = de_siempre(&al_encender);
        estrenando.onboarded = false;
        assert!(ensenar_ajustes(&estrenando));

        let mut actualizada = de_siempre(&al_encender);
        actualizada.recien_actualizado = true;
        assert!(ensenar_ajustes(&actualizada));

        let mut por_un_agente = de_siempre(&[]);
        por_un_agente.onboarded = false;
        por_un_agente.lo_arranca_un_agente = true;
        assert!(ensenar_ajustes(&por_un_agente));
    }

    #[test]
    fn la_de_la_store_mira_como_la_han_arrancado() {
        let mut arranque = de_siempre(&[]);
        arranque.por_la_store = PorLaStore::AMano;
        assert!(ensenar_ajustes(&arranque));
        arranque.por_la_store = PorLaStore::AlIniciarSesion;
        assert!(!ensenar_ajustes(&arranque));
    }

    /// Si Windows no contesta, la de la Store hace lo de antes: bandeja.
    #[test]
    fn la_de_la_store_sin_respuesta_se_queda_en_la_bandeja() {
        let mut arranque = de_siempre(&[]);
        arranque.por_la_store = PorLaStore::NoSeSabe;
        assert!(!ensenar_ajustes(&arranque));
        arranque.recien_actualizado = true;
        assert!(ensenar_ajustes(&arranque), "las novedades salen igual");
    }

    #[test]
    fn la_segunda_instancia() {
        assert!(la_segunda_abre_los_ajustes(&[], false), "abrirla a mano otra vez, como siempre");
        assert!(la_segunda_abre_los_ajustes(&args(&[AJUSTES]), false));
        assert!(!la_segunda_abre_los_ajustes(&args(&[AL_ENCENDER]), false));
        assert!(!la_segunda_abre_los_ajustes(&[], true));
    }

    /// Fuera de un paquete, la Store no cuenta (los `cargo test` nunca corren empaquetados).
    #[cfg(windows)]
    #[test]
    fn sin_paquete_no_hay_store() {
        assert_eq!(por_la_store(), PorLaStore::No);
    }
}
