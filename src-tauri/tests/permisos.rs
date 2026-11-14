//! Lo que la página de una ventana le pide a Tauri sobre su propia ventana tiene que estar
//! permitido en `capabilities/`, o falla sin decir nada: la promesa se rechaza, el frontend
//! la calla, y la ventana se queda como estaba.
//!
//! Pasó con los ajustes: `App.tsx` llamaba a `setSize` desde agosto de 2026 y ninguna
//! capacidad lo permitía, así que el alto que mandaba era siempre el de `tauri.conf.json`.
//! Los comentarios decían que mandaba esa línea; mandaba el otro archivo. Se vio el 14 de
//! noviembre (S24), al hacer que la ventana mida lo que mide su pestaña y comprobarlo en una
//! ventana de verdad: en el navegador de pruebas todo cuadraba, en Windows no crecía.

use std::collections::BTreeSet;
use std::path::Path;

/// Los permisos que tiene la ventana `label`, sumando todas las capacidades que la nombran.
fn permisos_de(label: &str) -> BTreeSet<String> {
    let carpeta = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut permisos = BTreeSet::new();
    for entrada in std::fs::read_dir(&carpeta).expect("la carpeta capabilities") {
        let ruta = entrada.unwrap().path();
        if ruta.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let texto = std::fs::read_to_string(&ruta).unwrap();
        let json: serde_json::Value = serde_json::from_str(&texto).unwrap();
        let ventanas = json["windows"].as_array().cloned().unwrap_or_default();
        if !ventanas.iter().any(|v| v.as_str() == Some(label)) {
            continue;
        }
        for p in json["permissions"].as_array().cloned().unwrap_or_default() {
            if let Some(p) = p.as_str() {
                permisos.insert(p.to_string());
            }
        }
    }
    permisos
}

#[test]
fn la_ventana_de_ajustes_puede_cambiar_su_tamanno_y_subirse() {
    let permisos = permisos_de("main");
    for hace_falta in ["core:window:allow-set-size", "core:window:allow-set-position"] {
        assert!(
            permisos.contains(hace_falta),
            "la ventana de ajustes no tiene {hace_falta}: `useVentanaALaMedida` no podría \
             ajustarla y se quedaría con el alto de tauri.conf.json sin dar ningún error"
        );
    }
}

#[test]
fn y_lo_que_lee_de_su_ventana_y_su_pantalla_va_en_el_permiso_de_serie() {
    // `core:default` trae `core:window:default`, que tiene inner_size, outer_size,
    // outer_position, scale_factor y current_monitor: lo que mide `ventanaALaMedida.ts`.
    assert!(permisos_de("main").contains("core:default"));
}
