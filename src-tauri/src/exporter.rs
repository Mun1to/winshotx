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
    pub destination: Option<String>,
    pub copy_to_clipboard: bool,
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
fn resample(
    session: &SessionData,
    from: usize,
    to: usize,
    fps: u32,
    velocidad: f32,
) -> (Vec<usize>, Vec<u32>) {
    let from = from.min(session.frames.len().saturating_sub(1));
    let to = to.min(session.frames.len().saturating_sub(1)).max(from);
    let slice = &session.frames[from..=to];
    let step_ms = (1000.0 / fps.max(1) as f32).round() as u64;
    // Lo que se avanza por la grabacion en cada paso. A 1x es el mismo paso de siempre.
    let paso_origen = ((step_ms as f32 * velocidad).round() as u64).max(1);
    let start_ms = slice.first().map(|f| f.timestamp_ms).unwrap_or(0);
    let end_ms = slice
        .last()
        .map(|f| f.timestamp_ms + f.duration_ms as u64)
        .unwrap_or(start_ms);

    let mut indices = Vec::new();
    let mut delays = Vec::new();
    let mut cursor = 0usize;
    let mut t = start_ms;
    while t < end_ms {
        while cursor + 1 < slice.len() && slice[cursor + 1].timestamp_ms <= t {
            cursor += 1;
        }
        // El ultimo paso se recorta: el clip exportado dura lo que dura el recorte, y a
        // velocidad distinta de 1 lo que sobra tambien se mide en el tiempo del resultado.
        let queda = end_ms - t;
        let slot = if queda >= paso_origen {
            step_ms
        } else {
            ((queda as f32 / velocidad).round() as u64).max(1)
        } as u32;
        let picked = from + cursor;
        if indices.last() != Some(&picked) {
            indices.push(picked);
            delays.push(slot);
        } else if let Some(last) = delays.last_mut() {
            // El mismo fotograma se queda en pantalla mas tiempo en vez de repetirse.
            *last += slot;
        }
        t += paso_origen;
    }

    if indices.is_empty() {
        indices.push(from);
        delays.push(step_ms.max(20) as u32);
    }
    (indices, delays)
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
    let width = request.width.max(1);
    let height = request.height.max(1);

    // El marco crece por fuera de lo que se pidio en «Dimensiones»: quien elige 800 de
    // ancho y 40 de aire quiere la captura a 800, no a 720 con bordes. Asi que el tamanno
    // que se le pide al codificador es el de despues de enmarcar.
    let marco = marco::Marco {
        margen: request.margin.min(400),
        fondo: marco::Fondo::desde(&request.background),
        sombra: request.shadow,
    };
    let (ancho_final, alto_final) = marco.medida(width, height);

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

    // El zoom, si se ha pedido. Los tramos se calculan una vez para toda la exportacion,
    // y despues cada fotograma solo pregunta donde mira la camara en su milisegundo.
    //
    // Los clics estan en pixeles de la region grabada. Si el usuario recorto, hay que
    // trasladarlos a ese trozo, que es sobre el que se va a acercar la camara.
    let camara = Camara::preparar(&session, &request);

    /// Los recortes que le tocan a un fotograma: el del usuario primero y el de la camara
    /// despues, porque la camara se mueve DENTRO de lo que el usuario dejo.
    macro_rules! recortes_de {
        ($ms:expr) => {{
            let mut v: Vec<Recorte> = Vec::new();
            if let Some(r) = request.crop {
                v.push(r);
            }
            if let Some(r) = camara.en($ms) {
                v.push(r);
            }
            v
        }};
    }

    let path = match request.format.as_str() {
        "png" => {
            let path = destination_path(app, &request, "png")?;
            emit("reading", 0, 1);
            let image = record::read_frame(&session, request.from)?;
            // Una foto no tiene zoom: la camara solo tiene sentido con el tiempo pasando.
            let recortes: Vec<Recorte> = request.crop.into_iter().collect();
            let image = enmarcar_y_anotar(image, width, height, marco, &request.annotations, &recortes, None);
            png::save(&image, &path, ancho_final, alto_final)?;
            path
        }
        // El mismo fotograma, pero pesando cinco o diez veces menos. Es lo que hace falta
        // para mandar una captura por correo o por un chat que la recomprime igual.
        "jpg" => {
            let path = destination_path(app, &request, "jpg")?;
            emit("reading", 0, 1);
            let image = record::read_frame(&session, request.from)?;
            let recortes: Vec<Recorte> = request.crop.into_iter().collect();
            let image = enmarcar_y_anotar(image, width, height, marco, &request.annotations, &recortes, None);
            jpg::save(&image, &path, ancho_final, alto_final, request.quality)?;
            path
        }
        "gif" => {
            let (indices, delays) = resample(
                &session,
                request.from,
                request.to,
                request.fps,
                request.velocidad(),
            );
            let path = destination_path(app, &request, "gif")?;
            if request.engine == "ffmpeg" && ffmpeg::available() {
                let temporary = session.dir.join("export-source.mp4");
                encode_mp4(&session, &indices, &delays, &temporary, &request, &emit)?;
                emit("encoding", 0, 1);
                ffmpeg::gif_from_video(&temporary, &path, request.fps, ancho_final, request.quality)?;
                let _ = std::fs::remove_file(&temporary);
            } else {
                let mut vestir = estudio_de(&session, &request);
                let mut loader = |index: usize| {
                    let ms = session.frames.get(index).map(|f| f.timestamp_ms).unwrap_or(0);
                    let recortes = recortes_de!(ms);
                    if let Some(e) = vestir.as_mut() {
                        e.ms = ms;
                    }
                    record::read_frame(&session, index).map(|imagen| {
                        enmarcar_y_anotar(
                            imagen,
                            width,
                            height,
                            marco,
                            &request.annotations,
                            &recortes,
                            vestir.as_mut(),
                        )
                    })
                };
                gif::encode(
                    &indices,
                    &delays,
                    &mut loader,
                    &path,
                    &gif::GifOptions {
                        width: ancho_final,
                        height: alto_final,
                        quality: request.quality,
                        loop_forever: request.loop_forever,
                    },
                    |stage, done, total| emit(stage, done, total),
                )?;
            }
            path
        }
        "mp4" => {
            let (indices, delays) = resample(
                &session,
                request.from,
                request.to,
                request.fps,
                request.velocidad(),
            );
            let path = destination_path(app, &request, "mp4")?;
            if request.engine == "ffmpeg" && ffmpeg::available() {
                let temporary = session.dir.join("export-source.mp4");
                encode_mp4(&session, &indices, &delays, &temporary, &request, &emit)?;
                emit("encoding", 0, 1);
                ffmpeg::mp4_from_video(
                    &temporary,
                    &path,
                    request.fps,
                    ancho_final,
                    alto_final,
                    request.quality,
                )?;
                let _ = std::fs::remove_file(&temporary);
            } else {
                encode_mp4(&session, &indices, &delays, &path, &request, &emit)?;
            }
            path
        }
        other => return Err(AppError::Msg(format!("formato desconocido: {other}"))),
    };

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
    origen: (u32, u32),
    ajustes: estudio::Ajustes,
    ms: u64,
    pastillas: crate::record::pastilla::Cache,
}

