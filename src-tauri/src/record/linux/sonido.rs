//! El sonido en Linux: lo que suena por el altavoz y el microfono, por PipeWire.
//!
//! PipeWire es quien lleva el sonido en casi todos los escritorios de hoy (Fedora, Ubuntu
//! desde la 22.10, Debian 12, Arch), y la libreria ya esta en winshotx porque por ella
//! llegan tambien los fotogramas de Wayland. Se le piden dos flujos: uno de lo que sale por
//! el altavoz (el «monitor» de la salida, con `stream.capture.sink`) y otro del microfono.
//! Cada uno va a su WAV, los mismos archivos que en Windows (`record::pistas`), y de ahi en
//! adelante el editor y el exportador no notan la diferencia.
//!
//! **Donde PipeWire no lleva el sonido, se le pide a PulseAudio.** En Ubuntu 22.04 y Linux
//! Mint 21 PipeWire esta, pero solo para compartir pantalla: el sonido lo lleva PulseAudio, y
//! aqui los flujos no encontrarian a que engancharse. `quien_lo_lleva` lo decide cada vez:
//! si PipeWire tiene aparatos de sonido, es suyo y nada cambia; si no tiene ninguno, se graba
//! por PulseAudio (`sonido_pulse`). Si no hay de ninguno de los dos, se graba sin sonido, sin
//! colgarse y sin error, y `disponible` lo dice antes, para que la interfaz apague los
//! interruptores y explique por que.
//!
//! **Cada pista mide lo mismo que el video.** Lo que llega se coloca segun el reloj de la
//! grabacion: si un flujo empieza tarde o se queda un rato sin mandar nada, el hueco se
//! rellena de silencio, y al parar se rellena hasta el final. En pausa no se escribe.
//!
//! **Para «Grabar siempre lo ultimo» las pistas no van a un archivo** (que solo sabe crecer)
//! sino a la memoria, cada una en su anillo (`PistaEnAnillo`, `empezar_en_anillo`), colocadas
//! con el mismo reloj. Al rescatar se copia el tramo que acompanna a la imagen.

use std::cell::RefCell;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use pipewire as pw;
use pw::spa;
use serde::Serialize;
use spa::param::audio::{AudioFormat, AudioInfoRaw};
use spa::param::ParamType;
use spa::pod::serialize::PodSerializer;
use spa::pod::Pod;
use spa::utils::{Direction, SpaTypes};

use super::Marcha;
use crate::contrato::PistasGrabadas;
use crate::record::buffer::AnilloAudio;
use crate::record::pistas::{EscritorWav, ARCHIVO_MICROFONO, ARCHIVO_PC, PICO_CASI_MUDO};
use crate::record::AudioInfo;

/// En que formato se pide el sonido: el de un WAV de toda la vida. PipeWire convierte lo que
/// tenga cada aparato a esto.
pub const FORMATO: AudioInfo = AudioInfo { channels: 2, sample_rate: 48_000 };
const BYTES_POR_MARCO: usize = 4;

/// Cuanto puede ir una pista por detras del reloj antes de rellenar el hueco con silencio.
/// Los trozos llegan de veinte en veinte milisegundos y nunca del todo a su hora: por debajo
/// de esto es ese vaiven y no un hueco.
const HUECO_MS: u64 = 120;

const LATIDO: Duration = Duration::from_millis(20);

/// Lo que se espera a que PipeWire diga que aparatos de sonido tiene.
const ESPERA_DE_LA_LISTA: Duration = Duration::from_millis(1500);

/// Que sonido se puede grabar en este sistema.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Disponible {
    /// Hay una salida de sonido en PipeWire: se puede grabar lo que suena.
    pub altavoz: bool,
    /// Hay una entrada: se puede grabar el microfono.
    pub microfono: bool,
    /// El sonido lo lleva PulseAudio pero falta el plugin de GStreamer que lo graba (va en
    /// `gstreamer1.0-plugins-good`): no se ve ningun aparato, y los ajustes dicen que instalar.
    pub falta_el_de_pulse: bool,
}

/// Quien lleva el sonido en este sistema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motor {
    PipeWire,
    PulseAudio,
}

/// Quien lleva el sonido aqui y que hay para grabar. Donde PipeWire tiene aparatos de sonido
/// es suyo, como siempre, y a PulseAudio ni se le pregunta (en esos sistemas quien contesta
/// por PulseAudio es el propio PipeWire). Solo si PipeWire no tiene ninguno se mira PulseAudio.
pub(crate) fn quien_lo_lleva(en_pipewire: Disponible, en_pulse: impl FnOnce() -> Disponible) -> (Motor, Disponible) {
    if en_pipewire.altavoz || en_pipewire.microfono {
        return (Motor::PipeWire, en_pipewire);
    }
    let en_pulse = en_pulse();
    if en_pulse.altavoz || en_pulse.microfono {
        (Motor::PulseAudio, en_pulse)
    } else {
        // Nada en ninguno de los dos: lo que diga PulseAudio de por que (le falta el plugin).
        (Motor::PipeWire, en_pulse)
    }
}

