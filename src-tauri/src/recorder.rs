use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::capture::Rect;
use crate::error::{AppError, Result};
use crate::record::{self, FrameCache, SessionData};
use crate::state::{AppState, Cabecera, RecordingState};
use crate::windows_mgr;

pub const EVENT_TICK: &str = "winshotx://recording-tick";
pub const EVENT_SESSION_READY: &str = "winshotx://session-ready";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordOptions {
    pub format: String,
    pub fps: u32,
    /// Lo que suena por los altavoces.
    pub audio: bool,
    /// Y la voz de quien graba. Los dos a la vez se mezclan en una sola pista.
    #[serde(default)]
    pub microphone: bool,
    /// Marcar cada clic con un aro, para que se vea donde se esta pulsando.
    #[serde(default)]
    pub highlight_clicks: bool,
    /// Ensennar los atajos que se pulsan, en una pastilla abajo. Solo atajos: una tecla
    /// suelta no sale nunca, para que una contrasenna escrita no acabe dentro del video.
    #[serde(default)]
    pub highlight_keys: bool,
}

/// Lo que la barra ensenna, cinco veces por segundo.
///
/// Lleva tambien lo que no cambia (formato, sonido) para que la barra no tenga que pedir
/// nada al abrirse: el primer tick ya lo trae todo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingTick {
    pub elapsed_ms: u64,
    pub frames: u64,
    pub bytes: u64,
    pub paused: bool,
    /// Ya se ha pulsado parar y se estan haciendo las miniaturas: la barra ensenna
    /// «Guardando…» y apaga los botones. Es el ultimo tick que llega.
    pub saving: bool,
    /// «video» o «gif».
    pub format: String,
    /// Si el sonido del sistema esta entrando de verdad.
    pub audio: bool,
    /// Y el microfono.
    pub microphone: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub region: Rect,
    pub fps: u32,
    pub frame_count: u32,
    pub duration_ms: u64,
    pub has_audio: bool,
    /// Si se pulso algo durante la grabacion. Sin clics no hay nada a lo que acercarse, y
    /// ofrecer el zoom en el editor seria ofrecer un interruptor que no hace nada.
    pub has_clicks: bool,
    /// Si el cursor de Windows quedo dentro de los fotogramas.
    pub cursor_baked: bool,
    pub format: String,
    pub mp4_path: Option<String>,
}

impl From<&SessionData> for SessionInfo {
    fn from(data: &SessionData) -> Self {
        Self {
            id: data.id.clone(),
            region: data.region,
            fps: data.fps,
            frame_count: data.frames.len() as u32,
            duration_ms: data.duration_ms(),
            has_audio: data.has_audio,
            has_clicks: !data.clics.is_empty(),
            cursor_baked: data.cursor_capturado,
            format: data.format.clone(),
            mp4_path: data
                .mp4_path
                .as_ref()
                .filter(|p| p.exists())
                .map(|p| p.to_string_lossy().to_string()),
        }
    }
}

/// La captura de Windows entrega los colores al reves de como los quiere `image`.
///
/// Se da la vuelta EN EL SITIO: el fotograma ya es nuestro (acaba de llegar por el canal)
/// y copiarlo para cambiarle el orden de los bytes eran ocho megabytes mas por fotograma,
/// sesenta veces por segundo con el anillo encendido.
pub(crate) fn bgra_a_rgba_en_sitio(pixeles: &mut [u8]) {
    for pixel in pixeles.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
}

/// Media Foundation quiere las filas al reves; la captura las entrega derechas.
///
/// Escribe en un buffer que se le presta y se reutiliza fotograma tras fotograma: reservar
/// ocho megabytes nuevos treinta veces por segundo era una parte medible de lo que
/// costaba grabar, y el codificador solo necesita los bytes mientras se los manda.
fn flip_rows_en(input: &[u8], width: u32, height: u32, out: &mut Vec<u8>) {
    let stride = width as usize * 4;
    out.clear();
    out.resize(input.len(), 0);
    for y in 0..height as usize {
        let src = y * stride;
        let dst = (height as usize - 1 - y) * stride;
        if src + stride <= input.len() && dst + stride <= out.len() {
            out[dst..dst + stride].copy_from_slice(&input[src..src + stride]);
        }
    }
}

