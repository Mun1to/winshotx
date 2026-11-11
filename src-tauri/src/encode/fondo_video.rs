//! El fondo animado del estudio (B5): un vídeo o un GIF del usuario detrás de la grabación.
//!
//! **Sin tope de tamaño, y nunca entero en memoria.** Todo se lee de fotograma en fotograma:
//! el MP4 (y lo que sepa abrir Windows: MOV, MKV, WMV...) con el lector de Media Foundation,
//! y el GIF con el crate `gif`, que va componiendo cada fotograma sobre el anterior.
//!
//! Va en dos tiempos:
//!
//! 1. **Importar** (`importar`) convierte lo que elija el usuario en un MP4 nuestro: como mucho
//!    1920x1080 y 30 fotogramas por segundo. Enseña su progreso, se puede cancelar, y
//!    cancelar a medias no deja nada en el disco (se escribe en un `.parcial` que solo se
//!    renombra al acabar). Así exportar siempre lee lo mismo, y un 4K de dos gigas se queda
//!    en unas decenas de megas.
//! 2. **Exportar** (`Animado`) lee ese MP4 en otro hilo, por delante, y entrega el fondo de
//!    cada instante ya escalado al lienzo y con la sombra puesta: el hilo que viste los
//!    fotogramas solo lo copia, igual que con un fondo quieto. Si el vídeo es más corto que
//!    la grabación, vuelve a empezar.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use image::{Rgba, RgbaImage};

use crate::error::{AppError, Result};

/// El lado más largo de un fondo importado. Más que esto no se ve: el fondo asoma por el
/// aire de alrededor, y un 4K solo haría más lento cada fotograma.
pub const ANCHO_MAXIMO: u32 = 1920;
pub const ALTO_MAXIMO: u32 = 1080;
/// Los fotogramas por segundo de un fondo importado. Un fondo a 60 no se distingue de uno a
/// 30 detrás de una grabación, y cuesta el doble de decodificar al exportar.
pub const FPS: u32 = 30;

/// Lo que devuelve `importar` cuando se cancela, para que quien llama no lo enseñe como un
/// fallo.
pub const CANCELADO: &str = "importación cancelada";

/// Un fotograma leído: cuándo empieza, cuánto dura y la imagen.
pub struct Cuadro {
    pub desde_ms: u64,
    pub dura_ms: u64,
    pub imagen: RgbaImage,
}

/// Algo que entrega fotogramas en orden, de uno en uno.
pub trait Lector {
    /// El siguiente, o `None` al acabar.
    fn siguiente(&mut self) -> Result<Option<Cuadro>>;
    /// Cuánto se ha leído, de 0 a 1, para la barra de progreso de importar.
    fn leido(&self) -> f32;
}

/// Abre un vídeo o un GIF. El GIF por su extensión; lo demás se lo pregunta a Windows.
///
/// Los `return` de los bloques `cfg` se quedan, como en `platform`: con un solo cuerpo por
/// sistema, quitarlos haría que una línea añadida detrás cambiara lo que se devuelve.
#[allow(clippy::needless_return)]
pub fn abrir(ruta: &Path) -> Result<Box<dyn Lector>> {
    let es_gif = ruta
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("gif"));
    if es_gif {
        return Ok(Box::new(LectorGif::abrir(ruta)?));
    }
    #[cfg(windows)]
    {
        return Ok(Box::new(mf::LectorMf::abrir(ruta)?));
    }
    #[cfg(not(windows))]
    {
        return Err(AppError::Unsupported);
    }
}

// --- El GIF ---

/// Un `Read` que cuenta lo que lleva leído: el GIF no sabe cuántos fotogramas tiene hasta
/// el final, así que el progreso de importarlo va por bytes.
struct Contado<R> {
    dentro: R,
    leido: Arc<std::sync::atomic::AtomicU64>,
}

impl<R: std::io::Read> std::io::Read for Contado<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.dentro.read(buf)?;
        self.leido.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// Lee un GIF de fotograma en fotograma, cada uno ya compuesto sobre lo que había debajo.
