//! Los ultimos segundos, siempre grabados.
//!
//! Lo bueno de una pantalla casi siempre pasa **antes** de que a nadie se le ocurra
//! grabarla: el error que salio y ya no vuelve, lo que acaba de hacer el programa, la
//! jugada. Esto graba en un anillo continuo y tira lo viejo, y con una tecla se queda con
//! lo ultimo que paso.
//!
//! El anillo y su formato viven en `record::buffer`. Aqui esta lo que lo alimenta: la
//! captura de la pantalla, el sonido, las anotaciones, y el trabajo de cosechar cuando se
//! pulsa la tecla.
//!
//! **Cosechar no para el anillo.** Se hace en otro hilo, sobre una copia de la lista de
//! trozos, y mientras tanto se sigue grabando: lo normal es que despues de guardar un
//! momento siga pasando algo que tambien se quiera.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::capture::{MonitorInfo, Rect};
use crate::error::{AppError, Result};
use crate::record::buffer::{Anillo, AnilloAudio, Copia, Segmento};
use crate::record::{self, AudioInfo, SessionData};
use crate::recorder::SessionInfo;
use crate::state::AppState;
use crate::windows_mgr;

/// Cambios de estado del anillo, para que los ajustes y la bandeja digan la verdad.
pub const EVENT_REPLAY: &str = "winshotx://replay";

/// Lo que se puede pedir: nunca menos de quince segundos ni mas de dos minutos.
///
/// Por arriba manda el disco: a pantalla completa, dos minutos del peor caso medido son
/// mas de un gigabyte dando vueltas. Por abajo manda para que sirva: menos de quince
/// segundos no llega a coger lo que acaba de pasar, porque quien lo ve tarda en reaccionar.
pub const SEGUNDOS_MIN: u32 = 15;
pub const SEGUNDOS_MAX: u32 = 120;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayStatus {
    pub running: bool,
    pub seconds: u32,
    /// Que pantalla se esta vigilando, empezando en 1, y como se llama.
    pub screen: u32,
    pub screen_label: String,
    /// Lo que ocupa ahora mismo el anillo en disco.
    pub bytes: u64,
    /// Cuanto lleva grabado. Hasta que no llega a la ventana entera, lo que se guarde
    /// durara menos de lo que pone el ajuste, y la gente tiene derecho a saberlo.
    pub buffered_ms: u64,
}

impl ReplayStatus {
    pub fn parado() -> Self {
        Self {
            running: false,
            seconds: 0,
            screen: 0,
            screen_label: String::new(),
            bytes: 0,
            buffered_ms: 0,
        }
    }
}

/// Lo que se le manda al hilo del anillo.
enum Orden {
    /// Quedarse con lo ultimo que paso.
    Cosechar,
}

/// Todo lo que hay que compartir con el hilo del anillo mientras corre.
pub struct ReplayState {
    pub seconds: u32,
    pub monitor: MonitorInfo,
    stop: Arc<AtomicBool>,
    ordenes: Sender<Orden>,
    bytes: Arc<AtomicU64>,
    grabado_ms: Arc<AtomicU64>,
    #[cfg(windows)]
    control: Option<crate::record::win::Control>,
    hilo: Option<JoinHandle<()>>,
}

impl ReplayState {
    pub fn status(&self) -> ReplayStatus {
        ReplayStatus {
            running: true,
            seconds: self.seconds,
            screen: self.monitor.id + 1,
            screen_label: self.monitor.label.clone(),
            bytes: self.bytes.load(Ordering::Relaxed),
            buffered_ms: self.grabado_ms.load(Ordering::Relaxed),
        }
    }
}

/// El estado de ahora mismo, corra o no corra.
pub fn status(app: &AppHandle) -> ReplayStatus {
    app.state::<AppState>()
        .replay
        .lock()
        .as_ref()
        .map(ReplayState::status)
        .unwrap_or_else(ReplayStatus::parado)
}

fn avisar(app: &AppHandle) {
    let _ = app.emit(EVENT_REPLAY, status(app));
}

/// Lo mismo, mas la entrada de la bandeja, que aparece y desaparece con el anillo.
///
/// Va aparte de `avisar` porque rehacer un menu de Windows toca ventanas, y eso solo se
/// hace desde donde se enciende y se apaga, nunca desde el hilo que esta cosiendo.
fn avisar_y_rehacer_menu(app: &AppHandle) {
    avisar(app);
    if let Err(error) = crate::tray::rehacer_menu(app) {
        eprintln!("[replay] no se ha podido rehacer el menú de la bandeja: {error}");
    }
}