fn monitor_origin(app: &AppHandle, region: Rect) -> (i32, i32) {
    let state = app.state::<AppState>();
    let (cx, cy) = region.center();
    if let Some(freeze) = state
        .freezes
        .read()
        .iter()
        .find(|f| f.monitor.contains(cx, cy))
    {
        return (freeze.monitor.x, freeze.monitor.y);
    }
    xcap::Monitor::from_point(cx, cy)
        .ok()
        .and_then(|m| Some((m.x().ok()?, m.y().ok()?)))
        .unwrap_or((0, 0))
}

/// El sonido va a dos sitios a la vez: al MP4 de vista previa, para poder oirlo de una
/// pieza, y a un archivo en crudo, que es lo que se recorta al exportar. Sin el archivo,
/// el video que guarda el usuario sale mudo, porque la exportacion vuelve a codificar
/// desde los fotogramas y ahi no hay sonido ninguno.
#[cfg(windows)]
struct SalidaDeAudio {
    captura: Option<crate::record::audio::Captura>,
    archivo: Option<std::io::BufWriter<std::fs::File>>,
    /// Bytes que han llegado al archivo. Sin uno solo, no hay sonido que exportar.
    escritos: u64,
}

#[cfg(windows)]
impl SalidaDeAudio {
    fn abrir(captura: Option<crate::record::audio::Captura>, ruta: std::path::PathBuf) -> Self {
        let archivo = captura.as_ref().and_then(|_| {
            std::fs::File::create(ruta)
                .ok()
                .map(std::io::BufWriter::new)
        });
        Self {
            captura,
            archivo,
            escritos: 0,
        }
    }

    fn formato(&self) -> Option<crate::record::audio::Formato> {
        self.captura.as_ref().map(|c| c.formato)
    }

    /// Lo que haya sonado desde la ultima vez: al archivo y, si sigue vivo, al codificador
    /// de la vista previa. Si el codificador falla, el sonido se queda fuera de la vista
    /// previa pero la imagen sigue y el archivo en crudo tambien: el video exportado
    /// saldra con sonido.
    fn volcar(&mut self, encoder: &mut Option<windows_capture::encoder::VideoEncoder>) {
        use std::io::Write;
        let Some(captura) = self.captura.as_ref() else { return };
        while let Ok(trozo) = captura.trozos.try_recv() {
            let pcm = crate::record::audio::a_pcm16(&trozo.datos);
            if let Some(f) = self.archivo.as_mut() {
                if f.write_all(&pcm).is_ok() {
                    self.escritos += pcm.len() as u64;
                }
            }
            let Some(codificador) = encoder.as_mut() else { continue };
            if codificador
                .send_audio_buffer(&pcm, trozo.desde_el_inicio)
                .is_err()
            {
                *encoder = None;
            }
        }
    }

    /// Para la captura, vacia lo ultimo y dice que sonido hay, si lo hay. Decir que lo hay
    /// sin un byte dejaria al exportador buscando un archivo vacio.
    fn cerrar(
        mut self,
        encoder: &mut Option<windows_capture::encoder::VideoEncoder>,
    ) -> Option<record::AudioInfo> {
        use std::io::Write;
        self.volcar(encoder);
        let captura = self.captura.take()?;
        let formato = captura.formato;
        captura.parar();
        if let Some(mut f) = self.archivo.take() {
            let _ = f.flush();
        }
        (self.escritos > 0).then_some(record::AudioInfo {
            channels: formato.canales,
            sample_rate: formato.muestras_por_segundo,
        })
    }
}

