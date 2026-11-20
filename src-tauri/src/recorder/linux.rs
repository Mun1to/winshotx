//! Arrancar una grabacion en Linux.
//!
//! Hace lo mismo que `start` en Windows con lo que hay alli: los fotogramas salen de
//! `record::linux` (X11 o el portal de Wayland) y van a la misma cache QOI, asi que el
//! editor, el recorte y el GIF son los de siempre. El MP4 de vista previa en vivo lo escribe
//! el GStreamer del sistema (`Previa`), y si no puede, la sesion sale sin `mp4_path` y se
//! escribe despues (`replay::preparar_por_detras`), como en Windows.
//!
//! **La barra de grabar no se puede esconder del video.** En Windows se le dice al sistema
//! que no la grabe; en Linux eso no existe, ni en X11 ni en Wayland. Asi que se pone fuera
//! de la zona que se graba, y si no cabe fuera (se graba la pantalla entera, o es Wayland,
//! donde una ventana no elige su sitio), no hay barra: se para con el atajo de grabar o
//! desde el icono de la bandeja. Como se para se dice ANTES de grabar, en la pantalla de
//! elegir zona (`platform::como_parar`): mientras se graba no se ensenna nada, ni un aviso
//! del escritorio, porque saldria en el video.

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

/// El alto de la vista previa en vivo, y sus fotogramas por segundo como mucho. Lo mismo que
/// la que se escribe despues (`replay::escribir_vista_previa`): no es lo que se lleva nadie,
/// es lo que se mira en el hueco del editor, y aqui se codifica por CPU mientras se graba.
const ALTO_DE_LA_PREVIA: u32 = 540;
const FPS_DE_LA_PREVIA: u64 = 30;

/// La vista previa en vivo: el MP4 que el editor reproduce nada mas abrirse.
///
/// Solo imagen: el sonido va a sus pistas, y el editor las hace sonar por su cuenta. Y nunca
/// frena a quien graba: si el codificador va por detras se salta ese fotograma en la vista
/// previa (en la cache, que es de donde se exporta, estan todos).
struct Previa {
    escritor: crate::encode::gst::Escritor,
    ruta: std::path::PathBuf,
    ancho: u32,
    alto: u32,
    /// El fotograma que espera a saber cuanto dura: hasta que llega el siguiente.
    pendiente: Option<(Vec<u8>, u64)>,
    /// Antes de este instante no se coge otro.
    siguiente_ms: u64,
}

impl Previa {
    fn abrir(dir: &std::path::Path, ancho: u32, alto: u32, fps: u32) -> Option<Self> {
        // Lo diminuto no lo acepta el codificador tal cual, y agrandarlo aqui no vale la
        // pena: se escribe al terminar, que es cuando se puede escalar con calma.
        if crate::encode::mp4::medida_que_acepta(ancho, alto) != (ancho, alto) {
            return None;
        }
        let (a, h) = crate::encode::escalar::medida_para(ancho, alto, ALTO_DE_LA_PREVIA).unwrap_or((ancho, alto));
        let (a, h) = crate::encode::mp4::medida_que_acepta(a / 2 * 2, h / 2 * 2);
        let fps = fps.clamp(1, FPS_DE_LA_PREVIA as u32);
        let opciones = crate::encode::mp4::Mp4Options { width: a, height: h, fps, quality: 75 };
        let medidas = crate::encode::gst::Medidas { ancho: a, alto: h, fps, bitrate: opciones.bitrate() };
        let ruta = dir.join("preview.mp4");
        match crate::encode::gst::Escritor::abrir(&ruta, &medidas, None) {
            Ok(escritor) => Some(Self { escritor, ruta, ancho: a, alto: h, pendiente: None, siguiente_ms: 0 }),
            Err(error) => {
                eprintln!("[grabar] sin vista previa en vivo ({ancho}x{alto}): {error}");
                None
            }
        }
    }

