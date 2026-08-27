use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use image::RgbaImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::capture::Rect;
use crate::error::{AppError, Result};

#[cfg(windows)]
pub mod audio;
pub mod delta;
pub mod mezcla;
pub mod raton;
pub mod realce;
#[cfg(windows)]
pub mod win;

use delta::Parche;

pub const THUMB_HEIGHT: u32 = 80;

/// Un fotograma dentro del cache: se guarda en QOI, que es sin perdida y muy rapido.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FrameEntry {
    pub index: u32,
    pub timestamp_ms: u64,
    pub duration_ms: u32,
    pub thumb_path: String,
    pub offset: u64,
    pub len: u32,
    /// La zona que cambio respecto al fotograma anterior, o `None` si lo que hay guardado
    /// es el fotograma entero. Ver `delta`: es lo que hace que un minuto de grabacion no
    /// ocupe un gigabyte.
    pub patch: Option<Parche>,
}

/// Cada cuantos fotogramas se guarda uno entero. Marca dos cosas a la vez: cuanto trabajo
/// cuesta reconstruir uno cualquiera (como mucho, esto), y cuanto se pierde si un byte del
/// archivo sale mal (como mucho, hasta el siguiente entero).
const FOTOGRAMA_ENTERO_CADA: u32 = 30;

/// Y si lo que ha cambiado ocupa mas que esto, se guarda entero: por encima el recorte no
/// ahorra lo suficiente para pagar el trabajo de reconstruirlo despues.
const PARTE_MAXIMA: f64 = 0.6;

/// El sonido que se grabo, guardado aparte de la imagen.
///
/// Se guarda en crudo (PCM de 16 bits) y no dentro del MP4 de vista previa porque el
/// usuario recorta por fotogramas: al exportar hay que quedarse con el tramo que va del
/// primer fotograma al ultimo, y de un MP4 ya montado eso no se saca sin desmontarlo.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    pub channels: u16,
    pub sample_rate: u32,
}

impl AudioInfo {
    /// Cuantos bytes ocupa un milisegundo de sonido, con todos sus canales.
    pub fn bytes_por_ms(&self) -> u64 {
        u64::from(self.sample_rate) * u64::from(self.channels) * 2 / 1000
    }