/// Arranca la grabacion de la region: cache sin perdida para editar y MP4 de
/// referencia para que el editor pueda reproducir sin decodificar nada a mano.
#[cfg(windows)]
pub fn start(app: &AppHandle, region: Rect, options: RecordOptions) -> Result<SessionInfo> {
    use std::sync::mpsc::channel;

    use crate::record::win::{self, CaptureFlags};

    let state = app.state::<AppState>();
    if state.is_recording() {
        return Err(AppError::Msg("ya hay una grabación en curso".into()));
    }

    windows_mgr::close_overlays(app);
    let region = region.to_even();
    let id = uuid::Uuid::new_v4().simple().to_string()[..10].to_string();
    let dir = state.session_dir(&id);
    std::fs::create_dir_all(&dir)?;

    let fps = options.fps.clamp(5, 60);
    let origin = monitor_origin(app, region);

    let session_seed = SessionData {
        id: id.clone(),
        dir: dir.clone(),
        region,
        fps,
        format: options.format.clone(),
        has_audio: false, // se pone a cierto abajo, si el altavoz llega a abrirse
        width: region.width,
        height: region.height,
        mp4_path: Some(dir.join("preview.mp4")),
        audio: None,
        clics: Vec::new(),
        teclas: Vec::new(),
        cursor: Vec::new(),
        // El puntero de Windows NO se mete en los fotogramas: se anota por donde va y el
        // editor lo dibuja al exportar, del tamanno que se quiera y sin pixelarlo. Cocido
        // dentro del video media 32 pixeles a 1080p, no se podia agrandar, y ademas
        // chocaba con el dibujado: al encenderlo salian dos punteros.
        cursor_capturado: false,
        frames: Vec::new(),
    };

    let stop = Arc::new(AtomicBool::new(false));
    let pause = Arc::new(AtomicBool::new(false));
    let descartar = Arc::new(AtomicBool::new(false));
    let paused_ms = Arc::new(AtomicU64::new(0));
    let frames_counter = Arc::new(AtomicU64::new(0));
    let bytes_counter = Arc::new(AtomicU64::new(0));

    // El altavoz se abre ANTES de arrancar el hilo que escribe: hay que saber a que
    // frecuencia y con cuantos canales suena para configurar el codificador, y eso no se
    // puede cambiar a mitad del MP4. Si no se puede abrir, se graba sin sonido y se dice:
    // quedarse sin grabacion por no tener altavoz seria mucho peor.
    let fuentes = crate::record::audio::Fuentes {
        sistema: options.audio,
        microfono: options.microphone,
    };
    let audio = if fuentes.ninguna() {
        None
    } else {
        match crate::record::audio::empezar(fuentes, pause.clone()) {
            Ok(captura) => Some(captura),
            Err(error) => {
                eprintln!("[winshotx] sin sonido: {error}");
                None
            }
        }
    };
    let cabecera = Cabecera {
        format: options.format.clone(),
        audio: audio.is_some() && fuentes.sistema,
        microphone: audio.is_some() && fuentes.microfono,
    };

    let (sender, receiver) = channel::<win::CapturedFrame>();
    let writer_frames = frames_counter.clone();
    let writer_bytes = bytes_counter.clone();
    let writer_stop = stop.clone();
    let writer_descartar = descartar.clone();
    let marcar_clics = options.highlight_clicks;
    let marcar_teclas = options.highlight_keys;
    let writer = std::thread::spawn(move || -> Result<SessionData> {
        let mut session = session_seed;
        session.has_audio = audio.is_some();
        let (width, height) = (session.width, session.height);
        let mut cache = FrameCache::new(&session.dir)?;
        let mut salida = SalidaDeAudio::abrir(audio, session.audio_path());
        let mut encoder = build_preview_encoder(&session, salida.formato());
        let mut anotador =
            crate::record::anotador::Anotador::new(region, marcar_clics, marcar_teclas);
        let mut last_ts = 0u64;
        let mut dado_la_vuelta: Vec<u8> = Vec::new();

        // El fin de la grabacion no puede depender de que el canal se cierre: si la
        // pantalla esta quieta no llegan fotogramas y el hilo se quedaria esperando.
        loop {
            let frame = match receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) => frame,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if writer_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    // Con la pantalla quieta no llegan fotogramas, pero el sonido sigue:
                    // se vuelca aqui para que no se acumule en memoria hasta el siguiente.
                    salida.volcar(&mut encoder);
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };
            last_ts = frame.ts_ms;
            // El codificador de vista previa quiere BGRA con las filas al reves, asi que su
            // copia se saca ANTES de dar la vuelta a los colores, y despues el fotograma que
            // llego se convierte en sitio sin pedir mas memoria.
            let para_vista_previa = encoder.is_some();
            if para_vista_previa {
                flip_rows_en(&frame.bgra, width, height, &mut dado_la_vuelta);
            }
            let mut rgba = frame.bgra;
            bgra_a_rgba_en_sitio(&mut rgba);

            // Los aros y la pastilla se pintan en el fotograma que se guarda, no en la
            // pantalla: pintarlos encima del escritorio seria otra ventana transparente,
            // que ademas se colaria en cualquier otra captura.
            anotador.mirar(frame.ts_ms);
            anotador.pintar(&mut rgba, width, height, frame.ts_ms);

            if cache.push_rgba(&rgba, width, height, frame.ts_ms)? {
                writer_frames.store(cache.frame_count() as u64, Ordering::Relaxed);
                writer_bytes.store(cache.bytes_written(), Ordering::Relaxed);
            }
            if let Some(enc) = encoder.as_mut().filter(|_| para_vista_previa) {
                if enc
                    .send_frame_buffer(&dado_la_vuelta, frame.ts_ms as i64 * 10_000)
                    .is_err()
                {
                    // Si el codificador falla, la grabacion sigue: el cache es la fuente real.
                    encoder = None;
                }
            }
            salida.volcar(&mut encoder);
        }

        session.audio = salida.cerrar(&mut encoder);
        session.has_audio = session.audio.is_some();
        let vista_previa_cerrada = encoder.take().is_some_and(|enc| enc.finish().is_ok());
        if !vista_previa_cerrada {
            session.mp4_path = None;
        }

        session.frames = cache.finish(last_ts, session.fps)?;
        let anotaciones = anotador.terminar();
        session.clics = anotaciones.clics;
        session.teclas = anotaciones.teclas;
        session.cursor = anotaciones.cursor;
        // Si se ha descartado, las miniaturas serian trabajo para una carpeta que se va a
        // borrar en cuanto esto vuelva.
        if writer_descartar.load(Ordering::Relaxed) {
            return Ok(session);
        }
        record::generate_thumbnails(&mut session)?;
        session.persist()?;
        Ok(session)
    });

    let control = win::start(
        region,
        origin,
        false,
        fps,
        CaptureFlags {
            sender,
            crop: (0, 0, 0, 0),
            stop: stop.clone(),
            pause: pause.clone(),
            paused_ms: paused_ms.clone(),
            min_interval_ms: 0,
        },
    )?;

    // El marco alrededor de lo que se graba. Que no se pueda abrir no impide grabar.
    let marco = match crate::platform::marco::Marco::abrir(region, grosor_del_marco(region)) {
        Ok(marco) => Some(marco),
        Err(error) => {
            eprintln!("[winshotx] sin marco alrededor de la grabacion: {error}");
            None
        }
    };

    let recording = RecordingState {
        started: Instant::now(),
        stop: stop.clone(),
        pause: pause.clone(),
        paused_ms: paused_ms.clone(),
        pause_started: parking_lot::Mutex::new(None),
        frames: frames_counter,
        bytes: bytes_counter,
        cabecera,
        control: Some(control),
        marco,
        descartar,
        barra: None,
        writer: Some(writer),
    };

    let info = SessionInfo {
        id: id.clone(),
        region,
        fps,
        frame_count: 0,
        duration_ms: 0,
        has_audio: false,
        // Todavia no se ha pulsado nada: esto es lo que se devuelve al EMPEZAR a grabar.
        has_clicks: false,
        cursor_baked: false,
        format: options.format,
        mp4_path: None,
    };

    *state.recording.lock() = Some(recording);
    match windows_mgr::open_recorder(app, region) {
        Ok(label) => {
            if let Some(recording) = state.recording.lock().as_mut() {
                recording.barra = Some(label);
            }
        }
        Err(error) => eprintln!("no se ha podido abrir la barra de grabacion: {error}"),
    }
    spawn_ticker(app.clone(), stop);
    Ok(info)
}