    /// Un fotograma de la grabacion, a su tamanno. Lo coge prestado y lo devuelve tal cual
    /// (asi no se copia entero), con si la vista previa sigue viva.
    fn fotograma(&mut self, rgba: Vec<u8>, ancho: u32, alto: u32, ts_ms: u64) -> (Vec<u8>, bool) {
        if ts_ms < self.siguiente_ms || !self.escritor.cabe() {
            return (rgba, true);
        }
        let Some(entero) = image::RgbaImage::from_raw(ancho, alto, rgba) else {
            return (Vec::new(), true);
        };
        let pequenno = crate::encode::escalar::a_medida(&entero, self.ancho, self.alto).into_raw();
        self.siguiente_ms = ts_ms + 1000 / FPS_DE_LA_PREVIA - 2;
        let viva = match self.pendiente.replace((pequenno, ts_ms)) {
            Some((anterior, desde)) => self.mandar(anterior, desde, ts_ms),
            None => true,
        };
        (entero.into_raw(), viva)
    }

    fn mandar(&mut self, rgba: Vec<u8>, desde_ms: u64, hasta_ms: u64) -> bool {
        match self.escritor.imagen(rgba, desde_ms, hasta_ms.saturating_sub(desde_ms).max(1)) {
            Ok(()) => true,
            Err(error) => {
                // La grabacion sigue: la cache es la fuente de verdad, y la vista previa se
                // escribe despues.
                eprintln!("[grabar] se ha caido la vista previa en vivo: {error}");
                false
            }
        }
    }

    /// Cierra el MP4 con el ultimo fotograma durando hasta `fin_ms`. Devuelve su ruta si ha
    /// quedado bien.
    fn cerrar(mut self, fin_ms: u64) -> Option<std::path::PathBuf> {
        if let Some((ultimo, desde)) = self.pendiente.take() {
            if !self.mandar(ultimo, desde, fin_ms) {
                let _ = std::fs::remove_file(&self.ruta);
                return None;
            }
        }
        match self.escritor.cerrar() {
            Ok(()) => Some(self.ruta),
            Err(error) => {
                eprintln!("[grabar] la vista previa en vivo no ha cerrado: {error}");
                let _ = std::fs::remove_file(&self.ruta);
                None
            }
        }
    }
}