/// Lo de `quien_lo_lleva`, preguntando de verdad a los dos.
fn mirar() -> (Motor, Disponible) {
    quien_lo_lleva(listar().unwrap_or_default(), super::sonido_pulse::disponible)
}

/// Mira que sonido se puede grabar: los aparatos que lleva PipeWire y, si PipeWire no lleva
/// ninguno (Ubuntu 22.04, Linux Mint 21), los de PulseAudio.
pub fn disponible() -> Disponible {
    let (listo, esperando) = std::sync::mpsc::channel();
    // En su hilo y con tope: si ninguno de los dos contesta, no se queda nadie esperando.
    std::thread::spawn(move || {
        let _ = listo.send(mirar().1);
    });
    esperando.recv_timeout(ESPERA_DE_LA_LISTA * 2).unwrap_or_default()
}

/// Que dice cada aparato de la lista de PipeWire: una salida, una entrada o ninguna de las dos.
pub(crate) fn clase_de_sonido(clase: &str) -> Disponible {
    Disponible {
        // «Audio/Duplex» es un aparato que hace las dos cosas (unos cascos con microfono).
        altavoz: clase == "Audio/Sink" || clase == "Audio/Duplex",
        microfono: clase == "Audio/Source" || clase == "Audio/Duplex" || clase == "Audio/Source/Virtual",
        falta_el_de_pulse: false,
    }
}

fn listar() -> Option<Disponible> {
    pw::init();
    let bucle = pw::main_loop::MainLoopRc::new(None).ok()?;
    let contexto = pw::context::ContextRc::new(&bucle, None).ok()?;
    let nucleo = contexto.connect_rc(None).ok()?;
    let registro = nucleo.get_registry().ok()?;
    let visto = Rc::new(RefCell::new(Disponible::default()));
    let acabado = Rc::new(std::cell::Cell::new(false));
    // La respuesta a esto llega cuando PipeWire ya ha contado todo lo que tiene.
    let pendiente = nucleo.sync(0).ok()?;
    let _del_nucleo = nucleo
        .add_listener_local()
        .done({
            let acabado = acabado.clone();
            let bucle = bucle.downgrade();
            move |id, numero| {
                if id == pw::core::PW_ID_CORE && numero == pendiente {
                    acabado.set(true);
                    if let Some(bucle) = bucle.upgrade() {
                        bucle.quit();
                    }
                }
            }
        })
        .register();
    let _del_registro = registro
        .add_listener_local()
        .global({
            let visto = visto.clone();
            move |cosa| {
                let Some(clase) = cosa.props.and_then(|p| p.get("media.class")) else {
                    return;
                };
                let esta = clase_de_sonido(clase);
                let mut visto = visto.borrow_mut();
                visto.altavoz |= esta.altavoz;
                visto.microfono |= esta.microfono;
            }
        })
        .register();
    // Si PipeWire no llega a contestar, este hilo no se queda aqui para siempre (quien
    // pregunta ya ha dejado de esperar): se da por visto lo que haya llegado.
    let tope = bucle.loop_().add_timer({
        let acabado = acabado.clone();
        let bucle = bucle.downgrade();
        move |_| {
            acabado.set(true);
            if let Some(bucle) = bucle.upgrade() {
                bucle.quit();
            }
        }
    });
    let _ = tope.update_timer(Some(ESPERA_DE_LA_LISTA), None);
    while !acabado.get() {
        bucle.run();
    }
    let visto = *visto.borrow();
    Some(visto)
}

/// Que se quiere grabar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fuentes {
    pub sistema: bool,
    pub microfono: bool,
}

/// Una pista que se va escribiendo: donde va, y cuanto lleva.
pub(crate) struct Pista {
    ruta: PathBuf,
    /// Se crea con el primer trozo: una pista de la que no llega nada no deja archivo.
    wav: Option<EscritorWav>,
    /// Marcos (una muestra por canal) escritos.
    escritos: u64,
}

impl Pista {
    pub(crate) fn nueva(ruta: PathBuf) -> Self {
        Self { ruta, wav: None, escritos: 0 }
    }

    fn marcos_en(ms: u64) -> u64 {
        ms * u64::from(FORMATO.sample_rate) / 1000
    }

    fn silencio(&mut self, marcos: u64) {
        let Some(wav) = self.wav.as_mut() else {
            return;
        };
        // A trozos de un segundo: un hueco largo no reserva de golpe lo que dure.
        let ceros = vec![0u8; FORMATO.sample_rate as usize * BYTES_POR_MARCO];
        let mut quedan = marcos as usize * BYTES_POR_MARCO;
        while quedan > 0 {
            let ahora = quedan.min(ceros.len());
            wav.escribir(&ceros[..ahora]);
            quedan -= ahora;
        }
        self.escritos += marcos;
    }

