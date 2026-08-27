use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::encode::{anotacion, ffmpeg, gif, marco, mp4, png};
use crate::error::{AppError, Result};
use crate::record::{self, SessionData};
use crate::state::AppState;

pub const EVENT_PROGRESS: &str = "winshotx://export-progress";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub session_id: String,
    pub format: String,
    pub engine: String,
    pub from: usize,
    pub to: usize,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u8,
    /// Si el video exportado lleva el sonido que se grabo. Lo decide el interruptor del
    /// editor, y solo tiene efecto si la grabacion llego a capturar audio.
    #[allow(dead_code)]
    pub audio: bool,
    #[serde(rename = "loop")]
    pub loop_forever: bool,
    /// Pixeles de aire alrededor de la captura. Cero es sin marco, que es lo de siempre.
    #[serde(default)]
    pub margin: u32,
    /// El nombre del fondo que eligio el panel: blanco, negro, gris, atardecer o menta.
    #[serde(default)]
    pub background: String,
    /// Si la captura lleva sombra sobre ese fondo.
    #[serde(default)]
    pub shadow: bool,
    /// Las marcas dibujadas encima, en coordenadas de 0 a 1 sobre la imagen.
    #[serde(default)]
    pub annotations: Vec<anotacion::Anotacion>,
    pub destination: Option<String>,
    pub copy_to_clipboard: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub bytes: u64,
    pub copied: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    stage: String,
    done: usize,
    total: usize,
}

/// Elige los fotogramas que tocan para el fps pedido y calcula sus retardos.
/// Bajar de 60 a 20 fps no puede consistir en tirar fotogramas al tuntun.
fn resample(session: &SessionData, from: usize, to: usize, fps: u32) -> (Vec<usize>, Vec<u32>) {
    let from = from.min(session.frames.len().saturating_sub(1));
    let to = to.min(session.frames.len().saturating_sub(1)).max(from);
    let slice = &session.frames[from..=to];
    let step_ms = (1000.0 / fps.max(1) as f32).round() as u64;
    let start_ms = slice.first().map(|f| f.timestamp_ms).unwrap_or(0);
    let end_ms = slice
        .last()
        .map(|f| f.timestamp_ms + f.duration_ms as u64)
        .unwrap_or(start_ms);

    let mut indices = Vec::new();
    let mut delays = Vec::new();
    let mut cursor = 0usize;
    let mut t = start_ms;
    while t < end_ms {
        while cursor + 1 < slice.len() && slice[cursor + 1].timestamp_ms <= t {
            cursor += 1;
        }
        // El ultimo paso se recorta: el clip exportado dura lo que dura el recorte.
        let slot = step_ms.min(end_ms - t) as u32;
        let picked = from + cursor;
        if indices.last() != Some(&picked) {
            indices.push(picked);
            delays.push(slot);
        } else if let Some(last) = delays.last_mut() {
            // El mismo fotograma se queda en pantalla mas tiempo en vez de repetirse.
            *last += slot;
        }
        t += step_ms;
    }

    if indices.is_empty() {
        indices.push(from);
        delays.push(step_ms.max(20) as u32);
    }
    (indices, delays)
}

fn timestamped_name(extension: &str) -> String {
    let now = chrono::Local::now();
    format!("winshotx-{}.{extension}", now.format("%Y%m%d-%H%M%S"))
}

fn destination_path(
    app: &AppHandle,
    request: &ExportRequest,
    extension: &str,
) -> Result<PathBuf> {
    let state = app.state::<AppState>();
    let dir = request
        .destination
        .clone()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| state.settings.read().save_directory.clone());
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(timestamped_name(extension)))
}

fn load_session(app: &AppHandle, id: &str) -> Result<SessionData> {
    let state = app.state::<AppState>();
    if let Some(session) = state.sessions.read().get(id) {
        return Ok(session.clone());
    }
    // Segunda oportunidad: la sesion sigue en disco aunque la app se haya reiniciado.
    let path = state.session_dir(id).join("session.json");
    let raw = std::fs::read_to_string(path).map_err(|_| AppError::UnknownSession(id.to_string()))?;
    let session: SessionData = serde_json::from_str(&raw)?;
    Ok(session)
}