/// Dos pixeles logicos: en una pantalla al 150 % son tres de verdad, para que se vea igual.
#[cfg(windows)]
fn grosor_del_marco(region: Rect) -> i32 {
    let (cx, cy) = region.center();
    let escala = windows_mgr::monitor_de(cx, cy)
        .map(|m| m.escala)
        .unwrap_or(1.0);
    (2.0 * escala).round().max(1.0) as i32
}

#[cfg(not(windows))]
pub fn start(_app: &AppHandle, _region: Rect, _options: RecordOptions) -> Result<SessionInfo> {
    Err(AppError::Unsupported)
}

#[cfg(windows)]
fn build_preview_encoder(
    session: &SessionData,
    audio: Option<crate::record::audio::Formato>,
) -> Option<windows_capture::encoder::VideoEncoder> {
    use windows_capture::encoder::{
        AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
        VideoSettingsSubType,
    };

    let path = session.mp4_path.as_ref()?;
    let options = crate::encode::mp4::Mp4Options {
        width: session.width,
        height: session.height,
        fps: session.fps,
        quality: 75,
    };
    VideoEncoder::new(
        VideoSettingsBuilder::new(session.width, session.height)
            .sub_type(VideoSettingsSubType::H264)
            .frame_rate(session.fps)
            .bitrate(options.bitrate()),
        match audio {
            // El codificador recibe enteros de 16 bits, no la coma flotante que da el
            // mezclador: la conversion la hace `audio::a_pcm16` antes de entregarlo.
            Some(formato) => AudioSettingsBuilder::default()
                .channel_count(u32::from(formato.canales))
                .sample_rate(formato.muestras_por_segundo)
                .bit_per_sample(16)
                .disabled(false),
            None => AudioSettingsBuilder::default().disabled(true),
        },
        ContainerSettingsBuilder::default(),
        path,
    )
    .ok()
}

