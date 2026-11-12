//! Las órdenes del canal hechas de verdad, sobre lo que la app ya sabe hacer: la lista de
//! pantallas y ventanas, la foto de la captura moderna, `recorder::start`, el «guardar sin
//! editor» (`recorder::parar_sin_editor`), `exporter::export` y el editor.
//!
//! Corre en el hilo de cada conexión del canal, nunca en el del bucle de eventos: puede
//! esperar a que una grabación arranque o a que una exportación acabe sin colgar la app.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

use crate::agente::ordenes::{
    self, Caja, Captura, Donde, Estado, Exportado, Hace, OpcionesDeGrabar, Pantalla, Parada, PedidoDeExportar, Ventana,
};
use crate::agente::traduccion;
use crate::agente::tuberia::Manejador;
use crate::capture::Rect;
use crate::error::AppError;
use crate::state::AppState;

/// Lo que contesta la tubería: cada línea, por las órdenes, sobre esta app.
pub fn manejador(app: AppHandle) -> Manejador {
    let hace = EnLaApp { app };
    Arc::new(move |linea: &str| ordenes::atender(linea, &hace))
}

pub struct EnLaApp {
    app: AppHandle,
}

/// Lo que hizo `maxSeconds` con la última grabación que pidió un agente: un `stop` que llega
/// después tiene que devolver esa sesión, no «no se está grabando nada».
enum Tope {
    Nada,
    /// Llegó al tope y se está guardando: un `stop` que llegue ahora espera a que acabe.
    Parando(String),
    Parada(Parada),
}

struct DelAgente {
    /// La última sesión que empezó un agente.
    ultima: Option<String>,
    tope: Tope,
}

static DEL_AGENTE: Mutex<DelAgente> = Mutex::new(DelAgente { ultima: None, tope: Tope::Nada });

/// Lo que se espera, como mucho, a que se guarde una grabación parada por su tope.
const ESPERA_AL_TOPE: Duration = Duration::from_secs(300);

fn rect(caja: Caja) -> Rect {
    Rect { x: caja.x, y: caja.y, width: caja.ancho, height: caja.alto }
}

fn segundos(session: &crate::record::SessionData) -> f64 {
    session.duration_ms() as f64 / 1000.0
}

fn sesion_que_no_esta(id: &str) -> String {
    format!(
        "There is no session \"{id}\". Use the id that \"record\" or \"stop\" returned; winshotx keeps sessions for a day."
    )
}

impl Hace for EnLaApp {
    fn estado(&self) -> Result<Estado, String> {
        let state = self.app.state::<AppState>();
        let grabando = state.recording.lock().as_ref().map(|r| r.id.clone());
        Ok(Estado { version: self.app.package_info().version.to_string(), grabando })
    }

    fn pantallas(&self) -> Result<Vec<Pantalla>, String> {
        let monitores = crate::capture::monitors().map_err(|e| format!("Could not list the screens ({e})."))?;
        Ok(monitores
            .into_iter()
            .map(|m| Pantalla {
                indice: m.id,
                caja: Caja { x: m.x, y: m.y, ancho: m.width, alto: m.height },
                escala: m.scale,
                principal: m.is_primary,
                nombre: m.label,
            })
            .collect())
    }

    fn ventanas(&self) -> Result<Vec<Ventana>, String> {
        Ok(ventanas_visibles())
    }

    fn capturar(&self, donde: &Donde, salida: Option<&Path>) -> Result<Captura, String> {
        let imagen = match donde.ventana {
            // La ventana misma, aunque algo la tape. Si Windows no la deja fotografiar (algunas
            // con la tarjeta gráfica salen en negro o fallan), lo que se ve de ella en pantalla.
            Some(id) => match foto_de_ventana(id) {
                Ok(imagen) => imagen,
                Err(error) => {
                    eprintln!("[agente] la ventana {id} no se deja fotografiar ({error}): va lo que se ve");
                    foto_de_caja(donde.caja)?
                }
            },
            None => foto_de_caja(donde.caja)?,
        };
        let ruta = match salida {
            Some(ruta) => {
                if let Some(carpeta) = ruta.parent() {
                    std::fs::create_dir_all(carpeta)
                        .map_err(|e| format!("Could not create the folder {} ({e}).", carpeta.display()))?;
                }
                ruta.to_path_buf()
            }
            None => {
                let carpeta = PathBuf::from(self.app.state::<AppState>().settings.read().save_directory.clone());
                crate::archivos::destino(&carpeta, "png")
                    .map_err(|e| format!("Could not use the captures folder {} ({e}).", carpeta.display()))?
            }
        };
        let (ancho, alto) = imagen.dimensions();
        crate::encode::png::save(&imagen, &ruta, ancho, alto)
            .map_err(|e| format!("Could not save {} ({e}).", ruta.display()))?;
        Ok(Captura { archivo: ruta, ancho, alto })
    }

