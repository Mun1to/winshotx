//! Los fotogramas de Wayland: el portal `ScreenCast` da el permiso y PipeWire trae la imagen.
//!
//! En Wayland una app no puede leer la pantalla. Se le pide al escritorio por el portal, que
//! pregunta a la persona que pantalla deja grabar, y lo que devuelve es un «nodo» de
//! PipeWire del que salen los fotogramas de esa pantalla entera. De cada uno se recorta la
//! region y se pasa a RGBA, que es lo que espera la cache de siempre.
//!
//! **Una sola pregunta.** El portal devuelve una ficha (`restore_token`) que, presentada la
//! vez siguiente, hace que no vuelva a preguntar. Solo vale una vez: cada respuesta trae una
//! nueva, que se guarda encima de la anterior.
//!
//! **El raton.** Si el escritorio sabe mandarlo aparte (GNOME y KDE: `CursorMode::Metadata`),
//! llega su posicion y su imagen con cada fotograma y se apunta como en X11 y en Windows. Si
//! no (los de wlroots), viene ya dibujado dentro de la imagen y se avisa con `raton_cocido`,
//! para que el exportador no lo dibuje dos veces.
//!
//! **Los fotogramas solo llegan cuando la pantalla cambia,** y al ritmo del escritorio, no al
//! nuestro. Por eso siempre se guarda el ultimo y se suelta cuando toca: si se tirara el que
//! llega demasiado pronto, una pantalla que se queda quieta justo despues se quedaria para
//! siempre con la imagen de antes.
//!
//! **El escritorio puede cortar la grabacion** (GNOME y KDE ensennan un aviso de «se esta
//! compartiendo la pantalla» con su boton de parar). Entonces se avisa por `al_cortarse`,
//! que para la grabacion como si se hubiera pulsado el atajo. PipeWire no lo dice: la
//! pantalla deja de llegar y nada mas. Quien lo dice es el portal, que cierra la sesion
//! (`Sesion`).

use std::cell::RefCell;
use std::io::Cursor;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::{PersistMode, Session};
use futures_channel::oneshot;
use futures_util::future::{select, Either};
use futures_util::StreamExt;
use pipewire as pw;
use pw::spa;
use spa::buffer::meta::MetaCursor;
use spa::buffer::ChunkFlags;
use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use spa::param::video::{VideoFormat, VideoInfoRaw};
use spa::param::ParamType;
use spa::pod::serialize::PodSerializer;
use spa::pod::Pod;
use spa::utils::{Choice, ChoiceEnum, ChoiceFlags, Direction, Fraction, Id, Rectangle, SpaTypes};

use super::{EnMarcha, Fotograma, Marcha, Puntero, Rastro, Reciclados};
use crate::capture::wayland::PantallaGtk;
use crate::capture::{MonitorInfo, Rect};
use crate::error::{AppError, Result};

/// Lo que se espera a que llegue la primera imagen despues de que el escritorio diga que si.
const ESPERA_DE_LA_IMAGEN: Duration = Duration::from_secs(5);

/// Cada cuanto se mira si toca soltar el fotograma guardado, y si hay que parar.
const LATIDO: Duration = Duration::from_millis(8);

/// Cada cuanto se apunta por donde va el raton, como mucho (lo mismo que en X11).
const PASO_DEL_RATON_MS: u64 = 16;

/// Una pantalla que el escritorio deja grabar: su nodo de PipeWire y, si lo dice, donde esta
/// y cuanto mide, en las medidas del escritorio (pixeles logicos).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Fuente {
    pub nodo: u32,
    pub posicion: Option<(i32, i32)>,
    pub medida: Option<(i32, i32)>,
}

/// El orden de los colores dentro de cada pixel que manda el escritorio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Orden {
    /// Azul, verde, rojo y uno que sobra: lo de casi siempre.
    Bgr,
    /// Rojo, verde, azul y uno que sobra.
    Rgb,
}

/// De que pixel del fotograma del escritorio sale cada pixel de la region.
///
/// Lo normal es que sea un recorte tal cual. Las tablas estan por si el escritorio manda la
/// pantalla a otra medida que la de la foto con la que se eligio la region (escalas partidas):
/// entonces cada pixel sale del que le toca por proporcion.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Recorte {
    pub columnas: Vec<u32>,
    pub filas: Vec<u32>,
    /// Las columnas van seguidas: cada fila se copia de un tiron.
    pub seguido: bool,
}

/// `dentro` es la region contada desde la esquina de su pantalla, y `pantalla` lo que mide
/// esa pantalla, las dos en pixeles de la foto; `fotograma` es lo que mide lo que manda el
/// escritorio. `None` si no hay nada que recortar.
pub(crate) fn recorte(dentro: Rect, pantalla: (u32, u32), fotograma: (u32, u32)) -> Option<Recorte> {
    if dentro.width == 0 || dentro.height == 0 || pantalla.0 == 0 || pantalla.1 == 0 || fotograma.0 == 0 || fotograma.1 == 0
    {
        return None;
    }
    let tabla = |desde: i32, cuantos: u32, de: u32, a: u32| -> Vec<u32> {
        (0..cuantos)
            .map(|i| {
                let en_la_foto = f64::from(desde.max(0)) + f64::from(i) + 0.5;
                ((en_la_foto * f64::from(a) / f64::from(de)) as u32).min(a - 1)
            })
            .collect()
    };
    let columnas = tabla(dentro.x, dentro.width, pantalla.0, fotograma.0);
    let filas = tabla(dentro.y, dentro.height, pantalla.1, fotograma.1);
    let seguido = columnas.windows(2).all(|par| par[1] == par[0] + 1);
    Some(Recorte { columnas, filas, seguido })
}