/// Lo que la barra tiene que ensennar ahora mismo.
fn tick_de(recording: &RecordingState, saving: bool) -> RecordingTick {
    RecordingTick {
        elapsed_ms: recording.elapsed_ms(),
        frames: recording.frames.load(Ordering::Relaxed),
        bytes: recording.bytes.load(Ordering::Relaxed),
        paused: recording.pause.load(Ordering::Relaxed),
        saving,
        format: recording.cabecera.format.clone(),
        audio: recording.cabecera.audio,
        microphone: recording.cabecera.microphone,
    }
}

/// Se emite directamente a la barra: un emit global tropieza con las ventanas recien
/// cerradas y el aviso se pierde por el camino.
fn avisar_barra(app: &AppHandle, tick: RecordingTick) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(windows_mgr::RECORDER_PREFIX) {
            let _ = window.emit(EVENT_TICK, tick.clone());
        }
    }
}

fn spawn_ticker(app: AppHandle, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            let tick = {
                let state = app.state::<AppState>();
                let guard = state.recording.lock();
                guard.as_ref().map(|recording| tick_de(recording, false))
            };
            let Some(tick) = tick else { break };
            avisar_barra(&app, tick);
            std::thread::sleep(Duration::from_millis(200));
        }
    });
}

/// Para la captura y espera a que el escritor termine. **Tarda**: al final el escritor
/// hace las miniaturas de todos los fotogramas, que con un minuto de grabacion son unos
/// segundos. Por eso nadie llama a esto desde el hilo principal: ver `stop`.
fn terminar(mut recording: RecordingState) -> Result<SessionData> {
    recording.stop.store(true, Ordering::Relaxed);

    // Parar la captura puede tardar en devolver el control; se hace aparte para que
    // el editor no dependa de ello.
    #[cfg(windows)]
    if let Some(control) = recording.control.take() {
        std::thread::spawn(move || {
            let _ = control.stop();
        });
    }
    // El marco se quita ya: lo que viene ahora es guardar, y un marco rojo encima de un
    // escritorio que ya no se graba dice lo contrario de lo que pasa.
    #[cfg(windows)]
    drop(recording.marco.take());

    let writer = recording
        .writer
        .take()
        .ok_or_else(|| AppError::Msg("la grabación no tenía escritor".into()))?;
    writer
        .join()
        .map_err(|_| AppError::Msg("el hilo de escritura se ha caído".into()))?
}

