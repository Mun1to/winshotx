//! La webcam, grabada aparte como una pista más de la sesión (F2, solo la grabación).
//!
//! **Va a su propio archivo y no dentro de los fotogramas.** Lo de enseñarla en el vídeo
//! (forma, recorte, tamaño, sitio) se decide al exportar, igual que el puntero y los aros:
//! quien graba no tiene que saber antes en qué esquina la quiere.
//!
//! El camino es el de Media Foundation de punta a punta, sin pasar los fotogramas por
//! nuestras manos: la cámara (`IMFSourceReader`, que ya descomprime el MJPG y convierte a
//! NV12) entrega cada imagen y el escritor (`IMFSinkWriter`) la comprime en H.264 dentro de
//! un MP4. Lo único que se toca es la marca de tiempo de cada imagen, para que la pausa de
//! la grabación también pause la cámara (`Linea`).
//!
//! **El reloj.** Todo lo de la sesión se mide con el reloj de la grabación (`Reloj`: desde el
//! cero de los fotogramas, sin las pausas). La cámara tiene el suyo, y lo que se apunta es
//! en qué instante del de la grabación llegó su primera imagen (`Pista::primer_ms`). De ahí
//! sale `CamaraGrabada::desfase_ms`, ver `desfase`.
//!
//! **Se enciende antes de grabar** (`Encendida`). Encender una cámara tarda (la de Munito,
//! unos 2 s), y la grabación espera su primera imagen antes de arrancar la pantalla, como
//! mucho `recorder::TOPE_DE_LA_CAMARA`: así la cara está viva desde el primer fotograma y
//! `desfase_ms` queda cerca de cero.
//!
//! Que no haya cámara, que la tenga otro programa o que Windows no deje usarla (Ajustes >
//! Privacidad > Cámara) no impide grabar: la grabación sigue sin cámara y se dice por qué.

#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use windows::core::{GUID, HSTRING, PWSTR};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_MULTITHREADED};

use super::anotador::Reloj;
use crate::error::{AppError, Result};

/// Cómo se llama el archivo de la cámara dentro de la carpeta de la sesión.
pub const ARCHIVO: &str = "camara.mp4";

/// Cuánto se espera a que la cámara cierre su archivo al parar. Escribir el índice de un MP4
/// largo tarda algo; más que esto es que la cámara se ha quedado colgada, y entonces la
/// grabación se entrega sin ella antes que dejar al editor esperando.
const ESPERA_AL_PARAR: Duration = Duration::from_secs(10);

/// Una cámara conectada, para elegirla en los ajustes.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Dispositivo {
    /// Con lo que Windows la reconoce aunque cambie de puerto. Es lo que se guarda en los
    /// ajustes (`camera_device`).
    pub id: String,
    /// Como la llama Windows: «Integrated Webcam», «Logitech C920».
    pub nombre: String,
}

/// Lo que deja la cámara al terminar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pista {
    /// El archivo, sin la carpeta.
    pub archivo: String,
    pub ancho: u32,
    pub alto: u32,
    /// En qué instante del reloj de la grabación se vio su primera imagen.
    pub primer_ms: u64,
}

/// Cuándo empieza la cámara respecto al primer fotograma de la pantalla, como lo pide
/// `CamaraGrabada::desfase_ms`: positivo es que la cámara arrancó más tarde.
///
/// Para el que la dibuje al exportar: el fotograma de la pantalla que se marcó en
/// `timestamp_ms` lleva encima la imagen de la cámara de
/// `timestamp_ms - frames[0].timestamp_ms - desfase_ms`, en milisegundos desde el principio
/// de `camara.mp4`. Si sale negativo, la cámara todavía no había arrancado.
pub fn desfase(primer_ms_camara: u64, primer_fotograma_ms: u64) -> i64 {
    primer_ms_camara as i64 - primer_fotograma_ms as i64
}

// --- Media Foundation ---

/// Media Foundation encendido en este hilo, y apagado al salir de él.
///
/// Cada cosa que habla con la cámara corre en un hilo suyo, con su COM y su Media
/// Foundation: así no depende de cómo dejó el hilo quien la llama (Tauri, el muestreador).
struct MediaFoundation {
    com: bool,
}

impl MediaFoundation {
    fn encender() -> Result<Self> {
        // Si el hilo ya tenía COM de otra manera no se apaga al salir: no es nuestro.
        let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }?;
        Ok(Self { com })
    }
}

impl Drop for MediaFoundation {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            if self.com {
                CoUninitialize();
            }
        }
    }
}

fn atributos(cuantos: u32) -> Result<IMFAttributes> {
    let mut atributos = None;
    unsafe { MFCreateAttributes(&mut atributos, cuantos) }?;
    atributos.ok_or_else(|| AppError::Msg("Media Foundation no ha dado atributos".into()))
}

/// Dos números de 32 bits metidos en uno, como guarda Media Foundation el tamaño y los
/// fotogramas por segundo.
fn empaquetar(alto: u32, bajo: u32) -> u64 {
    (u64::from(alto) << 32) | u64::from(bajo)
}

fn desempaquetar(valor: u64) -> (u32, u32) {
    ((valor >> 32) as u32, valor as u32)
}

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

/// Las cámaras que hay, sin encender ninguna: preguntar su nombre no enciende la luz.
pub fn listar() -> Result<Vec<Dispositivo>> {
    std::thread::spawn(|| -> Result<Vec<Dispositivo>> {
        let _mf = MediaFoundation::encender()?;
        Ok(activadores()?
            .iter()
            .map(|a| Dispositivo {
                id: texto_de(a, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK)
                    .unwrap_or_default(),
                nombre: texto_de(a, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
                    .unwrap_or_else(|| "Cámara".into()),
            })
            .collect())
    })
    .join()
    .map_err(|_| AppError::Msg("se ha caído el hilo que buscaba cámaras".into()))?
}

/// Las cámaras que ve Media Foundation, en el orden en que las da.
fn activadores() -> Result<Vec<IMFActivate>> {
    let filtro = atributos(1)?;
    unsafe {
        filtro.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )
    }?;
    let mut lista: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut cuantos = 0u32;
    unsafe { MFEnumDeviceSources(&filtro, &mut lista, &mut cuantos) }?;
    let mut salida = Vec::with_capacity(cuantos as usize);
    if !lista.is_null() {
        for i in 0..cuantos as usize {
            // La lista es nuestra: cada elemento se suelta al leerlo y la lista al final.
            if let Some(activador) = unsafe { std::ptr::read(lista.add(i)) } {
                salida.push(activador);
            }
        }
        unsafe { CoTaskMemFree(Some(lista as *const core::ffi::c_void)) };
    }
    Ok(salida)
}

fn texto_de(activador: &IMFActivate, clave: &GUID) -> Option<String> {
    let mut texto = PWSTR::null();
    let mut largo = 0u32;
    unsafe { activador.GetAllocatedString(clave, &mut texto, &mut largo) }.ok()?;
    let leido = unsafe { texto.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(texto.0 as *const core::ffi::c_void)) };
    leido.filter(|t| !t.is_empty())
}

