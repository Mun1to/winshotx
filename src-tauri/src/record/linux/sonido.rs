//! El sonido en Linux: lo que suena por el altavoz y el microfono, por PipeWire.
//!
//! PipeWire es quien lleva el sonido en casi todos los escritorios de hoy (Fedora, Ubuntu
//! desde la 22.10, Debian 12, Arch), y la libreria ya esta en winshotx porque por ella
//! llegan tambien los fotogramas de Wayland. Se le piden dos flujos: uno de lo que sale por
//! el altavoz (el «monitor» de la salida, con `stream.capture.sink`) y otro del microfono.
//! Cada uno va a su WAV, los mismos archivos que en Windows (`record::pistas`), y de ahi en
//! adelante el editor y el exportador no notan la diferencia.
//!
//! **Donde PipeWire no lleva el sonido, se graba sin sonido.** En Ubuntu 22.04 y Linux Mint
//! 21 PipeWire esta, pero solo para compartir pantalla: el sonido lo lleva PulseAudio. Alli
//! los flujos no encuentran a que engancharse, no llega nada y la grabacion sale sin pistas,
//! sin colgarse y sin error. `disponible` lo dice antes, para que la interfaz apague los
//! interruptores y explique por que.
//!
//! **Cada pista mide lo mismo que el video.** Lo que llega se coloca segun el reloj de la
//! grabacion: si un flujo empieza tarde o se queda un rato sin mandar nada, el hueco se
//! rellena de silencio, y al parar se rellena hasta el final. En pausa no se escribe.

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
use crate::record::pistas::{EscritorWav, ARCHIVO_MICROFONO, ARCHIVO_PC, PICO_CASI_MUDO};
use crate::record::AudioInfo;

/// En que formato se pide el sonido: el de un WAV de toda la vida. PipeWire convierte lo que
/// tenga cada aparato a esto.
const FORMATO: AudioInfo = AudioInfo { channels: 2, sample_rate: 48_000 };
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
}

/// Mira que aparatos de sonido lleva PipeWire. Sin PipeWire, o con uno que solo lleva video
/// (Ubuntu 22.04, Linux Mint 21), no hay ninguno.
pub fn disponible() -> Disponible {
    let (listo, esperando) = std::sync::mpsc::channel();
    // En su hilo y con tope: si PipeWire no contesta, no se queda nadie esperando.
    std::thread::spawn(move || {
        let _ = listo.send(listar().unwrap_or_default());
    });
    esperando.recv_timeout(ESPERA_DE_LA_LISTA).unwrap_or_default()
}

/// Que dice cada aparato de la lista de PipeWire: una salida, una entrada o ninguna de las dos.
pub(crate) fn clase_de_sonido(clase: &str) -> Disponible {
    Disponible {
        // «Audio/Duplex» es un aparato que hace las dos cosas (unos cascos con microfono).
        altavoz: clase == "Audio/Sink" || clase == "Audio/Duplex",
        microfono: clase == "Audio/Source" || clase == "Audio/Duplex" || clase == "Audio/Source/Virtual",
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
        let empieza_en = Self::marcos_en(ms).saturating_sub(marcos);
        // El primer trozo se coloca en su sitio exacto; despues solo se rellena un hueco de
        // verdad, no el vaiven de cada trozo.
        let hueco = empieza_en.saturating_sub(self.escritos);
        if hueco > 0 && (primero || hueco > Self::marcos_en(HUECO_MS)) {
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
            if let Err(error) = correr(&pistas, &marcha) {
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
        let voz = pistas.microfono.and_then(|p| p.wav);
        let pc = pistas.sistema.and_then(|p| p.wav);
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
enum Cual {
    Sistema,
    Microfono,
}

fn correr(pistas: &Rc<RefCell<Pistas>>, marcha: &Marcha) -> std::result::Result<(), String> {
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
            .add_local_listener_with_user_data((pistas.clone(), marcha.clone()))
            .process(move |flujo, (pistas, marcha)| {
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
                let mut pistas = pistas.borrow_mut();
                let pista = match cual {
                    Cual::Sistema => pistas.sistema.as_mut(),
                    Cual::Microfono => pistas.microfono.as_mut(),
                };
                if let Some(pista) = pista {
                    pista.recibir(pcm, marcha.ms());
                }
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

    let (con_sistema, con_microfono) = {
        let pistas = pistas.borrow();
        (pistas.sistema.is_some(), pistas.microfono.is_some())
    };
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

    #[test]
    fn solo_cuentan_las_salidas_y_las_entradas_de_sonido() {
        assert_eq!(clase_de_sonido("Audio/Sink"), Disponible { altavoz: true, microfono: false });
        assert_eq!(clase_de_sonido("Audio/Source"), Disponible { altavoz: false, microfono: true });
        assert_eq!(clase_de_sonido("Audio/Duplex"), Disponible { altavoz: true, microfono: true });
        // Lo que hay en un PipeWire que solo lleva video (Ubuntu 22.04): nada de esto cuenta.
        assert_eq!(clase_de_sonido("Video/Source"), Disponible::default());
        assert_eq!(clase_de_sonido("Stream/Output/Audio"), Disponible::default());
        assert_eq!(clase_de_sonido("Midi/Bridge"), Disponible::default());
    }
}