/// Saca la region de un fotograma del escritorio y la deja en RGBA opaco en `destino`.
///
/// `paso` es lo que ocupa cada fila en `origen`, que puede ser mas que cuatro bytes por
/// pixel. `false` si el fotograma es mas corto de lo que dice: entonces no se toca nada.
pub(crate) fn recortar(origen: &[u8], paso: usize, orden: Orden, plan: &Recorte, destino: &mut Vec<u8>) -> bool {
    let (ancho, alto) = (plan.columnas.len(), plan.filas.len());
    let (Some(&ultima_columna), Some(&ultima_fila)) = (plan.columnas.iter().max(), plan.filas.iter().max()) else {
        return false;
    };
    if origen.len() < ultima_fila as usize * paso + (ultima_columna as usize + 1) * 4 {
        return false;
    }
    destino.resize(ancho * alto * 4, 0);
    for (fila, salida) in plan.filas.iter().zip(destino.chunks_exact_mut(ancho * 4)) {
        let inicio = *fila as usize * paso;
        if plan.seguido {
            let desde = inicio + plan.columnas[0] as usize * 4;
            let trozo = &origen[desde..desde + ancho * 4];
            match orden {
                Orden::Bgr => super::bgrx_a_rgba(trozo, salida),
                Orden::Rgb => {
                    salida.copy_from_slice(trozo);
                    for pixel in salida.chunks_exact_mut(4) {
                        pixel[3] = 255;
                    }
                }
            }
        } else {
            for (columna, pixel) in plan.columnas.iter().zip(salida.chunks_exact_mut(4)) {
                let p = &origen[inicio + *columna as usize * 4..][..4];
                match orden {
                    Orden::Bgr => pixel.copy_from_slice(&[p[2], p[1], p[0], 255]),
                    Orden::Rgb => pixel.copy_from_slice(&[p[0], p[1], p[2], 255]),
                }
            }
        }
    }
    true
}

/// Cual de las pantallas que deja grabar el escritorio lleva dentro la region, y que pantalla
/// es de las que conoce winshotx.
///
/// `pantallas` es lo que dice GTK (en las medidas del escritorio, que es como habla el
/// portal) y `monitores` las mismas en pixeles de la foto, que es como viene la region.
pub(crate) fn elegir(
    fuentes: &[Fuente],
    pantallas: &[PantallaGtk],
    monitores: &[MonitorInfo],
    region: Rect,
) -> Option<(u32, MonitorInfo)> {
    let (cx, cy) = region.center();
    let i = monitores.iter().position(|m| m.contains(cx, cy))?;
    let (monitor, pantalla) = (monitores.get(i)?, pantallas.get(i)?);
    let fuente = if fuentes.iter().any(|f| f.posicion.is_some()) {
        fuentes.iter().find(|f| f.posicion == Some((pantalla.x, pantalla.y)))
    } else if fuentes.len() == 1 && monitores.len() == 1 {
        // Los escritorios que no dicen donde esta cada pantalla (los de wlroots).
        fuentes.first()
    } else {
        let de_esa_medida: Vec<&Fuente> =
            fuentes.iter().filter(|f| f.medida == Some((pantalla.ancho, pantalla.alto))).collect();
        match de_esa_medida.as_slice() {
            [unica] => Some(*unica),
            _ => None,
        }
    }?;
    Some((fuente.nodo, monitor.clone()))
}

/// La sesion del portal. Mientras vive, el escritorio ensenna que se esta grabando; se
/// cierra al soltarla.
///
/// La lleva una tarea, que hace dos cosas. Cerrarla cuando esto se suelta. Y avisar si quien
/// la cierra es el escritorio (la persona pulsa «dejar de compartir» en su aviso): entonces
/// la pantalla deja de llegar sin que PipeWire diga nada, y sin ese aviso la grabacion se
/// quedaba abierta y vacia hasta que alguien la parase (visto en GNOME 50, laboratorio de
/// S44: 8 fotogramas en 48 segundos).
struct Sesion {
    /// Soltarlo es la orden de cerrar la sesion.
    _soltar: oneshot::Sender<()>,
    /// Por donde se le dice al hilo de PipeWire que el escritorio la ha cerrado. Se pone
    /// cuando ese hilo existe (`preparar`).
    cortes: Cortes,
}

type Cortes = Arc<parking_lot::Mutex<Option<pw::channel::Sender<Aviso>>>>;

impl Sesion {
    fn vigilada(sesion: Session<'static, Screencast<'static>>) -> Self {
        let (soltar, soltada) = oneshot::channel::<()>();
        let cortes = Cortes::default();
        let avisar = cortes.clone();
        tauri::async_runtime::spawn(async move {
            let la_ha_cerrado_el_escritorio = match sesion.receive_closed().await {
                Ok(cierres) => {
                    let mut cierres = Box::pin(cierres);
                    match select(soltada, cierres.next()).await {
                        Either::Left(_) => false,
                        Either::Right((cierre, _)) => cierre.is_some(),
                    }
                }
                // Sin poder escuchar el cierre, al menos se cierra cuando toque.
                Err(_) => {
                    let _ = soltada.await;
                    false
                }
            };
            if la_ha_cerrado_el_escritorio {
                if let Some(avisos) = avisar.lock().take() {
                    let _ = avisos.send(Aviso::Cortada);
                }
            } else {
                let _ = sesion.close().await;
            }
        });
        Sesion { _soltar: soltar, cortes }
    }
}