pub fn export(app: &AppHandle, request: ExportRequest) -> Result<ExportResult> {
    let started = Instant::now();
    let session = load_session(app, &request.session_id)?;
    let width = request.width.max(1);
    let height = request.height.max(1);

    // El marco crece por fuera de lo que se pidio en «Dimensiones»: quien elige 800 de
    // ancho y 40 de aire quiere la captura a 800, no a 720 con bordes. Asi que el tamanno
    // que se le pide al codificador es el de despues de enmarcar.
    let marco = marco::Marco {
        margen: request.margin.min(400),
        fondo: marco::Fondo::desde(&request.background),
        sombra: request.shadow,
    };
    let (ancho_final, alto_final) = marco.medida(width, height);

    let emit = |stage: &str, done: usize, total: usize| {
        let _ = app.emit(
            EVENT_PROGRESS,
            ExportProgress {
                stage: stage.to_string(),
                done,
                total,
            },
        );
    };

    let path = match request.format.as_str() {
        "png" => {
            let path = destination_path(app, &request, "png")?;
            emit("reading", 0, 1);
            let image = record::read_frame(&session, request.from)?;
            let image = enmarcar_y_anotar(image, width, height, marco, &request.annotations);
            png::save(&image, &path, ancho_final, alto_final)?;
            path
        }
        "gif" => {
            let (indices, delays) = resample(&session, request.from, request.to, request.fps);
            let path = destination_path(app, &request, "gif")?;
            if request.engine == "ffmpeg" && ffmpeg::available() {
                let temporary = session.dir.join("export-source.mp4");
                encode_mp4(&session, &indices, &delays, &temporary, &request, &emit)?;
                emit("encoding", 0, 1);
                ffmpeg::gif_from_video(&temporary, &path, request.fps, ancho_final, request.quality)?;
                let _ = std::fs::remove_file(&temporary);
            } else {
                let mut loader = |index: usize| {
                    record::read_frame(&session, index).map(|imagen| {
                        enmarcar_y_anotar(imagen, width, height, marco, &request.annotations)
                    })
                };
                gif::encode(
                    &indices,
                    &delays,
                    &mut loader,
                    &path,
                    &gif::GifOptions {
                        width: ancho_final,
                        height: alto_final,
                        quality: request.quality,
                        loop_forever: request.loop_forever,
                    },
                    |stage, done, total| emit(stage, done, total),
                )?;
            }
            path
        }
        "mp4" => {
            let (indices, delays) = resample(&session, request.from, request.to, request.fps);
            let path = destination_path(app, &request, "mp4")?;
            if request.engine == "ffmpeg" && ffmpeg::available() {
                let temporary = session.dir.join("export-source.mp4");
                encode_mp4(&session, &indices, &delays, &temporary, &request, &emit)?;
                emit("encoding", 0, 1);
                ffmpeg::mp4_from_video(
                    &temporary,
                    &path,
                    request.fps,
                    ancho_final,
                    alto_final,
                    request.quality,
                )?;
                let _ = std::fs::remove_file(&temporary);
            } else {
                encode_mp4(&session, &indices, &delays, &path, &request, &emit)?;
            }
            path
        }
        other => return Err(AppError::Msg(format!("formato desconocido: {other}"))),
    };

    emit("done", 1, 1);

    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let copied = if request.copy_to_clipboard {
        copy_result(&path, &session, &request).is_ok()
    } else {
        false
    };

    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        bytes,
        copied,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

/// Escala, dibuja las marcas encima y despues pone el marco.
///
/// **Ese orden y no otro.** Las marcas van sobre la captura, no sobre el fondo: una flecha
/// dibujada en el 90 % del ancho apunta al 90 % de la CAPTURA, y si se enmarcara antes,
/// ese 90 % caeria dentro del aire de la derecha. Y se escala primero para que las
/// coordenadas, que van de 0 a 1, se apliquen sobre la imagen del tamanno final.
fn enmarcar_y_anotar(
    imagen: image::RgbaImage,
    ancho: u32,
    alto: u32,
    marco: marco::Marco,
    anotaciones: &[anotacion::Anotacion],
) -> image::RgbaImage {
    let mut escalada = if imagen.dimensions() == (ancho, alto) {
        imagen
    } else {
        image::imageops::resize(&imagen, ancho, alto, image::imageops::FilterType::Lanczos3)
    };
    if !anotaciones.is_empty() {
        anotacion::pintar(&mut escalada, anotaciones);
    }
    marco::poner(&escalada, marco)
}

fn encode_mp4<F>(
    session: &SessionData,
    indices: &[usize],
    delays: &[u32],
    path: &Path,
    request: &ExportRequest,
    emit: &F,
) -> Result<()>
where
    F: Fn(&str, usize, usize),
{
    let ancho = request.width.max(1);
    let alto = request.height.max(1);
    let marco = marco::Marco {
        margen: request.margin.min(400),
        fondo: marco::Fondo::desde(&request.background),
        sombra: request.shadow,
    };
    let (ancho_final, alto_final) = marco.medida(ancho, alto);
    let mut loader = |index: usize| {
        record::read_frame(session, index)
            .map(|imagen| enmarcar_y_anotar(imagen, ancho, alto, marco, &request.annotations))
    };
    mp4::encode(
        indices,
        delays,
        &mut loader,
        path,
        &mp4::Mp4Options {
            width: ancho_final,
            height: alto_final,
            fps: request.fps.max(1),
            quality: request.quality,
        },
        // El interruptor del editor manda: alguien puede querer el vídeo mudo.
        request.audio.then(|| pista_de_audio(session, indices)).flatten(),
        |stage, done, total| emit(stage, done, total),
    )
}

/// El trozo de sonido que le toca al recorte que se esta exportando.
///
/// El usuario recorta por fotogramas, asi que el tramo va del primero al ultimo de los
/// que se exportan. Sin esto el video que guarda sale mudo, aunque la grabacion tuviera
/// sonido: exportar vuelve a codificar desde los fotogramas, y ahi no hay audio ninguno.
fn pista_de_audio(session: &SessionData, indices: &[usize]) -> Option<mp4::Pista> {
    use std::io::{Seek, SeekFrom};

    let info = session.audio?;
    let primero = session.frames.get(*indices.first()?)?;
    let ultimo = session.frames.get(*indices.last()?)?;
    let desde = primero.timestamp_ms;
    let hasta = ultimo.timestamp_ms + u64::from(ultimo.duration_ms);
    let (inicio, largo) = info.tramo(desde, hasta);
    if largo == 0 {
        return None;
    }

    let mut archivo = std::fs::File::open(session.audio_path()).ok()?;
    archivo.seek(SeekFrom::Start(inicio)).ok()?;
    let mut datos = vec![0u8; largo as usize];
    // Lo que haya: si la grabacion acabo antes de lo que dicen los fotogramas, se exporta
    // el sonido que exista en vez de fallar por unos milisegundos de menos.
    let leidos = leer_lo_que_haya(&mut archivo, &mut datos);
    datos.truncate(leidos);
    if datos.is_empty() {
        return None;
    }
    Some(mp4::Pista {
        channels: info.channels,
        sample_rate: info.sample_rate,
        datos,
    })
}

fn leer_lo_que_haya(archivo: &mut std::fs::File, destino: &mut [u8]) -> usize {
    use std::io::Read;
    let mut total = 0;
    while total < destino.len() {
        match archivo.read(&mut destino[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(_) => break,
        }
    }
    total
}

/// Una imagen se pega como imagen; un GIF o un MP4 se pegan como archivo.
fn copy_result(path: &Path, session: &SessionData, request: &ExportRequest) -> Result<()> {
    if request.format == "png" {
        let image = record::read_frame(session, request.from)?;
        let bytes = png::to_bytes(&image)?;
        crate::platform::clipboard::copy_image(&image, &bytes)
    } else {
        crate::platform::clipboard::copy_files(&[path])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::Rect;
    use crate::record::FrameEntry;

    fn session_with(durations: &[u32]) -> SessionData {
        let mut timestamp = 0u64;
        let frames = durations
            .iter()
            .enumerate()
            .map(|(index, duration)| {
                let entry = FrameEntry {
                    index: index as u32,
                    timestamp_ms: timestamp,
                    duration_ms: *duration,
                    thumb_path: String::new(),
                    offset: 0,
                    len: 0,
                    patch: None,
                };
                timestamp += *duration as u64;
                entry
            })
            .collect();
        SessionData {
            id: "test".into(),
            dir: PathBuf::from("."),
            region: Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            fps: 50,
            format: "gif".into(),
            has_audio: false,
            width: 10,
            height: 10,
            mp4_path: None,
            audio: None,
            frames,
        }
    }

    #[test]
    fn bajar_los_fps_reparte_el_tiempo_sin_perderlo() {
        // 10 fotogramas de 20 ms = 200 ms de clip, exportados a 25 fps (40 ms).
        let session = session_with(&[20; 10]);
        let (indices, delays) = resample(&session, 0, 9, 25);
        assert_eq!(indices.len(), 5);
        assert_eq!(delays.iter().sum::<u32>(), 200);
    }

    #[test]
    fn los_fotogramas_repetidos_alargan_el_retardo() {
        // Un solo fotograma que dura 500 ms no debe repetirse 12 veces a 25 fps.
        let session = session_with(&[500]);
        let (indices, delays) = resample(&session, 0, 0, 25);
        assert_eq!(indices, vec![0]);
        assert_eq!(delays, vec![500]);
    }

    #[test]
    fn el_rango_recortado_manda() {
        let session = session_with(&[50; 8]);
        let (indices, _) = resample(&session, 2, 4, 20);
        assert!(indices.iter().all(|i| (2..=4).contains(i)));
    }
}
