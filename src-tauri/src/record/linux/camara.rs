//! La webcam en Linux, leída con el GStreamer del sistema.
//!
//! Hace lo mismo que `record/camara.rs` en Windows, que es donde se cuenta el porqué de cada
//! paso: se enciende antes de grabar, tira lo que da hasta que se le manda guardar, y deja
//! `camara.mp4` en la carpeta de la sesión con sus marcas de tiempo ya sin las pausas. Lo que
//! no depende del sistema (`Toma`, `Encendida`, `Grabadora`, qué formato se elige) es el
//! mismo código, el de `camara_comun`.
//!
//! **De dónde sale la cámara.** No se abre `/dev/video0` a mano. Se le pregunta a GStreamer
//! qué fuentes de vídeo hay (`DeviceMonitor`, clase `Video/Source`) y es él quien crea la
//! pieza que la lee: `pipewiresrc` donde las cámaras van por PipeWire (los escritorios de
//! hoy, y la única forma de ver las de libcamera) y `v4l2src` donde no. Cuando los dos ven
//! la misma cámara, el proveedor de PipeWire esconde al de V4L2, así que no sale dos veces.
//! No se pasa por el portal `Camera`: es para las apps encerradas (Flatpak), y fuera de un
//! sandbox la sesión de PipeWire ya deja ver las cámaras.
//!
//! **El camino.** La fuente, en el formato elegido (`elegir`: 720p a 30, como en Windows),
//! se descomprime si viene en JPEG, se pasa a RGBA y llega aquí por un `appsink`. Cada imagen
//! pasa por `Toma` (las marcas, la pausa) al mismo `Escritor` que escribe la vista previa:
//! H.264 con el codificador del sistema. Sin codificador no hay cámara, igual que no hay MP4.
//!
//! NO PROBADO con una cámara de verdad ni por V4L2: el laboratorio no tiene (el núcleo de
//! WSL no trae V4L2). Probado con una fuente de GStreamer que hace de cámara y con una
//! cámara de mentira publicada en PipeWire.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;

use crate::encode::gst as motor;
use crate::error::Result;
use crate::record::camara_comun::{bitrate, elegir, Formato, Orden, Toma};
pub use crate::record::camara_comun::{desfase, AlEsperar, Dispositivo, Encendida, Grabadora, Pista, ARCHIVO};

/// El alto más grande que se graba. Una cámara que solo dé 4K se encoge: aquí cada imagen
/// pasa por la CPU, y la cara va en una esquina del vídeo.
const ALTO_MAXIMO: u32 = 1080;

// --- Qué cámaras hay ---

/// Las cámaras que ve GStreamer, sin encender ninguna, con la que el sistema tiene por
/// defecto delante: «la primera» es esa.
fn camaras() -> Result<Vec<gst::Device>> {
    motor::encender()?;
    let monitor = gst::DeviceMonitor::new();
    if monitor.add_filter(Some("Video/Source"), None).is_none() {
        // Ningún proveedor sabe de fuentes de vídeo: faltan los plugins.
        return Ok(Vec::new());
    }
    // Arrancado es como el proveedor de PipeWire dice a quién esconde. Si no arranca ninguno
    // (no hay PipeWire en marcha) se pregunta igual, que el de V4L2 sabe contestar parado.
    let arrancado = monitor.start().is_ok();
    let mut lista: Vec<gst::Device> = monitor.devices().into_iter().filter(es_camara).collect();
    if arrancado {
        monitor.stop();
    }
    lista.sort_by_key(|d| !por_defecto(d));
    Ok(lista)
}

/// En PipeWire también es una «fuente de vídeo» la pantalla que alguien está compartiendo
/// (la nuestra, mientras se graba en Wayland). Las cámaras llevan `media.role=Camera`: se lo
/// pone el propio PipeWire a las de V4L2 y a las de libcamera, y es por lo que las reconoce
/// el portal. Lo que no viene de PipeWire (V4L2 directo) son todas cámaras.
fn es_camara(dispositivo: &gst::Device) -> bool {
    match dispositivo.properties() {
        Some(p) if p.name() == "pipewire-proplist" => p.get::<&str>("media.role").is_ok_and(|rol| rol == "Camera"),
        _ => true,
    }
}

fn por_defecto(dispositivo: &gst::Device) -> bool {
    dispositivo.properties().and_then(|p| p.get::<bool>("is-default").ok()).unwrap_or(false)
}

/// Con lo que se reconoce la cámara aunque cambie de sitio en la lista: el nombre del nodo
/// en PipeWire, y por V4L2 el puerto en el que está enchufada.
fn id_de(dispositivo: &gst::Device) -> String {
    let propiedades = dispositivo.properties();
    ["node.name", "device.bus_path", "device.path"]
        .iter()
        .find_map(|clave| propiedades.as_ref()?.get::<String>(*clave).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| dispositivo.display_name().to_string())
}

fn nombre_de(dispositivo: &gst::Device) -> String {
    let nombre = dispositivo.display_name().to_string();
    if nombre.is_empty() || nombre == "unknown" {
        "Cámara".into()
    } else {
        nombre
    }
}