/// Enciende la cámara elegida, o la primera si esa ya no está (se ha desenchufado, o nunca
/// se eligió ninguna).
fn abrir_fuente(dispositivo: Option<&str>) -> Result<IMFMediaSource> {
    let lista = activadores()?;
    let elegido = dispositivo
        .and_then(|id| {
            lista.iter().find(|a| {
                texto_de(a, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK).as_deref()
                    == Some(id)
            })
        })
        .or_else(|| lista.first())
        .ok_or_else(|| AppError::Msg("no hay ninguna cámara conectada".into()))?;
    Ok(unsafe { elegido.ActivateObject::<IMFMediaSource>() }?)
}

// --- Qué formato se graba ---

/// Uno de los formatos que da la cámara.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Formato {
    pub ancho: u32,
    pub alto: u32,
    pub fps: f64,
    /// MJPG y parecidos: hay que descomprimirlos antes de volver a comprimir.
    pub comprimido: bool,
}

/// Cuál de los formatos de la cámara se graba. `None` si no da ninguno.
///
/// La meta es 720p a unos 30 fotogramas: en una esquina del vídeo se ve de sobra, y es lo
/// que casi todas las cámaras dan sin ahogar el USB. Por orden:
///
/// 1. que vaya de 24 a 31 fotogramas por segundo (menos se ve a saltos, y 60 es el doble
///    de máquina para una imagen que va pequeña);
/// 2. que mida lo más cerca posible de 720 de alto, y antes por arriba que por abajo: una
///    imagen grande se puede reducir al exportar, una pequeña no se puede inventar;
/// 3. a igualdad, la que no viene comprimida, que no hay que descomprimir.
pub(crate) fn elegir(formatos: &[Formato]) -> Option<usize> {
    let puntos = |f: &Formato| {
        let fps = if (23.9..=31.0).contains(&f.fps) {
            0
        } else if f.fps > 31.0 {
            1
        } else if f.fps >= 15.0 {
            2
        } else {
            3
        };
        let alto = if f.alto >= 720 { f.alto - 720 } else { (720 - f.alto) * 2 };
        (fps, alto, f.comprimido)
    };
    formatos
        .iter()
        .enumerate()
        .min_by_key(|(_, f)| puntos(f))
        .map(|(i, _)| i)
}

fn formato_de(tipo: &IMFMediaType) -> Option<Formato> {
    let (ancho, alto) = desempaquetar(unsafe { tipo.GetUINT64(&MF_MT_FRAME_SIZE) }.ok()?);
    let (num, den) = desempaquetar(unsafe { tipo.GetUINT64(&MF_MT_FRAME_RATE) }.unwrap_or(0));
    let subtipo = unsafe { tipo.GetGUID(&MF_MT_SUBTYPE) }.ok()?;
    let sin_comprimir = [MFVideoFormat_NV12, MFVideoFormat_YUY2, MFVideoFormat_RGB32];
    Some(Formato {
        ancho,
        alto,
        fps: if den == 0 { 0.0 } else { f64::from(num) / f64::from(den) },
        comprimido: !sin_comprimir.contains(&subtipo),
    })
}

/// El lector de la cámara, puesto en el formato elegido y entregando NV12, que es lo que
/// come cualquier codificador de H.264 de Windows, el de la tarjeta y el de la CPU.
fn lector_de(fuente: &IMFMediaSource) -> Result<IMFSourceReader> {
    let ajustes = atributos(1)?;
    // Que el lector descomprima y convierta él solo (MJPG o YUY2 a NV12).
    unsafe { ajustes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1) }?;
    let lector = unsafe { MFCreateSourceReaderFromMediaSource(fuente, &ajustes) }?;

    let mut nativos = Vec::new();
    let mut formatos = Vec::new();
    for i in 0.. {
        match unsafe { lector.GetNativeMediaType(VIDEO, i) } {
            Ok(tipo) => {
                if let Some(f) = formato_de(&tipo) {
                    nativos.push(tipo);
                    formatos.push(f);
                }
            }
            Err(_) => break, // MF_E_NO_MORE_TYPES
        }
    }
    if let Some(i) = elegir(&formatos) {
        // Si la cámara no acepta el elegido, se queda con el suyo de siempre.
        if let Err(error) = unsafe { lector.SetCurrentMediaType(VIDEO, None, &nativos[i]) } {
            eprintln!("[winshotx] la cámara no acepta {:?}: {error}", formatos[i]);
        }
    }
    let salida = unsafe { MFCreateMediaType() }?;
    unsafe {
        salida.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        salida.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        lector.SetCurrentMediaType(VIDEO, None, &salida)?;
    }
    Ok(lector)
}

// --- El archivo ---

/// Cuántos bits por segundo para el H.264 de la cámara: una décima de bit por píxel y
/// fotograma, que en una cara que se mueve poco es de sobra (720p a 30 son 2,8 Mbit/s).
fn bitrate(ancho: u32, alto: u32, fps: f64) -> u32 {
    let bits = f64::from(ancho) * f64::from(alto) * fps.max(1.0) * 0.1;
    bits.clamp(1_000_000.0, 8_000_000.0) as u32
}

/// El MP4 de la cámara, escrito por Media Foundation.
pub(crate) struct Escritor {
    escritor: IMFSinkWriter,
    flujo: u32,
}

impl Escritor {
    /// Abre `ruta` para escribir en H.264 lo que llegue en `entrada` (NV12, con su tamaño y
    /// sus fotogramas por segundo puestos).
    ///
    /// Primero con el codificador de la tarjeta gráfica, que no cuesta CPU; si no lo acepta,
    /// con el de Windows por CPU, que está en todas las máquinas.
    pub(crate) fn nuevo(ruta: &Path, entrada: &IMFMediaType) -> Result<Self> {
        match Self::con(ruta, entrada, true) {
            Ok(escritor) => Ok(escritor),
            Err(error) => {
                eprintln!("[winshotx] la cámara no puede usar el codificador de la tarjeta ({error}): va por CPU");
                Self::con(ruta, entrada, false)
            }
        }
    }

