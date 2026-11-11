//! Los estilos guardados con nombre (D2 del plan de CleanShot X y Screen Studio).
//!
//! «Guardar estilo como…» en el editor guarda el fondo, el zoom, el puntero y los clics de
//! una exportación con un nombre, y una lista los vuelve a poner. Viven en los ajustes, en
//! `Settings::presets`.
//!
//! **Rust no mira lo que hay dentro de un estilo.** Lo escribe y lo lee el editor
//! (`src/lib/presets.ts`), que es quien sabe qué es cada campo del panel, y quien rellena
//! con lo de fábrica lo que le falte a un estilo guardado con una versión vieja. Aquí solo
//! se guarda la lista, con un nombre de verdad cada uno y sin repetirse.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::Result;
use crate::state::AppState;

/// Un estilo con su nombre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub nombre: String,
    /// El estilo tal cual lo guarda el editor. Ver `src/lib/presets.ts`.
    #[serde(default)]
    pub estilo: serde_json::Value,
}

/// Cuántos estilos caben. Nadie usa cincuenta, y una lista sin tope es un archivo de
/// ajustes que crece sin que nadie lo vea.
pub const MAXIMO: usize = 50;

/// Lo que mide un nombre como mucho, en letras.
const NOMBRE_MAXIMO: usize = 60;

/// El aviso de que la lista ha cambiado, para que la ventana de ajustes enseñe la nueva.
pub const AVISO: &str = "winshotx://presets";

/// La lista tal cual se guarda: sin nombres vacíos, con el nombre recortado, sin dos con
/// el mismo nombre (sin mirar mayúsculas: «Tutorial» y «tutorial» son el mismo, y gana el
/// último, que es el que se acaba de guardar) y sin pasar de `MAXIMO`.
pub fn limpiar(presets: Vec<Preset>) -> Vec<Preset> {
    let mut salen: Vec<Preset> = Vec::with_capacity(presets.len());
    for mut p in presets {
        let nombre: String = p.nombre.trim().chars().take(NOMBRE_MAXIMO).collect();
        if nombre.is_empty() {
            continue;
        }
        p.nombre = nombre;
        let clave = p.nombre.to_lowercase();
        match salen.iter().position(|s| s.nombre.to_lowercase() == clave) {
            Some(i) => salen[i] = p,
            None => salen.push(p),
        }
    }
    salen.truncate(MAXIMO);
    salen
}

/// Guarda la lista entera de estilos y avisa a las ventanas.
///
/// Va aparte de `set_settings` a propósito: aquel reengancha los atajos y el anillo en cada
/// llamada, y además el editor no tiene los ajustes frescos (se leen al abrirlo). Esto toca
/// solo la lista, sobre lo que haya guardado en ese momento.
#[tauri::command]
pub async fn presets_guardar(app: AppHandle, presets: Vec<Preset>) -> Result<Vec<Preset>> {
    let limpios = limpiar(presets);
    let state = app.state::<AppState>();
    let settings = {
        let mut guard = state.settings.write();
        guard.presets = limpios.clone();
        guard.clone()
    };
    crate::settings::save(&app, &settings)?;
    let _ = app.emit(AVISO, &limpios);
    Ok(limpios)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn preset(nombre: &str, n: i64) -> Preset {
        Preset { nombre: nombre.into(), estilo: serde_json::json!({ "zoom": n }) }
    }

    #[test]
    fn sin_nombre_no_se_guarda_y_el_nombre_va_recortado() {
        let salen = limpiar(vec![preset("   ", 1), preset("  Tutorial  ", 2), preset("", 3)]);
        assert_eq!(salen, vec![preset("Tutorial", 2)]);
    }

    #[test]
    fn dos_con_el_mismo_nombre_gana_el_ultimo_en_su_sitio() {
        let salen = limpiar(vec![preset("Tutorial", 1), preset("Redes", 2), preset("tutorial", 3)]);
        assert_eq!(salen, vec![preset("tutorial", 3), preset("Redes", 2)]);
    }

    #[test]
    fn no_pasa_del_maximo_ni_de_nombres_larguisimos() {
        let muchos: Vec<Preset> = (0..80).map(|i| preset(&format!("estilo {i}"), i)).collect();
        assert_eq!(limpiar(muchos).len(), MAXIMO);
        let largo = "á".repeat(200);
        assert_eq!(limpiar(vec![preset(&largo, 1)])[0].nombre.chars().count(), NOMBRE_MAXIMO);
    }

    /// Lo que manda el editor se lee tal cual, y un estilo sin nada dentro también: lo que
    /// falta lo pone de fábrica el editor al leerlo.
    #[test]
    fn se_lee_lo_que_manda_el_editor() {
        let json = r#"[{"nombre": "Tutorial", "estilo": {"zoom": 2, "fondo": "menta"}},
                       {"nombre": "Viejo"}]"#;
        let leidos: Vec<Preset> = serde_json::from_str(json).unwrap();
        assert_eq!(leidos[0].estilo["fondo"], "menta");
        assert!(leidos[1].estilo.is_null());
    }

    /// Unos ajustes de antes de los estilos se leen con la lista vacía.
    #[test]
    fn unos_ajustes_viejos_no_traen_ninguno() {
        let viejos: crate::settings::Settings = serde_json::from_str("{}").unwrap();
        assert!(viejos.presets.is_empty());
    }
}
