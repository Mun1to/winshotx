use std::path::Path;
use std::process::Command;

use crate::error::{AppError, Result};

#[cfg(windows)]
const NO_WINDOW: u32 = 0x0800_0000; // CREATE_NO_WINDOW: nada de consolas parpadeando

fn command() -> Command {
    let mut cmd = Command::new("ffmpeg");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(NO_WINDOW);
    }
    cmd
}

/// FFmpeg nunca se empaqueta: solo se usa si el usuario ya lo tiene en el PATH.
pub fn available() -> bool {
    command()
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run(args: &[String]) -> Result<()> {
    let output = command()
        .args(args)
        .output()
        .map_err(|e| AppError::Msg(format!("no se ha podido ejecutar ffmpeg: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr.lines().rev().take(4).collect::<Vec<_>>().join(" | ");
        return Err(AppError::Msg(format!("ffmpeg ha fallado: {tail}")));
    }
    Ok(())
}

/// Ruta de maxima calidad: paleta generada por el propio contenido del clip.
pub fn gif_from_video(
    source: &Path,
    destination: &Path,
    fps: u32,
    width: u32,
    quality: u8,
) -> Result<()> {
    let colors = (32 + (quality.clamp(10, 100) as u32 * 224) / 100).min(256);
    let dither = if quality >= 60 {
        "sierra2_4a"
    } else {
        "bayer:bayer_scale=3"
    };
    let filter = format!(
        "fps={fps},scale={width}:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors={colors}:stats_mode=diff[p];[s1][p]paletteuse=dither={dither}:diff_mode=rectangle"
    );
    run(&[
        "-y".into(),
        "-i".into(),
        source.to_string_lossy().to_string(),
        "-filter_complex".into(),
        filter,
        "-loop".into(),
        "0".into(),
        destination.to_string_lossy().to_string(),
    ])
}

pub fn mp4_from_video(
    source: &Path,
    destination: &Path,
    fps: u32,
    width: u32,
    height: u32,
    quality: u8,
) -> Result<()> {
    // CRF invertido: mas calidad pedida, menos compresion con perdida.
    let crf = 34 - (quality.clamp(10, 100) as u32 * 20) / 100;
    let filter = format!("fps={fps},scale={width}:{height}:flags=lanczos");
    run(&[
        "-y".into(),
        "-i".into(),
        source.to_string_lossy().to_string(),
        "-vf".into(),
        filter,
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-crf".into(),
        crf.to_string(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-movflags".into(),
        "+faststart".into(),
        destination.to_string_lossy().to_string(),
    ])
}