/// Lo que devuelve el portal cuando la persona dice que si.
struct Permiso {
    sesion: Sesion,
    conexion: OwnedFd,
    fuentes: Vec<Fuente>,
    ficha: Option<String>,
    raton_aparte: bool,
}

async fn pedir(ficha: Option<String>) -> ashpd::Result<Permiso> {
    // Antes de la primera llamada al portal, o se queda sin nombre para siempre.
    crate::platform::portal::registrar_la_app().await;
    let portal = Screencast::new().await?;
    let raton_aparte = portal
        .available_cursor_modes()
        .await
        .map(|modos| modos.contains(CursorMode::Metadata))
        .unwrap_or(false);
    let modo = if raton_aparte { CursorMode::Metadata } else { CursorMode::Embedded };
    let sesion = portal.create_session().await?;
    portal
        .select_sources(
            &sesion,
            modo,
            SourceType::Monitor.into(),
            // Con varias pantallas se pueden dejar todas de una vez, y despues grabar
            // cualquiera sin que vuelva a preguntar.
            true,
            ficha.as_deref(),
            PersistMode::ExplicitlyRevoked,
        )
        .await?
        .response()?;
    let respuesta = portal.start(&sesion, None).await?.response()?;
    let conexion = portal.open_pipe_wire_remote(&sesion).await?;
    let fuentes = respuesta
        .streams()
        .iter()
        .map(|s| Fuente { nodo: s.pipe_wire_node_id(), posicion: s.position(), medida: s.size() })
        .collect();
    Ok(Permiso {
        sesion: Sesion::vigilada(sesion),
        conexion,
        fuentes,
        ficha: respuesta.restore_token().map(str::to_string),
        raton_aparte,
    })
}

/// Guarda la ficha del permiso donde solo la puede leer su duenno.
///
/// Es una llave: quien la tenga graba la pantalla sin que el escritorio pregunte. Por eso el
/// archivo es de lectura y escritura solo para la persona (600), y la ficha no se escribe
/// nunca en el registro. Si el escritorio la rechaza, pregunta otra vez y da una nueva, que
/// se guarda encima; y si no da ninguna, `preparar` borra la que habia.
fn guardar_la_ficha(ruta: &Path, ficha: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut archivo =
        std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(ruta)?;
    // `mode` solo cuenta al crear el archivo: uno que ya estuviera se cierra tambien.
    archivo.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    archivo.write_all(ficha.as_bytes())
}

/// Lo que se le dice a la persona cuando el portal no deja grabar.
fn por_que_no(error: &ashpd::Error) -> String {
    match error {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => {
            "no se ha dado permiso para grabar la pantalla".into()
        }
        ashpd::Error::PortalNotFound(_) => {
            "este escritorio no deja grabar la pantalla: le falta el portal de grabación (xdg-desktop-portal)".into()
        }
        otro => format!("el escritorio no ha dejado grabar la pantalla: {otro}"),
    }
}

/// Lo que se le manda al hilo de PipeWire.
enum Aviso {
    /// Ya hay reloj: a soltar fotogramas.
    Empezar(Emision),
    /// No se va a grabar: que lo deje.
    Dejarlo,
    /// El escritorio ha cerrado la sesion: ya no va a llegar mas pantalla.
    Cortada,
}

struct Emision {
    marcha: Marcha,
    salida: Sender<Fotograma>,
    reciclados: Reciclados,
    intervalo_ms: u64,
}

/// La captura con el permiso dado y la imagen ya llegando, a falta de que empiece el reloj.
pub struct Preparada {
    region: Rect,
    fps: u32,
    raton_cocido: bool,
    avisos: Option<pw::channel::Sender<Aviso>>,
    hilo: Option<JoinHandle<Rastro>>,
}

impl Drop for Preparada {
    fn drop(&mut self) {
        // Si se suelta sin haber grabado, que el hilo no se quede esperando.
        if let Some(avisos) = self.avisos.take() {
            let _ = avisos.send(Aviso::Dejarlo);
        }
    }
}

