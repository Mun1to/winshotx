//! El GStreamer del sistema: escribir un MP4 y leer video y sonido en Linux.
//!
//! En Windows esto lo hace Media Foundation. En Linux winshotx no reparte ningun
//! codificador: H.264 tiene patentes y x264 es GPL. Se usa el GStreamer que ya hay en el
//! equipo (WebKitGTK depende de el) con `openh264enc`, el codec de Cisco que cada
//! distribucion reparte por su cuenta, y el codificador de AAC que haya. Si falta alguno el
//! MP4 no sale y se dice que instalar (`mp4::que_falta`); el GIF no pasa por aqui.
//!
//! **Cada fotograma va con su tiempo.** Una pantalla quieta es UN fotograma que dura lo que
//! dure, igual que en la cache: no se repite treinta veces por segundo para rellenar.
//!
//! **Nada se queda esperando para siempre.** Mandar un fotograma espera a que el
//! codificador tenga sitio mirando si ha fallado, y cerrar tiene un plazo: un codificador
//! roto devuelve un error, no cuelga a quien exporta.

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use image::RgbaImage;

use super::fondo_video::{Cuadro, Lector, FPS};
use crate::error::{AppError, Result};

/// Los codificadores de H.264 que se usan. Solo el de Cisco: `x264enc` es GPL y los de la
/// tarjeta grafica (VA-API) no se han podido probar sin una de verdad.
const DE_VIDEO: &[&str] = &["openh264enc"];
/// Los de AAC, por orden: el de Fedora, el de Debian y Ubuntu, y el de libav.
const DE_SONIDO: &[&str] = &["fdkaacenc", "voaacenc", "avenc_aac"];
/// Los fotogramas que pueden estar esperando al codificador. Con mas, quien los manda espera.
const EN_COLA: u64 = 4;
/// Lo que se espera a que el codificador haga sitio, o a que cierre el archivo.
const PACIENCIA: Duration = Duration::from_secs(60);
/// Lo que se espera al siguiente fotograma de un video que se lee.
const PACIENCIA_AL_LEER: Duration = Duration::from_secs(30);

pub(crate) fn encender() -> Result<()> {
    static HECHO: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    HECHO
        .get_or_init(|| gst::init().map_err(|e| format!("GStreamer no arranca: {e}")))
        .clone()
        .map_err(AppError::Msg)
}

pub(crate) fn mal(texto: impl Into<String>) -> AppError {
    AppError::Msg(texto.into())
}

/// El error que haya dejado la tuberia, si ha dejado alguno.
pub(crate) fn error_de(bus: &gst::Bus) -> Option<String> {
    let mensaje = bus.pop_filtered(&[gst::MessageType::Error])?;
    match mensaje.view() {
        gst::MessageView::Error(e) => Some(e.error().to_string()),
        _ => None,
    }
}

fn monta(texto: &str) -> Result<gst::Pipeline> {
    encender()?;
    gst::parse_launch(texto)
        .map_err(|e| mal(format!("GStreamer no puede montar esto: {e}")))?
        .downcast::<gst::Pipeline>()
        .map_err(|_| mal("GStreamer no ha devuelto una tuberia"))
}

pub(crate) fn pieza<T: IsA<gst::Element>>(tuberia: &gst::Pipeline, nombre: &str) -> Result<T> {
    tuberia
        .by_name(nombre)
        .and_then(|e| e.downcast::<T>().ok())
        .ok_or_else(|| mal(format!("a la tuberia le falta «{nombre}»")))
}

// --- Que codificadores hay ---

/// Los codificadores que funcionan en este equipo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Codificadores {
    pub video: Option<&'static str>,
    pub sonido: Option<&'static str>,
}

