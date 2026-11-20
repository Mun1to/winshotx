//! Lo de la captura con scroll que no depende de cómo se fotografía ni de cómo se gira la
//! rueda: quién manda parar, lo que se le cuenta a la barra, la zona dentro de su pantalla y
//! la entrega al editor. Las fotos, la rueda, el ratón, Esc y la barra son de `en_vivo` en
//! Windows y de `linux` en Linux.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(windows)]
use super::en_vivo::{abrir_barra, capturar};
#[cfg(target_os = "linux")]
use super::linux::{abrir_barra, capturar};
use super::{BARRA_LABEL, EVENTO_PROGRESO};
use crate::capture::{MonitorInfo, Rect};
use crate::error::{AppError, Result};

pub(super) const SEGUIR: u8 = 0;
pub(super) const TERMINAR: u8 = 1;
pub(super) const DESCARTAR: u8 = 2;

/// La orden de la captura que está en marcha, si hay una. Solo puede haber una.
static EN_MARCHA: Mutex<Option<Arc<AtomicU8>>> = Mutex::new(None);

/// Lo que mide la barra, en píxeles lógicos.
pub(super) const BARRA_ANCHO: f64 = 420.0;
pub(super) const BARRA_ALTO: f64 = 52.0;

/// Cómo va la captura, para la barra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progreso {
    /// El alto de la imagen cosida, en píxeles.
    alto: u32,
    /// `bajando` (sola), `tu` (quien captura desplaza), `perdida` (ha bajado demasiado
    /// de golpe), `no-baja` (la rueda no mueve nada) o `cosiendo` (terminando).
    estado: &'static str,
}

/// La primera orden es la que vale: «Terminar» seguido de un clic en la X no tira lo que
/// ya se está guardando.
pub(super) fn pedir(orden: u8) {
    if let Some(actual) = EN_MARCHA.lock().as_ref() {
        let _ = actual.compare_exchange(SEGUIR, orden, Ordering::SeqCst, Ordering::SeqCst);
    }
}

/// Si hay una captura en marcha, le pide que termine y dice que la había.
#[cfg(target_os = "linux")]
pub(super) fn terminar_si_hay() -> bool {
    match EN_MARCHA.lock().as_ref() {
        Some(actual) => {
            let _ = actual.compare_exchange(SEGUIR, TERMINAR, Ordering::SeqCst, Ordering::SeqCst);
            true
        }
        None => false,
    }
}

pub(super) fn iniciar(app: &AppHandle, region: Rect) -> Result<()> {
    if region.width < 40 || region.height < 80 {
        return Err(AppError::Msg(
            "Esa zona es muy pequeña para desplazarla: recorta algo más grande.".into(),
        ));
    }
    let monitores = crate::capture::monitors()?;
    let (cx, cy) = region.center();
    let monitor = monitores
        .iter()
        .find(|m| m.contains(cx, cy))
        .cloned()
        .ok_or_else(|| AppError::Msg("la selección queda fuera de la pantalla".into()))?;
    let region = dentro_de(region, &monitor);

    let orden = Arc::new(AtomicU8::new(SEGUIR));
    {
        let mut en_marcha = EN_MARCHA.lock();
        if en_marcha.is_some() {
            return Err(AppError::Msg("Ya hay una captura con scroll en marcha.".into()));
        }
        *en_marcha = Some(orden.clone());
    }

    crate::windows_mgr::close_overlays(app);
    // Sin barra se puede capturar igual (Esc termina), así que no se para por eso.
    if let Err(e) = abrir_barra(app, region) {
        eprintln!("[scroll] no se ha podido abrir la barra: {e}");
    }

    let handle = app.clone();
    let lanzado = std::thread::Builder::new()
        .name("winshotx-scroll".into())
        .spawn(move || {
            let resultado = capturar(&handle, region, &monitor, &orden);
            cerrar_barra(&handle);
            *EN_MARCHA.lock() = None;
            match resultado {
                Ok(Some(imagen)) => entregar(&handle, &imagen, region),
                Ok(None) => {}
                Err(e) => eprintln!("[scroll] la captura ha fallado: {e}"),
            }
        });
    if let Err(e) = lanzado {
        *EN_MARCHA.lock() = None;
        cerrar_barra(app);
        return Err(AppError::Msg(format!("no se ha podido empezar la captura: {e}")));
    }
    Ok(())
}

