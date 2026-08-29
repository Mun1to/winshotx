use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::Result;

/// Que pasa justo despues de soltar el raton sobre la region elegida.
/// Son las dos formas de trabajar que pidio la gente: la que deja decidir y la que
/// no pregunta nada. Cualquier otra cosa se puede montar encima de estas dos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum CaptureFlow {
    /// Sale la barra flotante y el usuario elige: copiar, guardar, editar o grabar.
    #[default]
    Toolbar,
    /// La imagen se copia al portapapeles sola y el overlay se cierra. Cero clics.
    Instant,
}


/// En que idioma habla la aplicacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum Language {
    /// El de Windows, si winshotx lo habla; si no, ingles. Es el de fabrica.
    #[default]
    Sistema,
    Es,
    En,
}


/// De que color se pinta la aplicacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum Theme {
    /// El que tenga puesto Windows, y cambiando con el sin reiniciar nada. Es el de
    /// fabrica: una aplicacion que vive en la bandeja no puede ser la unica ventana
    /// blanca de un escritorio oscuro, ni al reves.
    #[default]
    Sistema,
    Claro,
    Oscuro,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub capture_shortcut: String,
    pub record_shortcut: String,
    /// La tecla que se queda con lo que ACABA de pasar. Va aparte del atajo de grabar
    /// porque hace lo contrario: aquel decide que se empieza a grabar, y este se lleva algo
    /// que ya se grabo.
    #[serde(default = "atajo_de_repeticion")]
    pub replay_shortcut: String,
    pub save_directory: String,
    pub capture_flow: CaptureFlow,
    /// Claro, oscuro, o lo que diga Windows.
    pub theme: Theme,
    /// Espannol, ingles, o el idioma de Windows.
    pub language: Language,
    pub copy_after_capture: bool,
    pub open_editor_after_recording: bool,
    pub capture_cursor: bool,
    pub record_audio: bool,
    /// Grabar tambien la voz por el microfono. Con el del sistema puesto, van mezclados.
    #[serde(default)]
    pub record_microphone: bool,
    /// Marcar cada clic con un aro, para que una grabacion se entienda como tutorial.
    #[serde(default)]
    pub highlight_clicks: bool,
    /// Ensennar los atajos que se pulsan. Solo atajos, nunca una tecla suelta.
    #[serde(default)]
    pub highlight_keys: bool,
    pub fps: u32,
    /// Grabar siempre la pantalla en un anillo, para poder quedarse con lo ultimo que paso.
    ///
    /// Cuesta disco y maquina todo el rato, asi que viene apagado y se enciende a mano:
    /// es la unica funcion de winshotx que trabaja cuando nadie se lo ha pedido.
    #[serde(default)]
    pub replay_enabled: bool,
    /// Cuantos segundos guarda el anillo hacia atras.
    #[serde(default = "segundos_de_repeticion")]
    pub replay_seconds: u32,
    pub play_sound: bool,
    pub show_magnifier: bool,
    pub start_with_windows: bool,
    /// Segundos de espera antes de congelar la pantalla. 0 es sin espera, que es lo de
    /// siempre. Existe para poder capturar un menu abierto: pulsar el atajo lo cierra,
    /// asi que la unica forma de fotografiarlo es no capturar en ese instante.
    pub capture_delay_seconds: u32,
    /// Esconder los iconos del escritorio mientras se congela la pantalla. Se devuelven
    /// en cuanto la captura esta hecha: el overlay ya tapa el escritorio, asi que no hace
    /// falta tenerlos escondidos mas tiempo del que dura el disparo.
    pub hide_desktop_icons: bool,
    /// La tecla Impr Pant abre winshotx. Va aparte del atajo normal: se suma, no lo
    /// sustituye, asi que quien tenga el suyo puesto no lo pierde al activar esto.
    pub print_screen_capture: bool,
    /// Quedarse tambien con Win+Mayus+S. Va aparte a proposito: es la unica opcion de toda
    /// la aplicacion que le quita algo al usuario (Win+S, la busqueda), asi que no puede ir
    /// de propina con otra cosa.
    pub take_win_shift_s: bool,
    /// Falso hasta que se termina la bienvenida. Es lo que decide si al abrir la
    /// ventana se ven los cuatro pasos o directamente los ajustes.
    pub onboarded: bool,
    /// La version con la que arranco la app la ultima vez. Comparandola con la actual se
    /// sabe que se acaba de actualizar, venga de donde venga la actualizacion: desde la
    /// propia app, desde winget o reinstalando a mano. Sin esto, actualizar desde los
    /// ajustes reinicia winshotx en la bandeja y no se ve absolutamente nada.
    pub last_version: Option<String>,
    /// Lo que valia `PrintScreenKeyForSnippingEnabled` antes de que le quitaramos la
    /// tecla a la Herramienta de Recortes. Al desactivarlo se devuelve tal cual: la
    /// maquina tiene que quedarse como estaba, no como nos venga bien.
    pub snipping_key_restore: Option<u32>,
    /// Y lo que valia la lista de atajos de la tecla Windows. Misma idea: se guarda lo que
    /// hubiera para devolverlo tal cual, sin pisar las letras que apagara el usuario.
    pub disabled_hotkeys_restore: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            capture_shortcut: "CmdOrCtrl+Shift+2".into(),
            record_shortcut: "CmdOrCtrl+Shift+5".into(),
            replay_shortcut: atajo_de_repeticion(),
            save_directory: default_save_dir(),
            capture_flow: CaptureFlow::Toolbar,
            theme: Theme::Sistema,
            language: Language::Sistema,
            copy_after_capture: true,
            open_editor_after_recording: true,
            capture_cursor: true,
            record_audio: false,
            record_microphone: false,
            highlight_clicks: false,
            highlight_keys: false,
            fps: 30,
            replay_enabled: false,
            replay_seconds: segundos_de_repeticion(),
            play_sound: false,
            show_magnifier: true,
            start_with_windows: false,
            capture_delay_seconds: 0,
            hide_desktop_icons: false,
            print_screen_capture: false,
            take_win_shift_s: false,
            onboarded: false,
            last_version: None,
            snipping_key_restore: None,
            disabled_hotkeys_restore: None,
        }
    }
}