/// Mira que codificadores hay, escribiendo con cada uno un MP4 diminuto de verdad.
///
/// Que el plugin exista no basta: Fedora instala por defecto un `openh264` de mentira (el
/// paquete `noopenh264`) con el que el elemento esta y no codifica nada.
///
/// Lo que se encuentra se recuerda; lo que falta se vuelve a mirar cada vez, para que
/// instalar el paquete con winshotx abierto baste y no haga falta volver a arrancarlo.
pub fn codificadores() -> Codificadores {
    static HAY: parking_lot::Mutex<Codificadores> =
        parking_lot::Mutex::new(Codificadores { video: None, sonido: None });
    let mut hay = HAY.lock();
    if hay.video.is_some() && hay.sonido.is_some() {
        return *hay;
    }
    if encender().is_err() {
        return *hay;
    }
    // Por si se ha instalado algo desde la ultima vez que se miro.
    let _ = gst::update_registry();
    if hay.video.is_none() {
        hay.video = DE_VIDEO.iter().copied().find(|v| sonda(v, None));
    }
    if hay.sonido.is_none() {
        if let Some(video) = hay.video {
            hay.sonido = DE_SONIDO.iter().copied().find(|s| sonda(video, Some(s)));
        }
    }
    // Una vez, y otra cada vez que cambie: mientras falte algo esto se mira a menudo (cada vez
    // que el editor coge el foco), y repetir la misma linea solo llenaria el registro.
    static DICHO: parking_lot::Mutex<Option<Codificadores>> = parking_lot::Mutex::new(None);
    let mut dicho = DICHO.lock();
    if *dicho != Some(*hay) {
        *dicho = Some(*hay);
        eprintln!(
            "[mp4] codificadores del sistema: video={} sonido={}{}",
            hay.video.unwrap_or("ninguno"),
            hay.sonido.unwrap_or("ninguno"),
            if appimage_a_ciegas() { " (AppImage: su GStreamer no carga los plugins del sistema)" } else { "" }
        );
    }
    *hay
}

/// Si esto es un AppImage cuyo GStreamer no carga los plugins del sistema.
///
/// El AppImage lleva su propio `libgstreamer`, el de la distribucion donde se empaqueto. Un
/// GStreamer no carga plugins hechos para uno mas nuevo, ni los busca en la carpeta de otra
/// distribucion: en casi todos los sistemas no ve ninguno, y entonces instalar el
/// codificador no arregla nada. Se nota en que tampoco esta `videoconvert`, que es de los
/// plugins base y lo tiene cualquier sistema con GStreamer.
pub fn appimage_a_ciegas() -> bool {
    std::env::var_os("APPIMAGE").is_some()
        && (encender().is_err() || gst::ElementFactory::find("videoconvert").is_none())
}

/// Escribe tres fotogramas (y un instante de silencio) con esos codificadores.
fn sonda(video: &str, sonido: Option<&str>) -> bool {
    static CUAL: AtomicU32 = AtomicU32::new(0);
    if gst::ElementFactory::find(video).is_none() || sonido.is_some_and(|s| gst::ElementFactory::find(s).is_none()) {
        return false;
    }
    let ruta = std::env::temp_dir().join(format!(
        "winshotx-sonda-{}-{}.mp4",
        std::process::id(),
        CUAL.fetch_add(1, Ordering::Relaxed)
    ));
    let medidas = Medidas { ancho: 320, alto: 240, fps: 30, bitrate: 400_000 };
    let escrito = (|| -> Result<()> {
        let mut escritor = Escritor::abrir_con(&ruta, &medidas, video, sonido.map(|s| (s, 2, 48_000)))?;
        for cuadro in 0..3u64 {
            escritor.imagen(vec![128; 320 * 240 * 4], cuadro * 33, 33)?;
            escritor.sonido(&[0; 1600 * 4])?;
        }
        escritor.cerrar()
    })();
    let pesa = std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0);
    let _ = std::fs::remove_file(&ruta);
    if let Err(error) = &escrito {
        eprintln!("[mp4] {video}{} no sirve aqui: {error}", sonido.map(|s| format!(" con {s}")).unwrap_or_default());
    }
    escrito.is_ok() && pesa > 0
}

// --- Escribir ---

/// Como tiene que salir el video.
pub struct Medidas {
    pub ancho: u32,
    pub alto: u32,
    pub fps: u32,
    /// En bits por segundo.
    pub bitrate: u32,
}

struct Sonido {
    fuente: gst_app::AppSrc,
    /// Bytes de una muestra con todos sus canales.
    bloque: usize,
    frecuencia: u64,
    /// Las muestras mandadas: el tiempo del sonido se cuenta con ellas.
    mandadas: u64,
    acabado: bool,
}