/// La zona recortada a su pantalla: la foto es de una pantalla y no puede salirse.
fn dentro_de(region: Rect, monitor: &MonitorInfo) -> Rect {
    let x0 = region.x.max(monitor.x);
    let y0 = region.y.max(monitor.y);
    let x1 = (region.x + region.width as i32).min(monitor.x + monitor.width as i32);
    let y1 = (region.y + region.height as i32).min(monitor.y + monitor.height as i32);
    Rect {
        x: x0,
        y: y0,
        width: (x1 - x0).max(1) as u32,
        height: (y1 - y0).max(1) as u32,
    }
}

/// Le cuenta a la barra cómo va, sin inundarla: cuando cambia algo, y si no, dos veces
/// por segundo (la primera puede llegar antes de que la barra haya cargado).
pub(super) struct Aviso<'a> {
    app: &'a AppHandle,
    ultimo: Option<(u32, &'static str)>,
    cuando: Instant,
}

impl<'a> Aviso<'a> {
    pub(super) fn nuevo(app: &'a AppHandle) -> Self {
        Self {
            app,
            ultimo: None,
            cuando: Instant::now(),
        }
    }

    pub(super) fn contar(&mut self, alto: u32, estado: &'static str) {
        let ahora = (alto, estado);
        if self.ultimo == Some(ahora) && self.cuando.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.ultimo = Some(ahora);
        self.cuando = Instant::now();
        let _ = self
            .app
            .emit_to(BARRA_LABEL, EVENTO_PROGRESO, Progreso { alto, estado });
    }
}

/// La imagen al editor, como una foto cualquiera: desde ahí se copia, se guarda o se
/// anota, y guardar va por el mismo camino que el resto.
fn entregar(app: &AppHandle, imagen: &RgbaImage, region: Rect) {
    let con_sonido = app
        .state::<crate::state::AppState>()
        .settings
        .read()
        .play_sound;
    // Grabando, sin obturador: se colaria en el sonido del video (16-nov).
    let grabando = app.state::<crate::state::AppState>().is_recording();
    if con_sonido && !grabando {
        crate::platform::sonido::obturador();
    }
    let zona = Rect {
        x: region.x,
        y: region.y,
        width: imagen.width(),
        height: imagen.height(),
    };
    let abierto = crate::recorder::session_from_image(app, imagen, zona)
        .and_then(|sesion| crate::windows_mgr::open_editor(app, &sesion.id));
    if let Err(e) = abierto {
        eprintln!("[scroll] no se ha podido abrir la captura en el editor: {e}");
    }
}

fn cerrar_barra(app: &AppHandle) {
    if let Some(ventana) = app.get_webview_window(BARRA_LABEL) {
        let _ = ventana.close();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_zona_no_se_sale_de_su_pantalla() {
        let monitor = MonitorInfo {
            id: 0,
            label: "PRUEBA".into(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
            is_primary: true,
        };
        let r = dentro_de(Rect { x: 1800, y: -10, width: 400, height: 500 }, &monitor);
        assert_eq!((r.x, r.y, r.width, r.height), (1800, 0, 120, 490));
    }

    #[test]
    fn la_primera_orden_es_la_que_vale() {
        let orden = Arc::new(AtomicU8::new(SEGUIR));
        *EN_MARCHA.lock() = Some(orden.clone());
        pedir(TERMINAR);
        pedir(DESCARTAR);
        assert_eq!(orden.load(Ordering::SeqCst), TERMINAR);
        *EN_MARCHA.lock() = None;
        // En Linux, la tecla de capturar: termina la que haya, y sin ninguna no hace nada.
        // Va aquí y no en su prueba porque las dos tocan la misma captura en marcha.
        #[cfg(target_os = "linux")]
        {
            assert!(!terminar_si_hay(), "sin captura en marcha no hay nada que terminar");
            let orden = Arc::new(AtomicU8::new(SEGUIR));
            *EN_MARCHA.lock() = Some(orden.clone());
            assert!(terminar_si_hay());
            assert_eq!(orden.load(Ordering::SeqCst), TERMINAR);
            pedir(DESCARTAR);
            assert_eq!(orden.load(Ordering::SeqCst), TERMINAR, "lo que ya se guarda no se tira");
            *EN_MARCHA.lock() = None;
        }
    }
}