    fn con(ruta: &Path, entrada: &IMFMediaType, tarjeta: bool) -> Result<Self> {
        let (ancho, alto) = desempaquetar(unsafe { entrada.GetUINT64(&MF_MT_FRAME_SIZE) }?);
        let (num, den) = desempaquetar(
            unsafe { entrada.GetUINT64(&MF_MT_FRAME_RATE) }.unwrap_or(empaquetar(30, 1)),
        );
        let (num, den) = if num == 0 || den == 0 { (30, 1) } else { (num, den) };

        let ajustes = atributos(2)?;
        unsafe {
            ajustes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, u32::from(tarjeta))?;
            ajustes.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)?;
        }
        let escritor =
            unsafe { MFCreateSinkWriterFromURL(&HSTRING::from(ruta.as_os_str()), None, &ajustes) }?;

        let salida = unsafe { MFCreateMediaType() }?;
        unsafe {
            salida.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            salida.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            salida.SetUINT32(
                &MF_MT_AVG_BITRATE,
                bitrate(ancho, alto, f64::from(num) / f64::from(den)),
            )?;
            salida.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            salida.SetUINT64(&MF_MT_FRAME_SIZE, empaquetar(ancho, alto))?;
            salida.SetUINT64(&MF_MT_FRAME_RATE, empaquetar(num, den))?;
            salida.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, empaquetar(1, 1))?;
        }
        let flujo = unsafe { escritor.AddStream(&salida) }?;
        unsafe {
            escritor.SetInputMediaType(flujo, entrada, None)?;
            escritor.BeginWriting()?;
        }
        Ok(Self { escritor, flujo })
    }

    pub(crate) fn escribir(&self, muestra: &IMFSample) -> Result<()> {
        unsafe { self.escritor.WriteSample(self.flujo, muestra) }?;
        Ok(())
    }

    /// Cierra el MP4: sin esto no tiene índice y no lo abre nadie.
    pub(crate) fn cerrar(self) -> Result<()> {
        unsafe { self.escritor.Finalize() }?;
        Ok(())
    }
}

// --- Las marcas de tiempo ---

/// Lo que dura una imagen a 30 por segundo, en las unidades de Media Foundation (cien
/// nanosegundos). Se usa hasta saber lo que de verdad da la cámara.
const INTERVALO_DE_SERIE: i64 = 333_333;

/// Pasa las marcas de tiempo de la cámara a las del archivo: empiezan en cero, y lo que dura
/// una pausa de la grabación no está, igual que no está en los fotogramas de la pantalla.
///
/// Mientras la grabación está en pausa, las imágenes de la cámara se tiran. Al volver, el
/// hueco (menos lo que dura una imagen) se descuenta de todo lo que viene detrás, así que
/// el archivo sigue sin saltos y la cámara no se queda atrás de la pantalla.
#[derive(Debug, Default)]
pub(crate) struct Linea {
    primera: Option<i64>,
    ultima_entrada: i64,
    ultima_salida: i64,
    intervalo: i64,
    en_pausa: bool,
    descontado: i64,
}

impl Linea {
    /// La marca que lleva en el archivo una imagen que la cámara marcó con `entrada`, o
    /// `None` si se tira porque la grabación está en pausa.
    pub(crate) fn sello(&mut self, entrada: i64, pausa: bool) -> Option<i64> {
        if pausa {
            self.en_pausa = true;
            return None;
        }
        let Some(primera) = self.primera else {
            self.primera = Some(entrada);
            self.ultima_entrada = entrada;
            self.intervalo = INTERVALO_DE_SERIE;
            self.en_pausa = false;
            return Some(0);
        };
        let hueco = entrada - self.ultima_entrada;
        if self.en_pausa {
            self.descontado += (hueco - self.intervalo).max(0);
            self.en_pausa = false;
        } else if hueco > 0 {
            self.intervalo = hueco;
        }
        self.ultima_entrada = entrada;
        // Siempre hacia delante: dos imágenes con la misma marca no las quiere el MP4.
        let salida = (entrada - primera - self.descontado).max(self.ultima_salida + 1);
        self.ultima_salida = salida;
        Some(salida)
    }
}

/// Cuánto tardó esa imagen en llegar desde que la cámara la sacó, en milisegundos.
///
/// Las cámaras de Windows marcan sus imágenes con el reloj del sistema
/// (`MFGetSystemTime`), y entonces la diferencia es lo que tardan en llegar (unos 30 a
/// 150 ms). Si una marca con otro reloj, la diferencia sale disparatada y se toma cero
/// antes que mover la cámara un minuto. NO PROBADO con una cámara de verdad.
fn retraso_ms(marca: i64) -> u64 {
    let ahora = unsafe { MFGetSystemTime() };
    let retraso = ahora - marca;
    if (0..=10_000_000).contains(&retraso) {
        (retraso / 10_000) as u64
    } else {
        0
    }
}

// --- La grabación ---

/// Qué imágenes de la cámara entran en el archivo y con qué marca, desde que se le manda
/// grabar.
///
/// Con la cámara encendida de antes (`Encendida`), las primeras que se leen al empezar a
/// grabar pueden haberse sacado ANTES del cero de la grabación: llegan con su retraso. Esas
/// son de antes de grabar y se tiran, así que el archivo empieza en la primera que se sacó
/// desde el cero, y `primer_ms` queda cerca de cero. Lo demás (las pausas, las marcas
/// siempre hacia delante) lo hace `Linea`.
#[derive(Debug, Default)]
pub(crate) struct Toma {
    linea: Linea,
    /// En qué instante del reloj de la grabación se sacó la primera que entró.
    primer_ms: Option<u64>,
}

impl Toma {
    /// La marca en el archivo de una imagen que la cámara marcó con `marca`, o `None` si se
    /// tira. `ahora_ms` es el reloj de la grabación al leerla, y `retraso_ms` lo que tardó en
    /// llegar desde que se sacó (la función `retraso_ms`).
    pub(crate) fn sello(&mut self, marca: i64, ahora_ms: u64, retraso_ms: u64, pausa: bool) -> Option<i64> {
        let sacada = ahora_ms.checked_sub(retraso_ms);
        if self.primer_ms.is_none() && sacada.is_none() {
            return None;
        }
        let sello = self.linea.sello(marca, pausa)?;
        if self.primer_ms.is_none() {
            self.primer_ms = sacada;
        }
        Some(sello)
    }
}

/// Lo que se le manda a la cámara encendida para que empiece a guardar lo que da.
struct Orden {
    reloj: Reloj,
    pausa: Arc<AtomicBool>,
}

/// Lo que da una lectura de la cámara.
enum Lectura {
    Imagen { muestra: IMFSample, marca: i64 },
    /// Un aviso de hueco de la cámara, sin imagen: no es un error.
    Nada,
    /// La cámara se ha acabado (se ha desenchufado).
    Fin,
}

fn leer_de(lector: &IMFSourceReader) -> Result<Lectura> {
    let mut banderas = 0u32;
    let mut marca = 0i64;
    let mut muestra: Option<IMFSample> = None;
    unsafe {
        lector.ReadSample(
            VIDEO,
            0,
            None,
            Some(&mut banderas),
            Some(&mut marca),
            Some(&mut muestra),
        )
    }?;
    if banderas & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
        return Ok(Lectura::Fin);
    }
    Ok(match muestra {
        Some(muestra) => Lectura::Imagen { muestra, marca },
        None => Lectura::Nada,
    })
}

