//! El sonido de clic propio (S10): elegir un archivo, dejarlo listo en la carpeta de la app,
//! y darle a la vista previa del editor las mismas muestras que van al vídeo.
//!
//! **Lo elegido se copia a `<datos>/winshotx/sonidos`**, como los fondos propios: exportar no
//! puede depender de que el usuario no mueva su archivo, y un estilo guardado tiene que seguir
//! sonando igual dentro de un mes. La copia es un WAV nuestro (48 kHz, un canal, 16 bits) ya
//! preparado (`preparar`): sin el silencio del principio, que haría sonar el clic tarde, como
//! mucho un segundo, y con el pico en 1, para que el volumen del editor signifique lo mismo
//! con uno propio que con los de serie.
//!
//! Un WAV se lee en Rust (`encode::wav`). Lo demás (MP3, M4A, WMA, FLAC) lo abre Media
//! Foundation, que ya viene con Windows y que la app usa para la cámara y los fondos de
//! vídeo: ni una dependencia más.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::contrato::ClicElegido;
use crate::error::{AppError, Result};

/// Lo más largo que puede durar un sonido de clic propio. Dos clics pueden ir a 150 ms uno
/// de otro: un sonido de diez segundos se pisaría consigo mismo todo el vídeo.
pub const LARGO_MAXIMO_MS: u32 = 1_000;
/// A qué frecuencia se guarda la copia.
pub const FRECUENCIA: u32 = 48_000;
/// Cuántos sonidos propios se guardan: al importar uno nuevo se borran los más viejos.
const GUARDADOS: usize = 12;
/// Lo que se lee del original, como mucho: de un archivo largo solo hace falta el principio.
const SEGUNDOS_LEIDOS: f64 = 10.0;
/// Por debajo de esto, sobre el pico, es silencio al principio del sonido.
const UMBRAL_DE_SILENCIO: f32 = 0.02;
/// El final se apaga en este rato cuando hay que cortar, para que no suene un chasquido.
const FUNDIDO_MS: u32 = 30;

fn carpeta(app: &AppHandle) -> Result<PathBuf> {
    let carpeta = app.path().app_data_dir()?.join("winshotx").join("sonidos");
    std::fs::create_dir_all(&carpeta)?;
    Ok(carpeta)
}

/// Lo que se hace con un sonido antes de guardarlo como clic: a 48 kHz, sin el silencio del
/// principio (deja 2 ms de antes, para no comerse el ataque) ni el del final, como mucho
/// `LARGO_MAXIMO_MS` con su fundido si se corta, y con el pico en 1.
pub fn preparar(muestras: &[f32], frecuencia: u32) -> Result<Vec<f32>> {
    let mut s = crate::encode::wav::a_frecuencia(muestras, frecuencia, FRECUENCIA);
    let pico = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if pico < 1e-4 {
        return Err("ese sonido está en silencio".into());
    }
    let umbral = pico * UMBRAL_DE_SILENCIO;
    let primero = s.iter().position(|x| x.abs() >= umbral).unwrap_or(0);
    let ultimo = s.iter().rposition(|x| x.abs() >= umbral).unwrap_or(s.len() - 1);
    let desde = primero.saturating_sub((FRECUENCIA / 500) as usize);
    s = s[desde..=ultimo].to_vec();
    let maximo = (FRECUENCIA * LARGO_MAXIMO_MS / 1000) as usize;
    if s.len() > maximo {
        s.truncate(maximo);
        let fundido = (FRECUENCIA * FUNDIDO_MS / 1000) as usize;
        let empieza = maximo - fundido;
        for (k, x) in s[empieza..].iter_mut().enumerate() {
            *x *= 1.0 - k as f32 / fundido as f32;
        }
    }
    for x in &mut s {
        *x /= pico;
    }
    Ok(s)
}

