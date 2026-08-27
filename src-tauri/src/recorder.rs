use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::capture::Rect;
use crate::error::{AppError, Result};
use crate::record::{self, FrameCache, SessionData};
use crate::state::{AppState, RecordingState};
use crate::windows_mgr;

pub const EVENT_TICK: &str = "winshotx://recording-tick";
pub const EVENT_SESSION_READY: &str = "winshotx://session-ready";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordOptions {
    pub format: String,
    pub fps: u32,
    pub capture_cursor: bool,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingTick {
    pub elapsed_ms: u64,
    pub frames: u64,
    pub bytes: u64,
    pub paused: bool,
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
            format: data.format.clone(),
            mp4_path: data
                .mp4_path
                .as_ref()
                .filter(|p| p.exists())
                .map(|p| p.to_string_lossy().to_string()),
        }
    }
}

fn bgra_to_rgba(input: &[u8]) -> Vec<u8> {
    let mut out = input.to_vec();
    for pixel in out.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    out
}

/// Media Foundation quiere las filas al reves; la captura las entrega derechas.
fn flip_rows(input: &[u8], width: u32, height: u32) -> Vec<u8> {
    let stride = width as usize * 4;
    let mut out = vec![0u8; input.len()];
    for y in 0..height as usize {
        let src = y * stride;
        let dst = (height as usize - 1 - y) * stride;
        if src + stride <= input.len() && dst + stride <= out.len() {
            out[dst..dst + stride].copy_from_slice(&input[src..src + stride]);
        }
    }
    out
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
        frames: Vec::new(),
    };

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
        match crate::record::audio::empezar(fuentes) {
            Ok(captura) => Some(captura),
            Err(error) => {
                eprintln!("[winshotx] sin sonido: {error}");
                None
            }
        }
    };

    let (sender, receiver) = channel::<win::CapturedFrame>();
    let stop = Arc::new(AtomicBool::new(false));
    let pause = Arc::new(AtomicBool::new(false));
    let paused_ms = Arc::new(AtomicU64::new(0));
    let frames_counter = Arc::new(AtomicU64::new(0));
    let bytes_counter = Arc::new(AtomicU64::new(0));

    let writer_frames = frames_counter.clone();
    let writer_bytes = bytes_counter.clone();
    let writer_stop = stop.clone();
    let marcar_clics = options.highlight_clicks;
    let marcar_teclas = options.highlight_keys;
    let writer = std::thread::spawn(move || -> Result<SessionData> {
        let mut session = session_seed;
        session.has_audio = audio.is_some();
        let width = session.width;
        let height = session.height;
        let mut cache = FrameCache::new(&session.dir)?;
        let formato_audio = audio.as_ref().map(|a| a.formato);
        let mut encoder = build_preview_encoder(&session, formato_audio);
        let mut last_ts = 0u64;

        // El sonido va a dos sitios a la vez: al MP4 de vista previa, para poder oirlo de
        // una pieza, y a un archivo en crudo, que es lo que se recorta al exportar. Sin el
        // archivo, el video que guarda el usuario sale mudo, porque la exportacion vuelve
        // a codificar desde los fotogramas y ahi no hay sonido ninguno.
        // Los clics que todavia se ven. El vigilante mira los botones en cada fotograma,
        // que es mucho mas barato y mucho menos peligroso que engancharse al raton del
        // sistema: un enganche mal hecho le deja el escritorio a tirones a quien lo tenga.
        let mut vigilante = crate::record::raton::Vigilante::default();
        let mut clics: Vec<crate::record::realce::Clic> = Vec::new();
        let mut teclado = crate::record::teclas::Vigilante::default();
        let mut atajo: Option<crate::record::teclas::Atajo> = None;
        let mut pastillas = crate::record::pastilla::Cache::default();

        let mut audio_file = audio.as_ref().and_then(|_| {
            std::fs::File::create(session.audio_path())
                .ok()
                .map(std::io::BufWriter::new)
        });
        let mut bytes_de_audio: u64 = 0;

        let volcar_audio = |enc: &mut Option<windows_capture::encoder::VideoEncoder>,
                                archivo: &mut Option<std::io::BufWriter<std::fs::File>>,
                                escritos: &mut u64| {
            use std::io::Write;
            let Some(captura) = audio.as_ref() else { return };
            while let Ok(trozo) = captura.trozos.try_recv() {
                let pcm = crate::record::audio::a_pcm16(&trozo.datos);
                if let Some(f) = archivo.as_mut() {
                    if f.write_all(&pcm).is_ok() {
                        *escritos += pcm.len() as u64;
                    }
                }
                let Some(codificador) = enc.as_mut() else { continue };
                if codificador
                    .send_audio_buffer(&pcm, trozo.desde_el_inicio)
                    .is_err()
                {
                    // El sonido se queda fuera de la vista previa, pero la imagen sigue y
                    // el archivo en crudo tambien: el video exportado saldra con sonido.
                    *enc = None;
                }
            }
        };

        // El fin de la grabacion no puede depender de que el canal se cierre: si la
        // pantalla esta quieta no llegan fotogramas y el hilo se quedaria esperando.
        loop {
            let frame = match receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) => frame,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if writer_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };
            last_ts = frame.ts_ms;
            let mut rgba = bgra_to_rgba(&frame.bgra);

            // El aro se pinta en el fotograma que se guarda, no en la pantalla: pintarlo
            // encima del escritorio seria otra ventana transparente, que ademas se colaria
            // en cualquier otra captura.
            if marcar_clics {
                if let Some(clic) = vigilante.mirar(frame.ts_ms) {
                    clics.push(clic);
                }
                crate::record::realce::olvidar_viejos(&mut clics, frame.ts_ms);
            }
            if marcar_teclas {
                if let Some(nuevo) = teclado.mirar(frame.ts_ms) {
                    atajo = Some(nuevo);
                }
            }

            // Se monta la imagen UNA vez aunque haya que pintar las dos cosas: cada vuelta
            // copia dos millones de pixeles y a treinta por segundo eso se nota.
            let hay_teclas = atajo.as_ref().is_some_and(|a| {
                crate::record::pastilla::opacidad(
                    frame.ts_ms.saturating_sub(a.ms),
                    crate::record::teclas::DURACION_MS,
                ) > 0.0
            });
            if !clics.is_empty() || hay_teclas {
                if let Some(mut imagen) = image::RgbaImage::from_raw(width, height, rgba.clone()) {
                    if !clics.is_empty() {
                        crate::record::realce::pintar(
                            &mut imagen,
                            &clics,
                            region.x,
                            region.y,
                            frame.ts_ms,
                        );
                    }
                    if let Some(a) = atajo.as_ref().filter(|_| hay_teclas) {
                        let opaca = crate::record::pastilla::opacidad(
                            frame.ts_ms.saturating_sub(a.ms),
                            crate::record::teclas::DURACION_MS,
                        );
                        if let Some(dibujo) = pastillas.pastilla(&a.texto) {
                            crate::record::pastilla::pegar(&mut imagen, dibujo, opaca);
                        }
                    }
                    rgba = imagen.into_raw();
                }
            }

            if cache.push_rgba(&rgba, width, height, frame.ts_ms)? {
                writer_frames.store(cache.frame_count() as u64, Ordering::Relaxed);
                writer_bytes.store(cache.bytes_written(), Ordering::Relaxed);
            }
            if let Some(enc) = encoder.as_mut() {
                let flipped = flip_rows(&frame.bgra, width, height);
                if enc
                    .send_frame_buffer(&flipped, frame.ts_ms as i64 * 10_000)
                    .is_err()
                {
                    // Si el codificador falla, la grabacion sigue: el cache es la fuente real.
                    encoder = None;
                }
            }
            volcar_audio(&mut encoder, &mut audio_file, &mut bytes_de_audio);
        }

        // Lo ultimo que quedo sonando, ya con la captura parada.
        volcar_audio(&mut encoder, &mut audio_file, &mut bytes_de_audio);
        if let Some(captura) = audio {
            let formato = captura.formato;
            captura.parar();
            if let Some(mut f) = audio_file.take() {
                use std::io::Write;
                let _ = f.flush();
            }
            // Sin un solo byte no hay sonido que exportar, y decir que lo hay dejaria al
            // exportador buscando un archivo vacio.
            session.audio = (bytes_de_audio > 0).then_some(crate::record::AudioInfo {
                channels: formato.canales,
                sample_rate: formato.muestras_por_segundo,
            });
            session.has_audio = session.audio.is_some();
        }

        if let Some(enc) = encoder.take() {
            if enc.finish().is_err() {
                session.mp4_path = None;
            }
        } else {
            session.mp4_path = None;
        }

        session.frames = cache.finish(last_ts, session.fps)?;
        record::generate_thumbnails(&mut session)?;
        session.persist()?;
        Ok(session)
    });

    let control = win::start(
        region,
        origin,
        options.capture_cursor,
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

    let recording = RecordingState {
        session_id: id.clone(),
        region,
        started: Instant::now(),
        stop: stop.clone(),
        pause: pause.clone(),
        paused_ms: paused_ms.clone(),
        pause_started: parking_lot::Mutex::new(None),
        frames: frames_counter,
        bytes: bytes_counter,
        cancelled: Arc::new(AtomicBool::new(false)),
        control: Some(control),
        writer: Some(writer),
    };

    let info = SessionInfo {
        id: id.clone(),
        region,
        fps,
        frame_count: 0,
        duration_ms: 0,
        has_audio: false,
        format: options.format,
        mp4_path: None,
    };

    *state.recording.lock() = Some(recording);
    if let Err(error) = windows_mgr::open_recorder(app, region) {
        eprintln!("no se ha podido abrir la barra de grabacion: {error}");
    }
    spawn_ticker(app.clone(), stop);
    Ok(info)
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

fn spawn_ticker(app: AppHandle, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            let tick = {
                let state = app.state::<AppState>();
                let guard = state.recording.lock();
                guard.as_ref().map(|recording| RecordingTick {
                    elapsed_ms: recording.elapsed_ms(),
                    frames: recording.frames.load(Ordering::Relaxed),
                    bytes: recording.bytes.load(Ordering::Relaxed),
                    paused: recording.pause.load(Ordering::Relaxed),
                })
            };
            let Some(tick) = tick else { break };
            // Se emite directamente a la barra: un emit global tropieza con las
            // ventanas recien cerradas y el aviso se pierde por el camino.
            for (label, window) in app.webview_windows() {
                if label.starts_with(windows_mgr::RECORDER_PREFIX) {
                    let _ = window.emit(EVENT_TICK, tick.clone());
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
}

fn finish_recording(app: &AppHandle) -> Result<SessionData> {
    let state = app.state::<AppState>();
    let Some(mut recording) = state.recording.lock().take() else {
        return Err(AppError::NoRecording);
    };
    recording.stop.store(true, Ordering::Relaxed);

    // Parar la captura puede tardar en devolver el control; se hace aparte para que
    // el editor no dependa de ello.
    #[cfg(windows)]
    if let Some(control) = recording.control.take() {
        std::thread::spawn(move || {
            let _ = control.stop();
        });
    }

    let writer = recording
        .writer
        .take()
        .ok_or_else(|| AppError::Msg("la grabación no tenía escritor".into()))?;
    writer
        .join()
        .map_err(|_| AppError::Msg("el hilo de escritura se ha caído".into()))?
}

pub fn stop(app: &AppHandle) -> Result<SessionInfo> {
    // La barra es always-on-top y no tiene aspa: si esto se va por el desague sin
    // cerrarla, se queda encima de todo y no hay forma de quitarla.
    let session = match finish_recording(app) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("fallo al parar: {error}");
            cerrar_barra(app);
            return Err(error);
        }
    };

    if session.frames.is_empty() {
        let _ = std::fs::remove_dir_all(&session.dir);
        cerrar_barra(app);
        return Err(AppError::Msg(
            "no se ha capturado ningún fotograma; prueba a bajar los fps".into(),
        ));
    }

    let info = SessionInfo::from(&session);
    let state = app.state::<AppState>();
    let open_editor = state.settings.read().open_editor_after_recording;
    state
        .sessions
        .write()
        .insert(session.id.clone(), session.clone());

    // Tocar ventanas desde aqui es peligroso: esta funcion la llama tanto un comando
    // como el hilo del atajo global, y crear una ventana desde ese hilo bloquea el
    // bucle de eventos. Se hace siempre desde un hilo neutral.
    let handle = app.clone();
    let session_id = session.id.clone();
    std::thread::spawn(move || {
        windows_mgr::close_recorder(&handle);
        if open_editor {
            if let Err(error) = windows_mgr::open_editor(&handle, &session_id) {
                eprintln!("no se ha podido abrir el editor: {error}");
            }
        }
        let _ = handle.emit(EVENT_SESSION_READY, session_id);
    });

    Ok(info)
}

/// Cierra la barra de grabacion desde un hilo neutral, nunca desde el del atajo.
fn cerrar_barra(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || windows_mgr::close_recorder(&handle));
}

pub fn cancel(app: &AppHandle) -> Result<()> {
    let session = match finish_recording(app) {
        Ok(session) => session,
        Err(error) => {
            cerrar_barra(app);
            return Err(error);
        }
    };
    cerrar_barra(app);
    let _ = std::fs::remove_dir_all(&session.dir);
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