/// Para la grabacion y vuelve enseguida. Lo que tarda (las miniaturas, abrir el editor o
/// guardar el archivo) pasa en otro hilo.
///
/// **Vuelve enseguida a proposito.** Esto lo llama el atajo global, y el manejador del
/// atajo corre en el hilo principal: mientras esperaba a las miniaturas, la aplicacion
/// entera se quedaba clavada, la barra incluida, sin ensennar ni que se estaba guardando.
/// Ahora la barra recibe un ultimo tick con `saving` y se queda diciendo «Guardando…»
/// hasta que se cierra sola.
pub fn stop(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let Some(recording) = state.recording.lock().take() else {
        return Err(AppError::NoRecording);
    };
    avisar_barra(app, tick_de(&recording, true));
    let open_editor = state.settings.read().open_editor_after_recording;
    let barra = recording.barra.clone();

    let handle = app.clone();
    std::thread::spawn(move || match terminar(recording) {
        Ok(session) if session.frames.is_empty() => {
            eprintln!("no se ha capturado ningún fotograma; prueba a bajar los fps");
            let _ = std::fs::remove_dir_all(&session.dir);
            cerrar_esta_barra(&handle, barra.as_deref());
        }
        Ok(session) => entregar(&handle, session, open_editor, barra.as_deref()),
        Err(error) => {
            eprintln!("fallo al parar: {error}");
            // La barra es always-on-top y no tiene aspa: si esto se fuera sin cerrarla,
            // se quedaria encima de todo y no habria forma de quitarla.
            cerrar_esta_barra(&handle, barra.as_deref());
        }
    });
    Ok(())
}

/// La barra de esta grabacion y solo esa. Si no se sabe cual era, todas: mejor cerrar una
/// de mas que dejar una siempre encima sin aspa.
fn cerrar_esta_barra(app: &AppHandle, barra: Option<&str>) {
    match barra {
        Some(label) => windows_mgr::close_recorder_label(app, label),
        None => windows_mgr::close_recorder(app),
    }
}

/// Lo que pasa con la grabacion ya terminada: al editor, o directa a la carpeta.
///
/// Corre en un hilo neutral: tocar ventanas desde el hilo del atajo bloquea el bucle de
/// eventos, y desde el de un comando tambien puede.
fn entregar(app: &AppHandle, session: SessionData, open_editor: bool, barra: Option<&str>) {
    let state = app.state::<AppState>();
    state
        .sessions
        .write()
        .insert(session.id.clone(), session.clone());
    cerrar_esta_barra(app, barra);

    if open_editor {
        if let Err(error) = windows_mgr::open_editor(app, &session.id) {
            eprintln!("no se ha podido abrir el editor: {error}");
        }
    } else {
        // Sin editor, la grabacion se guarda sola en la carpeta de siempre, con los mismos
        // ajustes con los que abre el editor. Antes se quedaba en la carpeta temporal, sin
        // archivo, sin aviso y sin forma de volver a ella: el ajuste decia «dejarla
        // guardada» y lo que hacia era perderla.
        let con_sonido = state.settings.read().play_sound;
        match crate::exporter::export(app, peticion_por_defecto(&session)) {
            Ok(resultado) => {
                if con_sonido {
                    crate::platform::sonido::obturador();
                }
                eprintln!("[winshotx] grabación guardada en {}", resultado.path);
            }
            Err(error) => eprintln!("[winshotx] no se ha podido guardar la grabación: {error}"),
        }
    }
    let _ = app.emit(EVENT_SESSION_READY, session.id);
}