    /// Un trozo de sonido que acaba en `ms`, el instante del reloj de la grabacion.
    pub(crate) fn recibir(&mut self, pcm: &[u8], ms: u64) {
        let pcm = &pcm[..pcm.len() - pcm.len() % BYTES_POR_MARCO];
        if pcm.is_empty() {
            return;
        }
        let primero = self.wav.is_none();
        if primero {
            match EscritorWav::crear(&self.ruta, FORMATO) {
                Ok(wav) => self.wav = Some(wav),
                Err(error) => {
                    eprintln!("[grabar] no se puede escribir {}: {error}", self.ruta.display());
                    return;
                }
            }
        }
        let marcos = (pcm.len() / BYTES_POR_MARCO) as u64;
        let hueco = silencio_delante(self.escritos, marcos, ms, primero);
        if hueco > 0 {
            self.silencio(hueco);
        }
        if let Some(wav) = self.wav.as_mut() {
            wav.escribir(pcm);
            self.escritos += marcos;
        }
    }

    /// Rellena de silencio hasta `ms`, para que la pista mida lo que el video.
    pub(crate) fn hasta(&mut self, ms: u64) {
        let falta = Self::marcos_en(ms).saturating_sub(self.escritos);
        if falta > 0 {
            self.silencio(falta);
        }
    }

    #[cfg(test)]
    pub(crate) fn cerrar(mut self) -> u64 {
        self.wav.take().map(EscritorWav::cerrar).unwrap_or(0)
    }
}

/// Cuantos marcos de silencio van delante de un trozo de `marcos` que acaba en `ms`, en una
/// pista que lleva `escritos`. El primer trozo se coloca en su sitio exacto; despues solo se
/// rellena un hueco de verdad, no el vaiven de cada trozo.
fn silencio_delante(escritos: u64, marcos: u64, ms: u64, primero: bool) -> u64 {
    let empieza_en = Pista::marcos_en(ms).saturating_sub(marcos);
    let hueco = empieza_en.saturating_sub(escritos);
    if primero || hueco > Pista::marcos_en(HUECO_MS) {
        hueco
    } else {
        0
    }
}

/// Una pista de «Grabar siempre lo ultimo»: los ultimos segundos de un flujo, en memoria.
///
/// Se coloca con el reloj igual que la de archivo (`Pista`): `escritos` es por donde va la
/// pista contando desde el cero del reloj, con sus silencios, aunque de lo viejo ya no quede
/// nada guardado.
pub struct PistaEnAnillo {
    anillo: AnilloAudio,
    /// Los marcos que caben: la ventana y un poco mas.
    caben: u64,
    escritos: u64,
    /// Ha llegado sonido alguna vez. Una pista de la que no llega nada no existe.
    sonado: bool,
}

impl PistaEnAnillo {
    pub fn nueva(ventana_ms: u64) -> Self {
        Self {
            anillo: AnilloAudio::nuevo(FORMATO.bytes_por_ms(), FORMATO.channels, ventana_ms),
            // Lo mismo que guarda `AnilloAudio`: la ventana y dos segundos.
            caben: Pista::marcos_en(ventana_ms + 2_000),
            escritos: 0,
            sonado: false,
        }
    }

    fn silencio(&mut self, marcos: u64) {
        // Un hueco mas largo que el anillo lo vacia entero: no hace falta pasarlo todo por el.
        let ceros = vec![0u8; FORMATO.sample_rate as usize * BYTES_POR_MARCO];
        let mut quedan = marcos.min(self.caben) as usize * BYTES_POR_MARCO;
        while quedan > 0 {
            let ahora = quedan.min(ceros.len());
            self.anillo.empujar(&ceros[..ahora]);
            quedan -= ahora;
        }
        self.escritos += marcos;
    }

    /// Un trozo de sonido que acaba en `ms`, el instante del reloj del anillo.
    pub(crate) fn recibir(&mut self, pcm: &[u8], ms: u64) {
        let pcm = &pcm[..pcm.len() - pcm.len() % BYTES_POR_MARCO];
        if pcm.is_empty() {
            return;
        }
        let marcos = (pcm.len() / BYTES_POR_MARCO) as u64;
        let hueco = silencio_delante(self.escritos, marcos, ms, !self.sonado);
        if hueco > 0 {
            self.silencio(hueco);
        }
        self.sonado = true;
        self.anillo.empujar(pcm);
        self.escritos += marcos;
    }