///
/// Un GIF guarda casi siempre solo lo que cambia (un trozo, no la imagen entera), y cada
/// trozo dice qué hacer con él al pasar al siguiente: dejarlo, borrarlo o volver a lo de
/// antes. Aquí se lleva ese lienzo, y solo el lienzo: ni un fotograma de más en memoria.
pub struct LectorGif {
    decoder: gif::Decoder<std::io::BufReader<Contado<std::fs::File>>>,
    lienzo: RgbaImage,
    /// Lo que había antes del último trozo, si ese trozo pide volver a ello al acabar.
    antes: Option<RgbaImage>,
    /// Lo que hay que hacer con el último trozo antes de pintar el siguiente.
    pendiente: Option<(gif::DisposalMethod, u32, u32, u32, u32)>,
    reloj_ms: u64,
    leido: Arc<std::sync::atomic::AtomicU64>,
    total: u64,
}

impl LectorGif {
    pub fn abrir(ruta: &Path) -> Result<Self> {
        let archivo = std::fs::File::open(ruta)?;
        let total = archivo.metadata().map(|m| m.len()).unwrap_or(0).max(1);
        let leido = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let mut opciones = gif::DecodeOptions::new();
        opciones.set_color_output(gif::ColorOutput::RGBA);
        let lector = std::io::BufReader::new(Contado { dentro: archivo, leido: leido.clone() });
        let decoder = opciones
            .read_info(lector)
            .map_err(|e| AppError::Msg(format!("ese GIF no se puede leer: {e}")))?;
        let (ancho, alto) = (u32::from(decoder.width()).max(1), u32::from(decoder.height()).max(1));
        Ok(Self {
            decoder,
            lienzo: RgbaImage::new(ancho, alto),
            antes: None,
            pendiente: None,
            reloj_ms: 0,
            leido,
            total,
        })
    }
}

impl Lector for LectorGif {
    fn siguiente(&mut self) -> Result<Option<Cuadro>> {
        let Some(trozo) = self
            .decoder
            .read_next_frame()
            .map_err(|e| AppError::Msg(format!("ese GIF se corta a la mitad: {e}")))?
        else {
            return Ok(None);
        };
        let (ancho, alto) = self.lienzo.dimensions();
        // Primero lo que pedía el trozo anterior.
        match self.pendiente.take() {
            Some((gif::DisposalMethod::Background, x, y, w, h)) => {
                for yy in y..(y + h).min(alto) {
                    for xx in x..(x + w).min(ancho) {
                        self.lienzo.put_pixel(xx, yy, Rgba([0, 0, 0, 0]));
                    }
                }
            }
            Some((gif::DisposalMethod::Previous, ..)) => {
                if let Some(antes) = self.antes.take() {
                    self.lienzo = antes;
                }
            }
            _ => {}
        }
        let (x, y) = (u32::from(trozo.left), u32::from(trozo.top));
        let (w, h) = (u32::from(trozo.width), u32::from(trozo.height));
        if trozo.dispose == gif::DisposalMethod::Previous {
            self.antes = Some(self.lienzo.clone());
        }
        // Los píxeles transparentes del trozo dejan ver lo de debajo: así se compone un GIF.
        for (i, p) in trozo.buffer.chunks_exact(4).enumerate() {
            if p[3] == 0 {
                continue;
            }
            let (xx, yy) = (x + i as u32 % w.max(1), y + i as u32 / w.max(1));
            if xx < ancho && yy < alto {
                self.lienzo.put_pixel(xx, yy, Rgba([p[0], p[1], p[2], 255]));
            }
        }
        self.pendiente = Some((trozo.dispose, x, y, w, h));
        // Los navegadores enseñan a 100 ms lo que dice durar 0 o 10: son GIF hechos para
        // ir «lo más rápido posible», y a esa velocidad nadie los ha visto nunca.
        let centesimas = if trozo.delay <= 1 { 10 } else { trozo.delay };
        let dura_ms = u64::from(centesimas) * 10;
        let desde_ms = self.reloj_ms;
        self.reloj_ms += dura_ms;
        // Lo que quede transparente sale negro: detrás de un fondo no hay nada que asome.
        let mut imagen = self.lienzo.clone();
        for p in imagen.pixels_mut() {
            if p.0[3] < 255 {
                let a = u32::from(p.0[3]);
                *p = Rgba([
                    (u32::from(p.0[0]) * a / 255) as u8,
                    (u32::from(p.0[1]) * a / 255) as u8,
                    (u32::from(p.0[2]) * a / 255) as u8,
                    255,
                ]);
            }
        }
        Ok(Some(Cuadro { desde_ms, dura_ms, imagen }))
    }

