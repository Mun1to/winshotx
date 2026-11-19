//! Arrancar una grabacion en Linux.
//!
//! Hace lo mismo que `start` en Windows con lo que hay alli: los fotogramas salen de
//! `record::linux` (X11 o el portal de Wayland) y van a la misma cache QOI, asi que el
//! editor, el recorte y el GIF son los de siempre. Lo que en Windows hace Media Foundation
//! (el MP4 de vista previa en vivo) no esta: la sesion sale sin `mp4_path`.
//!
//! **La barra de grabar no se puede esconder del video.** En Windows se le dice al sistema
//! que no la grabe; en Linux eso no existe, ni en X11 ni en Wayland. Asi que se pone fuera
//! de la zona que se graba, y si no cabe fuera (se graba la pantalla entera), no hay barra:
//! se para con el atajo de grabar o desde el icono de la bandeja.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::{spawn_ticker, Arrancando, RecordOptions, SessionInfo};
use crate::capture::Rect;
use crate::error::{AppError, Result};
use crate::record::linux::{Fotograma, Marcha, Reciclados};
use crate::record::{self, FrameCache, SessionData};
use crate::state::{AppState, Cabecera, RecordingState};
use crate::windows_mgr;

/// Lo que se espera desde que se esconde el overlay hasta la primera imagen.
///
/// Esconder una ventana no es instantaneo: el escritorio tiene que quitarla y lo de debajo
/// tiene que volver a pintarse. Sin esta espera el primer fotograma del video es el propio
/// overlay, con la pantalla oscurecida y el recuadro de la seleccion.
const QUE_SE_VAYA_EL_OVERLAY: Duration = Duration::from_millis(250);