    /// Los `cuantos_ms` de sonido que acaban en `hasta_ms`, con silencio donde no hay nada:
    /// por delante si no hay tanto guardado, y por detras si el flujo lleva un rato callado.
    /// No toca la pista (rescatar no puede descolocar lo que venga despues). `None` si de este
    /// flujo no ha llegado nunca nada.
    pub fn tramo(&self, hasta_ms: u64, cuantos_ms: u64) -> Option<Vec<u8>> {
        if !self.sonado {
            return None;
        }
        let hasta = Pista::marcos_en(hasta_ms);
        let cuantos = Pista::marcos_en(cuantos_ms).min(hasta);
        let desde = hasta - cuantos;
        // Lo guardado acaba en `escritos`: de ahi hacia atras hasta `desde`, sin lo que pase
        // de `hasta` (el vaiven de los trozos puede llevar la pista un pelo por delante).
        let del_final = self.escritos.saturating_sub(desde);
        let mut datos = self.anillo.ultimos(del_final * 1000 / u64::from(FORMATO.sample_rate) + 1);
        let quiero = del_final as usize * BYTES_POR_MARCO;
        if datos.len() > quiero {
            datos.drain(..datos.len() - quiero);
        }
        let de_mas = (self.escritos.saturating_sub(hasta) as usize * BYTES_POR_MARCO).min(datos.len());
        datos.truncate(datos.len() - de_mas);

        let total = cuantos as usize * BYTES_POR_MARCO;
        let detras = (hasta.saturating_sub(self.escritos) as usize * BYTES_POR_MARCO).min(total);
        let mut tramo = vec![0u8; total.saturating_sub(datos.len() + detras)];
        tramo.extend_from_slice(&datos);
        tramo.resize(total, 0);
        Some(tramo)
    }
}

/// El sonido de «Grabar siempre lo ultimo» en marcha: cada flujo en su anillo.
pub struct EnAnillo {
    sistema: Option<Arc<parking_lot::Mutex<PistaEnAnillo>>>,
    microfono: Option<Arc<parking_lot::Mutex<PistaEnAnillo>>>,
    hilo: JoinHandle<()>,
}

/// Lo que se saca de los anillos al rescatar: el tramo de cada pista que haya sonado.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TramoDeSonido {
    pub sistema: Option<Vec<u8>>,
    pub microfono: Option<Vec<u8>>,
}

/// Empieza a guardar en memoria los ultimos `ventana_ms` de cada fuente pedida. Para solo
/// cuando `Marcha::stop` se pone a cierto, y en pausa no guarda (ver `replay::apartar`).
pub fn empezar_en_anillo(fuentes: Fuentes, ventana_ms: u64, marcha: Marcha) -> Option<EnAnillo> {
    if !fuentes.sistema && !fuentes.microfono {
        return None;
    }
    let nueva = |quiere: bool| quiere.then(|| Arc::new(parking_lot::Mutex::new(PistaEnAnillo::nueva(ventana_ms))));
    let (sistema, microfono) = (nueva(fuentes.sistema), nueva(fuentes.microfono));
    let hilo = std::thread::Builder::new()
        .name("anillo-sonido".into())
        .spawn({
            let (sistema, microfono) = (sistema.clone(), microfono.clone());
            move || {
                let recibir: Recibir = Rc::new(move |cual, pcm, ms| {
                    let pista = match cual {
                        Cual::Sistema => sistema.as_ref(),
                        Cual::Microfono => microfono.as_ref(),
                    };
                    if let Some(pista) = pista {
                        pista.lock().recibir(pcm, ms);
                    }
                });
                if let Err(error) = correr(fuentes, &marcha, recibir) {
                    eprintln!("[replay] sin sonido: {error}");
                }
            }
        })
        .ok()?;
    Some(EnAnillo { sistema, microfono, hilo })
}

impl EnAnillo {
    /// Los `cuantos_ms` de cada pista que acaban en `hasta_ms`, sin parar de guardar.
    pub fn tramo(&self, hasta_ms: u64, cuantos_ms: u64) -> TramoDeSonido {
        let de = |pista: &Option<Arc<parking_lot::Mutex<PistaEnAnillo>>>| {
            pista.as_ref().and_then(|pista| pista.lock().tramo(hasta_ms, cuantos_ms))
        };
        TramoDeSonido { sistema: de(&self.sistema), microfono: de(&self.microfono) }
    }

    /// Espera a que el hilo acabe (ya se le ha dicho que pare).
    pub fn terminar(self) {
        let _ = self.hilo.join();
    }
}

/// La captura de sonido en marcha. Para sola cuando `Marcha::stop` se pone a cierto.
pub struct Captura {
    hilo: JoinHandle<Pistas>,
}

struct Pistas {
    sistema: Option<Pista>,
    microfono: Option<Pista>,
}