/// Pide permiso al escritorio (si hace falta), abre la pantalla que lleva la region y espera
/// a que llegue la primera imagen. Puede tardar lo que tarde la persona en contestar.
pub fn preparar(
    region: Rect,
    fps: u32,
    ficha: &Path,
    al_cortarse: Box<dyn Fn() + Send>,
) -> Result<Preparada> {
    let guardada = std::fs::read_to_string(ficha)
        .ok()
        .map(|texto| texto.trim().to_string())
        .filter(|texto| !texto.is_empty());
    let habia_ficha = guardada.is_some();
    let permiso = match std::thread::spawn(move || tauri::async_runtime::block_on(pedir(guardada))).join() {
        Ok(Ok(permiso)) => permiso,
        Ok(Err(error)) => {
            // Con una ficha buena el escritorio no pregunta ni falla. Si no ha dejado grabar,
            // esa ficha ya no abre nada: fuera, y la vez siguiente se pregunta de cero.
            if habia_ficha {
                let _ = std::fs::remove_file(ficha);
            }
            return Err(AppError::Msg(por_que_no(&error)));
        }
        Err(_) => return Err(AppError::Msg("el portal de grabación ha reventado".into())),
    };
    // La ficha vieja ya no vale, se haya dado una nueva o no.
    match &permiso.ficha {
        Some(nueva) => {
            if let Some(carpeta) = ficha.parent() {
                let _ = std::fs::create_dir_all(carpeta);
            }
            if let Err(error) = guardar_la_ficha(ficha, nueva) {
                eprintln!("[grabar] no se ha podido guardar el permiso del escritorio: {error}");
            }
        }
        None => {
            let _ = std::fs::remove_file(ficha);
        }
    }

    let pantallas = crate::capture::wayland::pantallas()?;
    let monitores = crate::capture::wayland::monitores()?;
    let Some((nodo, monitor)) = elegir(&permiso.fuentes, &pantallas, &monitores, region) else {
        // Para que la vez siguiente vuelva a preguntar y se pueda elegir esa pantalla.
        let _ = std::fs::remove_file(ficha);
        return Err(AppError::Msg(
            "esa pantalla no está entre las que has dejado grabar: vuelve a intentarlo y elígela".into(),
        ));
    };
    // La region, contada desde la esquina de su pantalla y sin salirse de ella.
    let relativa = Rect { x: region.x - monitor.x, y: region.y - monitor.y, ..region };
    let dentro = super::x11::dentro_de_la_pantalla(relativa, monitor.width, monitor.height)
        .ok_or_else(|| AppError::Msg("esa región se sale de la pantalla".into()))?;
    let region = Rect { x: dentro.x + monitor.x, y: dentro.y + monitor.y, ..dentro };
    eprintln!(
        "[grabar] el escritorio deja grabar {} pantalla(s); nodo {nodo}, ratón {}",
        permiso.fuentes.len(),
        if permiso.raton_aparte { "aparte" } else { "dibujado en la imagen" }
    );

    let (avisos, recibidos) = pw::channel::channel::<Aviso>();
    *permiso.sesion.cortes.lock() = Some(avisos.clone());
    let (listo, esperando) = std::sync::mpsc::channel::<std::result::Result<(), String>>();
    let raton_aparte = permiso.raton_aparte;
    let plan = Plan { dentro, pantalla: (monitor.width, monitor.height), raton_aparte };
    let hilo = std::thread::Builder::new()
        .name("grabar-pipewire".into())
        .spawn(move || bucle(permiso, nodo, plan, recibidos, listo, al_cortarse))
        .map_err(|error| AppError::Msg(format!("no se ha podido empezar a grabar: {error}")))?;
    let preparada = Preparada { region, fps, raton_cocido: !raton_aparte, avisos: Some(avisos), hilo: Some(hilo) };
    match esperando.recv_timeout(ESPERA_DE_LA_IMAGEN) {
        Ok(Ok(())) => Ok(preparada),
        Ok(Err(error)) => Err(AppError::Msg(format!("el escritorio no manda la imagen de la pantalla: {error}"))),
        Err(_) => Err(AppError::Msg("el escritorio ha dado permiso, pero no llega la imagen de la pantalla".into())),
    }
}

impl Preparada {
    pub fn region(&self) -> Rect {
        self.region
    }

    /// El raton viene ya dibujado dentro de la imagen: el exportador no tiene que dibujarlo.
    pub fn raton_cocido(&self) -> bool {
        self.raton_cocido
    }

    pub fn grabar(mut self, marcha: Marcha, salida: Sender<Fotograma>, reciclados: Reciclados) -> EnMarcha {
        let intervalo_ms = 1000 / u64::from(self.fps.max(1));
        if let Some(avisos) = self.avisos.take() {
            let _ = avisos.send(Aviso::Empezar(Emision { marcha, salida, reciclados, intervalo_ms }));
        }
        // Un solo hilo trae la imagen y el raton, y devuelve el rastro al acabar.
        EnMarcha { imagen: None, raton: self.hilo.take() }
    }
}

/// Lo que hay que saber para recortar: donde cae la region en su pantalla.
struct Plan {
    dentro: Rect,
    pantalla: (u32, u32),
    raton_aparte: bool,
}

/// Todo lo que cambia mientras corre el bucle de PipeWire. Solo lo toca ese hilo.
struct Estado {
    plan: Plan,
    formato: VideoInfoRaw,
    recorte: Option<Recorte>,
    /// El ultimo fotograma que ha llegado, ya recortado y en RGBA.
    ultimo: Vec<u8>,
    /// `ultimo` todavia no se ha mandado.
    por_mandar: bool,
    /// Para avisar, una sola vez, de que ya hay imagen (o de que no va a haberla).
    listo: Option<Sender<std::result::Result<(), String>>>,
    emision: Option<Emision>,
    proximo_ms: u64,
    rastro: Rastro,
    /// Donde esta el raton y que imagen lleva, aunque todavia no se este grabando.
    raton: Option<(i32, i32)>,
    puntero: Option<u32>,
    ultimo_apunte_ms: Option<u64>,
    cortada: bool,
    dejarlo: bool,
    al_cortarse: Box<dyn Fn() + Send>,
}

impl Estado {
    /// Un fotograma con imagen: se recorta y se guarda como el ultimo.
    fn recibir(&mut self, origen: &[u8], paso: usize) {
        let medida = self.formato.size();
        if self.recorte.is_none() {
            self.recorte = recorte(self.plan.dentro, self.plan.pantalla, (medida.width, medida.height));
        }
        let Some(plan) = self.recorte.as_ref() else {
            return;
        };
        let orden = match self.formato.format() {
            VideoFormat::BGRx | VideoFormat::BGRA => Orden::Bgr,
            VideoFormat::RGBx | VideoFormat::RGBA => Orden::Rgb,
            _ => return,
        };
        let paso = if paso >= medida.width as usize * 4 { paso } else { medida.width as usize * 4 };
        if !recortar(origen, paso, orden, plan, &mut self.ultimo) {
            return;
        }
        self.por_mandar = true;
        if let Some(listo) = self.listo.take() {
            let _ = listo.send(Ok(()));
        }
    }