/// Como se exporta una grabacion cuando nadie la va a recortar: entera, al tamanno al
/// que se grabo, con el sonido que tenga, y a la carpeta de los ajustes. Son los mismos
/// valores con los que el editor abre su panel, para que «sin editor» de el mismo archivo
/// que «editor y Ctrl+S sin tocar nada».
pub(crate) fn peticion_por_defecto(session: &SessionData) -> crate::exporter::ExportRequest {
    crate::exporter::ExportRequest {
        session_id: session.id.clone(),
        format: if session.format == "gif" { "gif" } else { "mp4" }.into(),
        engine: "native".into(),
        from: 0,
        to: session.frames.len().saturating_sub(1),
        width: session.width,
        height: session.height,
        fps: session.fps.min(30),
        quality: 80,
        audio: session.has_audio,
        loop_forever: true,
        margin: 0,
        background: String::new(),
        shadow: false,
        annotations: Vec::new(),
        crop: None,
        zoom: 0.0,
        clicks: false,
        keys: false,
        // El puntero se dibuja, al tamanno normal: sin esto el video saldria sin raton,
        // porque la grabacion ya no lo mete en los fotogramas.
        cursor: if session.cursor_capturado {
            0.0
        } else {
            crate::encode::estudio::puntero_normal(session.height)
        },
        speed: 1.0,
        destination: None,
        copy_to_clipboard: false,
    }
}

/// Cierra la barra de grabacion desde un hilo neutral, nunca desde el del atajo.
fn cerrar_barra(app: &AppHandle, barra: Option<String>) {
    let handle = app.clone();
    std::thread::spawn(move || cerrar_esta_barra(&handle, barra.as_deref()));
}

/// Tira la grabacion. Vuelve enseguida, como `stop`, y la carpeta se borra por detras.
pub fn cancel(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let Some(recording) = state.recording.lock().take() else {
        cerrar_barra(app, None);
        return Err(AppError::NoRecording);
    };
    recording.descartar.store(true, Ordering::Relaxed);
    cerrar_barra(app, recording.barra.clone());
    std::thread::spawn(move || match terminar(recording) {
        Ok(session) => {
            let _ = std::fs::remove_dir_all(&session.dir);
        }
        Err(error) => eprintln!("fallo al descartar: {error}"),
    });
    Ok(())
}

pub fn set_paused(app: &AppHandle, paused: bool) -> Result<()> {
    let state = app.state::<AppState>();
    let guard = state.recording.lock();
    let recording = guard.as_ref().ok_or(AppError::NoRecording)?;
    let mut pause_started = recording.pause_started.lock();
    if paused {
        if pause_started.is_none() {
            *pause_started = Some(Instant::now());
        }
    } else if let Some(started) = pause_started.take() {
        recording
            .paused_ms
            .fetch_add(started.elapsed().as_millis() as u64, Ordering::Relaxed);
    }
    recording.pause.store(paused, Ordering::Relaxed);
    #[cfg(windows)]
    if let Some(marco) = recording.marco.as_ref() {
        marco.pausado(paused);
    }
    Ok(())
}

