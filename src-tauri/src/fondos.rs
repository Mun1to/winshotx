//! Las órdenes de los fondos propios del estudio (S3): elegir un archivo, traerse una imagen,
//! el fondo de escritorio, e importar un vídeo o un GIF con su progreso y su botón de cancelar.
//!
//! **Todo lo elegido se copia a la carpeta de la app** (`<datos>/winshotx/fondos`), por dos
//! motivos: la vista previa del editor solo puede enseñar archivos de ahí (el permiso del
//! protocolo `asset` de `tauri.conf.json`), y exportar no puede depender de que el usuario
//! no mueva su foto del escritorio. Lo importado se reconoce por la ruta, el tamaño y la
//! hora del original: elegir otra vez el mismo vídeo no lo vuelve a convertir.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::encode::fondo_video::{self, Importado};
use crate::error::{AppError, Result};
use crate::platform::escritorio::{self, FondoDeEscritorio};

/// El progreso de importar un fondo animado, de 0 a 1.
pub const EVENT_PROGRESO: &str = "winshotx://fondo-progreso";

/// Cuántos fondos de cada clase se guardan. Al importar uno nuevo se borran los más viejos:
/// un fondo de vídeo son decenas de megas, y nadie usa veinte.
const VIDEOS_GUARDADOS: usize = 8;
const IMAGENES_GUARDADAS: usize = 16;
/// El lado más largo de una imagen de fondo guardada: más no se ve en un vídeo 4K.
const IMAGEN_MAXIMA: (u32, u32) = (3840, 2160);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progreso {
    hecho: f32,
}

/// Lo que tiene puesto el escritorio, para la vista previa: la imagen ya copiada a la
/// carpeta de la app, o el color liso.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Escritorio {
    Imagen { vista: String },
    Color { rgb: [u8; 3] },
}

fn carpeta(app: &AppHandle) -> Result<PathBuf> {
    // `app_data_dir` es lo mismo que `$APPDATA` en el permiso del protocolo `asset`.
    let carpeta = app.path().app_data_dir()?.join("winshotx").join("fondos");
    std::fs::create_dir_all(&carpeta)?;
    Ok(carpeta)
}

/// Un nombre que solo depende del original: su ruta, cuánto pesa y cuándo se escribió.
pub(crate) fn huella(origen: &Path) -> Result<String> {
    let datos = std::fs::metadata(origen)
        .map_err(|_| AppError::Msg(format!("no encuentro {}", origen.display())))?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    origen.to_string_lossy().to_lowercase().hash(&mut h);
    datos.len().hash(&mut h);
    datos.modified().ok().hash(&mut h);
    Ok(format!("{:016x}", h.finish()))
}

/// Deja como mucho `guardar` archivos que empiecen por `prefijo` (y su vista y su ficha),
/// borrando los más viejos.
pub(crate) fn podar(carpeta: &Path, prefijo: &str, extension: &str, guardar: usize) {
    let Ok(entradas) = std::fs::read_dir(carpeta) else { return };
    let mut hay: Vec<(std::time::SystemTime, PathBuf)> = entradas
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let nombre = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            nombre.starts_with(prefijo) && nombre.ends_with(extension) && !nombre.contains(".parcial.")
        })
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    hay.sort_by_key(|(hora, _)| std::cmp::Reverse(*hora));
    for (_, ruta) in hay.into_iter().skip(guardar) {
        let _ = std::fs::remove_file(&ruta);
        for otra in ["jpg", "json"] {
            if extension != format!(".{otra}") {
                let _ = std::fs::remove_file(ruta.with_extension(otra));
            }
        }
    }
}

/// Le pone la hora de ahora, para que `podar` sepa que se ha vuelto a usar.
pub(crate) fn tocar(ruta: &Path) {
    if let Ok(archivo) = std::fs::File::options().write(true).open(ruta) {
        let _ = archivo.set_modified(std::time::SystemTime::now());
    }
}