pub(super) fn start(app: &AppHandle, region: Rect, options: RecordOptions) -> Result<SessionInfo> {
    let state = app.state::<AppState>();
    let Some(_arrancando) = Arrancando::tomar() else {
        return Err(AppError::Msg("ya hay una grabación arrancando".into()));
    };
    if state.is_recording() {
        return Err(AppError::Msg("ya hay una grabación en curso".into()));
    }

    windows_mgr::close_overlays(app);
    let fps = options.fps.clamp(5, 60);
    // Aqui se abre lo que haga falta, y en Wayland se espera a que la persona conteste al
    // escritorio: el reloj no empieza hasta despues.
    let preparada = record::linux::preparar(region.to_even(), fps)?;
    let region = preparada.region();
    std::thread::sleep(QUE_SE_VAYA_EL_OVERLAY);

    let id = uuid::Uuid::new_v4().simple().to_string()[..10].to_string();
    let dir = state.session_dir(&id);
    std::fs::create_dir_all(&dir)?;

    let session_seed = SessionData {
        id: id.clone(),
        dir: dir.clone(),
        region,
        fps,
        format: options.format.clone(),
        has_audio: false,
        width: region.width,
        height: region.height,
        // Sin vista previa en vivo: el MP4 es de la Fase 3 del plan de Linux.
        mp4_path: None,
        audio: None,
        clics: Vec::new(),
        teclas: Vec::new(),
        cursor: Vec::new(),
        // Como en Windows, el puntero no va cocido: se apunta y lo dibuja el exportador.
        cursor_capturado: false,
        formas: Vec::new(),
        punteros: Vec::new(),
        cambios_puntero: Vec::new(),
        marcas_zoom: Vec::new(),
        camara: None,
        escritura: Vec::new(),
        pistas: None,
        frames: Vec::new(),
    };

    let stop = Arc::new(AtomicBool::new(false));
    let pause = Arc::new(AtomicBool::new(false));
    let descartar = Arc::new(AtomicBool::new(false));
    let paused_ms = Arc::new(AtomicU64::new(0));
    let frames_counter = Arc::new(AtomicU64::new(0));
    let bytes_counter = Arc::new(AtomicU64::new(0));
    let fin_ms = Arc::new(AtomicU64::new(0));
    // El cero del reloj: el mismo para los fotogramas, el raton y la barra.
    let reloj_inicio = Instant::now();
    let marcha = Marcha {
        stop: stop.clone(),
        pause: pause.clone(),
        paused_ms: paused_ms.clone(),
        start: reloj_inicio,
    };

    eprintln!(
        "[grabar] {}x{} en {},{} a {fps} fps ({})",
        region.width, region.height, region.x, region.y, options.format
    );
    let (sender, receiver) = channel::<Fotograma>();
    let reciclados = Reciclados::default();
    let en_marcha = preparada.grabar(marcha, sender, reciclados.clone());

    let writer_frames = frames_counter.clone();
    let writer_bytes = bytes_counter.clone();
    let writer_stop = stop.clone();
    let writer_fin = fin_ms.clone();
    let writer_descartar = descartar.clone();
    let writer = std::thread::spawn(move || -> Result<SessionData> {
        let mut session = session_seed;
        let (width, height) = (session.width, session.height);
        let mut cache = FrameCache::new(&session.dir)?;
        let mut last_ts = 0u64;
        // El fin no puede depender de que el canal se cierre: con la pantalla quieta no
        // llegan fotogramas y el hilo se quedaria esperando.
        loop {
            let frame = match receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) => frame,
                Err(RecvTimeoutError::Timeout) => {
                    if writer_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            };
            last_ts = frame.ts_ms;
            if cache.push_rgba(&frame.rgba, width, height, frame.ts_ms)? {
                writer_frames.store(cache.frame_count() as u64, Ordering::Relaxed);
                writer_bytes.store(cache.bytes_written(), Ordering::Relaxed);
            }
            reciclados.devolver(frame.rgba);
        }
        // Cuanto se grabo de verdad, y no el instante del ultimo fotograma: con el final
        // quieto no llega ninguno, y el video se cortaria donde llego el ultimo.
        let grabado_ms = writer_fin.load(Ordering::Relaxed).max(last_ts);
        session.frames = cache.finish(grabado_ms, session.fps)?;

        eprintln!("[grabar] {} fotogramas en {grabado_ms} ms", session.frames.len());
        let rastro = en_marcha.terminar();
        session.cursor = rastro.cursor;
        session.cambios_puntero = rastro.cambios_puntero;
        // Las imagenes de los punteros, a disco: son lo que dibuja el exportador. Una que no
        // se pueda escribir se queda fuera y se dibuja la flecha en su lugar.
        for (id, p) in rastro.punteros.iter().enumerate() {
            let archivo = session.dir.join(format!("cursor-{id}.png"));
            if crate::encode::png::save_fast(&p.imagen, &archivo).is_ok() {
                session.punteros.push(record::PunteroGrabado {
                    id: id as u32,
                    ancho: p.imagen.width(),
                    alto: p.imagen.height(),
                    caliente: p.caliente,
                    archivo: archivo.to_string_lossy().to_string(),
                });
            }
        }
        if writer_descartar.load(Ordering::Relaxed) {
            return Ok(session);
        }
        session.persist()?;
        Ok(session)
    });

    let recording = RecordingState {
        id: id.clone(),
        started: reloj_inicio,
        stop: stop.clone(),
        pause,
        paused_ms,
        pause_started: parking_lot::Mutex::new(None),
        fin_ms,
        frames: frames_counter,
        bytes: bytes_counter,
        cabecera: Cabecera { format: options.format.clone(), audio: false, microphone: false },
        descartar,
        barra: None,
        writer: Some(writer),
    };
    *state.recording.lock() = Some(recording);
    crate::tray::al_cambiar_la_grabacion(app);

    match windows_mgr::open_recorder_fuera(app, region) {
        Ok(Some(label)) => {
            if let Some(recording) = state.recording.lock().as_mut() {
                recording.barra = Some(label);
            }
        }
        Ok(None) => eprintln!("[grabar] la barra no cabe fuera de lo que se graba: se para con el atajo o desde la bandeja"),
        Err(error) => eprintln!("no se ha podido abrir la barra de grabacion: {error}"),
    }
    spawn_ticker(app.clone(), stop);

    Ok(SessionInfo {
        id,
        region,
        fps,
        frame_count: 0,
        duration_ms: 0,
        has_audio: false,
        has_clicks: false,
        cursor_baked: false,
        format: options.format,
        mp4_path: None,
        sonido_pc: None,
        microfono: None,
        pico_pc: None,
        pico_microfono: None,
        subida_pc: None,
        previa_fallida: false,
    })
}