    /// Manda el ultimo fotograma si ha cambiado y ya toca.
    fn soltar(&mut self) {
        let Some(emision) = self.emision.as_ref() else {
            return;
        };
        if !self.por_mandar || emision.marcha.en_pausa() || emision.marcha.parada() {
            return;
        }
        let ahora = emision.marcha.ms();
        if ahora < self.proximo_ms {
            return;
        }
        let mut rgba = emision.reciclados.coger();
        rgba.clear();
        rgba.extend_from_slice(&self.ultimo);
        let _ = emision.salida.send(Fotograma { rgba, ts_ms: ahora });
        self.proximo_ms = ahora + emision.intervalo_ms;
        self.por_mandar = false;
    }

    /// Ya hay reloj. Lo que se sabia del raton se apunta en el instante cero.
    fn empezar(&mut self, emision: Emision) {
        self.emision = Some(emision);
        self.proximo_ms = 0;
        if let Some((x, y)) = self.raton {
            self.rastro.cursor.push((0, x, y));
            self.ultimo_apunte_ms = Some(0);
        }
        if let Some(id) = self.puntero {
            self.rastro.cambios_puntero.push((0, id));
        }
        self.soltar();
    }

    /// Lo que cuenta el escritorio del raton con este fotograma.
    fn apuntar_raton(&mut self, cursor: &MetaCursor) {
        if !self.plan.raton_aparte || !cursor.is_valid() {
            return;
        }
        let Some(plan) = self.recorte.as_ref() else {
            return;
        };
        // De pixeles del fotograma a pixeles de la region: lo contrario del recorte.
        let (ancho, alto) = (plan.columnas.len() as f64, plan.filas.len() as f64);
        let (x0, x1) = (f64::from(plan.columnas[0]), f64::from(plan.columnas[plan.columnas.len() - 1]) + 1.0);
        let (y0, y1) = (f64::from(plan.filas[0]), f64::from(plan.filas[plan.filas.len() - 1]) + 1.0);
        let sitio = cursor.position();
        let x = ((f64::from(sitio.x) - x0) * ancho / (x1 - x0)).round() as i32;
        let y = ((f64::from(sitio.y) - y0) * alto / (y1 - y0)).round() as i32;
        let ahora = self.emision.as_ref().filter(|e| !e.marcha.en_pausa()).map(|e| e.marcha.ms());

        if let Some(imagen) = cursor.bitmap().and_then(imagen_del_puntero) {
            let caliente = cursor.hotspot();
            let caliente = (caliente.x.max(0) as u32, caliente.y.max(0) as u32);
            let id = match self.rastro.punteros.iter().position(|p| p.caliente == caliente && p.imagen == imagen) {
                Some(ya) => ya as u32,
                None => {
                    self.rastro.punteros.push(Puntero { caliente, imagen });
                    (self.rastro.punteros.len() - 1) as u32
                }
            };
            if self.puntero != Some(id) {
                self.puntero = Some(id);
                if let Some(ms) = ahora {
                    self.rastro.cambios_puntero.push((ms, id));
                }
            }
        }

        if self.raton == Some((x, y)) {
            return;
        }
        match ahora {
            None => self.raton = Some((x, y)),
            Some(ms) => {
                if self.ultimo_apunte_ms.is_some_and(|ultimo| ms < ultimo + PASO_DEL_RATON_MS) {
                    return;
                }
                self.raton = Some((x, y));
                self.rastro.cursor.push((ms, x, y));
                self.ultimo_apunte_ms = Some(ms);
            }
        }
    }

    /// El escritorio ha dejado de mandar la pantalla.
    fn cortar(&mut self, por_que: &str) {
        if let Some(listo) = self.listo.take() {
            let _ = listo.send(Err(por_que.to_string()));
            return;
        }
        let grabando = self.emision.as_ref().is_some_and(|e| !e.marcha.parada());
        if grabando && !self.cortada {
            self.cortada = true;
            eprintln!("[grabar] el escritorio ha cortado la grabación ({por_que})");
            (self.al_cortarse)();
        }
    }

    fn hay_que_parar(&self) -> bool {
        self.dejarlo || self.emision.as_ref().is_some_and(|e| e.marcha.parada())
    }
}

/// La imagen del puntero tal y como la manda el escritorio, a RGBA sin multiplicar.
fn imagen_del_puntero(mapa: &spa::buffer::meta::MetaBitmap) -> Option<image::RgbaImage> {
    let medida = mapa.size();
    if medida.width == 0 || medida.height == 0 {
        return None;
    }
    let datos = mapa.bitmap_data()?;
    let paso = mapa.stride().unsigned_abs() as usize;
    if paso < medida.width as usize * 4 {
        return None;
    }
    // A que byte de cada pixel va el rojo, el verde, el azul y la transparencia.
    let (r, g, b, a) = match mapa.format() {
        VideoFormat::RGBA => (0, 1, 2, 3),
        VideoFormat::BGRA => (2, 1, 0, 3),
        VideoFormat::ARGB => (1, 2, 3, 0),
        VideoFormat::ABGR => (3, 2, 1, 0),
        _ => return None,
    };
    let mut pixeles = Vec::with_capacity((medida.width * medida.height) as usize);
    for fila in datos.chunks_exact(paso).take(medida.height as usize) {
        for p in fila.chunks_exact(4).take(medida.width as usize) {
            pixeles.push(u32::from(p[a]) << 24 | u32::from(p[r]) << 16 | u32::from(p[g]) << 8 | u32::from(p[b]));
        }
    }
    super::puntero_de_argb(&pixeles, medida.width, medida.height)
}

