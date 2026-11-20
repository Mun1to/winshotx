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
    /// Y lo que le escribe al disco por segundo, que es lo que de verdad cuesta tenerlo
    /// puesto: los megabytes guardados se quedan igual una vez lleno el anillo, pero esto
    /// sigue pasando toda la tarde.
    pub bytes_per_second: u64,
    /// A que tamanno esta grabando, ya con la calidad elegida aplicada.
    pub width: u32,
    pub height: u32,
    /// Cuanto lleva grabado. Hasta que no llega a la ventana entera, lo que se guarde
    /// durara menos de lo que pone el ajuste, y la gente tiene derecho a saberlo.
    pub buffered_ms: u64,
    /// Por que esta apagado, si no lo apago la persona (`ApagadoPor`). Solo pasa en Linux; en
    /// Windows no va nunca y lo que se manda a la interfaz es lo de siempre.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apagado_por: Option<&'static str>,
}

impl ReplayStatus {
    pub fn parado() -> Self {
        Self {
            running: false,
            seconds: 0,
            screen: 0,
            screen_label: String::new(),
            bytes: 0,
            bytes_per_second: 0,
            width: 0,
            height: 0,
            buffered_ms: 0,
            apagado_por: None,
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
    /// A que tamanno se esta guardando, con la calidad ya aplicada.
    pub medida: (u32, u32),
    stop: Arc<AtomicBool>,
    /// Puesto, la captura de Windows no entrega ni un fotograma. Ver `apartar`.
    pause: Arc<AtomicBool>,
    /// Milisegundos acumulados apartado, para que el reloj no cuente ese tiempo y lo de
    /// antes y lo de despues queden pegados.
    paused_ms: Arc<AtomicU64>,
    ordenes: Sender<Orden>,
    bytes: Arc<AtomicU64>,
    /// Todo lo que se ha escrito desde que arranco, sin restar lo podado: dividido por el
    /// tiempo, es el ritmo al que le esta dando al disco.
    escritos: Arc<AtomicU64>,
    arranque: Instant,
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
            bytes_per_second: {
                // Con menos de dos segundos de vida el numero baila muchisimo, y ensennar
                // «180 MB/s» el primer segundo asustaria sin motivo.
                let vivo = self.arranque.elapsed().as_secs();
                if vivo < 2 {
                    0
                } else {
                    self.escritos.load(Ordering::Relaxed) / vivo
                }
            },
            width: self.medida.0,
            height: self.medida.1,
            buffered_ms: self.grabado_ms.load(Ordering::Relaxed),
            apagado_por: None,
        }
    }
}

/// El estado de ahora mismo, corra o no corra.
pub fn status(app: &AppHandle) -> ReplayStatus {
    let estado = app
        .state::<AppState>()
        .replay
        .lock()
        .as_ref()
        .map(ReplayState::status)
        .unwrap_or_else(ReplayStatus::parado);
    #[cfg(target_os = "linux")]
    let estado = ReplayStatus {
        apagado_por: if estado.running { None } else { APAGADO_POR.lock().map(ApagadoPor::nombre) },
        ..estado
    };
    estado
}

/// Por que el anillo esta apagado sin que la persona tocara su interruptor. Los ajustes lo
/// dicen en la fila, en gris: un interruptor que aparece apagado sin explicacion parece un
/// fallo, y quien contaba con rescatar algo tiene que saber que paso.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApagadoPor {
    /// Al arrancar en Wayland no habia permiso guardado para compartir la pantalla, y no se
    /// le saca a nadie una pregunta del escritorio al iniciar sesion.
    SinPermiso,
    /// El escritorio dejo de compartir la pantalla: la persona lo paro desde su aviso.
    Cortado,
    /// La pantalla dejo de llegar, o no se pudo abrir al arrancar.
    Fallo,
}

#[cfg(target_os = "linux")]
impl ApagadoPor {
    /// Como lo conoce la interfaz (`ReplayStatus::apagado_por`) y como se guarda.
    fn nombre(self) -> &'static str {
        match self {
            Self::SinPermiso => "sin-permiso",
            Self::Cortado => "cortado",
            Self::Fallo => "fallo",
        }
    }

    fn de_nombre(nombre: &str) -> Option<Self> {
        [Self::SinPermiso, Self::Cortado, Self::Fallo].into_iter().find(|por| por.nombre() == nombre)
    }
}

#[cfg(target_os = "linux")]
static APAGADO_POR: parking_lot::Mutex<Option<ApagadoPor>> = parking_lot::Mutex::new(None);

/// El archivo, junto a los ajustes, que guarda ese porque de un arranque a otro: la app
/// arranca con la sesion y los ajustes se abren cuando se abren, quiza dias despues. No va en
/// `settings.json` porque no es un ajuste (nadie lo elige) y solo existe en Linux.
#[cfg(target_os = "linux")]
const MARCA_DEL_APAGADO: &str = "replay-apagado";

#[cfg(target_os = "linux")]
fn marca_del_apagado(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|carpeta| carpeta.join(MARCA_DEL_APAGADO))
}

#[cfg(target_os = "linux")]
fn apuntar_el_apagado(app: &AppHandle, por: ApagadoPor) {
    *APAGADO_POR.lock() = Some(por);
    let Some(marca) = marca_del_apagado(app) else { return };
    let escrito = marca.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&marca, por.nombre()));
    if let Err(error) = escrito {
        eprintln!("[replay] no se ha podido apuntar por que se apago ({error}): los ajustes no lo diran tras reiniciar");
    }
}

/// La persona ha tocado el interruptor: ya sabe como esta, y el porque de antes sobra.
#[cfg(target_os = "linux")]
pub fn olvidar_el_apagado(app: &AppHandle) {
    *APAGADO_POR.lock() = None;
    if let Some(marca) = marca_del_apagado(app) {
        let _ = std::fs::remove_file(marca);
    }
}

/// Lo que hay que mirar del anillo al arrancar, ANTES de pedir las teclas (`lib.rs`).
///
/// **En Wayland el anillo solo se enciende solo si el escritorio ya dio permiso.** Sin el,
/// encender es que el escritorio saque su pregunta de compartir pantalla al iniciar sesion,
/// sin que nadie haya tocado nada. Entonces el ajuste pasa a apagado aqui mismo: un
/// interruptor en «si» sobre un anillo que no graba seria mentirle a quien confia en rescatar
/// algo, y con el ajuste ya apagado su tecla tampoco se le pide al escritorio (otra pregunta
/// al iniciar sesion). El porque queda apuntado para que la fila lo diga.
#[cfg(target_os = "linux")]
pub fn ajustes_al_arrancar(app: &AppHandle, ajustes: &mut crate::settings::Settings) {
    if !ajustes.replay_enabled {
        // Apagado desde un arranque anterior: el porque vale hasta que se toque el interruptor.
        *APAGADO_POR.lock() = marca_del_apagado(app)
            .and_then(|marca| std::fs::read_to_string(marca).ok())
            .and_then(|nombre| ApagadoPor::de_nombre(nombre.trim()));
        return;
    }
    olvidar_el_apagado(app);
    let con_permiso = !crate::platform::xdg::es_wayland()
        || ficha_del_permiso(app).is_ok_and(|ficha| std::fs::metadata(ficha).is_ok_and(|datos| datos.len() > 0));
    if con_permiso {
        return;
    }
    eprintln!("[replay] no se enciende al arrancar: el escritorio no tiene guardado el permiso; se enciende en Ajustes");
    ajustes.replay_enabled = false;
    if let Err(error) = crate::settings::save(app, ajustes) {
        eprintln!("[replay] no se ha podido guardar el ajuste apagado: {error}");
    }
    apuntar_el_apagado(app, ApagadoPor::SinPermiso);
}