    /// El trozo de archivo que corresponde a ese tramo de tiempo, alineado para no cortar
    /// una muestra por la mitad: media muestra suena a chasquido.
    pub fn tramo(&self, desde_ms: u64, hasta_ms: u64) -> (u64, u64) {
        let bloque = u64::from(self.channels) * 2;
        let alinear = |bytes: u64| bytes / bloque * bloque;
        let inicio = alinear(desde_ms * self.bytes_por_ms());
        let fin = alinear(hasta_ms.max(desde_ms) * self.bytes_por_ms());
        (inicio, fin.saturating_sub(inicio))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionData {
    pub id: String,
    pub dir: PathBuf,
    pub region: Rect,
    pub fps: u32,
    pub format: String,
    pub has_audio: bool,
    pub width: u32,
    pub height: u32,
    pub mp4_path: Option<PathBuf>,
    #[serde(default)]
    pub audio: Option<AudioInfo>,
    pub frames: Vec<FrameEntry>,
}

impl SessionData {
    pub fn cache_path(&self) -> PathBuf {
        self.dir.join("frames.bin")
    }

    /// El sonido en crudo, si lo hubo.
    pub fn audio_path(&self) -> PathBuf {
        self.dir.join("audio.pcm")
    }

    pub fn duration_ms(&self) -> u64 {
        self.frames
            .last()
            .map(|f| f.timestamp_ms + f.duration_ms as u64)
            .unwrap_or(0)
    }

    pub fn persist(&self) -> Result<()> {
        std::fs::write(
            self.dir.join("session.json"),
            serde_json::to_string_pretty(self)?,
        )?;
        Ok(())
    }
}

/// Escritor secuencial del cache de fotogramas.
pub struct FrameCache {
    file: BufWriter<File>,
    offset: u64,
    entries: Vec<FrameEntry>,
    last_hash: Option<u64>,
    dir: PathBuf,
    /// El ultimo fotograma completo, para poder comparar con el que llega. Ocupa una
    /// imagen en memoria (9 MB a 1920x1200), que es mucho menos de lo que se ahorra en
    /// disco cada segundo.
    anterior: Option<Vec<u8>>,
    desde_entero: u32,
}

impl FrameCache {
    pub fn new(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        std::fs::create_dir_all(dir.join("thumbs"))?;
        Ok(Self {
            file: BufWriter::with_capacity(1 << 20, File::create(dir.join("frames.bin"))?),
            offset: 0,
            entries: Vec::new(),
            last_hash: None,
            dir: dir.to_path_buf(),
            anterior: None,
            desde_entero: 0,
        })
    }

    /// Devuelve false cuando el fotograma es identico al anterior: en ese caso
    /// no se escribe nada y solo se alarga la duracion del ultimo, como ScreenToGif.
    pub fn push_rgba(&mut self, rgba: &[u8], width: u32, height: u32, ts_ms: u64) -> Result<bool> {
        let hash = quick_hash(rgba);
        if self.last_hash == Some(hash) {
            return Ok(false);
        }

        // De cada fotograma se guarda solo la zona que ha cambiado. Se guarda entero
        // cuando no hay con que comparar, cuando toca uno de referencia, o cuando ha
        // cambiado tanto que recortar ya no ahorra nada.
        let entero = u64::from(width) * u64::from(height);
        let parche = self
            .anterior
            .as_ref()
            .filter(|_| self.desde_entero + 1 < FOTOGRAMA_ENTERO_CADA)
            .and_then(|previo| delta::zona_cambiada(previo, rgba, width, height))
            .filter(|p| (p.pixeles() as f64) < entero as f64 * PARTE_MAXIMA);

        let (encoded, guardado) = match parche {
            Some(p) => (
                qoi::encode_to_vec(&delta::recortar(rgba, width, p), p.width, p.height)?,
                Some(p),
            ),
            None => (qoi::encode_to_vec(rgba, width, height)?, None),
        };

        self.file.write_all(&encoded)?;
        self.entries.push(FrameEntry {
            index: self.entries.len() as u32,
            timestamp_ms: ts_ms,
            duration_ms: 0,
            thumb_path: String::new(),
            offset: self.offset,
            len: encoded.len() as u32,
            patch: guardado,
        });
        self.offset += encoded.len() as u64;
        self.last_hash = Some(hash);
        self.desde_entero = if guardado.is_some() {
            self.desde_entero + 1
        } else {
            0
        };
        match self.anterior.as_mut() {
            Some(previo) if previo.len() == rgba.len() => previo.copy_from_slice(rgba),
            _ => self.anterior = Some(rgba.to_vec()),
        }
        Ok(true)
    }

    pub fn bytes_written(&self) -> u64 {
        self.offset
    }

    pub fn frame_count(&self) -> usize {
        self.entries.len()
    }

    /// Cierra el fichero y calcula la duracion real de cada fotograma.
    pub fn finish(mut self, total_ms: u64, fallback_fps: u32) -> Result<Vec<FrameEntry>> {
        self.file.flush()?;
        let fallback = (1000 / fallback_fps.max(1)) as u32;
        let count = self.entries.len();
        for i in 0..count {
            let next_ts = if i + 1 < count {
                self.entries[i + 1].timestamp_ms
            } else {
                total_ms.max(self.entries[i].timestamp_ms + fallback as u64)
            };
            let delta = next_ts.saturating_sub(self.entries[i].timestamp_ms) as u32;
            self.entries[i].duration_ms = delta.clamp(10, 10_000);
        }
        let _ = self.dir;
        Ok(self.entries)
    }
}

/// Hash barato para saber si el fotograma ha cambiado. El paso es 5 y no 4 a
/// proposito: con 4 sobre datos RGBA se leeria siempre el mismo canal de cada
/// pixel, y un cambio que solo tocara el verde o el azul pasaria por identico.
fn quick_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < data.len() {
        hash ^= data[i] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        i += 5;
    }
    hash ^= data.len() as u64;
    hash
}

/// El ultimo fotograma guardado entero en o antes de `index`. Desde ahi hacia delante
/// solo hay parches, y hay que aplicarlos todos en orden para tener la imagen completa.
fn desde_el_entero(session: &SessionData, index: usize) -> usize {
    (0..=index)
        .rev()
        .find(|&i| session.frames[i].patch.is_none())
        .unwrap_or(0)
}

/// Lee un fotograma leyendo el ultimo entero y pegandole encima los parches que vienen
/// detras. Como se guarda uno entero cada treinta, esto son treinta pegados como mucho.
fn reconstruir(session: &SessionData, index: usize) -> Result<(u32, u32, Vec<u8>)> {
    let mut file = File::open(session.cache_path())?;
    let mut ancho = 0u32;
    let mut alto = 0u32;
    let mut lienzo: Vec<u8> = Vec::new();
    let mut leido = LectorFotogramas::new(&mut file);
    for i in desde_el_entero(session, index)..=index {
        leido.aplicar_en(&session.frames[i], &mut ancho, &mut alto, &mut lienzo)?;
    }
    Ok((ancho, alto, lienzo))
}

/// Un archivo abierto y un buffer que se reutiliza, para no pedir memoria nueva por cada
/// parche cuando se reconstruye una tira entera de fotogramas.
struct LectorFotogramas<'a> {
    file: &'a mut File,
    buffer: Vec<u8>,
}