    fn grabar(&self, donde: &Donde, opciones: &OpcionesDeGrabar) -> Result<String, String> {
        let state = self.app.state::<AppState>();
        if let Some(id) = state.recording.lock().as_ref().map(|r| r.id.clone()) {
            return Err(format!("A recording is already running (session {id}). Send \"stop\" first."));
        }
        let ajustes = state.settings.read().clone();
        let peticion = traduccion::opciones_de_grabar(opciones, &ajustes, donde.ventana);
        let info = crate::recorder::start(&self.app, rect(donde.caja), peticion).map_err(|e| match e {
            AppError::Msg(m) if m.contains("arrancando") => {
                "Another recording is starting right now. Wait a moment and check \"status\".".to_string()
            }
            AppError::Msg(m) if m.contains("en curso") => "A recording is already running. Send \"stop\" first.".to_string(),
            otro => format!("winshotx could not start recording ({otro})."),
        })?;
        {
            let mut del_agente = DEL_AGENTE.lock();
            del_agente.ultima = Some(info.id.clone());
            del_agente.tope = Tope::Nada;
        }
        if let Some(tope) = opciones.max_segundos {
            vigilar_tope(self.app.clone(), info.id.clone(), tope);
        }
        Ok(info.id)
    }

    fn parar(&self) -> Result<Parada, String> {
        match crate::recorder::parar_sin_editor(&self.app, None) {
            Ok(session) => Ok(Parada { sesion: session.id.clone(), segundos: segundos(&session), por_tope: false }),
            Err(AppError::NoRecording) => parada_por_tope().ok_or_else(|| {
                "Nothing is being recorded. If you started a recording, it was already stopped (from the screen, or by its maxSeconds); export it with the session id that \"record\" returned.".to_string()
            }),
            Err(error) => Err(format!("winshotx could not save the recording ({error}).")),
        }
    }

    fn exportar(&self, pedido: &PedidoDeExportar) -> Result<Exportado, String> {
        let state = self.app.state::<AppState>();
        if state.recording.lock().as_ref().is_some_and(|r| r.id == pedido.sesion) {
            return Err(format!("Session {} is still recording. Send \"stop\" first.", pedido.sesion));
        }
        let session = crate::exporter::load_session(&self.app, &pedido.sesion).map_err(|_| sesion_que_no_esta(&pedido.sesion))?;
        let presets = state.settings.read().presets.clone();
        let peticion = traduccion::peticion_de_exportar(&session, pedido, &presets)?;
        let duracion = traduccion::segundos_exportados(&session, &peticion);
        let hecho = crate::exporter::export(&self.app, peticion)
            .map_err(|e| format!("winshotx could not export session {} ({e}).", pedido.sesion))?;
        Ok(Exportado { archivo: PathBuf::from(hecho.path), segundos: duracion, bytes: hecho.bytes })
    }

    fn abrir(&self, sesion: &str) -> Result<(), String> {
        let state = self.app.state::<AppState>();
        if state.recording.lock().as_ref().is_some_and(|r| r.id == sesion) {
            return Err(format!("Session {sesion} is still recording. Send \"stop\" first."));
        }
        // El editor lee la sesión de `state.sessions`: si la app se reinició desde que se
        // grabó, se vuelve a traer del disco.
        let guardada = state.sessions.read().get(sesion).cloned();
        let session = match guardada {
            Some(s) => s,
            None => {
                let s = crate::exporter::load_session(&self.app, sesion).map_err(|_| sesion_que_no_esta(sesion))?;
                state.sessions.write().insert(s.id.clone(), s.clone());
                s
            }
        };
        crate::recorder::al_editor(&self.app, &session).map_err(|e| format!("winshotx could not open the editor ({e})."))
    }
}