/// Convierte una captura estatica en una sesion de un solo fotograma,
/// para que el editor pueda escalarla y exportarla como cualquier otra.
pub fn session_from_image(app: &AppHandle, image: &RgbaImage, region: Rect) -> Result<SessionData> {
    let state = app.state::<AppState>();
    let id = uuid::Uuid::new_v4().simple().to_string()[..10].to_string();
    let dir = state.session_dir(&id);
    std::fs::create_dir_all(&dir)?;

    let mut cache = FrameCache::new(&dir)?;
    cache.push_rgba(image.as_raw(), image.width(), image.height(), 0)?;
    let frames = cache.finish(0, 1)?;

    let mut session = SessionData {
        id: id.clone(),
        dir,
        region,
        fps: 1,
        format: "still".into(),
        has_audio: false,
        audio: None,
        // Una captura fija no tiene clics que anotar: no hay tiempo dentro.
        clics: Vec::new(),
        cursor_capturado: false,
        teclas: Vec::new(),
        cursor: Vec::new(),
        width: image.width(),
        height: image.height(),
        mp4_path: None,
        frames,
    };
    record::generate_thumbnails(&mut session)?;
    session.persist()?;
    state.sessions.write().insert(id, session.clone());
    Ok(session)
}

#[cfg(all(test, windows))]
mod pruebas_de_audio {
    /// Fabrica un MP4 pequenno con imagen y sonido usando el mismo camino que la
    /// grabacion de verdad, y comprueba que el archivo sale con pista de sonido dentro.
    ///
    /// No corre sola porque usa el codificador por hardware de Windows:
    /// `cargo test --lib el_mp4_sale_con_pista -- --ignored --nocapture`.
    ///
    /// Comprueba lo unico que no se puede saber leyendo el codigo: que Media Foundation
    /// acepta ese formato de audio. Si lo rechaza, el MP4 sale mudo y no se entera nadie
    /// hasta que alguien reproduce el video.
    #[test]
    #[ignore]
    fn el_mp4_sale_con_pista_de_sonido() {
        use windows_capture::encoder::{
            AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
            VideoSettingsSubType,
        };

        let (ancho, alto, fps) = (320u32, 240u32, 30u32);
        let unico = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let destino = std::env::temp_dir().join(format!("winshotx-audio-{unico}.mp4"));

        let mut encoder = VideoEncoder::new(
            VideoSettingsBuilder::new(ancho, alto)
                .sub_type(VideoSettingsSubType::H264)
                .frame_rate(fps)
                .bitrate(2_000_000),
            AudioSettingsBuilder::default()
                .channel_count(2)
                .sample_rate(48_000)
                .bit_per_sample(16)
                .disabled(false),
            ContainerSettingsBuilder::default(),
            &destino,
        )
        .expect("no se ha podido crear el codificador");

        // Un segundo: treinta fotogramas y el sonido que les corresponde.
        let bgra = vec![90u8; (ancho * alto) as usize * 4];
        // 48.000 instantes por segundo entre 30 fotogramas, dos canales de dos bytes.
        let por_fotograma = vec![0u8; (48_000 / 30) * 2 * 2];
        for i in 0..fps {
            encoder
                .send_frame_buffer(&bgra, i as i64 * (10_000_000 / fps as i64))
                .expect("no se ha podido enviar el fotograma");
            encoder
                .send_audio_buffer(&por_fotograma, 0)
                .expect("no se ha podido enviar el sonido");
        }
        encoder.finish().expect("no se ha podido cerrar el MP4");

        let bytes = std::fs::read(&destino).expect("no se ha escrito el MP4");
        let _ = std::fs::remove_file(&destino);
        println!("MP4 de un segundo con sonido: {} bytes", bytes.len());

        // `mp4a` es como se llama la pista de audio dentro del archivo. Si no está, el
        // vídeo salió mudo por mucho que el codificador no se quejara.
        let tiene_pista = bytes.windows(4).any(|v| v == b"mp4a");
        assert!(tiene_pista, "el MP4 ha salido sin pista de sonido");
        assert!(bytes.len() > 5_000, "el MP4 ha salido demasiado pequeño");
    }
}