impl<'a> LectorFotogramas<'a> {
    fn new(file: &'a mut File) -> Self {
        Self {
            file,
            buffer: Vec::new(),
        }
    }

    fn aplicar_en(
        &mut self,
        entry: &FrameEntry,
        ancho: &mut u32,
        alto: &mut u32,
        lienzo: &mut Vec<u8>,
    ) -> Result<()> {
        self.file.seek(SeekFrom::Start(entry.offset))?;
        self.buffer.resize(entry.len as usize, 0);
        self.file.read_exact(&mut self.buffer)?;
        let (header, pixels) = qoi::decode_to_vec(&self.buffer)?;
        match entry.patch {
            None => {
                *ancho = header.width;
                *alto = header.height;
                *lienzo = pixels;
            }
            Some(parche) => delta::aplicar(lienzo, *ancho, parche, &pixels),
        }
        Ok(())
    }
}

pub fn read_frame(session: &SessionData, index: usize) -> Result<RgbaImage> {
    if index >= session.frames.len() {
        return Err(AppError::Msg(format!("fotograma {index} inexistente")));
    }
    let (ancho, alto, pixeles) = reconstruir(session, index)?;
    RgbaImage::from_raw(ancho, alto, pixeles)
        .ok_or_else(|| AppError::Msg("fotograma corrupto en la caché".into()))
}