fn avisar(app: &AppHandle) {
    let _ = app.emit(EVENT_REPLAY, status(app));
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
#[cfg(any(windows, target_os = "linux"))]
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

/// La pantalla que cogeria el anillo ahora mismo, para el banco que mide lo que cuesta.
#[cfg(all(test, windows))]
pub(crate) fn pantalla_para_medir() -> Result<MonitorInfo> {
    pantalla_elegida(None)
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

    let (segundos, fps, alto_pedido, pedida, quiere_audio, quiere_micro, con_cursor) = {
        let settings = state.settings.read();
        (
            settings.replay_seconds.clamp(SEGUNDOS_MIN, SEGUNDOS_MAX),
            // El ritmo del anillo va aparte del de la grabacion normal: aqui se graba todo
            // el rato, asi que lo que cuesta cada fotograma se paga toda la tarde.
            settings.replay_fps.clamp(5, 60),
            settings.replay_height,
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
    // A que tamanno se guarda. `None` es tal cual sale de la pantalla.
    let destino = crate::encode::escalar::medida_para(region.width, region.height, alto_pedido);
    // Lo que va a medir de verdad lo guardado. Es lo que ve el editor, asi que el resto de
    // la sesion (los clics, el raton, el video exportado) se mide contra esto y no contra
    // la pantalla.
    let (ancho_guardado, alto_guardado) = destino.unwrap_or((region.width, region.height));
    let region_guardada = Rect {
        width: ancho_guardado,
        height: alto_guardado,
        ..region
    };

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
        .join(&uuid::Uuid::new_v4().simple().to_string()[..8]);

    // La bandera de apartar se crea antes que el sonido a proposito: el hilo de audio la
    // recibe al nacer, y asi apartar el anillo tambien calla lo que suene mientras tanto.
    let pause = Arc::new(AtomicBool::new(false));
    let audio = {
        let fuentes = record::audio::Fuentes {
            sistema: quiere_audio,
            microfono: quiere_micro,
        };
        if fuentes.ninguna() {
            None
        } else {
            match record::audio::empezar(fuentes, pause.clone()) {
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
    let reciclados = win::Reciclados::default();
    let reciclados_cocina = reciclados.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let paused_ms = Arc::new(AtomicU64::new(0));
    let bytes = Arc::new(AtomicU64::new(0));
    let escritos = Arc::new(AtomicU64::new(0));
    let grabado_ms = Arc::new(AtomicU64::new(0));

    let hilo = {
        let app = app.clone();
        let stop = stop.clone();
        let bytes = bytes.clone();
        let escritos = escritos.clone();
        let grabado_ms = grabado_ms.clone();
        std::thread::spawn(move || {
            let ventana_ms = u64::from(segundos) * 1000;
            let anillo = match Anillo::nuevo(
                &dir,
                ventana_ms,
                fps,
                crate::record::buffer::bytes_max(segundos, fps, ancho_guardado, alto_guardado),
            ) {
                Ok(anillo) => anillo,
                Err(error) => {
                    eprintln!("[replay] no se ha podido abrir el anillo: {error}");
                    return;
                }
            };
            let mut cocina = Cocina::nueva(
                app,
                anillo,
                Receta { region, region_guardada, destino, fps, ventana_ms, con_cursor },
                reciclados_cocina,
            );
            cocina.audio_anillo = audio.as_ref().map(|captura| {
                let info = AudioInfo {
                    channels: captura.formato.canales,
                    sample_rate: captura.formato.muestras_por_segundo,
                };
                (info, AnilloAudio::nuevo(info.bytes_por_ms(), info.channels, ventana_ms))
            });
            cocina.trabajar(&receiver, &buzon, &stop, &bytes, &escritos, &grabado_ms, audio.as_ref());
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
            // El anillo no se pausa a peticion de nadie: o graba o esta apagado, porque
            // una pausa a mano seria un agujero justo en el trozo que alguien va a querer.
            // Lo unico que lo aparta es abrir una captura (ver `apartar`), y ahi no hay
            // agujero: el tiempo apartado se descuenta del reloj con `paused_ms`, asi que
            // lo de antes y lo de despues quedan cosidos.
            pause: pause.clone(),
            paused_ms: paused_ms.clone(),
            min_interval_ms: 0,
            start: Instant::now(),
            reciclados,
            // S29: el anillo pide solo lo que cambia. Ver `Cocina::tragar`.
            solo_cambios: true,
        },
    )?;

    *state.replay.lock() = Some(ReplayState {
        seconds: segundos,
        monitor,
        medida: (ancho_guardado, alto_guardado),
        stop,
        pause,
        paused_ms,
        ordenes,
        bytes,
        escritos,
        arranque: Instant::now(),
        grabado_ms,
        control: Some(control),
        hilo: Some(hilo),
    });
    avisar(app);
    Ok(status(app))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn start(_app: &AppHandle) -> Result<ReplayStatus> {
    Err(AppError::Unsupported)
}

/// Enciende el anillo en Linux, sobre una pantalla entera.
///
/// La imagen sale de la captura de grabar (`record::linux`: MIT-SHM en X11, el portal y
/// PipeWire en Wayland), que manda cada fotograma entero y solo cuando la pantalla cambia. El
/// sonido va por PipeWire a un anillo por fuente, y sale como las dos pistas de siempre.
///
/// **En Wayland puede tardar lo que tarde la persona en contestar al escritorio**, que
/// pregunta que pantalla se comparte la primera vez. El permiso se guarda en su propia ficha
/// (`replay-token`), aparte de la de grabar: cada una se gasta al usarla y el escritorio da
/// otra, y con una sola se la pisarian. Mientras el anillo esta encendido el escritorio
/// ensenna su aviso de que se comparte la pantalla: es suyo y no se puede quitar.
#[cfg(target_os = "linux")]
pub fn start(app: &AppHandle) -> Result<ReplayStatus> {
    use std::sync::mpsc::channel;

    use crate::record::linux::{self, sonido, Marcha};

    let state = app.state::<AppState>();
    if state.replay.lock().is_some() {
        return Ok(status(app));
    }

    let (segundos, fps, alto_pedido, pedida, quiere_audio, quiere_micro) = {
        let settings = state.settings.read();
        (
            settings.replay_seconds.clamp(SEGUNDOS_MIN, SEGUNDOS_MAX),
            settings.replay_fps.clamp(5, 60),
            settings.replay_height,
            settings.replay_screen,
            settings.record_audio,
            settings.record_microphone,
        )
    };
    // Quien apaga este anillo lo deja dicho aqui. Tambien sirve para saber quien fue el
    // primero: si lo apaga la persona, lo que diga despues la captura ya no cuenta.
    let stop = Arc::new(AtomicBool::new(false));
    let opciones = linux::Opciones {
        ficha: ficha_del_permiso(app)?,
        al_cortarse: {
            let app = app.clone();
            let stop = stop.clone();
            Box::new(move || {
                if stop.swap(true, Ordering::Relaxed) {
                    return;
                }
                // En otro hilo: apagar espera a que acabe la captura, y esto lo llama la captura.
                let app = app.clone();
                std::thread::spawn(move || apagado_por_su_cuenta(&app, ApagadoPor::Cortado));
            })
        },
    };
    // En Wayland la pantalla la elige la persona en la pregunta del escritorio: la pedida es
    // solo la que se prefiere si ha dejado varias.
    let (preparada, monitor) = linux::preparar_pantalla(pantalla_elegida(pedida)?, fps, opciones)?;
    let region = preparada.region();
    let destino = crate::encode::escalar::medida_para(region.width, region.height, alto_pedido);
    let (ancho_guardado, alto_guardado) = destino.unwrap_or((region.width, region.height));
    let region_guardada = Rect {
        width: ancho_guardado,
        height: alto_guardado,
        ..region
    };

    // Cada anillo estrena carpeta: ver el `start` de Windows.
    let dir = state
        .temp_root
        .join("replay")
        .join(&uuid::Uuid::new_v4().simple().to_string()[..8]);
    let ventana_ms = u64::from(segundos) * 1000;
    let anillo = Anillo::nuevo(
        &dir,
        ventana_ms,
        fps,
        crate::record::buffer::bytes_max(segundos, fps, ancho_guardado, alto_guardado),
    )?;

    let pause = Arc::new(AtomicBool::new(false));
    let paused_ms = Arc::new(AtomicU64::new(0));
    let bytes = Arc::new(AtomicU64::new(0));
    let escritos = Arc::new(AtomicU64::new(0));
    let grabado_ms = Arc::new(AtomicU64::new(0));
    // El cero del reloj: el mismo para los fotogramas, el raton, las teclas y el sonido. Lo
    // unico que lo para es `apartar`, con la pausa de siempre.
    let marcha = Marcha {
        stop: stop.clone(),
        pause: pause.clone(),
        paused_ms: paused_ms.clone(),
        start: Instant::now(),
    };
    let en_anillo = sonido::empezar_en_anillo(
        sonido::Fuentes { sistema: quiere_audio, microfono: quiere_micro },
        ventana_ms,
        marcha.clone(),
    );
    let (sender, receiver) = channel::<linux::Fotograma>();
    let (ordenes, buzon) = channel::<Orden>();
    let reciclados = linux::Reciclados::default();
    let raton_cocido = preparada.raton_cocido();
    let en_marcha = preparada.grabar(marcha.clone(), sender, reciclados.clone());
    eprintln!(
        "[replay] encendido: {}x{} en {},{} a {fps} por segundo, {segundos} s",
        region.width, region.height, region.x, region.y
    );

    let hilo = {
        let app = app.clone();
        let stop = stop.clone();
        let bytes = bytes.clone();
        let escritos = escritos.clone();
        let grabado_ms = grabado_ms.clone();
        // Que tecla rescata, para no apuntarla como un atajo de lo que se estaba haciendo.
        let tecla_de_rescate = record::teclas::TeclaZoom::de_atajo(&state.settings.read().replay_shortcut);
        std::thread::spawn(move || {
            let mut cocina = Cocina::nueva(
                app,
                anillo,
                // En Linux el raton no va en la imagen, salvo en los escritorios de Wayland
                // que solo saben darlo ya dibujado: entonces no se dibuja otra vez.
                Receta { region, region_guardada, destino, fps, ventana_ms, con_cursor: raton_cocido },
                DeLinux {
                    marcha,
                    rastro: en_marcha.rastro_vivo(),
                    sonido: en_anillo,
                    reciclados,
                    // El raton y el teclado solo se dejan leer en X11.
                    con_entrada: !crate::platform::xdg::es_wayland(),
                    tecla_de_rescate,
                },
            );
            cocina.trabajar(&receiver, &buzon, &stop, &bytes, &escritos, &grabado_ms);
            if !stop.load(Ordering::Relaxed) {
                // Nadie lo ha apagado: es la captura la que ha dejado de mandar la pantalla.
                // Si ha sido el escritorio el que ha dejado de compartir, lo dice el por su
                // lado (`al_cortarse`) y es mejor porque: se le deja un momento para llegar.
                std::thread::sleep(std::time::Duration::from_millis(300));
                if !stop.swap(true, Ordering::Relaxed) {
                    let app = cocina.app.clone();
                    std::thread::spawn(move || apagado_por_su_cuenta(&app, ApagadoPor::Fallo));
                }
            }
            drop(en_marcha.terminar());
            if let Some(sonido) = cocina.linux.sonido.take() {
                sonido.terminar();
            }
            cocina.anillo.limpiar();
        })
    };

    *state.replay.lock() = Some(ReplayState {
        seconds: segundos,
        monitor,
        medida: (ancho_guardado, alto_guardado),
        stop,
        pause,
        paused_ms,
        ordenes,
        bytes,
        escritos,
        arranque: Instant::now(),
        grabado_ms,
        hilo: Some(hilo),
    });
    avisar(app);
    Ok(status(app))
}

/// Donde se guarda el permiso del escritorio para el anillo (Wayland).
#[cfg(target_os = "linux")]
fn ficha_del_permiso(app: &AppHandle) -> Result<std::path::PathBuf> {
    Ok(app.path().app_config_dir().map_err(|e| AppError::Msg(e.to_string()))?.join("replay-token"))
}

/// Enciende al arrancar la app el anillo que se dejo encendido, en Linux. Si en este arranque
/// no tocaba encenderlo (Wayland sin permiso guardado), el ajuste ya llega apagado:
/// `ajustes_al_arrancar`.
///
/// Va en su hilo: abrir la captura habla con el escritorio, y el arranque no espera a eso. Si
/// no se enciende, el ajuste pasa a apagado, como cuando falla al encenderlo en los ajustes.
#[cfg(target_os = "linux")]
pub fn al_arrancar(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = start(&app) {
            eprintln!("[replay] no se ha podido encender al arrancar: {error}");
            apagar_el_ajuste(&app, ApagadoPor::Fallo);
        }
    });
}

/// El anillo ha dejado de grabar sin que nadie lo apague en winshotx: casi siempre porque la
/// persona ha parado de compartir la pantalla desde el aviso del escritorio. Se apaga del
/// todo y el interruptor vuelve a apagado, que es lo que esta pasando.
#[cfg(target_os = "linux")]
fn apagado_por_su_cuenta(app: &AppHandle, por: ApagadoPor) {
    eprintln!("[replay] el anillo se apaga solo: {}", por.nombre());
    let _ = stop(app);
    apagar_el_ajuste(app, por);
}

/// Deja el ajuste en apagado, lo guarda, apunta por que y suelta la tecla de rescatar.
#[cfg(target_os = "linux")]
fn apagar_el_ajuste(app: &AppHandle, por: ApagadoPor) {
    let state = app.state::<AppState>();
    let settings = {
        let mut ajustes = state.settings.write();
        ajustes.replay_enabled = false;
        ajustes.clone()
    };
    if let Err(error) = crate::settings::save(app, &settings) {
        eprintln!("[replay] no se ha podido guardar el ajuste apagado: {error}");
    }
    apuntar_el_apagado(app, por);
    crate::hotkeys::register(app, &settings);
    avisar(app);
}

/// El anillo, apartado mientras se abre una captura.
///
/// **Por que existe, y esta medido.** El 31 de agosto de 2026, en la maquina de Munir: el
/// anillo a 60 fps y alto nativo se come el **86% de un nucleo** (2,9% la aplicacion sin
/// el). Abrir una captura es congelar todas las pantallas a la vez, y hacerlo mientras eso
/// corre es competir contra uno mismo con la misma maquina. Munir lo dijo asi: «tarda en
/// cargar al principio cuando le das al shortcut y quieres hacer la captura».
///
/// **Y no deja agujero**, que es lo que hasta ahora impedia pausarlo: el tiempo apartado se
/// descuenta del reloj de la captura (`paused_ms`, ver `record::win`), asi que el fotograma
/// de antes y el de despues quedan pegados. Quien rescate los ultimos segundos no vera un
/// salto, vera que la captura no se grabo a si misma, que ademas es lo que quiere.
///
/// El tramo es corto a proposito: se aparta justo antes de congelar y vuelve con las
/// ventanas ya puestas, medio segundo. NO dura lo que el usuario tarde en elegir el
/// recorte, porque eso si serian minutos de anillo perdidos sin que nadie lo pidiera.
#[must_use = "el anillo vuelve a grabar cuando se suelta este guardia, no antes"]
pub struct Apartado(Option<(Arc<AtomicBool>, Arc<AtomicU64>, Instant)>);

impl Drop for Apartado {
    fn drop(&mut self) {
        let Some((pause, paused_ms, desde)) = self.0.take() else {
            return;
        };
        // El orden importa: primero se suma lo que ha durado y despues se suelta. Al reves,
        // un fotograma podria colarse entre las dos lineas leyendo el `paused_ms` viejo, y
        // su marca de tiempo saltaria hacia adelante justo el rato apartado.
        paused_ms.fetch_add(desde.elapsed().as_millis() as u64, Ordering::Relaxed);
        pause.store(false, Ordering::Relaxed);
    }
}

/// Aparta el anillo hasta que se suelte lo devuelto. Sin anillo encendido no hace nada,
/// y el guardia vale igual: asi quien llama no tiene que preguntar si hay anillo.
pub fn apartar(app: &AppHandle) -> Apartado {
    let state = app.state::<AppState>();
    let guard = state.replay.lock();
    let Some(replay) = guard.as_ref() else {
        return Apartado(None);
    };
    replay.pause.store(true, Ordering::Relaxed);
    Apartado(Some((
        replay.pause.clone(),
        replay.paused_ms.clone(),
        Instant::now(),
    )))
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
    avisar(app);
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
    // En Linux el raton y el teclado los mira el `Vigia`, en su hilo.
    #[cfg(windows)]
    vigilante: record::raton::Vigilante,
    #[cfg(windows)]
    teclado: record::teclas::Vigilante,
    region: Rect,
    /// La misma region pero con el tamanno al que se guarda, que es el que ve el editor.
    region_guardada: Rect,
    /// A que tamanno se guarda cada fotograma, si no es el de la pantalla.
    destino: Option<(u32, u32)>,
    fps: u32,
    ventana_ms: u64,
    con_cursor: bool,
    #[cfg(windows)]
    desde_la_limpieza: u32,
    /// Por donde vuelven los bufers a la captura. Ver `win::Reciclados`.
    #[cfg(windows)]
    reciclados: crate::record::win::Reciclados,
    /// Lo que el anillo solo lleva en Linux.
    #[cfg(target_os = "linux")]
    linux: DeLinux,
    /// La pantalla puesta al dia con lo que llega, y como se guarda. Ver `ImagenDelAnillo`.
    imagen: crate::record::buffer::ImagenDelAnillo,
}

/// Que se graba y como se guarda: lo mismo en los dos sistemas.
#[derive(Debug, Clone, Copy)]
struct Receta {
    region: Rect,
    region_guardada: Rect,
    destino: Option<(u32, u32)>,
    fps: u32,
    ventana_ms: u64,
    /// El raton va ya dibujado en la imagen: el exportador no lo dibuja otra vez.
    con_cursor: bool,
}

/// Lo que el anillo solo lleva en Linux.
#[cfg(target_os = "linux")]
struct DeLinux {
    /// El reloj del anillo, que tambien dice si esta apartado (`apartar`).
    marcha: crate::record::linux::Marcha,
    /// Por donde va el raton y que puntero lleva: lo apunta quien captura, en su hilo.
    rastro: crate::record::linux::RastroVivo,
    /// El sonido, cada fuente en su anillo. `None` si no se graba ninguna.
    sonido: Option<crate::record::linux::sonido::EnAnillo>,
    reciclados: crate::record::linux::Reciclados,
    /// Se puede preguntar por los botones y las teclas: en X11 si, en Wayland no.
    con_entrada: bool,
    /// La tecla que rescata, para no apuntarla como un atajo de lo que se estaba haciendo.
    tecla_de_rescate: Option<record::teclas::TeclaZoom>,
}

/// Los clics y los atajos que el `Vigia` ha visto y la cocina no ha recogido todavia.
#[cfg(target_os = "linux")]
#[derive(Default)]
struct Pulsado {
    clics: Vec<crate::encode::zoom::Clic>,
    teclas: Vec<record::teclas::Atajo>,
}

/// El hilo que mira el raton y el teclado mientras el anillo graba (solo X11).
///
/// Tiene hilo propio, como el `Muestreador` de grabar, porque un clic dura menos que guardar
/// un fotograma. Mirando entre fotograma y fotograma, en el hilo de la cocina, con la pantalla
/// entera cambiando pasaban mas de 60 ms entre dos miradas y un clic rapido cabia entero en
/// medio: en el laboratorio se perdian 3 de cada 10 clics de 60 ms y todos los de 25.
#[cfg(target_os = "linux")]
struct Vigia {
    pulsado: Arc<parking_lot::Mutex<Pulsado>>,
    parar: Arc<AtomicBool>,
    hilo: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
impl Vigia {
    /// Empieza a mirar. `x_en` e `y_en` llevan un punto de la region a la escala a la que se
    /// guarda (`Cocina::escalar_punto`): lo apuntado sale ya en esa escala.
    fn empezar(
        marcha: crate::record::linux::Marcha,
        region: Rect,
        x_en: impl Fn(i32) -> i32 + Send + 'static,
        y_en: impl Fn(i32) -> i32 + Send + 'static,
        tecla_de_rescate: Option<record::teclas::TeclaZoom>,
    ) -> Self {
        let pulsado = Arc::new(parking_lot::Mutex::new(Pulsado::default()));
        let parar = Arc::new(AtomicBool::new(false));
        let hilo = {
            let (pulsado, parar) = (pulsado.clone(), parar.clone());
            std::thread::spawn(move || {
                let mut raton = record::raton::Vigilante::default();
                let mut teclado = record::teclas::Vigilante::default();
                let mut lector = record::teclas::Lector::default();
                loop {
                    std::thread::sleep(record::anotador::CADA);
                    if parar.load(Ordering::Relaxed) {
                        break;
                    }
                    // Apartado, el reloj no vale: va por delante hasta que se suelta (`Apartado`).
                    if marcha.en_pausa() {
                        continue;
                    }
                    let ms = marcha.ms();
                    let clic = raton.mirar(ms).and_then(|clic| {
                        let (x, y) = (clic.x - region.x, clic.y - region.y);
                        // Un clic en otra pantalla no es de lo que se esta guardando.
                        let dentro = x >= 0 && y >= 0 && x < region.width as i32 && y < region.height as i32;
                        dentro.then(|| crate::encode::zoom::Clic { ms: clic.ms, x: x_en(x), y: y_en(y), derecho: clic.derecho })
                    });
                    let foto = lector.leer();
                    // La tecla que rescata es de winshotx, no de lo que se estaba haciendo: no se apunta.
                    let rescate = tecla_de_rescate.as_ref().is_some_and(|tecla| tecla.pulsada(&foto));
                    let atajo = teclado.mirar_foto(&foto, ms).filter(|_| !rescate).map(|atajo| {
                        // Un atajo no tiene sitio propio: va donde este el raton.
                        let (x, y) = record::raton::cursor().unwrap_or((region.x, region.y));
                        record::teclas::Atajo { x: x_en(x - region.x), y: y_en(y - region.y), ..atajo }
                    });
                    if clic.is_some() || atajo.is_some() {
                        let mut pulsado = pulsado.lock();
                        pulsado.clics.extend(clic);
                        pulsado.teclas.extend(atajo);
                    }
                }
            })
        };
        Self { pulsado, parar, hilo: Some(hilo) }
    }

    /// Lo visto desde la ultima vez, que pasa a ser de quien lo recoge.
    fn recoger(&self) -> Pulsado {
        std::mem::take(&mut *self.pulsado.lock())
    }
}

#[cfg(target_os = "linux")]
impl Drop for Vigia {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

/// El raton tal y como lo apunta la captura de Linux, ya en la escala a la que se guarda:
/// por donde fue, las imagenes de sus punteros (con su punto caliente) y cual llevaba.
#[cfg(target_os = "linux")]
struct RastroDelAnillo {
    cursor: Vec<(u64, i32, i32)>,
    punteros: Vec<((u32, u32), image::RgbaImage)>,
    cambios_puntero: Vec<(u64, u32)>,
}

impl Cocina {
    /// La cocina recien abierta. **Se construye aqui para los dos sistemas**: lo que cambie
    /// de como se anota (por ejemplo, en que idioma se nombran las teclas de la pastilla) se
    /// cambia una vez y vale en Windows y en Linux.
    #[cfg(any(windows, target_os = "linux"))]
    fn nueva(
        app: AppHandle,
        anillo: Anillo,
        receta: Receta,
        #[cfg(windows)] reciclados: crate::record::win::Reciclados,
        #[cfg(target_os = "linux")] linux: DeLinux,
    ) -> Self {
        Self {
            app,
            anillo,
            audio_anillo: None,
            clics: Vec::new(),
            teclas: Vec::new(),
            cursor: Vec::new(),
            #[cfg(windows)]
            vigilante: record::raton::Vigilante::default(),
            #[cfg(windows)]
            teclado: record::teclas::Vigilante::default(),
            region: receta.region,
            region_guardada: receta.region_guardada,
            destino: receta.destino,
            fps: receta.fps,
            ventana_ms: receta.ventana_ms,
            con_cursor: receta.con_cursor,
            #[cfg(windows)]
            desde_la_limpieza: 0,
            #[cfg(windows)]
            reciclados,
            #[cfg(target_os = "linux")]
            linux,
            imagen: crate::record::buffer::ImagenDelAnillo::nueva(receta.region.width, receta.region.height, receta.destino),
        }
    }

    // Ocho argumentos, uno mas de los que clippy da por buenos. Se queda asi a proposito: los
    // cinco ultimos son los marcadores que este hilo comparte con quien lo mira desde fuera
    // (`ReplayState`), y meterlos en una struct solo para bajar la cuenta seria un envoltorio
    // que hay que abrir en cada uso sin que nadie entienda mejor lo que pasa aqui.
    #[allow(clippy::too_many_arguments)]
    #[cfg(windows)]
    fn trabajar(
        &mut self,
        receiver: &Receiver<crate::record::win::CapturedFrame>,
        buzon: &Receiver<Orden>,
        stop: &Arc<AtomicBool>,
        bytes: &Arc<AtomicU64>,
        escritos: &Arc<AtomicU64>,
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
                Ok(primero) => {
                    // Si mientras se guardaba el anterior han llegado varios, se guarda UNO
                    // con todo lo que traian. Esto corre durante horas: si el disco o la
                    // maquina no dan para el ritmo pedido, la cola crece sola y cada
                    // fotograma entero son ocho megabytes de memoria. Mejor grabar a menos
                    // fotogramas por segundo que llenarle la RAM a alguien que ni sabe que
                    // esto esta puesto.
                    //
                    // S29: los que traen solo zonas NO se pueden tirar (cada uno dice lo que
                    // cambio desde el anterior); se pegan todos, que es copiar unos pocos
                    // pixeles. Los enteros si: uno entero deja viejo todo lo de antes.
                    let mut lote = vec![primero];
                    while let Ok(siguiente) = receiver.try_recv() {
                        lote.push(siguiente);
                    }
                    let desde = lote.iter().rposition(|f| f.zonas.is_none()).unwrap_or(0);
                    perdidos += (lote.len() - 1) as u64;
                    for (i, frame) in lote.into_iter().enumerate() {
                        ultimo_ts = frame.ts_ms;
                        let bufer = if i < desde {
                            frame.bgra
                        } else {
                            self.imagen.recibir(frame.bgra, frame.zonas)
                        };
                        self.reciclados.devolver(bufer);
                    }
                    let antes = self.anillo.escritos();
                    self.tragar(ultimo_ts);
                    escritos.fetch_add(
                        self.anillo.escritos().saturating_sub(antes),
                        Ordering::Relaxed,
                    );
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

    /// El mismo trabajo en Linux, con dos diferencias. La captura manda cada fotograma ENTERO
    /// y solo cuando la pantalla cambia, asi que si llegan varios juntos vale el ultimo. Y el
    /// raton y el teclado se miran en otro hilo (`Vigia`), a su ritmo y no al de los
    /// fotogramas: con la pantalla quieta no llega ninguno, y con ella cambiando entera
    /// guardar uno tarda mas de lo que dura un clic.
    #[cfg(target_os = "linux")]
    fn trabajar(
        &mut self,
        receiver: &Receiver<crate::record::linux::Fotograma>,
        buzon: &Receiver<Orden>,
        stop: &Arc<AtomicBool>,
        bytes: &Arc<AtomicU64>,
        escritos: &Arc<AtomicU64>,
        grabado_ms: &Arc<AtomicU64>,
    ) {
        use std::sync::mpsc::RecvTimeoutError;
        use std::time::Duration;

        // En X11 el raton y el teclado se miran aparte, mientras dure este trabajo. En Wayland
        // no se dejan leer y no hay nada que mirar.
        let vigia = if self.linux.con_entrada {
            let (x_en, y_en) = self.escalar_punto();
            Some(Vigia::empezar(self.linux.marcha.clone(), self.region, x_en, y_en, self.linux.tecla_de_rescate.take()))
        } else {
            None
        };
        // Sin nada que mirar entre fotograma y fotograma, se espera lo mismo que en Windows.
        let espera = Duration::from_millis(150);
        let mut ultimo_ts = 0u64;
        let mut perdidos = 0u64;
        let mut avisado = 0u64;
        let mut podado_ms = 0u64;
        loop {
            // Las ordenes, antes de esperar fotograma, y varias seguidas son una: ver Windows.
            let mut piden = false;
            while let Ok(Orden::Cosechar) = buzon.try_recv() {
                piden = true;
            }
            if piden {
                self.recoger_lo_pulsado(vigia.as_ref());
                let ahora = self.linux.marcha.ms();
                if let Err(error) = self.cosechar(ahora.max(ultimo_ts)) {
                    eprintln!("[replay] no se ha podido guardar: {error}");
                }
            }
            if stop.load(Ordering::Relaxed) {
                break;
            }

            match receiver.recv_timeout(espera) {
                Ok(mut fotograma) => {
                    // Cada uno es la pantalla entera: si la maquina no da para el ritmo
                    // pedido, se guarda el ultimo y los de en medio vuelven a la captura.
                    while let Ok(siguiente) = receiver.try_recv() {
                        perdidos += 1;
                        self.linux.reciclados.devolver(std::mem::replace(&mut fotograma, siguiente).rgba);
                    }
                    ultimo_ts = fotograma.ts_ms;
                    let de_antes = self.imagen.recibir_rgba(fotograma.rgba);
                    self.linux.reciclados.devolver(de_antes);
                    let antes = self.anillo.escritos();
                    if let Err(error) = self.imagen.guardar(&mut self.anillo, ultimo_ts) {
                        eprintln!("[replay] fotograma perdido: {error}");
                    }
                    escritos.fetch_add(self.anillo.escritos().saturating_sub(antes), Ordering::Relaxed);
                    bytes.store(self.anillo.bytes(), Ordering::Relaxed);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if perdidos >= avisado + 600 {
                avisado = perdidos;
                eprintln!("[replay] {perdidos} fotogramas tirados por no dar abasto");
            }

            // Apartado, el reloj no vale: va por delante hasta que se suelta (`Apartado`).
            if self.linux.marcha.en_pausa() {
                continue;
            }
            let ahora = self.linux.marcha.ms().max(ultimo_ts);
            // Lo guardado se cuenta con el reloj y no con el ultimo fotograma: con la pantalla
            // quieta no llega ninguno, y lo que se puede rescatar sigue creciendo.
            grabado_ms.store(self.anillo.guardado_ms(ahora), Ordering::Relaxed);
            if ahora >= podado_ms + 5_000 {
                podado_ms = ahora;
                self.recoger_lo_pulsado(vigia.as_ref());
                self.podar(ahora);
            }
        }
    }

    /// Se trae los clics y los atajos que el `Vigia` ha visto desde la ultima vez. Se hace
    /// antes de rescatar y antes de podar, que son los dos que los usan.
    #[cfg(target_os = "linux")]
    fn recoger_lo_pulsado(&mut self, vigia: Option<&Vigia>) {
        if let Some(vigia) = vigia {
            let Pulsado { clics, teclas } = vigia.recoger();
            self.clics.extend(clics);
            self.teclas.extend(teclas);
        }
    }

    /// Tira las anotaciones que ya se han salido de la ventana: un anillo que corre toda la
    /// tarde no puede ir acumulando los clics y el rastro del raton de toda la tarde.
    #[cfg(target_os = "linux")]
    fn podar(&mut self, ahora_ms: u64) {
        let corte = ahora_ms.saturating_sub(self.ventana_ms + crate::record::buffer::SEGMENTO_MS);
        self.clics.retain(|clic| clic.ms >= corte);
        self.teclas.retain(|atajo| atajo.ms >= corte);
        self.linux.rastro.lock().podar(corte);
    }

    /// Lo que el rescate lleva de mas en Linux: el rastro del raton con sus punteros, ya en
    /// la escala a la que se guarda, y el tramo de sonido de cada fuente que acaba en `ahora_ms`.
    #[cfg(target_os = "linux")]
    fn lo_de_linux(&self, ahora_ms: u64) -> (RastroDelAnillo, Option<crate::record::linux::sonido::TramoDeSonido>) {
        let (x_en, y_en) = self.escalar_punto();
        let rastro = self.linux.rastro.lock().clone();
        let escala = self.destino.map(|(w, h)| {
            (w as f32 / self.region.width.max(1) as f32, h as f32 / self.region.height.max(1) as f32)
        });
        let punteros = rastro
            .punteros
            .into_iter()
            .map(|puntero| match escala {
                None => (puntero.caliente, puntero.imagen),
                // Guardando a menos tamanno, el puntero encoge lo mismo que la pantalla.
                Some((fx, fy)) => {
                    let a = |v: u32, f: f32| (v as f32 * f).round() as u32;
                    let (ancho, alto) = (a(puntero.imagen.width(), fx).max(1), a(puntero.imagen.height(), fy).max(1));
                    (
                        (a(puntero.caliente.0, fx), a(puntero.caliente.1, fy)),
                        image::imageops::resize(&puntero.imagen, ancho, alto, image::imageops::FilterType::Triangle),
                    )
                }
            })
            .collect();
        let rastro = RastroDelAnillo {
            cursor: rastro.cursor.into_iter().map(|(ms, x, y)| (ms, x_en(x), y_en(y))).collect(),
            punteros,
            cambios_puntero: rastro.cambios_puntero,
        };
        let sonido = self
            .linux
            .sonido
            .as_ref()
            .map(|sonido| sonido.tramo(ahora_ms, self.ventana_ms + 2_000))
            .filter(|tramo| tramo.sistema.is_some() || tramo.microfono.is_some());
        (rastro, sonido)
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    fn trabajar(
        &mut self,
        _receiver: &Receiver<()>,
        _buzon: &Receiver<Orden>,
        _stop: &Arc<AtomicBool>,
        _bytes: &Arc<AtomicU64>,
        _escritos: &Arc<AtomicU64>,
        _grabado_ms: &Arc<AtomicU64>,
        _audio: Option<&()>,
    ) {
    }

    /// Guarda la pantalla como esta (ya puesta al dia por `imagen.recibir`) y anota donde
    /// estaba el raton, los clics y los atajos en ese instante.
    #[cfg(windows)]
    fn tragar(&mut self, ts_ms: u64) {
        // Todo lo que se anota va YA en la escala a la que se guarda. Guardar a 720p desde
        // una pantalla de 1080p y anotar el clic en coordenadas de 1080 dejaria la camara
        // acercandose a un sitio que no es, y el puntero dibujado fuera de la pantalla.
        let (x_en, y_en) = self.escalar_punto();

        // Se anota lo mismo que en una grabacion normal (donde se pulso, que atajo y donde
        // estaba el raton) para que el editor pueda acercar la camara y dibujar el puntero
        // despues. Son doce bytes por clic: no se decide antes lo que se va a querer luego.
        if let Some((cx, cy)) = record::raton::cursor() {
            self.cursor
                .push((ts_ms, x_en(cx - self.region.x), y_en(cy - self.region.y)));
        }
        if let Some(clic) = self.vigilante.mirar(ts_ms) {
            self.clics.push(crate::encode::zoom::Clic {
                ms: clic.ms,
                x: x_en(clic.x - self.region.x),
                y: y_en(clic.y - self.region.y),
                derecho: clic.derecho,
            });
        }
        if let Some(atajo) = self.teclado.mirar(ts_ms) {
            self.teclas.push(record::teclas::Atajo {
                x: x_en(atajo.x - self.region.x),
                y: y_en(atajo.y - self.region.y),
                ..atajo
            });
        }

        if let Err(error) = self.imagen.guardar(&mut self.anillo, ts_ms) {
            eprintln!("[replay] fotograma perdido: {error}");
        }

        // Las anotaciones tambien se tiran cuando se salen de la ventana: un anillo que
        // corre toda la tarde no puede ir acumulando el rastro del raton de toda la tarde.
        self.desde_la_limpieza += 1;
        if self.desde_la_limpieza >= 60 {
            self.desde_la_limpieza = 0;
            let corte = ts_ms.saturating_sub(self.ventana_ms + crate::record::buffer::SEGMENTO_MS);
            self.cursor.retain(|(ms, _, _)| *ms >= corte);
            self.clics.retain(|clic| clic.ms >= corte);
            self.teclas.retain(|atajo| atajo.ms >= corte);
        }
    }

    /// Las dos funciones que llevan un punto de la pantalla a la escala guardada.
    fn escalar_punto(&self) -> (impl Fn(i32) -> i32 + use<>, impl Fn(i32) -> i32 + use<>) {
        let (fx, fy) = match self.destino {
            None => (1.0f32, 1.0f32),
            Some((w, h)) => (
                w as f32 / self.region.width.max(1) as f32,
                h as f32 / self.region.height.max(1) as f32,
            ),
        };
        (
            move |x: i32| (x as f32 * fx).round() as i32,
            move |y: i32| (y as f32 * fy).round() as i32,
        )
    }

    #[cfg(windows)]
    fn tragar_sonido(&mut self, captura: &record::audio::Captura) {
        let Some((_, anillo)) = self.audio_anillo.as_mut() else {
            return;
        };
        // S11 (pistas): la repetición instantánea sigue con las dos fuentes sumadas en una.
        while let Ok(trozo) = captura.trozos.try_recv() {
            anillo.empujar(&record::audio::a_pcm16(&trozo.mezcla()));
        }
    }

    /// Se queda con lo ultimo que paso y abre el editor, sin dejar de grabar.
    fn cosechar(&mut self, ahora_ms: u64) -> Result<()> {
        let (segmentos, corte, copia) = self.anillo.instantanea(ahora_ms)?;
        let sonido = self
            .audio_anillo
            .as_ref()
            .map(|(info, anillo)| (*info, anillo.ultimos(self.ventana_ms + 2_000)));
        #[cfg(target_os = "linux")]
        let (rastro, pistas) = self.lo_de_linux(ahora_ms);

        let encargo = Encargo {
            segmentos,
            corte,
            ahora: ahora_ms,
            copia,
            sonido,
            clics: self.clics.clone(),
            teclas: self.teclas.clone(),
            cursor: self.cursor.clone(),
            #[cfg(target_os = "linux")]
            rastro: Some(rastro),
            #[cfg(target_os = "linux")]
            pistas,
            region: self.region_guardada,
            fps: self.fps,
            con_cursor: self.con_cursor,
        };
        // Coser treinta segundos tarda lo suyo, y mientras tanto el anillo tiene que
        // seguir tragando fotogramas: si se parara, el hueco caeria justo en los segundos
        // siguientes a lo que alguien acaba de guardar.
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
    /// El raton como lo apunta la captura de Linux, con sus punteros. Manda sobre `cursor`.
    #[cfg(target_os = "linux")]
    rastro: Option<RastroDelAnillo>,
    /// El sonido de Linux: cada fuente por separado, para que salgan sus dos pistas.
    #[cfg(target_os = "linux")]
    pistas: Option<crate::record::linux::sonido::TramoDeSonido>,
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
    let suyo = id.clone();
    std::thread::spawn(move || {
        if let Err(error) = windows_mgr::open_editor(&handle, &suyo) {
            eprintln!("no se ha podido abrir el editor: {error}");
        }
    });
    preparar_por_detras(app, id);
    avisar(app);
    Ok(info)
}

/// Las sesiones cuya vista previa se intento escribir y no salio.
///
/// El aviso de que ha fallado viaja por un evento, y el que sale antes de que el editor monte
/// su escucha no lo recoge nadie. Donde la vista previa falla en el acto (en Linux todavia no
/// existe) el editor se quedaba en «Preparando el video…» para siempre, medido por S44 en el
/// laboratorio. Con esta lista, la sesion lo dice tambien cuando el editor le pregunta
/// (`SessionInfo::previa_fallida`).
static PREVIAS_FALLIDAS: parking_lot::Mutex<Vec<String>> = parking_lot::Mutex::new(Vec::new());

/// Si la vista previa de esa sesion se intento escribir y no salio.
pub(crate) fn previa_fallida(id: &str) -> bool {
    PREVIAS_FALLIDAS.lock().iter().any(|otra| otra == id)
}

fn apuntar_previa_fallida(id: &str) {
    let mut fallidas = PREVIAS_FALLIDAS.lock();
    if !fallidas.iter().any(|otra| otra == id) {
        fallidas.push(id.to_string());
    }
}

fn olvidar_previa_fallida(id: &str) {
    PREVIAS_FALLIDAS.lock().retain(|otra| otra != id);
}

#[cfg(test)]
mod pruebas_de_las_fallidas {
    use super::*;

    #[test]
    fn una_vista_previa_que_falla_se_recuerda_hasta_que_se_vuelve_a_intentar() {
        assert!(!previa_fallida("s44-una"));
        apuntar_previa_fallida("s44-una");
        apuntar_previa_fallida("s44-una");
        assert!(previa_fallida("s44-una"));
        assert!(!previa_fallida("s44-otra"), "cada sesion lleva lo suyo");
        // Al volver a intentarlo se empieza de cero: la primera linea de `preparar_por_detras`.
        olvidar_previa_fallida("s44-una");
        assert!(!previa_fallida("s44-una"));
    }
}

/// Escribe POR DETRAS lo que el editor no necesita para abrirse: el MP4 de la vista
/// previa primero y las miniaturas de la tira despues.
///
/// **En ese orden y en un solo hilo.** El play es la funcion principal del editor y la
/// tira se ve igual con huecos, asi que el video va delante; y en un hilo porque los dos
/// trabajos leen el mismo cache y ya usan todos los nucleos por dentro: a la vez solo se
/// quitarian tiempo el uno al otro.
///
/// Es lo que el editor sabe reproducir: quien mueve los fotogramas al darle al play es el
/// `<video>`, y sin video el boton no hace absolutamente nada. La grabacion normal lo
/// escribe mientras graba, en streaming y casi gratis; el anillo no puede, porque hasta que
/// alguien pulsa la tecla no se sabe donde empieza lo que se va a guardar.
///
/// **Tambien la llama la grabacion normal cuando su codificador en vivo se ha caido**
/// (`recorder::entregar`). Antes, esa sesion llegaba al editor con `mp4_path` a nada y sin
/// nadie que avisara nunca: el cartel de «preparando la reproduccion» se quedaba puesto
/// para siempre, sin porcentaje y sin fallo, con el play apagado. Escribirla por aqui
/// cuesta unos segundos y deja la grabacion reproducible; si tampoco se puede, al menos
/// sale el aviso de que no va a venir.
///
/// **Y no se puede hacer antes de abrir el editor.** Medido el 29 de agosto de 2026:
/// Media Foundation tarda unos 26 ms por fotograma a 1080p, asi que treinta segundos son
/// doce segundos de espera mirando una ventana que no llega. Se abre el editor con lo que
/// ya esta (que se ve, se recorta y se exporta igual) y cuando el video esta listo se avisa.
///
/// El aviso sale **pase lo que pase**, tambien cuando falla: un editor esperando para
/// siempre a algo que no viene es peor que un editor que dice que no va a venir.
/// Y el fallo, ademas de avisarse, se apunta (`previa_fallida`).
pub(crate) fn preparar_por_detras(app: &AppHandle, id: String) {
    olvidar_previa_fallida(&id);
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let Some(mut session) = state.sessions.read().get(&id).cloned() else {
            return;
        };
        // El porcentaje se manda cada cinco, no en cada fotograma: son novecientos, y
        // novecientos avisos para mover un numero de sitio es trabajo tirado.
        let mut ultimo = 0;
        let avisador = |hechos: usize, total: usize| {
            let ahora = (hechos * 100 / total.max(1)) as u32;
            if ahora >= ultimo + 5 {
                ultimo = ahora;
                let _ = app.emit(crate::EVENT_SESSION_PREVIEW, AvisoVistaPrevia::yendo(&id, ahora));
            }
        };
        if let Err(error) = escribir_vista_previa(&mut session, avisador) {
            // Sin vista previa la sesion sigue siendo buena: se ve y se exporta igual, solo
            // que sin reproducir. Mucho mejor eso que perder lo que se acaba de rescatar.
            eprintln!("[replay] sin vista previa: {error}");
            apuntar_previa_fallida(&id);
            let _ = app.emit(crate::EVENT_SESSION_PREVIEW, AvisoVistaPrevia::fallida(&id));
            return;
        }
        let _ = session.persist();
        state.sessions.write().insert(id.clone(), session.clone());
        eprintln!("[replay] vista previa lista: {}", ruta_de(&session));
        let _ = app.emit(crate::EVENT_SESSION_PREVIEW, AvisoVistaPrevia::lista(&id));

        // Y ahora la tira. Lo que viene de una grabacion normal ya las trae hechas, asi
        // que no se repiten: son cuatro milisegundos por fotograma tirados.
        if session.frames.iter().any(|f| f.thumb_path.is_empty()) {
            match record::generate_thumbnails(&mut session) {
                Ok(()) => {
                    let _ = session.persist();
                    state.sessions.write().insert(id.clone(), session);
                    crate::recorder::avisar_de_la_tira(&app, &id);
                }
                // Sin miniaturas la tira ensenna huecos, pero el video y la exportacion
                // van igual: no es motivo para tirar nada.
                Err(error) => eprintln!("[replay] sin miniaturas para la tira: {error}"),
            }
        }
    });
}

/// Donde ha quedado el video, para decirlo en el log.
fn ruta_de(session: &SessionData) -> String {
    session
        .mp4_path
        .as_ref()
        .map(|r| r.display().to_string())
        .unwrap_or_else(|| "sin ruta".into())
}

/// Como va la vista previa que se esta escribiendo por detras.
///
/// Lleva el resultado dentro a proposito: el editor tiene un boton apagado esperando este
/// aviso, y si solo se avisara al ir bien, un fallo lo dejaria apagado y callado para
/// siempre. Esa fue justo la queja de Munir el 29 de agosto de 2026.
///
/// Y lleva el porcentaje porque la espera existe: once segundos para medio minuto de
/// anillo. Once segundos con un numero subiendo se aguantan; once segundos mirando un
/// boton que no responde, no.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvisoVistaPrevia {
    pub session_id: String,
    pub por_ciento: u32,
    pub listo: bool,
    pub fallida: bool,
}

impl AvisoVistaPrevia {
    fn yendo(id: &str, por_ciento: u32) -> Self {
        Self {
            session_id: id.to_string(),
            por_ciento,
            listo: false,
            fallida: false,
        }
    }

    fn lista(id: &str) -> Self {
        Self {
            session_id: id.to_string(),
            por_ciento: 100,
            listo: true,
            fallida: false,
        }
    }

    fn fallida(id: &str) -> Self {
        Self {
            session_id: id.to_string(),
            por_ciento: 0,
            listo: false,
            fallida: true,
        }
    }
}

/// Todo el trabajo de convertir el anillo en una sesion: coser los fotogramas, cortar el
/// sonido y poner los relojes a cero. Las miniaturas NO: se sacan despues de abrir el
/// editor, en `preparar_por_detras`, que son cuatro milisegundos por fotograma y aqui
/// se pagarian con la ventana todavia sin salir.
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

    let session = SessionData {
        id,
        dir,
        region: encargo.region,
        fps: encargo.fps,
        format: "mp4".into(),
        has_audio: audio.is_some(),
        width: encargo.region.width,
        height: encargo.region.height,
        // Se rellena al final, en `escribir_vista_previa`: hasta que no estan cosidos los
        // fotogramas no hay nada que codificar.
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
        formas: Vec::new(),
        punteros: Vec::new(),
        cambios_puntero: Vec::new(),
        marcas_zoom: Vec::new(),
        camara: None,
        escritura: Vec::new(),
        pistas: None,
        frames,
    };
    #[cfg(target_os = "linux")]
    let session = {
        let mut session = session;
        if let Some(rastro) = encargo.rastro {
            poner_el_rastro(&mut session, rastro, t0);
        }
        if let Some(pistas) = encargo.pistas {
            poner_las_pistas(&mut session, pistas, duracion);
        }
        session
    };
    session.persist()?;
    Ok(session)
}

/// Pone en la sesion el raton como lo apunta la captura de Linux: por donde fue, que puntero
/// llevaba y las imagenes de esos punteros, en disco, que es de donde las lee el exportador.
#[cfg(target_os = "linux")]
fn poner_el_rastro(session: &mut SessionData, rastro: RastroDelAnillo, t0: u64) {
    session.cursor = desde_el_corte(rastro.cursor, t0, |punto| punto.0, |punto, ms| punto.0 = ms);
    session.cambios_puntero = desde_el_corte(rastro.cambios_puntero, t0, |cambio| cambio.0, |cambio, ms| cambio.0 = ms);
    for (id, (caliente, imagen)) in rastro.punteros.into_iter().enumerate() {
        let id = id as u32;
        // Solo los que salen en lo guardado. Uno que no se pueda escribir se queda fuera y
        // en su tramo se dibuja la flecha.
        if !session.cambios_puntero.iter().any(|(_, suyo)| *suyo == id) {
            continue;
        }
        let archivo = session.dir.join(format!("cursor-{id}.png"));
        if crate::encode::png::save_fast(&imagen, &archivo).is_ok() {
            session.punteros.push(record::PunteroGrabado {
                id,
                ancho: imagen.width(),
                alto: imagen.height(),
                caliente,
                archivo: archivo.to_string_lossy().to_string(),
            });
        }
    }
}

/// Lo que cae dentro de lo guardado, con el reloj a cero, y delante lo ultimo de antes, que
/// es lo que valia cuando empieza el video: un raton que llevaba un rato quieto sigue
/// estando en su sitio, con el puntero que tuviera.
#[cfg(target_os = "linux")]
fn desde_el_corte<T>(apuntes: Vec<T>, t0: u64, leer: impl Fn(&T) -> u64, escribir: impl Fn(&mut T, u64)) -> Vec<T> {
    let de_antes = apuntes.iter().rposition(|apunte| leer(apunte) < t0);
    apuntes
        .into_iter()
        .enumerate()
        .filter(|(i, apunte)| leer(apunte) >= t0 || Some(*i) == de_antes)
        .map(|(_, mut apunte)| {
            let ms = leer(&apunte).saturating_sub(t0);
            escribir(&mut apunte, ms);
            apunte
        })
        .collect()
}

/// Escribe las pistas de Linux (el PC y la voz, cada una en su WAV) con lo que acompanna a la
/// imagen. Se corta por el FINAL, como el sonido de Windows: el tramo y los fotogramas acaban
/// en el mismo instante.
#[cfg(target_os = "linux")]
fn poner_las_pistas(session: &mut SessionData, tramo: crate::record::linux::sonido::TramoDeSonido, duracion_ms: u64) {
    use crate::record::linux::sonido::{cerrar_pistas, FORMATO};
    use crate::record::pistas::{EscritorWav, ARCHIVO_MICROFONO, ARCHIVO_PC};

    let escribir = |pcm: Option<Vec<u8>>, nombre: &str| -> Option<EscritorWav> {
        let pcm = pcm?;
        let quiero = (duracion_ms * FORMATO.bytes_por_ms()) as usize;
        let bloque = usize::from(FORMATO.channels) * 2;
        let desde = pcm.len().saturating_sub(quiero) / bloque * bloque;
        let mut wav = EscritorWav::crear(&session.dir.join(nombre), FORMATO)
            .map_err(|error| eprintln!("[replay] no se puede escribir {nombre}: {error}"))
            .ok()?;
        wav.escribir(&pcm[desde..]);
        Some(wav)
    };
    let pc = escribir(tramo.sistema, ARCHIVO_PC);
    let voz = escribir(tramo.microfono, ARCHIVO_MICROFONO);
    (session.audio, session.pistas) = cerrar_pistas(pc, voz);
    session.has_audio = session.audio.is_some();
}

/// Lo alto que se escribe la vista previa, como mucho.
///
/// Se ve en un hueco de unos 750 px de ancho dentro de una ventana; escribirla a 1080
/// era escribir cuatro veces mas pixeles de los que se llegan a mirar.
///
/// **Y 540 en vez de 720 porque el play esta apagado hasta que esto termina.** Medido el
/// 16 de octubre de 2026 en la maquina de Munir, con los 900 fotogramas que salen de su
/// anillo (treinta segundos a 60, servidos a 30): a 720 son 9,8 segundos de espera y a
/// 540 son 5,3. En el hueco del editor, que no llega a 750 px de ancho, no se distingue
/// uno de otro; lo que se exporta no pasa por aqui y sigue saliendo a tamanno completo.
const ALTO_VISTA_PREVIA: u32 = 540;

/// Y a cuantos fotogramas por segundo, como mucho.
const FPS_VISTA_PREVIA: u32 = 30;

/// Los fotogramas que entran en la vista previa, a un ritmo de como mucho `fps`.
///
/// Lo que se salta no se pierde: su duracion se le suma al fotograma que se queda, asi que
/// el video dura exactamente lo mismo y los relojes del editor siguen cuadrando.
///
/// Va aparte y sin tocar nada para poder probarlo: lo unico que hay que comprobar de esto
/// es que la suma de las duraciones no cambie, y eso no necesita ni pantalla ni codificador.
fn a_ritmo(duraciones_origen: &[u32], fps: u32) -> (Vec<usize>, Vec<u32>) {
    let paso = (1000 / fps.max(1)).max(1);
    let mut indices: Vec<usize> = Vec::new();
    let mut duraciones: Vec<u32> = Vec::new();
    // El reloj de lo grabado y el instante en que toca guardar el siguiente. El objetivo
    // avanza de paso en paso y NO se recoloca sobre el reloj: asi el reparto no se va
    // acumulando el sobrante y de sesenta por segundo salen treinta, no veinte.
    let mut reloj: u32 = 0;
    let mut objetivo: u32 = 0;
    for (i, dura) in duraciones_origen.iter().copied().enumerate() {
        match duraciones.last_mut() {
            // Lo que cae entre dos pasos no se codifica: su tiempo se le suma al ultimo
            // que si entro, que es lo que mantiene la duracion del video clavada.
            Some(ultima) if reloj < objetivo => *ultima += dura,
            _ => {
                indices.push(i);
                duraciones.push(dura);
                objetivo += paso;
            }
        }
        reloj += dura;
    }
    (indices, duraciones)
}

/// Escribe el `preview.mp4` de la sesion, con su sonido si lo hay.
///
/// Va por el mismo camino que exportar un MP4: los fotogramas se leen del cache cosido y
/// los escribe Media Foundation por hardware.
///
/// **Pero no a tamanno completo.** Esto no es lo que se lleva nadie (exportar vuelve al
/// cache y codifica de cero, a la calidad que se pida): es lo que se mira en la ventana
/// mientras se recorta. Treinta segundos de un anillo a 1080p y 60 por segundo salian 72 MB
/// y tardaban veintiun segundos en escribirse, veintiun segundos con el boton de play
/// apagado. A 720p y 30 por segundo se ve igual de bien en el hueco del editor y esta listo
/// en un tercio del tiempo.
///
/// En Linux es el mismo camino, con el GStreamer del sistema en vez de Media Foundation.
#[cfg(any(windows, target_os = "linux"))]
fn escribir_vista_previa(
    session: &mut SessionData,
    mut avisar: impl FnMut(usize, usize),
) -> Result<()> {
    let ruta = session.dir.join("preview.mp4");
    let duraciones: Vec<u32> = session.frames.iter().map(|f| f.duration_ms).collect();
    let (indices, retardos) = a_ritmo(&duraciones, FPS_VISTA_PREVIA);
    let (ancho, alto) =
        crate::encode::escalar::medida_para(session.width, session.height, ALTO_VISTA_PREVIA)
            .unwrap_or((session.width, session.height));

    // S11 (pistas): con el PC y la voz en dos archivos, la vista previa lleva los dos
    // sumados, como la que se escribe en vivo; lo de antes es su `audio.pcm` tal cual.
    let sonido = session.audio.and_then(|info| {
        record::pistas::mezcla_entera(session).map(|datos| crate::encode::mp4::Pista {
            channels: info.channels,
            sample_rate: info.sample_rate,
            datos,
        })
    });
    // En Linux puede faltar el codificador de AAC. La vista previa sale entonces muda en vez
    // de no salir: alli el editor hace sonar las pistas por su cuenta, y sin vista previa no
    // habria ni boton de reproducir.
    #[cfg(target_os = "linux")]
    let sonido = sonido.filter(|_| crate::encode::gst::codificadores().sonido.is_some());

    // En orden y sin volver a empezar: los fotogramas se piden del primero al ultimo, que
    // es justo el caso para el que existe `LectorEnOrden`. El encogido va aqui y no dentro
    // del codificador porque el suyo es un Lanczos que cuesta 46 ms por fotograma, diez
    // veces mas que el filtro de caja de `reducir`.
    //
    // **Y se lee en otro hilo, dos fotogramas por delante del codificador.** Leer uno
    // (descomprimir el QOI, pegar sus zonas, encogerlo) y codificarlo son dos trabajos
    // que no se pisan: mientras Media Foundation mastica uno, aqui ya esta el siguiente.
    // Medido el 17 de septiembre de 2026 sobre el anillo de verdad: de 37 a 21 ms por
    // fotograma, y un rescate de treinta segundos a 60 fps son novecientos.
    let mut lector = record::LectorPorDelante::nuevo_con(
        session.clone(),
        indices.clone(),
        move |fotograma| {
            if fotograma.dimensions() == (ancho, alto) {
                fotograma
            } else {
                crate::encode::escalar::reducir(&fotograma, ancho, alto)
            }
        },
    );
    let mut cargar = |indice: usize| lector.en(indice);
    let resultado = crate::encode::mp4::encode(
        &indices,
        &retardos,
        &mut cargar,
        &ruta,
        &crate::encode::mp4::Mp4Options {
            width: ancho,
            height: alto,
            fps: session.fps.clamp(1, FPS_VISTA_PREVIA),
            // La vista previa no es el archivo que se lleva nadie: se mira y se recorta.
            // Setenta y cinco es lo mismo que usa la grabacion normal para lo suyo.
            quality: 75,
        },
        sonido,
        |_, hechos, total| avisar(hechos, total),
    );
    resultado?;
    session.mp4_path = Some(ruta);
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn escribir_vista_previa(
    _session: &mut SessionData,
    _avisar: impl FnMut(usize, usize),
) -> Result<()> {
    Err(AppError::Unsupported)
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
            Anillo::nuevo(&raiz.join("anillo"), 10_000, 30, crate::record::buffer::bytes_max(10, 30, 1920, 1080))
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
        // Y SIN miniaturas: montar ya no las hace, porque son cuatro milisegundos por
        // fotograma con el editor sin abrir todavia. Las saca `preparar_por_detras`
        // cuando la ventana ya esta delante, que es justo lo que se comprueba aqui.
        assert!(
            session.frames[0].thumb_path.is_empty(),
            "montar ha vuelto a sacar las miniaturas: eso retrasa la ventana"
        );
        let mut con_tira = session.clone();
        record::generate_thumbnails(&mut con_tira).expect("las miniaturas tienen que salir");
        assert!(
            std::path::Path::new(&con_tira.frames[0].thumb_path).exists(),
            "la miniatura del primer fotograma no llego al disco"
        );

        anillo.limpiar();
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// **Una grabacion corta tambien tiene vista previa.**
    ///
    /// Munir, 17 de octubre de 2026: el editor le decia «Sin vista previa» con una
    /// grabacion de verdad. Eso solo sale cuando `escribir_vista_previa` falla, asi que
    /// aqui se prueba con lo que mas facil se rompe: una sesion de un solo fotograma, una
    /// de dos, y una con sonido.
    #[test]
    fn la_vista_previa_sale_aunque_la_grabacion_sea_cortisima() {
        let (ancho, alto) = (64u32, 48u32);
        for cuantos in [1usize, 2, 5] {
            let dir = std::env::temp_dir().join(format!("winshotx-previa-corta-{cuantos}"));
            let _ = std::fs::remove_dir_all(&dir);
            let mut cache = crate::record::FrameCache::new(&dir).unwrap();
            for i in 0..cuantos {
                let frame = pantalla(ancho, alto, i as u32);
                cache.push_rgba(&frame, ancho, alto, i as u64 * 33).unwrap();
            }
            let frames = cache.finish(cuantos as u64 * 33, 30).unwrap();
            let mut session = SessionData {
                id: format!("corta-{cuantos}"),
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
                pistas: None,
                frames,
            };
            let salida = escribir_vista_previa(&mut session, |_, _| {});
            assert!(
                salida.is_ok(),
                "con {cuantos} fotogramas la vista previa ha fallado: {:?}",
                salida.err()
            );
            let ruta = session.mp4_path.clone().expect("no ha quedado ruta de video");
            assert!(ruta.exists(), "el mp4 de {cuantos} fotogramas no llego al disco");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// **Y con sonido tambien, que es como graba Munir.**
    ///
    /// 17 de octubre de 2026: el editor se quedaba en «preparando el video… 100%» para
    /// siempre con una grabacion de 74x40 CON audio del sistema y microfono. El 100% es
    /// el ultimo fotograma mandado, asi que lo que se quedaba colgado era el cierre.
    /// Esta prueba corre el camino en otro hilo y se planta si tarda mas de medio minuto.
    #[test]
    fn la_vista_previa_con_sonido_no_se_queda_colgada() {
        use std::sync::mpsc;
        use std::time::Duration;

        let (ancho, alto) = (74u32, 40u32);
        let dir = std::env::temp_dir().join("winshotx-previa-con-sonido");
        let _ = std::fs::remove_dir_all(&dir);
        // Trece fotogramas en tres segundos: es lo que deja una pantalla quieta, donde
        // solo cambia un parpadeo al principio y el ultimo fotograma dura dos segundos y
        // medio. Esa fue la grabacion de Munir que dejo el editor en «100%».
        let mut cache = crate::record::FrameCache::new(&dir).unwrap();
        for i in 0..13u32 {
            let frame = pantalla(ancho, alto, i);
            cache.push_rgba(&frame, ancho, alto, u64::from(i) * 33).unwrap();
        }
        let frames = cache.finish(3_000, 30).unwrap();

        // Tres segundos de sonido de verdad: 48 kHz, dos canales, PCM de 16 bits.
        let info = crate::record::AudioInfo { channels: 2, sample_rate: 48_000 };
        let muestras = 48_000 * 3;
        let mut pcm = Vec::with_capacity(muestras * 4);
        for n in 0..muestras {
            let v = ((n as f32 / 40.0).sin() * 8_000.0) as i16;
            pcm.extend_from_slice(&v.to_le_bytes());
            pcm.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(dir.join("audio.pcm"), &pcm).unwrap();

        let session = SessionData {
            id: "con-sonido".into(),
            dir: dir.clone(),
            region: crate::capture::Rect { x: 0, y: 0, width: ancho, height: alto },
            fps: 30,
            format: "mp4".into(),
            has_audio: true,
            width: ancho,
            height: alto,
            mp4_path: None,
            audio: Some(info),
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
            pistas: None,
            frames,
        };

        let (avisa, espera) = mpsc::channel();
        std::thread::spawn(move || {
            let mut suya = session;
            let salida = escribir_vista_previa(&mut suya, |_, _| {});
            let _ = avisa.send(salida.map(|()| suya.mp4_path.clone()));
        });
        match espera.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(Some(ruta))) => assert!(ruta.exists(), "no ha llegado el mp4 al disco"),
            Ok(Ok(None)) => panic!("ha terminado sin dejar ruta de video"),
            Ok(Err(error)) => panic!("la vista previa con sonido ha fallado: {error}"),
            Err(_) => panic!(
                "la vista previa con sonido se ha quedado colgada: eso es el «100%» que no avanza"
            ),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// La vista previa de una sesion DE VERDAD, de las que quedan en el disco.
    ///
    /// `WINSHOTX_SESION=C:\...\Temp\winshotx\sessions\xxxx cargo test --release --lib
    /// la_vista_previa_de_una_sesion_del_disco -- --ignored --nocapture`
    ///
    /// Existe porque el 17 de octubre de 2026 el editor se quedo en «100%» con una
    /// grabacion de Munir y ninguna sesion inventada lo reproducia. Con la suya delante,
    /// o se cuelga aqui tambien o el problema no esta en escribir el video.
    #[test]
    #[ignore = "necesita una sesion de verdad en WINSHOTX_SESION"]
    fn la_vista_previa_de_una_sesion_del_disco() {
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        let dir = std::env::var("WINSHOTX_SESION").expect("falta WINSHOTX_SESION");
        let dir = std::path::PathBuf::from(dir);
        let texto = std::fs::read_to_string(dir.join("session.json")).expect("sin session.json");
        let mut session: SessionData = serde_json::from_str(&texto).expect("session.json ilegible");
        // La carpeta puede haberse movido: manda la que se pasa.
        session.dir = dir.clone();
        println!(
            "sesion {}: {}x{}, {} fotogramas, {} ms, audio={:?}",
            session.id,
            session.width,
            session.height,
            session.frames.len(),
            session.duration_ms(),
            session.audio
        );

        let (avisa, espera) = mpsc::channel();
        let reloj = Instant::now();
        std::thread::spawn(move || {
            let mut suya = session;
            let salida = escribir_vista_previa(&mut suya, |hechos, total| {
                if hechos % 50 == 0 || hechos + 1 == total {
                    println!("  {hechos}/{total}");
                }
            });
            let _ = avisa.send(salida.map(|()| suya.mp4_path.clone()));
        });
        match espera.recv_timeout(Duration::from_secs(60)) {
            Ok(Ok(ruta)) => println!("ha terminado en {:?}: {ruta:?}", reloj.elapsed()),
            Ok(Err(error)) => panic!("ha fallado: {error}"),
            Err(_) => panic!("SE HA COLGADO: un minuto sin terminar, que es el «100%» que no avanza"),
        }
    }

    /// **Lo que se espera con el play apagado, partido en sus dos trozos.**
    ///
    /// `cargo test --release --lib medir_la_vista_previa -- --ignored --nocapture`
    ///
    /// Rescatar los ultimos segundos abre el editor sin video y lo escribe por detras: ese
    /// rato el boton de play no hace nada. Aqui se mide cuanto cuesta LEER del cache (QOI,
    /// las zonas del delta y el encogido) y cuanto CODIFICAR, porque solo el trozo gordo
    /// merece que se le toque.
    #[test]
    #[ignore = "mide, no comprueba: pide release y maquina tranquila"]
    fn medir_la_vista_previa() {
        use std::time::Instant;

        let (ancho, alto, cuantos) = (1920u32, 1080u32, 900u32);
        let dir = std::env::temp_dir().join("winshotx-bench-vista-previa");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cache = crate::record::FrameCache::new(&dir).unwrap();
        for i in 0..cuantos {
            let frame = pantalla(ancho, alto, i);
            cache.push_rgba(&frame, ancho, alto, u64::from(i) * 33).unwrap();
        }
        let frames = cache.finish(u64::from(cuantos) * 33, 30).unwrap();
        println!("\n[cache] {} fotogramas de {ancho}x{alto}", frames.len());

        let mut session = SessionData {
            id: "bench-previa".into(),
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
            pistas: None,
            frames,
        };

        // Primero el trozo de leer, solo: los mismos fotogramas que pedira la vista previa,
        // leidos y encogidos, sin codificar nada.
        let duraciones: Vec<u32> = session.frames.iter().map(|f| f.duration_ms).collect();
        let (indices, _) = a_ritmo(&duraciones, FPS_VISTA_PREVIA);
        let (w, h) = crate::encode::escalar::medida_para(ancho, alto, ALTO_VISTA_PREVIA)
            .unwrap_or((ancho, alto));
        let mut lector = record::LectorPorDelante::nuevo_con(
            session.clone(),
            indices.clone(),
            move |f| {
                if f.dimensions() == (w, h) { f } else { crate::encode::escalar::reducir(&f, w, h) }
            },
        );
        let reloj = Instant::now();
        for i in &indices {
            let _ = lector.en(*i).unwrap();
        }
        let leer = reloj.elapsed().as_secs_f64() * 1000.0;
        drop(lector);
        println!(
            "[leer] {} fotogramas a {w}x{h}: {leer:.0} ms ({:.1} ms/fotograma)",
            indices.len(),
            leer / indices.len() as f64
        );

        // Y ahora el camino entero, que es lo que de verdad se espera.
        let reloj = Instant::now();
        escribir_vista_previa(&mut session, |_, _| {}).unwrap();
        let entero = reloj.elapsed().as_secs_f64() * 1000.0;
        let bytes = std::fs::metadata(dir.join("preview.mp4")).map(|m| m.len()).unwrap_or(0);
        println!(
            "[entero] {entero:.0} ms ({:.1} ms/fotograma), {:.1} MB",
            entero / indices.len() as f64,
            bytes as f64 / 1_048_576.0
        );
        let _ = std::fs::remove_dir_all(&dir);
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

    /// Cuánto dice durar un MP4, leído de su cabecera `mvhd`.
    ///
    /// A mano y no con una biblioteca: son doce bytes en un sitio fijo, y meter un lector
    /// de MP4 entero en el instalador para leer un número sería pagar de más.
    fn duracion_del_mp4(ruta: &std::path::Path) -> Option<u64> {
        let bytes = std::fs::read(ruta).ok()?;
        let pos = bytes.windows(4).position(|v| v == b"mvhd")?;
        // Detrás de «mvhd»: versión (1), banderas (3), creación (4), modificación (4),
        // escala de tiempo (4) y duración (4). Solo se lee la versión 0, que es la que
        // escribe Media Foundation.
        let leer = |desde: usize| -> Option<u64> {
            let trozo = bytes.get(desde..desde + 4)?;
            Some(u64::from(u32::from_be_bytes(trozo.try_into().ok()?)))
        };
        if bytes.get(pos + 4)? != &0 {
            return None;
        }
        let escala = leer(pos + 16)?;
        let duracion = leer(pos + 20)?;
        (escala > 0).then(|| duracion * 1000 / escala)
    }

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
            crate::record::buffer::bytes_max(3, fps, region.width, region.height),
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
                start: Instant::now(),
                reciclados: win::Reciclados::default(),
                solo_cambios: false,
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
                    let mut rgba = frame.bgra;
                    crate::recorder::bgra_a_rgba_en_sitio(&mut rgba);
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
        let empezo = Instant::now();
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

        // El numero que decide si esto se puede hacer antes de abrir el editor o hay que
        // hacerlo por detras: cuanto se queda esperando quien acaba de pulsar la tecla.
        println!(
            "guardados {} fotogramas, {} ms de video · cosechar tardo {} ms",
            session.frames.len(),
            session.duration_ms(),
            empezo.elapsed().as_millis()
        );
        // La vista previa se escribe por detrás, con el editor ya abierto, así que aquí
        // se comprueba a mano lo que en la app pasa en otro hilo.
        let antes = Instant::now();
        let mut con_video = session.clone();
        escribir_vista_previa(&mut con_video, |_, _| {}).expect("no se ha podido escribir la vista previa");
        let ruta = con_video.mp4_path.as_ref().expect("no ha quedado ruta de vídeo");
        println!(
            "vista previa: {} KB en {} ms ({} ms por fotograma)",
            std::fs::metadata(ruta).map(|m| m.len() / 1024).unwrap_or(0),
            antes.elapsed().as_millis(),
            antes.elapsed().as_millis() / session.frames.len().max(1) as u128
        );
        assert!(ruta.exists(), "el vídeo de vista previa no llegó al disco");
        // Y que dure lo que dura la sesión, leído del propio archivo. Un MP4 de cero
        // segundos existe, pesa y no reproduce nada: es justo el fallo que se está
        // arreglando aquí, y comprobar que el archivo está no lo vería.
        let dentro = duracion_del_mp4(ruta).expect("el MP4 no dice cuánto dura");
        println!("el vídeo dice durar {dentro} ms");
        assert!(
            dentro.abs_diff(session.duration_ms()) < 500,
            "el vídeo dura {dentro} ms y la sesión {} ms",
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

    /// Bajar el ritmo de la vista previa no puede acortar el vídeo.
    ///
    /// Es lo único que hay que comprobar de `a_ritmo`, y es justo lo que rompería los
    /// relojes del editor: los fotogramas que se saltan tienen que dejarle su tiempo al
    /// que se queda, no llevárselo.
    #[test]
    fn saltarse_fotogramas_no_acorta_el_video() {
        // Un segundo grabado a sesenta por segundo.
        let sesenta: Vec<u32> = std::iter::repeat_n(16, 60).collect();
        let (indices, retardos) = a_ritmo(&sesenta, 30);

        assert_eq!(
            retardos.iter().sum::<u32>(),
            sesenta.iter().sum::<u32>(),
            "el vídeo ha cambiado de duración al bajar el ritmo"
        );
        assert_eq!(indices.len(), retardos.len());
        assert!(
            (28..=32).contains(&indices.len()),
            "de 60 fotogramas a 30 por segundo tienen que quedar unos 30, no {}",
            indices.len()
        );
        // Y en orden y sin repetir, que es lo que espera el lector del caché.
        assert!(indices.windows(2).all(|par| par[0] < par[1]));
    }

    /// Lo que ya va lento se queda como está: pedir 30 a algo grabado a 15 no puede tirar
    /// uno de cada dos y dejar la vista previa a trompicones.
    #[test]
    fn a_quince_por_segundo_no_se_tira_ninguno() {
        let quince: Vec<u32> = std::iter::repeat_n(66, 15).collect();
        let (indices, retardos) = a_ritmo(&quince, 30);

        assert_eq!(indices.len(), 15, "no había nada que saltarse");
        assert_eq!(retardos, quince);
    }

    /// Un guardia con los dos relojes a mano, sin necesitar ni anillo ni aplicación.
    fn apartado_de_prueba(desde: Instant) -> (Apartado, Arc<AtomicBool>, Arc<AtomicU64>) {
        let pause = Arc::new(AtomicBool::new(true));
        let paused_ms = Arc::new(AtomicU64::new(0));
        (
            Apartado(Some((pause.clone(), paused_ms.clone(), desde))),
            pause,
            paused_ms,
        )
    }

    #[test]
    fn apartado_el_anillo_no_captura_y_al_soltarlo_vuelve() {
        let (guardia, pause, _) = apartado_de_prueba(Instant::now());
        assert!(pause.load(Ordering::Relaxed), "apartado tiene que estar parado");

        drop(guardia);
        assert!(
            !pause.load(Ordering::Relaxed),
            "soltado el guardia el anillo vuelve a grabar; si no, se queda parado para              siempre y nadie se entera hasta que va a rescatar algo y no hay nada"
        );
    }

    /// El agujero que este arreglo NO puede dejar.
    ///
    /// La marca de tiempo de cada fotograma es `elapsed - paused_ms` (ver `record::win`).
    /// Si el rato apartado no se descontara, el fotograma siguiente a una captura saltaría
    /// hacia adelante justo lo que durase, y quien rescatara los últimos segundos vería un
    /// tirón. Descontándolo, lo de antes y lo de después quedan pegados.
    #[test]
    fn el_rato_apartado_no_cuenta_en_el_reloj_de_lo_grabado() {
        // Nace con 300 ms de antigüedad: es lo que va a durar la captura.
        let (guardia, _, paused_ms) =
            apartado_de_prueba(Instant::now() - std::time::Duration::from_millis(300));
        let antes = 1_000u64.saturating_sub(paused_ms.load(Ordering::Relaxed));

        drop(guardia);

        // El reloj de la máquina ha seguido corriendo esos 300 ms, y el del anillo no.
        let despues = 1_300u64.saturating_sub(paused_ms.load(Ordering::Relaxed));
        assert!(
            despues.abs_diff(antes) <= 20,
            "el fotograma de después tiene que caer donde se quedó el de antes ({antes} ms),              y ha caído en {despues}"
        );
    }

    #[test]
    fn sin_anillo_encendido_el_guardia_vale_igual_y_no_revienta() {
        // `apartar` devuelve esto cuando no hay nada que apartar: quien llama no tiene que
        // preguntar si el anillo está puesto.
        drop(Apartado(None));
    }
}

/// Lo que el anillo hace solo en Linux: las dos pistas de sonido y el raton que apunta la
/// captura. El resto del camino es el de Windows y lo prueban sus pruebas.
#[cfg(all(test, target_os = "linux"))]
mod pruebas_de_linux {
    use super::*;
    use crate::record::buffer::{bytes_max, ImagenDelAnillo};
    use crate::record::linux::sonido::{TramoDeSonido, FORMATO};

    /// Una pantalla de mentira con algo que se mueve, ya en RGBA, como la da la captura.
    fn pantalla(ancho: u32, alto: u32, paso: u32) -> Vec<u8> {
        let mut frame: Vec<u8> = [20u8, 20, 20, 255].repeat((ancho * alto) as usize);
        let x0 = (paso * 2) % (ancho - 8);
        for y in 2..10u32 {
            for x in x0..x0 + 8 {
                let p = ((y * ancho + x) * 4) as usize;
                frame[p..p + 3].copy_from_slice(&[240, 60, 10]);
            }
        }
        frame
    }

    /// **El camino entero en Linux, sin la ventana delante.** Lo que llega de la captura
    /// entra en el anillo, se pulsa la tecla, y lo que sale es una sesion que el editor puede
    /// abrir: con las imagenes que se vuelven a dibujar, el sonido del PC en su pista y con
    /// el largo de la imagen, y el raton donde estaba, con su puntero en el disco.
    #[test]
    fn el_rescate_de_linux_sale_con_sus_pistas_y_su_raton() {
        let (ancho, alto) = (48u32, 32u32);
        let raiz = std::env::temp_dir().join(format!("winshotx-replay-linux-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let mut anillo = Anillo::nuevo(&raiz.join("anillo"), 10_000, 15, bytes_max(10, 15, 1920, 1080)).unwrap();
        let mut imagen = ImagenDelAnillo::nueva(ancho, alto, None);
        // Doce segundos moviendose y ocho con la pantalla quieta, en los que no llega nada.
        for paso in 0..48u32 {
            imagen.recibir_rgba(pantalla(ancho, alto, paso));
            imagen.guardar(&mut anillo, u64::from(paso) * 250).unwrap();
        }
        let ahora = 20_000u64;
        let (segmentos, corte, copia) = anillo.instantanea(ahora).unwrap();

        // El tramo de sonido que da el anillo: la ventana y dos segundos mas, acabando ahora.
        // Los dos primeros segundos (los que sobran) van en silencio, para ver que se corta
        // por delante y no por detras.
        let por_ms = FORMATO.bytes_por_ms() as usize;
        let mut pc = vec![0u8; 2_000 * por_ms];
        pc.extend([0x10u8, 0x20].repeat(10_000 * por_ms / 2));

        let dibujo = |color: u8| image::RgbaImage::from_pixel(4, 6, image::Rgba([color, color, color, 255]));
        let encargo = Encargo {
            segmentos,
            corte,
            ahora,
            copia,
            sonido: None,
            clics: vec![
                crate::encode::zoom::Clic { ms: 2_000, x: 5, y: 5, derecho: false },
                crate::encode::zoom::Clic { ms: 15_000, x: 20, y: 10, derecho: true },
            ],
            teclas: Vec::new(),
            cursor: Vec::new(),
            rastro: Some(RastroDelAnillo {
                // El raton se movio por ultima vez un segundo antes de lo que se guarda.
                cursor: vec![(1_000, 1, 1), (9_000, 5, 6), (16_000, 30, 20)],
                punteros: vec![((0, 0), dibujo(1)), ((1, 2), dibujo(2)), ((3, 3), dibujo(3))],
                cambios_puntero: vec![(0, 0), (8_000, 1)],
            }),
            pistas: Some(TramoDeSonido { sistema: Some(pc), microfono: None }),
            region: Rect { x: 0, y: 0, width: ancho, height: alto },
            fps: 15,
            con_cursor: false,
        };

        let session = montar(raiz.join("sesion"), "prueba".into(), encargo).unwrap();

        let duracion = session.duration_ms();
        assert_eq!(duracion, 10_000, "se guardan los diez segundos pedidos, aunque la pantalla llevara ocho quieta");
        assert_eq!(session.frames[0].timestamp_ms, 0);
        assert_eq!(session.clics, vec![crate::encode::zoom::Clic { ms: 5_000, x: 20, y: 10, derecho: true }]);

        // El raton: donde estaba al empezar (lo ultimo de antes, en el instante cero) y por
        // donde fue despues. Y el puntero que llevaba, que es el que se escribe en el disco.
        assert_eq!(session.cursor, vec![(0, 5, 6), (6_000, 30, 20)]);
        assert_eq!(session.cambios_puntero, vec![(0, 1)]);
        assert_eq!(session.punteros.len(), 1, "solo el puntero que sale en lo guardado");
        let puntero = &session.punteros[0];
        assert_eq!((puntero.id, puntero.caliente, puntero.ancho, puntero.alto), (1, (1, 2), 4, 6));
        let leido = image::open(&puntero.archivo).expect("el puntero no llego al disco").to_rgba8();
        assert_eq!(leido, dibujo(2));

        // El sonido: la pista del PC, con el largo de la imagen y cortada por delante. La
        // voz no ha sonado y no tiene archivo.
        assert!(session.has_audio);
        assert_eq!(session.audio.map(|a| (a.channels, a.sample_rate)), Some((FORMATO.channels, FORMATO.sample_rate)));
        let pistas = session.pistas.as_ref().expect("el rescate sale con sus pistas");
        assert_eq!(pistas.sistema.as_deref(), Some(record::pistas::ARCHIVO_PC));
        assert_eq!((pistas.microfono.as_deref(), pistas.pico_sistema), (None, Some(0x2010)));
        let (info, datos) = record::pistas::leer_wav(&session.dir.join(record::pistas::ARCHIVO_PC)).expect("un WAV entero");
        assert_eq!((info.channels, info.sample_rate), (FORMATO.channels, FORMATO.sample_rate));
        assert_eq!(datos.len(), 10_000 * por_ms, "la pista mide lo que la imagen");
        assert_eq!((&datos[..2], &datos[datos.len() - 2..]), (&[0x10u8, 0x20][..], &[0x10u8, 0x20][..]));
        assert!(!session.dir.join(record::pistas::ARCHIVO_MICROFONO).exists());
        assert!(!session.audio_path().exists(), "en Linux el rescate no deja un audio.pcm mezclado");

        for indice in [0, session.frames.len() / 2, session.frames.len() - 1] {
            let dibujado = record::read_frame(&session, indice).unwrap();
            assert_eq!((dibujado.width(), dibujado.height()), (ancho, alto));
        }
        // Lo ultimo que se ve es lo ultimo que llego, que se quedo en pantalla hasta ahora.
        let ultimo = record::read_frame(&session, session.frames.len() - 1).unwrap().into_raw();
        assert!(ultimo == pantalla(ancho, alto, 47), "el ultimo fotograma no es la pantalla como se quedo");

        anillo.limpiar();
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Un rescate sin raton apuntado (un escritorio de Wayland que lo da ya dibujado) y sin
    /// sonido sale igual: sin punteros, sin pistas y sin decir que hay sonido.
    #[test]
    fn un_rescate_sin_raton_ni_sonido_no_se_inventa_nada() {
        let (ancho, alto) = (48u32, 32u32);
        let raiz = std::env::temp_dir().join(format!("winshotx-replay-linux-pelado-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let mut anillo = Anillo::nuevo(&raiz.join("anillo"), 10_000, 15, bytes_max(10, 15, 1920, 1080)).unwrap();
        let mut imagen = ImagenDelAnillo::nueva(ancho, alto, None);
        for paso in 0..12u32 {
            imagen.recibir_rgba(pantalla(ancho, alto, paso));
            imagen.guardar(&mut anillo, u64::from(paso) * 250).unwrap();
        }
        let (segmentos, corte, copia) = anillo.instantanea(3_000).unwrap();
        let encargo = Encargo {
            segmentos,
            corte,
            ahora: 3_000,
            copia,
            sonido: None,
            clics: Vec::new(),
            teclas: Vec::new(),
            cursor: Vec::new(),
            rastro: Some(RastroDelAnillo { cursor: Vec::new(), punteros: Vec::new(), cambios_puntero: Vec::new() }),
            // Los flujos se pidieron pero no sono ninguno: no hay tramo.
            pistas: None,
            region: Rect { x: 0, y: 0, width: ancho, height: alto },
            fps: 15,
            con_cursor: true,
        };
        let session = montar(raiz.join("sesion"), "pelado".into(), encargo).unwrap();
        assert_eq!(session.duration_ms(), 3_000, "todavia no habia diez segundos: se guarda lo que hay");
        assert!(session.cursor.is_empty() && session.punteros.is_empty() && session.cambios_puntero.is_empty());
        assert!(session.cursor_capturado);
        assert!(!session.has_audio && session.audio.is_none() && session.pistas.is_none());
        anillo.limpiar();
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_de_antes_del_corte_solo_deja_lo_ultimo() {
        let puntos = |lista: Vec<(u64, i32)>, t0| desde_el_corte(lista, t0, |p| p.0, |p, ms| p.0 = ms);
        assert_eq!(puntos(vec![(10, 1), (20, 2), (30, 3), (40, 4)], 25), vec![(0, 2), (5, 3), (15, 4)]);
        assert_eq!(puntos(vec![(10, 1), (20, 2)], 5), vec![(5, 1), (15, 2)], "sin nada antes, tal cual");
        assert_eq!(puntos(vec![(10, 1), (20, 2)], 90), vec![(0, 2)], "todo es de antes: queda lo ultimo");
        assert_eq!(puntos(Vec::new(), 90), Vec::new());
    }
}