/// Las muestras de un archivo de sonido, en un canal, y su frecuencia. Solo el principio:
/// `SEGUNDOS_LEIDOS`.
pub fn leer_archivo(ruta: &Path) -> Result<(Vec<f32>, u32)> {
    use std::io::Read;
    let mut cabecera = [0u8; 12];
    let mut archivo = std::fs::File::open(ruta)
        .map_err(|_| AppError::Msg(format!("no encuentro {}", ruta.display())))?;
    let leidos = archivo.read(&mut cabecera)?;
    if leidos == 12 && &cabecera[0..4] == b"RIFF" && &cabecera[8..12] == b"WAVE" {
        // Diez segundos de lo más gordo que se ve (192 kHz, 8 canales de 64 bits) son 120 MB:
        // con 32 sobra para cualquier clic, y un WAV de una hora no se lee entero.
        let mut bytes = cabecera.to_vec();
        archivo.take(32 << 20).read_to_end(&mut bytes)?;
        if let Ok(leido) = crate::encode::wav::leer(&bytes) {
            return Ok(recortar_lectura(leido));
        }
    }
    leer_con_windows(ruta)
}

fn recortar_lectura((mut muestras, frecuencia): (Vec<f32>, u32)) -> (Vec<f32>, u32) {
    muestras.truncate((SEGUNDOS_LEIDOS * f64::from(frecuencia)) as usize);
    (muestras, frecuencia)
}

#[cfg(windows)]
fn leer_con_windows(ruta: &Path) -> Result<(Vec<f32>, u32)> {
    mf::leer(ruta, SEGUNDOS_LEIDOS).map_err(|e| {
        AppError::Msg(format!("ese sonido no se puede abrir ({e}). Prueba con un WAV o un MP3."))
    })
}

#[cfg(not(windows))]
fn leer_con_windows(_ruta: &Path) -> Result<(Vec<f32>, u32)> {
    Err("ese sonido no es un WAV, y sin Windows solo se leen WAV".into())
}

#[cfg(windows)]
mod mf {
    use super::*;
    use windows::core::HSTRING;
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    /// Media Foundation y COM encendidos mientras viva esto, en el hilo que lo crea.
    struct Encendido {
        com: bool,
    }

    impl Encendido {
        fn nuevo() -> Result<Self> {
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

    /// El sonido de cualquier archivo que sepa abrir Windows, ya descomprimido en coma
    /// flotante, en un canal y hasta `segundos`.
    pub fn leer(ruta: &Path, segundos: f64) -> Result<(Vec<f32>, u32)> {
        let _encendido = Encendido::nuevo()?;
        let audio = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
        let lector = unsafe {
            let mut atributos: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut atributos, 1)?;
            let atributos = atributos.ok_or_else(|| AppError::Msg("Media Foundation no responde".into()))?;
            let lector = MFCreateSourceReaderFromURL(&HSTRING::from(ruta.as_os_str()), &atributos)?;
            lector.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            lector
                .SetStreamSelection(audio, true)
                .map_err(|_| AppError::Msg("ese archivo no trae sonido".into()))?;
            let tipo = MFCreateMediaType()?;
            tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            tipo.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)?;
            lector.SetCurrentMediaType(audio, None, &tipo)?;
            lector
        };
        let (canales, frecuencia) = unsafe {
            let tipo = lector.GetCurrentMediaType(audio)?;
            (
                tipo.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)?.max(1) as usize,
                tipo.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?,
            )
        };
        if frecuencia == 0 {
            return Err("ese sonido no dice a qué frecuencia va".into());
        }
        let hasta = (segundos * f64::from(frecuencia)) as usize;
        let mut muestras: Vec<f32> = Vec::new();
        while muestras.len() < hasta {
            let mut banderas = 0u32;
            let mut muestra: Option<IMFSample> = None;
            unsafe {
                lector.ReadSample(
                    audio,
                    0,
                    None,
                    Some(&mut banderas as *mut u32),
                    None,
                    Some(&mut muestra as *mut Option<IMFSample>),
                )?;
            }
            if banderas & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
            let Some(muestra) = muestra else { continue };
            unsafe {
                let bufer = muestra.ConvertToContiguousBuffer()?;
                let mut p = std::ptr::null_mut();
                let mut largo = 0u32;
                bufer.Lock(&mut p, None, Some(&mut largo))?;
                let bytes = std::slice::from_raw_parts(p, largo as usize);
                for bloque in bytes.chunks_exact(4 * canales) {
                    let suma: f32 = bloque
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .sum();
                    let m = suma / canales as f32;
                    muestras.push(if m.is_finite() { m.clamp(-1.0, 1.0) } else { 0.0 });
                }
                bufer.Unlock()?;
            }
        }
        muestras.truncate(hasta);
        Ok((muestras, frecuencia))
    }
}