/// Empieza a grabar el sonido en la carpeta de la sesion. `fin_ms` es donde `stop` deja
/// apuntado cuanto se grabo: hasta ahi se rellenan las pistas.
pub fn empezar(fuentes: Fuentes, carpeta: &Path, marcha: Marcha, fin_ms: Arc<AtomicU64>) -> Option<Captura> {
    if !fuentes.sistema && !fuentes.microfono {
        return None;
    }
    let pistas = Pistas {
        sistema: fuentes.sistema.then(|| Pista::nueva(carpeta.join(ARCHIVO_PC))),
        microfono: fuentes.microfono.then(|| Pista::nueva(carpeta.join(ARCHIVO_MICROFONO))),
    };
    let hilo = std::thread::Builder::new()
        .name("grabar-sonido".into())
        .spawn(move || {
            let pistas = Rc::new(RefCell::new(pistas));
            let recibir: Recibir = {
                let pistas = pistas.clone();
                Rc::new(move |cual, pcm, ms| {
                    let mut pistas = pistas.borrow_mut();
                    let pista = match cual {
                        Cual::Sistema => pistas.sistema.as_mut(),
                        Cual::Microfono => pistas.microfono.as_mut(),
                    };
                    if let Some(pista) = pista {
                        pista.recibir(pcm, ms);
                    }
                })
            };
            if let Err(error) = correr(fuentes, &marcha, recibir) {
                eprintln!("[grabar] sin sonido: {error}");
            }
            let mut pistas = Rc::try_unwrap(pistas)
                .map(RefCell::into_inner)
                .unwrap_or_else(|_| Pistas { sistema: None, microfono: None });
            let fin = match fin_ms.load(Ordering::Relaxed) {
                0 => marcha.ms(),
                apuntado => apuntado,
            };
            for pista in [pistas.sistema.as_mut(), pistas.microfono.as_mut()].into_iter().flatten() {
                pista.hasta(fin);
            }
            pistas
        })
        .ok()?;
    Some(Captura { hilo })
}

impl Captura {
    /// Espera a que el hilo acabe (ya se le ha dicho que pare), cierra los archivos y dice
    /// que sonido hay. Decir que lo hay sin un byte dejaria al exportador buscando un
    /// archivo vacio.
    pub fn cerrar(self) -> (Option<AudioInfo>, Option<PistasGrabadas>) {
        let Ok(pistas) = self.hilo.join() else {
            return (None, None);
        };
        cerrar_pistas(pistas.sistema.and_then(|p| p.wav), pistas.microfono.and_then(|p| p.wav))
    }
}

/// Cierra los dos archivos (los que haya) y dice que sonido queda en la sesion. Es el final de
/// una grabacion y tambien el de un rescate de «Grabar siempre lo ultimo», que escribe los
/// suyos de una vez.
pub(crate) fn cerrar_pistas(pc: Option<EscritorWav>, voz: Option<EscritorWav>) -> (Option<AudioInfo>, Option<PistasGrabadas>) {
    // Como en Windows: el del PC se sube si llego casi mudo, y menos si hay una voz que
    // se oye; la voz no se toca.
    let con_voz = voz.as_ref().is_some_and(|voz| voz.nivel() >= PICO_CASI_MUDO);
    let mut grabadas = PistasGrabadas::default();
    if let Some(pc) = pc {
        let (escritos, pico, subida) = pc.cerrar_y_nivelar(con_voz);
        if escritos > 0 {
            grabadas.sistema = Some(ARCHIVO_PC.to_string());
            grabadas.pico_sistema = Some(pico);
            grabadas.subida_sistema = subida;
        }
    }
    if let Some(voz) = voz {
        let pico = voz.pico();
        if voz.cerrar() > 0 {
            grabadas.microfono = Some(ARCHIVO_MICROFONO.to_string());
            grabadas.pico_microfono = Some(pico);
        }
    }
    if grabadas.sistema.is_none() && grabadas.microfono.is_none() {
        return (None, None);
    }
    (Some(FORMATO), Some(grabadas))
}

/// Lo que se le pide a PipeWire: PCM de 16 bits, 48 kHz y dos canales.
fn oferta() -> std::result::Result<Vec<u8>, String> {
    let mut formato = AudioInfoRaw::new();
    formato.set_format(AudioFormat::S16LE);
    formato.set_rate(FORMATO.sample_rate);
    formato.set_channels(u32::from(FORMATO.channels));
    let mut sitios = [0u32; spa::sys::SPA_AUDIO_MAX_CHANNELS as usize];
    sitios[0] = spa::sys::SPA_AUDIO_CHANNEL_FL;
    sitios[1] = spa::sys::SPA_AUDIO_CHANNEL_FR;
    formato.set_position(sitios);
    let objeto = spa::pod::Object {
        type_: SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: formato.into(),
    };
    PodSerializer::serialize(Cursor::new(Vec::new()), &spa::pod::Value::Object(objeto))
        .map(|(escrito, _)| escrito.into_inner())
        .map_err(|error| format!("{error:?}"))
}

/// Cual de las dos pistas alimenta un flujo.
#[derive(Clone, Copy)]
pub(super) enum Cual {
    Sistema,
    Microfono,
}

/// A donde va cada trozo que llega: de que flujo es, el sonido y el instante del reloj en el
/// que acaba. Se llama en el hilo del sonido (el de PipeWire, o el que vacia lo de PulseAudio).
pub(super) type Recibir = Rc<dyn Fn(Cual, &[u8], u64)>;

