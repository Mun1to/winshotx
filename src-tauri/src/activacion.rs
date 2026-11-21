//! Una sesión abierta bajo demanda dura mientras tenga una ventana o trabajo en curso.
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{AppHandle, Manager};

#[derive(Default)]
pub(crate) struct PorEncargo(AtomicUsize);

pub(crate) fn es_por_encargo(app: &AppHandle) -> bool {
    app.try_state::<PorEncargo>().is_some()
}

/// Mantiene viva la sesión al pasar de una ventana a otra o terminar de escribir.
pub(crate) struct Trabajo(AppHandle);

pub(crate) fn trabajando(app: &AppHandle) -> Option<Trabajo> {
    app.try_state::<PorEncargo>()?
        .0
        .fetch_add(1, Ordering::AcqRel);
    Some(Trabajo(app.clone()))
}

impl Drop for Trabajo {
    fn drop(&mut self) {
        if self
            .0
            .state::<PorEncargo>()
            .0
            .fetch_sub(1, Ordering::AcqRel)
            == 1
        {
            salir_si_termino(&self.0);
        }
    }
}

pub(crate) fn salir_si_termino(app: &AppHandle) {
    if !es_por_encargo(app) {
        return;
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let trabajos = &handle.state::<PorEncargo>().0;
        if trabajos.load(Ordering::Acquire) != 0
            || handle.state::<crate::state::AppState>().is_recording()
        {
            return;
        }
        let hay_ventanas =
            handle
                .webview_windows()
                .iter()
                .any(|(label, window)| match label.as_str() {
                    "main" => window.is_visible().unwrap_or(false),
                    crate::windows_mgr::COUNTDOWN_LABEL => false, // La cuenta atrás tiene su Trabajo.
                    crate::miniatura::ETIQUETA => crate::miniatura::hay_miniaturas(),
                    _ => true, // El overlay puede seguir cargando fuera de la pantalla.
                });
        if !hay_ventanas && trabajos.load(Ordering::Acquire) == 0 {
            handle.exit(0);
        }
    });
}
