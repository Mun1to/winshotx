use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::encode::{
    anotacion, escalar, estudio, ffmpeg, gif, jpg, marco, mp4, png, recorte::Recorte, zoom,
};
use crate::error::{AppError, Result};
use crate::record::{self, SessionData};
use crate::state::AppState;

pub const EVENT_PROGRESS: &str = "winshotx://export-progress";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub session_id: String,
    pub format: String,
    pub engine: String,
    pub from: usize,
    pub to: usize,
    /// Donde empieza lo que se exporta, en milisegundos de la grabacion. Lo manda la linea
    /// de tiempo del editor, y con el manda el reloj sobre `from`.
    ///
    /// Va en milisegundos y no en fotogramas porque una pantalla quieta es UN fotograma que
    /// dura lo que dure la quietud: Windows no manda otro mientras nada cambia. Cortando por
    /// fotogramas, los diez segundos parados del final de una grabacion se quitaban enteros
    /// o se dejaban enteros, pero no se podian cortar por la mitad.
    #[serde(default)]
    pub desde_ms: Option<u64>,
    /// Y donde acaba, sin incluirlo. Igual que `desde_ms`, manda sobre `to`.
    #[serde(default)]
    pub hasta_ms: Option<u64>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u8,
    /// Si el video exportado lleva el sonido que se grabo. Lo decide el interruptor del
    /// editor, y solo tiene efecto si la grabacion llego a capturar audio.
    pub audio: bool,
    #[serde(rename = "loop")]
    pub loop_forever: bool,
    /// Pixeles de aire alrededor de la captura. Cero es sin marco, que es lo de siempre.
    #[serde(default)]
    pub margin: u32,
    /// El nombre del fondo que eligio el panel: blanco, negro, gris, atardecer o menta.
    #[serde(default)]
    pub background: String,
    /// Si la captura lleva sombra sobre ese fondo.
    #[serde(default)]
    pub shadow: bool,
    /// Las marcas dibujadas encima, en coordenadas de 0 a 1 sobre la imagen.
    #[serde(default)]
    pub annotations: Vec<anotacion::Anotacion>,
    /// El trozo que se queda, de 0 a 1. Sin esto se exporta la captura entera.
    #[serde(default)]
    pub crop: Option<Recorte>,
    /// El archivo exacto que eligio el usuario en el dialogo de «Guardar en…», con su
    /// nombre. Manda sobre `destination`: es para la captura que va a un proyecto concreto
    /// y no a la carpeta de siempre. Sin esto, el nombre lo pone `archivos`.
    #[serde(default)]
    pub file: Option<String>,
    /// Cuanto se acerca la camara a cada clic. 0 o 1 es no acercarse.
    ///
    /// Se decide AQUI y no al grabar: los clics quedaron anotados en la sesion, asi que
    /// subir el zoom, bajarlo o quitarlo no obliga a volver a grabar nada.
    #[serde(default)]
    pub zoom: f32,
    /// Un aro donde se pulso. Tambien se decide aqui, y por lo mismo.
    #[serde(default)]
    pub clicks: bool,
    /// La pastilla de abajo con el atajo que se acaba de pulsar.
    #[serde(default)]
    pub keys: bool,
    /// El alto del puntero dibujado, en pixeles del fotograma. Cero es no dibujarlo.
    #[serde(default)]
    pub cursor: f32,
    /// A que velocidad se reproduce lo exportado. 1 es la de verdad, 2 el doble de rapido.
    ///
    /// Un tutorial tiene tramos en los que no pasa nada (algo que carga, un formulario que
    /// se rellena) y esos tramos se ven a 2x sin perder nada. Se decide aqui y no al
    /// grabar, como todo lo demas del estudio.
    #[serde(default = "velocidad_normal")]
    pub speed: f32,
    /// Quitarle al puntero dibujado los temblores de la mano.
    #[serde(default)]
    pub smooth_cursor: bool,
    /// Apagar el puntero dibujado cuando lleva un rato quieto.
    #[serde(default)]
    pub hide_idle_cursor: bool,
    /// El radio de las esquinas de la captura cuando va sobre un fondo, en pixeles.
    #[serde(default)]
    pub corners: u32,
    /// El volumen del sonido exportado: 1 es como se grabo, 0 mudo y 2 el doble.
    #[serde(default = "velocidad_normal")]
    pub volume: f32,
    /// A que velocidad se acerca y se aleja el zoom: 1 es la de siempre, 2 el doble de
    /// rapido. Ver `zoom::Ajustes::a_velocidad`.
    #[serde(default = "velocidad_normal")]
    pub zoom_speed: f32,
    pub destination: Option<String>,
    pub copy_to_clipboard: bool,
    /// Lo nuevo del plan de CleanShot X y Screen Studio: los zooms a mano, los tramos, los
    /// fondos propios, la webcam... Va aplanado, así que en el JSON son campos sueltos, y
    /// sin ellos todo sale como antes. Ver `contrato::Edicion`.
    #[serde(flatten)]
    pub edicion: crate::contrato::Edicion,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub bytes: u64,
    pub copied: bool,
    /// Por que no se pudo copiar, si se pidio copiar y no salio.
    pub copy_error: Option<String>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    stage: String,
    done: usize,
    total: usize,
}

/// Elige los fotogramas que tocan para el fps pedido y calcula sus retardos.
/// Bajar de 60 a 20 fps no puede consistir en tirar fotogramas al tuntun.
///
/// `velocidad` es a que ritmo se reproduce: a 2x se avanza el doble por la grabacion
/// original y cada fotograma elegido sigue durando lo que dura uno a los fps pedidos. Asi
/// el resultado tiene los fps que se pidieron y la mitad de duracion, en vez de un video
/// con retardos imposibles de cinco milisegundos que ningun GIF sabe representar.
///
/// Por fotogramas enteros, como pedia el editor antes de la linea de tiempo. Ya solo lo usan
/// las pruebas, que siguen comprobando ese reparto; exportar va por `resample_tramo`.
#[cfg(test)]
fn resample(
    session: &SessionData,
    from: usize,
    to: usize,
    fps: u32,
    velocidad: f32,
) -> (Vec<usize>, Vec<u32>) {
    let (desde, hasta) = tramo_de(session, from, to, None, None);
    resample_tramo(session, desde, hasta, fps, velocidad)
}

/// Lo mismo, pero sobre un tramo de reloj: de `desde_ms` a `hasta_ms`, sin incluirlo, y
/// juntando siempre los pasos que repiten fotograma (sin estudio encima se ven igual). Ya
/// solo lo usan las pruebas del reparto; exportar va por `pasos_de`.
#[cfg(test)]
fn resample_tramo(
    session: &SessionData,
    desde_ms: u64,
    hasta_ms: u64,
    fps: u32,
    velocidad: f32,
) -> (Vec<usize>, Vec<u32>) {
    let p = pasos(session, desde_ms, hasta_ms, fps, velocidad, &|_, _| true);
    (p.indices, p.retardos)
}

/// Lo que se escribe, paso a paso: qué fotograma de la grabacion va en cada uno, cuanto
/// dura, y en que instante de la grabacion cae.
///
/// El instante hace falta porque el estudio (el puntero, los aros, la pastilla, la camara)
/// se dibuja en el momento del PASO y no en el del fotograma. Una pantalla quieta es UN
/// fotograma de varios segundos (Windows no manda otro mientras nada cambia, y el puntero
/// ya no va dentro), asi que dibujarlo en el instante del fotograma dejaba el raton clavado
/// donde estaba cuando llego, y la camara del zoom igual, aunque se moviera por encima.
#[derive(Debug, Default)]
struct Pasos {
    indices: Vec<usize>,
    retardos: Vec<u32>,
    tiempos: Vec<u64>,
}

impl Pasos {
    /// Los fotogramas distintos en el orden en que se usan: lo que hay que leer del disco.
    fn a_leer(&self) -> Vec<usize> {
        let mut leer = self.indices.clone();
        leer.dedup();
        leer
    }

    /// Las posiciones 0, 1, 2... que reciben los codificadores: cada una es un paso.
    fn posiciones(&self) -> Vec<usize> {
        (0..self.indices.len()).collect()
    }
}

/// Los pasos de un tramo de reloj, de `desde_ms` a `hasta_ms` sin incluirlo.
///
/// Cada paso coge el fotograma que SE ESTABA VIENDO en ese instante, que es el ultimo que
/// empezo antes, no el primero de despues: una pantalla quieta no manda fotogramas, asi
/// que cortar dentro de una quietud tiene que dar esa quietud, no el fotograma siguiente.
///
/// Un paso se junta con el anterior (que dura mas, en vez de escribirse otra vez) cuando es
/// el mismo fotograma Y `igual` dice que, vestidos, se ven igual. Asi una pantalla quieta
/// con el raton quieto sigue costando un fotograma, y con el raton moviendose, los que hagan
/// falta para verlo moverse.
fn pasos(
    session: &SessionData,
    desde_ms: u64,
    hasta_ms: u64,
    fps: u32,
    velocidad: f32,
    igual: &dyn Fn(u64, u64) -> bool,
) -> Pasos {
    let frames = &session.frames;
    // Sin un solo fotograma no hay nada que recortar. Quien llama rechaza la lista vacia
    // igual, pero con un error que se puede leer en vez de un panic a mitad de exportar.
    if frames.is_empty() {
        return Pasos::default();
    }
    let step_ms = (1000.0 / fps.max(1) as f32).round() as u64;
    // Lo que se avanza por la grabacion en cada paso. A 1x es el mismo paso de siempre.
    let paso_origen = ((step_ms as f32 * velocidad).round() as u64).max(1);
    let mut cursor = frames
        .partition_point(|f| f.timestamp_ms <= desde_ms)
        .saturating_sub(1);
    // Ninguno que empiece en el corte final o despues puede colarse, aunque la duracion
    // minima de diez milisegundos haga que el anterior parezca llegar hasta el.
    let ultimo = frames
        .partition_point(|f| f.timestamp_ms < hasta_ms)
        .saturating_sub(1)
        .max(cursor);

    let mut salen = Pasos::default();
    let mut t = desde_ms;
    while t < hasta_ms {
        while cursor < ultimo && frames[cursor + 1].timestamp_ms <= t {
            cursor += 1;
        }
        // El ultimo paso se recorta: el clip exportado dura lo que dura el recorte, y a
        // velocidad distinta de 1 lo que sobra tambien se mide en el tiempo del resultado.
        let queda = hasta_ms - t;
        let slot = if queda >= paso_origen {
            step_ms
        } else {
            ((queda as f32 / velocidad).round() as u64).max(1)
        } as u32;
        let se_junta = salen.indices.last() == Some(&cursor)
            && salen.tiempos.last().is_some_and(|&antes| igual(antes, t));
        if se_junta {
            // El mismo fotograma, que se ve igual, se queda mas tiempo en vez de repetirse.
            if let Some(ultimo) = salen.retardos.last_mut() {
                *ultimo += slot;
            }
        } else {
            salen.indices.push(cursor);
            salen.retardos.push(slot);
            salen.tiempos.push(t);
        }
        t += paso_origen;
    }

    if salen.indices.is_empty() {
        salen.indices.push(cursor);
        salen.retardos.push(step_ms.max(20) as u32);
        salen.tiempos.push(desde_ms);
    }
    salen
}

/// Los pasos de una peticion: su tramo, a sus fotogramas por segundo y su velocidad, sin
/// juntar dos pasos que vestidos no se vean igual.
fn pasos_de(session: &SessionData, request: &ExportRequest) -> Pasos {
    let (desde, hasta) = request.tramo(session);
    let camara = Camara::preparar(session, request);
    let ve = QueSeVe::nuevo(session, request, &camara);
    // Con un fondo animado no hay dos instantes iguales: cada paso lleva su fotograma (S3).
    let animado = marco_de(request).animado();
    pasos(session, desde, hasta, request.fps, request.velocidad(), &|a, b| !animado && ve.igual(a, b))
}

/// Si dos instantes de la grabacion se ven igual una vez vestidos: el puntero en el mismo
/// sitio y con la misma cara, ningun aro ni pastilla a medias, y la camara sin acercar.
struct QueSeVe<'a> {
    session: &'a SessionData,
    cursor: bool,
    aros: bool,
    teclas: bool,
    tramos: Vec<zoom::Tramo>,
    suave: bool,
    /// Con el puntero escondiendose, cuando se movio el raton (ver `estudio::movimientos`).
    movimientos: Option<Vec<u64>>,
}

impl<'a> QueSeVe<'a> {
    fn nuevo(session: &'a SessionData, request: &ExportRequest, camara: &Option<Camara>) -> Self {
        Self {
            session,
            cursor: request.cursor > 0.0 && !session.cursor.is_empty(),
            aros: request.clicks && !session.clics.is_empty(),
            teclas: request.keys && !session.teclas.is_empty(),
            tramos: camara.as_ref().map(|c| c.tramos.clone()).unwrap_or_default(),
            suave: request.smooth_cursor,
            movimientos: request
                .hide_idle_cursor
                .then(|| estudio::movimientos(&session.cursor)),
        }
    }

    fn igual(&self, a: u64, b: u64) -> bool {
        let s = self.session;
        if self.cursor {
            // Al pixel, que es lo que se ve: con el movimiento suave el sitio es una media
            // y se queda unas decimas acercandose despues de que el raton pare.
            let sitio = |ms| {
                estudio::sitio_del_puntero(&s.cursor, ms, self.suave)
                    .map(|(x, y)| (x.round() as i32, y.round() as i32))
            };
            let visible = |ms| {
                self.movimientos
                    .as_deref()
                    .map(|m| (estudio::opacidad_del_puntero(m, &s.clics, ms) * 32.0).round() as i32)
            };
            if sitio(a) != sitio(b) || visible(a) != visible(b)
                || estudio::puntero_en(&s.cambios_puntero, a) != estudio::puntero_en(&s.cambios_puntero, b)
                || estudio::forma_en(&s.formas, a) != estudio::forma_en(&s.formas, b)
                || estudio::alto_del_puntero(1.0, &s.clics, a)
                    != estudio::alto_del_puntero(1.0, &s.clics, b)
            {
                return false;
            }
        }
        let vivo = |desde: u64, dura: u64, ms: u64| ms >= desde && ms - desde < dura;
        let aro = crate::record::realce::DURACION_MS;
        if self.aros && s.clics.iter().any(|c| vivo(c.ms, aro, a) || vivo(c.ms, aro, b)) {
            return false;
        }
        let pastilla = crate::record::teclas::DURACION_MS;
        if self.teclas && s.teclas.iter().any(|t| vivo(t.ms, pastilla, a) || vivo(t.ms, pastilla, b)) {
            return false;
        }
        // La camara solo se mueve dentro de sus tramos: fuera enseña la imagen entera.
        let acercada = |ms: u64| self.tramos.iter().any(|t| (t.desde_ms..=t.hasta_ms).contains(&ms));
        !acercada(a) && !acercada(b)
    }
}

/// El marco que pide una peticion. El fondo propio (`edicion.fondo`: un color, una imagen,
/// el escritorio o un video) manda sobre el de nombre cuando viene.
fn marco_de(request: &ExportRequest) -> marco::Marco {
    marco::Marco {
        margen: request.margin.min(400),
        fondo: marco::Fondo::elegido(&request.background, request.edicion.fondo.as_ref()),
        sombra: request.shadow,
        esquinas: request.corners.min(400),
    }
}