/// La pantalla que se haya pedido, y si no la que tenga el raton al encenderlo.
///
/// El anillo vigila UNA pantalla y no puede cambiar de opinion a mitad: mudarse se
/// llevaria por delante todo lo grabado, que es justo lo que se esta guardando. Por eso se
/// elige al encender y se dice cual es.
///
/// Y si la pantalla pedida ya no esta (se desenchufo el monitor desde la ultima vez), se
/// coge la del raton en vez de fallar: quedarse sin la funcion por mover un cable seria
/// una forma tonta de perderla.
#[cfg(windows)]
fn pantalla_elegida(pedida: Option<u32>) -> Result<MonitorInfo> {
    let monitores = crate::capture::monitors()?;
    let elegida = pedida
        .and_then(|id| monitores.iter().find(|m| m.id == id).cloned())
        .or_else(|| {
            record::raton::cursor()
                .and_then(|(x, y)| monitores.iter().find(|m| m.contains(x, y)).cloned())
        })
        .or_else(|| monitores.iter().find(|m| m.is_primary).cloned())
        .or_else(|| monitores.first().cloned());
    elegida.ok_or_else(|| AppError::Msg("no se ha detectado ningún monitor".into()))
}

/// Enciende el anillo sobre la pantalla donde este el raton.
#[cfg(windows)]
pub fn start(app: &AppHandle) -> Result<ReplayStatus> {
    use std::sync::mpsc::channel;

    use crate::record::win::{self, CaptureFlags};

    let state = app.state::<AppState>();
    if state.replay.lock().is_some() {
        return Ok(status(app));
    }

    let (segundos, fps, pedida, quiere_audio, quiere_micro, con_cursor) = {
        let settings = state.settings.read();
        (
            settings.replay_seconds.clamp(SEGUNDOS_MIN, SEGUNDOS_MAX),
            // El ritmo del anillo va aparte del de la grabacion normal: aqui se graba todo
            // el rato, asi que lo que cuesta cada fotograma se paga toda la tarde.
            settings.replay_fps.clamp(5, 60),
            settings.replay_screen,
            settings.record_audio,
            settings.record_microphone,
            settings.capture_cursor,
        )
    };
    let monitor = pantalla_elegida(pedida)?;
    let region = Rect {
        x: monitor.x,
        y: monitor.y,
        width: monitor.width,
        height: monitor.height,
    }
    .to_even();

    // El anillo vive en su propia carpeta y se borra entera al apagarlo. No va dentro de
    // `sessions`: eso son grabaciones que alguien puede querer, y esto es material que se
    // tira solo.
    //
    // Y cada anillo estrena carpeta, con su nombre al azar. Apagar y encender seguido
    // (que es justo lo que hace cambiar los segundos) deja al hilo viejo terminando de
    // limpiar mientras el nuevo ya esta escribiendo: con una carpeta fija, el que se iba
    // le borraba los archivos al que acababa de llegar.
    let dir = state
        .temp_root
        .join("replay")
        .join(uuid::Uuid::new_v4().simple().to_string()[..8].to_string());

    let audio = {
        let fuentes = record::audio::Fuentes {
            sistema: quiere_audio,
            microfono: quiere_micro,
        };
        if fuentes.ninguna() {
            None
        } else {
            match record::audio::empezar(fuentes) {
                Ok(captura) => Some(captura),
                Err(error) => {
                    eprintln!("[replay] sin sonido: {error}");
                    None
                }
            }
        }
    };

    let (sender, receiver) = channel::<win::CapturedFrame>();
    let (ordenes, buzon) = channel::<Orden>();
    let stop = Arc::new(AtomicBool::new(false));
    let bytes = Arc::new(AtomicU64::new(0));
    let grabado_ms = Arc::new(AtomicU64::new(0));

    let hilo = {
        let app = app.clone();
        let stop = stop.clone();
        let bytes = bytes.clone();
        let grabado_ms = grabado_ms.clone();
        std::thread::spawn(move || {
            let ventana_ms = u64::from(segundos) * 1000;
            let anillo = match Anillo::nuevo(
                &dir,
                ventana_ms,
                fps,
                crate::record::buffer::bytes_max(segundos, fps),
            ) {
                Ok(anillo) => anillo,
                Err(error) => {
                    eprintln!("[replay] no se ha podido abrir el anillo: {error}");
                    return;
                }
            };
            let mut cocina = Cocina {
                app,
                anillo,
                audio_anillo: audio.as_ref().map(|captura| {
                    let info = AudioInfo {
                        channels: captura.formato.canales,
                        sample_rate: captura.formato.muestras_por_segundo,
                    };
                    (info, AnilloAudio::nuevo(info.bytes_por_ms(), info.channels, ventana_ms))
                }),
                clics: Vec::new(),
                teclas: Vec::new(),
                cursor: Vec::new(),
                vigilante: record::raton::Vigilante::default(),
                teclado: record::teclas::Vigilante::default(),
                region,
                fps,
                ventana_ms,
                con_cursor,
                desde_la_limpieza: 0,
            };
            cocina.trabajar(&receiver, &buzon, &stop, &bytes, &grabado_ms, audio.as_ref());
            if let Some(captura) = audio {
                captura.parar();
            }
            cocina.anillo.limpiar();
        })
    };

    let control = win::start(
        region,
        (monitor.x, monitor.y),
        con_cursor,
        fps,
        CaptureFlags {
            sender,
            crop: (0, 0, 0, 0),
            stop: stop.clone(),
            // El anillo no se pausa: o graba o esta apagado. Una pausa aqui seria un
            // agujero en el tiempo justo en el trozo que alguien va a querer.
            pause: Arc::new(AtomicBool::new(false)),
            paused_ms: Arc::new(AtomicU64::new(0)),
            min_interval_ms: 0,
        },
    )?;

    *state.replay.lock() = Some(ReplayState {
        seconds: segundos,
        monitor,
        stop,
        ordenes,
        bytes,
        grabado_ms,
        control: Some(control),
        hilo: Some(hilo),
    });
    avisar_y_rehacer_menu(app);
    Ok(status(app))
}