/// Un MP4 que se va escribiendo: H.264 y, si se pide, AAC.
pub struct Escritor {
    tuberia: gst::Pipeline,
    video: gst_app::AppSrc,
    sonido: Option<Sonido>,
    bus: gst::Bus,
    /// Los bytes de fotogramas que pueden estar esperando al codificador.
    tope: u64,
    /// El primer error de la tuberia: desde que lo hay, todo lo demas lo devuelve.
    roto: Option<String>,
}

impl Escritor {
    /// Abre el archivo con los codificadores del sistema. `sonido` son los canales y la
    /// frecuencia del PCM de 16 bits que se va a mandar. Sin codificador, el error dice que
    /// falta (`mp4::que_falta`).
    pub fn abrir(ruta: &Path, medidas: &Medidas, sonido: Option<(u16, u32)>) -> Result<Self> {
        let hay = codificadores();
        let falta = |que| mal(super::mp4::que_falta(que));
        let video = hay.video.ok_or_else(|| falta(super::mp4::Falta::Video))?;
        let sonido = match sonido {
            Some((canales, frecuencia)) => {
                Some((hay.sonido.ok_or_else(|| falta(super::mp4::Falta::Sonido))?, canales, frecuencia))
            }
            None => None,
        };
        Self::abrir_con(ruta, medidas, video, sonido)
    }

    fn abrir_con(ruta: &Path, medidas: &Medidas, video: &str, sonido: Option<(&str, u16, u32)>) -> Result<Self> {
        let fps = medidas.fps.max(1);
        // Un fotograma clave cada dos segundos: es lo mas lejos que tiene que retroceder el
        // editor para ensennar un instante cualquiera.
        let mut texto = format!(
            "appsrc name=v format=time ! videoconvert ! {video} bitrate={} gop-size={} ! h264parse ! queue ! mp4mux name=m ! filesink name=f",
            medidas.bitrate.max(64_000),
            fps * 2,
        );
        if let Some((codificador, _, _)) = sonido {
            texto.push_str(&format!(
                " appsrc name=a format=time ! audioconvert ! audioresample ! {codificador} bitrate=192000 ! queue ! m."
            ));
        }
        let tuberia = monta(&texto)?;
        let fuente: gst_app::AppSrc = pieza(&tuberia, "v")?;
        fuente.set_caps(Some(
            &gst::Caps::builder("video/x-raw")
                .field("format", "RGBA")
                .field("width", medidas.ancho as i32)
                .field("height", medidas.alto as i32)
                .field("framerate", gst::Fraction::new(fps as i32, 1))
                .build(),
        ));
        let sonido = match sonido {
            Some((_, canales, frecuencia)) => {
                let fuente: gst_app::AppSrc = pieza(&tuberia, "a")?;
                fuente.set_caps(Some(
                    &gst::Caps::builder("audio/x-raw")
                        .field("format", "S16LE")
                        .field("layout", "interleaved")
                        .field("rate", frecuencia as i32)
                        .field("channels", i32::from(canales))
                        .build(),
                ));
                Some(Sonido {
                    fuente,
                    bloque: usize::from(canales.max(1)) * 2,
                    frecuencia: u64::from(frecuencia.max(1)),
                    mandadas: 0,
                    acabado: false,
                })
            }
            None => None,
        };
        pieza::<gst::Element>(&tuberia, "f")?.set_property("location", ruta.to_string_lossy().as_ref());
        let bus = tuberia.bus().ok_or_else(|| mal("la tuberia no tiene por donde avisar"))?;
        let este = Self {
            tuberia,
            video: fuente,
            sonido,
            bus,
            tope: u64::from(medidas.ancho) * u64::from(medidas.alto) * 4 * EN_COLA,
            roto: None,
        };
        if este.tuberia.set_state(gst::State::Playing).is_err() {
            let porque = error_de(&este.bus).unwrap_or_else(|| "GStreamer no ha querido empezar".into());
            return Err(mal(format!("el MP4 no se puede escribir: {porque}")));
        }
        Ok(este)
    }