/// Las cámaras que hay, sin encender ninguna: preguntar su nombre no enciende la luz.
pub fn listar() -> Result<Vec<Dispositivo>> {
    // En un hilo suyo, como en Windows: preguntar habla con PipeWire y con udev, y no tiene
    // por qué hacerlo desde el hilo de quien llama.
    std::thread::spawn(|| -> Result<Vec<Dispositivo>> {
        Ok(camaras()?.iter().map(|d| Dispositivo { id: id_de(d), nombre: nombre_de(d) }).collect())
    })
    .join()
    .map_err(|_| motor::mal("se ha caído el hilo que buscaba cámaras"))?
}

/// La cámara elegida, o la primera si esa ya no está (se ha desenchufado, o nunca se eligió
/// ninguna).
fn buscar(id: Option<&str>) -> Result<gst::Device> {
    let lista = camaras()?;
    id.and_then(|id| lista.iter().find(|d| id_de(d) == id))
        .or_else(|| lista.first())
        .cloned()
        .ok_or_else(|| motor::mal("no hay ninguna cámara conectada"))
}

// --- Qué formato se graba ---

/// Lo que se le pide a la cámara: uno de sus formatos, y a qué tamaño llega aquí.
#[derive(Debug, Clone, PartialEq)]
struct Pedido {
    /// Los caps que lo piden tal cual: tipo, tamaño y fotogramas por segundo.
    caps: gst::Caps,
    comprimido: bool,
    /// El tamaño al que se graba: el suyo, par y como mucho `ALTO_MAXIMO` de alto.
    medida: (u32, u32),
    fps: u32,
}

/// El valor de ese campo más cercano a `meta`: el suyo si es fijo, y si la cámara da a
/// elegir (un intervalo o una lista), el que más se acerque.
fn entero(formato: &gst::StructureRef, campo: &str, meta: i32) -> Option<i32> {
    let valor = formato.value(campo).ok()?;
    if let Ok(fijo) = valor.get::<i32>() {
        return Some(fijo);
    }
    if let Ok(intervalo) = valor.get::<gst::IntRange<i32>>() {
        let paso = intervalo.step().max(1);
        let dentro = meta.clamp(intervalo.min(), intervalo.max());
        return Some(intervalo.min() + (dentro - intervalo.min()) / paso * paso);
    }
    valor
        .get::<gst::List>()
        .ok()?
        .as_slice()
        .iter()
        .filter_map(|v| v.get::<i32>().ok())
        .min_by_key(|v| (v - meta).abs())
}