/// Lee una imagen, la reduce si pasa de 4K, le quita la transparencia sobre blanco y la
/// guarda en JPEG en `destino`.
fn guardar_imagen(origen: &Path, destino: &Path) -> Result<()> {
    let imagen = image::ImageReader::open(origen)?
        .with_guessed_format()?
        .decode()
        .map_err(|e| AppError::Msg(format!("esa imagen no se puede abrir ({e}). Prueba con un PNG o un JPG.")))?
        .to_rgba8();
    let (ancho, alto) = imagen.dimensions();
    let escala = (f64::from(IMAGEN_MAXIMA.0) / f64::from(ancho.max(1)))
        .min(f64::from(IMAGEN_MAXIMA.1) / f64::from(alto.max(1)))
        .min(1.0);
    let (nuevo_ancho, nuevo_alto) = (
        ((f64::from(ancho) * escala).round() as u32).max(1),
        ((f64::from(alto) * escala).round() as u32).max(1),
    );
    let mut imagen = crate::encode::escalar::a_medida(&imagen, nuevo_ancho, nuevo_alto);
    for p in imagen.pixels_mut() {
        let a = u32::from(p.0[3]);
        if a < 255 {
            for c in 0..3 {
                p.0[c] = ((u32::from(p.0[c]) * a + 255 * (255 - a)) / 255) as u8;
            }
            p.0[3] = 255;
        }
    }
    crate::encode::jpg::save(&imagen, destino, nuevo_ancho, nuevo_alto, 90)
}

/// El diálogo de abrir de Windows, con los formatos de esa clase de fondo: `imagen` o
/// `video`. Devuelve la ruta, o nada si se cancela.
#[tauri::command]
pub async fn elegir_archivo_de_fondo(app: AppHandle, tipo: String) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let dialogo = app.dialog().file();
    let dialogo = if tipo == "video" {
        dialogo.add_filter("Vídeo o GIF", &["mp4", "mov", "m4v", "mkv", "webm", "wmv", "avi", "gif"])
    } else {
        dialogo.add_filter("Imagen", &["png", "jpg", "jpeg", "bmp"])
    };
    dialogo.blocking_pick_file()?.into_path().ok().map(|p| p.to_string_lossy().to_string())
}

/// Copia una imagen del usuario a la carpeta de fondos y devuelve la ruta de la copia, que es
/// la que se pone en `edicion.fondo`.
#[tauri::command]
pub async fn importar_imagen_de_fondo(app: AppHandle, origen: String) -> Result<String> {
    let origen = PathBuf::from(origen);
    let destino = carpeta(&app)?.join(format!("imagen-{}.jpg", huella(&origen)?));
    if destino.is_file() {
        tocar(&destino);
    } else {
        let carpeta = destino.parent().map(Path::to_path_buf);
        let (o, d) = (origen.clone(), destino.clone());
        tauri::async_runtime::spawn_blocking(move || guardar_imagen(&o, &d))
            .await
            .map_err(|e| AppError::Msg(e.to_string()))??;
        if let Some(carpeta) = carpeta {
            podar(&carpeta, "imagen-", ".jpg", IMAGENES_GUARDADAS);
        }
    }
    Ok(destino.to_string_lossy().to_string())
}

/// Lo que tiene puesto ahora el escritorio de Windows, para enseñarlo en la vista previa.
/// Al exportar se vuelve a preguntar: si cambia entre medias, sale el de ese momento.
#[tauri::command]
pub async fn fondo_escritorio(app: AppHandle) -> Result<Escritorio> {
    match escritorio::actual() {
        Some(FondoDeEscritorio::Imagen(ruta)) => {
            let destino = carpeta(&app)?.join(format!("escritorio-{}.jpg", huella(&ruta)?));
            if !destino.is_file() {
                let d = destino.clone();
                tauri::async_runtime::spawn_blocking(move || guardar_imagen(&ruta, &d))
                    .await
                    .map_err(|e| AppError::Msg(e.to_string()))??;
                if let Some(carpeta) = destino.parent() {
                    podar(carpeta, "escritorio-", ".jpg", 2);
                }
            }
            Ok(Escritorio::Imagen { vista: destino.to_string_lossy().to_string() })
        }
        Some(FondoDeEscritorio::Color(rgb)) => Ok(Escritorio::Color { rgb }),
        None => Err(AppError::Msg("Windows no dice qué fondo tiene el escritorio".into())),
    }
}