/// Cómo está la cámara cuando la grabación deja de esperarla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlEsperar {
    /// Ya da imagen: la cara sale viva desde el primer fotograma.
    Lista,
    /// No ha llegado a encenderse (no hay, la tiene otro programa, Windows no deja usarla):
    /// se graba sin ella, y `Grabadora::terminar` dice por qué.
    Fallida,
    /// Sigue sin dar imagen: se graba igual, y entra en el vídeo cuando llegue, con su
    /// desfase, como antes de encenderla de antemano.
    Tarda,
}

/// La webcam encendida ANTES de grabar, todavía sin guardar nada.
///
/// Encender una cámara tarda (la Insta360 Link 2 Pro de Munito, unos 2 s: 1977 y 2093 ms
/// el 12 de noviembre de 2026), y si se enciende a la vez que la pantalla, la cara sale
/// quieta en su primera imagen todo ese rato. Así que va en dos pasos: `encender` la pone en
/// marcha en su hilo, con la fuente, el lector y el archivo abiertos, leyendo y tirando lo
/// que da (y avisando con la primera imagen, ver `esperar_imagen`); y `grabar`, ya con el
/// reloj de la grabación, empieza a guardar desde ahí.
///
/// Si la grabación no llega a empezar (se suelta sin `grabar`), la cámara se apaga, y con
/// ella su luz: lleva dentro su `Grabadora`, que la para al soltarse.
pub struct Encendida {
    grabadora: Grabadora,
    orden: mpsc::Sender<Orden>,
    aviso: mpsc::Receiver<()>,
    lista: bool,
}

impl Encendida {
    /// Enciende la cámara elegida (o la primera) y abre `carpeta/camara.mp4`. Vuelve
    /// enseguida.
    pub fn encender(dispositivo: Option<String>, carpeta: &Path) -> Self {
        Self::con(carpeta.join(ARCHIVO), move |ruta, parar, aviso, orden| {
            encender_y_grabar(dispositivo.as_deref(), ruta, parar, aviso, orden)
        })
    }

    /// `encender`, con lo que corre en el hilo de la cámara puesto aparte: las pruebas le
    /// ponen una cámara de mentira.
    fn con<F>(ruta: PathBuf, hilo: F) -> Self
    where
        F: FnOnce(&Path, &AtomicBool, mpsc::Sender<()>, mpsc::Receiver<Orden>) -> Result<Pista>
            + Send
            + 'static,
    {
        let parar = Arc::new(AtomicBool::new(false));
        let (enviar, resultado) = mpsc::channel();
        let (avisar, aviso) = mpsc::channel();
        let (ordenar, orden) = mpsc::channel();
        let parar_hilo = Arc::clone(&parar);
        std::thread::spawn(move || {
            let hecho = hilo(&ruta, &parar_hilo, avisar, orden);
            if hecho.is_err() {
                let _ = std::fs::remove_file(&ruta);
            }
            let _ = enviar.send(hecho);
        });
        Self {
            grabadora: Grabadora { parar, resultado },
            orden: ordenar,
            aviso,
            lista: false,
        }
    }

    /// Espera como mucho `como_mucho` a que la cámara dé su primera imagen. Vuelve en cuanto
    /// la da, o en cuanto se sabe que no va a dar ninguna (`Fallida`): una cámara que no hay
    /// o que Windows no deja usar no hace esperar a nadie.
    pub fn esperar_imagen(&mut self, como_mucho: Duration) -> AlEsperar {
        if self.lista {
            return AlEsperar::Lista;
        }
        match self.aviso.recv_timeout(como_mucho) {
            Ok(()) => {
                self.lista = true;
                AlEsperar::Lista
            }
            Err(mpsc::RecvTimeoutError::Timeout) => AlEsperar::Tarda,
            // El hilo ha acabado sin dar imagen: ha fallado al encenderla.
            Err(mpsc::RecvTimeoutError::Disconnected) => AlEsperar::Fallida,
        }
    }

    /// Empieza a guardar, con el reloj y la pausa de la grabación: desde aquí, lo que da la
    /// cámara va a `camara.mp4`.
    pub fn grabar(self, reloj: Reloj, pausa: Arc<AtomicBool>) -> Grabadora {
        // Si el hilo ya acabó (la cámara falló), la orden no llega a nadie y `terminar` dice
        // por qué.
        let _ = self.orden.send(Orden { reloj, pausa });
        self.grabadora
    }
}

/// La cámara grabando en su hilo, hasta que se le diga que pare.
pub struct Grabadora {
    parar: Arc<AtomicBool>,
    resultado: mpsc::Receiver<Result<Pista>>,
}

impl Grabadora {
    /// Le dice a la cámara que pare, sin esperarla. Así va cerrando su archivo mientras la
    /// grabación cierra el suyo.
    pub fn avisar(&self) {
        self.parar.store(true, Ordering::Relaxed);
    }

    /// Para la cámara y devuelve lo grabado. `None` si no hubo cámara, y entonces dice por
    /// qué en la consola.
    pub fn terminar(self) -> Option<Pista> {
        self.avisar();
        match self.resultado.recv_timeout(ESPERA_AL_PARAR) {
            Ok(Ok(pista)) => Some(pista),
            Ok(Err(error)) => {
                eprintln!("[winshotx] sin cámara: {error}");
                None
            }
            Err(_) => {
                eprintln!("[winshotx] la cámara no ha cerrado su archivo a tiempo: se entrega sin ella");
                None
            }
        }
    }
}

/// Si la grabación se cae por el camino y nadie llega a llamar a `terminar`, la cámara se
/// para igual: si no, se quedaría encendida, con su luz, hasta cerrar winshotx.
impl Drop for Grabadora {
    fn drop(&mut self) {
        self.avisar();
    }
}

fn encender_y_grabar(
    dispositivo: Option<&str>,
    ruta: &Path,
    parar: &AtomicBool,
    aviso: mpsc::Sender<()>,
    orden: mpsc::Receiver<Orden>,
) -> Result<Pista> {
    let encendida = std::time::Instant::now();
    let _mf = MediaFoundation::encender()?;
    let fuente = abrir_fuente(dispositivo)?;
    let hecho = grabar_de(&fuente, ruta, parar, aviso, orden, encendida);
    // Apagar la fuente es lo que apaga la luz de la cámara.
    let _ = unsafe { fuente.Shutdown() };
    hecho
}