/// Los fotogramas por segundo que da ese formato. De un intervalo, 30 si caben o el extremo
/// más cercano. Vacío si no lo dice.
fn ritmos(formato: &gst::StructureRef) -> Vec<gst::Fraction> {
    let Ok(valor) = formato.value("framerate") else { return Vec::new() };
    if let Ok(fijo) = valor.get::<gst::Fraction>() {
        return vec![fijo];
    }
    if let Ok(intervalo) = valor.get::<gst::FractionRange>() {
        return vec![gst::Fraction::new(30, 1).clamp(intervalo.min(), intervalo.max())];
    }
    match valor.get::<gst::List>() {
        Ok(lista) => lista.as_slice().iter().filter_map(|v| v.get::<gst::Fraction>().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

/// Cuál de los formatos que da la cámara se graba, con el mismo criterio que en Windows
/// (`elegir`). `None` si no da ninguno que se sepa leer: sin comprimir o en JPEG.
fn pedido_de(caps: &gst::Caps) -> Option<Pedido> {
    let mut formatos = Vec::new();
    let mut pedidos = Vec::new();
    for formato in caps.iter() {
        let comprimido = match formato.name().as_str() {
            "video/x-raw" => false,
            "image/jpeg" => true,
            _ => continue,
        };
        let (Some(ancho), Some(alto)) = (entero(formato, "width", 1280), entero(formato, "height", 720)) else {
            continue;
        };
        if ancho < 2 || alto < 2 {
            continue;
        }
        let (ancho, alto) = (ancho as u32, alto as u32);
        let mut apuntar = |fps: f64, ritmo: Option<gst::Fraction>| {
            let mut caps = gst::Caps::builder(formato.name().as_str()).field("width", ancho as i32).field("height", alto as i32);
            if let Some(ritmo) = ritmo {
                caps = caps.field("framerate", ritmo);
            }
            formatos.push(Formato { ancho, alto, fps, comprimido });
            pedidos.push(Pedido {
                caps: caps.build(),
                comprimido,
                medida: medida_de(ancho, alto),
                fps: (fps.round() as u32).clamp(1, 60),
            });
        };
        let ritmos = ritmos(formato);
        if ritmos.is_empty() {
            // No dice a cuánto va: se cuenta como 30, que es lo que dan casi todas.
            apuntar(30.0, None);
            continue;
        }
        for ritmo in ritmos.into_iter().filter(|r| r.numer() > 0 && r.denom() > 0) {
            apuntar(f64::from(ritmo.numer()) / f64::from(ritmo.denom()), Some(ritmo));
        }
    }
    elegir(&formatos).map(|i| pedidos.swap_remove(i))
}

/// El tamaño al que se graba una imagen de la cámara: par, que H.264 no acepta un lado
/// impar, y como mucho `ALTO_MAXIMO` de alto.
fn medida_de(ancho: u32, alto: u32) -> (u32, u32) {
    let (ancho, alto) = crate::encode::escalar::medida_para(ancho, alto, ALTO_MAXIMO).unwrap_or((ancho, alto));
    crate::encode::mp4::medida_que_acepta((ancho / 2 * 2).max(2), (alto / 2 * 2).max(2))
}

// --- La cámara abierta ---

/// De dónde sale la imagen.
enum Fuente {
    /// Una cámara de las que ve GStreamer.
    Camara(gst::Device),
    /// Una fuente descrita a mano, como en `gst-launch`: las pruebas ponen una que hace de
    /// cámara. `dice` son los formatos que dice dar, si no son los que da de verdad.
    #[cfg(test)]
    Descrita { fuente: String, dice: Option<String> },
}

impl Fuente {
    /// La pieza que da las imágenes, sin encender.
    fn pieza(&self) -> Result<gst::Element> {
        match self {
            Fuente::Camara(dispositivo) => dispositivo
                .create_element(None)
                .map_err(|e| motor::mal(format!("GStreamer no puede abrir esa cámara: {e}"))),
            #[cfg(test)]
            Fuente::Descrita { fuente, .. } => Ok(gst::parse_bin_from_description(fuente, true)
                .map_err(|e| motor::mal(format!("GStreamer no puede montar esto: {e}")))?
                .upcast::<gst::Element>()),
        }
    }

    /// Los formatos que dice dar, sin encenderla.
    fn formatos(&self) -> Option<gst::Caps> {
        match self {
            Fuente::Camara(dispositivo) => dispositivo.caps(),
            #[cfg(test)]
            Fuente::Descrita { dice: Some(texto), .. } => texto.parse().ok(),
            #[cfg(test)]
            Fuente::Descrita { .. } => self.pieza().ok()?.static_pad("src").map(|salida| salida.query_caps(None)),
        }
    }
}

/// Una imagen de la cámara, ya en RGBA.
struct Imagen {
    rgba: Vec<u8>,
    ancho: u32,
    alto: u32,
    /// Cuándo la sacó la cámara, con el reloj de su tubería, en cien nanosegundos: las
    /// unidades de `Toma`.
    marca: i64,
    /// Lo que ha tardado en llegar aquí desde que se sacó.
    retraso_ms: u64,
    dura_ms: u64,
}

/// Lo que da una lectura de la cámara.
enum Lectura {
    Imagen(Imagen),
    /// Todavía no hay imagen: no es un error.
    Nada,
    /// La cámara se ha acabado (se ha desenchufado).
    Fin,
}

/// La cámara encendida y dando imágenes. Al soltarla se apaga, y con ella su luz.
struct Abierta {
    tuberia: gst::Pipeline,
    salida: gst_app::AppSink,
    bus: gst::Bus,
}

impl Abierta {
    /// Enciende la fuente. Con `pedido`, en ese formato; sin él, en el que dé ella.
    fn abrir(fuente: &Fuente, pedido: Option<&Pedido>) -> Result<Self> {
        motor::encender()?;
        let origen = fuente.pieza()?;
        // Dos imágenes esperando como mucho, y si no se llega a leerlas se tira la vieja: es
        // una cámara en vivo, y una imagen atrasada ya no vale.
        const SALIDA: &str = "videoconvert ! videoscale ! capsfilter name=r ! appsink name=s sync=false max-buffers=2 drop=true";
        // Sin pedido, delante va un `decodebin` que se pone aparte, más abajo: escrito aquí,
        // GStreamer adivina mal por dónde entra este trozo (ve dos entradas sin enlazar, la
        // suya y la de `videoconvert`, que espera a que el `decodebin` saque su salida).
        let texto = match pedido {
            Some(p) if p.comprimido => format!("capsfilter name=c ! jpegdec ! {SALIDA}"),
            Some(_) => format!("capsfilter name=c ! {SALIDA}"),
            None => SALIDA.to_string(),
        };
        let resto = gst::parse_bin_from_description(&texto, true)
            .map_err(|e| motor::mal(format!("GStreamer no puede leer la cámara: {e}")))?;
        let falta = |nombre: &str| motor::mal(format!("a la tubería de la cámara le falta «{nombre}»"));
        let mut llega = gst::Caps::builder("video/x-raw").field("format", "RGBA");
        if let Some(p) = pedido {
            resto.by_name("c").ok_or_else(|| falta("c"))?.set_property("caps", &p.caps);
            llega = llega.field("width", p.medida.0 as i32).field("height", p.medida.1 as i32);
        }
        resto.by_name("r").ok_or_else(|| falta("r"))?.set_property("caps", llega.build());
        let salida = resto
            .by_name("s")
            .and_then(|s| s.downcast::<gst_app::AppSink>().ok())
            .ok_or_else(|| falta("s"))?;

        let tuberia = gst::Pipeline::new();
        tuberia
            .add_many([&origen, resto.upcast_ref::<gst::Element>()])
            .map_err(|e| motor::mal(format!("la cámara no se puede conectar: {e}")))?;
        // Se enlaza sin mirar si los formatos casan: eso se sabe al encenderla, y si no casan
        // llega como un error de la tubería, que es donde se le da la segunda oportunidad
        // (`vivir`). Mirándolo aquí, una cámara que dice dar un formato que no da ni se
        // encendería.
        let (Some(sale), Some(entra)) = (origen.static_pad("src"), resto.static_pad("sink")) else {
            return Err(motor::mal("la cámara no tiene por donde dar la imagen"));
        };
        let no_conecta = |e: &dyn std::fmt::Display| motor::mal(format!("la cámara no se puede conectar: {e}"));
        if pedido.is_some() {
            sale.link_full(&entra, gst::PadLinkCheck::HIERARCHY).map_err(|e| no_conecta(&e))?;
        } else {
            // En el formato que dé ella: `decodebin` lo descomprime si hace falta, y saca su
            // salida cuando ya sabe qué le llega. Entonces se enlaza con el resto.
            let lector = gst::ElementFactory::make("decodebin").build().map_err(|e| no_conecta(&e))?;
            tuberia.add(&lector).map_err(|e| no_conecta(&e))?;
            lector.connect_pad_added(move |_, salida| {
                if !entra.is_linked() {
                    let _ = salida.link(&entra);
                }
            });
            let a_leer = lector.static_pad("sink").ok_or_else(|| no_conecta(&"el lector no tiene entrada"))?;
            sale.link_full(&a_leer, gst::PadLinkCheck::HIERARCHY).map_err(|e| no_conecta(&e))?;
        }
        let bus = tuberia.bus().ok_or_else(|| motor::mal("la tubería no tiene por donde avisar"))?;
        let esta = Self { tuberia, salida, bus };
        if esta.tuberia.set_state(gst::State::Playing).is_err() {
            let porque = motor::error_de(&esta.bus).unwrap_or_else(|| "GStreamer no ha querido empezar".into());
            return Err(motor::mal(format!("la cámara no se puede encender: {porque}")));
        }
        Ok(esta)
    }

    /// La siguiente imagen, esperándola como mucho una décima: quien llama mira entre una y
    /// otra si tiene que parar.
    fn leer(&self) -> Result<Lectura> {
        let Some(muestra) = self.salida.try_pull_sample(gst::ClockTime::from_mseconds(100)) else {
            if let Some(porque) = motor::error_de(&self.bus) {
                return Err(motor::mal(format!("la cámara ha fallado: {porque}")));
            }
            return Ok(if self.salida.is_eos() { Lectura::Fin } else { Lectura::Nada });
        };
        let medida = |campo: &str| {
            muestra
                .caps()
                .and_then(|c| c.structure(0))
                .and_then(|s| s.get::<i32>(campo).ok())
                .filter(|&v| v > 0)
                .map(|v| v as u32)
                .ok_or_else(|| motor::mal("la cámara no dice cuánto mide su imagen"))
        };
        let (ancho, alto) = (medida("width")?, medida("height")?);
        let Some(bufer) = muestra.buffer() else { return Ok(Lectura::Nada) };
        // La fuente marca cada imagen con el reloj de la tubería en el instante en que se
        // sacó: lo que va de esa marca a ahora es lo que ha tardado en llegar. Una marca
        // disparatada (o ninguna) cuenta como recién sacada.
        let ahora = self.tuberia.current_running_time();
        let marca = bufer.pts().or(ahora).unwrap_or(gst::ClockTime::ZERO);
        let retraso_ms = ahora.and_then(|a| a.checked_sub(marca)).map_or(0, |r| r.mseconds());
        let dura_ms = bufer.duration().map(|d| d.mseconds()).filter(|&d| d > 0).unwrap_or(33);
        let mapa = bufer.map_readable().map_err(|_| motor::mal("una imagen de la cámara no se deja leer"))?;
        let fila = ancho as usize * 4;
        // Las filas pueden venir acolchadas: el paso sale de lo que ocupa el búfer.
        let paso = mapa.len() / alto as usize;
        if paso < fila {
            return Err(motor::mal("una imagen de la cámara ha llegado más pequeña de lo que dice"));
        }
        let mut rgba = Vec::with_capacity(fila * alto as usize);
        for desde in mapa.chunks_exact(paso) {
            rgba.extend_from_slice(&desde[..fila]);
        }
        Ok(Lectura::Imagen(Imagen {
            rgba,
            ancho,
            alto,
            marca: (marca.nseconds() / 100) as i64,
            retraso_ms: if retraso_ms <= 1_000 { retraso_ms } else { 0 },
            dura_ms,
        }))
    }
}

impl Drop for Abierta {
    fn drop(&mut self) {
        let _ = self.tuberia.set_state(gst::State::Null);
    }
}

// --- La grabación ---

impl Encendida {
    /// Enciende la cámara elegida (o la primera) y deja listo `carpeta/camara.mp4`. Vuelve
    /// enseguida.
    pub fn encender(dispositivo: Option<String>, carpeta: &Path) -> Self {
        Self::con(carpeta.join(ARCHIVO), move |ruta, parar, aviso, orden| {
            let fuente = Fuente::Camara(buscar(dispositivo.as_deref())?);
            vivir(&fuente, ruta, parar, aviso, orden)
        })
    }
}

/// Lo que hace la cámara mientras está encendida, como en Windows: hasta que le llega la
/// orden de grabar tira lo que da (y con la primera imagen, ya con su archivo abierto, avisa
/// de que se puede empezar); desde la orden, lo pasa por `Toma` al archivo.
fn vivir(
    fuente: &Fuente,
    ruta: &Path,
    parar: &AtomicBool,
    aviso: mpsc::Sender<()>,
    orden: mpsc::Receiver<Orden>,
) -> Result<Pista> {
    let encendida = Instant::now();
    // GStreamer encendido del todo antes de preguntarle nada. Una cámara de verdad ya viene de
    // uno encendido (`camaras`) y esto no hace nada; la de mentira de las pruebas no, y
    // preguntaba sus formatos sin él o, peor, mientras otro hilo lo encendía: GStreamer dice
    // que lo está antes de cargar sus plugins, y ahí no existe ni «videotestsrc».
    motor::encender()?;
    let mut pedido = fuente.formatos().as_ref().and_then(pedido_de);
    let mut camara = Abierta::abrir(fuente, pedido.as_ref())?;
    let mut aviso = Some(aviso);
    let mut grabando: Option<Orden> = None;
    let mut toma = Toma::default();
    let mut archivo: Option<(motor::Escritor, u32, u32)> = None;
    let mut ultimo_ms: Option<u64> = None;

    while !parar.load(Ordering::Relaxed) {
        let imagen = match camara.leer() {
            Ok(Lectura::Imagen(imagen)) => imagen,
            Ok(Lectura::Nada) => continue,
            Ok(Lectura::Fin) => break,
            // Si la cámara no arranca en el formato elegido, se queda con el suyo de siempre.
            Err(error) if pedido.is_some() && archivo.is_none() => {
                let caps = pedido.take().map(|p| p.caps.to_string()).unwrap_or_default();
                eprintln!("[winshotx] la cámara no acepta «{caps}» ({error}): va en el formato que dé");
                drop(camara);
                camara = Abierta::abrir(fuente, None)?;
                continue;
            }
            Err(error) => return Err(error),
        };
        let (escritor, ancho, alto) = match archivo.as_mut() {
            Some(abierto) => abierto,
            None => {
                // El archivo se abre con la primera imagen, que es la que dice cuánto mide, y
                // antes de avisar de que hay cámara: poner en marcha el codificador tarda, y
                // abierto al empezar a grabar la cara saldría quieta ese rato.
                let (ancho, alto) = (imagen.ancho, imagen.alto);
                if medida_de(ancho, alto) != (ancho, alto) {
                    return Err(motor::mal(format!("la cámara da una imagen de {ancho}x{alto}, que no se puede grabar")));
                }
                let fps = pedido.as_ref().map_or(30, |p| p.fps);
                let medidas = motor::Medidas { ancho, alto, fps, bitrate: bitrate(ancho, alto, f64::from(fps)) };
                let llego = encendida.elapsed();
                let escritor = motor::Escritor::abrir(ruta, &medidas, None)?;
                eprintln!(
                    "[winshotx] la cámara: primera imagen ({ancho}x{alto}) a los {} ms y su archivo a los {} ms",
                    llego.as_millis(),
                    encendida.elapsed().as_millis()
                );
                archivo.insert((escritor, ancho, alto))
            }
        };
        if let Some(aviso) = aviso.take() {
            let _ = aviso.send(());
        }
        if grabando.is_none() {
            match orden.try_recv() {
                Ok(o) => grabando = Some(o),
                // Encendida y sin grabar todavía: la imagen se tira.
                Err(mpsc::TryRecvError::Empty) => continue,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        let Some(o) = grabando.as_ref() else { continue };
        // Una imagen de otro tamaño no cabe en este archivo, y si el codificador va por
        // detrás se salta esta antes que frenar a la cámara: la siguiente llega enseguida.
        if (imagen.ancho, imagen.alto) != (*ancho, *alto) || !escritor.cabe() {
            continue;
        }
        let Some(sello) = toma.sello(imagen.marca, o.reloj.ms(), imagen.retraso_ms, o.pausa.load(Ordering::Relaxed)) else {
            continue;
        };
        // El archivo va en milisegundos, y siempre hacia delante.
        let ms = (sello.max(0) as u64 / 10_000).max(ultimo_ms.map_or(0, |u| u + 1));
        ultimo_ms = Some(ms);
        escritor.imagen(imagen.rgba, ms, imagen.dura_ms)?;
    }
    // La cámara se apaga ya: cerrar el archivo puede tardar, y su luz no tiene que esperar.
    drop(camara);

    let (Some(primer_ms), Some((escritor, ancho, alto))) = (toma.primer_ms, archivo) else {
        // Se paró antes de que la cámara diera imagen (encenderla tarda), o la grabación no
        // llegó a empezar: no hay archivo que cerrar, y uno sin imágenes no lo abre nadie.
        let por_que = if grabando.is_some() {
            "la cámara no ha llegado a dar imagen antes de parar"
        } else {
            "la grabación no ha llegado a empezar"
        };
        return Err(motor::mal(por_que));
    };
    escritor.cerrar()?;
    Ok(Pista { archivo: ARCHIVO.into(), ancho, alto, primer_ms })
}

/// La orden de los ajustes: qué cámaras hay, para elegir una.
#[tauri::command]
pub async fn list_cameras() -> Result<Vec<Dispositivo>> {
    listar()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU64;
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::encode::fondo_video::Lector;
    use crate::record::reloj::Reloj;

    fn carpeta(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("winshotx-camara-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("carpeta de la prueba");
        dir
    }

    fn reloj_desde_ya() -> Reloj {
        Reloj { start: Instant::now(), paused_ms: Arc::new(AtomicU64::new(0)) }
    }

    fn caps(texto: &str) -> gst::Caps {
        motor::encender().expect("GStreamer");
        texto.parse().expect("caps de la prueba")
    }

    /// Una fuente de GStreamer que hace de cámara: en vivo, con sus marcas de tiempo, y dando
    /// a elegir tamaño y ritmo como una de verdad. No enciende ninguna. `dice` son los
    /// formatos que dice dar, si no son los que da. Devuelve también dónde deja su archivo.
    fn de_mentira(nombre: &str, fuente: &str, dice: Option<&str>) -> (Encendida, PathBuf) {
        let ruta = carpeta(nombre).join(ARCHIVO);
        let fuente = Fuente::Descrita { fuente: fuente.into(), dice: dice.map(str::to_string) };
        let camara = Encendida::con(ruta.clone(), move |ruta, parar, aviso, orden| vivir(&fuente, ruta, parar, aviso, orden));
        (camara, ruta)
    }

    /// Lo que hay en `camara.mp4`: cuánto mide, cuántas imágenes, cuánto dura y el color del
    /// centro de la última.
    fn leer(ruta: &Path) -> ((u32, u32), usize, u64, [u8; 3]) {
        let mut lector = motor::LectorVideo::abrir(ruta).expect("abrir camara.mp4");
        let (mut medida, mut cuantas, mut fin, mut centro) = ((0, 0), 0, 0, [0u8; 3]);
        while let Some(cuadro) = lector.siguiente().expect("una imagen de camara.mp4") {
            medida = cuadro.imagen.dimensions();
            cuantas += 1;
            fin = cuadro.desde_ms + cuadro.dura_ms;
            let p = cuadro.imagen.get_pixel(medida.0 / 2, medida.1 / 2).0;
            centro = [p[0], p[1], p[2]];
        }
        (medida, cuantas, fin, centro)
    }

    /// Lo mismo que elige Windows con una webcam corriente por USB 2: en YUY2 solo llega a
    /// 720p a 10 fotogramas, y en MJPG a 30. Se queda con el MJPG a 720p y a 30.
    #[test]
    fn de_los_formatos_de_una_webcam_se_queda_con_720p_a_30() {
        let pedido = pedido_de(&caps(
            "video/x-raw, format=YUY2, width=640, height=480, framerate={ (fraction)30/1, (fraction)15/1 }; \
             video/x-raw, format=YUY2, width=1280, height=720, framerate=10/1; \
             image/jpeg, width=1280, height=720, framerate={ (fraction)30/1, (fraction)15/1 }; \
             image/jpeg, width=1920, height=1080, framerate=30/1; \
             video/x-h264, width=1920, height=1080, framerate=30/1",
        ))
        .expect("alguno tiene que valer");
        assert_eq!(pedido.caps, caps("image/jpeg, width=1280, height=720, framerate=30/1"));
        assert!(pedido.comprimido);
        assert_eq!((pedido.medida, pedido.fps), ((1280, 720), 30));
    }

    /// Una fuente que deja elegir (PipeWire lo da así a veces): se le pide 720p a 30.
    #[test]
    fn si_la_camara_deja_elegir_se_le_pide_720p_a_30() {
        let pedido = pedido_de(&caps(
            "video/x-raw, format=YUY2, width=[ 16, 4096 ], height=[ 16, 2160 ], framerate=[ 1/1, 120/1 ]",
        ))
        .expect("vale");
        assert_eq!(pedido.caps, caps("video/x-raw, width=1280, height=720, framerate=30/1"));
        assert!(!pedido.comprimido);
    }

    /// Una cámara que solo da 4K se graba encogida a 1080 de alto, y una de lado impar, par.
    #[test]
    fn lo_enorme_se_encoge_y_lo_impar_se_hace_par() {
        let pedido = pedido_de(&caps("image/jpeg, width=3840, height=2160, framerate=30/1")).expect("vale");
        assert_eq!(pedido.medida, (1920, 1080));
        assert_eq!(pedido.caps, caps("image/jpeg, width=3840, height=2160, framerate=30/1"));
        assert_eq!(medida_de(641, 481), (640, 480));
    }

    #[test]
    fn sin_un_formato_que_se_sepa_leer_no_se_pide_nada() {
        assert_eq!(pedido_de(&caps("video/x-h264, width=1920, height=1080, framerate=30/1")), None);
        assert_eq!(pedido_de(&gst::Caps::new_empty()), None);
    }

    /// Preguntar qué cámaras hay no revienta, haya PipeWire en marcha o no, y no enciende
    /// ninguna.
    #[test]
    fn listar_las_camaras_no_revienta() {
        let camaras = listar().expect("GStreamer tendría que contestar");
        println!("cámaras: {camaras:?}");
        assert!(camaras.iter().all(|c| !c.nombre.is_empty() && !c.id.is_empty()));
    }

    /// **Encendida antes, la cara está viva desde el cero.** Lo que da mientras no se graba
    /// se tira, y al grabar el archivo empieza en el cero de la grabación (`primer_ms` cerca
    /// de 0), dura lo grabado y mide lo que se le pidió a la cámara. Y la imagen es la suya:
    /// azul, no roja (los colores no llegan cambiados de sitio).
    #[test]
    fn encendida_antes_la_cara_esta_desde_el_cero() {
        let (mut camara, ruta) = de_mentira("desde-el-cero", "videotestsrc is-live=true pattern=blue", None);
        assert_eq!(camara.esperar_imagen(Duration::from_secs(20)), AlEsperar::Lista);
        assert!(ruta.exists(), "el archivo se abre antes de avisar de que hay cámara");
        // Encendida y sin grabar: lo que da se tira.
        std::thread::sleep(Duration::from_millis(700));
        let grabadora = camara.grabar(reloj_desde_ya(), Arc::new(AtomicBool::new(false)));
        std::thread::sleep(Duration::from_secs(1));
        let pista = grabadora.terminar().expect("tendría que haber grabado");
        assert_eq!((pista.archivo.as_str(), pista.ancho, pista.alto), (ARCHIVO, 1280, 720));
        assert!(pista.primer_ms < 150, "la primera imagen entra a los {} ms", pista.primer_ms);
        let (medida, cuantas, fin, centro) = leer(&ruta);
        assert_eq!(medida, (1280, 720));
        assert!((20..=34).contains(&cuantas), "un segundo a 30 son unas 30 imágenes, y hay {cuantas}");
        assert!((850..=1250).contains(&fin), "tendría que durar lo grabado (1 s), sin los 0,7 s de antes, y dura {fin} ms");
        assert!(centro[2] > 150 && centro[0] < 100, "la imagen tendría que ser azul: {centro:?}");
        let _ = std::fs::remove_dir_all(ruta.parent().unwrap());
    }

    /// Un rato de pausa en la grabación no es un rato de cámara: el archivo dura lo grabado
    /// sin la pausa, y lo que la cámara dio mientras tanto no está.
    #[test]
    fn la_pausa_de_la_grabacion_no_esta_en_el_archivo() {
        let (mut camara, ruta) = de_mentira("pausa", "videotestsrc is-live=true pattern=ball", None);
        assert_eq!(camara.esperar_imagen(Duration::from_secs(20)), AlEsperar::Lista);
        let pausa = Arc::new(AtomicBool::new(false));
        let grabadora = camara.grabar(reloj_desde_ya(), pausa.clone());
        std::thread::sleep(Duration::from_millis(600));
        pausa.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(1200));
        pausa.store(false, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(600));
        grabadora.terminar().expect("tendría que haber grabado");
        let (_, cuantas, fin, _) = leer(&ruta);
        assert!((1000..=1500).contains(&fin), "1,2 s grabados y 1,2 de pausa: dura {fin} ms");
        assert!((24..=42).contains(&cuantas), "en 1,2 s a 30 caben unas 36 imágenes, y hay {cuantas}");
        let _ = std::fs::remove_dir_all(ruta.parent().unwrap());
    }

    /// Una cámara que viene en JPEG, como casi todas las de USB a 720p: pedida en ese formato,
    /// llega descomprimida, en RGBA, a su tamaño y con sus colores.
    #[test]
    fn una_camara_que_da_jpeg_llega_descomprimida() {
        let dice = "image/jpeg, width=640, height=480, framerate=30/1";
        let fuente = Fuente::Descrita {
            fuente: "videotestsrc is-live=true pattern=green ! video/x-raw,width=640,height=480,framerate=30/1 ! jpegenc".into(),
            dice: Some(dice.into()),
        };
        let pedido = pedido_de(&caps(dice)).expect("vale");
        assert!(pedido.comprimido);
        let camara = Abierta::abrir(&fuente, Some(&pedido)).expect("encender");
        let antes = Instant::now();
        let imagen = loop {
            match camara.leer().expect("la cámara no tendría que fallar") {
                Lectura::Imagen(imagen) => break imagen,
                _ => assert!(antes.elapsed() < Duration::from_secs(20), "no llega ninguna imagen"),
            }
        };
        assert_eq!((imagen.ancho, imagen.alto, imagen.rgba.len()), (640, 480, 640 * 480 * 4));
        let centro = &imagen.rgba[(240 * 640 + 320) * 4..][..4];
        assert!(centro[1] > 150 && centro[0] < 100 && centro[2] < 100, "la imagen tendría que ser verde: {centro:?}");
        assert!(imagen.retraso_ms < 1_000);
    }

    /// Si la cámara no arranca en el formato que se le pide (dice dar uno que luego no da),
    /// se graba en el que dé, que es mejor que quedarse sin cámara.
    #[test]
    fn si_no_acepta_el_formato_elegido_se_graba_en_el_suyo() {
        let (camara, ruta) = de_mentira(
            "otro-formato",
            // Con `capsfilter` escrito: una descripción no puede acabar en un formato suelto.
            "videotestsrc is-live=true pattern=red ! capsfilter caps=video/x-raw,width=320,height=240,framerate=30/1",
            Some("video/x-raw, width=1280, height=720, framerate=30/1"),
        );
        tiene_que_grabar_a(camara, &ruta, (320, 240));
    }

    /// Una cámara que no dice qué formatos da se graba en el que dé, descomprimido por
    /// GStreamer. Esta da JPEG sin decirlo: no hay formato que pedirle.
    #[test]
    fn una_camara_que_no_dice_sus_formatos_se_graba_en_el_que_da() {
        let (camara, ruta) = de_mentira(
            "sin-formatos",
            "videotestsrc is-live=true pattern=green ! capsfilter caps=video/x-raw,width=320,height=240,framerate=30/1 ! jpegenc",
            Some("ANY"),
        );
        tiene_que_grabar_a(camara, &ruta, (320, 240));
    }

    /// Enciende, graba medio segundo largo y mira que el archivo mida eso.
    fn tiene_que_grabar_a(mut camara: Encendida, ruta: &Path, medida: (u32, u32)) {
        let estado = camara.esperar_imagen(Duration::from_secs(20));
        if estado != AlEsperar::Lista {
            // `terminar` dice en la consola por qué no ha arrancado.
            camara.grabar(reloj_desde_ya(), Arc::new(AtomicBool::new(false))).terminar();
            panic!("tendría que haber arrancado en su formato, y está {estado:?}");
        }
        let grabadora = camara.grabar(reloj_desde_ya(), Arc::new(AtomicBool::new(false)));
        std::thread::sleep(Duration::from_millis(600));
        let pista = grabadora.terminar().expect("tendría que haber grabado en su formato");
        assert_eq!((pista.ancho, pista.alto), medida);
        assert_eq!(leer(ruta).0, medida);
        let _ = std::fs::remove_dir_all(ruta.parent().unwrap());
    }

    /// Si la grabación no llega a empezar (se suelta sin `grabar`), la cámara se apaga y su
    /// archivo, que se abrió con la primera imagen, no se queda en la carpeta.
    #[test]
    fn si_la_grabacion_no_llega_a_empezar_no_queda_archivo() {
        let (mut camara, ruta) = de_mentira("sin-grabar", "videotestsrc is-live=true", None);
        assert_eq!(camara.esperar_imagen(Duration::from_secs(20)), AlEsperar::Lista);
        assert!(ruta.exists(), "el archivo se abre al encenderla");
        drop(camara);
        let antes = Instant::now();
        while ruta.exists() && antes.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!ruta.exists(), "el archivo de una grabación que no empezó se ha quedado en la carpeta");
        let _ = std::fs::remove_dir_all(ruta.parent().unwrap());
    }

    /// Una cámara que no se puede abrir no hace esperar: se sabe enseguida, y la grabación
    /// sigue sin ella.
    #[test]
    fn una_camara_que_falla_no_hace_esperar() {
        let (mut camara, ruta) = de_mentira("falla", "v4l2src device=/dev/esta-camara-no-existe", None);
        let antes = Instant::now();
        assert_eq!(camara.esperar_imagen(Duration::from_secs(20)), AlEsperar::Fallida);
        assert!(antes.elapsed() < Duration::from_secs(5), "ha esperado {:?} a una cámara que no hay", antes.elapsed());
        let grabadora = camara.grabar(reloj_desde_ya(), Arc::new(AtomicBool::new(false)));
        assert_eq!(grabadora.terminar(), None);
        assert!(!ruta.exists(), "una cámara que no ha dado imagen no deja archivo");
        let _ = std::fs::remove_dir_all(ruta.parent().unwrap());
    }
}