/// Las pruebas de los fondos propios (S3) por `escribir`, aparte para no pisar las de aqui.
#[cfg(test)]
#[path = "exporter_fondos.rs"]
mod pruebas_de_fondos;

/// Los fotogramas de una exportacion, ya vestidos y en orden: leidos por delante en otro
/// hilo, con el recorte, la camara, el estudio y las marcas puestos en el instante de cada
/// paso. Se piden por posicion (0, 1, 2...), que es lo que reciben los codificadores.
struct Vestidor<'a> {
    request: &'a ExportRequest,
    pasos: &'a Pasos,
    camara: Option<Camara>,
    vestir: Option<Estudio<'a>>,
    lector: record::LectorPorDelante,
    /// El ultimo fotograma leido, guardado solo cuando el paso siguiente lo repite.
    guardado: Option<(usize, image::RgbaImage)>,
    marco: marco::Marco,
}

impl<'a> Vestidor<'a> {
    fn nuevo(session: &'a SessionData, request: &'a ExportRequest, pasos: &'a Pasos) -> Self {
        Self {
            request,
            pasos,
            camara: Camara::preparar(session, request),
            vestir: estudio_de(session, request),
            // En orden y no `read_frame`: reconstruir cada uno desde el ultimo entero eran
            // hasta treinta parches por fotograma en vez de uno. Y por delante, en otro hilo,
            // para que leer el siguiente no espere a vestir y codificar este.
            lector: record::LectorPorDelante::nuevo(session.clone(), pasos.a_leer()),
            guardado: None,
            marco: marco_de(request),
        }
    }

    fn en(&mut self, posicion: usize) -> Result<image::RgbaImage> {
        let indice = *self
            .pasos
            .indices
            .get(posicion)
            .ok_or_else(|| AppError::Msg(format!("paso {posicion} inexistente")))?;
        let ms = self.pasos.tiempos[posicion];
        let base = match self.guardado.take() {
            Some((i, imagen)) if i == indice => imagen,
            _ => self.lector.en(indice)?,
        };
        if self.pasos.indices.get(posicion + 1) == Some(&indice) {
            self.guardado = Some((indice, base.clone()));
        }
        let mut recortes: Vec<Recorte> = self.request.crop.into_iter().collect();
        if let Some(r) = self.camara.en(ms) {
            recortes.push(r);
        }
        if let Some(e) = self.vestir.as_mut() {
            e.ms = ms;
        }
        Ok(enmarcar_y_anotar(
            base,
            self.request.width.max(1),
            self.request.height.max(1),
            // Con fondo animado, el del instante de este paso en el video exportado.
            self.marco.tras(&self.pasos.retardos[..posicion]),
            &self.request.annotations,
            &recortes,
            self.vestir.as_mut(),
        ))
    }
}

/// El tramo de reloj que se exporta, metido dentro de la grabacion y nunca vacio.
///
/// Con `desde_ms` y `hasta_ms` manda el reloj; sin ellos, los fotogramas `from` y `to`
/// enteros, que es como pedia el editor antes de la linea de tiempo y como siguen pidiendo
/// las fotos (un PNG es UN fotograma, no un tramo).
fn tramo_de(
    session: &SessionData,
    from: usize,
    to: usize,
    desde_ms: Option<u64>,
    hasta_ms: Option<u64>,
) -> (u64, u64) {
    let frames = &session.frames;
    let (Some(primero), Some(ultimo)) = (frames.first(), frames.last()) else {
        return (0, 0);
    };
    let inicio = primero.timestamp_ms;
    let fin = ultimo.timestamp_ms + u64::from(ultimo.duration_ms);
    let from = from.min(frames.len() - 1);
    let to = to.min(frames.len() - 1).max(from);
    let desde = desde_ms
        .unwrap_or(frames[from].timestamp_ms)
        .clamp(inicio, fin);
    let mut hasta = hasta_ms
        .unwrap_or(frames[to].timestamp_ms + u64::from(frames[to].duration_ms))
        .clamp(inicio, fin);
    // Al reves o del mismo tamanno que nada: se exporta lo minimo que se puede ver, en vez
    // de un archivo vacio que no abre ningun reproductor.
    if hasta <= desde {
        hasta = (desde + 1).min(fin);
    }
    (desde.min(hasta.saturating_sub(1)), hasta)
}

impl ExportRequest {
    /// El tramo de reloj de esta peticion. Ver `tramo_de`.
    fn tramo(&self, session: &SessionData) -> (u64, u64) {
        tramo_de(session, self.from, self.to, self.desde_ms, self.hasta_ms)
    }
}

/// La velocidad, ya acotada. Fuera de este rango deja de ser un video: a 0,2 son
/// fotogramas sueltos y a 10 no se ve nada.
impl ExportRequest {
    pub fn velocidad(&self) -> f32 {
        if self.speed.is_finite() && self.speed > 0.0 {
            self.speed.clamp(0.25, 4.0)
        } else {
            1.0
        }
    }
}

fn velocidad_normal() -> f32 {
    1.0
}

fn destination_path(
    app: &AppHandle,
    request: &ExportRequest,
    extension: &str,
) -> Result<PathBuf> {
    if let Some(elegido) = request.file.as_deref().filter(|f| !f.trim().is_empty()) {
        let ruta = ruta_elegida(elegido, extension);
        if let Some(carpeta) = ruta.parent() {
            std::fs::create_dir_all(carpeta)?;
        }
        return Ok(ruta);
    }
    let state = app.state::<AppState>();
    let dir = request
        .destination
        .clone()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| state.settings.read().save_directory.clone());
    // Quien decide donde cae un archivo y como se llama es `archivos`, y solo el: son
    // tres sitios los que guardan y los tres tienen que nombrar igual y no pisar nada.
    Ok(crate::archivos::destino(&PathBuf::from(dir), extension)?)
}

/// La ruta que eligio el usuario, con la extension del formato puesta si no la lleva: el
/// dialogo de Windows deja escribir «captura» a secas, y un MP4 sin extension no lo abre
/// nadie. Si ya trae la buena, en mayusculas o minusculas, se respeta tal cual.
fn ruta_elegida(elegido: &str, extension: &str) -> PathBuf {
    let mut ruta = PathBuf::from(elegido.trim());
    let la_lleva = ruta
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(extension));
    if !la_lleva {
        let nombre = ruta
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("winshotx")
            .to_string();
        ruta.set_file_name(format!("{nombre}.{extension}"));
    }
    ruta
}

fn load_session(app: &AppHandle, id: &str) -> Result<SessionData> {
    let state = app.state::<AppState>();
    if let Some(session) = state.sessions.read().get(id) {
        return Ok(session.clone());
    }
    // Segunda oportunidad: la sesion sigue en disco aunque la app se haya reiniciado.
    let path = state.session_dir(id).join("session.json");
    let raw = std::fs::read_to_string(path).map_err(|_| AppError::UnknownSession(id.to_string()))?;
    let session: SessionData = serde_json::from_str(&raw)?;
    Ok(session)
}

pub fn export(app: &AppHandle, request: ExportRequest) -> Result<ExportResult> {
    let started = Instant::now();
    let session = load_session(app, &request.session_id)?;
    // Sin fotogramas no hay nada que exportar, y los caminos de abajo lo dan por hecho.
    // Un error aqui se lee; un panic a mitad de exportar deja la ventana esperando.
    if session.frames.is_empty() {
        return Err(AppError::Msg("esa grabación no tiene ningún fotograma".into()));
    }
    let extension = match request.format.as_str() {
        formato @ ("png" | "jpg" | "gif" | "mp4") => formato,
        other => return Err(AppError::Msg(format!("formato desconocido: {other}"))),
    };
    let path = destination_path(app, &request, extension)?;

    let emit = |stage: &str, done: usize, total: usize| {
        let _ = app.emit(
            EVENT_PROGRESS,
            ExportProgress {
                stage: stage.to_string(),
                done,
                total,
            },
        );
    };

    escribir(&session, &request, &path, &emit)?;

    emit("done", 1, 1);

    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    // Si el portapapeles falla NO se cae la exportacion: el archivo ya esta escrito y
    // perderlo por eso seria mucho peor. Pero el motivo sube hasta la pantalla en vez de
    // tragarse con un `.is_ok()`, que es lo que hacia que pulsar copiar pareciera no hacer
    // absolutamente nada.
    let (copied, copy_error) = match request.copy_to_clipboard {
        false => (false, None),
        true => match copy_result(&path, &request) {
            Ok(()) => (true, None),
            Err(e) => (false, Some(e.to_string())),
        },
    };

    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        bytes,
        copied,
        copy_error,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

/// Escribe el archivo de la exportacion en `path`. Es todo lo de exportar que no necesita
/// la aplicacion abierta, y por eso se puede probar de punta a punta con una sesion de
/// verdad en el disco: lo de antes y lo de despues (elegir el nombre, el portapapeles) vive
/// en `export`.
fn escribir(
    session: &SessionData,
    request: &ExportRequest,
    path: &Path,
    emit: &dyn Fn(&str, usize, usize),
) -> Result<()> {
    let width = request.width.max(1);
    let height = request.height.max(1);

    // El marco crece por fuera de lo que se pidio en «Dimensiones»: quien elige 800 de
    // ancho y 40 de aire quiere la captura a 800, no a 720 con bordes. Asi que el tamanno
    // que se le pide al codificador es el de despues de enmarcar.
    let marco = marco_de(request);
    let (ancho_final, alto_final) = marco.medida(width, height);

    match request.format.as_str() {
        "png" => {
            emit("reading", 0, 1);
            let image = record::read_frame(session, request.from)?;
            // Una foto no tiene zoom: la camara solo tiene sentido con el tiempo pasando.
            let recortes: Vec<Recorte> = request.crop.into_iter().collect();
            let image = enmarcar_y_anotar(image, width, height, marco, &request.annotations, &recortes, None);
            png::save(&image, path, ancho_final, alto_final)?;
        }
        // El mismo fotograma, pero pesando cinco o diez veces menos. Es lo que hace falta
        // para mandar una captura por correo o por un chat que la recomprime igual.
        "jpg" => {
            emit("reading", 0, 1);
            let image = record::read_frame(session, request.from)?;
            let recortes: Vec<Recorte> = request.crop.into_iter().collect();
            let image = enmarcar_y_anotar(image, width, height, marco, &request.annotations, &recortes, None);
            jpg::save(&image, path, ancho_final, alto_final, request.quality)?;
        }
        "gif" => {
            let pasos = pasos_de(session, request);
            if request.engine == "ffmpeg" && ffmpeg::available() {
                let temporary = session.dir.join("export-source.mp4");
                encode_mp4(session, &pasos, &temporary, request, emit)?;
                emit("encoding", 0, 1);
                ffmpeg::gif_from_video(&temporary, path, request.fps, ancho_final, request.quality)?;
                let _ = std::fs::remove_file(&temporary);
            } else {
                let mut vestidor = Vestidor::nuevo(session, request, &pasos);
                // La paleta se saca de unos pocos pasos repartidos, leidos cada uno desde su
                // fotograma entero: el lector de `vestidor` va en orden y solo una vez. Sin
                // la camara, que tiene inercia y se pide en orden; los colores que se ven
                // acercado son los mismos que se ven sin acercar.
                let mut vestir_muestra = estudio_de(session, request);
                let mut muestra = |posicion: usize| -> Result<image::RgbaImage> {
                    let base = record::read_frame(session, pasos.indices[posicion])?;
                    let recortes: Vec<Recorte> = request.crop.into_iter().collect();
                    if let Some(e) = vestir_muestra.as_mut() {
                        e.ms = pasos.tiempos[posicion];
                    }
                    Ok(enmarcar_y_anotar(
                        base,
                        width,
                        height,
                        // Con fondo animado, el de ese instante: si no, los colores del
                        // resto del video no entran en la paleta y salen cambiados.
                        marco.tras(&pasos.retardos[..posicion]),
                        &request.annotations,
                        &recortes,
                        vestir_muestra.as_mut(),
                    ))
                };
                let mut loader = |posicion: usize| vestidor.en(posicion);
                gif::encode(
                    &pasos.posiciones(),
                    &pasos.retardos,
                    &mut muestra,
                    &mut loader,
                    path,
                    &gif::GifOptions {
                        width: ancho_final,
                        height: alto_final,
                        quality: request.quality,
                        loop_forever: request.loop_forever,
                    },
                    |stage, done, total| emit(stage, done, total),
                )?;
            }
        }
        "mp4" => {
            let pasos = pasos_de(session, request);
            if request.engine == "ffmpeg" && ffmpeg::available() {
                let temporary = session.dir.join("export-source.mp4");
                encode_mp4(session, &pasos, &temporary, request, emit)?;
                emit("encoding", 0, 1);
                ffmpeg::mp4_from_video(
                    &temporary,
                    path,
                    request.fps,
                    ancho_final,
                    alto_final,
                    request.quality,
                )?;
                let _ = std::fs::remove_file(&temporary);
            } else {
                encode_mp4(session, &pasos, path, request, emit)?;
            }
        }
        other => return Err(AppError::Msg(format!("formato desconocido: {other}"))),
    }
    Ok(())
}

/// Recorta, escala, dibuja las marcas encima y despues pone el marco.
///
/// **Ese orden y no otro.**
///
/// El recorte va primero porque todo lo demas se mide sobre lo que queda: quien recorta un
/// trozo y pide 800 de ancho quiere ESE trozo a 800.
///
/// Las marcas van sobre la captura y no sobre el fondo: una flecha dibujada en el 90 % del
/// ancho apunta al 90 % de la CAPTURA, y si se enmarcara antes, ese 90 % caeria dentro del
/// aire de la derecha. Y se escala antes de pintarlas para que las coordenadas, que van de
/// 0 a 1, se apliquen sobre la imagen del tamanno final.
///
/// Las marcas se dibujan sobre la vista previa entera, asi que al recortar hay que volver
/// a medirlas sobre el trozo: una flecha en el centro de la captura no esta en el centro
/// del recorte.
/// Lo que hay que saber para dibujar el estudio sobre un fotograma concreto.
///
/// Lleva la cache de pastillas dentro porque dibujar el texto de un atajo con GDI cuesta, y
/// a treinta fotogramas por segundo el mismo atajo se dibuja una y otra vez.
struct Estudio<'a> {
    clics: &'a [zoom::Clic],
    teclas: &'a [crate::record::teclas::Atajo],
    rastro: &'a [(u64, i32, i32)],
    formas: &'a [(u64, u8)],
    /// Las imagenes de verdad de los punteros, cargadas del disco una vez por exportacion.
    punteros: Vec<estudio::PunteroCargado>,
    cambios_puntero: &'a [(u64, u32)],
    movimientos: Vec<u64>,
    origen: (u32, u32),
    ajustes: estudio::Ajustes,
    ms: u64,
    caches: estudio::Caches,
}