/// Treinta segundos: es lo que dura lo que alguien quiere volver a ver. Menos no llega a
/// coger lo que acaba de pasar, porque quien lo ve tarda unos segundos en reaccionar, y
/// mucho mas es tener el disco dando vueltas para nada.
fn segundos_de_repeticion() -> u32 {
    30
}

/// Sigue la serie de las otras dos: capturar es la 2, grabar la 5.
fn atajo_de_repeticion() -> String {
    "CmdOrCtrl+Shift+6".into()
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
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Settings::default();
    };
    // El Bloc de notas y PowerShell guardan UTF-8 con marca de orden de bytes, y con esos
    // tres bytes delante serde no lee ni la primera llave. Sin quitarlos, editar el archivo
    // a mano borra la configuracion entera sin decir nada.
    let raw = raw.trim_start_matches('\u{feff}');

    let Ok(mut settings) = serde_json::from_str::<Settings>(raw) else {
        // Se aparta antes de que la app lo pise con los valores por defecto: unos ajustes
        // que no se entienden se pueden arreglar a mano, pero no si ya no estan.
        let _ = std::fs::rename(&path, path.with_extension("json.roto"));
        return Settings::default();
    };

    // Quien ya tenia winshotx configurado no esta estrenandolo. Si el archivo viene de
    // una version anterior a la bienvenida no trae la clave, y sin esto le saldrian los
    // cuatro pasos a todo el mundo al actualizar.
    let decidido = serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|valor| valor.get("onboarded").cloned())
        .is_some();
    if !decidido {
        settings.onboarded = true;
    }
    settings
}

/// Si este arranque viene de una actualizacion, mirando lo que quedo guardado la ultima
/// vez. `onboarded` separa actualizar de estrenar: en una instalacion nueva no hay
/// version anterior con la que comparar y la ventana ya se abre sola con la bienvenida.
///
/// Que `guardada` sea `None` cuenta como actualizacion: significa que los ajustes vienen
/// de una version que todavia no apuntaba esto, o sea que se ha actualizado desde ella.
pub fn viene_de_actualizar(guardada: Option<&str>, actual: &str, onboarded: bool) -> bool {
    onboarded && guardada != Some(actual)
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<()> {
    let path = config_path(app)?;
    std::fs::write(path, serde_json::to_string_pretty(settings)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un `settings.json` escrito por una version anterior no trae las claves nuevas. Sin
    /// el `default` del contenedor, serde falla ahi y `load` aparta el archivo entero y
    /// devuelve los valores de fabrica: quien actualice pierde sus atajos sin enterarse.
    #[test]
    fn unos_ajustes_de_antes_del_temporizador_se_leen_enteros() {
        let viejo = r#"{
            "captureShortcut": "Alt+KeyX",
            "saveDirectory": "D:\\capturas",
            "showMagnifier": false
        }"#;
        let settings: Settings = serde_json::from_str(viejo).expect("no se ha podido leer");

        assert_eq!(settings.capture_shortcut, "Alt+KeyX");
        assert_eq!(settings.save_directory, "D:\\capturas");
        assert!(!settings.show_magnifier);
        assert_eq!(settings.capture_delay_seconds, 0, "el temporizador nace apagado");
        assert!(!settings.hide_desktop_icons, "los iconos no se tocan sin pedirlo");
    }

    /// Las tres opciones que ofrece la interfaz tienen que caber en el tipo del campo.
    /// Es un `u32`: un valor negativo o con decimales no llegaria nunca hasta aqui.
    #[test]
    fn las_esperas_que_ofrece_la_interfaz_se_guardan_y_se_releen() {
        for segundos in [0u32, 3, 5] {
            let settings = Settings {
                capture_delay_seconds: segundos,
                ..Settings::default()
            };
            let json = serde_json::to_string(&settings).expect("no se ha podido escribir");
            let vuelta: Settings = serde_json::from_str(&json).expect("no se ha podido leer");
            assert_eq!(vuelta.capture_delay_seconds, segundos);
        }
    }

    /// Los cuatro casos de abrir los ajustes solos al arrancar. El que de verdad importa
    /// es el tercero: unos ajustes guardados por una version que no apuntaba `lastVersion`
    /// tienen que contar como actualizacion, porque si no, la primera vez que alguien
    /// actualiza a una version con esto dentro no se le abre nada y parece que no funciona.
    #[test]
    fn solo_se_abre_sola_cuando_de_verdad_se_ha_actualizado() {
        assert!(
            !viene_de_actualizar(Some("0.1.12"), "0.1.12", true),
            "la misma version de siempre no es ninguna novedad"
        );
        assert!(
            viene_de_actualizar(Some("0.1.11"), "0.1.12", true),
            "cambiar de version es lo que hay que anunciar"
        );
        assert!(
            viene_de_actualizar(None, "0.1.12", true),
            "sin version guardada, los ajustes vienen de una version anterior a esto"
        );
        assert!(
            !viene_de_actualizar(None, "0.1.12", false),
            "recien instalada ya se abre la bienvenida: no hay nada que actualizar"
        );
    }
}