/// Lo que se le deja al usuario en el nombre de la copia, para enseñarlo en el editor: letras,
/// números, espacios, guiones y rayas bajas, como mucho 40.
fn nombre_limpio(origen: &Path) -> String {
    let base = origen.file_stem().and_then(|n| n.to_str()).unwrap_or("");
    let limpio: String = base
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .take(40)
        .collect();
    let limpio = limpio.trim();
    if limpio.is_empty() {
        "sonido".into()
    } else {
        limpio.into()
    }
}

/// Prepara el sonido de `origen` y lo deja en `carpeta`. El nombre es `clic-<huella>-<nombre>.wav`:
/// la huella es la del original (ruta, tamaño y hora), así que elegir otra vez el mismo
/// archivo no lo vuelve a convertir; el nombre es el suyo, para enseñarlo.
fn importar_en(carpeta: &Path, origen: &Path) -> Result<PathBuf> {
    let huella = crate::fondos::huella(origen)?;
    let destino = carpeta.join(format!("clic-{huella}-{}.wav", nombre_limpio(origen)));
    if destino.is_file() {
        crate::fondos::tocar(&destino);
        return Ok(destino);
    }
    let (muestras, frecuencia) = leer_archivo(origen)?;
    let listo = preparar(&muestras, frecuencia)?;
    let parcial = destino.with_extension("parcial.wav");
    std::fs::write(&parcial, crate::encode::wav::escribir_mono16(&listo, FRECUENCIA))?;
    std::fs::rename(&parcial, &destino)?;
    crate::fondos::podar(carpeta, "clic-", ".wav", GUARDADOS);
    Ok(destino)
}