impl Estudio<'_> {
    fn pintar(&mut self, imagen: &mut image::RgbaImage, recortes: &[Recorte]) {
        let fuente = estudio::Fuente {
            clics: self.clics,
            atajos: self.teclas,
            rastro: self.rastro,
            formas: self.formas,
            punteros: &self.punteros,
            cambios_puntero: self.cambios_puntero,
            movimientos: &self.movimientos,
        };
        estudio::pintar(
            imagen,
            self.ms,
            &fuente,
            self.origen,
            recortes,
            &self.ajustes,
            &mut self.caches,
        );
    }
}

fn estudio_de<'a>(session: &'a SessionData, request: &ExportRequest) -> Option<Estudio<'a>> {
    let ajustes = estudio::Ajustes {
        clics: request.clicks,
        teclas: request.keys,
        cursor: request.cursor,
        suave: request.smooth_cursor,
        esconder: request.hide_idle_cursor,
    };
    if !ajustes.hay_algo() {
        return None;
    }
    // Los punteros que se leyeron al grabar. Uno que no se pueda abrir se queda fuera y
    // en su tramo se dibuja la forma a mano.
    let punteros = session
        .punteros
        .iter()
        .filter_map(|p| {
            image::open(&p.archivo).ok().map(|imagen| estudio::PunteroCargado {
                id: p.id,
                caliente: p.caliente,
                imagen: imagen.to_rgba8(),
            })
        })
        .collect();
    Some(Estudio {
        clics: &session.clics,
        teclas: &session.teclas,
        rastro: &session.cursor,
        formas: &session.formas,
        punteros,
        cambios_puntero: &session.cambios_puntero,
        movimientos: estudio::movimientos(&session.cursor),
        origen: (session.width.max(1), session.height.max(1)),
        ajustes,
        ms: 0,
        caches: estudio::Caches::default(),
    })
}

/// La camara del zoom, lista para preguntarle por cualquier fotograma.
///
/// Se prepara una vez por exportacion: agrupar los clics en tramos cuesta lo mismo para uno
/// que para trescientos fotogramas, y hacerlo dentro del bucle seria repetirlo trescientas
/// veces para obtener siempre lo mismo.
struct Camara {
    tramos: Vec<zoom::Tramo>,
    /// Donde estuvo el raton, ya trasladado al trozo que se exporta. La camara lo sigue
    /// mientras esta acercada, en vez de quedarse clavada en el punto del clic.
    rastro: Vec<(u64, i32, i32)>,
    ajustes: zoom::Ajustes,
    /// La inercia del centro. Con estado, y por eso los fotogramas se le piden en orden:
    /// es lo que evita que un clic en la otra punta mueva la camara de golpe.
    seguimiento: std::cell::RefCell<zoom::Seguimiento>,
    /// El tamanno sobre el que se mide, que es el de la imagen YA recortada por el usuario.
    ancho: u32,
    alto: u32,
}

impl Camara {
    fn preparar(session: &SessionData, request: &ExportRequest) -> Option<Self> {
        Self::nueva(session, request.zoom, request.crop, request.zoom_speed)
    }

    /// La misma camara que usa la exportacion, para quien la quiera preguntar por su
    /// cuenta: la vista previa del editor la usa para ensennar el zoom antes de exportar.
    fn nueva(session: &SessionData, zoom: f32, crop: Option<Recorte>, velocidad: f32) -> Option<Self> {
        // Menos de 1,05 no se ve y solo cuesta trabajo: es apagado, dicho con un numero.
        if zoom < 1.05 || (session.clics.is_empty() && session.teclas.is_empty()) {
            return None;
        }
        let (ancho, alto) = (session.width.max(1), session.height.max(1));
        // Los clics estan en pixeles de la region grabada. Si el usuario recorto, la camara
        // se mueve dentro de ESE trozo, asi que hay que trasladarlos y descartar los que
        // se quedaron fuera: acercarse a un clic que ya no se ve seria acercarse a nada.
        let (dx, dy, ancho, alto) = match crop {
            Some(r) => {
                let (x, y, w, h) = r.en_pixeles(ancho, alto);
                (x as i32, y as i32, w, h)
            }
            None => (0, 0, ancho, alto),
        };
        // La camara se acerca a los clics **y a los atajos**: quien pulsa Ctrl+C esta
        // mirando a algun lado, y ese lado es donde tiene el raton. Un atajo sin sitio
        // propio dejaria la mitad de un tutorial sin acercarse a nada.
        let mut puntos: Vec<zoom::Clic> = session
            .clics
            .iter()
            .copied()
            .chain(session.teclas.iter().map(|a| zoom::Clic {
                ms: a.ms,
                x: a.x,
                y: a.y,
                derecho: false,
            }))
            .collect();
        // Van mezclados en el tiempo, y agrupar en tramos exige que vengan en orden.
        puntos.sort_by_key(|c| c.ms);
        let clics: Vec<zoom::Clic> = puntos
            .iter()
            .map(|c| zoom::Clic {
                ms: c.ms,
                x: c.x - dx,
                y: c.y - dy,
                derecho: c.derecho,
            })
            .filter(|c| c.x >= 0 && c.y >= 0 && c.x < ancho as i32 && c.y < alto as i32)
            .collect();
        if clics.is_empty() {
            return None;
        }
        let ajustes = zoom::Ajustes::a_velocidad(zoom.min(4.0), velocidad);
        let rastro = session
            .cursor
            .iter()
            .map(|(t, x, y)| (*t, x - dx, y - dy))
            .collect();
        Some(Self {
            tramos: zoom::tramos(&clics, &ajustes),
            rastro,
            ajustes,
            ancho,
            alto,
            seguimiento: std::cell::RefCell::new(zoom::Seguimiento::default()),
        })
    }
}

/// Donde mira la camara en un fotograma, como trozo de la imagen ENTERA (de 0 a 1).
///
/// Es lo que la vista previa del editor necesita para ensennar el zoom antes de exportar:
/// el mismo encuadre que va a usar el exportador en ese instante, ya compuesto con el
/// recorte del usuario, para que se pueda aplicar directamente sobre el video de la vista
/// previa sin volver a hacer ninguna cuenta.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MuestraCamara {
    pub ms: u64,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

/// La camara del zoom, muestreada en cada fotograma de la sesion.
///
/// Sale de la MISMA camara que usa la exportacion, asi que lo que ensenna la vista previa
/// es lo que va a salir: antes el zoom solo se veia en el archivo ya exportado, y quien lo
/// ponia tenia que exportar, abrir el video, volver, tocar y exportar otra vez.
/// Cada cuánto se pregunta a la cámara para la vista previa: lo mismo que un paso de una
/// exportación a 30 fotogramas por segundo, que es como se exporta de fábrica.
const PASO_DE_CAMARA_MS: u64 = 33;

pub fn muestras_de_camara(
    session: &SessionData,
    zoom: f32,
    crop: Option<Recorte>,
    velocidad: f32,
) -> Vec<MuestraCamara> {
    let camara = Camara::nueva(session, zoom, crop, velocidad);
    // El recorte del usuario, como esta en la imagen entera. La camara se mueve dentro.
    let (ux1, uy1, ux2, uy2) = match crop {
        Some(r) => {
            let (x, y, w, h) = r.en_pixeles(session.width.max(1), session.height.max(1));
            let (aw, ah) = (session.width.max(1) as f32, session.height.max(1) as f32);
            (x as f32 / aw, y as f32 / ah, (x + w) as f32 / aw, (y + h) as f32 / ah)
        }
        None => (0.0, 0.0, 1.0, 1.0),
    };
    // A ritmo fijo, como el exportador, y no una muestra por fotograma guardado: sobre una
    // pantalla quieta (un fotograma de varios segundos) la vista previa del zoom pegaba un
    // salto de una muestra a la siguiente en vez de seguir al raton.
    let total = session.duration_ms();
    let mut muestras: Vec<MuestraCamara> = (0..=total / PASO_DE_CAMARA_MS)
        .map(|k| {
            let ms = k * PASO_DE_CAMARA_MS;
            let (x1, y1, x2, y2) = match camara.en(ms) {
                // El encuadre de la camara se mide sobre el trozo recortado; se lleva a
                // la imagen entera para que la vista previa lo aplique tal cual.
                Some(r) => (
                    ux1 + r.x1 * (ux2 - ux1),
                    uy1 + r.y1 * (uy2 - uy1),
                    ux1 + r.x2 * (ux2 - ux1),
                    uy1 + r.y2 * (uy2 - uy1),
                ),
                None => (ux1, uy1, ux2, uy2),
            };
            MuestraCamara { ms, x1, y1, x2, y2 }
        })
        .collect();
    // Las rachas quietas se quedan en su primera y su ultima muestra: la vista previa
    // interpola entre muestras seguidas, asi que lo de en medio no cambia nada y diez
    // minutos sin zoom no viajan como dieciocho mil muestras iguales.
    let igual = |a: &MuestraCamara, b: &MuestraCamara| {
        (a.x1, a.y1, a.x2, a.y2) == (b.x1, b.y1, b.x2, b.y2)
    };
    let mut quedan = Vec::with_capacity(muestras.len());
    for i in 0..muestras.len() {
        let antes = i > 0 && igual(&muestras[i - 1], &muestras[i]);
        let despues = i + 1 < muestras.len() && igual(&muestras[i + 1], &muestras[i]);
        if !(antes && despues) {
            quedan.push(muestras[i]);
        }
    }
    muestras.clear();
    quedan
}

/// Un `Option<Camara>` sabe contestar igual que una: sin zoom, no recorta nada.
trait EnElInstante {
    fn en(&self, ms: u64) -> Option<Recorte>;
}

impl EnElInstante for Option<Camara> {
    fn en(&self, ms: u64) -> Option<Recorte> {
        let c = self.as_ref()?;
        let objetivo = zoom::siguiendo(&c.tramos, &c.rastro, ms, c.ancho, c.alto, &c.ajustes);
        let mirando = c
            .seguimiento
            .borrow_mut()
            .avanzar(objetivo, ms, c.ancho, &c.ajustes);
        (mirando.escala > 1.001).then(|| mirando.como_recorte(c.ancho, c.alto))
    }
}

#[allow(clippy::too_many_arguments)]
fn enmarcar_y_anotar(
    imagen: image::RgbaImage,
    ancho: u32,
    alto: u32,
    marco: marco::Marco,
    anotaciones: &[anotacion::Anotacion],
    recortes: &[Recorte],
    estudio: Option<&mut Estudio<'_>>,
) -> image::RgbaImage {
    // Los recortes se encadenan, y cada uno se mide sobre lo que dejo el anterior. Son
    // dos: el que puso el usuario en el editor y el que pide la camara del zoom, que
    // cambia en cada fotograma. Los dos hacen lo mismo, asi que hacen lo mismo.
    let mut recortada = imagen;
    let mut usados: Vec<Recorte> = Vec::new();
    for r in recortes {
        if r.recorta_algo(recortada.width(), recortada.height()) {
            recortada = r.aplicar(&recortada);
            usados.push(*r);
        }
    }
    // Escalar es lo mas caro de exportar, y con el zoom pasa por aqui CADA fotograma: sin
    // zoom la imagen ya mide lo que se pide y no se toca, con zoom hay que estirar el
    // trozo. Con `image` eso costaba 64 ms por fotograma; `escalar::a_medida` hace lo
    // mismo en dos.
    let mut escalada = if recortada.dimensions() == (ancho, alto) {
        recortada
    } else {
        escalar::a_medida(&recortada, ancho, alto)
    };
    // El estudio va ANTES que las anotaciones: los aros y la pastilla son parte de la
    // grabacion, y lo que dibuja el usuario encima manda sobre ellos.
    if let Some(e) = estudio {
        e.pintar(&mut escalada, &usados);
    }
    if !anotaciones.is_empty() {
        // Cada recorte vuelve a medir las marcas, en el mismo orden en que se aplicaron.
        // Lo que caiga fuera se sale de [0, 1] y lo recorta quien pinta.
        let mut marcas: Vec<anotacion::Anotacion> = anotaciones.to_vec();
        for r in &usados {
            marcas = marcas.iter().map(|a| r.reencuadrar(a)).collect();
        }
        anotacion::pintar(&mut escalada, &marcas);
    }
    marco::poner(&escalada, marco)
}

fn encode_mp4<F>(
    session: &SessionData,
    pasos: &Pasos,
    path: &Path,
    request: &ExportRequest,
    emit: &F,
) -> Result<()>
where
    F: Fn(&str, usize, usize) + ?Sized,
{
    let (ancho_final, alto_final) =
        marco_de(request).medida(request.width.max(1), request.height.max(1));
    let mut vestidor = Vestidor::nuevo(session, request, pasos);
    let mut loader = |posicion: usize| vestidor.en(posicion);
    mp4::encode(
        &pasos.posiciones(),
        &pasos.retardos,
        &mut loader,
        path,
        &mp4::Mp4Options {
            width: ancho_final,
            height: alto_final,
            fps: request.fps.max(1),
            quality: request.quality,
        },
        // El interruptor del editor manda: alguien puede querer el vídeo mudo. Y a
        // velocidad distinta de la real no hay sonido que valga: acelerar la pista sin
        // tocar el tono es otro problema entero, y dejarla tal cual la despegaria de la
        // imagen desde el primer segundo. El panel lo dice antes de exportar.
        (request.audio && (request.velocidad() - 1.0).abs() < f32::EPSILON)
            .then(|| {
                let (desde, hasta) = request.tramo(session);
                pista_de_audio(session, desde, hasta).map(|p| con_volumen(p, request.volume))
            })
            .flatten(),
        |stage, done, total| emit(stage, done, total),
    )
}

/// El trozo de sonido que le toca al recorte que se esta exportando.
///
/// Es el MISMO tramo de reloj que la imagen (`ExportRequest::tramo`): si el sonido se
/// midiera por su cuenta, un corte a mitad de una pantalla quieta dejaria la voz
/// empezando antes que el video. Sin esto el video que guarda sale mudo, aunque la
/// grabacion tuviera sonido: exportar vuelve a codificar desde los fotogramas, y ahi no
/// hay audio ninguno.
fn pista_de_audio(session: &SessionData, desde: u64, hasta: u64) -> Option<mp4::Pista> {
    use std::io::{Seek, SeekFrom};

    let info = session.audio?;
    let (inicio, largo) = info.tramo(desde, hasta);
    if largo == 0 {
        return None;
    }

    let mut archivo = std::fs::File::open(session.audio_path()).ok()?;
    archivo.seek(SeekFrom::Start(inicio)).ok()?;
    let mut datos = vec![0u8; largo as usize];
    // Lo que haya: si la grabacion acabo antes de lo que dicen los fotogramas, se exporta
    // el sonido que exista en vez de fallar por unos milisegundos de menos.
    let leidos = leer_lo_que_haya(&mut archivo, &mut datos);
    datos.truncate(leidos);
    if datos.is_empty() {
        return None;
    }
    Some(mp4::Pista {
        channels: info.channels,
        sample_rate: info.sample_rate,
        datos,
    })
}

/// La pista a otro volumen. Las muestras son de 16 bits con signo, y lo que se pase del
/// tope se queda en el tope: subir el volumen satura antes que dar la vuelta, que sonaria
/// a chasquido.
fn con_volumen(mut pista: mp4::Pista, volumen: f32) -> mp4::Pista {
    let v = if volumen.is_finite() { volumen.clamp(0.0, 4.0) } else { 1.0 };
    if (v - 1.0).abs() < 0.005 {
        return pista;
    }
    for muestra in pista.datos.chunks_exact_mut(2) {
        let antes = i16::from_le_bytes([muestra[0], muestra[1]]) as f32;
        let ahora = (antes * v).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        muestra.copy_from_slice(&ahora.to_le_bytes());
    }
    pista
}