    fn leido(&self) -> f32 {
        (self.leido.load(Ordering::Relaxed) as f32 / self.total as f32).clamp(0.0, 1.0)
    }
}

// --- El vídeo, con Media Foundation ---

#[cfg(windows)]
mod mf {
    use super::*;
    use windows::core::{Interface, HSTRING};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    /// Media Foundation y COM encendidos mientras viva esto, en el hilo que lo crea.
    struct Encendido {
        com: bool,
    }

    impl Encendido {
        fn nuevo() -> Result<Self> {
            // Si el hilo ya tenía COM de otra forma, contesta que no y no pasa nada: vale
            // igual, solo que entonces no le toca apagarlo.
            let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
            unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }?;
            Ok(Self { com })
        }
    }

    impl Drop for Encendido {
        fn drop(&mut self) {
            unsafe {
                let _ = MFShutdown();
                if self.com {
                    CoUninitialize();
                }
            }
        }
    }

    /// Lee un vídeo en orden, ya en RGB: el lector de Windows convierte el formato del
    /// codificador (NV12, casi siempre) y aquí solo se copian las filas.
    pub struct LectorMf {
        // El orden importa: el lector se suelta antes de apagar Media Foundation.
        lector: IMFSourceReader,
        ancho: u32,
        alto: u32,
        /// Bytes de una fila a la siguiente cuando el búfer no dice el suyo. Negativo es que
        /// la imagen viene de abajo arriba.
        paso: i32,
        duracion_ms: Option<u64>,
        ultimo_ms: u64,
        _encendido: Encendido,
    }

    impl LectorMf {
        pub fn abrir(ruta: &Path) -> Result<Self> {
            let encendido = Encendido::nuevo()?;
            let lector = unsafe {
                let mut atributos: Option<IMFAttributes> = None;
                MFCreateAttributes(&mut atributos, 1)?;
                let atributos = atributos.ok_or_else(|| AppError::Msg("Media Foundation no responde".into()))?;
                // Con esto el lector convierte a RGB él solo, sea cual sea el codificador.
                atributos.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;
                MFCreateSourceReaderFromURL(&HSTRING::from(ruta.as_os_str()), &atributos).map_err(|e| {
                    AppError::Msg(format!(
                        "Windows no sabe abrir ese vídeo ({}). Prueba con un MP4 o un GIF.",
                        e.message()
                    ))
                })?
            };
            unsafe {
                lector.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
                lector.SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true).map_err(|_| {
                    AppError::Msg("ese archivo no tiene imagen: es solo sonido".into())
                })?;
                let tipo = MFCreateMediaType()?;
                tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
                tipo.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
                lector.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &tipo)?;
            }
            let duracion_ms = unsafe {
                lector
                    .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
                    .ok()
                    .and_then(|v| u64::try_from(&v).ok())
                    .map(|cien_ns| cien_ns / 10_000)
                    .filter(|&ms| ms > 0)
            };
            let mut este = Self {
                lector,
                ancho: 0,
                alto: 0,
                paso: 0,
                duracion_ms,
                ultimo_ms: 0,
                _encendido: encendido,
            };
            este.leer_medida()?;
            Ok(este)
        }

        fn leer_medida(&mut self) -> Result<()> {
            unsafe {
                let tipo = self.lector.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)?;
                let medida = tipo.GetUINT64(&MF_MT_FRAME_SIZE)?;
                self.ancho = (medida >> 32) as u32;
                self.alto = (medida & 0xFFFF_FFFF) as u32;
                self.paso = tipo
                    .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                    .map(|p| p as i32)
                    .unwrap_or(self.ancho as i32 * 4);
            }
            if self.ancho == 0 || self.alto == 0 {
                return Err(AppError::Msg("ese vídeo no dice cuánto mide".into()));
            }
            Ok(())
        }

        /// Copia un búfer BGRX de Windows a una imagen RGBA, fila a fila.
        fn copiar(&self, muestra: &IMFSample) -> Result<RgbaImage> {
            let (ancho, alto) = (self.ancho, self.alto);
            let mut imagen = RgbaImage::new(ancho, alto);
            let fila = ancho as usize * 4;
            unsafe {
                let bufer = if muestra.GetBufferCount()? == 1 {
                    muestra.GetBufferByIndex(0)?
                } else {
                    muestra.ConvertToContiguousBuffer()?
                };
                // El búfer 2D dice dónde empieza la fila de arriba y cuánto se salta: es lo
                // fiable, también con imágenes de abajo arriba o con filas acolchadas.
                let (inicio, paso, dos_d) = if let Ok(b2) = bufer.cast::<IMF2DBuffer>() {
                    let mut p = std::ptr::null_mut();
                    let mut paso = 0i32;
                    b2.Lock2D(&mut p, &mut paso)?;
                    (p, paso, Some(b2))
                } else {
                    let mut p = std::ptr::null_mut();
                    bufer.Lock(&mut p, None, None)?;
                    // Sin búfer 2D, el paso del formato. Negativo: la primera fila en memoria
                    // es la de abajo, así que la de arriba está al final.
                    let paso = self.paso;
                    let p = if paso < 0 { p.offset(-(paso as isize) * (alto as isize - 1)) } else { p };
                    (p, paso, None)
                };
                let destino: &mut [u8] = &mut imagen;
                for y in 0..alto as usize {
                    let origen = std::slice::from_raw_parts(inicio.offset(paso as isize * y as isize), fila);
                    let fila_destino = &mut destino[y * fila..(y + 1) * fila];
                    for (d, s) in fila_destino.chunks_exact_mut(4).zip(origen.chunks_exact(4)) {
                        d[0] = s[2];
                        d[1] = s[1];
                        d[2] = s[0];
                        d[3] = 255;
                    }
                }
                match dos_d {
                    Some(b2) => b2.Unlock2D()?,
                    None => bufer.Unlock()?,
                }
            }
            Ok(imagen)
        }
    }

    impl Lector for LectorMf {
        fn siguiente(&mut self) -> Result<Option<Cuadro>> {
            loop {
                let mut banderas = 0u32;
                let mut cuando = 0i64;
                let mut muestra: Option<IMFSample> = None;
                unsafe {
                    self.lector.ReadSample(
                        MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                        0,
                        None,
                        Some(&mut banderas as *mut u32),
                        Some(&mut cuando as *mut i64),
                        Some(&mut muestra as *mut Option<IMFSample>),
                    )?;
                }
                if banderas & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                    self.leer_medida()?;
                }
                if banderas & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    return Ok(None);
                }
                // Sin muestra es un hueco del vídeo, no el final: se sigue leyendo.
                let Some(muestra) = muestra else { continue };
                let desde_ms = (cuando.max(0) / 10_000) as u64;
                let dura_ms = unsafe { muestra.GetSampleDuration() }
                    .ok()
                    .map(|d| (d.max(0) / 10_000) as u64)
                    .filter(|&d| d > 0)
                    .unwrap_or(1000 / u64::from(FPS));
                self.ultimo_ms = desde_ms;
                let imagen = self.copiar(&muestra)?;
                return Ok(Some(Cuadro { desde_ms, dura_ms, imagen }));
            }
        }

        fn leido(&self) -> f32 {
            match self.duracion_ms {
                Some(total) => (self.ultimo_ms as f32 / total as f32).clamp(0.0, 1.0),
                None => 0.0,
            }
        }
    }
}