fn grabar_de(
    fuente: &IMFMediaSource,
    ruta: &Path,
    parar: &AtomicBool,
    aviso: mpsc::Sender<()>,
    orden: mpsc::Receiver<Orden>,
    encendida: std::time::Instant,
) -> Result<Pista> {
    let abierta = encendida.elapsed();
    let lector = lector_de(fuente)?;
    let tipo = unsafe { lector.GetCurrentMediaType(VIDEO) }?;
    let tamano = desempaquetar(unsafe { tipo.GetUINT64(&MF_MT_FRAME_SIZE) }?);
    let con_lector = encendida.elapsed();
    // El archivo se abre ya, antes de grabar: poner en marcha el codificador tarda (de 0,3 a
    // 0,6 s en la máquina de Munito, 12-nov-2026), y abierto al empezar a grabar, la cara
    // saldría quieta ese rato. Va antes de la primera lectura, que es la que pone la cámara
    // a dar imágenes, así que su tiempo se suma al de la cámara: para solaparlos habría que
    // abrirlo en otro hilo, y los objetos de Media Foundation no se pueden pasar entre hilos.
    let escritor = Escritor::nuevo(ruta, &tipo)?;
    // Para saber en qué se va la espera del arranque (lo que tarda en dar su primera imagen
    // lo dice `recorder::start`).
    eprintln!(
        "[winshotx] la cámara: abierta a los {} ms, con el lector a los {} ms y su archivo a los {} ms",
        abierta.as_millis(),
        con_lector.as_millis(),
        encendida.elapsed().as_millis()
    );
    vivir(|| leer_de(&lector), escritor, tamano, parar, aviso, orden)
}