/// La importación en marcha, para poder cancelarla. Una sola a la vez: empezar otra cancela
/// la de antes.
static EN_MARCHA: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Importa un vídeo o un GIF como fondo animado. Avisa de su progreso con `EVENT_PROGRESO`, y
/// `cancelar_fondo_animado` lo para sin dejar nada a medias en el disco.
#[tauri::command]
pub async fn importar_fondo_animado(app: AppHandle, origen: String) -> Result<Importado> {
    let origen = PathBuf::from(origen);
    let carpeta = carpeta(&app)?;
    let destino = carpeta.join(format!("video-{}.mp4", huella(&origen)?));
    let ficha = destino.with_extension("json");
    // Ya importado antes: su ficha dice lo que mide y lo que dura.
    if destino.is_file() {
        if let Some(hecho) = std::fs::read_to_string(&ficha)
            .ok()
            .and_then(|t| serde_json::from_str::<Importado>(&t).ok())
            .filter(|h| Path::new(&h.vista).is_file())
        {
            tocar(&destino);
            let _ = app.emit(EVENT_PROGRESO, Progreso { hecho: 1.0 });
            return Ok(hecho);
        }
    }
    let cancelar = Arc::new(AtomicBool::new(false));
    if let Ok(mut en_marcha) = EN_MARCHA.lock() {
        if let Some(antes) = en_marcha.replace(cancelar.clone()) {
            antes.store(true, Ordering::Relaxed);
        }
    }
    let avisar = app.clone();
    let parar = cancelar.clone();
    let hecho = tauri::async_runtime::spawn_blocking(move || {
        let mut ultimo = -1.0f32;
        fondo_video::importar(&origen, &destino, &parar, &mut |hecho| {
            // Un aviso por cada punto: más, y la ventana se pasa el rato repintando la barra.
            if hecho - ultimo >= 0.01 || hecho >= 1.0 {
                ultimo = hecho;
                let _ = avisar.emit(EVENT_PROGRESO, Progreso { hecho });
            }
        })
    })
    .await
    .map_err(|e| AppError::Msg(e.to_string()))?;
    if let Ok(mut en_marcha) = EN_MARCHA.lock() {
        if en_marcha.as_ref().is_some_and(|a| Arc::ptr_eq(a, &cancelar)) {
            *en_marcha = None;
        }
    }
    let hecho = hecho?;
    let _ = std::fs::write(&ficha, serde_json::to_string(&hecho)?);
    podar(&carpeta, "video-", ".mp4", VIDEOS_GUARDADOS);
    Ok(hecho)
}

/// Para la importación en marcha. Contesta enseguida; la importación acaba con el error
/// `fondo_video::CANCELADO`, que el editor no enseña como un fallo.
#[tauri::command]
pub fn cancelar_fondo_animado() {
    if let Ok(en_marcha) = EN_MARCHA.lock() {
        if let Some(cancelar) = en_marcha.as_ref() {
            cancelar.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn dir(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("winshotx-fondos-cmd-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn la_huella_cambia_con_el_archivo_y_no_con_quien_la_pida() {
        let dir = dir("huella");
        let a = dir.join("a.png");
        std::fs::write(&a, b"uno").unwrap();
        let primera = huella(&a).unwrap();
        assert_eq!(huella(&a).unwrap(), primera, "el mismo archivo, la misma huella");
        std::fs::write(&a, b"otro contenido").unwrap();
        assert_ne!(huella(&a).unwrap(), primera, "otro contenido, otra huella");
        assert!(huella(&dir.join("no-existe.png")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Una imagen enorme y con transparencia se guarda a 4K como mucho y sin agujeros.
    #[test]
    fn una_imagen_se_guarda_reducida_y_sin_transparencia() {
        let dir = dir("guardar");
        let origen = dir.join("grande.png");
        image::RgbaImage::from_pixel(5000, 1000, image::Rgba([0, 0, 0, 0])).save(&origen).unwrap();
        let destino = dir.join("guardada.jpg");
        guardar_imagen(&origen, &destino).unwrap();
        let leida = image::open(&destino).unwrap().to_rgba8();
        assert_eq!(leida.dimensions(), (3840, 768));
        assert!(leida.get_pixel(10, 10).0[0] > 240, "lo transparente tiene que salir blanco");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn podar_deja_los_mas_nuevos_con_su_vista_y_su_ficha() {
        let dir = dir("podar");
        for i in 0..4 {
            let video = dir.join(format!("video-{i}.mp4"));
            std::fs::write(&video, b"v").unwrap();
            std::fs::write(video.with_extension("jpg"), b"j").unwrap();
            std::fs::write(video.with_extension("json"), b"{}").unwrap();
            let hora = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000 + i);
            std::fs::File::options().write(true).open(&video).unwrap().set_modified(hora).unwrap();
        }
        podar(&dir, "video-", ".mp4", 2);
        let mut quedan: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        quedan.sort();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            quedan,
            ["video-2.jpg", "video-2.json", "video-2.mp4", "video-3.jpg", "video-3.json", "video-3.mp4"]
        );
    }
}