impl Estudio<'_> {
    fn pintar(&mut self, imagen: &mut image::RgbaImage, recortes: &[Recorte]) {
        estudio::pintar(
            imagen,
            self.ms,
            self.clics,
            self.teclas,
            self.rastro,
            self.origen,
            recortes,
            &self.ajustes,
            &mut self.pastillas,
        );
    }
}

fn estudio_de<'a>(session: &'a SessionData, request: &ExportRequest) -> Option<Estudio<'a>> {
    let ajustes = estudio::Ajustes {
        clics: request.clicks,
        teclas: request.keys,
        cursor: request.cursor,
    };
    if !ajustes.hay_algo() {
        return None;
    }
    Some(Estudio {
        clics: &session.clics,
        teclas: &session.teclas,
        rastro: &session.cursor,
        origen: (session.width.max(1), session.height.max(1)),
        ajustes,
        ms: 0,
        pastillas: crate::record::pastilla::Cache::default(),
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
    /// El tamanno sobre el que se mide, que es el de la imagen YA recortada por el usuario.
    ancho: u32,
    alto: u32,
}

impl Camara {
    fn preparar(session: &SessionData, request: &ExportRequest) -> Option<Self> {
        // Menos de 1,05 no se ve y solo cuesta trabajo: es apagado, dicho con un numero.
        if request.zoom < 1.05 || (session.clics.is_empty() && session.teclas.is_empty()) {
            return None;
        }
        let (ancho, alto) = (session.width.max(1), session.height.max(1));
        // Los clics estan en pixeles de la region grabada. Si el usuario recorto, la camara
        // se mueve dentro de ESE trozo, asi que hay que trasladarlos y descartar los que
        // se quedaron fuera: acercarse a un clic que ya no se ve seria acercarse a nada.
        let (dx, dy, ancho, alto) = match request.crop {
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
        let ajustes = zoom::Ajustes {
            escala: request.zoom.min(4.0),
            ..zoom::Ajustes::default()
        };
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
        })
    }
}