#[cfg(not(windows))]
pub fn start(_app: &AppHandle) -> Result<ReplayStatus> {
    Err(AppError::Unsupported)
}

/// Apaga el anillo y borra lo que tenia grabado.
pub fn stop(app: &AppHandle) -> Result<ReplayStatus> {
    let state = app.state::<AppState>();
    let Some(mut replay) = state.replay.lock().take() else {
        return Ok(ReplayStatus::parado());
    };
    replay.stop.store(true, Ordering::Relaxed);

    // Parar la captura puede tardar en devolver el control, asi que se hace aparte: nadie
    // tiene que esperar mirando a un interruptor que no se apaga.
    #[cfg(windows)]
    if let Some(control) = replay.control.take() {
        std::thread::spawn(move || {
            let _ = control.stop();
        });
    }
    if let Some(hilo) = replay.hilo.take() {
        std::thread::spawn(move || {
            let _ = hilo.join();
        });
    }
    avisar_y_rehacer_menu(app);
    Ok(ReplayStatus::parado())
}

/// Quedarse con lo ultimo que paso. Vuelve enseguida: el trabajo lo hace el hilo del
/// anillo, que es el unico que sabe lo que hay grabado.
pub fn save(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let guard = state.replay.lock();
    let replay = guard
        .as_ref()
        .ok_or_else(|| AppError::Msg("los últimos segundos no se están grabando".into()))?;
    replay
        .ordenes
        .send(Orden::Cosechar)
        .map_err(|_| AppError::Msg("el anillo se ha parado".into()))
}

/// El hilo que alimenta el anillo y lo cosecha.
struct Cocina {
    app: AppHandle,
    anillo: Anillo,
    audio_anillo: Option<(AudioInfo, AnilloAudio)>,
    clics: Vec<crate::encode::zoom::Clic>,
    teclas: Vec<record::teclas::Atajo>,
    cursor: Vec<(u64, i32, i32)>,
    vigilante: record::raton::Vigilante,
    teclado: record::teclas::Vigilante,
    region: Rect,
    fps: u32,
    ventana_ms: u64,
    con_cursor: bool,
    desde_la_limpieza: u32,
}