/// Lo que hace la cámara mientras está encendida: hasta que le llega la orden de grabar,
/// tira lo que da (y con la primera imagen avisa de que ya se puede empezar); desde la orden,
/// lo pasa por `Toma` al archivo. Sin la orden y sin nadie que la vaya a mandar (la
/// grabación no llegó a empezar), se acaba, y quien la llamó apaga la cámara.
fn vivir(
    mut leer: impl FnMut() -> Result<Lectura>,
    escritor: Escritor,
    (ancho, alto): (u32, u32),
    parar: &AtomicBool,
    aviso: mpsc::Sender<()>,
    orden: mpsc::Receiver<Orden>,
) -> Result<Pista> {
    let mut aviso = Some(aviso);
    let mut grabando: Option<Orden> = None;
    let mut toma = Toma::default();

    while !parar.load(Ordering::Relaxed) {
        let (muestra, marca) = match leer()? {
            Lectura::Imagen { muestra, marca } => (muestra, marca),
            Lectura::Nada => continue,
            Lectura::Fin => break,
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
        let Some(sello) = toma.sello(
            marca,
            o.reloj.ms(),
            retraso_ms(marca),
            o.pausa.load(Ordering::Relaxed),
        ) else {
            continue;
        };
        unsafe { muestra.SetSampleTime(sello) }?;
        escritor.escribir(&muestra)?;
    }

    let Some(primer_ms) = toma.primer_ms else {
        // Se paró antes de que la cámara diera imagen (encenderla tarda), o la grabación no
        // llegó a empezar: no hay archivo que cerrar, y uno sin imágenes no lo abre nadie.
        drop(escritor);
        let por_que = if grabando.is_some() {
            "la cámara no ha llegado a dar imagen antes de parar"
        } else {
            "la grabación no ha llegado a empezar"
        };
        return Err(AppError::Msg(por_que.into()));
    };
    escritor.cerrar()?;
    Ok(Pista {
        archivo: ARCHIVO.into(),
        ancho,
        alto,
        primer_ms,
    })
}

/// La orden de los ajustes: qué cámaras hay, para elegir una.
#[tauri::command]
pub async fn list_cameras() -> Result<Vec<Dispositivo>> {
    listar()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formato(ancho: u32, alto: u32, fps: f64, comprimido: bool) -> Formato {
        Formato {
            ancho,
            alto,
            fps,
            comprimido,
        }
    }

    /// Lo que da una webcam corriente por USB 2: en YUY2 solo llega a 720p a 10 fotogramas,
    /// y en MJPG a 30. Se queda con el MJPG a 720p, que es lo que se ve bien.
    #[test]
    fn se_queda_con_720p_a_30_aunque_venga_comprimido() {
        let formatos = [
            formato(640, 480, 30.0, false),
            formato(1280, 720, 10.0, false),
            formato(1280, 720, 30.0, true),
            formato(1920, 1080, 30.0, true),
            formato(320, 240, 30.0, false),
        ];
        assert_eq!(elegir(&formatos), Some(2));
    }

    #[test]
    fn a_igualdad_prefiere_el_que_no_hay_que_descomprimir() {
        let formatos = [formato(1280, 720, 30.0, true), formato(1280, 720, 30.0, false)];
        assert_eq!(elegir(&formatos), Some(1));
    }

    #[test]
    fn sin_720p_prefiere_pasarse_que_quedarse_corto() {
        // 1080p se reduce al exportar; 480p no se puede agrandar sin verse borroso.
        let formatos = [formato(640, 480, 30.0, false), formato(1920, 1080, 30.0, true)];
        assert_eq!(elegir(&formatos), Some(1));
    }

    #[test]
    fn antes_30_fotogramas_que_60() {
        let formatos = [formato(1280, 720, 60.0, true), formato(1280, 720, 30.0, true)];
        assert_eq!(elegir(&formatos), Some(1));
    }

    #[test]
    fn sin_formatos_no_elige_nada() {
        assert_eq!(elegir(&[]), None);
    }

    /// 30000/1001 son los 29,97 de las cámaras que siguen la norma americana: valen como 30.
    #[test]
    fn veintinueve_con_noventa_y_siete_cuenta_como_treinta() {
        let formatos = [formato(1280, 720, 15.0, false), formato(1280, 720, 30_000.0 / 1001.0, true)];
        assert_eq!(elegir(&formatos), Some(1));
    }

    #[test]
    fn el_desfase_es_positivo_si_la_camara_arranca_tarde() {
        assert_eq!(desfase(850, 40), 810);
        assert_eq!(desfase(10, 40), -30);
    }

    #[test]
    fn el_tamano_se_empaqueta_como_lo_guarda_media_foundation() {
        assert_eq!(empaquetar(1280, 720), (1280u64 << 32) | 720);
        assert_eq!(desempaquetar(empaquetar(1920, 1080)), (1920, 1080));
    }

    const FOTO: i64 = 333_333;

    #[test]
    fn el_archivo_empieza_en_cero() {
        let mut linea = Linea::default();
        assert_eq!(linea.sello(77_000_000, false), Some(0));
        assert_eq!(linea.sello(77_000_000 + FOTO, false), Some(FOTO));
        assert_eq!(linea.sello(77_000_000 + 2 * FOTO, false), Some(2 * FOTO));
    }

    /// Cinco segundos de pausa en la grabación no pueden ser cinco segundos de cámara: la
    /// pantalla no los tiene, y desde ahí la cara iría por detrás de lo que se ve.
    #[test]
    fn la_pausa_no_deja_hueco_en_el_archivo() {
        let mut linea = Linea::default();
        let mut entrada = 0;
        for _ in 0..30 {
            linea.sello(entrada, false);
            entrada += FOTO;
        }
        let antes = entrada - FOTO;
        // Cinco segundos en pausa: las imágenes llegan y se tiran.
        for _ in 0..150 {
            assert_eq!(linea.sello(entrada, true), None);
            entrada += FOTO;
        }
        let despues = linea.sello(entrada, false).unwrap();
        // Al volver, la imagen va justo detrás de la última de antes de la pausa.
        assert_eq!(despues, antes + FOTO, "la cámara se ha quedado atrás");
        assert_eq!(linea.sello(entrada + FOTO, false), Some(despues + FOTO));
    }

    #[test]
    fn una_pausa_antes_de_la_primera_imagen_no_cuenta() {
        let mut linea = Linea::default();
        assert_eq!(linea.sello(0, true), None);
        assert_eq!(linea.sello(FOTO, true), None);
        assert_eq!(linea.sello(2 * FOTO, false), Some(0));
    }

    #[test]
    fn las_marcas_siempre_van_hacia_delante() {
        // Una cámara que repite una marca (pasa con algunas al cambiar la luz).
        let mut linea = Linea::default();
        linea.sello(1_000, false);
        let a = linea.sello(1_000 + FOTO, false).unwrap();
        let b = linea.sello(1_000 + FOTO, false).unwrap();
        assert!(b > a, "{b} no va detrás de {a}");
    }

    /// Preguntar qué cámaras hay no revienta, tenga la máquina las que tenga, y no enciende
    /// ninguna: solo lee sus nombres.
    #[test]
    fn listar_las_camaras_no_revienta() {
        let camaras = listar().expect("Media Foundation tendría que contestar");
        println!("cámaras: {camaras:?}");
        assert!(camaras.iter().all(|c| !c.nombre.is_empty()));
    }

    /// NV12 de un color que cambia con el tiempo: lo que daría una cámara, sin cámara.
    fn muestra_nv12(ancho: u32, alto: u32, n: u32, sello: i64) -> IMFSample {
        let tam = (ancho * alto * 3 / 2) as usize;
        let buffer = unsafe { MFCreateMemoryBuffer(tam as u32) }.unwrap();
        unsafe {
            let mut datos = std::ptr::null_mut();
            buffer.Lock(&mut datos, None, None).unwrap();
            let bytes = std::slice::from_raw_parts_mut(datos, tam);
            let luz = ((ancho * alto) as usize).min(tam);
            bytes[..luz].fill((n * 4 % 220 + 16) as u8);
            bytes[luz..].fill(128);
            buffer.Unlock().unwrap();
            buffer.SetCurrentLength(tam as u32).unwrap();
        }
        let muestra = unsafe { MFCreateSample() }.unwrap();
        unsafe {
            muestra.AddBuffer(&buffer).unwrap();
            muestra.SetSampleTime(sello).unwrap();
            muestra.SetSampleDuration(FOTO).unwrap();
        }
        muestra
    }

    fn entrada_nv12(ancho: u32, alto: u32) -> IMFMediaType {
        let tipo = unsafe { MFCreateMediaType() }.unwrap();
        unsafe {
            tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).unwrap();
            tipo.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12).unwrap();
            tipo.SetUINT64(&MF_MT_FRAME_SIZE, empaquetar(ancho, alto)).unwrap();
            tipo.SetUINT64(&MF_MT_FRAME_RATE, empaquetar(30, 1)).unwrap();
            tipo.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).unwrap();
            tipo.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, empaquetar(1, 1)).unwrap();
        }
        tipo
    }

    /// Lo que ffprobe dice del vídeo: ancho, alto, códec y duración en segundos.
    fn medir(ruta: &Path) -> Option<(u32, u32, String, f64)> {
        let salida = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,codec_name:format=duration",
                "-of",
                "default=noprint_wrappers=1",
            ])
            .arg(ruta)
            .output()
            .ok()?;
        let texto = String::from_utf8_lossy(&salida.stdout).to_string();
        let campo = |nombre: &str| {
            texto
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{nombre}=")))
                .map(str::to_string)
        };
        Some((
            campo("width")?.parse().ok()?,
            campo("height")?.parse().ok()?,
            campo("codec_name")?,
            campo("duration")?.parse().ok()?,
        ))
    }

    /// **El camino del archivo de la cámara, sin cámara.** Lo que entrega la cámara
    /// (NV12 con sus marcas) pasa por la misma `Linea` y el mismo `Escritor` que en una
    /// grabación, con una pausa de dos segundos en medio, y el MP4 se abre con ffprobe: dura
    /// lo que se grabó SIN la pausa, mide lo que medía la imagen y va en H.264.
    #[test]
    fn el_archivo_de_la_camara_sale_entero_y_sin_la_pausa() {
        let carpeta = std::env::temp_dir().join(format!(
            "winshotx-camara-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&carpeta).unwrap();
        let ruta = carpeta.join(ARCHIVO);
        let (ancho, alto) = (640, 480);

        let hilo = std::thread::spawn({
            let ruta = ruta.clone();
            move || {
                let _mf = MediaFoundation::encender().unwrap();
                let escritor = Escritor::nuevo(&ruta, &entrada_nv12(ancho, alto)).unwrap();
                let mut linea = Linea::default();
                // Marcas como las de una cámara: grandes y a su ritmo. Dos segundos, dos
                // de pausa y otro más: el archivo tiene que durar tres.
                let base = 123_456_789_000i64;
                for n in 0..150u32 {
                    let marca = base + i64::from(n) * FOTO;
                    let pausa = (60..120).contains(&n);
                    if let Some(sello) = linea.sello(marca, pausa) {
                        escritor.escribir(&muestra_nv12(ancho, alto, n, sello)).unwrap();
                    }
                }
                escritor.cerrar().unwrap();
            }
        });
        hilo.join().expect("el hilo de la cámara se ha caído");

        let bytes = std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0);
        assert!(bytes > 1_000, "el MP4 de la cámara ha salido vacío: {bytes} bytes");
        let leido = medir(&ruta);
        let _ = std::fs::remove_dir_all(&carpeta);
        let Some((w, h, codec, duracion)) = leido else {
            eprintln!("sin ffprobe en el PATH no se puede abrir el MP4: solo se ha mirado que existe");
            return;
        };
        println!("camara.mp4: {w}x{h} {codec}, {duracion:.3} s, {bytes} bytes");
        assert_eq!((w, h), (ancho, alto));
        assert_eq!(codec, "h264");
        assert!(
            (duracion - 3.0).abs() < 0.1,
            "tendría que durar 3 s (5 grabados menos 2 de pausa) y dura {duracion}"
        );
    }

    /// Las imágenes de `camara.mp4`, en gris y a 64x64, para compararlas entre sí.
    fn grises(ruta: &Path) -> Vec<Vec<u8>> {
        let salida = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(ruta)
            .args(["-vf", "scale=64:64,format=gray", "-f", "rawvideo", "-"])
            .output()
            .expect("hace falta ffmpeg en el PATH");
        salida.stdout.chunks(64 * 64).map(<[u8]>::to_vec).collect()
    }

    /// Las imágenes que son copia exacta de una de las ocho anteriores sin serlo de la de
    /// justo antes: lo que deja un escritor que lee una caja que la cámara ya ha vuelto a
    /// llenar. Una cámara de verdad no da dos imágenes iguales (su ruido no se repite), así
    /// que una sola ya es el fallo. Devuelve (imagen, de cuál es copia).
    fn copias(imagenes: &[Vec<u8>]) -> Vec<(usize, usize)> {
        let mut halladas = Vec::new();
        for i in 2..imagenes.len() {
            if imagenes[i] == imagenes[i - 1] {
                continue;
            }
            if let Some(k) = (2..=8.min(i)).find(|&k| imagenes[i] == imagenes[i - k]) {
                halladas.push((i, i - k));
            }
        }
        halladas
    }

    #[test]
    fn una_copia_de_hace_cuatro_imagenes_se_ve() {
        let a = |n: u8| vec![n; 64 * 64];
        assert!(copias(&[a(1), a(2), a(3), a(4), a(5)]).is_empty());
        assert_eq!(copias(&[a(1), a(2), a(3), a(4), a(1), a(2)]), vec![(4, 0), (5, 1)]);
        // Repetir la de justo antes no cuenta: eso lo hace a veces una cámara al arrancar.
        assert!(copias(&[a(1), a(1), a(2), a(2)]).is_empty());
    }

    // --- S12 (cámara antes): encendida antes de grabar ---

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let carpeta = std::env::temp_dir().join(format!("winshotx-{nombre}-{}", std::process::id()));
        std::fs::create_dir_all(&carpeta).unwrap();
        carpeta
    }

    fn reloj_desde_ya() -> Reloj {
        Reloj {
            start: std::time::Instant::now(),
            paused_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Lo que se sacó antes del cero de la grabación (la cámara ya estaba encendida, y la
    /// imagen llega con su retraso) no entra en el archivo: la primera que entra es la
    /// primera sacada desde el cero, y de ella sale `primer_ms`.
    #[test]
    fn con_la_camara_encendida_de_antes_no_entra_lo_sacado_antes_de_grabar() {
        let mut toma = Toma::default();
        // Leída a los 10 ms de empezar, pero sacada 40 ms antes: es de antes de grabar.
        assert_eq!(toma.sello(5_000_000, 10, 40, false), None);
        assert_eq!(toma.primer_ms, None);
        // Leída a los 43 y sacada a los 3: la primera del archivo.
        assert_eq!(toma.sello(5_000_000 + FOTO, 43, 40, false), Some(0));
        assert_eq!(toma.primer_ms, Some(3));
        // Y desde ahí, como siempre.
        assert_eq!(toma.sello(5_000_000 + 2 * FOTO, 76, 40, false), Some(FOTO));
        assert_eq!(toma.primer_ms, Some(3));
    }

    #[test]
    fn una_pausa_antes_de_la_primera_imagen_tampoco_cuenta_en_la_toma() {
        let mut toma = Toma::default();
        assert_eq!(toma.sello(0, 50, 30, true), None);
        assert_eq!(toma.primer_ms, None);
        assert_eq!(toma.sello(FOTO, 80, 30, false), Some(0));
        assert_eq!(toma.primer_ms, Some(50));
    }

    /// Una cámara de mentira para `Encendida::con`: sin imagen hasta `tarda` después de
    /// encenderla en `desde` (lo que tarda en arrancar una de verdad), y luego una imagen NV12
    /// cada 30 ms, marcada con el reloj de Media Foundation como las de verdad. Va por el mismo
    /// `vivir` y el mismo `Escritor` que una cámara, y no enciende ninguna.
    fn camara_de_mentira(
        desde: std::time::Instant,
        tarda: Duration,
    ) -> impl FnOnce(&Path, &AtomicBool, mpsc::Sender<()>, mpsc::Receiver<Orden>) -> Result<Pista> + Send + 'static
    {
        move |ruta, parar, aviso, orden| {
            let _mf = MediaFoundation::encender()?;
            let (ancho, alto) = (320, 240);
            let escritor = Escritor::nuevo(ruta, &entrada_nv12(ancho, alto))?;
            let mut n = 0u32;
            let leer = || {
                std::thread::sleep(Duration::from_millis(30));
                if desde.elapsed() < tarda {
                    return Ok(Lectura::Nada);
                }
                n += 1;
                let marca = unsafe { MFGetSystemTime() };
                Ok(Lectura::Imagen {
                    muestra: muestra_nv12(ancho, alto, n, 0),
                    marca,
                })
            };
            vivir(leer, escritor, (ancho, alto), parar, aviso, orden)
        }
    }

    /// **Encendida antes, la cara está viva desde el cero.** Una cámara que tarda 300 ms en
    /// dar imagen: `esperar_imagen` vuelve en cuanto la da (no al tope), lo que da mientras no
    /// se graba se tira, y al grabar el archivo empieza en el cero de la grabación
    /// (`primer_ms` cerca de 0) y dura lo grabado, no lo que estuvo encendida.
    #[test]
    fn encendida_antes_la_cara_esta_desde_el_cero() {
        let carpeta = carpeta_de_prueba("camara-encendida");
        let ruta = carpeta.join(ARCHIVO);
        let antes = std::time::Instant::now();
        let mut camara = Encendida::con(ruta.clone(), camara_de_mentira(antes, Duration::from_millis(300)));
        assert_eq!(camara.esperar_imagen(Duration::from_secs(3)), AlEsperar::Lista);
        let tardo = antes.elapsed();
        assert!(
            tardo >= Duration::from_millis(300) && tardo < Duration::from_millis(2_500),
            "tendría que volver en cuanto hay imagen, y ha vuelto a los {tardo:?}"
        );
        // Encendida y sin grabar: lo que da se tira.
        std::thread::sleep(Duration::from_millis(700));
        let grabadora = camara.grabar(reloj_desde_ya(), Arc::new(AtomicBool::new(false)));
        std::thread::sleep(Duration::from_secs(1));
        let pista = grabadora.terminar().expect("tendría que haber grabado");
        let leido = medir(&ruta);
        let _ = std::fs::remove_dir_all(&carpeta);
        println!("primera imagen a los {tardo:?}; primer_ms = {}; archivo: {leido:?}", pista.primer_ms);
        assert!(
            pista.primer_ms < 80,
            "la primera imagen tendría que entrar al empezar, y entra a los {} ms",
            pista.primer_ms
        );
        let Some((_, _, _, duracion)) = leido else {
            eprintln!("sin ffprobe en el PATH no se puede medir el MP4");
            return;
        };
        assert!(
            (0.9..1.25).contains(&duracion),
            "tendría que durar lo grabado (1 s), sin los 0,7 s de antes, y dura {duracion} s"
        );
    }

    /// Si la grabación no llega a empezar (se suelta sin `grabar`), la cámara se apaga y su
    /// archivo, que se abrió al encenderla, no se queda en la carpeta.
    #[test]
    fn si_la_grabacion_no_llega_a_empezar_la_camara_se_apaga() {
        let carpeta = carpeta_de_prueba("camara-sin-grabar");
        let ruta = carpeta.join(ARCHIVO);
        let apagada = Arc::new(AtomicBool::new(false));
        let mentira = camara_de_mentira(std::time::Instant::now(), Duration::ZERO);
        let hilo = {
            let apagada = Arc::clone(&apagada);
            move |r: &Path, p: &AtomicBool, a: mpsc::Sender<()>, o: mpsc::Receiver<Orden>| {
                let hecho = mentira(r, p, a, o);
                apagada.store(true, Ordering::SeqCst);
                hecho
            }
        };
        let mut camara = Encendida::con(ruta.clone(), hilo);
        assert_eq!(camara.esperar_imagen(Duration::from_secs(3)), AlEsperar::Lista);
        assert!(ruta.exists(), "el archivo se abre al encenderla");
        drop(camara);
        let antes = std::time::Instant::now();
        while (ruta.exists() || !apagada.load(Ordering::SeqCst)) && antes.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let quedo = ruta.exists();
        let _ = std::fs::remove_dir_all(&carpeta);
        assert!(apagada.load(Ordering::SeqCst), "soltada sin grabar, la cámara sigue encendida");
        assert!(!quedo, "el archivo de una grabación que no empezó se ha quedado en la carpeta");
    }

    /// Una cámara que no hay, o que Windows no deja usar, no hace esperar: se sabe enseguida,
    /// y la grabación sigue sin ella.
    #[test]
    fn una_camara_que_falla_no_hace_esperar() {
        let carpeta = carpeta_de_prueba("camara-que-falla");
        let mut camara = Encendida::con(
            carpeta.join(ARCHIVO),
            |_: &Path, _: &AtomicBool, _: mpsc::Sender<()>, _: mpsc::Receiver<Orden>| {
                Err(AppError::Msg("no hay ninguna cámara conectada".into()))
            },
        );
        let antes = std::time::Instant::now();
        assert_eq!(camara.esperar_imagen(Duration::from_secs(3)), AlEsperar::Fallida);
        let esperado = antes.elapsed();
        let grabadora = camara.grabar(reloj_desde_ya(), Arc::new(AtomicBool::new(false)));
        assert_eq!(grabadora.terminar(), None);
        let _ = std::fs::remove_dir_all(&carpeta);
        assert!(esperado < Duration::from_secs(1), "ha esperado {esperado:?} a una cámara que no hay");
    }

    /// Una cámara más lenta que la espera: se deja de esperar al tope, se graba, y la cara
    /// entra cuando llega, con su desfase, como antes de encenderla de antemano.
    #[test]
    fn una_camara_que_tarda_mas_que_la_espera_entra_cuando_llega() {
        let carpeta = carpeta_de_prueba("camara-que-tarda");
        let ruta = carpeta.join(ARCHIVO);
        let antes = std::time::Instant::now();
        let mut camara = Encendida::con(ruta.clone(), camara_de_mentira(antes, Duration::from_millis(600)));
        assert_eq!(camara.esperar_imagen(Duration::from_millis(150)), AlEsperar::Tarda);
        let esperado = antes.elapsed();
        let grabadora = camara.grabar(reloj_desde_ya(), Arc::new(AtomicBool::new(false)));
        std::thread::sleep(Duration::from_millis(1500));
        let pista = grabadora.terminar().expect("llegó tarde, pero llegó");
        let _ = std::fs::remove_dir_all(&carpeta);
        assert!(esperado < Duration::from_millis(500), "ha esperado {esperado:?} y no 150 ms");
        // Da su primera imagen a los 600 ms de encenderla, unos 450 después de empezar a
        // grabar (más, si abrir el codificador tarda más que eso).
        assert!(
            (400..1_500).contains(&pista.primer_ms),
            "llega unos 450 ms después de empezar, y su primer_ms es {}",
            pista.primer_ms
        );
    }

    /// **Con la cámara de verdad** (la de `WINSHOTX_CAMARA`, o la primera), por el mismo camino
    /// que una grabación: se enciende antes, se espera su primera imagen como mucho
    /// `TOPE_DE_LA_CAMARA`, y se graban cinco segundos.
    ///
    /// Mira dos cosas. Que, encendida antes, la primera imagen llega ANTES de empezar a grabar
    /// y entra en el cero (`primer_ms` cerca de 0: la cara viva desde el primer fotograma, y
    /// no quieta los ~2 s que tarda la Insta360 de Munito). Y que ninguna imagen del archivo
    /// sea copia de otra de antes: en la 0.4.0 los primeros dos segundos repetían en bucle
    /// cuatro imágenes, porque el escritor guardaba la muestra que le daba la cámara sin
    /// copiarla y la cámara volvía a llenar esa misma caja antes de que el codificador la
    /// leyera. Enciende la cámara: solo con quien está delante avisado.
    #[test]
    #[ignore]
    fn la_camara_de_verdad_no_repite_imagenes() {
        let carpeta = carpeta_de_prueba("camara-real");
        let dispositivo = std::env::var("WINSHOTX_CAMARA").ok();
        let antes = std::time::Instant::now();
        let mut camara = Encendida::encender(dispositivo, &carpeta);
        let estado = camara.esperar_imagen(crate::recorder::TOPE_DE_LA_CAMARA);
        let tardo = antes.elapsed();
        let grabadora = camara.grabar(reloj_desde_ya(), Arc::new(AtomicBool::new(false)));
        std::thread::sleep(Duration::from_secs(5));
        let pista = grabadora.terminar().expect("la cámara no ha grabado nada");
        let imagenes = grises(&carpeta.join(&pista.archivo));
        let halladas = copias(&imagenes);
        println!(
            "encendida antes: {estado:?} a los {} ms; en la grabación entra a los {} ms (primer_ms)",
            tardo.as_millis(),
            pista.primer_ms
        );
        println!(
            "{} imágenes de {}x{}; copias de una anterior: {}",
            imagenes.len(),
            pista.ancho,
            pista.alto,
            halladas.len()
        );
        for (i, de) in halladas.iter().take(12) {
            println!("  la {i} es la {de}");
        }
        let _ = std::fs::remove_dir_all(&carpeta);
        assert_eq!(
            estado,
            AlEsperar::Lista,
            "la cámara no ha dado imagen en {} ms: tarda más que el tope",
            tardo.as_millis()
        );
        assert!(
            pista.primer_ms < 150,
            "encendida antes, la cara tendría que estar desde el cero, y entra a los {} ms",
            pista.primer_ms
        );
        assert!(imagenes.len() > 60, "cinco segundos de cámara son más de 60 imágenes");
        assert!(halladas.is_empty(), "{} imágenes son copia de otra de antes", halladas.len());
    }
}
