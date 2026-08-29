//! Los textos que escribe Rust, que son los del menu de la bandeja y poco mas.
//!
//! El resto de la aplicacion los pone la interfaz, con su propio diccionario en
//! `src/lib/textos-en.ts`. Aqui hay cinco frases y no merecen un diccionario: un `match`
//! por campo se lee mejor y no se puede quedar a medias sin que el compilador avise.

use crate::settings::Language;

/// Lo que se ve al pulsar el icono de la bandeja con el boton derecho.
pub struct Menu {
    pub capturar: &'static str,
    pub grabar: &'static str,
    /// Solo sale si el anillo de los ultimos segundos esta encendido.
    pub ultimos: &'static str,
    pub carpeta: &'static str,
    pub ajustes: &'static str,
    pub actualizaciones: &'static str,
    pub salir: &'static str,
}

pub fn menu(idioma: Language) -> Menu {
    match resolver(idioma) {
        Language::En => Menu {
            capturar: "Capture a region",
            grabar: "Record a region",
            ultimos: "Keep the last few seconds",
            carpeta: "Open the shots folder",
            ajustes: "Settings…",
            actualizaciones: "Check for updates…",
            salir: "Quit",
        },
        // El espannol es la respuesta por defecto, tambien para `Sistema`, que aqui ya no
        // deberia llegar: `resolver` lo convierte antes en uno de los dos idiomas reales.
        _ => Menu {
            capturar: "Capturar región",
            grabar: "Grabar región",
            ultimos: "Quedarme con lo último",
            carpeta: "Abrir la carpeta de capturas",
            ajustes: "Ajustes…",
            actualizaciones: "Buscar actualizaciones…",
            salir: "Salir",
        },
    }
}

/// Convierte `Sistema` en el idioma que de verdad toca hablar.
pub fn resolver(idioma: Language) -> Language {
    match idioma {
        Language::Sistema => del_sistema(),
        otro => otro,
    }
}

/// El idioma de Windows, si winshotx lo habla; si no, ingles.
#[cfg(windows)]
fn del_sistema() -> Language {
    use windows::Win32::Globalization::GetUserDefaultUILanguage;

    // Un LANGID lleva el idioma en los diez bits de abajo y la variante del pais en el
    // resto. Nos da igual la variante: el espannol de Mexico y el de Espanna hablan el
    // mismo winshotx, asi que se compara solo el idioma primario. El 0x0A es el espannol.
    const ESPANNOL: u16 = 0x0A;
    let id = unsafe { GetUserDefaultUILanguage() };
    if id & 0x03FF == ESPANNOL {
        Language::Es
    } else {
        Language::En
    }
}

#[cfg(not(windows))]
fn del_sistema() -> Language {
    Language::Es
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Elegir un idioma a mano manda siempre, y `resolver` no puede devolver `Sistema`:
    /// quien lo llama espera un idioma en el que se pueda escribir.
    #[test]
    fn elegir_un_idioma_a_mano_manda_sobre_windows() {
        assert_eq!(resolver(Language::Es), Language::Es);
        assert_eq!(resolver(Language::En), Language::En);
        assert_ne!(resolver(Language::Sistema), Language::Sistema);
    }

    /// El menu tiene que estar entero en los dos idiomas: una entrada suelta en espannol
    /// dentro de un menu en ingles se ve mas que si no estuviera traducido nada.
    #[test]
    fn el_menu_no_se_queda_a_medias() {
        let en = menu(Language::En);
        let es = menu(Language::Es);
        assert_ne!(en.capturar, es.capturar);
        assert_ne!(en.grabar, es.grabar);
        assert_ne!(en.ultimos, es.ultimos);
        assert_ne!(en.carpeta, es.carpeta);
        assert_ne!(en.ajustes, es.ajustes);
        assert_ne!(en.actualizaciones, es.actualizaciones);
        assert_ne!(en.salir, es.salir);
    }
}