/// Que se le pide al escritorio: imagen sin comprimir, en uno de los cuatro ordenes de color
/// que se saben leer, a la medida que tenga la pantalla.
fn oferta() -> std::result::Result<Vec<u8>, String> {
    let objeto = spa::pod::object!(
        SpaTypes::ObjectParamFormat,
        ParamType::EnumFormat,
        spa::pod::property!(FormatProperties::MediaType, Id, MediaType::Video),
        spa::pod::property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        spa::pod::property!(
            FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            VideoFormat::BGRx,
            VideoFormat::BGRx,
            VideoFormat::BGRA,
            VideoFormat::RGBx,
            VideoFormat::RGBA,
        ),
        spa::pod::property!(
            FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            Rectangle { width: 1920, height: 1080 },
            Rectangle { width: 1, height: 1 },
            Rectangle { width: 16384, height: 16384 }
        ),
        spa::pod::property!(
            FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            Fraction { num: 30, denom: 1 },
            Fraction { num: 0, denom: 1 },
            Fraction { num: 1000, denom: 1 }
        ),
    );
    serializar(objeto)
}

/// Que el escritorio mande el raton aparte con cada fotograma, con sitio para su imagen.
fn pedir_el_raton() -> std::result::Result<Vec<u8>, String> {
    let cabeceras = std::mem::size_of::<spa::sys::spa_meta_cursor>() + std::mem::size_of::<spa::sys::spa_meta_bitmap>();
    let con_imagen = |lado: usize| (cabeceras + lado * lado * 4) as i32;
    let objeto = spa::pod::Object {
        type_: SpaTypes::ObjectParamMeta.as_raw(),
        id: ParamType::Meta.as_raw(),
        properties: vec![
            spa::pod::Property::new(spa::sys::SPA_PARAM_META_type, spa::pod::Value::Id(Id(spa::sys::SPA_META_Cursor))),
            spa::pod::Property::new(
                spa::sys::SPA_PARAM_META_size,
                spa::pod::Value::Choice(spa::pod::ChoiceValue::Int(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Range { default: con_imagen(64), min: con_imagen(1), max: con_imagen(1024) },
                ))),
            ),
        ],
    };
    serializar(objeto)
}

fn serializar(objeto: spa::pod::Object) -> std::result::Result<Vec<u8>, String> {
    PodSerializer::serialize(Cursor::new(Vec::new()), &spa::pod::Value::Object(objeto))
        .map(|(escrito, _)| escrito.into_inner())
        .map_err(|error| format!("{error:?}"))
}

/// El hilo de PipeWire: conecta con la pantalla, recibe sus fotogramas y, cuando hay reloj,
/// los suelta. Acaba cuando se para la grabacion y devuelve el rastro del raton.
fn bucle(
    permiso: Permiso,
    nodo: u32,
    plan: Plan,
    avisos: pw::channel::Receiver<Aviso>,
    listo: Sender<std::result::Result<(), String>>,
    al_cortarse: Box<dyn Fn() + Send>,
) -> Rastro {
    let Permiso { sesion, conexion, .. } = permiso;
    let estado = Rc::new(RefCell::new(Estado {
        plan,
        formato: VideoInfoRaw::default(),
        recorte: None,
        ultimo: Vec::new(),
        por_mandar: false,
        listo: Some(listo),
        emision: None,
        proximo_ms: 0,
        rastro: Rastro::default(),
        raton: None,
        puntero: None,
        ultimo_apunte_ms: None,
        cortada: false,
        dejarlo: false,
        al_cortarse,
    }));
    if let Err(error) = correr(conexion, nodo, avisos, &estado) {
        eprintln!("[grabar] PipeWire: {error}");
        estado.borrow_mut().cortar(&error);
    }
    // Cerrar la sesion es lo que apaga el aviso de «se esta compartiendo la pantalla».
    drop(sesion);
    let rastro = std::mem::take(&mut estado.borrow_mut().rastro);
    rastro
}