// --- Importar ---

/// Lo que queda en el disco al importar: el MP4 y su primer fotograma en JPEG, que es lo
/// que enseña el selector mientras no se está reproduciendo.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Importado {
    pub ruta: String,
    pub vista: String,
    pub ancho: u32,
    pub alto: u32,
    pub dura_ms: u64,
}

/// El tamaño al que se guarda un fondo: cabe en 1920x1080 sin deformarse, con los dos lados
/// pares y sin bajar del mínimo que acepta el codificador.
pub fn medida_importada(ancho: u32, alto: u32) -> (u32, u32) {
    let escala = (f64::from(ANCHO_MAXIMO) / f64::from(ancho.max(1)))
        .min(f64::from(ALTO_MAXIMO) / f64::from(alto.max(1)))
        .min(1.0);
    let par = |v: f64| ((v.round() as u32).max(2) / 2) * 2;
    crate::encode::mp4::medida_que_acepta(par(f64::from(ancho) * escala), par(f64::from(alto) * escala))
}

/// Convierte un vídeo o un GIF en el fondo animado de `destino` (un `.mp4`), con su vista en
/// `destino` cambiado a `.jpg`.
///
/// `progreso` recibe de 0 a 1. Si `cancelar` se enciende, para en el fotograma siguiente,
/// borra lo que llevaba escrito y devuelve `CANCELADO`.
#[cfg(windows)]
pub fn importar(
    origen: &Path,
    destino: &Path,
    cancelar: &AtomicBool,
    progreso: &mut dyn FnMut(f32),
) -> Result<Importado> {
    use windows_capture::encoder::{
        AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder, VideoSettingsSubType,
    };

    if let Some(carpeta) = destino.parent() {
        std::fs::create_dir_all(carpeta)?;
    }
    let parcial = destino.with_extension("parcial.mp4");
    let vista = destino.with_extension("jpg");
    let _ = std::fs::remove_file(&parcial);

    let mut lector = abrir(origen)?;
    let Some(primero) = lector.siguiente()? else {
        return Err(AppError::Msg("ese archivo no tiene ni un fotograma".into()));
    };
    let (ancho, alto) = medida_importada(primero.imagen.width(), primero.imagen.height());
    let opciones = crate::encode::mp4::Mp4Options { width: ancho, height: alto, fps: FPS, quality: 60 };

    // Todo lo que puede fallar a medias va aquí dentro, para borrar el `.parcial` al salir
    // por donde sea: cancelado, un fotograma roto o el disco lleno.
    let escrito = (|| -> Result<u64> {
        let mut codificador = VideoEncoder::new(
            VideoSettingsBuilder::new(ancho, alto)
                .sub_type(VideoSettingsSubType::H264)
                .frame_rate(FPS)
                .bitrate(opciones.bitrate()),
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            &parcial,
        )
        .map_err(|e| AppError::Msg(e.to_string()))?;

        let paso_ms = 1000.0 / f64::from(FPS);
        let origen_ms = primero.desde_ms;
        let mut siguiente_ms = 0.0f64;
        let mut ultimo: Option<(Vec<u8>, u64)> = None;
        let mut fin_ms = 0u64;
        let mut cuadro = Some(primero);
        let mut vista_hecha = false;
        while let Some(c) = cuadro {
            if cancelar.load(Ordering::Relaxed) {
                return Err(AppError::Msg(CANCELADO.into()));
            }
            let ms = c.desde_ms.saturating_sub(origen_ms);
            fin_ms = fin_ms.max(ms + c.dura_ms);
            // A 30 por segundo como mucho: de un vídeo a 60 se queda uno de cada dos.
            if ms as f64 + 0.5 >= siguiente_ms {
                let imagen = crate::encode::escalar::a_medida(&c.imagen, ancho, alto);
                if !vista_hecha {
                    crate::encode::jpg::save(&imagen, &vista, ancho, alto, 85)?;
                    vista_hecha = true;
                }
                let bufer = crate::encode::mp4::rgba_to_bgra_bottom_up(&imagen);
                codificador
                    .send_frame_buffer(&bufer, ms as i64 * 10_000)
                    .map_err(|e| AppError::Msg(e.to_string()))?;
                ultimo = Some((bufer, ms));
                while siguiente_ms <= ms as f64 + 0.5 {
                    siguiente_ms += paso_ms;
                }
            }
            progreso(lector.leido());
            cuadro = lector.siguiente()?;
        }
        // El último otra vez, en el instante en que acaba: sin esto el vídeo terminaría donde
        // empieza su último fotograma (ver `mp4::encode`), y un GIF de dos fotogramas largos
        // se quedaría en la mitad.
        if let Some((bufer, ms)) = ultimo {
            if fin_ms > ms {
                codificador
                    .send_frame_buffer(&bufer, fin_ms as i64 * 10_000)
                    .map_err(|e| AppError::Msg(e.to_string()))?;
            }
        }
        codificador.finish().map_err(|e| AppError::Msg(e.to_string()))?;
        Ok(fin_ms)
    })();

    let dura_ms = match escrito {
        Ok(ms) => ms,
        Err(e) => {
            borrar_con_paciencia(&parcial);
            let _ = std::fs::remove_file(&vista);
            return Err(e);
        }
    };
    let _ = std::fs::remove_file(destino);
    std::fs::rename(&parcial, destino)?;
    progreso(1.0);
    Ok(Importado {
        ruta: destino.to_string_lossy().to_string(),
        vista: vista.to_string_lossy().to_string(),
        ancho,
        alto,
        dura_ms,
    })
}