/// El diálogo de abrir de Windows, con los formatos de sonido que se saben leer. Devuelve la
/// ruta, o nada si se cancela.
#[tauri::command]
pub async fn elegir_sonido_de_clic(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .add_filter("Sonido", &["wav", "mp3", "m4a", "aac", "wma", "flac"])
        .blocking_pick_file()?
        .into_path()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

/// Copia un sonido del usuario a la carpeta de sonidos, ya preparado, y devuelve la ruta de
/// la copia, que es la que va en `edicion.clic`.
#[tauri::command]
pub async fn importar_sonido_de_clic(app: AppHandle, origen: String) -> Result<String> {
    let carpeta = carpeta(&app)?;
    let origen = PathBuf::from(origen);
    let destino = tauri::async_runtime::spawn_blocking(move || importar_en(&carpeta, &origen))
        .await
        .map_err(|e| AppError::Msg(e.to_string()))??;
    Ok(destino.to_string_lossy().to_string())
}

/// Si la ruta de un sonido propio es de la carpeta de sonidos. La vista previa solo lee de
/// ahí: esta orden no sirve para leer cualquier archivo del disco.
fn es_de_la_carpeta(carpeta: &Path, ruta: &Path) -> bool {
    match (carpeta.canonicalize(), ruta.canonicalize()) {
        (Ok(carpeta), Ok(ruta)) => ruta.starts_with(carpeta),
        _ => false,
    }
}

/// Las muestras del clic elegido, para que la vista previa suene igual que el vídeo: en coma
/// flotante de 32 bits, una detrás de otra, en un canal y a `FRECUENCIA`. Son las mismas que
/// `encode::clic::mezclar` pone en el archivo.
#[tauri::command]
pub async fn muestras_de_clic(app: AppHandle, clic: Option<ClicElegido>) -> Result<tauri::ipc::Response> {
    if let Some(ClicElegido::Propio { ruta }) = &clic {
        if !es_de_la_carpeta(&carpeta(&app)?, Path::new(ruta)) {
            return Err("ese sonido no es de la carpeta de sonidos de winshotx".into());
        }
    }
    let muestras = crate::encode::clic::sonido(clic.as_ref(), FRECUENCIA);
    Ok(tauri::ipc::Response::new(muestras.iter().flat_map(|m| m.to_le_bytes()).collect::<Vec<u8>>()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn dir(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("winshotx-sonidos-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn seno(f: f32, ms: u32, frecuencia: u32, alto: f32) -> Vec<f32> {
        (0..frecuencia * ms / 1000)
            .map(|k| alto * (std::f32::consts::TAU * f * k as f32 / frecuencia as f32).sin())
            .collect()
    }

    #[test]
    fn preparar_quita_el_silencio_de_delante_y_deja_el_pico_en_uno() {
        // Medio segundo de silencio y luego 100 ms de un «la» flojito, a 44,1 kHz.
        let mut s = vec![0.0; 22_050];
        s.extend(seno(440.0, 100, 44_100, 0.2));
        let listo = preparar(&s, 44_100).unwrap();
        let ms = listo.len() as f32 * 1000.0 / FRECUENCIA as f32;
        assert!((100.0..=103.0).contains(&ms), "dura lo que suena, sin el silencio: {ms} ms");
        let pico = listo.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((pico - 1.0).abs() < 1e-3, "pico {pico}");
        // Y lo primero que suena está en los primeros 3 ms: el clic no llega tarde.
        let primero = listo.iter().position(|x| x.abs() > 0.05).unwrap();
        assert!(primero < 3 * 48, "empieza en la muestra {primero}");
    }

    #[test]
    fn preparar_corta_lo_largo_con_un_fundido_y_se_niega_al_silencio() {
        let largo = seno(300.0, 3_000, 48_000, 0.5);
        let listo = preparar(&largo, 48_000).unwrap();
        assert_eq!(listo.len(), 48_000, "como mucho un segundo");
        let cola = listo[listo.len() - 10..].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(cola < 0.01, "el final se apaga, no se corta en seco: {cola}");
        assert!(preparar(&[0.0; 4_800], 48_000).is_err());
    }

    #[test]
    fn importar_deja_un_wav_nuestro_que_suena_como_clic_y_no_lo_repite() {
        let d = dir("importar");
        let origen = d.join("mi clic (bueno).wav");
        // Estéreo de 16 bits a 44,1 kHz, como lo deja cualquier editor.
        let mono = seno(880.0, 200, 44_100, 0.4);
        let mut wav = crate::encode::wav::escribir_mono16(&[], 44_100);
        let datos: Vec<u8> = mono
            .iter()
            .flat_map(|m| {
                let v = ((m * 32_767.0) as i16).to_le_bytes();
                [v[0], v[1], v[0], v[1]]
            })
            .collect();
        wav[22] = 2; // dos canales
        wav[28..32].copy_from_slice(&(44_100u32 * 4).to_le_bytes());
        wav[32] = 4;
        wav[40..44].copy_from_slice(&(datos.len() as u32).to_le_bytes());
        wav.extend_from_slice(&datos);
        std::fs::write(&origen, &wav).unwrap();

        let guardados = d.join("sonidos");
        std::fs::create_dir_all(&guardados).unwrap();
        let copia = importar_en(&guardados, &origen).unwrap();
        let nombre = copia.file_name().unwrap().to_string_lossy().to_string();
        assert!(nombre.starts_with("clic-") && nombre.ends_with("-mi clic bueno.wav"), "{nombre}");
        let (muestras, frecuencia) = crate::encode::wav::leer(&std::fs::read(&copia).unwrap()).unwrap();
        assert_eq!(frecuencia, 48_000);
        assert!((muestras.len() as i64 - 9_600).abs() <= 2, "200 ms a 48 kHz: {}", muestras.len());
        // Y suena como clic de verdad, por el mismo camino que al exportar.
        let clic = crate::encode::clic::sonido(
            Some(&ClicElegido::Propio { ruta: copia.to_string_lossy().into() }),
            48_000,
        );
        assert_eq!(clic.len(), muestras.len());

        // Elegir otra vez el mismo archivo no lo vuelve a convertir.
        let hora = std::fs::metadata(&copia).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(importar_en(&guardados, &origen).unwrap(), copia);
        assert!(std::fs::metadata(&copia).unwrap().modified().unwrap() >= hora);
        assert_eq!(std::fs::read_dir(&guardados).unwrap().count(), 1, "sin copias de más ni a medias");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lo_que_no_es_sonido_se_dice() {
        let d = dir("malo");
        let malo = d.join("foto.mp3");
        std::fs::write(&malo, b"esto no es un sonido").unwrap();
        assert!(importar_en(&d, &malo).is_err());
        assert!(importar_en(&d, &d.join("no-esta.wav")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Media Foundation, el camino de los MP3, sin depender de tener uno: también abre WAV,
    /// así que se le da uno y tiene que salir lo mismo que lo lee Rust.
    #[cfg(windows)]
    #[test]
    fn windows_lee_lo_mismo_que_rust() {
        let d = dir("mf");
        let ruta = d.join("la.wav");
        let la = seno(440.0, 250, 44_100, 0.5);
        std::fs::write(&ruta, crate::encode::wav::escribir_mono16(&la, 44_100)).unwrap();
        let (de_windows, frecuencia) = mf::leer(&ruta, 10.0).unwrap();
        assert_eq!(frecuencia, 44_100);
        assert_eq!(de_windows.len(), la.len());
        let error = de_windows.iter().zip(&la).fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(error < 1e-3, "la mayor diferencia: {error}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Un MP3 de verdad, hecho con ffmpeg: sale por Media Foundation, empieza donde suena y
    /// dura lo que dura.
    ///
    ///     cargo test --lib un_mp3_de_verdad -- --ignored --nocapture
    #[cfg(windows)]
    #[test]
    #[ignore = "necesita ffmpeg para hacer el MP3: se corre a mano"]
    fn un_mp3_de_verdad() {
        let d = dir("mp3");
        let wav = d.join("la.wav");
        let mut s = vec![0.0; 4_410];
        s.extend(seno(440.0, 300, 44_100, 0.5));
        std::fs::write(&wav, crate::encode::wav::escribir_mono16(&s, 44_100)).unwrap();
        let mp3 = d.join("la.mp3");
        let hecho = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-i"])
            .arg(&wav)
            .args(["-codec:a", "libmp3lame", "-b:a", "128k"])
            .arg(&mp3)
            .status()
            .expect("ffmpeg");
        assert!(hecho.success());
        let guardados = d.join("sonidos");
        std::fs::create_dir_all(&guardados).unwrap();
        let copia = importar_en(&guardados, &mp3).unwrap();
        let (muestras, _) = crate::encode::wav::leer(&std::fs::read(&copia).unwrap()).unwrap();
        let ms = muestras.len() * 1000 / 48_000;
        eprintln!("[s10] MP3 importado: {ms} ms");
        assert!((290..=320).contains(&ms), "el «la» de 300 ms, sin los 100 de silencio: {ms}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