/// La sesión que paró `maxSeconds`, si es la última que empezó un agente. Si se está guardando
/// todavía, espera a que acabe.
fn parada_por_tope() -> Option<Parada> {
    let empezado = std::time::Instant::now();
    loop {
        {
            let mut del_agente = DEL_AGENTE.lock();
            let ultima = del_agente.ultima.clone();
            let es_la_ultima = |id: &str| ultima.as_deref() == Some(id);
            let (parada, guardandose) = match &del_agente.tope {
                Tope::Parada(p) => (es_la_ultima(&p.sesion), false),
                Tope::Parando(id) => (false, es_la_ultima(id)),
                Tope::Nada => (false, false),
            };
            if parada {
                if let Tope::Parada(p) = std::mem::replace(&mut del_agente.tope, Tope::Nada) {
                    return Some(p);
                }
            }
            if !guardandose || empezado.elapsed() >= ESPERA_AL_TOPE {
                return None;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Para sola la grabación `sesion` cuando lleva `tope` segundos grabados (sin las pausas, como
/// el reloj de la barra), como si se hubiera mandado `stop`.
fn vigilar_tope(app: AppHandle, sesion: String, tope: f64) {
    let tope_ms = (tope * 1000.0).round() as u64;
    let _ = std::thread::Builder::new().name("agente-tope".into()).spawn(move || {
        let lleva = || {
            let state = app.state::<AppState>();
            let guard = state.recording.lock();
            guard.as_ref().filter(|r| r.id == sesion).map(|r| r.elapsed_ms())
        };
        if !esperar_tope(tope_ms, Duration::from_millis(100), lleva) {
            return;
        }
        DEL_AGENTE.lock().tope = Tope::Parando(sesion.clone());
        match crate::recorder::parar_sin_editor(&app, Some(&sesion)) {
            Ok(session) => {
                DEL_AGENTE.lock().tope =
                    Tope::Parada(Parada { sesion: session.id.clone(), segundos: segundos(&session), por_tope: true });
            }
            Err(error) => {
                // Lo paró otro justo a la vez (un `stop`, la barra): ese ya tiene su sesión.
                if !matches!(error, AppError::NoRecording) {
                    eprintln!("[agente] no se ha podido parar al llegar a maxSeconds: {error}");
                }
                DEL_AGENTE.lock().tope = Tope::Nada;
            }
        }
    });
}

/// Espera a que `lleva` (lo grabado, en ms) llegue a `tope_ms`: `true` si llega y `false` si
/// antes deja de haber grabación (la paró otro). Mira cada `cada`, o antes si falta menos.
pub(crate) fn esperar_tope(tope_ms: u64, cada: Duration, lleva: impl Fn() -> Option<u64>) -> bool {
    loop {
        match lleva() {
            None => return false,
            Some(ms) if ms >= tope_ms => return true,
            Some(ms) => std::thread::sleep(cada.min(Duration::from_millis(tope_ms - ms))),
        }
    }
}

/// Si la ventana deja pasar el ratón (`WS_EX_TRANSPARENT`): las capas de NVIDIA, Discord, la
/// Game Bar... Están encima de todo y no se ven; como en `capture::window_rects`, no cuentan.
fn deja_pasar_el_raton(hwnd: u32) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TRANSPARENT};
    let estilo = unsafe { GetWindowLongPtrW(HWND(hwnd as isize as *mut _), GWL_EXSTYLE) } as u32;
    estilo & WS_EX_TRANSPARENT.0 != 0
}

/// El nombre del ejecutable de ese proceso, `msedge.exe`. Vacío si no se deja mirar (los del
/// sistema, con más permisos).
fn nombre_del_proceso(pid: u32) -> String {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let Ok(proceso) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut bufer = [0u16; 1024];
        let mut largo = bufer.len() as u32;
        let leido = QueryFullProcessImageNameW(proceso, PROCESS_NAME_WIN32, PWSTR(bufer.as_mut_ptr()), &mut largo);
        let _ = CloseHandle(proceso);
        if leido.is_err() {
            return String::new();
        }
        let ruta = String::from_utf16_lossy(&bufer[..largo as usize]);
        Path::new(&ruta).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(ruta)
    }
}

/// Las ventanas que se pueden grabar o capturar, de delante a atrás (el orden de
/// `EnumWindows`), con los mismos filtros que el ajuste automático del overlay
/// (`capture::window_rects`) y, además, solo las que tienen título: una ventana sin título no
/// se puede pedir por su nombre y casi nunca es lo que se quiere grabar.
fn ventanas_visibles() -> Vec<Ventana> {
    let Ok(todas) = xcap::Window::all() else {
        return Vec::new();
    };
    let propio = std::process::id();
    let mut procesos: HashMap<u32, String> = HashMap::new();
    todas
        .iter()
        .filter(|w| w.pid().map(|pid| pid != propio).unwrap_or(true))
        .filter(|w| !w.is_minimized().unwrap_or(true))
        .filter(|w| !w.id().map(deja_pasar_el_raton).unwrap_or(false))
        .filter_map(|w| {
            let titulo = w.title().ok()?.trim().to_string();
            let (ancho, alto) = (w.width().ok()?, w.height().ok()?);
            if titulo.is_empty() || ancho < 40 || alto < 40 {
                return None;
            }
            let proceso = w
                .pid()
                .ok()
                .map(|pid| procesos.entry(pid).or_insert_with(|| nombre_del_proceso(pid)).clone())
                .unwrap_or_default();
            Some(Ventana {
                id: w.id().ok()? as isize,
                titulo,
                proceso,
                caja: Caja { x: w.x().ok()?, y: w.y().ok()?, ancho, alto },
            })
        })
        .collect()
}

/// La foto de UNA ventana, aunque algo la tape (`PrintWindow` con el contenido de la tarjeta
/// gráfica, por xcap).
fn foto_de_ventana(id: isize) -> Result<image::RgbaImage, String> {
    let todas = xcap::Window::all().map_err(|e| e.to_string())?;
    let ventana = todas
        .into_iter()
        .find(|w| w.id().is_ok_and(|v| v as isize == id))
        .ok_or_else(|| format!("No visible window has id {id}: it may have closed. List them with \"windows\"."))?;
    ventana.capture_image().map_err(|e| e.to_string())
}

/// La foto de un trozo del escritorio: se fotografían las pantallas que toca (por la captura
/// moderna, la misma de los vídeos), se juntan y se recorta. Sin overlay ni sonido.
fn foto_de_caja(caja: Caja) -> Result<image::RgbaImage, String> {
    let monitores = crate::capture::monitors().map_err(|e| format!("Could not list the screens ({e})."))?;
    let tocadas: Vec<usize> = monitores
        .iter()
        .filter(|m| caja.cruce(&Caja { x: m.x, y: m.y, ancho: m.width, alto: m.height }).is_some())
        .map(|m| m.id as usize)
        .collect();
    if tocadas.is_empty() {
        return Err("That region is outside every screen. See \"screens\".".into());
    }
    let fotos = crate::capture::freeze_monitors(&tocadas).map_err(|e| format!("Could not capture the screen ({e})."))?;
    let (lienzo, marco) = crate::capture::stitch_all(&fotos).map_err(|e| format!("Could not capture the screen ({e})."))?;
    let dentro = caja
        .cruce(&Caja { x: marco.x, y: marco.y, ancho: marco.width, alto: marco.height })
        .ok_or_else(|| "That region is outside every screen. See \"screens\".".to_string())?;
    Ok(image::imageops::crop_imm(
        &lienzo,
        (dentro.x - marco.x) as u32,
        (dentro.y - marco.y) as u32,
        dentro.ancho,
        dentro.alto,
    )
    .to_image())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn el_tope_llega_cuando_lo_grabado_pasa_de_los_segundos_pedidos() {
        let lleva = Cell::new(0u64);
        let mirado = Cell::new(0u32);
        let llegado = esperar_tope(300, Duration::from_millis(1), || {
            mirado.set(mirado.get() + 1);
            lleva.set(lleva.get() + 100);
            Some(lleva.get())
        });
        assert!(llegado);
        assert_eq!(mirado.get(), 3, "a los 300 ms y no antes");
    }

    #[test]
    fn si_la_paran_antes_el_tope_no_hace_nada() {
        let veces = Cell::new(0u32);
        let llegado = esperar_tope(10_000, Duration::from_millis(1), || {
            veces.set(veces.get() + 1);
            (veces.get() < 3).then_some(50)
        });
        assert!(!llegado);
    }

    /// La lista de ventanas de verdad se puede pedir sin pantalla: solo lee lo que hay. Ninguna
    /// sin título ni de este proceso, y todas con su programa si se deja leer.
    #[test]
    fn las_ventanas_de_verdad_tienen_titulo_y_no_son_de_winshotx() {
        for v in ventanas_visibles() {
            assert!(!v.titulo.is_empty() && v.caja.ancho >= 40 && v.caja.alto >= 40, "{v:?}");
            assert_ne!(v.id, 0);
        }
        // El nombre de este mismo proceso se lee.
        assert!(nombre_del_proceso(std::process::id()).to_lowercase().ends_with(".exe"));
    }
}