impl Cocina {
    #[cfg(windows)]
    fn trabajar(
        &mut self,
        receiver: &Receiver<crate::record::win::CapturedFrame>,
        buzon: &Receiver<Orden>,
        stop: &Arc<AtomicBool>,
        bytes: &Arc<AtomicU64>,
        grabado_ms: &Arc<AtomicU64>,
        audio: Option<&record::audio::Captura>,
    ) {
        use std::time::Duration;

        let arranque = Instant::now();
        let mut ultimo_ts = 0u64;
        let mut perdidos = 0u64;
        let mut avisado = 0u64;
        loop {
            // Las ordenes se atienden ANTES de esperar fotograma: con la pantalla quieta
            // no llega ninguno, y quien pulsa la tecla no puede quedarse esperando a que
            // algo se mueva para que le guarden lo que ya paso.
            // Varias pulsaciones seguidas son UNA cosecha, no cinco editores abiertos: el
            // buzon se vacia entero y despues se guarda una vez.
            let mut piden = false;
            while let Ok(Orden::Cosechar) = buzon.try_recv() {
                piden = true;
            }
            if piden {
                let ahora = arranque.elapsed().as_millis() as u64;
                if let Err(error) = self.cosechar(ahora.max(ultimo_ts)) {
                    // Aqui NO se dice que el anillo se ha parado, porque no se ha parado:
                    // sigue grabando y la siguiente pulsacion puede ir bien. Lo que falla
                    // casi siempre es pulsar en los dos primeros segundos, cuando todavia
                    // no hay nada dentro, y eso la fila de ajustes ya lo cuenta.
                    eprintln!("[replay] no se ha podido guardar: {error}");
                }
            }
            if stop.load(Ordering::Relaxed) {
                break;
            }

            match receiver.recv_timeout(Duration::from_millis(150)) {
                Ok(mut frame) => {
                    // Si mientras se guardaba el anterior han llegado varios, se atiende
                    // SOLO el ultimo y los de en medio se tiran. Esto corre durante horas:
                    // si el disco o la maquina no dan para el ritmo pedido, la cola crece
                    // sola y cada fotograma sin comprimir son ocho megabytes de memoria.
                    // Mejor grabar a menos fotogramas por segundo que llenarle la RAM a
                    // alguien que ni sabe que esto esta puesto.
                    let mut tirados = 0u32;
                    while let Ok(siguiente) = receiver.try_recv() {
                        frame = siguiente;
                        tirados += 1;
                    }
                    perdidos += u64::from(tirados);
                    ultimo_ts = frame.ts_ms;
                    self.tragar(frame);
                    bytes.store(self.anillo.bytes(), Ordering::Relaxed);
                    grabado_ms.store(self.anillo.guardado_ms(ultimo_ts), Ordering::Relaxed);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
            // Se avisa de vez en cuando, no en cada fotograma tirado: si la maquina no da
            // abasto, escribir una linea por cada uno seria parte del problema.
            if perdidos >= avisado + 600 {
                avisado = perdidos;
                eprintln!("[replay] {perdidos} fotogramas tirados por no dar abasto");
            }

            if let Some(captura) = audio {
                self.tragar_sonido(captura);
            }
        }
    }

    #[cfg(not(windows))]
    fn trabajar(
        &mut self,
        _receiver: &Receiver<()>,
        _buzon: &Receiver<Orden>,
        _stop: &Arc<AtomicBool>,
        _bytes: &Arc<AtomicU64>,
        _grabado_ms: &Arc<AtomicU64>,
        _audio: Option<&()>,
    ) {
    }

    #[cfg(windows)]
    fn tragar(&mut self, frame: crate::record::win::CapturedFrame) {
        let rgba = crate::recorder::bgra_a_rgba(&frame.bgra);
        // Se anota lo mismo que en una grabacion normal (donde se pulso, que atajo y donde
        // estaba el raton) para que el editor pueda acercar la camara y dibujar el puntero
        // despues. Son doce bytes por clic: no se decide antes lo que se va a querer luego.
        if let Some((cx, cy)) = record::raton::cursor() {
            self.cursor
                .push((frame.ts_ms, cx - self.region.x, cy - self.region.y));
        }
        if let Some(clic) = self.vigilante.mirar(frame.ts_ms) {
            self.clics.push(crate::encode::zoom::Clic {
                ms: clic.ms,
                x: clic.x - self.region.x,
                y: clic.y - self.region.y,
                derecho: clic.derecho,
            });
        }
        if let Some(atajo) = self.teclado.mirar(frame.ts_ms) {
            self.teclas.push(record::teclas::Atajo {
                x: atajo.x - self.region.x,
                y: atajo.y - self.region.y,
                ..atajo
            });
        }

        if let Err(error) = self
            .anillo
            .empujar(&rgba, self.region.width, self.region.height, frame.ts_ms)
        {
            eprintln!("[replay] fotograma perdido: {error}");
        }

        // Las anotaciones tambien se tiran cuando se salen de la ventana: un anillo que
        // corre toda la tarde no puede ir acumulando el rastro del raton de toda la tarde.
        self.desde_la_limpieza += 1;
        if self.desde_la_limpieza >= 60 {
            self.desde_la_limpieza = 0;
            let corte = frame
                .ts_ms
                .saturating_sub(self.ventana_ms + crate::record::buffer::SEGMENTO_MS);
            self.cursor.retain(|(ms, _, _)| *ms >= corte);
            self.clics.retain(|clic| clic.ms >= corte);
            self.teclas.retain(|atajo| atajo.ms >= corte);
        }
    }

    #[cfg(windows)]
    fn tragar_sonido(&mut self, captura: &record::audio::Captura) {
        let Some((_, anillo)) = self.audio_anillo.as_mut() else {
            return;
        };
        while let Ok(trozo) = captura.trozos.try_recv() {
            anillo.empujar(&record::audio::a_pcm16(&trozo.datos));
        }
    }

    /// Se queda con lo ultimo que paso y abre el editor, sin dejar de grabar.
    fn cosechar(&mut self, ahora_ms: u64) -> Result<()> {
        let (segmentos, corte, copia) = self.anillo.instantanea(ahora_ms)?;
        let sonido = self
            .audio_anillo
            .as_ref()
            .map(|(info, anillo)| (*info, anillo.ultimos(self.ventana_ms + 2_000)));

        let encargo = Encargo {
            segmentos,
            corte,
            ahora: ahora_ms,
            copia,
            sonido,
            clics: self.clics.clone(),
            teclas: self.teclas.clone(),
            cursor: self.cursor.clone(),
            region: self.region,
            fps: self.fps,
            con_cursor: self.con_cursor,
        };
        // Coser treinta segundos y sacar sus miniaturas tarda lo suyo, y mientras tanto el
        // anillo tiene que seguir tragando fotogramas: si se parara, el hueco caeria justo
        // en los segundos siguientes a lo que alguien acaba de guardar.
        let app = self.app.clone();
        std::thread::spawn(move || {
            if let Err(error) = servir(&app, encargo) {
                eprintln!("[replay] no se ha podido montar la sesión: {error}");
            }
        });
        Ok(())
    }
}

/// Lo que se lleva el hilo que cose, para no tocar nada del anillo desde fuera.
struct Encargo {
    segmentos: Vec<Segmento>,
    corte: u64,
    /// El instante en el que se pulso la tecla. Es el final de lo que se guarda, y lo que
    /// le da su duracion al ultimo fotograma cuando la pantalla llevaba un rato quieta.
    ahora: u64,
    /// Mientras esto viva, el anillo no borra los archivos que se estan copiando.
    #[allow(dead_code)]
    copia: Copia,
    sonido: Option<(AudioInfo, Vec<u8>)>,
    clics: Vec<crate::encode::zoom::Clic>,
    teclas: Vec<record::teclas::Atajo>,
    cursor: Vec<(u64, i32, i32)>,
    region: Rect,
    fps: u32,
    con_cursor: bool,
}

/// Cose lo grabado en una sesion normal y abre el editor con ella.
fn servir(app: &AppHandle, encargo: Encargo) -> Result<SessionInfo> {
    let state = app.state::<AppState>();
    let id = uuid::Uuid::new_v4().simple().to_string()[..10].to_string();
    let session = montar(state.session_dir(&id), id.clone(), encargo)?;

    let info = SessionInfo::from(&session);
    state.sessions.write().insert(id.clone(), session);

    // El editor se abre SIEMPRE, aunque el ajuste de «abrir el editor al terminar» esté
    // apagado. Ahí se decide qué pasa al parar una grabación, que deja un archivo hecho;
    // aquí no hay archivo ninguno todavía, así que no abrirlo sería quedarse con lo último
    // que pasó en un sitio al que nadie puede llegar. Y es además el único aviso de que la
    // tecla ha hecho algo.
    let handle = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = windows_mgr::open_editor(&handle, &id) {
            eprintln!("no se ha podido abrir el editor: {error}");
        }
    });
    avisar(app);
    Ok(info)
}