fn leer_lo_que_haya(archivo: &mut std::fs::File, destino: &mut [u8]) -> usize {
    use std::io::Read;
    let mut total = 0;
    while total < destino.len() {
        match archivo.read(&mut destino[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(_) => break,
        }
    }
    total
}

/// Una imagen se pega como imagen; un GIF o un MP4 se pegan como archivo.
///
/// Lo que se pega es **el archivo que se acaba de escribir**, no el fotograma del cache.
/// Leerlo del cache dejaba fuera todo lo que hace exportar: las marcas dibujadas encima,
/// el recorte, el marco, el escalado y el zoom. Munir lo vio el 29 de agosto de 2026
/// copiando una captura con una flecha roja: el archivo la tenia, y lo pegado no.
fn copy_result(path: &Path, request: &ExportRequest) -> Result<()> {
    match que_se_pega(path, request)? {
        ComoSePega::Imagen(image, bytes) => {
            crate::platform::clipboard::copy_image(&image, &bytes)
        }
        ComoSePega::Archivo(ruta) => crate::platform::clipboard::copy_files(&[&ruta]),
    }
}

/// Que se le va a dar al portapapeles.
enum ComoSePega {
    /// Los pixeles y los bytes PNG de lo exportado, para pegarlo como imagen.
    Imagen(image::RgbaImage, Vec<u8>),
    /// El archivo tal cual, para lo que no cabe en el portapapeles como imagen.
    Archivo(PathBuf),
}

/// La decision, separada de la ejecucion para poder probarla.
///
/// Aqui es donde estaba el fallo, y por eso esto no recibe la sesion: si algun dia alguien
/// vuelve a sacar los pixeles del cache en vez del archivo, esta funcion necesitara la
/// sesion otra vez y la prueba dejara de compilar.
fn que_se_pega(path: &Path, request: &ExportRequest) -> Result<ComoSePega> {
    if request.format == "png" {
        let (image, bytes) = imagen_escrita(path)?;
        Ok(ComoSePega::Imagen(image, bytes))
    } else {
        Ok(ComoSePega::Archivo(path.to_path_buf()))
    }
}

/// Lo que quedo en el archivo, en las dos formas que quiere el portapapeles de Windows:
/// los pixeles para el mapa de bits y los bytes tal cual para el formato PNG.
///
/// Va aparte de `copy_result` para poder probarlo: comprobar que lo copiado lleva la
/// flecha no puede depender de tocarle el portapapeles a nadie.
fn imagen_escrita(path: &Path) -> Result<(image::RgbaImage, Vec<u8>)> {
    let bytes = std::fs::read(path)?;
    let image = image::load_from_memory(&bytes)?.to_rgba8();
    Ok((image, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que eligio el usuario en «Guardar en…» se respeta, y solo se le pone la
    /// extension cuando no la trae: ni se le cambia el nombre ni se le quita la carpeta.
    #[test]
    fn la_ruta_elegida_lleva_siempre_su_extension() {
        assert_eq!(ruta_elegida(r"C:\Proyectos\demo\captura", "png"), PathBuf::from(r"C:\Proyectos\demo\captura.png"));
        assert_eq!(ruta_elegida(r"C:\Proyectos\demo\captura.png", "png"), PathBuf::from(r"C:\Proyectos\demo\captura.png"));
        assert_eq!(ruta_elegida(r"C:\Proyectos\demo\captura.PNG", "png"), PathBuf::from(r"C:\Proyectos\demo\captura.PNG"));
        // Una extension que no es la del formato no vale: un MP4 llamado .txt no lo abre nadie.
        assert_eq!(ruta_elegida(r"C:\demo\video.txt", "mp4"), PathBuf::from(r"C:\demo\video.txt.mp4"));
        // Un punto en medio del nombre no es una extension.
        assert_eq!(ruta_elegida(r"C:\demo\v1.2 final", "gif"), PathBuf::from(r"C:\demo\v1.2 final.gif"));
    }
    use crate::capture::Rect;
    use crate::record::FrameEntry;

    pub(super) fn session_with(durations: &[u32]) -> SessionData {
        let mut timestamp = 0u64;
        let frames = durations
            .iter()
            .enumerate()
            .map(|(index, duration)| {
                let entry = FrameEntry {
                    index: index as u32,
                    timestamp_ms: timestamp,
                    duration_ms: *duration,
                    thumb_path: String::new(),
                    offset: 0,
                    len: 0,
                    patch: None,
                    parches: Vec::new(),
                };
                timestamp += *duration as u64;
                entry
            })
            .collect();
        SessionData {
            id: "test".into(),
            dir: PathBuf::from("."),
            region: Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            fps: 50,
            format: "gif".into(),
            has_audio: false,
            width: 10,
            height: 10,
            mp4_path: None,
            audio: None,
            clics: Vec::new(),
            teclas: Vec::new(),
            cursor: Vec::new(),
            cursor_capturado: false,
            formas: Vec::new(),
            punteros: Vec::new(),
            cambios_puntero: Vec::new(),
            marcas_zoom: Vec::new(),
            camara: None,
            escritura: Vec::new(),
            frames,
        }
    }

    /// Sobre una pantalla quieta (UN fotograma de tres segundos) con el raton moviendose, la
    /// camara de la vista previa tiene que seguirlo: antes daba una muestra por fotograma
    /// guardado, o sea una, y la vista previa saltaba de golpe al final.
    #[test]
    fn la_camara_de_la_vista_previa_sigue_al_raton_sobre_una_pantalla_quieta() {
        let mut session = session_with(&[3_000]);
        session.width = 400;
        session.height = 300;
        session.clics.push(zoom::Clic { ms: 600, x: 100, y: 150, derecho: false });
        session.cursor = (0..=180u64).map(|k| (k * 16, 100 + k as i32, 150)).collect();
        let muestras = muestras_de_camara(&session, 2.0, None, 1.0);
        let acercadas: Vec<&MuestraCamara> = muestras.iter().filter(|m| m.x2 - m.x1 < 0.9).collect();
        assert!(acercadas.len() > 20, "solo {} muestras acercadas", acercadas.len());
        let (primera, ultima) = (acercadas[0], acercadas[acercadas.len() - 1]);
        assert!(ultima.x1 > primera.x1, "la camara no ha seguido al raton: {primera:?} -> {ultima:?}");
    }

    /// La camara muestreada para la vista previa dice lo mismo que la de exportar: sin
    /// zoom o sin clics es la imagen entera, y con zoom se acerca en el momento del clic
    /// sin salirse nunca de la imagen ni del recorte del usuario.
    #[test]
    fn la_camara_muestreada_se_acerca_al_clic_y_no_se_sale() {
        let mut session = session_with(&[20; 20]);
        let entera = |m: &MuestraCamara| m.x1 == 0.0 && m.y1 == 0.0 && m.x2 == 1.0 && m.y2 == 1.0;

        // Sin clics no hay a donde acercarse, con zoom o sin el.
        assert!(muestras_de_camara(&session, 2.0, None, 1.0).iter().all(entera));

        session.clics.push(zoom::Clic {
            ms: 200,
            x: 5,
            y: 5,
            derecho: false,
        });
        assert!(muestras_de_camara(&session, 1.0, None, 1.0).iter().all(entera), "sin zoom, entera");

        let muestras = muestras_de_camara(&session, 2.0, None, 1.0);
        assert!(muestras.windows(2).all(|p| p[1].ms > p[0].ms), "en orden");
        let en_el_clic = muestras
            .iter()
            .min_by_key(|m| m.ms.abs_diff(200))
            .expect("hay muestras");
        assert!(!entera(en_el_clic), "en el clic tiene que estar acercada");
        assert!(
            (en_el_clic.x2 - en_el_clic.x1 - 0.5).abs() < 0.11,
            "al doble se ve la mitad del ancho: {en_el_clic:?}"
        );
        for m in &muestras {
            assert!(m.x1 >= 0.0 && m.y1 >= 0.0 && m.x2 <= 1.0 && m.y2 <= 1.0, "{m:?}");
            assert!(m.x1 < m.x2 && m.y1 < m.y2, "{m:?}");
        }

        // Con el recorte del usuario, la camara se mueve DENTRO de ese trozo.
        let mitad_derecha = Recorte {
            x1: 0.5,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        session.clics[0].x = 8;
        for m in muestras_de_camara(&session, 2.0, Some(mitad_derecha), 1.0) {
            assert!(m.x1 >= 0.5 - 0.001 && m.x2 <= 1.0, "se sale del recorte: {m:?}");
        }
    }

    /// Dos clics en puntas opuestas, uno detras de otro: el encuadre muestreado se mueve
    /// de uno al otro sin saltos, nunca mas de lo que permite la velocidad maxima entre dos
    /// fotogramas seguidos. Es la queja de Munir del 12 de septiembre de 2026.
    #[test]
    fn la_camara_muestreada_no_pega_saltos_entre_dos_clics_lejanos() {
        let mut session = session_with(&[33; 120]);
        session.width = 1920;
        session.height = 1080;
        session.region.width = 1920;
        session.region.height = 1080;
        session.clics = vec![
            zoom::Clic { ms: 500, x: 150, y: 500, derecho: false },
            zoom::Clic { ms: 1300, x: 1750, y: 500, derecho: false },
        ];
        // El raton salta con el clic: en 500 esta a la izquierda y en 1300 a la derecha.
        session.cursor = (0..120u64)
            .map(|i| {
                let ms = i * 33;
                (ms, if ms < 1300 { 150 } else { 1750 }, 500)
            })
            .collect();
        let muestras = muestras_de_camara(&session, 2.0, None, 1.0);
        let ajustes = zoom::Ajustes::default();
        let tope = ajustes.velocidad_max * 33.0 / 1000.0 + 0.002;
        let mut cruzo = false;
        for par in muestras.windows(2) {
            // Solo mientras la camara esta acercada y quieta de escala: al acercarse o
            // alejarse el encuadre cambia de tamanno, y eso mueve el borde sin ser un salto.
            let ancho_antes = par[0].x2 - par[0].x1;
            let ancho_ahora = par[1].x2 - par[1].x1;
            if (ancho_antes - ancho_ahora).abs() > 1e-4 {
                continue;
            }
            let salto = (par[1].x1 - par[0].x1).abs();
            assert!(salto <= tope, "entre {} y {} ms el encuadre salta {salto} (tope {tope})", par[0].ms, par[1].ms);
            if par[1].x1 > 0.4 {
                cruzo = true;
            }
        }
        assert!(cruzo, "la camara tenia que acabar mirando a la derecha");
    }

    #[test]
    fn bajar_los_fps_reparte_el_tiempo_sin_perderlo() {
        // 10 fotogramas de 20 ms = 200 ms de clip, exportados a 25 fps (40 ms).
        let session = session_with(&[20; 10]);
        let (indices, delays) = resample(&session, 0, 9, 25, 1.0);
        assert_eq!(indices.len(), 5);
        assert_eq!(delays.iter().sum::<u32>(), 200);
    }

    /// Acelerar es coger un fotograma de cada dos, no meterles retardos de cinco
    /// milisegundos: asi el resultado sigue teniendo los fps que se pidieron.
    #[test]
    fn al_doble_de_velocidad_dura_la_mitad() {
        // 40 fotogramas de 25 ms = un segundo, exportado a 20 fps.
        let session = session_with(&[25; 40]);
        let (normal, retardos_normales) = resample(&session, 0, 39, 20, 1.0);
        let (rapido, retardos_rapidos) = resample(&session, 0, 39, 20, 2.0);

        assert_eq!(retardos_normales.iter().sum::<u32>(), 1_000);
        assert_eq!(retardos_rapidos.iter().sum::<u32>(), 500, "a 2x tiene que durar la mitad");
        assert!(
            rapido.len() < normal.len(),
            "a 2x entran menos fotogramas: {} contra {}",
            rapido.len(),
            normal.len()
        );
        // Y cada fotograma sigue durando lo que dura uno a 20 fps.
        assert!(retardos_rapidos.iter().all(|d| (45..=55).contains(d)));
    }

    /// **Y que llegue al archivo.** Los retardos que salen de `resample` son una lista de
    /// numeros; lo que hay que ver es que el GIF que se abre en un navegador dure la mitad.
    /// Es la leccion del audio: las piezas verdes no son la funcion hecha.
    #[test]
    fn el_gif_exportado_a_2x_dura_de_verdad_la_mitad() {
        let dir = std::env::temp_dir().join("winshotx-velocidad");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Un segundo de grabacion: 40 fotogramas de 25 ms, cada uno de otro color.
        let (ancho, alto) = (16u32, 16u32);
        let mut lienzos = Vec::new();
        for paso in 0..40u32 {
            let mut rgba = vec![255u8; (ancho * alto) as usize * 4];
            for (i, byte) in rgba.iter_mut().enumerate() {
                if i % 4 == 0 {
                    *byte = (paso * 6) as u8;
                }
            }
            lienzos.push(rgba);
        }

        let duracion_de = |velocidad: f32| -> u64 {
            let session = session_with(&[25; 40]);
            let (indices, delays) = resample(&session, 0, 39, 20, velocidad);
            let salida = dir.join(format!("v{velocidad}.gif"));
            let mut muestra = |index: usize| {
                Ok(image::RgbaImage::from_raw(ancho, alto, lienzos[index].clone()).unwrap())
            };
            let mut loader = |index: usize| {
                Ok(image::RgbaImage::from_raw(ancho, alto, lienzos[index].clone()).unwrap())
            };
            crate::encode::gif::encode(
                &indices,
                &delays,
                &mut muestra,
                &mut loader,
                &salida,
                &crate::encode::gif::GifOptions {
                    width: ancho,
                    height: alto,
                    quality: 80,
                    loop_forever: true,
                },
                |_, _, _| {},
            )
            .unwrap();

            // Se relee del disco, y a mano: cada fotograma de un GIF lleva delante un
            // bloque `21 F9 04` con su retardo en centesimas de segundo. Ese redondeo a
            // centesimas solo se ve mirando el archivo, que es justo lo que hay que mirar.
            let bytes = std::fs::read(&salida).unwrap();
            bytes
                .windows(6)
                .filter(|v| v[0] == 0x21 && v[1] == 0xF9 && v[2] == 0x04)
                .map(|v| (u64::from(v[4]) | (u64::from(v[5]) << 8)) * 10)
                .sum()
        };

        let normal = duracion_de(1.0);
        let rapido = duracion_de(2.0);
        let lento = duracion_de(0.5);
        println!("1x: {normal} ms · 2x: {rapido} ms · 0,5x: {lento} ms");

        assert!((950..=1_050).contains(&normal), "a 1x salen {normal} ms de un segundo");
        assert!((450..=550).contains(&rapido), "a 2x salen {rapido} ms");
        assert!((1_900..=2_100).contains(&lento), "a 0,5x salen {lento} ms");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_camara_lenta_dura_el_doble() {
        let session = session_with(&[25; 40]);
        let (_, retardos) = resample(&session, 0, 39, 20, 0.5);
        assert_eq!(retardos.iter().sum::<u32>(), 2_000);
    }

    #[test]
    fn los_fotogramas_repetidos_alargan_el_retardo() {
        // Un solo fotograma que dura 500 ms no debe repetirse 12 veces a 25 fps.
        let session = session_with(&[500]);
        let (indices, delays) = resample(&session, 0, 0, 25, 1.0);
        assert_eq!(indices, vec![0]);
        assert_eq!(delays, vec![500]);
    }

    #[test]
    fn el_rango_recortado_manda() {
        let session = session_with(&[50; 8]);
        let (indices, _) = resample(&session, 2, 4, 20, 1.0);
        assert!(indices.iter().all(|i| (2..=4).contains(i)));
    }

    /// Una sesion sin fotogramas no puede reventar. `resample` hacia `&frames[0..=0]`
    /// sobre una lista vacia, que es un panic y no un error: en vez de decir que no hay
    /// nada que exportar, la exportacion se quedaba sin respuesta.
    #[test]
    fn una_sesion_sin_fotogramas_no_revienta() {
        let vacia = session_with(&[]);
        let (indices, delays) = resample(&vacia, 0, 0, 30, 1.0);
        assert!(indices.is_empty(), "de una lista vacia no sale ningun indice");
        assert!(delays.is_empty(), "ni ninguna duracion");
    }

    /// Diez segundos de pantalla quieta son UN fotograma. Desde la linea de tiempo se tiene
    /// que poder cortar por dentro de el, que es el caso normal: quitar el rato que se
    /// tardo en ir a pulsar parar.
    #[test]
    fn se_puede_cortar_por_dentro_de_una_pantalla_quieta() {
        let session = session_with(&[100, 10_000, 100]);
        let (indices, delays) = resample_tramo(&session, 3_100, 5_100, 30, 1.0);
        assert_eq!(indices, vec![1], "solo se ve la quietud");
        assert_eq!(delays.iter().sum::<u32>(), 2_000, "y dura los dos segundos cortados");
    }

    #[test]
    fn el_tramo_dura_exactamente_lo_que_se_corto() {
        let session = session_with(&[33; 300]);
        let (_, delays) = resample_tramo(&session, 1_234, 5_678, 30, 1.0);
        assert_eq!(delays.iter().sum::<u32>(), 5_678 - 1_234);
    }

    #[test]
    fn el_primero_es_el_que_se_estaba_viendo_en_el_corte() {
        let session = session_with(&[100, 100, 100]);
        let (indices, _) = resample_tramo(&session, 150, 300, 20, 1.0);
        assert_eq!(indices.first(), Some(&1));
    }

    #[test]
    fn ninguno_que_empiece_en_el_corte_final_se_cuela() {
        let session = session_with(&[100, 100, 100]);
        let (indices, _) = resample_tramo(&session, 0, 200, 20, 1.0);
        assert!(!indices.contains(&2), "el tercero empieza en 200: {indices:?}");
    }

    #[test]
    fn sin_milisegundos_se_corta_por_fotogramas_como_siempre() {
        let session = session_with(&[50; 8]);
        assert_eq!(tramo_de(&session, 2, 4, None, None), (100, 250));
    }

    #[test]
    fn con_milisegundos_manda_el_reloj() {
        let session = session_with(&[50; 8]);
        assert_eq!(tramo_de(&session, 0, 7, Some(120), Some(330)), (120, 330));
    }

    #[test]
    fn un_tramo_que_se_sale_de_la_grabacion_se_mete_dentro() {
        let session = session_with(&[50; 8]);
        assert_eq!(tramo_de(&session, 0, 7, Some(0), Some(90_000)), (0, 400));
        let (desde, hasta) = tramo_de(&session, 0, 7, Some(9_000), Some(9_500));
        assert!(desde < hasta && hasta <= 400, "{desde}..{hasta}");
    }

    #[test]
    fn un_tramo_al_reves_no_deja_un_archivo_vacio() {
        let session = session_with(&[50; 8]);
        let (desde, hasta) = tramo_de(&session, 0, 7, Some(300), Some(100));
        assert!(hasta > desde, "{desde}..{hasta}");
    }

    /// El sonido sale del MISMO tramo de reloj que la imagen, al milisegundo: cortado a
    /// mitad de una pantalla quieta, la voz no puede empezar donde empezaba el fotograma.
    #[test]
    fn el_sonido_sale_del_mismo_tramo_que_la_imagen() {
        let dir = std::env::temp_dir().join(format!("winshotx-pista-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut session = session_with(&[100, 10_000, 100]);
        session.dir = dir.clone();
        session.audio = Some(record::AudioInfo {
            channels: 2,
            sample_rate: 48_000,
        });
        // Cada milisegundo lleva escrito su numero, para saber que trozo ha salido.
        let por_ms = 192usize;
        let mut pcm = Vec::with_capacity(10_200 * por_ms);
        for ms in 0..10_200u32 {
            pcm.extend(std::iter::repeat_n((ms % 251) as u8, por_ms));
        }
        std::fs::write(session.audio_path(), &pcm).unwrap();

        let pista = pista_de_audio(&session, 3_100, 5_100).expect("sin pista");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(pista.datos.len(), 2_000 * por_ms, "dura lo que el corte");
        assert_eq!(pista.datos[0], (3_100 % 251) as u8, "empieza en el corte");
    }
}

/// El orden entero de exportar una imagen: recortar, escalar, anotar y enmarcar.
///
/// Cada paso por separado ya tiene sus pruebas. Esto comprueba lo unico que ninguna de
/// ellas puede ver: que van en ese orden. Un recorte despues de escalar da el mismo trozo
/// a otro tamanno, y una marca sin reencuadrar apunta a otro sitio, y las dos cosas dejan
/// todas las pruebas de abajo en verde.
#[cfg(test)]
mod el_orden_de_exportar {
    use super::*;

    const ROJO: image::Rgba<u8> = image::Rgba([255, 0, 0, 255]);
    const AZUL: image::Rgba<u8> = image::Rgba([0, 0, 255, 255]);

    /// Mitad izquierda roja, mitad derecha azul: asi se sabe que trozo ha salido.
    fn dos_mitades(ancho: u32, alto: u32) -> image::RgbaImage {
        image::RgbaImage::from_fn(ancho, alto, |x, _| if x < ancho / 2 { ROJO } else { AZUL })
    }

    /// Una peticion de exportar PNG, la de siempre, para las pruebas que la necesitan.
    fn peticion_png() -> ExportRequest {
        ExportRequest {
            session_id: "s".into(),
            format: "png".into(),
            engine: "native".into(),
            from: 0,
            to: 0,
            desde_ms: None,
            hasta_ms: None,
            width: 200,
            height: 200,
            fps: 30,
            quality: 90,
            audio: false,
            loop_forever: false,
            margin: 0,
            background: String::new(),
            shadow: false,
            annotations: Vec::new(),
            crop: None,
            file: None,
            zoom: 0.0,
            clicks: false,
            keys: false,
            cursor: 0.0,
            speed: 1.0,
            smooth_cursor: false,
            hide_idle_cursor: false,
            corners: 0,
            volume: 1.0,
            zoom_speed: 1.0,
            edicion: Default::default(),
            destination: None,
            copy_to_clipboard: true,
        }
    }

    fn sin_marco() -> marco::Marco {
        marco::Marco {
            margen: 0,
            fondo: marco::Fondo::desde("blanco"),
            sombra: false,
            esquinas: 0,
        }
    }

    fn mitad_derecha() -> Recorte {
        Recorte {
            x1: 0.5,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        }
    }

    #[test]
    fn recortar_la_mitad_derecha_deja_solo_la_mitad_derecha() {
        let salida = enmarcar_y_anotar(
            dos_mitades(400, 300),
            200,
            300,
            sin_marco(),
            &[],
            &[mitad_derecha()],
            None,
        );
        assert_eq!(salida.dimensions(), (200, 300));
        assert_eq!(*salida.get_pixel(4, 150), AZUL);
        assert_eq!(*salida.get_pixel(195, 150), AZUL);
    }

    #[test]
    fn se_recorta_antes_de_escalar_y_no_al_reves() {
        // Recortar despues de escalar daria el mismo trozo, pero de 200 de ancho en vez
        // de los 400 que se pidieron. La medida es lo unico que separa los dos ordenes.
        let salida = enmarcar_y_anotar(
            dos_mitades(400, 300),
            400,
            300,
            sin_marco(),
            &[],
            &[mitad_derecha()],
            None,
        );
        assert_eq!(salida.dimensions(), (400, 300));
        assert_eq!(*salida.get_pixel(10, 150), AZUL, "ha entrado parte de la mitad roja");
    }

    /// Lo que se pega tiene que ser lo que se exporto, con las marcas dentro.
    ///
    /// El fallo, que vio Munir el 29 de agosto de 2026: copiar leia el fotograma del
    /// CACHE, asi que una captura con una flecha se guardaba con flecha y se pegaba sin
    /// ella. Y sin ella se pegaban tambien el recorte, el marco, la escala y el zoom.
    ///
    /// Aqui se escribe un PNG ya anotado y se comprueba lo que sale para el portapapeles.
    /// No se toca el portapapeles de verdad a proposito: es de quien este delante.
    #[test]
    fn lo_que_se_copia_lleva_las_marcas_dibujadas() {
        let marca = anotacion::Anotacion {
            kind: "box".into(),
            x1: 0.25,
            y1: 0.25,
            x2: 0.75,
            y2: 0.75,
            color: "#00ff00".into(),
            text: String::new(),
        };
        let anotada = enmarcar_y_anotar(
            image::RgbaImage::from_pixel(200, 200, AZUL),
            200,
            200,
            sin_marco(),
            std::slice::from_ref(&marca),
            &[],
            None,
        );
        let unico = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let destino = std::env::temp_dir().join(format!("winshotx-copiar-{unico}.png"));
        png::save(&anotada, &destino, 200, 200).expect("no se ha escrito el PNG");

        // Por el mismo camino que sigue el boton de copiar, no por un atajo.
        let peticion = peticion_png();
        let ComoSePega::Imagen(imagen, bytes) =
            que_se_pega(&destino, &peticion).expect("no se ha podido leer lo escrito")
        else {
            panic!("una captura tiene que pegarse como imagen, no como archivo");
        };

        assert_eq!(imagen.dimensions(), (200, 200));
        let verde = imagen
            .pixels()
            .any(|p| p.0[1] > 200 && p.0[0] < 80 && p.0[2] < 80);
        assert!(verde, "lo que se copia no lleva la marca que el archivo si tiene");
        assert_eq!(
            bytes,
            std::fs::read(&destino).unwrap(),
            "los bytes del portapapeles no son los del archivo escrito"
        );
        let _ = std::fs::remove_file(&destino);
    }

    #[test]
    fn un_video_se_pega_como_archivo() {
        // Un MP4 no cabe en el portapapeles como imagen: se pega el archivo, que es lo
        // que entienden el Explorador, Slack o Discord.
        let peticion = ExportRequest {
            format: "mp4".into(),
            ..peticion_png()
        };
        let ruta = std::env::temp_dir().join("winshotx-no-existe.mp4");
        let ComoSePega::Archivo(pegado) =
            que_se_pega(&ruta, &peticion).expect("decidir no puede fallar")
        else {
            panic!("un vídeo no se pega como imagen");
        };
        assert_eq!(pegado, ruta);
    }

    #[test]
    fn sin_recorte_sale_la_captura_entera() {
        let salida = enmarcar_y_anotar(dos_mitades(400, 300), 400, 300, sin_marco(), &[], &[], None);
        assert_eq!(*salida.get_pixel(10, 150), ROJO);
        assert_eq!(*salida.get_pixel(390, 150), AZUL);
    }

    #[test]
    fn una_marca_se_vuelve_a_medir_sobre_el_trozo() {
        // La marca ocupa el cuarto izquierdo de la MITAD DERECHA de la captura, o sea de
        // 0,5 a 0,625. Dentro del recorte eso es su cuarto izquierdo: de 0 a 0,25. Sin
        // reencuadrar, ese 0,5 caeria en el centro del trozo.
        let marca = anotacion::Anotacion {
            kind: "box".into(),
            x1: 0.5,
            y1: 0.1,
            x2: 0.625,
            y2: 0.9,
            color: "#000000".into(),
            text: String::new(),
        };
        let salida = enmarcar_y_anotar(
            dos_mitades(400, 300),
            200,
            300,
            sin_marco(),
            std::slice::from_ref(&marca),
            &[mitad_derecha()],
            None,
        );
        let negro = |x: u32| {
            (0..300).any(|y| {
                let p = salida.get_pixel(x, y).0;
                p[0] < 60 && p[1] < 60 && p[2] < 60
            })
        };
        assert!(negro(2), "el borde izquierdo del rectangulo no esta donde tocaba");
        assert!(!negro(150), "el rectangulo ha aparecido en el centro, sin reencuadrar");
    }

    #[test]
    fn el_marco_se_pone_despues_del_recorte_y_crece_por_fuera() {
        let salida = enmarcar_y_anotar(
            dos_mitades(400, 300),
            200,
            300,
            marco::Marco {
                margen: 20,
                fondo: marco::Fondo::desde("blanco"),
                sombra: false,
                esquinas: 0,
            },
            &[],
            &[mitad_derecha()],
            None,
        );
        assert_eq!(salida.dimensions(), (240, 340));
        assert_eq!(*salida.get_pixel(2, 2), image::Rgba([255, 255, 255, 255]));
        assert_eq!(*salida.get_pixel(120, 170), AZUL);
    }

    #[test]
    fn el_recorte_del_usuario_y_el_del_zoom_se_encadenan() {
        // La camara se mueve DENTRO de lo que el usuario dejo, no sobre la captura entera.
        // Recortar la mitad derecha (azul) y despues acercarse a su mitad izquierda tiene
        // que seguir dando azul, y del tamanno que se pidio.
        let zoom_izquierda = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 0.5,
            y2: 1.0,
        };
        let salida = enmarcar_y_anotar(
            dos_mitades(400, 300),
            100,
            300,
            sin_marco(),
            &[],
            &[mitad_derecha(), zoom_izquierda],
            None,
        );
        assert_eq!(salida.dimensions(), (100, 300));
        assert_eq!(*salida.get_pixel(50, 150), AZUL);
    }

    #[test]
    fn un_recorte_que_no_recorta_nada_no_descoloca_las_marcas() {
        // La camara devuelve la imagen entera mientras no hay zoom, y eso pasa por aqui en
        // cada fotograma. Si contara como recorte, volveria a medir las marcas sin motivo.
        let marca = anotacion::Anotacion {
            kind: "box".into(),
            x1: 0.5,
            y1: 0.1,
            x2: 0.6,
            y2: 0.9,
            color: "#000000".into(),
            text: String::new(),
        };
        let entero = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let con = enmarcar_y_anotar(
            dos_mitades(400, 300),
            400,
            300,
            sin_marco(),
            std::slice::from_ref(&marca),
            &[entero],
            None,
        );
        let sin = enmarcar_y_anotar(
            dos_mitades(400, 300),
            400,
            300,
            sin_marco(),
            std::slice::from_ref(&marca),
            &[],
            None,
        );
        assert_eq!(con.as_raw(), sin.as_raw());
    }
}

/// La cámara, montada como la monta la exportación de verdad.
///
/// `zoom.rs` ya prueba la aritmética. Aquí se prueba lo que solo pasa al pegar las piezas:
/// que los clics, que están en píxeles de la región grabada, se trasladen al trozo que el
/// usuario recortó. Es donde este proyecto se ha equivocado antes.
#[cfg(test)]
mod la_camara_del_exportador {
    use super::*;
    use crate::capture::Rect;

    fn sesion_con_clics(clics: Vec<zoom::Clic>) -> SessionData {
        SessionData {
            id: "z".into(),
            dir: PathBuf::from("."),
            region: Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 300,
            },
            fps: 30,
            format: "video".into(),
            has_audio: false,
            width: 400,
            height: 300,
            mp4_path: None,
            audio: None,
            clics,
            teclas: Vec::new(),
            cursor: Vec::new(),
            cursor_capturado: false,
            formas: Vec::new(),
            punteros: Vec::new(),
            cambios_puntero: Vec::new(),
            marcas_zoom: Vec::new(),
            camara: None,
            escritura: Vec::new(),
            frames: Vec::new(),
        }
    }

    fn peticion(zoom: f32, crop: Option<Recorte>) -> ExportRequest {
        ExportRequest {
            session_id: "z".into(),
            format: "mp4".into(),
            engine: "native".into(),
            from: 0,
            to: 0,
            desde_ms: None,
            hasta_ms: None,
            width: 400,
            height: 300,
            fps: 30,
            quality: 80,
            audio: false,
            loop_forever: false,
            margin: 0,
            background: String::new(),
            shadow: false,
            annotations: Vec::new(),
            crop,
            file: None,
            zoom,
            clicks: false,
            keys: false,
            cursor: 0.0,
            speed: 1.0,
            smooth_cursor: false,
            hide_idle_cursor: false,
            corners: 0,
            volume: 1.0,
            zoom_speed: 1.0,
            edicion: Default::default(),
            destination: None,
            copy_to_clipboard: false,
        }
    }

    /// Una velocidad que no es un numero, o que es cero, no puede dejar la exportacion
    /// dividiendo por cero ni el video congelado: se queda en la de siempre.
    #[test]
    fn una_velocidad_imposible_se_queda_en_la_normal() {
        let mut pedido = peticion(1.0, None);
        for disparate in [0.0, -3.0, f32::NAN, f32::INFINITY] {
            pedido.speed = disparate;
            assert_eq!(pedido.velocidad(), 1.0, "con {disparate}");
        }
        pedido.speed = 99.0;
        assert_eq!(pedido.velocidad(), 4.0, "por arriba se corta en 4x");
    }

    #[test]
    fn sin_zoom_pedido_no_hay_camara() {
        let sesion = sesion_con_clics(vec![zoom::Clic { ms: 1000, x: 100, y: 100, derecho: false }]);
        assert!(Camara::preparar(&sesion, &peticion(1.0, None)).en(1000).is_none());
    }

    #[test]
    fn sin_clics_tampoco() {
        // Una grabacion en la que nadie pulso nada no tiene a donde acercarse.
        let sesion = sesion_con_clics(vec![]);
        assert!(Camara::preparar(&sesion, &peticion(2.0, None)).en(1000).is_none());
    }

    #[test]
    fn con_zoom_y_un_clic_la_camara_recorta_en_el_momento_del_clic() {
        let sesion = sesion_con_clics(vec![zoom::Clic { ms: 1000, x: 100, y: 100, derecho: false }]);
        let camara = Camara::preparar(&sesion, &peticion(2.0, None));
        let r = camara.en(1000).expect("tendria que estar acercada");
        let (x, y, w, h) = r.en_pixeles(400, 300);
        assert_eq!((w, h), (200, 150));
        // Centrada en el clic, no en el medio de la pantalla.
        assert_eq!((x + w / 2, y + h / 2), (100, 100));
    }

    #[test]
    fn y_lejos_del_clic_vuelve_a_la_imagen_entera() {
        let sesion = sesion_con_clics(vec![zoom::Clic { ms: 1000, x: 100, y: 100, derecho: false }]);
        let camara = Camara::preparar(&sesion, &peticion(2.0, None));
        assert!(camara.en(20_000).is_none());
    }

    #[test]
    fn con_un_recorte_del_usuario_el_clic_se_traslada_a_ese_trozo() {
        // El clic esta en (300, 150) de la captura. Recortando la mitad derecha, ese punto
        // es el (100, 150) del trozo. Sin trasladarlo, la camara se iria a otro sitio.
        let sesion = sesion_con_clics(vec![zoom::Clic { ms: 1000, x: 300, y: 150, derecho: false }]);
        let mitad_derecha = Recorte {
            x1: 0.5,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let camara = Camara::preparar(&sesion, &peticion(2.0, Some(mitad_derecha)));
        let r = camara.en(1000).expect("tendria que estar acercada");
        // El recorte de la camara se mide sobre el trozo, que son 200 x 300.
        let (x, y, w, h) = r.en_pixeles(200, 300);
        assert_eq!((w, h), (100, 150));
        assert_eq!((x + w / 2, y + h / 2), (100, 150));
    }

    #[test]
    fn un_clic_que_se_quedo_fuera_del_recorte_no_mueve_la_camara() {
        // Acercarse a un clic que ya no se ve seria acercarse a nada.
        let sesion = sesion_con_clics(vec![zoom::Clic { ms: 1000, x: 20, y: 20, derecho: false }]);
        let mitad_derecha = Recorte {
            x1: 0.5,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let camara = Camara::preparar(&sesion, &peticion(2.0, Some(mitad_derecha)));
        assert!(camara.en(1000).is_none());
    }

    #[test]
    fn ni_uno_que_se_quedo_por_el_otro_lado() {
        // El de arriba cae a la IZQUIERDA del trozo y lo descarta el «mayor que cero».
        // Este cae a la derecha, y hace falta la otra mitad de la comprobacion: sin ella,
        // la camara se iria a un punto que no existe dentro del recorte.
        let sesion = sesion_con_clics(vec![zoom::Clic { ms: 1000, x: 350, y: 150, derecho: false }]);
        let mitad_izquierda = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 0.5,
            y2: 1.0,
        };
        let camara = Camara::preparar(&sesion, &peticion(2.0, Some(mitad_izquierda)));
        assert!(camara.en(1000).is_none());
    }

    fn sesion_con_teclas(teclas: Vec<crate::record::teclas::Atajo>) -> SessionData {
        let mut s = sesion_con_clics(vec![]);
        s.teclas = teclas;
        s
    }

    fn atajo(ms: u64, x: i32, y: i32) -> crate::record::teclas::Atajo {
        crate::record::teclas::Atajo {
            texto: "Ctrl + C".into(),
            ms,
            x,
            y,
        }
    }

    #[test]
    fn la_camara_tambien_se_acerca_a_los_atajos() {
        // Un atajo no tiene sitio propio, pero quien lo pulsa esta mirando a algun lado, y
        // ese lado es donde tiene el raton. Sin esto, media grabacion de un tutorial (la
        // que se hace a teclazos) no se acercaria a nada.
        let sesion = sesion_con_teclas(vec![atajo(1000, 100, 100)]);
        let camara = Camara::preparar(&sesion, &peticion(2.0, None));
        let r = camara.en(1000).expect("tendria que estar acercada al atajo");
        let (x, y, w, h) = r.en_pixeles(400, 300);
        assert_eq!((x + w / 2, y + h / 2), (100, 100));
    }

    #[test]
    fn los_clics_y_los_atajos_se_ordenan_juntos_en_el_tiempo() {
        // Vienen en dos listas y se agrupan en tramos, y agrupar exige orden. Sin ordenar,
        // un atajo anterior a un clic abriria un tramo hacia atras y la camara saltaria.
        let mut sesion = sesion_con_clics(vec![zoom::Clic {
            ms: 5000,
            x: 300,
            y: 200,
            derecho: false,
        }]);
        sesion.teclas = vec![atajo(1000, 100, 100)];
        let camara = Camara::preparar(&sesion, &peticion(2.0, None));
        let primero = camara.en(1000).unwrap().en_pixeles(400, 300);
        let segundo = camara.en(5000).unwrap().en_pixeles(400, 300);
        assert_eq!((primero.0 + primero.2 / 2, primero.1 + primero.3 / 2), (100, 100));
        assert_eq!((segundo.0 + segundo.2 / 2, segundo.1 + segundo.3 / 2), (300, 200));
    }

    #[test]
    fn un_zoom_disparatado_se_queda_en_cuatro() {
        // El numero llega del frontend. A 50x, un fotograma seria un pixel estirado.
        let sesion = sesion_con_clics(vec![zoom::Clic { ms: 1000, x: 200, y: 150, derecho: false }]);
        let camara = Camara::preparar(&sesion, &peticion(50.0, None));
        let (_, _, w, _) = camara.en(1000).unwrap().en_pixeles(400, 300);
        assert_eq!(w, 100);
    }
}

/// Cuanto cuesta cada paso de exportar, medido sobre una grabacion de verdad.
///
/// Munir, el 29 de agosto de 2026: *«tarda mucho en procesar el video»*, con una grabacion
/// de 1890x1052, 50 segundos y 1320 fotogramas. Antes de tocar nada hay que saber DONDE se
/// va el tiempo, que es la leccion de `docs/TRAMPAS.md`: el filtro nunca es el filtro.
///
/// Se corre a mano, en release y apuntando a una carpeta de sesion:
///
/// ```text
/// WINSHOTX_SESION=C:\...\Temp\winshotx\sessions\55483c6bef \
///   cargo test --release --lib medir::reparto_del_tiempo -- --ignored --nocapture
/// ```
#[cfg(all(test, windows))]
mod medir {
    use super::*;
    use std::time::Instant;

    /// Exportar a MP4 leyendo por delante (en otro hilo) frente a leer en el mismo hilo,
    /// sobre una sesion de 1080p hecha aqui mismo, vistiendo cada fotograma como lo hace
    /// `encode_mp4`. `cargo test --release --lib medir_exportar_por_delante -- --ignored --nocapture`
    #[test]
    #[ignore = "es una medicion, no una comprobacion"]
    fn medir_exportar_por_delante() {
        let (ancho, alto, cuantos) = (1920u32, 1080u32, 90usize);
        let dir = std::env::temp_dir().join("winshotx-bench-exportar");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cache = record::FrameCache::new(&dir).unwrap();
        // Ruido de fondo y un bloque que se mueve: parches pequennos, pero cada fotograma
        // hay que descomprimirlo y pegarlo igual.
        let mut fondo = vec![0u8; (ancho * alto) as usize * 4];
        let mut s: u32 = 99;
        for p in fondo.chunks_exact_mut(4) {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            p.copy_from_slice(&[(s >> 24) as u8, (s >> 16) as u8, (s >> 8) as u8, 255]);
        }
        for i in 0..cuantos as u32 {
            let mut f = fondo.clone();
            for y in 400..464u32 {
                for x in (100 + i * 12)..(164 + i * 12) {
                    let p = ((y * ancho + x) * 4) as usize;
                    f[p..p + 3].copy_from_slice(&[255, 255, 255]);
                }
            }
            cache.push_rgba(&f, ancho, alto, u64::from(i) * 33).unwrap();
        }
        let frames = cache.finish(cuantos as u64 * 33, 30).unwrap();
        let session = SessionData {
            id: "bench".into(),
            dir: dir.clone(),
            region: crate::capture::Rect { x: 0, y: 0, width: ancho, height: alto },
            fps: 30,
            format: "mp4".into(),
            has_audio: false,
            width: ancho,
            height: alto,
            mp4_path: None,
            audio: None,
            clics: Vec::new(),
            teclas: Vec::new(),
            cursor: Vec::new(),
            cursor_capturado: false,
            formas: Vec::new(),
            punteros: Vec::new(),
            cambios_puntero: Vec::new(),
            marcas_zoom: Vec::new(),
            camara: None,
            escritura: Vec::new(),
            frames,
        };
        let indices: Vec<usize> = (0..cuantos).collect();
        let delays = vec![33u32; cuantos];
        let marco = marco::Marco { margen: 0, fondo: marco::Fondo::desde("blanco"), sombra: false, esquinas: 0 };
        let opciones = mp4::Mp4Options { width: ancho, height: alto, fps: 30, quality: 75 };

        let vestir = |imagen: image::RgbaImage| enmarcar_y_anotar(imagen, ancho, alto, marco, &[], &[], None);

        let t = Instant::now();
        let mut en_orden = record::LectorEnOrden::nuevo(&session).unwrap();
        let mut loader = |i: usize| en_orden.en(i).map(vestir);
        mp4::encode(&indices, &delays, &mut loader, &dir.join("mismo-hilo.mp4"), &opciones, None, |_, _, _| {}).unwrap();
        let mismo_hilo = t.elapsed().as_secs_f64() * 1000.0 / cuantos as f64;

        let t = Instant::now();
        let mut por_delante = record::LectorPorDelante::nuevo(session.clone(), indices.clone());
        let mut loader = |i: usize| por_delante.en(i).map(vestir);
        mp4::encode(&indices, &delays, &mut loader, &dir.join("por-delante.mp4"), &opciones, None, |_, _, _| {}).unwrap();
        let otro_hilo = t.elapsed().as_secs_f64() * 1000.0 / cuantos as f64;

        println!("[exportar 1080p] mismo hilo {mismo_hilo:.1} ms/fotograma · leyendo por delante {otro_hilo:.1} ms/fotograma");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn sesion() -> Option<SessionData> {
        let dir = std::env::var("WINSHOTX_SESION").ok()?;
        let texto = std::fs::read_to_string(std::path::Path::new(&dir).join("session.json")).ok()?;
        serde_json::from_str(&texto).ok()
    }

    /// Reducir con lo de casa frente a Lanczos3, para poder mirar las dos y decidir.
    ///
    /// `WINSHOTX_IMAGEN=<png> cargo test --release --lib medir::calidad -- --ignored`
    #[test]
    #[ignore]
    fn calidad_de_la_reduccion() {
        let Ok(ruta) = std::env::var("WINSHOTX_IMAGEN") else {
            eprintln!("sin WINSHOTX_IMAGEN no hay nada que comparar");
            return;
        };
        let ruta = std::path::PathBuf::from(ruta);
        let origen = image::open(&ruta).expect("imagen").to_rgba8();
        let (ancho, alto) = (1280u32, 712u32);

        let lanczos =
            image::imageops::resize(&origen, ancho, alto, image::imageops::FilterType::Lanczos3);
        let casero = escalar::reducir(&origen, ancho, alto);

        let padre = ruta.parent().expect("carpeta");
        lanczos.save(padre.join("reduce-lanczos.png")).expect("guardar");
        casero.save(padre.join("reduce-casero.png")).expect("guardar");

        // Cuanto se separan, en promedio y en el peor pixel.
        let (mut suma, mut peor) = (0u64, 0u32);
        for (a, b) in lanczos.as_raw().iter().zip(casero.as_raw()) {
            let d = (*a as i32 - *b as i32).unsigned_abs();
            suma += u64::from(d);
            peor = peor.max(d);
        }
        eprintln!(
            "diferencia media {:.2} de 255, peor pixel {peor}",
            suma as f64 / lanczos.as_raw().len() as f64
        );
    }

    /// Un video de verdad, escrito con el escalado nuevo, para poder abrirlo y mirarlo.
    ///
    /// Contar milisegundos y comparar pixeles no ve un video roto: los dos extremos pueden
    /// salir verdes y el archivo que se lleva el usuario estar mal. Este deja el mp4 en
    /// `%TEMP%\winshotx-mirar.mp4` para abrirlo.
    ///
    /// `WINSHOTX_IMAGEN=<png> cargo test --release --lib medir::un_video -- --ignored`
    #[test]
    #[ignore]
    fn un_video_de_verdad_para_mirarlo() {
        let Ok(ruta) = std::env::var("WINSHOTX_IMAGEN") else {
            eprintln!("sin WINSHOTX_IMAGEN no hay nada que codificar");
            return;
        };
        let origen = image::open(&ruta).expect("imagen").to_rgba8();
        let (ancho, alto) = (1280u32, 712u32);
        let cuantos = 60usize;

        // La misma imagen desplazandose, para que el video tenga movimiento de verdad y no
        // sea un fotograma repetido que el codificador se come sin comprimir nada.
        let indices: Vec<usize> = (0..cuantos).collect();
        let delays: Vec<u32> = indices.iter().map(|_| 33).collect();
        let mut loader = |i: usize| -> Result<image::RgbaImage> {
            let d = (i as i64 % 20) - 10;
            let mut movida = image::RgbaImage::new(origen.width(), origen.height());
            image::imageops::replace(&mut movida, &origen, d, d / 2);
            Ok(movida)
        };

        let destino = std::env::temp_dir().join("winshotx-mirar.mp4");
        mp4::encode(
            &indices,
            &delays,
            &mut loader,
            &destino,
            &mp4::Mp4Options {
                width: ancho,
                height: alto,
                fps: 30,
                quality: 80,
            },
            None,
            |_, _, _| {},
        )
        .expect("codificar");
        let bytes = std::fs::metadata(&destino).map(|m| m.len()).unwrap_or(0);
        eprintln!("{} · {} KB", destino.display(), bytes / 1024);
        assert!(bytes > 10_000, "el video ha salido vacio: {bytes} bytes");
    }

    #[test]
    #[ignore]
    fn reparto_del_tiempo() {
        let Some(session) = sesion() else {
            eprintln!("sin WINSHOTX_SESION no hay nada que medir");
            return;
        };
        let cuantos: usize = std::env::var("WINSHOTX_N")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(120);
        let indices: Vec<usize> = (0..session.frames.len().min(cuantos)).collect();
        eprintln!(
            "sesion {} · {}x{} · {} fotogramas · midiendo {}",
            session.id,
            session.width,
            session.height,
            session.frames.len(),
            indices.len()
        );

        // 1. Sacar el fotograma del cache, tal y como lo hace hoy el exportador.
        let t = Instant::now();
        let mut ultimo = None;
        for i in &indices {
            ultimo = Some(record::read_frame(&session, *i).expect("fotograma"));
        }
        let leer = t.elapsed();
        let imagen = ultimo.expect("al menos uno");
        eprintln!(
            "leer del cache      {:>7.1} ms/fotograma  ({:.1} s los {})",
            leer.as_secs_f64() * 1000.0 / indices.len() as f64,
            leer.as_secs_f64(),
            indices.len()
        );

        // 2. Escalar a 1280 de ancho, que es lo que hace el codificador cuando el panel
        //    pide otra medida.
        let ancho = 1280u32;
        let alto = (session.height * ancho / session.width.max(1)) / 2 * 2;
        let t = Instant::now();
        for _ in 0..10 {
            let _ = image::imageops::resize(
                &imagen,
                ancho,
                alto,
                image::imageops::FilterType::Lanczos3,
            );
        }
        eprintln!(
            "  image/Lanczos3    {:>7.1} ms/fotograma",
            t.elapsed().as_secs_f64() * 1000.0 / 10.0
        );

        let t = Instant::now();
        for _ in 0..10 {
            let _ = escalar::reducir(&imagen, ancho, alto);
        }
        eprintln!(
            "  escalar::reducir  {:>7.1} ms/fotograma",
            t.elapsed().as_secs_f64() * 1000.0 / 10.0
        );

        // Y el codificador solo, sin escalar nada: es lo que pasa al exportar al tamanno
        // nativo, que es como viene el panel.
        let nativo = (session.width / 2 * 2, session.height / 2 * 2);
        let destino = std::env::temp_dir().join("winshotx-medir-nativo.mp4");
        let delays: Vec<u32> = indices.iter().map(|_| 33).collect();
        let t = Instant::now();
        let mut loader = |index: usize| record::read_frame(&session, index);
        mp4::encode(
            &indices,
            &delays,
            &mut loader,
            &destino,
            &mp4::Mp4Options {
                width: nativo.0,
                height: nativo.1,
                fps: 30,
                quality: 80,
            },
            None,
            |_, _, _| {},
        )
        .expect("codificar");
        eprintln!(
            "exportar a lo nativo{:>7.1} ms/fotograma  ({:.1} s los {})",
            t.elapsed().as_secs_f64() * 1000.0 / indices.len() as f64,
            t.elapsed().as_secs_f64(),
            indices.len()
        );
        let _ = std::fs::remove_file(&destino);

        // 3. La exportacion entera, con el codificador de Windows por medio.
        let destino = std::env::temp_dir().join("winshotx-medir.mp4");
        let delays: Vec<u32> = indices.iter().map(|_| 33).collect();
        let t = Instant::now();
        let mut loader = |index: usize| record::read_frame(&session, index);
        mp4::encode(
            &indices,
            &delays,
            &mut loader,
            &destino,
            &mp4::Mp4Options {
                width: ancho,
                height: alto,
                fps: 30,
                quality: 80,
            },
            None,
            |_, _, _| {},
        )
        .expect("codificar");
        let entero = t.elapsed();
        eprintln!(
            "exportar entero     {:>7.1} ms/fotograma  ({:.1} s los {}) -> {:.1} s los {} de Munir",
            entero.as_secs_f64() * 1000.0 / indices.len() as f64,
            entero.as_secs_f64(),
            indices.len(),
            entero.as_secs_f64() / indices.len() as f64 * session.frames.len() as f64,
            session.frames.len()
        );
        let _ = std::fs::remove_file(&destino);
    }
}

/// El recorte de la linea de tiempo, de punta a punta y con las piezas de verdad: una
/// grabacion escrita en disco con la cache de siempre, un tramo cortado al milisegundo
/// (dentro de una pantalla quieta, que es el caso que por fotogramas no se podia), el
/// codificador de Windows, y el MP4 abierto despues con ffprobe para mirar lo que se lleva
/// quien lo guarda: cuanto dura la imagen, cuanto dura el sonido y de que color son el
/// primer y el ultimo fotograma.
///
///     cargo test --release --lib recortar_de_punta_a_punta -- --ignored --nocapture
///
/// Ignorada porque necesita el codificador por hardware de Windows y ffmpeg en el PATH.
#[cfg(all(test, windows))]
mod recortar_de_punta_a_punta {
    use super::*;
    use crate::record::FrameCache;
    use std::process::Command;

    const ANCHO: u32 = 320;
    const ALTO: u32 = 240;
    const ROJO: [u8; 3] = [220, 40, 40];
    const VERDE: [u8; 3] = [40, 200, 60];
    const AZUL: [u8; 3] = [40, 80, 230];
    const AMARILLO: [u8; 3] = [230, 210, 40];

    /// Un fondo de un color con un cuadro blanco en `x`, para que haya movimiento de verdad.
    fn fotograma(color: [u8; 3], x: u32) -> Vec<u8> {
        let mut rgba = Vec::with_capacity((ANCHO * ALTO * 4) as usize);
        for fila in 0..ALTO {
            for col in 0..ANCHO {
                let blanco = (x..x + 24).contains(&col) && (100..124).contains(&fila);
                let [r, g, b] = if blanco { [255, 255, 255] } else { color };
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
        }
        rgba
    }

    /// El color medio de un fotograma del MP4, sacado con ffmpeg reduciendolo a un pixel.
    fn color_medio(mp4: &Path, del_final: bool) -> Option<[u8; 3]> {
        let mut orden = Command::new("ffmpeg");
        orden.args(["-v", "error"]);
        if del_final {
            orden.args(["-sseof", "-0.3"]);
        }
        orden.arg("-i").arg(mp4).args([
            "-vf",
            "scale=1:1:flags=area",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]);
        let salida = orden.output().ok()?.stdout;
        // Del principio vale el primero; del final, el ultimo de los que salen.
        let trozo = if del_final { salida.get(salida.len().checked_sub(3)?..)? } else { salida.get(..3)? };
        Some([trozo[0], trozo[1], trozo[2]])
    }

    /// Lo que dura cada pista del MP4, segun ffprobe: `(video, sonido)` en segundos.
    fn duraciones(mp4: &Path) -> Option<(f64, Option<f64>)> {
        let salida = Command::new("ffprobe")
            .args(["-v", "error", "-show_entries", "stream=codec_type,duration", "-of", "csv=p=0"])
            .arg(mp4)
            .output()
            .ok()?;
        let texto = String::from_utf8_lossy(&salida.stdout).to_string();
        let mut video = None;
        let mut sonido = None;
        for linea in texto.lines() {
            let mut partes = linea.split(',');
            let tipo = partes.next()?;
            let segundos: f64 = partes.next()?.trim().parse().ok()?;
            match tipo {
                "video" => video = Some(segundos),
                "audio" => sonido = Some(segundos),
                _ => {}
            }
        }
        Some((video?, sonido))
    }

    /// El fotograma del MP4 en ese segundo, en RGB, sacado con ffmpeg.
    fn fotograma_del_mp4(mp4: &Path, segundo: f32) -> Option<Vec<u8>> {
        let salida = Command::new("ffmpeg")
            .args(["-v", "error", "-ss", &format!("{segundo:.2}")])
            .arg("-i")
            .arg(mp4)
            .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .ok()?;
        (salida.stdout.len() == (ANCHO * ALTO * 3) as usize).then_some(salida.stdout)
    }

    /// La columna mas a la izquierda donde hay algo que no es el fondo azul: el puntero.
    fn columna_del_puntero(rgb: &[u8]) -> Option<u32> {
        (0..ALTO)
            .flat_map(|y| (0..ANCHO).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let i = ((y * ANCHO + x) * 3) as usize;
                let distinto = (0..3).map(|c| (rgb[i + c] as i32 - AZUL[c] as i32).abs()).sum::<i32>();
                distinto > 150
            })
            .map(|(x, _)| x)
            .min()
    }

    /// Lo que cuesta exportar un tutorial: 1080p, veinte segundos de pagina quieta con el
    /// raton yendo de un lado a otro, el puntero suave, fondo, esquinas y sombra.
    ///
    ///     cargo test --release --lib medir_un_tutorial -- --ignored --nocapture
    #[test]
    #[ignore = "mide, no comprueba: pide release y el codificador de Windows"]
    fn medir_un_tutorial() {
        let (ancho, alto) = (1920u32, 1080u32);
        let dir = std::env::temp_dir().join(format!("winshotx-tutorial-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cache = FrameCache::new(&dir).expect("cache");
        let pagina: Vec<u8> = (0..ancho * alto).flat_map(|i| {
            let (x, y) = (i % ancho, i / ancho);
            let v = if (x / 120 + y / 80) % 2 == 0 { 235 } else { 215 };
            [v, v, v, 255]
        }).collect();
        cache.push_rgba(&pagina, ancho, alto, 0).unwrap();
        let mut session = super::tests::session_with(&[]);
        session.frames = cache.finish(20_000, 30).expect("cerrar la cache");
        session.dir = dir.clone();
        session.width = ancho;
        session.height = alto;
        session.region = crate::capture::Rect { x: 0, y: 0, width: ancho, height: alto };
        session.fps = 30;
        // El raton va y viene por la pantalla: la mitad del tiempo moviendose, la otra quieto.
        session.cursor = (0..1_250u64)
            .map(|k| {
                let t = k * 16;
                let fase = (t / 2_000) % 2;
                let x = if fase == 0 { 200 + ((t % 2_000) as i32 * 1_400) / 2_000 } else { 1_600 };
                (t, x, 540)
            })
            .collect();
        let mut peticion = crate::recorder::peticion_por_defecto(&session);
        peticion.format = "mp4".into();
        peticion.audio = false;
        peticion.smooth_cursor = true;
        peticion.margin = 96;
        peticion.corners = 23;
        peticion.background = "noche".into();
        peticion.shadow = true;
        let pasos = pasos_de(&session, &peticion);
        let destino = dir.join("tutorial.mp4");
        let empieza = Instant::now();
        escribir(&session, &peticion, &destino, &|_, _, _| {}).expect("exportar");
        let tardo = empieza.elapsed().as_millis();
        let pesa = std::fs::metadata(&destino).map(|m| m.len()).unwrap_or(0);
        let _ = std::fs::remove_dir_all(&dir);
        println!(
            "tutorial 1080p de 20 s: {} pasos escritos de {} posibles, {tardo} ms en total, {:.1} ms por paso, {} KB",
            pasos.indices.len(),
            20_000 / 33,
            tardo as f64 / pasos.indices.len().max(1) as f64,
            pesa / 1024
        );
    }

    /// El puntero dibujado tiene que cruzar una pantalla quieta en el MP4, no quedarse
    /// clavado: una pagina quieta con el raton yendo al boton es medio tutorial.
    #[test]
    #[ignore = "necesita el codificador de Windows y ffmpeg: se corre a mano"]
    fn el_puntero_cruza_una_pantalla_quieta_en_el_mp4() {
        if Command::new("ffprobe").arg("-version").output().is_err() {
            eprintln!("sin ffmpeg en el PATH no se puede abrir el MP4: no compruebo nada");
            return;
        }
        let dir = std::env::temp_dir().join(format!("winshotx-puntero-mp4-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cache = FrameCache::new(&dir).expect("cache");
        let azul: Vec<u8> = (0..ANCHO * ALTO).flat_map(|_| [AZUL[0], AZUL[1], AZUL[2], 255]).collect();
        cache.push_rgba(&azul, ANCHO, ALTO, 0).unwrap();
        let mut session = super::tests::session_with(&[]);
        session.frames = cache.finish(3_000, 30).expect("cerrar la cache");
        session.dir = dir.clone();
        session.width = ANCHO;
        session.height = ALTO;
        session.region = crate::capture::Rect { x: 0, y: 0, width: ANCHO, height: ALTO };
        session.fps = 30;
        session.cursor = (0..=187u64).map(|k| (k * 16, 20 + (k as i32 * 260) / 187, 110)).collect();

        let mut peticion = crate::recorder::peticion_por_defecto(&session);
        // MP4 a proposito: la sesion de prueba nace como GIF y sin esto salia un GIF.
        peticion.format = "mp4".into();
        peticion.cursor = 24.0;
        peticion.audio = false;
        let destino = dir.join("puntero.mp4");
        escribir(&session, &peticion, &destino, &|_, _, _| {}).expect("exportar");
        let al_principio = fotograma_del_mp4(&destino, 0.3).and_then(|f| columna_del_puntero(&f));
        let al_final = fotograma_del_mp4(&destino, 2.7).and_then(|f| columna_del_puntero(&f));
        let _ = std::fs::remove_dir_all(&dir);
        println!("el puntero esta en x = {al_principio:?} a los 0,3 s y en x = {al_final:?} a los 2,7 s");
        let (a, b) = (al_principio.expect("sin puntero al principio"), al_final.expect("sin puntero al final"));
        assert!(b > a + 150, "el puntero no ha cruzado la pantalla: de {a} a {b}");
    }

    #[test]
    #[ignore = "necesita el codificador de Windows y ffmpeg: se corre a mano"]
    fn recortar_de_punta_a_punta() {
        if Command::new("ffprobe").arg("-version").output().is_err() {
            eprintln!("sin ffprobe en el PATH no se puede abrir el MP4: no compruebo nada");
            return;
        }
        let dir = std::env::temp_dir().join(format!("winshotx-recorte-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Dos segundos moviendose (rojo y verde), SEIS de pantalla quieta en azul, que es
        // UN solo fotograma, y uno mas moviendose en amarillo.
        let mut cache = FrameCache::new(&dir).expect("cache");
        let mut ts = 0u64;
        while ts < 2_000 {
            let color = if ts < 1_000 { ROJO } else { VERDE };
            cache.push_rgba(&fotograma(color, (ts / 10) as u32 % 290), ANCHO, ALTO, ts).unwrap();
            ts += 33;
        }
        cache.push_rgba(&fotograma(AZUL, 150), ANCHO, ALTO, 2_000).unwrap();
        let mut ts = 8_000u64;
        while ts < 9_000 {
            cache.push_rgba(&fotograma(AMARILLO, (ts / 10) as u32 % 290), ANCHO, ALTO, ts).unwrap();
            ts += 33;
        }
        let frames = cache.finish(9_000, 30).expect("cerrar la cache");

        // Nueve segundos de sonido, para comprobar que se corta con la imagen.
        let audio = record::AudioInfo { channels: 2, sample_rate: 48_000 };
        let bytes_por_ms = audio.bytes_por_ms() as usize;
        let pcm: Vec<u8> = (0..9_000 * bytes_por_ms).map(|i| ((i / 7) % 200) as u8).collect();

        let mut session = super::tests::session_with(&[]);
        session.dir = dir.clone();
        session.width = ANCHO;
        session.height = ALTO;
        session.region = crate::capture::Rect { x: 0, y: 0, width: ANCHO, height: ALTO };
        session.fps = 30;
        session.has_audio = true;
        session.audio = Some(audio);
        session.frames = frames;
        std::fs::write(session.audio_path(), &pcm).unwrap();
        let quieto = session.frames.iter().find(|f| f.timestamp_ms == 2_000).expect("el azul");
        assert_eq!(quieto.duration_ms, 6_000, "la quietud tiene que ser UN fotograma de seis segundos");

        // Lo que manda la linea de tiempo: de dentro de la quietud a dentro del amarillo.
        let mut peticion = crate::recorder::peticion_por_defecto(&session);
        // MP4 a proposito: la sesion de prueba nace como GIF y sin esto salia un GIF.
        peticion.format = "mp4".into();
        peticion.desde_ms = Some(3_000);
        peticion.hasta_ms = Some(8_500);
        peticion.cursor = 0.0;
        peticion.audio = true;
        let destino = dir.join("recortado.mp4");
        let empieza = Instant::now();
        escribir(&session, &peticion, &destino, &|_, _, _| {}).expect("exportar");
        let tardo = empieza.elapsed();
        let indices = pasos_de(&session, &peticion).indices;

        let (video, sonido) = duraciones(&destino).expect("ffprobe no ha leido el MP4");
        let primero = color_medio(&destino, false).expect("primer fotograma");
        let ultimo = color_medio(&destino, true).expect("ultimo fotograma");
        println!(
            "recortado de 3,0 s a 8,5 s: imagen {video:.3} s, sonido {:?} s, primero {primero:?}, \
             ultimo {ultimo:?}, {} fotogramas exportados en {} ms",
            sonido,
            indices.len(),
            tardo.as_millis()
        );
        let _ = std::fs::remove_dir_all(&dir);

        assert!((video - 5.5).abs() < 0.1, "la imagen dura {video} s y se cortaron 5,5");
        let sonido = sonido.expect("el MP4 ha salido sin sonido");
        assert!((sonido - 5.5).abs() < 0.1, "el sonido dura {sonido} s y se cortaron 5,5");
        let [r, g, b] = primero;
        assert!(b > r + 60 && b > g + 60, "el primero tenia que ser el azul quieto: {primero:?}");
        let [r, g, b] = ultimo;
        assert!(r > b + 60 && g > b + 60, "el ultimo tenia que ser el amarillo: {ultimo:?}");
    }
}

/// Exportar por el camino de verdad: una sesion escrita en disco con la cache de siempre y
/// `escribir`, que es lo mismo que corre al pulsar «Guardar» menos elegir el nombre.
///
/// Las de GIF no necesitan nada de Windows (el GIF es Rust de punta a punta), asi que corren
/// siempre, y abren el archivo escrito para mirar lo que se lleva quien lo guarda.
#[cfg(test)]
mod exportar_de_verdad {
    use super::*;
    use crate::record::FrameCache;

    const ANCHO: u32 = 96;
    const ALTO: u32 = 64;
    const AZUL: [u8; 3] = [40, 80, 230];

    fn fondo(color: [u8; 3]) -> Vec<u8> {
        (0..ANCHO * ALTO).flat_map(|_| [color[0], color[1], color[2], 255]).collect()
    }

    /// Una sesion de verdad en una carpeta temporal, con estos fotogramas `(ms, color)`.
    fn sesion(nombre: &str, fotogramas: &[(u64, [u8; 3])], total_ms: u64) -> SessionData {
        let dir = std::env::temp_dir().join(format!("winshotx-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cache = FrameCache::new(&dir).expect("cache");
        for (ms, color) in fotogramas {
            cache.push_rgba(&fondo(*color), ANCHO, ALTO, *ms).unwrap();
        }
        let mut session = super::tests::session_with(&[]);
        session.frames = cache.finish(total_ms, 30).expect("cerrar la cache");
        session.dir = dir;
        session.width = ANCHO;
        session.height = ALTO;
        session.region = crate::capture::Rect { x: 0, y: 0, width: ANCHO, height: ALTO };
        session.fps = 30;
        session
    }

    fn peticion_gif(session: &SessionData) -> ExportRequest {
        let mut peticion = crate::recorder::peticion_por_defecto(session);
        peticion.format = "gif".into();
        peticion.fps = 20;
        peticion.cursor = 0.0;
        peticion.audio = false;
        peticion
    }

    /// Los fotogramas del GIF ya montados, como los ve quien lo abre: cada uno se pega
    /// encima del anterior en su sitio, porque el codificador solo guarda lo que cambia.
    fn fotogramas_del_gif(ruta: &Path) -> Vec<(u16, image::RgbaImage)> {
        let mut opciones = ::gif::DecodeOptions::new();
        opciones.set_color_output(::gif::ColorOutput::RGBA);
        let mut lector = opciones
            .read_info(std::fs::File::open(ruta).expect("abrir el GIF"))
            .expect("el GIF no se puede leer");
        let (ancho, alto) = (u32::from(lector.width()), u32::from(lector.height()));
        let mut lienzo = image::RgbaImage::new(ancho, alto);
        let mut salen = Vec::new();
        while let Some(f) = lector.read_next_frame().expect("fotograma roto") {
            for y in 0..u32::from(f.height) {
                for x in 0..u32::from(f.width) {
                    let i = ((y * u32::from(f.width) + x) * 4) as usize;
                    let p = &f.buffer[i..i + 4];
                    if p[3] > 0 {
                        lienzo.put_pixel(u32::from(f.left) + x, u32::from(f.top) + y, image::Rgba([p[0], p[1], p[2], 255]));
                    }
                }
            }
            salen.push((f.delay, lienzo.clone()));
        }
        salen
    }

    /// Dónde está lo que no es fondo: el puntero, en estas pruebas. `None` si no hay nada.
    fn donde_hay_algo(imagen: &image::RgbaImage, fondo: [u8; 3]) -> Option<(u32, u32)> {
        let distinto = |p: &image::Rgba<u8>| {
            (0..3).map(|i| (p.0[i] as i32 - fondo[i] as i32).abs()).sum::<i32>() > 120
        };
        imagen
            .enumerate_pixels()
            .filter(|(_, _, p)| distinto(p))
            .map(|(x, y, _)| (x, y))
            .min_by_key(|(x, y)| (*x, *y))
    }

    /// El GIF por el motor de casa: la paleta se saca leyendo una muestra de fotogramas y
    /// despues se escriben todos, o sea que se leen DOS veces.
    #[test]
    fn un_gif_se_escribe_entero_y_se_abre() {
        let colores = [[220, 40, 40], [40, 200, 60], AZUL, [230, 210, 40]];
        let fotogramas: Vec<(u64, [u8; 3])> =
            (0..60).map(|i| (i * 50, colores[(i as usize / 15) % 4])).collect();
        let session = sesion("gif-entero", &fotogramas, 3_000);
        let destino = session.dir.join("salida.gif");
        let resultado = escribir(&session, &peticion_gif(&session), &destino, &|_, _, _| {});
        let salen = resultado.as_ref().ok().map(|_| fotogramas_del_gif(&destino));
        let _ = std::fs::remove_dir_all(&session.dir);
        resultado.expect("exportar a GIF ha fallado");
        let salen = salen.unwrap();
        assert!(salen.len() >= 4, "el GIF ha salido con {} fotogramas", salen.len());
        let dura: u32 = salen.iter().map(|(d, _)| u32::from(*d) * 10).sum();
        assert!((2_800..=3_200).contains(&dura), "el GIF dura {dura} ms y la grabacion 3000");
    }

    /// El marco del estudio de punta a punta: fondo, aire y esquinas redondeadas en el GIF
    /// que se lleva quien lo guarda.
    #[test]
    fn el_marco_con_esquinas_redondeadas_sale_en_el_gif() {
        let rojo = [220, 30, 30];
        let session = sesion("gif-marco", &[(0, rojo), (500, rojo)], 1_000);
        let mut peticion = peticion_gif(&session);
        peticion.margin = 12;
        peticion.corners = 10;
        peticion.background = "negro".into();
        peticion.shadow = false;
        let destino = session.dir.join("marco.gif");
        let resultado = escribir(&session, &peticion, &destino, &|_, _, _| {});
        let salen = resultado.as_ref().ok().map(|_| fotogramas_del_gif(&destino));
        let _ = std::fs::remove_dir_all(&session.dir);
        resultado.expect("exportar a GIF ha fallado");
        let (_, imagen) = &salen.unwrap()[0];
        assert_eq!(imagen.dimensions(), (ANCHO + 24, ALTO + 24), "el marco crece por fuera");
        let oscuro = |p: &image::Rgba<u8>| p.0[0] < 60 && p.0[1] < 60 && p.0[2] < 60;
        assert!(oscuro(imagen.get_pixel(2, 2)), "el aire es el fondo negro");
        assert!(oscuro(imagen.get_pixel(12, 12)), "la esquina de la captura deja ver el fondo");
        let centro = imagen.get_pixel(12 + ANCHO / 2, 12 + ALTO / 2);
        assert!(centro.0[0] > 180 && centro.0[1] < 80, "el centro es la captura: {centro:?}");
    }

    #[test]
    fn el_volumen_baja_sube_y_satura_en_vez_de_dar_la_vuelta() {
        let pista = |muestras: &[i16]| mp4::Pista {
            channels: 1,
            sample_rate: 48_000,
            datos: muestras.iter().flat_map(|m| m.to_le_bytes()).collect(),
        };
        let leer = |p: &mp4::Pista| -> Vec<i16> {
            p.datos.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect()
        };
        assert_eq!(leer(&con_volumen(pista(&[1_000, -1_000, 30_000]), 0.5)), vec![500, -500, 15_000]);
        assert_eq!(leer(&con_volumen(pista(&[1_000, -1_000, 30_000]), 2.0)), vec![2_000, -2_000, 32_767]);
        assert_eq!(leer(&con_volumen(pista(&[1_000]), 1.0)), vec![1_000], "a 1 no se toca");
        assert_eq!(leer(&con_volumen(pista(&[1_000]), 0.0)), vec![0], "a 0, mudo");
    }

    /// Lo quieto sigue siendo barato: sin mover el raton, tres segundos de pantalla quieta
    /// con el puntero dibujado son UN paso, no noventa fotogramas iguales que codificar.
    #[test]
    fn quieto_y_con_el_raton_quieto_sigue_siendo_un_paso() {
        let mut session = super::tests::session_with(&[3_000]);
        session.cursor = vec![(0, 40, 30), (1_500, 40, 30)];
        let mut peticion = crate::recorder::peticion_por_defecto(&session);
        peticion.cursor = 16.0;
        let pasos = pasos_de(&session, &peticion);
        assert_eq!(pasos.indices.len(), 1, "salen {} pasos", pasos.indices.len());
        assert_eq!(pasos.retardos.iter().sum::<u32>(), 3_000);
        // Y moviendolo, salen los que hagan falta para verlo moverse.
        session.cursor = (0..=100u64).map(|k| (k * 30, 10 + k as i32, 30)).collect();
        let pasos = pasos_de(&session, &peticion);
        assert!(pasos.indices.len() > 50, "con el raton moviendose salen {} pasos", pasos.indices.len());
        assert_eq!(pasos.retardos.iter().sum::<u32>(), 3_000, "y el tiempo no cambia");
    }

    /// Una pantalla quieta es UN fotograma guardado, pero el raton se movio por encima: el
    /// puntero dibujado tiene que moverse en lo exportado, no quedarse clavado donde estaba
    /// cuando llego el fotograma. Es lo que hace un tutorial: la pagina quieta y el raton
    /// yendo al boton.
    #[test]
    fn el_puntero_se_mueve_aunque_la_pantalla_este_quieta() {
        let session = {
            let mut s = sesion("puntero-quieto", &[(0, AZUL)], 3_000);
            // De izquierda a derecha en tres segundos, una muestra cada 16 ms.
            s.cursor = (0..=187u64).map(|k| (k * 16, 6 + (k as i32 * 70) / 187, 30)).collect();
            s
        };
        let mut peticion = peticion_gif(&session);
        peticion.cursor = 16.0;
        let destino = session.dir.join("salida.gif");
        let resultado = escribir(&session, &peticion, &destino, &|_, _, _| {});
        let salen = resultado.as_ref().ok().map(|_| fotogramas_del_gif(&destino));
        let _ = std::fs::remove_dir_all(&session.dir);
        resultado.expect("exportar a GIF ha fallado");
        let salen = salen.unwrap();
        let primero = donde_hay_algo(&salen.first().unwrap().1, AZUL).expect("sin puntero al principio");
        let ultimo = donde_hay_algo(&salen.last().unwrap().1, AZUL).expect("sin puntero al final");
        assert!(
            ultimo.0 > primero.0 + 40,
            "el puntero no se ha movido: empieza en {primero:?} y acaba en {ultimo:?}, en {} fotogramas",
            salen.len()
        );
    }
}