    /// Lo que ha fallado, si algo ha fallado: se mira en cada paso.
    fn comprobar(&mut self) -> Result<()> {
        if self.roto.is_none() {
            self.roto = error_de(&self.bus);
        }
        match &self.roto {
            Some(porque) => Err(mal(format!("el MP4 no se ha podido escribir: {porque}"))),
            None => Ok(()),
        }
    }

    /// Si al codificador le cabe otro fotograma ahora mismo. Quien graba en vivo lo mira
    /// para saltarse uno en vez de esperar.
    pub fn cabe(&self) -> bool {
        self.video.current_level_bytes() < self.tope
    }

    /// Un fotograma RGBA de arriba abajo, con el instante en que empieza y lo que dura.
    /// Si el codificador va por detras, espera a que haga sitio.
    pub fn imagen(&mut self, rgba: Vec<u8>, desde_ms: u64, dura_ms: u64) -> Result<()> {
        let empieza = Instant::now();
        loop {
            self.comprobar()?;
            if self.cabe() {
                break;
            }
            if empieza.elapsed() > PACIENCIA {
                return Err(mal("el codificador de vídeo no avanza"));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut bufer = gst::Buffer::from_mut_slice(rgba);
        if let Some(b) = bufer.get_mut() {
            b.set_pts(gst::ClockTime::from_mseconds(desde_ms));
            b.set_duration(gst::ClockTime::from_mseconds(dura_ms.max(1)));
        }
        if self.video.push_buffer(bufer).is_err() {
            self.comprobar()?;
            return Err(mal("el codificador de vídeo ya no acepta fotogramas"));
        }
        Ok(())
    }

    /// Mas sonido: PCM de 16 bits con los canales intercalados, a continuacion del anterior.
    pub fn sonido(&mut self, pcm: &[u8]) -> Result<()> {
        self.comprobar()?;
        let Some(sonido) = self.sonido.as_mut().filter(|s| !s.acabado) else { return Ok(()) };
        let muestras = (pcm.len() / sonido.bloque) as u64;
        if muestras == 0 {
            return Ok(());
        }
        let en_ns = |n: u64| gst::ClockTime::from_nseconds(n * 1_000_000_000 / sonido.frecuencia);
        let mut bufer = gst::Buffer::from_slice(pcm[..muestras as usize * sonido.bloque].to_vec());
        if let Some(b) = bufer.get_mut() {
            b.set_pts(en_ns(sonido.mandadas));
            b.set_duration(en_ns(sonido.mandadas + muestras) - en_ns(sonido.mandadas));
        }
        sonido.mandadas += muestras;
        if sonido.fuente.push_buffer(bufer).is_err() {
            self.comprobar()?;
            return Err(mal("el codificador de sonido ya no acepta nada"));
        }
        Ok(())
    }

    /// Ya no va a llegar mas sonido. Hay que decirlo en cuanto se sabe: quien junta el
    /// video con el sonido espera a los dos, y sin esto se quedaria esperando a este.
    pub fn fin_del_sonido(&mut self) {
        if let Some(sonido) = self.sonido.as_mut().filter(|s| !s.acabado) {
            sonido.acabado = true;
            let _ = sonido.fuente.end_of_stream();
        }
    }

    /// Cierra el archivo. Hasta que esto no vuelve bien, el MP4 no se puede abrir.
    pub fn cerrar(mut self) -> Result<()> {
        self.comprobar()?;
        self.fin_del_sonido();
        let _ = self.video.end_of_stream();
        let fin = self.bus.timed_pop_filtered(
            gst::ClockTime::from_seconds(PACIENCIA.as_secs()),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        );
        let resultado = match fin {
            Some(mensaje) => match mensaje.view() {
                gst::MessageView::Error(e) => Err(mal(format!("el MP4 no se ha podido cerrar: {}", e.error()))),
                _ => Ok(()),
            },
            None => Err(mal("el codificador no ha terminado a tiempo")),
        };
        let _ = self.tuberia.set_state(gst::State::Null);
        resultado
    }
}

impl Drop for Escritor {
    fn drop(&mut self) {
        let _ = self.tuberia.set_state(gst::State::Null);
    }
}

// --- Leer video ---

/// Lee un video en orden, fotograma a fotograma, ya en RGBA.
pub struct LectorVideo {
    tuberia: gst::Pipeline,
    salida: gst_app::AppSink,
    bus: gst::Bus,
    duracion_ms: Option<u64>,
    ultimo_ms: u64,
}

impl LectorVideo {
    pub fn abrir(ruta: &Path) -> Result<Self> {
        // Como mucho dos fotogramas descomprimidos esperando: se lee al ritmo de quien pide.
        let tuberia = monta(
            "filesrc name=f ! decodebin ! videoconvert ! video/x-raw,format=RGBA ! appsink name=s sync=false max-buffers=2",
        )?;
        pieza::<gst::Element>(&tuberia, "f")?.set_property("location", ruta.to_string_lossy().as_ref());
        let salida: gst_app::AppSink = pieza(&tuberia, "s")?;
        let bus = tuberia.bus().ok_or_else(|| mal("la tuberia no tiene por donde avisar"))?;
        let este = Self { tuberia, salida, bus, duracion_ms: None, ultimo_ms: 0 };
        if este.tuberia.set_state(gst::State::Playing).is_err() {
            let porque = error_de(&este.bus).unwrap_or_else(|| "GStreamer no ha querido empezar".into());
            return Err(mal(format!("ese vídeo no se puede abrir: {porque}")));
        }
        Ok(este)
    }
}

impl Drop for LectorVideo {
    fn drop(&mut self) {
        let _ = self.tuberia.set_state(gst::State::Null);
    }
}

impl Lector for LectorVideo {
    fn siguiente(&mut self) -> Result<Option<Cuadro>> {
        let empieza = Instant::now();
        let muestra = loop {
            if let Some(muestra) = self.salida.try_pull_sample(gst::ClockTime::from_mseconds(250)) {
                break muestra;
            }
            if let Some(porque) = error_de(&self.bus) {
                return Err(mal(format!("ese vídeo no se puede leer: {porque}")));
            }
            if self.salida.is_eos() {
                return Ok(None);
            }
            if empieza.elapsed() > PACIENCIA_AL_LEER {
                return Err(mal("ese vídeo no se puede leer: el descodificador no contesta"));
            }
        };
        let medida = |campo: &str| {
            muestra
                .caps()
                .and_then(|c| c.structure(0))
                .and_then(|s| s.get::<i32>(campo).ok())
                .filter(|&v| v > 0)
                .map(|v| v as u32)
                .ok_or_else(|| mal("ese vídeo no dice cuánto mide"))
        };
        let (ancho, alto) = (medida("width")?, medida("height")?);
        let bufer = muestra.buffer().ok_or_else(|| mal("un fotograma ha llegado vacío"))?;
        let desde_ms = bufer.pts().map(|t| t.mseconds()).unwrap_or(self.ultimo_ms);
        let dura_ms = bufer.duration().map(|t| t.mseconds()).filter(|&d| d > 0).unwrap_or(1000 / u64::from(FPS));
        let mapa = bufer.map_readable().map_err(|_| mal("un fotograma no se deja leer"))?;
        let fila = ancho as usize * 4;
        // Las filas pueden venir acolchadas: el paso sale de lo que ocupa el bufer.
        let paso = mapa.len() / alto as usize;
        if paso < fila {
            return Err(mal("un fotograma ha llegado más pequeño de lo que dice"));
        }
        let mut imagen = RgbaImage::new(ancho, alto);
        let destino: &mut [u8] = &mut imagen;
        for (hacia, desde) in destino.chunks_exact_mut(fila).zip(mapa.chunks_exact(paso)) {
            hacia.copy_from_slice(&desde[..fila]);
            // Un fondo no tiene transparencia, traiga lo que traiga el archivo.
            for pixel in hacia.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
        }
        if self.duracion_ms.is_none() {
            self.duracion_ms = self.tuberia.query_duration::<gst::ClockTime>().map(|t| t.mseconds()).filter(|&d| d > 0);
        }
        self.ultimo_ms = desde_ms;
        Ok(Some(Cuadro { desde_ms, dura_ms, imagen }))
    }

    fn leido(&self) -> f32 {
        match self.duracion_ms {
            Some(total) => (self.ultimo_ms as f32 / total as f32).clamp(0.0, 1.0),
            None => 0.0,
        }
    }
}

// --- Leer sonido ---

/// El sonido de cualquier archivo que sepa abrir GStreamer, descomprimido en coma flotante,
/// en un canal y hasta `segundos`. Devuelve las muestras y su frecuencia.
pub fn leer_sonido(ruta: &Path, segundos: f64) -> Result<(Vec<f32>, u32)> {
    let tuberia = monta(
        "filesrc name=f ! decodebin ! audioconvert ! audio/x-raw,format=F32LE,channels=1,layout=interleaved ! appsink name=s sync=false max-buffers=8",
    )?;
    pieza::<gst::Element>(&tuberia, "f")?.set_property("location", ruta.to_string_lossy().as_ref());
    let salida: gst_app::AppSink = pieza(&tuberia, "s")?;
    let bus = tuberia.bus().ok_or_else(|| mal("la tuberia no tiene por donde avisar"))?;
    let leido = (|| -> Result<(Vec<f32>, u32)> {
        if tuberia.set_state(gst::State::Playing).is_err() {
            let porque = error_de(&bus).unwrap_or_else(|| "GStreamer no ha querido empezar".into());
            return Err(mal(format!("ese sonido no se puede abrir: {porque}")));
        }
        let mut muestras: Vec<f32> = Vec::new();
        let mut frecuencia = 0u32;
        let mut hasta = usize::MAX;
        let empieza = Instant::now();
        while muestras.len() < hasta {
            let Some(muestra) = salida.try_pull_sample(gst::ClockTime::from_mseconds(250)) else {
                if let Some(porque) = error_de(&bus) {
                    return Err(mal(format!("ese sonido no se puede leer: {porque}")));
                }
                if salida.is_eos() {
                    break;
                }
                if empieza.elapsed() > PACIENCIA_AL_LEER {
                    return Err(mal("ese sonido no se puede leer: el descodificador no contesta"));
                }
                continue;
            };
            if frecuencia == 0 {
                frecuencia = muestra
                    .caps()
                    .and_then(|c| c.structure(0))
                    .and_then(|s| s.get::<i32>("rate").ok())
                    .filter(|&v| v > 0)
                    .map(|v| v as u32)
                    .ok_or_else(|| mal("ese sonido no dice a qué frecuencia va"))?;
                hasta = (segundos * f64::from(frecuencia)) as usize;
            }
            let Some(bufer) = muestra.buffer() else { continue };
            let mapa = bufer.map_readable().map_err(|_| mal("un trozo de sonido no se deja leer"))?;
            for b in mapa.chunks_exact(4) {
                if muestras.len() >= hasta {
                    break;
                }
                let m = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                muestras.push(if m.is_finite() { m.clamp(-1.0, 1.0) } else { 0.0 });
            }
        }
        if frecuencia == 0 {
            return Err(mal("ese archivo no trae sonido"));
        }
        Ok((muestras, frecuencia))
    })();
    let _ = tuberia.set_state(gst::State::Null);
    leido
}

/// Corre hasta el final una tuberia escrita a mano. Solo para preparar archivos en las
/// pruebas (un MP3 de verdad, por ejemplo).
#[cfg(test)]
pub(crate) fn correr(texto: &str) -> Result<()> {
    let tuberia = monta(texto)?;
    let bus = tuberia.bus().ok_or_else(|| mal("la tuberia no tiene por donde avisar"))?;
    let _ = tuberia.set_state(gst::State::Playing);
    let fin = bus.timed_pop_filtered(gst::ClockTime::from_seconds(30), &[gst::MessageType::Eos, gst::MessageType::Error]);
    let _ = tuberia.set_state(gst::State::Null);
    match fin {
        Some(mensaje) => match mensaje.view() {
            gst::MessageView::Error(e) => Err(mal(e.error().to_string())),
            _ => Ok(()),
        },
        None => Err(mal("la tuberia no ha terminado")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carpeta(nombre: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("winshotx-gst-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("carpeta de la prueba");
        dir
    }

    fn liso(ancho: u32, alto: u32, color: [u8; 3]) -> Vec<u8> {
        [color[0], color[1], color[2], 255].repeat((ancho * alto) as usize)
    }

    /// El color del centro de la imagen.
    fn centro(imagen: &RgbaImage) -> [u8; 3] {
        let p = imagen.get_pixel(imagen.width() / 2, imagen.height() / 2).0;
        [p[0], p[1], p[2]]
    }

    fn parecido(a: [u8; 3], b: [u8; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| x.abs_diff(y) < 40)
    }

    /// Un tono en PCM de 16 bits, dos canales.
    fn tono(frecuencia: u32, ms: u32) -> Vec<u8> {
        let muestras = frecuencia * ms / 1000;
        (0..muestras)
            .flat_map(|n| {
                let v = ((n as f32 * 440.0 * std::f32::consts::TAU / frecuencia as f32).sin() * 12_000.0) as i16;
                [v.to_le_bytes(), v.to_le_bytes()].concat()
            })
            .collect()
    }

    #[test]
    fn en_este_equipo_hay_con_que_escribir_un_mp4() {
        let hay = codificadores();
        assert_eq!(hay.video, Some("openh264enc"), "sin codificador de H.264 el MP4 no sale");
        assert!(hay.sonido.is_some(), "sin codificador de AAC el MP4 sale mudo");
    }

    #[test]
    fn un_mp4_escrito_se_vuelve_a_leer_con_cada_fotograma_en_su_tiempo() {
        let dir = carpeta("tiempos");
        let ruta = dir.join("tres.mp4");
        let medidas = Medidas { ancho: 320, alto: 240, fps: 30, bitrate: 2_000_000 };
        // Una pantalla roja quieta dos segundos, un instante verde y un segundo azul: tres
        // fotogramas, no noventa.
        let cuadros = [([255, 0, 0], 0u64, 2000u64), ([0, 255, 0], 2000, 100), ([0, 0, 255], 2100, 1000)];
        let mut escritor = Escritor::abrir(&ruta, &medidas, None).expect("abrir");
        for (color, desde, dura) in cuadros {
            escritor.imagen(liso(320, 240, color), desde, dura).expect("fotograma");
        }
        escritor.cerrar().expect("cerrar");

        let mut lector = LectorVideo::abrir(&ruta).expect("leer lo escrito");
        let mut leidos = Vec::new();
        while let Some(cuadro) = lector.siguiente().expect("fotograma leido") {
            assert_eq!(cuadro.imagen.dimensions(), (320, 240));
            leidos.push((centro(&cuadro.imagen), cuadro.desde_ms, cuadro.dura_ms));
        }
        assert_eq!(leidos.len(), 3, "ha salido {leidos:?}");
        for ((color, desde, dura), (visto, desde_leido, dura_leida)) in cuadros.iter().zip(&leidos) {
            assert!(parecido(*color, *visto), "se esperaba {color:?} y se lee {visto:?}");
            assert!(desde.abs_diff(*desde_leido) <= 2, "empieza en {desde_leido} y no en {desde}");
            assert!(dura.abs_diff(*dura_leida) <= 2, "dura {dura_leida} y no {dura}: {leidos:?}");
        }
        assert!(lector.leido() > 0.5, "el progreso no ha avanzado: {}", lector.leido());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_sonido_escrito_se_vuelve_a_oir_y_dura_lo_mismo() {
        let dir = carpeta("sonido");
        let ruta = dir.join("tono.mp4");
        let medidas = Medidas { ancho: 320, alto: 240, fps: 30, bitrate: 1_000_000 };
        let mut escritor = Escritor::abrir(&ruta, &medidas, Some((2, 48_000))).expect("abrir");
        // Un segundo: treinta fotogramas, cada uno con su trozo de tono detras.
        let pista = tono(48_000, 1000);
        let trozo = pista.len() / 30;
        for cuadro in 0..30u64 {
            escritor.imagen(liso(320, 240, [40, 90, 200]), cuadro * 33, 33).expect("fotograma");
            escritor.sonido(&pista[cuadro as usize * trozo..(cuadro as usize + 1) * trozo]).expect("sonido");
        }
        escritor.cerrar().expect("cerrar");

        let (muestras, frecuencia) = leer_sonido(&ruta, 10.0).expect("leer el sonido");
        let segundos = muestras.len() as f64 / f64::from(frecuencia);
        assert!((segundos - 1.0).abs() < 0.1, "dura {segundos} s");
        let fuerza = (muestras.iter().map(|m| m * m).sum::<f32>() / muestras.len() as f32).sqrt();
        assert!(fuerza > 0.1, "no suena: {fuerza}");
        // Y el video sigue ahi, con el sonido al lado.
        let mut lector = LectorVideo::abrir(&ruta).expect("leer el video");
        let mut cuantos = 0;
        while lector.siguiente().expect("fotograma").is_some() {
            cuantos += 1;
        }
        assert_eq!(cuantos, 30);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_sonido_puede_acabar_antes_que_el_video_sin_colgar_a_nadie() {
        let dir = carpeta("corto");
        let ruta = dir.join("corto.mp4");
        let medidas = Medidas { ancho: 320, alto: 240, fps: 30, bitrate: 1_000_000 };
        let mut escritor = Escritor::abrir(&ruta, &medidas, Some((2, 48_000))).expect("abrir");
        escritor.sonido(&tono(48_000, 200)).expect("sonido");
        escritor.fin_del_sonido();
        // Tres segundos de video detras de un sonido de dos decimas.
        for cuadro in 0..90u64 {
            escritor.imagen(liso(320, 240, [200, 200, 40]), cuadro * 33, 33).expect("fotograma");
        }
        escritor.cerrar().expect("cerrar");
        let mut lector = LectorVideo::abrir(&ruta).expect("leer");
        let mut cuantos = 0;
        while lector.siguiente().expect("fotograma").is_some() {
            cuantos += 1;
        }
        assert_eq!(cuantos, 90);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn escribir_donde_no_se_puede_devuelve_un_error_y_no_se_queda_colgado() {
        let dir = carpeta("imposible");
        let ruta = dir.join("no-existe").join("nada.mp4");
        let medidas = Medidas { ancho: 320, alto: 240, fps: 30, bitrate: 1_000_000 };
        let empieza = Instant::now();
        let resultado = Escritor::abrir(&ruta, &medidas, None).and_then(|mut escritor| {
            for cuadro in 0..30u64 {
                escritor.imagen(liso(320, 240, [1, 2, 3]), cuadro * 33, 33)?;
            }
            escritor.cerrar()
        });
        let error = resultado.expect_err("no se puede escribir en una carpeta que no existe").to_string();
        assert!(error.starts_with("el MP4 no se "), "{error}");
        assert!(empieza.elapsed() < Duration::from_secs(20), "ha tardado {:?}", empieza.elapsed());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lo_que_no_es_un_video_dice_que_no_se_puede_leer() {
        let dir = carpeta("basura");
        let ruta = dir.join("basura.mp4");
        std::fs::write(&ruta, b"esto no es un video, por mucho que se llame mp4").expect("escribir");
        let leido = LectorVideo::abrir(&ruta).and_then(|mut lector| lector.siguiente());
        let error = leido.err().expect("la basura no se lee").to_string();
        assert!(error.starts_with("ese vídeo no se puede "), "{error}");
        let error = leer_sonido(&ruta, 1.0).expect_err("ni se oye").to_string();
        assert!(error.starts_with("ese sonido no se puede "), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_wav_se_lee_en_un_canal_y_solo_hasta_donde_se_pide() {
        let dir = carpeta("wav");
        let ruta = dir.join("tono.wav");
        let pcm = tono(44_100, 2000);
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&44_100u32.to_le_bytes());
        wav.extend_from_slice(&(44_100u32 * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(&pcm);
        std::fs::write(&ruta, wav).expect("escribir el WAV");
        let (muestras, frecuencia) = leer_sonido(&ruta, 0.5).expect("leer");
        assert_eq!(frecuencia, 44_100);
        assert_eq!(muestras.len(), 22_050, "medio segundo y ni una muestra mas");
        assert!(muestras.iter().any(|m| m.abs() > 0.2), "no suena");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