/// Todo el trabajo de convertir el anillo en una sesion: coser los fotogramas, cortar el
/// sonido, poner los relojes a cero y sacar las miniaturas.
///
/// Va aparte de `servir` para poder probarlo sin una aplicacion de Tauri delante. Lo que
/// hay que comprobar de esto no es que se abra una ventana, es que el archivo que sale se
/// pueda volver a dibujar: la leccion del audio, que salio mudo con las dos puntas verdes.
fn montar(dir: std::path::PathBuf, id: String, encargo: Encargo) -> Result<SessionData> {
    std::fs::create_dir_all(&dir)?;

    let (frames, t0) = record::buffer::ensamblar(
        &encargo.segmentos,
        encargo.corte,
        encargo.ahora,
        encargo.fps,
        &dir.join("frames.bin"),
    )?;
    let duracion = frames
        .last()
        .map(|f| f.timestamp_ms + u64::from(f.duration_ms))
        .unwrap_or(0);

    // El sonido se corta por el FINAL, no por el principio: el anillo y los fotogramas
    // acaban en el mismo instante (ahora), asi que quedarse con los ultimos milisegundos
    // de sonido es quedarse justo con los que acompanan a las imagenes que se guardan.
    let audio = match encargo.sonido {
        Some((info, pcm)) if !pcm.is_empty() => {
            let quiero = (duracion * info.bytes_por_ms()) as usize;
            let bloque = usize::from(info.channels) * 2;
            let desde = pcm.len().saturating_sub(quiero) / bloque * bloque;
            std::fs::write(dir.join("audio.pcm"), &pcm[desde..])?;
            Some(info)
        }
        _ => None,
    };

    let mut session = SessionData {
        id,
        dir,
        region: encargo.region,
        fps: encargo.fps,
        format: "mp4".into(),
        has_audio: audio.is_some(),
        width: encargo.region.width,
        height: encargo.region.height,
        // El anillo no escribe MP4 de vista previa: seria codificar sin parar durante
        // horas algo que casi siempre se tira. El editor reproduce por fotogramas.
        mp4_path: None,
        audio,
        clics: recortar(encargo.clics, t0, |c| c.ms, |c, ms| c.ms = ms),
        teclas: recortar(encargo.teclas, t0, |a| a.ms, |a, ms| a.ms = ms),
        cursor: encargo
            .cursor
            .into_iter()
            .filter(|(ms, _, _)| *ms >= t0)
            .map(|(ms, x, y)| (ms - t0, x, y))
            .collect(),
        cursor_capturado: encargo.con_cursor,
        frames,
    };
    record::generate_thumbnails(&mut session)?;
    session.persist()?;
    Ok(session)
}