fn correr(
    conexion: OwnedFd,
    nodo: u32,
    avisos: pw::channel::Receiver<Aviso>,
    estado: &Rc<RefCell<Estado>>,
) -> std::result::Result<(), String> {
    let mal = |error: pw::Error| error.to_string();
    pw::init();
    let bucle = pw::main_loop::MainLoopRc::new(None).map_err(mal)?;
    let contexto = pw::context::ContextRc::new(&bucle, None).map_err(mal)?;
    // Por la conexion que da el portal, y no por la del sistema: es la unica por la que se
    // ve el nodo de la pantalla cuando la app va en una caja (Flatpak).
    let nucleo = contexto.connect_fd_rc(conexion, None).map_err(mal)?;
    let flujo = pw::stream::StreamRc::new(
        nucleo,
        "winshotx",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )
    .map_err(mal)?;

    let raton_aparte = estado.borrow().plan.raton_aparte;
    let _escucha = flujo
        .add_local_listener_with_user_data(estado.clone())
        .state_changed(|_, estado, _, nuevo| match nuevo {
            pw::stream::StreamState::Error(error) => estado.borrow_mut().cortar(&error),
            pw::stream::StreamState::Unconnected => estado.borrow_mut().cortar("el escritorio ha cerrado la conexión"),
            _ => {}
        })
        .param_changed(move |flujo, estado, id, param| {
            let Some(param) = param else {
                return;
            };
            if id != ParamType::Format.as_raw() {
                return;
            }
            let Ok((tipo, subtipo)) = spa::param::format_utils::parse_format(param) else {
                return;
            };
            if tipo != MediaType::Video || subtipo != MediaSubtype::Raw {
                return;
            }
            let mut estado = estado.borrow_mut();
            if estado.formato.parse(param).is_err() {
                return;
            }
            // Si la pantalla cambia de medida a media grabacion, el recorte se rehace.
            estado.recorte = None;
            if raton_aparte {
                if let Ok(bytes) = pedir_el_raton() {
                    if let Some(pod) = Pod::from_bytes(&bytes) {
                        let _ = flujo.update_params(&mut [pod]);
                    }
                }
            }
        })
        .process(|flujo, estado| {
            let Some(mut bufer) = flujo.dequeue_buffer() else {
                return;
            };
            let mut estado = estado.borrow_mut();
            if let Some(cursor) = bufer.find_meta::<MetaCursor>() {
                estado.apuntar_raton(cursor);
            }
            let Some(datos) = bufer.datas_mut().first_mut() else {
                return;
            };
            let trozo = datos.chunk();
            let (desde, cuanto, paso) = (trozo.offset() as usize, trozo.size() as usize, trozo.stride().max(0) as usize);
            // Un bufer sin imagen solo trae noticias del raton.
            if cuanto == 0 || trozo.flags().contains(ChunkFlags::CORRUPTED) {
                return;
            }
            let Some(bytes) = datos.data() else {
                return;
            };
            let Some(imagen) = bytes.get(desde..) else {
                return;
            };
            estado.recibir(imagen, paso);
            estado.soltar();
        })
        .register()
        .map_err(mal)?;

    let oferta = oferta()?;
    let mut parametros = [Pod::from_bytes(&oferta).ok_or("no se ha podido escribir el formato que se pide")?];
    flujo
        .connect(
            Direction::Input,
            Some(nodo),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut parametros,
        )
        .map_err(mal)?;

    let _avisos = avisos.attach(bucle.loop_(), {
        let estado = estado.clone();
        let bucle = bucle.downgrade();
        move |aviso| match aviso {
            Aviso::Empezar(emision) => estado.borrow_mut().empezar(emision),
            Aviso::Dejarlo => {
                estado.borrow_mut().dejarlo = true;
                if let Some(bucle) = bucle.upgrade() {
                    bucle.quit();
                }
            }
            Aviso::Cortada => estado.borrow_mut().cortar("el escritorio ha dejado de compartir la pantalla"),
        }
    });
    let latido = bucle.loop_().add_timer({
        let estado = estado.clone();
        let bucle = bucle.downgrade();
        move |_| {
            let mut estado = estado.borrow_mut();
            estado.soltar();
            if estado.hay_que_parar() {
                if let Some(bucle) = bucle.upgrade() {
                    bucle.quit();
                }
            }
        }
    });
    let _ = latido.update_timer(Some(LATIDO), Some(LATIDO));

    bucle.run();
    let _ = flujo.disconnect();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pantalla(x: i32, y: i32, ancho: i32, alto: i32) -> PantallaGtk {
        PantallaGtk { nombre: "p".into(), x, y, ancho, alto, escala: 1, principal: x == 0 }
    }

    fn monitor(id: u32, x: i32, y: i32, ancho: u32, alto: u32) -> MonitorInfo {
        MonitorInfo { id, label: "p".into(), x, y, width: ancho, height: alto, scale: 1.0, is_primary: id == 0 }
    }

    fn fuente(nodo: u32, posicion: Option<(i32, i32)>, medida: Option<(i32, i32)>) -> Fuente {
        Fuente { nodo, posicion, medida }
    }

    #[test]
    fn con_una_pantalla_el_recorte_es_tal_cual() {
        let plan = recorte(Rect { x: 450, y: 200, width: 4, height: 2 }, (1600, 1000), (1600, 1000)).expect("recorte");
        assert_eq!(plan.columnas, vec![450, 451, 452, 453]);
        assert_eq!(plan.filas, vec![200, 201]);
        assert!(plan.seguido);
    }

    #[test]
    fn si_el_escritorio_manda_el_doble_de_pixeles_se_coge_uno_de_cada_dos() {
        let plan = recorte(Rect { x: 10, y: 0, width: 3, height: 1 }, (100, 50), (200, 100)).expect("recorte");
        assert_eq!(plan.columnas, vec![21, 23, 25]);
        assert_eq!(plan.filas, vec![1]);
        assert!(!plan.seguido);
    }

    #[test]
    fn el_recorte_nunca_lee_fuera_del_fotograma() {
        let plan = recorte(Rect { x: 98, y: 48, width: 2, height: 2 }, (100, 50), (99, 49)).expect("recorte");
        assert!(plan.columnas.iter().all(|c| *c < 99), "{:?}", plan.columnas);
        assert!(plan.filas.iter().all(|f| *f < 49), "{:?}", plan.filas);
        assert!(recorte(Rect { x: 0, y: 0, width: 0, height: 2 }, (100, 50), (100, 50)).is_none());
    }

    /// Un fotograma de 3x2 con relleno al final de cada fila (paso 16 en vez de 12): el pixel
    /// (x, y) vale [x, y, 100 + x, 0] en el orden en que lo manda el escritorio.
    fn fotograma_de_prueba() -> Vec<u8> {
        let mut bytes = vec![0xEE; 32];
        for y in 0..2u8 {
            for x in 0..3u8 {
                let en = usize::from(y) * 16 + usize::from(x) * 4;
                bytes[en..en + 4].copy_from_slice(&[x, y, 100 + x, 0]);
            }
        }
        bytes
    }

    #[test]
    fn recortar_salta_el_relleno_de_cada_fila_y_gira_los_colores() {
        let plan = recorte(Rect { x: 1, y: 0, width: 2, height: 2 }, (3, 2), (3, 2)).expect("recorte");
        let mut destino = Vec::new();
        assert!(recortar(&fotograma_de_prueba(), 16, Orden::Bgr, &plan, &mut destino));
        assert_eq!(destino, [101, 0, 1, 255, 102, 0, 2, 255, 101, 1, 1, 255, 102, 1, 2, 255]);
        assert!(recortar(&fotograma_de_prueba(), 16, Orden::Rgb, &plan, &mut destino));
        assert_eq!(destino, [1, 0, 101, 255, 2, 0, 102, 255, 1, 1, 101, 255, 2, 1, 102, 255]);
    }

    #[test]
    fn recortar_a_saltos_da_lo_mismo_que_seguido() {
        let seguido = recorte(Rect { x: 0, y: 0, width: 3, height: 2 }, (3, 2), (3, 2)).expect("recorte");
        let a_saltos = Recorte { seguido: false, ..seguido.clone() };
        let (mut uno, mut otro) = (Vec::new(), Vec::new());
        assert!(recortar(&fotograma_de_prueba(), 16, Orden::Bgr, &seguido, &mut uno));
        assert!(recortar(&fotograma_de_prueba(), 16, Orden::Bgr, &a_saltos, &mut otro));
        assert_eq!(uno, otro);
    }

    #[test]
    fn un_fotograma_mas_corto_de_lo_que_dice_no_se_lee() {
        let plan = recorte(Rect { x: 0, y: 0, width: 3, height: 2 }, (3, 2), (3, 2)).expect("recorte");
        let mut destino = vec![7u8; 4];
        assert!(!recortar(&fotograma_de_prueba()[..20], 16, Orden::Bgr, &plan, &mut destino));
        assert_eq!(destino, [7, 7, 7, 7], "no se toca lo que habia");
    }

    #[test]
    fn se_graba_la_pantalla_que_lleva_la_region() {
        let pantallas = [pantalla(0, 0, 1920, 1080), pantalla(1920, 0, 1280, 720)];
        let monitores = [monitor(0, 0, 0, 1920, 1080), monitor(1, 1920, 0, 1280, 720)];
        let fuentes = [fuente(41, Some((1920, 0)), Some((1280, 720))), fuente(40, Some((0, 0)), Some((1920, 1080)))];
        let izquierda = Rect { x: 100, y: 100, width: 200, height: 200 };
        let derecha = Rect { x: 2000, y: 100, width: 200, height: 200 };
        assert_eq!(elegir(&fuentes, &pantallas, &monitores, izquierda).map(|(nodo, m)| (nodo, m.id)), Some((40, 0)));
        assert_eq!(elegir(&fuentes, &pantallas, &monitores, derecha).map(|(nodo, m)| (nodo, m.id)), Some((41, 1)));
    }

    #[test]
    fn si_no_se_ha_dejado_grabar_esa_pantalla_no_se_graba_otra() {
        let pantallas = [pantalla(0, 0, 1920, 1080), pantalla(1920, 0, 1280, 720)];
        let monitores = [monitor(0, 0, 0, 1920, 1080), monitor(1, 1920, 0, 1280, 720)];
        let solo_la_izquierda = [fuente(40, Some((0, 0)), Some((1920, 1080)))];
        let derecha = Rect { x: 2000, y: 100, width: 200, height: 200 };
        assert!(elegir(&solo_la_izquierda, &pantallas, &monitores, derecha).is_none());
    }

    #[test]
    fn un_escritorio_que_no_dice_donde_esta_la_pantalla_vale_si_solo_hay_una() {
        let region = Rect { x: 100, y: 100, width: 200, height: 200 };
        let una = [fuente(7, None, None)];
        assert_eq!(
            elegir(&una, &[pantalla(0, 0, 1920, 1080)], &[monitor(0, 0, 0, 1920, 1080)], region).map(|(nodo, _)| nodo),
            Some(7)
        );
        // Con dos pantallas iguales y sin saber cual es cual, no se adivina.
        let pantallas = [pantalla(0, 0, 1920, 1080), pantalla(1920, 0, 1920, 1080)];
        let monitores = [monitor(0, 0, 0, 1920, 1080), monitor(1, 1920, 0, 1920, 1080)];
        let dos = [fuente(7, None, Some((1920, 1080))), fuente(8, None, Some((1920, 1080)))];
        assert!(elegir(&dos, &pantallas, &monitores, region).is_none());
        // Con medidas distintas si se sabe.
        let pantallas = [pantalla(0, 0, 1920, 1080), pantalla(1920, 0, 1280, 720)];
        let monitores = [monitor(0, 0, 0, 1920, 1080), monitor(1, 1920, 0, 1280, 720)];
        let dos = [fuente(7, None, Some((1280, 720))), fuente(8, None, Some((1920, 1080)))];
        assert_eq!(elegir(&dos, &pantallas, &monitores, region).map(|(nodo, _)| nodo), Some(8));
    }
}