#[cfg(not(windows))]
pub fn importar(
    _origen: &Path,
    _destino: &Path,
    _cancelar: &AtomicBool,
    _progreso: &mut dyn FnMut(f32),
) -> Result<Importado> {
    Err(AppError::Unsupported)
}

/// Borra un archivo que puede seguir abierto unos instantes: el codificador de Windows lo
/// suelta al destruirse, pero no siempre antes de que se pida borrarlo.
fn borrar_con_paciencia(ruta: &Path) {
    for _ in 0..20 {
        match std::fs::remove_file(ruta) {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

// --- Exportar ---

/// Lo que se le hace a cada fotograma del fondo antes de entregarlo: ponerle la sombra.
pub type Vestir = Box<dyn Fn(&mut RgbaImage) + Send>;

/// Un fotograma del fondo, ya del tamaño del lienzo y con la sombra puesta.
struct Listo {
    desde_ms: u64,
    imagen: Arc<RgbaImage>,
}

/// El fondo animado de una exportación: un hilo que lee el vídeo por delante y contesta con
/// el fondo de cada instante.
///
/// Se le pide en orden (el instante del vídeo exportado) y lleva siempre dos fotogramas
/// preparados: mientras se viste y se codifica uno, el siguiente ya se está leyendo. El
/// hilo se cierra solo cuando lleva un rato sin que nadie le pida nada, y suelta el lector
/// y la memoria: entre una exportación y otra no se queda nada abierto.
pub struct Animado {
    pide: mpsc::Sender<u64>,
    contesta: mpsc::Receiver<std::result::Result<Arc<RgbaImage>, String>>,
}

/// Cuánto espera el hilo sin peticiones antes de cerrarse.
const SIN_USO: Duration = Duration::from_secs(10);
/// Cuántos fotogramas lleva preparados por delante.
const POR_DELANTE: usize = 2;

impl Animado {
    /// Empieza a leer `ruta` para un lienzo de `ancho` x `alto`.
    pub fn abrir(ruta: PathBuf, ancho: u32, alto: u32, vestir: Vestir) -> Self {
        let (pide, peticiones) = mpsc::channel::<u64>();
        let (respuestas, contesta) = mpsc::channel();
        std::thread::Builder::new()
            .name("fondo-animado".into())
            .spawn(move || {
                let mut hilo = Hilo { ruta, ancho, alto, vestir, lector: None, vuelta_ms: 0, fin_ms: 0, hubo: false };
                let mut actual: Option<Listo> = None;
                let mut cola: std::collections::VecDeque<Listo> = Default::default();
                while let Ok(ms) = peticiones.recv_timeout(SIN_USO) {
                    let respuesta = (|| -> Result<Arc<RgbaImage>> {
                        // Hacia atrás (otra exportación, o la paleta del GIF) se vuelve a
                        // empezar: el vídeo solo se lee hacia delante.
                        if actual.as_ref().is_some_and(|a| ms < a.desde_ms) {
                            hilo.reiniciar();
                            actual = None;
                            cola.clear();
                        }
                        loop {
                            if cola.is_empty() {
                                cola.push_back(hilo.siguiente()?);
                            }
                            let entra = actual.is_none() || cola.front().is_some_and(|s| s.desde_ms <= ms);
                            if !entra {
                                break;
                            }
                            actual = cola.pop_front();
                        }
                        actual
                            .as_ref()
                            .map(|a| a.imagen.clone())
                            .ok_or_else(|| AppError::Msg("el fondo no tiene fotogramas".into()))
                    })();
                    let fallo = respuesta.is_err();
                    if respuestas.send(respuesta.map_err(|e| e.to_string())).is_err() || fallo {
                        break;
                    }
                    // Lo de por delante, mientras quien pidió viste y codifica el suyo.
                    while cola.len() < POR_DELANTE {
                        match hilo.siguiente() {
                            Ok(listo) => cola.push_back(listo),
                            Err(_) => break,
                        }
                    }
                }
            })
            .expect("no se ha podido crear el hilo del fondo animado");
        Self { pide, contesta }
    }

    /// El fondo en ese instante del vídeo exportado. `None` si el hilo ya no está (se cerró
    /// por no usarse) o no pudo leer el vídeo: quien llama decide qué pinta entonces.
    pub fn en(&self, ms: u64) -> std::result::Result<Arc<RgbaImage>, Option<String>> {
        self.pide.send(ms).map_err(|_| None)?;
        match self.contesta.recv_timeout(Duration::from_secs(60)) {
            Ok(Ok(imagen)) => Ok(imagen),
            Ok(Err(motivo)) => Err(Some(motivo)),
            Err(_) => Err(None),
        }
    }
}

/// Lo que vive dentro del hilo: el lector y por dónde va.
struct Hilo {
    ruta: PathBuf,
    ancho: u32,
    alto: u32,
    vestir: Vestir,
    lector: Option<Box<dyn Lector>>,
    /// Lo que suman las vueltas ya dadas: el vídeo se repite, pero el reloj sigue.
    vuelta_ms: u64,
    /// Hasta dónde llega lo leído en esta vuelta.
    fin_ms: u64,
    /// Si ya salió algún fotograma: un vídeo que acaba sin dar ninguno no se repite.
    hubo: bool,
}

impl Hilo {
    fn reiniciar(&mut self) {
        self.lector = None;
        self.vuelta_ms = 0;
        self.fin_ms = 0;
    }

    fn siguiente(&mut self) -> Result<Listo> {
        for _ in 0..2 {
            if self.lector.is_none() {
                self.lector = Some(abrir(&self.ruta)?);
            }
            let lector = self.lector.as_mut().expect("recien abierto");
            if let Some(c) = lector.siguiente()? {
                self.hubo = true;
                self.fin_ms = self.fin_ms.max(c.desde_ms + c.dura_ms);
                let mut imagen = crate::encode::marco::cubrir(&c.imagen, self.ancho, self.alto);
                (self.vestir)(&mut imagen);
                return Ok(Listo { desde_ms: self.vuelta_ms + c.desde_ms, imagen: Arc::new(imagen) });
            }
            if !self.hubo {
                break;
            }
            // Se acabó: otra vuelta, que empieza donde acabó esta.
            self.vuelta_ms += self.fin_ms.max(1);
            self.fin_ms = 0;
            self.lector = None;
        }
        Err(AppError::Msg("el fondo animado no tiene fotogramas".into()))
    }
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;

    /// Un GIF pequeño hecho aquí: `n` fotogramas de `ancho` x `alto`, cada uno de un color,
    /// que dura `centesimas`. El primero entero y los demás solo un trozo, como los de verdad.
    pub(crate) fn gif_de_prueba(ruta: &Path, colores: &[[u8; 3]], ancho: u16, alto: u16, centesimas: u16) {
        let archivo = std::fs::File::create(ruta).unwrap();
        let mut gif = gif::Encoder::new(archivo, ancho, alto, &[]).unwrap();
        for (i, c) in colores.iter().enumerate() {
            let (w, h) = if i == 0 { (ancho, alto) } else { (ancho / 2, alto) };
            let mut rgba: Vec<u8> = (0..u32::from(w) * u32::from(h)).flat_map(|_| [c[0], c[1], c[2], 255]).collect();
            let mut trozo = gif::Frame::from_rgba_speed(w, h, &mut rgba, 10);
            trozo.delay = centesimas;
            gif.write_frame(&trozo).unwrap();
        }
    }

    #[test]
    fn el_gif_se_lee_compuesto_y_con_su_tiempo() {
        let dir = std::env::temp_dir().join(format!("winshotx-gif-fondo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("tres.gif");
        gif_de_prueba(&ruta, &[[255, 0, 0], [0, 0, 255], [0, 255, 0]], 40, 20, 5);
        let mut lector = LectorGif::abrir(&ruta).unwrap();
        let a = lector.siguiente().unwrap().unwrap();
        let b = lector.siguiente().unwrap().unwrap();
        let c = lector.siguiente().unwrap().unwrap();
        assert!(lector.siguiente().unwrap().is_none(), "solo hay tres");
        assert_eq!((a.desde_ms, b.desde_ms, c.desde_ms), (0, 50, 100));
        assert_eq!(a.imagen.dimensions(), (40, 20));
        // El segundo solo pinta la mitad izquierda: la derecha sigue siendo el primero.
        assert!(b.imagen.get_pixel(5, 10).0[2] > 200, "la izquierda tendría que ser azul");
        assert!(b.imagen.get_pixel(35, 10).0[0] > 200, "la derecha sigue roja");
        assert!(lector.leido() > 0.99);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_gif_que_dice_durar_cero_dura_cien_milisegundos() {
        let dir = std::env::temp_dir().join(format!("winshotx-gif-cero-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("cero.gif");
        gif_de_prueba(&ruta, &[[10, 10, 10], [200, 200, 200]], 8, 8, 0);
        let mut lector = LectorGif::abrir(&ruta).unwrap();
        lector.siguiente().unwrap();
        assert_eq!(lector.siguiente().unwrap().unwrap().desde_ms, 100);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lo_grande_se_guarda_a_1080_sin_deformarse_y_lo_pequenno_no_crece() {
        assert_eq!(medida_importada(3840, 2160), (1920, 1080));
        assert_eq!(medida_importada(1080, 1920), (608, 1080));
        assert_eq!(medida_importada(640, 360), (640, 360));
        let (a, b) = medida_importada(4001, 999);
        assert_eq!((a % 2, b % 2), (0, 0), "{a}x{b} tiene un lado impar");
        assert!(a <= ANCHO_MAXIMO && b <= ALTO_MAXIMO);
    }

    /// El hilo de exportar da vueltas: con un GIF de 150 ms, el instante 400 es el 100 de
    /// la tercera vuelta, y el fondo es el tercer fotograma.
    #[test]
    fn el_fondo_animado_avanza_y_se_repite() {
        let dir = std::env::temp_dir().join(format!("winshotx-gif-vueltas-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("vueltas.gif");
        gif_de_prueba(&ruta, &[[255, 0, 0], [0, 0, 255], [0, 255, 0]], 16, 16, 5);
        let animado = Animado::abrir(ruta, 32, 32, Box::new(|_| {}));
        let color = |ms: u64| *animado.en(ms).expect("un fondo").get_pixel(4, 16);
        assert_eq!(color(0).0[0], 255, "empieza rojo");
        assert_eq!(color(60).0[2], 255, "a los 60 ms, azul");
        assert_eq!(color(120).0[1], 255, "a los 120, verde");
        assert_eq!(color(160).0[0], 255, "a los 160 vuelve a empezar: rojo");
        assert_eq!(color(400).0[1], 255, "a los 400 es el 100 de la tercera vuelta: verde");
        // Y hacia atrás también contesta, empezando de nuevo.
        assert_eq!(color(0).0[0], 255);
        drop(animado);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_fondo_que_no_existe_contesta_con_un_motivo_y_no_cuelga() {
        let animado = Animado::abrir(PathBuf::from("C:\\no-existe\\fondo.gif"), 32, 32, Box::new(|_| {}));
        assert!(matches!(animado.en(0), Err(Some(_))));
    }
}