/// Graba lo pedido hasta que `Marcha::stop` se pone a cierto, por quien lleve el sonido aqui.
fn correr(fuentes: Fuentes, marcha: &Marcha, recibir: Recibir) -> std::result::Result<(), String> {
    let antes = std::time::Instant::now();
    let (motor, _) = mirar();
    eprintln!("[grabar] el sonido lo lleva {motor:?} (mirado en {} ms)", antes.elapsed().as_millis());
    match motor {
        Motor::PipeWire => correr_en_pipewire(fuentes, marcha, recibir),
        Motor::PulseAudio => super::sonido_pulse::correr(fuentes, marcha, recibir),
    }
}

fn correr_en_pipewire(fuentes: Fuentes, marcha: &Marcha, recibir: Recibir) -> std::result::Result<(), String> {
    let mal = |error: pw::Error| error.to_string();
    pw::init();
    let bucle = pw::main_loop::MainLoopRc::new(None).map_err(mal)?;
    let contexto = pw::context::ContextRc::new(&bucle, None).map_err(mal)?;
    let nucleo = contexto.connect_rc(None).map_err(mal)?;
    let oferta = oferta()?;

    let abrir = |cual: Cual| -> std::result::Result<_, String> {
        let propiedades = match cual {
            // El «monitor» de la salida: lo que esta sonando por el altavoz.
            Cual::Sistema => pw::properties::properties! {
                *pw::keys::MEDIA_TYPE => "Audio",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Music",
                "stream.capture.sink" => "true",
                "node.description" => "winshotx",
            },
            Cual::Microfono => pw::properties::properties! {
                *pw::keys::MEDIA_TYPE => "Audio",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Communication",
                "node.description" => "winshotx",
            },
        };
        let nombre = match cual {
            Cual::Sistema => "winshotx-altavoz",
            Cual::Microfono => "winshotx-microfono",
        };
        let flujo = pw::stream::StreamRc::new(nucleo.clone(), nombre, propiedades).map_err(mal)?;
        let escucha = flujo
            .add_local_listener_with_user_data((recibir.clone(), marcha.clone()))
            .process(move |flujo, (recibir, marcha)| {
                let Some(mut bufer) = flujo.dequeue_buffer() else {
                    return;
                };
                if marcha.en_pausa() || marcha.parada() {
                    return;
                }
                let Some(datos) = bufer.datas_mut().first_mut() else {
                    return;
                };
                let (desde, cuanto) = (datos.chunk().offset() as usize, datos.chunk().size() as usize);
                let Some(bytes) = datos.data() else {
                    return;
                };
                let Some(pcm) = bytes.get(desde..desde + cuanto) else {
                    return;
                };
                recibir(cual, pcm, marcha.ms());
            })
            .register()
            .map_err(mal)?;
        let mut parametros = [Pod::from_bytes(&oferta).ok_or("no se ha podido escribir el formato del sonido")?];
        flujo
            .connect(
                Direction::Input,
                None,
                pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
                &mut parametros,
            )
            .map_err(mal)?;
        Ok((flujo, escucha))
    };

    let (con_sistema, con_microfono) = (fuentes.sistema, fuentes.microfono);
    // Si uno de los dos no se puede abrir, el otro se graba igual.
    let _altavoz = con_sistema.then(|| abrir(Cual::Sistema)).and_then(|r| r.map_err(|e| eprintln!("[grabar] sin el sonido del sistema: {e}")).ok());
    let _microfono = con_microfono.then(|| abrir(Cual::Microfono)).and_then(|r| r.map_err(|e| eprintln!("[grabar] sin el micrófono: {e}")).ok());

    let latido = bucle.loop_().add_timer({
        let marcha = marcha.clone();
        let bucle = bucle.downgrade();
        move |_| {
            if marcha.parada() {
                if let Some(bucle) = bucle.upgrade() {
                    bucle.quit();
                }
            }
        }
    });
    let _ = latido.update_timer(Some(LATIDO), Some(LATIDO));
    bucle.run();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let carpeta = std::env::temp_dir().join(format!("winshotx-sonido-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&carpeta);
        std::fs::create_dir_all(&carpeta).expect("carpeta de prueba");
        carpeta
    }

    /// Medio segundo de un tono cualquiera, en PCM de 16 bits y dos canales.
    fn trozo(ms: u64) -> Vec<u8> {
        let marcos = Pista::marcos_en(ms) as usize;
        (0..marcos * 2).flat_map(|i| (((i % 200) as i16 - 100) * 100).to_le_bytes()).collect()
    }

    fn ms_de(bytes: u64) -> u64 {
        bytes / FORMATO.bytes_por_ms()
    }

    #[test]
    fn una_pista_de_la_que_no_llega_nada_no_deja_archivo() {
        let carpeta = carpeta("nada");
        let mut pista = Pista::nueva(carpeta.join("a.wav"));
        pista.hasta(5000);
        assert_eq!(pista.cerrar(), 0);
        assert!(!carpeta.join("a.wav").exists());
        let _ = std::fs::remove_dir_all(carpeta);
    }

    #[test]
    fn el_primer_trozo_se_coloca_donde_sono() {
        // Un flujo que empieza a mandar al segundo y medio: delante va un segundo de silencio.
        let carpeta = carpeta("tarde");
        let mut pista = Pista::nueva(carpeta.join("a.wav"));
        pista.recibir(&trozo(500), 1500);
        assert_eq!(ms_de(pista.cerrar()), 1500);
        let _ = std::fs::remove_dir_all(carpeta);
    }

    #[test]
    fn el_vaiven_de_los_trozos_no_mete_silencio_y_un_hueco_de_verdad_si() {
        let carpeta = carpeta("hueco");
        let mut pista = Pista::nueva(carpeta.join("a.wav"));
        pista.recibir(&trozo(20), 20);
        // Llega treinta milisegundos tarde: es el vaiven de siempre, va pegado al anterior.
        pista.recibir(&trozo(20), 70);
        assert_eq!(pista.escritos, Pista::marcos_en(40));
        // Dos segundos sin nada (la salida se durmio): el siguiente va en su sitio.
        pista.recibir(&trozo(20), 2070);
        assert_eq!(pista.escritos, Pista::marcos_en(2070));
        let _ = std::fs::remove_dir_all(carpeta);
    }

    #[test]
    fn al_parar_la_pista_mide_lo_que_el_video() {
        let carpeta = carpeta("final");
        let mut pista = Pista::nueva(carpeta.join("a.wav"));
        pista.recibir(&trozo(100), 100);
        pista.hasta(3000);
        assert_eq!(ms_de(pista.cerrar()), 3000);
        let _ = std::fs::remove_dir_all(carpeta);
    }

    /// Un trozo en el que cada marco lleva su numero (desde `primero`), para saber cual es cual.
    fn numerado(primero: u64, marcos: u64) -> Vec<u8> {
        (primero..primero + marcos).flat_map(|n| [(n % 30_000) as i16; 2]).flat_map(i16::to_le_bytes).collect()
    }

    /// El numero de cada marco de un tramo (0 es silencio: la cuenta empieza en 1).
    fn numeros(tramo: &[u8]) -> Vec<i16> {
        tramo.chunks_exact(BYTES_POR_MARCO).map(|m| i16::from_le_bytes([m[0], m[1]])).collect()
    }

    #[test]
    fn una_pista_en_anillo_de_la_que_no_llega_nada_no_existe() {
        let pista = PistaEnAnillo::nueva(10_000);
        assert_eq!(pista.tramo(5_000, 3_000), None);
    }

    /// Lo que se rescata acaba en el instante que se pide y mide lo que se pide, con silencio
    /// delante si el flujo empezo tarde y detras si lleva un rato callado.
    #[test]
    fn el_tramo_del_anillo_acaba_donde_se_pide_y_mide_lo_que_se_pide() {
        let mut pista = PistaEnAnillo::nueva(10_000);
        // El flujo empieza a mandar en el segundo 2 y suena un segundo, de 20 en 20 ms.
        let por_trozo = Pista::marcos_en(20);
        for n in 0..50u64 {
            pista.recibir(&numerado(1 + n * por_trozo, por_trozo), 2_020 + n * 20);
        }
        // Rescate en el segundo 4, pidiendo los cuatro: 2 s de silencio, 1 s de sonido y 1 s
        // de silencio (el flujo callo en el 3).
        let tramo = pista.tramo(4_000, 4_000).expect("ha sonado");
        assert_eq!(tramo.len() as u64, 4_000 * FORMATO.bytes_por_ms());
        let n = numeros(&tramo);
        let seg = Pista::marcos_en(1_000) as usize;
        assert!(n[..2 * seg].iter().all(|v| *v == 0), "delante va silencio");
        assert_eq!(n[2 * seg], 1, "el sonido empieza justo en el segundo 2");
        assert_eq!(n[3 * seg - 1], (seg % 30_000) as i16, "y acaba justo en el 3");
        assert!(n[3 * seg..].iter().all(|v| *v == 0), "detras va silencio");

        // Pedir no descoloca nada: lo que llega despues va en su sitio.
        pista.recibir(&numerado(7, por_trozo), 5_000);
        let n = numeros(&pista.tramo(5_000, 20).expect("ha sonado"));
        assert_eq!((n[0], n.len() as u64), (7, por_trozo));

        // Y un tramo que acaba antes de lo ultimo guardado no trae lo de despues.
        let n = numeros(&pista.tramo(2_500, 1_000).expect("ha sonado"));
        assert!(n[..seg / 2].iter().all(|v| *v == 0));
        assert_eq!((n[seg / 2], n[seg - 1]), (1, (seg / 2) as i16));
    }

    /// El anillo tira lo viejo: horas despues sigue ocupando lo que la ventana, y un hueco
    /// de una hora no pasa una hora de ceros por la memoria.
    #[test]
    fn el_anillo_de_sonido_solo_guarda_la_ventana() {
        let mut pista = PistaEnAnillo::nueva(3_000);
        let por_trozo = Pista::marcos_en(20);
        for n in 0..1_000u64 {
            pista.recibir(&numerado(1 + n * por_trozo, por_trozo), 20 + n * 20);
        }
        // Veinte segundos sonando: quedan los ultimos cinco (la ventana y dos mas).
        assert_eq!(pista.anillo.ultimos(60_000).len() as u64, 5_000 * FORMATO.bytes_por_ms());
        // Pidiendo ocho, los tres de delante ya no estan: silencio.
        let n = numeros(&pista.tramo(20_000, 8_000).expect("ha sonado"));
        let seg = Pista::marcos_en(1_000) as usize;
        assert!(n[..3 * seg].iter().all(|v| *v == 0));
        assert_eq!(n[3 * seg], ((15 * seg + 1) % 30_000) as i16);

        // Una hora callado y vuelve a sonar.
        let antes = std::time::Instant::now();
        pista.recibir(&numerado(1, por_trozo), 3_620_000);
        assert!(antes.elapsed() < Duration::from_millis(500), "el hueco ha pasado entero por la memoria");
        assert_eq!(pista.escritos, Pista::marcos_en(3_620_000));
        let n = numeros(&pista.tramo(3_620_000, 1_000).expect("ha sonado"));
        assert!(n[..seg - por_trozo as usize].iter().all(|v| *v == 0));
        assert_eq!(n[seg - por_trozo as usize], 1);
    }

    #[test]
    fn solo_cuentan_las_salidas_y_las_entradas_de_sonido() {
        let nada = Disponible::default();
        assert_eq!(clase_de_sonido("Audio/Sink"), Disponible { altavoz: true, ..nada });
        assert_eq!(clase_de_sonido("Audio/Source"), Disponible { microfono: true, ..nada });
        assert_eq!(clase_de_sonido("Audio/Duplex"), Disponible { altavoz: true, microfono: true, ..nada });
        // Lo que hay en un PipeWire que solo lleva video (Ubuntu 22.04): nada de esto cuenta.
        assert_eq!(clase_de_sonido("Video/Source"), Disponible::default());
        assert_eq!(clase_de_sonido("Stream/Output/Audio"), Disponible::default());
        assert_eq!(clase_de_sonido("Midi/Bridge"), Disponible::default());
    }

    /// Donde PipeWire lleva el sonido (Fedora, Ubuntu desde la 22.10, Debian 12 con GNOME)
    /// nada cambia: es suyo, y a PulseAudio ni se le pregunta. Alli quien contestaria por
    /// PulseAudio es el propio PipeWire, y se grabaria lo mismo por un camino mas largo.
    #[test]
    fn donde_pipewire_lleva_el_sonido_no_se_pregunta_a_pulseaudio() {
        let nada = Disponible::default();
        let no_se_pregunta = || -> Disponible { panic!("se ha preguntado a PulseAudio") };
        for en_pipewire in [
            Disponible { altavoz: true, microfono: true, ..nada },
            Disponible { altavoz: true, ..nada },
            Disponible { microfono: true, ..nada },
        ] {
            assert_eq!(quien_lo_lleva(en_pipewire, no_se_pregunta), (Motor::PipeWire, en_pipewire));
        }
    }

    /// Ubuntu 22.04 y Linux Mint 21: PipeWire esta, pero sin un solo aparato de sonido. Se
    /// graba por PulseAudio, con lo que PulseAudio tenga.
    #[test]
    fn si_pipewire_no_lleva_sonido_se_graba_por_pulseaudio() {
        let nada = Disponible::default();
        let con_todo = Disponible { altavoz: true, microfono: true, ..nada };
        assert_eq!(quien_lo_lleva(nada, || con_todo), (Motor::PulseAudio, con_todo));
        // Un PC sin microfono: el altavoz se graba igual.
        let sin_micro = Disponible { altavoz: true, ..nada };
        assert_eq!(quien_lo_lleva(nada, || sin_micro), (Motor::PulseAudio, sin_micro));
    }

    /// Sin sonido en ninguno de los dos se graba sin sonido, y si lo que pasa es que falta el
    /// plugin de PulseAudio, eso llega a los ajustes.
    #[test]
    fn sin_sonido_en_ninguno_de_los_dos_se_dice_por_que() {
        let nada = Disponible::default();
        assert_eq!(quien_lo_lleva(nada, || nada), (Motor::PipeWire, nada));
        let falta = Disponible { falta_el_de_pulse: true, ..nada };
        assert_eq!(quien_lo_lleva(nada, || falta), (Motor::PipeWire, falta));
    }
}