/// Genera las miniaturas de la tira de tiempo. Se hace en paralelo porque es
/// lo unico que separa al usuario del editor cuando para la grabacion.
pub fn generate_thumbnails(session: &mut SessionData) -> Result<()> {
    let dir = session.dir.join("thumbs");
    std::fs::create_dir_all(&dir)?;
    let cache_path = session.cache_path();
    let ratio = THUMB_HEIGHT as f32 / session.height.max(1) as f32;
    let thumb_width = ((session.width as f32 * ratio).round() as u32).max(1);

    // Ya no vale sacar cada miniatura por su cuenta: un fotograma guardado a parches
    // necesita los de delante. Se reparte por GRUPOS, cada uno empezando en un fotograma
    // entero, y dentro del grupo se va reconstruyendo en orden. Asi se sigue usando toda
    // la maquina, que es lo unico que separa a Munir del editor al parar de grabar.
    let mut grupos: Vec<Vec<FrameEntry>> = Vec::new();
    for entry in &session.frames {
        if entry.patch.is_none() || grupos.is_empty() {
            grupos.push(Vec::new());
        }
        grupos
            .last_mut()
            .expect("acabamos de meter uno")
            .push(entry.clone());
    }

    let results: Vec<Result<()>> = grupos
        .par_iter()
        .map(|grupo| -> Result<()> {
            let destinos: Vec<PathBuf> = grupo
                .iter()
                .map(|f| dir.join(format!("{:06}.png", f.index)))
                .collect();
            if destinos.iter().all(|p| p.exists()) {
                return Ok(());
            }
            let mut file = File::open(&cache_path)?;
            let mut lector = LectorFotogramas::new(&mut file);
            let mut ancho = 0u32;
            let mut alto = 0u32;
            let mut lienzo: Vec<u8> = Vec::new();
            for (entry, destino) in grupo.iter().zip(destinos) {
                // Hay que pasar por todos aunque su miniatura ya exista: el siguiente
                // fotograma se apoya en este.
                lector.aplicar_en(entry, &mut ancho, &mut alto, &mut lienzo)?;
                if destino.exists() {
                    continue;
                }
                let image = RgbaImage::from_raw(ancho, alto, lienzo.clone())
                    .ok_or_else(|| AppError::Msg("miniatura corrupta".into()))?;
                let thumb = image::imageops::resize(
                    &image,
                    thumb_width,
                    THUMB_HEIGHT,
                    image::imageops::FilterType::Triangle,
                );
                thumb.save(destino)?;
            }
            Ok(())
        })
        .collect();

    for result in results {
        result?;
    }

    for frame in session.frames.iter_mut() {
        frame.thumb_path = dir
            .join(format!("{:06}.png", frame.index))
            .to_string_lossy()
            .to_string();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una pantalla de mentira: fondo fijo con un cuadrado que se mueve, que es lo que
    /// pasa de verdad al grabar (casi todo quieto y una cosa moviendose).
    fn pantalla(width: u32, height: u32, paso: u32) -> Vec<u8> {
        let mut frame = vec![24u8; (width * height) as usize * 4];
        for (i, byte) in frame.iter_mut().enumerate() {
            if i % 4 == 3 {
                *byte = 255;
            }
        }
        let x0 = (paso * 3) % (width - 8);
        for y in 4..12u32 {
            for x in x0..x0 + 8 {
                let p = ((y * width + x) * 4) as usize;
                frame[p] = 220;
                frame[p + 1] = 40;
                frame[p + 2] = 40;
            }
        }
        frame
    }

    /// Un fondo con grano determinista, que se parece a lo que comprime una pantalla real
    /// mucho mas que un color plano.
    fn con_grano(width: u32, height: u32) -> Vec<u8> {
        let mut frame = vec![0u8; (width * height) as usize * 4];
        let mut semilla: u32 = 0x1234_5678;
        for pixel in frame.chunks_exact_mut(4) {
            semilla = semilla.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            pixel[0] = (semilla >> 16) as u8;
            pixel[1] = (semilla >> 8) as u8;
            pixel[2] = semilla as u8;
            pixel[3] = 255;
        }
        frame
    }

    fn pegar_cursor(frame: &mut [u8], width: u32, paso: u32) {
        let x0 = (paso * 3) % (width - 16);
        for y in 4..20u32 {
            for x in x0..x0 + 16 {
                let p = ((y * width + x) * 4) as usize;
                frame[p] = 255;
                frame[p + 1] = 255;
                frame[p + 2] = 255;
            }
        }
    }

    fn carpeta(nombre: &str) -> PathBuf {
        let unico = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("winshotx-test-{nombre}-{unico}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sesion_de(dir: &Path, frames: Vec<FrameEntry>, width: u32, height: u32) -> SessionData {
        SessionData {
            id: "test".into(),
            dir: dir.to_path_buf(),
            region: Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            fps: 30,
            format: "mp4".into(),
            has_audio: false,
            width,
            height,
            mp4_path: None,
            audio: None,
            frames,
        }
    }

    /// Lo unico que no se puede negociar: lo que se lee tiene que ser exactamente lo que
    /// se grabo, fotograma a fotograma. Si esto falla, el editor ensenna basura y el video
    /// exportado sale mal, y ademas no se notaria hasta que alguien mirase el resultado.
    #[test]
    fn lo_que_se_lee_es_exactamente_lo_que_se_grabo() {
        let (ancho, alto, cuantos) = (64u32, 40u32, 75usize);
        let dir = carpeta("ida-y-vuelta");
        let originales: Vec<Vec<u8>> = (0..cuantos as u32).map(|i| pantalla(ancho, alto, i)).collect();

        let mut cache = FrameCache::new(&dir).expect("no se ha podido crear la caché");
        for (i, frame) in originales.iter().enumerate() {
            let guardado = cache
                .push_rgba(frame, ancho, alto, i as u64 * 33)
                .expect("no se ha podido escribir");
            assert!(guardado, "el fotograma {i} tenía que entrar, no es igual al anterior");
        }
        let entries = cache.finish(cuantos as u64 * 33, 30).expect("no se ha cerrado");
        let sesion = sesion_de(&dir, entries, ancho, alto);

        for (i, original) in originales.iter().enumerate() {
            let leido = read_frame(&sesion, i).expect("no se ha podido leer");
            assert_eq!(
                leido.as_raw(),
                original,
                "el fotograma {i} no se reconstruye igual"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Y que sirva para algo: grabando una pantalla casi quieta, guardar solo lo que
    /// cambia tiene que ocupar mucho menos que guardar la imagen entera cada vez. Es la
    /// razon de ser de todo esto: a pantalla completa eran 19 MB por segundo.
    #[test]
    fn guardar_solo_lo_que_cambia_ocupa_mucho_menos() {
        let (ancho, alto, cuantos) = (320u32, 200u32, 60u32);
        let dir = carpeta("ahorro");
        let mut cache = FrameCache::new(&dir).expect("no se ha podido crear la caché");
        let mut enteros = 0u64;
        for i in 0..cuantos {
            let frame = pantalla(ancho, alto, i);
            enteros += qoi::encode_to_vec(&frame, ancho, alto)
                .expect("no se ha podido comprimir")
                .len() as u64;
            cache
                .push_rgba(&frame, ancho, alto, u64::from(i) * 33)
                .expect("no se ha podido escribir");
        }
        let escrito = cache.bytes_written();
        let _ = cache.finish(u64::from(cuantos) * 33, 30);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            escrito * 3 < enteros,
            "guardando solo lo que cambia se han escrito {escrito} bytes y guardándolo \
             todo serían {enteros}: no llega ni a un tercio de ahorro"
        );
    }

    /// El corte del sonido tiene que caer donde cae el recorte de imagen, y siempre en
    /// una muestra entera: media muestra suena a chasquido, y un byte de desfase arrastra
    /// todo el sonido que viene detras.
    #[test]
    fn el_sonido_se_corta_donde_se_corta_la_imagen() {
        let info = AudioInfo {
            channels: 2,
            sample_rate: 48_000,
        };
        // Dos canales de dos bytes a 48 kHz: 192 bytes por milisegundo.
        assert_eq!(info.bytes_por_ms(), 192);

        // Del segundo 1 al 3: empieza en 192.000 y dura dos segundos.
        let (inicio, largo) = info.tramo(1_000, 3_000);
        assert_eq!(inicio, 192_000);
        assert_eq!(largo, 384_000);

        // Todo alineado a bloques de cuatro bytes, pase lo que pase con los milisegundos.
        let (inicio, largo) = info.tramo(333, 777);
        assert_eq!(inicio % 4, 0, "el principio corta una muestra por la mitad");
        assert_eq!(largo % 4, 0, "y el final también");

        // Un recorte al revés no puede devolver un tamaño dado la vuelta.
        assert_eq!(info.tramo(5_000, 1_000).1, 0);
        assert_eq!(info.tramo(0, 0).1, 0);
    }

    /// La cifra de la META 3, medida en vez de estimada. No corre sola porque tarda unos
    /// segundos: `cargo test --release medir_lo_que_ocupa -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn medir_lo_que_ocupa_un_segundo_de_grabacion() {
        let (ancho, alto, fps) = (1920u32, 1200u32, 30u32);
        let dir = carpeta("medida");
        let mut cache = FrameCache::new(&dir).expect("no se ha podido crear la caché");
        let mut enteros = 0u64;
        // Un fondo con grano, no liso: sobre un color plano QOI comprime tanto que la
        // medida saldria bonita y falsa. Una pantalla de verdad tiene texto, iconos y
        // fotos, y ahi es donde se llegaba a los 19 MB por segundo.
        let fondo = con_grano(ancho, alto);
        for i in 0..fps {
            let mut frame = fondo.clone();
            pegar_cursor(&mut frame, ancho, i);
            enteros += qoi::encode_to_vec(&frame, ancho, alto).expect("qoi").len() as u64;
            cache
                .push_rgba(&frame, ancho, alto, u64::from(i) * 33)
                .expect("no se ha podido escribir");
        }
        let escrito = cache.bytes_written();
        let _ = cache.finish(1000, fps);
        let _ = std::fs::remove_dir_all(&dir);
        let mb = |b: u64| b as f64 / 1024.0 / 1024.0;
        println!(
            "un segundo a {ancho}x{alto} y {fps} fps: antes {:.1} MB, ahora {:.2} MB ({:.0} veces menos)",
            mb(enteros),
            mb(escrito),
            enteros as f64 / escrito.max(1) as f64
        );
    }

    /// Cada treinta fotogramas se guarda uno entero, para que reconstruir uno cualquiera
    /// no obligue a recorrer la grabacion desde el principio.
    #[test]
    fn hay_un_fotograma_entero_cada_treinta() {
        let (ancho, alto) = (48u32, 32u32);
        let dir = carpeta("enteros");
        let mut cache = FrameCache::new(&dir).expect("no se ha podido crear la caché");
        for i in 0..90u32 {
            cache
                .push_rgba(&pantalla(ancho, alto, i), ancho, alto, u64::from(i) * 33)
                .expect("no se ha podido escribir");
        }
        let entries = cache.finish(90 * 33, 30).expect("no se ha cerrado");
        let _ = std::fs::remove_dir_all(&dir);

        let enteros: Vec<u32> = entries
            .iter()
            .filter(|e| e.patch.is_none())
            .map(|e| e.index)
            .collect();
        assert_eq!(enteros, vec![0, 30, 60], "los enteros no caen donde deberían");
    }
}