/// Un `Option<Camara>` sabe contestar igual que una: sin zoom, no recorta nada.
trait EnElInstante {
    fn en(&self, ms: u64) -> Option<Recorte>;
}

impl EnElInstante for Option<Camara> {
    fn en(&self, ms: u64) -> Option<Recorte> {
        let c = self.as_ref()?;
        let mirando = zoom::siguiendo(&c.tramos, &c.rastro, ms, c.ancho, c.alto, &c.ajustes);
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
    indices: &[usize],
    delays: &[u32],
    path: &Path,
    request: &ExportRequest,
    emit: &F,
) -> Result<()>
where
    F: Fn(&str, usize, usize),
{
    let ancho = request.width.max(1);
    let alto = request.height.max(1);
    let marco = marco::Marco {
        margen: request.margin.min(400),
        fondo: marco::Fondo::desde(&request.background),
        sombra: request.shadow,
    };
    let (ancho_final, alto_final) = marco.medida(ancho, alto);
    let camara = Camara::preparar(session, request);
    let mut vestir = estudio_de(session, request);
    let mut loader = |index: usize| {
        let ms = session.frames.get(index).map(|f| f.timestamp_ms).unwrap_or(0);
        let mut recortes: Vec<Recorte> = request.crop.into_iter().collect();
        if let Some(r) = camara.en(ms) {
            recortes.push(r);
        }
        if let Some(e) = vestir.as_mut() {
            e.ms = ms;
        }
        record::read_frame(session, index).map(|imagen| {
            enmarcar_y_anotar(
                imagen,
                ancho,
                alto,
                marco,
                &request.annotations,
                &recortes,
                vestir.as_mut(),
            )
        })
    };
    mp4::encode(
        indices,
        delays,
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
            .then(|| pista_de_audio(session, indices))
            .flatten(),
        |stage, done, total| emit(stage, done, total),
    )
}

/// El trozo de sonido que le toca al recorte que se esta exportando.
///
/// El usuario recorta por fotogramas, asi que el tramo va del primero al ultimo de los
/// que se exportan. Sin esto el video que guarda sale mudo, aunque la grabacion tuviera
/// sonido: exportar vuelve a codificar desde los fotogramas, y ahi no hay audio ninguno.
fn pista_de_audio(session: &SessionData, indices: &[usize]) -> Option<mp4::Pista> {
    use std::io::{Seek, SeekFrom};

    let info = session.audio?;
    let primero = session.frames.get(*indices.first()?)?;
    let ultimo = session.frames.get(*indices.last()?)?;
    let desde = primero.timestamp_ms;
    let hasta = ultimo.timestamp_ms + u64::from(ultimo.duration_ms);
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
    use crate::capture::Rect;
    use crate::record::FrameEntry;

    fn session_with(durations: &[u32]) -> SessionData {
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
            frames,
        }
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
            let mut loader = |index: usize| {
                Ok(image::RgbaImage::from_raw(ancho, alto, lienzos[index].clone()).unwrap())
            };
            crate::encode::gif::encode(
                &indices,
                &delays,
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
            zoom: 0.0,
            clicks: false,
            keys: false,
            cursor: 0.0,
            speed: 1.0,
            destination: None,
            copy_to_clipboard: true,
        }
    }

    fn sin_marco() -> marco::Marco {
        marco::Marco {
            margen: 0,
            fondo: marco::Fondo::desde("blanco"),
            sombra: false,
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
            zoom,
            clicks: false,
            keys: false,
            cursor: 0.0,
            speed: 1.0,
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
            let d = (i as i64 % 20) as i64 - 10;
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
