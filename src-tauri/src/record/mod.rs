use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use image::RgbaImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::capture::Rect;
use crate::error::{AppError, Result};

#[cfg(windows)]
pub mod win;

pub const THUMB_HEIGHT: u32 = 80;

/// Un fotograma dentro del cache: se guarda en QOI, que es sin perdida y muy rapido.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameEntry {
    pub index: u32,
    pub timestamp_ms: u64,
    pub duration_ms: u32,
    pub thumb_path: String,
    pub offset: u64,
    pub len: u32,
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
    pub frames: Vec<FrameEntry>,
}

impl SessionData {
    pub fn cache_path(&self) -> PathBuf {
        self.dir.join("frames.bin")
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
        })
    }

    /// Devuelve false cuando el fotograma es identico al anterior: en ese caso
    /// no se escribe nada y solo se alarga la duracion del ultimo, como ScreenToGif.
    pub fn push_rgba(&mut self, rgba: &[u8], width: u32, height: u32, ts_ms: u64) -> Result<bool> {
        let hash = quick_hash(rgba);
        if self.last_hash == Some(hash) {
            return Ok(false);
        }
        let encoded = qoi::encode_to_vec(rgba, width, height)?;
        self.file.write_all(&encoded)?;
        self.entries.push(FrameEntry {
            index: self.entries.len() as u32,
            timestamp_ms: ts_ms,
            duration_ms: 0,
            thumb_path: String::new(),
            offset: self.offset,
            len: encoded.len() as u32,
        });
        self.offset += encoded.len() as u64;
        self.last_hash = Some(hash);
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

pub fn read_frame(session: &SessionData, index: usize) -> Result<RgbaImage> {
    let entry = session
        .frames
        .get(index)
        .ok_or_else(|| AppError::Msg(format!("fotograma {index} inexistente")))?;
    let mut file = File::open(session.cache_path())?;
    file.seek(SeekFrom::Start(entry.offset))?;
    let mut buffer = vec![0u8; entry.len as usize];
    file.read_exact(&mut buffer)?;
    let (header, pixels) = qoi::decode_to_vec(&buffer)?;
    RgbaImage::from_raw(header.width, header.height, pixels)
        .ok_or_else(|| AppError::Msg("fotograma corrupto en el cache".into()))
}

/// Genera las miniaturas de la tira de tiempo. Se hace en paralelo porque es
/// lo unico que separa al usuario del editor cuando para la grabacion.
pub fn generate_thumbnails(session: &mut SessionData) -> Result<()> {
    let dir = session.dir.join("thumbs");
    std::fs::create_dir_all(&dir)?;
    let cache_path = session.cache_path();
    let ratio = THUMB_HEIGHT as f32 / session.height.max(1) as f32;
    let thumb_width = ((session.width as f32 * ratio).round() as u32).max(1);

    let jobs: Vec<(usize, u64, u32, PathBuf)> = session
        .frames
        .iter()
        .map(|f| {
            (
                f.index as usize,
                f.offset,
                f.len,
                dir.join(format!("{:06}.png", f.index)),
            )
        })
        .collect();

    let results: Vec<Result<()>> = jobs
        .par_iter()
        .map(|(_, offset, len, path)| -> Result<()> {
            if path.exists() {
                return Ok(());
            }
            let mut file = File::open(&cache_path)?;
            file.seek(SeekFrom::Start(*offset))?;
            let mut buffer = vec![0u8; *len as usize];
            file.read_exact(&mut buffer)?;
            let (header, pixels) = qoi::decode_to_vec(&buffer)?;
            let image = RgbaImage::from_raw(header.width, header.height, pixels)
                .ok_or_else(|| AppError::Msg("miniatura corrupta".into()))?;
            let thumb = image::imageops::resize(
                &image,
                thumb_width,
                THUMB_HEIGHT,
                image::imageops::FilterType::Triangle,
            );
            thumb.save(path)?;
            Ok(())
        })
        .collect();

    for result in results {
        result?;
    }

    for (frame, (_, _, _, path)) in session.frames.iter_mut().zip(jobs.iter()) {
        frame.thumb_path = path.to_string_lossy().to_string();
    }
    Ok(())
}