/// Se queda con lo que cae dentro de lo guardado y le pone el reloj a cero.
fn recortar<T>(
    anotaciones: Vec<T>,
    t0: u64,
    leer: impl Fn(&T) -> u64,
    escribir: impl Fn(&mut T, u64),
) -> Vec<T> {
    anotaciones
        .into_iter()
        .filter(|a| leer(a) >= t0)
        .map(|mut a| {
            let ms = leer(&a) - t0;
            escribir(&mut a, ms);
            a
        })
        .collect()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::record::buffer::Anillo;

    /// Una pantalla de mentira con algo que se mueve, para que cada fotograma sea distinto
    /// del anterior y el anillo no los descarte por repetidos.
    fn pantalla(ancho: u32, alto: u32, paso: u32) -> Vec<u8> {
        let mut frame = vec![20u8; (ancho * alto) as usize * 4];
        for (i, byte) in frame.iter_mut().enumerate() {
            if i % 4 == 3 {
                *byte = 255;
            }
        }
        let x0 = (paso * 2) % (ancho - 8);
        for y in 2..10u32 {
            for x in x0..x0 + 8 {
                let p = ((y * ancho + x) * 4) as usize;
                frame[p] = 240;
                frame[p + 1] = 60;
                frame[p + 2] = 10;
            }
        }
        frame
    }

    /// **El camino entero, sin la ventana delante.**
    ///
    /// Se graba en el anillo, se pulsa la tecla, y lo que sale tiene que ser una sesion
    /// que el editor pueda abrir de verdad: con sus miniaturas en el disco, con el sonido
    /// recortado a lo que dura, y con fotogramas que se vuelven a dibujar. Que el anillo
    /// tenga archivos no dice nada; lo que cuenta es lo que acaba en manos de quien pulso.
    #[test]
    fn lo_que_se_pulsa_acaba_en_una_sesion_que_se_puede_abrir() {
        let (ancho, alto) = (48u32, 32u32);
        let raiz = std::env::temp_dir().join("winshotx-replay-camino");
        let _ = std::fs::remove_dir_all(&raiz);
        let mut anillo =
            Anillo::nuevo(&raiz.join("anillo"), 10_000, 30, crate::record::buffer::bytes_max(10, 30))
                .unwrap();

        // Veinte segundos grabando, con un clic por el medio y el raton paseando.
        for paso in 0..80u32 {
            let ts = u64::from(paso) * 250;
            anillo
                .empujar(&pantalla(ancho, alto, paso), ancho, alto, ts)
                .unwrap();
        }
        let (segmentos, corte, copia) = anillo.instantanea(20_000).unwrap();

        // Un sonido de mentira: un byte por cada milesima, con el formato mas comun.
        let info = AudioInfo {
            channels: 2,
            sample_rate: 48_000,
        };
        let pcm = vec![7u8; (25_000 * info.bytes_por_ms()) as usize];

        let encargo = Encargo {
            segmentos,
            corte,
            ahora: 20_000,
            copia,
            sonido: Some((info, pcm)),
            // Uno dentro de la ventana y otro que se quedo fuera: el de fuera no puede
            // colarse, porque el zoom se acercaria a algo que no se ve en el video.
            clics: vec![
                crate::encode::zoom::Clic { ms: 2_000, x: 5, y: 5, derecho: false },
                crate::encode::zoom::Clic { ms: 15_000, x: 20, y: 10, derecho: false },
            ],
            teclas: Vec::new(),
            cursor: vec![(1_000, 1, 1), (16_000, 30, 20)],
            region: Rect { x: 0, y: 0, width: ancho, height: alto },
            fps: 30,
            con_cursor: false,
        };

        let session = montar(raiz.join("sesion"), "prueba".into(), encargo).unwrap();

        // Dura lo que se pidio, con dos margenes que son de verdad: por arriba, lo que
        // sobra de empezar en un fotograma entero; por abajo, que el ultimo fotograma es
        // de hace un momento y no de este instante, porque una pantalla no cambia en el
        // milisegundo en el que a alguien le da por pulsar la tecla.
        let duracion = session.duration_ms();
        assert!(
            (9_700..=11_500).contains(&duracion),
            "la sesión dura {duracion} ms y tenía que durar diez segundos"
        );

        // Los relojes empiezan en cero y lo de antes de la ventana se quedo fuera.
        assert_eq!(session.frames[0].timestamp_ms, 0);
        assert_eq!(session.clics.len(), 1, "el clic de hace quince segundos no entra");
        assert!(session.clics[0].ms < duracion, "el clic tiene que caer dentro del vídeo");
        assert_eq!(session.cursor.len(), 1);

        // El sonido se corta a lo que dura la imagen, no a lo que hubiera guardado.
        let sonido = std::fs::metadata(session.audio_path()).unwrap().len();
        let esperado = duracion * info.bytes_por_ms();
        assert!(
            sonido.abs_diff(esperado) < info.bytes_por_ms() * 100,
            "el sonido son {sonido} bytes y la imagen pide {esperado}"
        );

        // Y lo que de verdad importa: que las imagenes se puedan volver a dibujar. Una
        // sesion con fotogramas rotos abre el editor y ensenna un rectangulo negro.
        for indice in [0, session.frames.len() / 2, session.frames.len() - 1] {
            let imagen = record::read_frame(&session, indice).unwrap();
            assert_eq!((imagen.width(), imagen.height()), (ancho, alto));
        }
        // Con sus miniaturas ya en el disco: es lo que pinta la tira de tiempo.
        assert!(
            std::path::Path::new(&session.frames[0].thumb_path).exists(),
            "la miniatura del primer fotograma no llego al disco"
        );

        anillo.limpiar();
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

/// Lo unico que no se puede comprobar con fotogramas de mentira: que la captura continua de
/// Windows alimente el anillo de verdad, con una pantalla que existe.
///
/// Se corre a mano, porque necesita una pantalla:
/// `cargo test --lib el_anillo_traga_la_pantalla_de_verdad -- --ignored --nocapture`
#[cfg(all(test, windows))]
mod pruebas_con_pantalla {
    use super::*;
    use crate::record::buffer::Anillo;
    use crate::record::win::{self, CaptureFlags};
    use std::time::Duration;

    #[test]
    #[ignore = "necesita una pantalla de verdad"]
    fn el_anillo_traga_la_pantalla_de_verdad() {
        let monitor = pantalla_elegida(None).expect("no hay ninguna pantalla");
        let region = Rect {
            x: monitor.x,
            y: monitor.y,
            width: monitor.width,
            height: monitor.height,
        }
        .to_even();
        println!(
            "vigilando «{}», {}x{} en ({}, {})",
            monitor.label, region.width, region.height, region.x, region.y
        );

        let raiz = std::env::temp_dir().join("winshotx-replay-pantalla");
        let _ = std::fs::remove_dir_all(&raiz);
        let fps = crate::record::buffer::FPS_ANILLO;
        let mut anillo = Anillo::nuevo(
            &raiz.join("anillo"),
            3_000,
            fps,
            crate::record::buffer::bytes_max(3, fps),
        )
        .unwrap();

        let (sender, receiver) = std::sync::mpsc::channel::<win::CapturedFrame>();
        let stop = Arc::new(AtomicBool::new(false));
        let control = win::start(
            region,
            (monitor.x, monitor.y),
            false,
            fps,
            CaptureFlags {
                sender,
                crop: (0, 0, 0, 0),
                stop: stop.clone(),
                pause: Arc::new(AtomicBool::new(false)),
                paused_ms: Arc::new(AtomicU64::new(0)),
                min_interval_ms: 0,
            },
        )
        .expect("no ha arrancado la captura");

        // Seis segundos con una ventana de tres: al final tiene que haber tirado lo viejo.
        let arranque = Instant::now();
        let mut ultimo_ts = 0;
        let mut empujados = 0u32;
        while arranque.elapsed() < Duration::from_secs(6) {
            match receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) => {
                    ultimo_ts = frame.ts_ms;
                    let rgba = crate::recorder::bgra_a_rgba(&frame.bgra);
                    if anillo
                        .empujar(&rgba, region.width, region.height, frame.ts_ms)
                        .unwrap()
                    {
                        empujados += 1;
                    }
                }
                Err(_) => continue,
            }
        }
        stop.store(true, Ordering::Relaxed);
        let _ = control.stop();
        // El numero que de verdad importa de esta funcion: lo que le escribe al disco por
        // segundo mientras esta puesta. Con una partida a pantalla completa delante es el
        // peor caso que se puede medir en esta maquina.
        println!(
            "anillo: {} MB en disco, {empujados} fotogramas en seis segundos, {} MB/s",
            anillo.bytes() / 1024 / 1024,
            anillo.bytes() / 1024 / 1024 / 6
        );
        assert!(empujados > 0, "la captura no ha entregado ni un fotograma");

        let ahora = arranque.elapsed().as_millis() as u64;
        let (segmentos, corte, copia) = anillo.instantanea(ahora.max(ultimo_ts)).unwrap();
        let session = montar(
            raiz.join("sesion"),
            "pantalla".into(),
            Encargo {
                segmentos,
                corte,
                ahora,
                copia,
                sonido: None,
                clics: Vec::new(),
                teclas: Vec::new(),
                cursor: Vec::new(),
                region,
                fps,
                con_cursor: false,
            },
        )
        .expect("no se ha podido montar la sesión");

        println!(
            "guardados {} fotogramas, {} ms",
            session.frames.len(),
            session.duration_ms()
        );
        // Aqui NO se exige un monton de fotogramas: si nadie toca el escritorio mientras
        // corre la prueba, la pantalla no cambia y un solo fotograma es la respuesta
        // correcta. Lo que si tiene que cuadrar es el tiempo.
        assert!(!session.frames.is_empty(), "no ha entrado ningún fotograma");
        assert!(
            (2_800..=3_300).contains(&session.duration_ms()),
            "se ha guardado {} ms y la ventana eran tres segundos",
            session.duration_ms()
        );

        // Y que lo guardado sea una PANTALLA y no un rectangulo negro: un escritorio real
        // tiene muchos colores distintos. Es la unica forma de ver, sin mirar, que lo que
        // se ha grabado es lo que hay delante.
        let imagen = record::read_frame(&session, session.frames.len() - 1).unwrap();
        let mut colores = std::collections::HashSet::new();
        for pixel in imagen.pixels().step_by(37) {
            colores.insert(pixel.0);
        }
        println!("{} colores distintos en el último fotograma", colores.len());
        assert!(colores.len() > 20, "el fotograma parece vacío");

        let copia = raiz.join("ultimo.png");
        imagen.save(&copia).unwrap();
        println!("para mirarlo con los ojos: {}", copia.display());

        anillo.limpiar();
    }
}