pub(super) fn start(app: &AppHandle, region: Rect, options: RecordOptions) -> Result<SessionInfo> {
    let state = app.state::<AppState>();
    let Some(_arrancando) = Arrancando::tomar() else {
        return Err(AppError::Msg("ya hay una grabación arrancando".into()));
    };
    if state.is_recording() {
        return Err(AppError::Msg("ya hay una grabación en curso".into()));
    }

    windows_mgr::close_overlays(app);
    // Una miniatura que estuviera a la vista saldria en el video: aqui no se puede excluir.
    crate::miniatura::cerrar_todas(app);
    let fps = options.fps.clamp(5, 60);
    // Aqui se abre lo que haga falta, y en Wayland se espera a que la persona conteste al
    // escritorio: el reloj no empieza hasta despues.
    let opciones = record::linux::Opciones {
        ficha: app.path().app_config_dir().map_err(|e| AppError::Msg(e.to_string()))?.join("screencast-token"),
        al_cortarse: {
            let app = app.clone();
            // En otro hilo: parar espera a que acabe la captura, y esto lo llama la captura.
            Box::new(move || {
                let app = app.clone();
                std::thread::spawn(move || {
                    if let Err(error) = super::stop(&app) {
                        eprintln!("no se ha podido parar la grabación: {error}");
                    }
                });
            })
        },
    };
    let preparada = record::linux::preparar(region.to_even(), fps, opciones)?;
    let region = preparada.region();
    let raton_cocido = preparada.raton_cocido();

    let id = uuid::Uuid::new_v4().simple().to_string()[..10].to_string();
    let dir = state.session_dir(&id);
    std::fs::create_dir_all(&dir)?;
    // La webcam se enciende en cuanto se sabe que se va a grabar (en Wayland, cuando la
    // persona ya ha contestado al escritorio: antes tendria la luz encendida delante de una
    // pregunta), y lee y tira imagenes en su hilo hasta que se le manda grabar. Como en
    // Windows, la pantalla y el reloj no arrancan hasta que da su primera imagen, y como
    // mucho `TOPE_DE_LA_CAMARA`: si no la da para entonces se graba igual, y entra cuando
    // llegue. Que venga en `options` (un agente) manda sobre los ajustes.
    // La tecla de zoom sale de los ajustes y no de `options`, como en Windows: no es de cada
    // grabacion sino de quien graba.
    let (tecla_zoom, nombres_de_teclas, camara_pedida) = {
        let ajustes = state.settings.read();
        (
            record::teclas::TeclaZoom::de_atajo(&ajustes.zoom_shortcut),
            // La pastilla va dentro del video: se escribe ya en el idioma de la app.
            record::teclas::Nombres::de(ajustes.language),
            options.camara.unwrap_or(ajustes.record_camera).then(|| ajustes.camera_device.clone()),
        )
    };
    let encendida_en = Instant::now();
    let mut camara = camara_pedida.map(|dispositivo| record::camara::Encendida::encender(dispositivo, &dir));
    // La marimba de empezar, como en Windows: con la camara ya encendiendose y ANTES de abrir
    // nada que la pueda grabar (el sonido del sistema se graba de lo que sale por el altavoz, y
    // el microfono la oye). Si se graba sonido, esto espera a que acabe. Que venga en
    // `options` (un agente) manda sobre los ajustes.
    let antes_de_sonar = Instant::now();
    crate::platform::sonido::sonar_al_empezar(crate::platform::sonido::al_empezar(
        options.marimba.unwrap_or_else(|| state.settings.read().sonido_al_empezar),
        options.audio || options.microphone,
    ));
    // Lo que haya tardado la marimba ya es tiempo para que el overlay se vaya.
    std::thread::sleep(QUE_SE_VAYA_EL_OVERLAY.saturating_sub(antes_de_sonar.elapsed()));
    if let Some(camara) = camara.as_mut() {
        let estado = camara.esperar_imagen(super::falta_por_la_camara(encendida_en.elapsed()));
        eprintln!("[grabar] la cámara a los {} ms de encenderla: {estado:?}", encendida_en.elapsed().as_millis());
    }

    let session_seed = SessionData {
        id: id.clone(),
        dir: dir.clone(),
        region,
        fps,
        format: options.format.clone(),
        has_audio: false,
        width: region.width,
        height: region.height,
        // La vista previa en vivo se apunta al cerrarla, si ha salido bien.
        mp4_path: None,
        audio: None,
        clics: Vec::new(),
        teclas: Vec::new(),
        cursor: Vec::new(),
        // Como en Windows, el puntero no va cocido: se apunta y lo dibuja el exportador. Salvo
        // en los escritorios de Wayland que solo saben darlo ya dibujado en la imagen.
        cursor_capturado: raton_cocido,
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
    let sonido = record::linux::sonido::empezar(
        record::linux::sonido::Fuentes { sistema: options.audio, microfono: options.microphone },
        &dir,
        marcha.clone(),
        fin_ms.clone(),
    );
    let en_marcha = preparada.grabar(marcha, sender, reciclados.clone());
    // Los clics, los atajos, la tecla de zoom y los ratos de escritura los apunta el mismo
    // anotador que en Windows, en su hilo y con el mismo reloj. Solo en X11, que deja
    // preguntar por el raton y el teclado; en Wayland una app no puede, y la grabacion sale
    // sin ellos. El rastro del raton y su imagen siguen viniendo de quien captura.
    let (marcar_clics, marcar_teclas) = (options.highlight_clicks, options.highlight_keys);
    let muestreador = (!crate::platform::xdg::es_wayland()).then(|| {
        record::anotador::Muestreador::empezar(
            record::anotador::Anotador::new(region, marcar_clics, marcar_teclas)
                .con_tecla_zoom(tecla_zoom)
                .con_nombres(nombres_de_teclas),
            record::reloj::Reloj { start: reloj_inicio, paused_ms: paused_ms.clone() },
            pause.clone(),
        )
    });
    // La webcam, ya encendida, empieza a guardar en su archivo con el mismo reloj: lo que
    // saco antes del cero no entra (`camara_comun::Toma`).
    let camara = camara.map(|camara| {
        camara.grabar(record::reloj::Reloj { start: reloj_inicio, paused_ms: paused_ms.clone() }, pause.clone())
    });

    let writer_frames = frames_counter.clone();
    let writer_bytes = bytes_counter.clone();
    let writer_stop = stop.clone();
    let writer_fin = fin_ms.clone();
    let writer_descartar = descartar.clone();
    let writer = std::thread::spawn(move || -> Result<SessionData> {
        let mut session = session_seed;
        let (width, height) = (session.width, session.height);
        let mut cache = FrameCache::new(&session.dir)?;
        let mut previa = Previa::abrir(&session.dir, width, height, session.fps);
        // Los aros de los clics y la pastilla del atajo, si se pidieron al grabar, se pintan
        // en el fotograma que se guarda (y en la vista previa, que sale de el).
        let pintor = muestreador.as_ref().filter(|_| marcar_clics || marcar_teclas);
        let mut last_ts = 0u64;
        // El fin no puede depender de que el canal se cierre: con la pantalla quieta no
        // llegan fotogramas y el hilo se quedaria esperando.
        let mut parado = false;
        loop {
            // Al parar, la camara se avisa YA y no cuando este hilo acabe su cola de
            // fotogramas: mientras tanto seguiria grabando, con su luz encendida.
            if !parado && writer_stop.load(Ordering::Relaxed) {
                if let Some(camara) = camara.as_ref() {
                    camara.avisar();
                }
                parado = true;
            }
            let mut frame = match receiver.recv_timeout(Duration::from_millis(200)) {
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
            if let Some(pintor) = pintor {
                pintor.anotador().lock().pintar(&mut frame.rgba, width, height, frame.ts_ms);
            }
            if cache.push_rgba(&frame.rgba, width, height, frame.ts_ms)? {
                writer_frames.store(cache.frame_count() as u64, Ordering::Relaxed);
                writer_bytes.store(cache.bytes_written(), Ordering::Relaxed);
            }
            if let Some(p) = previa.as_mut() {
                let (devuelto, viva) = p.fotograma(frame.rgba, width, height, frame.ts_ms);
                frame.rgba = devuelto;
                if !viva {
                    previa = None;
                }
            }
            reciclados.devolver(frame.rgba);
        }
        // La camara va cerrando su archivo mientras aqui se cierra el resto.
        if let Some(camara) = camara.as_ref() {
            camara.avisar();
        }
        // Cuanto se grabo de verdad, y no el instante del ultimo fotograma: con el final
        // quieto no llega ninguno, y el video se cortaria donde llego el ultimo.
        let grabado_ms = writer_fin.load(Ordering::Relaxed).max(last_ts);
        session.mp4_path = previa.take().and_then(|p| p.cerrar(grabado_ms));
        session.frames = cache.finish(grabado_ms, session.fps)?;

        eprintln!("[grabar] {} fotogramas en {grabado_ms} ms", session.frames.len());
        // El sonido, si lo hubo: sin un byte no se dice que lo hay.
        if let Some(sonido) = sonido {
            (session.audio, session.pistas) = sonido.cerrar();
            session.has_audio = session.audio.is_some();
            match session.pistas.as_ref() {
                Some(pistas) => eprintln!(
                    "[grabar] sonido: sistema={} micrófono={}",
                    pistas.sistema.as_deref().unwrap_or("-"),
                    pistas.microfono.as_deref().unwrap_or("-")
                ),
                None => eprintln!("[grabar] sin sonido: no ha llegado nada de PipeWire"),
            }
        }
        if let Some(muestreador) = muestreador {
            let anotaciones = muestreador.terminar();
            session.clics = anotaciones.clics;
            session.teclas = anotaciones.teclas;
            session.marcas_zoom = anotaciones.marcas_zoom;
            session.escritura = anotaciones.escritura;
            eprintln!(
                "[grabar] anotado: {} clics, {} atajos, {} marcas de zoom, {} ratos de escritura",
                session.clics.len(),
                session.teclas.len(),
                session.marcas_zoom.len(),
                session.escritura.len()
            );
        }
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
        // La camara se espera aqui, antes de mirar si se descarta: si no, su hilo seguiria
        // escribiendo en una carpeta que se esta borrando.
        if let Some(pista) = camara.and_then(record::camara::Grabadora::terminar) {
            let primer_fotograma = session.frames.first().map_or(0, |f| f.timestamp_ms);
            session.camara = Some(crate::contrato::CamaraGrabada {
                archivo: pista.archivo,
                ancho: pista.ancho,
                alto: pista.alto,
                desfase_ms: record::camara::desfase(pista.primer_ms, primer_fotograma),
            });
            eprintln!("[grabar] cámara: {}x{}", pista.ancho, pista.alto);
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
        cabecera: Cabecera { format: options.format.clone(), audio: options.audio, microphone: options.microphone },
        descartar,
        barra: None,
        marco: None,
        writer: Some(writer),
    };
    *state.recording.lock() = Some(recording);
    crate::tray::al_cambiar_la_grabacion(app);

    // En Wayland una ventana no elige su sitio, asi que no se puede garantizar que la barra
    // quede fuera de lo que se graba: no hay barra.
    let barra = if crate::platform::xdg::es_wayland() { Ok(None) } else { windows_mgr::open_recorder_fuera(app, region) };
    match barra {
        Ok(Some(label)) => {
            if let Some(recording) = state.recording.lock().as_mut() {
                recording.barra = Some(label);
            }
        }
        Ok(None) => {
            // Como se para ya se dijo en la pantalla de elegir zona. Aqui no se ensenna nada:
            // saldria en el video.
            eprintln!(
                "[grabar] no hay barra (no se puede poner fuera de lo que se graba): se para con el atajo, con la orden --record, desde la bandeja o desde el aviso del escritorio"
            );
        }
        Err(error) => eprintln!("no se ha podido abrir la barra de grabacion: {error}"),
    }
    // El marco alrededor de lo que se graba, solo en X11: en Wayland una ventana no elige su
    // sitio, y el escritorio ya ensenna su aviso de que se graba. Va siempre por fuera de la
    // zona (aqui nada se puede sacar del video), y que no se pueda abrir no impide grabar.
    if !crate::platform::xdg::es_wayland() {
        match crate::platform::marco_x11::Marco::abrir(region, super::escala_del_marco(region)) {
            Ok(marco) => {
                if let Some(recording) = state.recording.lock().as_mut() {
                    recording.marco = Some(marco);
                }
            }
            Err(error) => eprintln!("[grabar] sin marco alrededor de la grabación: {error}"),
        }
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
        cursor_baked: raton_cocido,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::fondo_video::Lector;

    /// Dos segundos de una grabacion de 1280x720 a 60 por segundo, y medio segundo mas con la
    /// pantalla quieta: la vista previa sale encogida a 540 de alto, a 30 por segundo como
    /// mucho, y dura lo que duro la grabacion.
    #[test]
    fn la_vista_previa_en_vivo_sale_encogida_a_treinta_por_segundo_y_dura_lo_grabado() {
        let dir = std::env::temp_dir().join(format!("winshotx-previa-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (ancho, alto) = (1280u32, 720u32);
        let mut previa = Previa::abrir(&dir, ancho, alto, 60).expect("la vista previa no abre");
        for n in 0..120u64 {
            // Rojo el primer segundo y azul el segundo.
            let color = if n < 60 { [220u8, 30, 30, 255] } else { [30, 30, 220, 255] };
            let rgba = color.repeat((ancho * alto) as usize);
            let largo = rgba.len();
            let (devuelto, viva) = previa.fotograma(rgba, ancho, alto, n * 1000 / 60);
            assert!(viva, "la vista previa se ha caido en el fotograma {n}");
            assert_eq!(devuelto.len(), largo, "el fotograma no se devuelve entero");
        }
        let ruta = previa.cerrar(2500).expect("la vista previa no cierra");
        assert_eq!(ruta, dir.join("preview.mp4"));

        let mut lector = crate::encode::gst::LectorVideo::abrir(&ruta).expect("leer la vista previa");
        let mut cuadros = Vec::new();
        while let Some(c) = lector.siguiente().expect("fotograma") {
            assert_eq!(c.imagen.dimensions(), (960, 540));
            cuadros.push((c.desde_ms, c.dura_ms, c.imagen.get_pixel(480, 270).0));
        }
        // A 60 por segundo llegaron 120; se queda uno de cada dos, y alguno menos si el
        // codificador fue por detras.
        assert!((20..=62).contains(&cuadros.len()), "han salido {} fotogramas", cuadros.len());
        let (desde, dura, _) = cuadros[cuadros.len() - 1];
        assert!((2480..=2520).contains(&(desde + dura)), "acaba en {} y la grabacion en 2500", desde + dura);
        assert!(cuadros[0].2[0] > 150 && cuadros[0].2[2] < 100, "empieza en rojo: {:?}", cuadros[0]);
        let ultimo = cuadros[cuadros.len() - 1].2;
        assert!(ultimo[2] > 150 && ultimo[0] < 100, "acaba en azul: {ultimo:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lo_diminuto_no_tiene_vista_previa_en_vivo() {
        let dir = std::env::temp_dir().join(format!("winshotx-previa-chica-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(Previa::abrir(&dir, 96, 48, 30).is_none());
        assert!(!dir.join("preview.mp4").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
