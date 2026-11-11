//! El historial de capturas y grabaciones, dentro de la app (E6 del plan de CleanShot X).
//!
//! Munir lo pidió con sus palabras: «que además tengas un historial dentro de la propia app
//! de vídeo y fotos». Hasta ahora solo estaban las últimas fotos en el menú de la bandeja.
//!
//! **El historial es la carpeta de capturas, no una lista aparte**, igual que las últimas
//! de la bandeja (`commands::ultimas_capturas`). Una lista guardada se desincroniza en
//! cuanto alguien borra o mueve un archivo por fuera, y habría que limpiarla. Lo que hay en
//! la carpeta ES el historial. Lo que solo se copió y nunca se guardó no sale, porque no
//! existe en ninguna parte.
//!
//! **Las miniaturas las da Windows**, las mismas que enseña el Explorador
//! (`IShellItemImageFactory`), y se guardan en el temporal. Así sale la de un MP4 sin
//! decodificar ni un fotograma aquí, suma cero bytes al instalador, y la ventana las enseña
//! por el protocolo `asset:`, que no llega a la carpeta de capturas del usuario.
//!
//! **Borrar manda a la papelera**, nunca borra del todo: un clic despistado en una lista de
//! cien no puede costar una grabación de una hora.

use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, Result};
use crate::state::AppState;

/// Una foto o un vídeo de la carpeta de capturas.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entrada {
    pub ruta: String,
    pub nombre: String,
    /// `foto` o `video`. Un GIF es un vídeo: es una grabación, aunque sea una imagen.
    pub tipo: &'static str,
    pub bytes: u64,
    /// Cuándo se guardó, en milisegundos desde 1970: la ventana lo escribe en su idioma.
    pub cuando_ms: u64,
}

/// Una página del historial y cuántas hay en total con ese filtro.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagina {
    pub entradas: Vec<Entrada>,
    pub total: usize,
}

/// De qué tipo es un archivo por su extensión, o nada si no es de los que guarda winshotx.
fn tipo_de(ruta: &Path) -> Option<&'static str> {
    let extension = ruta.extension()?.to_string_lossy().to_lowercase();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "webp" => Some("foto"),
        "mp4" | "gif" => Some("video"),
        _ => None,
    }
}

/// Todo lo de la carpeta, de lo más nuevo a lo más viejo, con el filtro puesto.
pub fn leer(carpeta: &Path, filtro: Option<&str>) -> Vec<Entrada> {
    let Ok(lista) = std::fs::read_dir(carpeta) else {
        // Que la carpeta no exista todavía no es un error: es que no se ha guardado nada.
        return Vec::new();
    };
    let mut entradas: Vec<Entrada> = lista
        .flatten()
        .filter_map(|e| {
            let ruta = e.path();
            let tipo = tipo_de(&ruta)?;
            if filtro.is_some_and(|f| f != tipo) {
                return None;
            }
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let cuando_ms = meta
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_millis() as u64;
            Some(Entrada {
                nombre: ruta.file_name()?.to_string_lossy().to_string(),
                ruta: ruta.to_string_lossy().to_string(),
                tipo,
                bytes: meta.len(),
                cuando_ms,
            })
        })
        .collect();
    entradas.sort_by(|a, b| b.cuando_ms.cmp(&a.cuando_ms).then_with(|| b.nombre.cmp(&a.nombre)));
    entradas
}

/// Comprueba que una ruta que manda la ventana es un archivo de la carpeta de capturas.
///
/// Lo mismo que con las ancladas (`dentro_de_las_ancladas`): sin esto, la orden de borrar
/// borraría cualquier archivo del disco que alguien le pidiera. Tiene que estar DIRECTAMENTE
/// en la carpeta (el historial no entra en subcarpetas), sin `..` por medio, y ser de un
/// tipo de los que guarda winshotx.
pub fn de_la_carpeta(ruta: &Path, carpeta: &Path) -> bool {
    !ruta.components().any(|c| matches!(c, Component::ParentDir))
        && ruta.parent() == Some(carpeta)
        && tipo_de(ruta).is_some()
}

fn carpeta(app: &AppHandle) -> PathBuf {
    PathBuf::from(app.state::<AppState>().settings.read().save_directory.clone())
}

fn comprobada(app: &AppHandle, ruta: &str) -> Result<PathBuf> {
    let ruta = PathBuf::from(ruta);
    if !de_la_carpeta(&ruta, &carpeta(app)) {
        return Err(AppError::Msg("ese archivo no es del historial".into()));
    }
    Ok(ruta)
}

/// El nombre de la miniatura en el temporal: cambia si cambia el archivo, para que una
/// captura sobrescrita no enseñe la imagen de antes.
fn nombre_de_miniatura(ruta: &Path, bytes: u64, cuando_ms: u64) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ruta.hash(&mut h);
    bytes.hash(&mut h);
    cuando_ms.hash(&mut h);
    format!("{:016x}.png", h.finish())
}

/// El lado de las miniaturas: a doble resolución de lo que miden en la ventana.
const LADO: i32 = 320;

/// La miniatura que da Windows para ese archivo, la misma del Explorador.
#[cfg(windows)]
fn miniatura_de_windows(ruta: &Path) -> Result<image::RgbaImage> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_RESIZETOFIT, SIIGBF_THUMBNAILONLY,
    };

    let ancho: Vec<u16> = ruta.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let fallo = |e: windows::core::Error| AppError::Msg(format!("Windows no da miniatura: {e}"));
    unsafe {
        let fabrica: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(ancho.as_ptr()), None).map_err(fallo)?;
        // Solo la miniatura de verdad, nunca el icono del tipo de archivo: un icono de «MP4»
        // en una rejilla de vídeos no dice cuál es cuál, mejor el hueco con su nombre.
        let mapa = fabrica
            .GetImage(SIZE { cx: LADO, cy: LADO }, SIIGBF_THUMBNAILONLY | SIIGBF_RESIZETOFIT)
            .map_err(fallo)?;
        let mut info = BITMAP::default();
        GetObjectW(
            HGDIOBJ(mapa.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&raw mut info).cast()),
        );
        let (w, h) = (info.bmWidth.max(1), info.bmHeight.abs().max(1));
        let mut cabecera = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                // Negativo: las filas de arriba abajo, como las quiere `image`.
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let dc = CreateCompatibleDC(None);
        let filas = GetDIBits(
            dc,
            mapa,
            0,
            h as u32,
            Some(bgra.as_mut_ptr().cast()),
            &mut cabecera,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        let _ = DeleteObject(HGDIOBJ(mapa.0));
        if filas == 0 {
            return Err(AppError::Msg("Windows no ha dado los píxeles de la miniatura".into()));
        }
        // BGRA a RGBA, y opaca: una miniatura con el alfa a cero (las de vídeo lo traen
        // así) se vería transparente sobre el fondo de la ventana.
        let opaca = bgra.chunks_exact(4).all(|p| p[3] == 0);
        for p in bgra.chunks_exact_mut(4) {
            p.swap(0, 2);
            if opaca {
                p[3] = 255;
            }
        }
        image::RgbaImage::from_raw(w as u32, h as u32, bgra)
            .ok_or_else(|| AppError::Msg("la miniatura no cuadra".into()))
    }
}

#[cfg(not(windows))]
fn miniatura_de_windows(_ruta: &Path) -> Result<image::RgbaImage> {
    Err(AppError::Unsupported)
}

/// La miniatura, sacada de Windows o, si es una foto y Windows no la da, de la propia foto.
fn hacer_miniatura(ruta: &Path, destino: &Path) -> Result<()> {
    let imagen = match miniatura_de_windows(ruta) {
        Ok(imagen) => imagen,
        Err(_) if tipo_de(ruta) == Some("foto") => image::open(ruta)?.to_rgba8(),
        Err(e) => return Err(e),
    };
    // Windows puede devolverla más grande de lo pedido (la que ya tenga en su caché), y una
    // de 1.000 píxeles en una rejilla de cien pesaría diez veces lo que hace falta.
    let (w, h) = encajada(imagen.width(), imagen.height());
    let imagen = if (w, h) == imagen.dimensions() {
        imagen
    } else {
        image::imageops::thumbnail(&imagen, w, h)
    };
    crate::encode::png::save_fast(&imagen, destino)
}

/// Lo que mide una imagen metida en un cuadrado de `LADO`, sin deformarla ni agrandarla.
/// `thumbnail` estira a lo que se le pida: las proporciones se calculan aquí.
fn encajada(ancho: u32, alto: u32) -> (u32, u32) {
    let lado = LADO as f32;
    let escala = (lado / ancho.max(1) as f32).min(lado / alto.max(1) as f32).min(1.0);
    (
        ((ancho as f32 * escala).round() as u32).max(1),
        ((alto as f32 * escala).round() as u32).max(1),
    )
}

/// Lo que pide la ventana: una página del historial.
#[tauri::command]
pub async fn historial_lista(
    app: AppHandle,
    filtro: Option<String>,
    desde: usize,
    cuantas: usize,
) -> Pagina {
    let todas = leer(&carpeta(&app), filtro.as_deref());
    let total = todas.len();
    Pagina {
        entradas: todas.into_iter().skip(desde).take(cuantas.clamp(1, 200)).collect(),
        total,
    }
}

/// La ruta de la miniatura de un archivo del historial, en el temporal. La primera vez se
/// pide a Windows; las siguientes ya está.
#[tauri::command]
pub async fn historial_miniatura(app: AppHandle, ruta: String) -> Result<String> {
    let ruta = comprobada(&app, &ruta)?;
    let meta = std::fs::metadata(&ruta)?;
    let cuando = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let dir = app.state::<AppState>().temp_root.join("historial");
    std::fs::create_dir_all(&dir)?;
    let destino = dir.join(nombre_de_miniatura(&ruta, meta.len(), cuando));
    if !destino.exists() {
        let para_el_hilo = destino.clone();
        // En un hilo con COM de un solo apartamento, que es lo que piden los extractores de
        // miniaturas del shell (algunos fallan en uno de varios apartamentos).
        tauri::async_runtime::spawn_blocking(move || {
            #[cfg(windows)]
            unsafe {
                use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
                let iniciado = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
                let hecho = hacer_miniatura(&ruta, &para_el_hilo);
                if iniciado {
                    CoUninitialize();
                }
                hecho
            }
            #[cfg(not(windows))]
            hacer_miniatura(&ruta, &para_el_hilo)
        })
        .await
        .map_err(|e| AppError::Msg(e.to_string()))??;
    }
    Ok(destino.to_string_lossy().to_string())
}

/// La abre con el programa que tenga Windows para ese tipo: el visor de fotos o el
/// reproductor de vídeo.
#[tauri::command]
pub async fn historial_abrir(app: AppHandle, ruta: String) -> Result<()> {
    let ruta = comprobada(&app, &ruta)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .creation_flags(0x0800_0000)
            .arg(&ruta)
            .spawn()?;
    }
    #[cfg(not(windows))]
    let _ = ruta;
    Ok(())
}

/// La enseña en su carpeta, seleccionada.
#[tauri::command]
pub async fn historial_mostrar(app: AppHandle, ruta: String) -> Result<()> {
    let ruta = comprobada(&app, &ruta)?;
    crate::platform::reveal(&ruta)
}

/// La copia para pegarla en otro sitio: una foto, como imagen (se pega en un chat o en un
/// documento); un vídeo, como archivo (se pega en un chat o en una carpeta).
#[tauri::command]
pub async fn historial_copiar(app: AppHandle, ruta: String) -> Result<()> {
    let ruta = comprobada(&app, &ruta)?;
    if tipo_de(&ruta) == Some("foto") {
        let imagen = image::open(&ruta)?.to_rgba8();
        let bytes = crate::encode::png::to_bytes(&imagen)?;
        crate::platform::clipboard::copy_image(&imagen, &bytes)
    } else {
        crate::platform::clipboard::copy_files(&[ruta.as_path()])
    }
}

/// La manda a la papelera de Windows, de donde se puede recuperar.
#[tauri::command]
pub async fn historial_borrar(app: AppHandle, ruta: String) -> Result<()> {
    let ruta = comprobada(&app, &ruta)?;
    a_la_papelera(&ruta)
}

/// Manda un archivo a la papelera, sin preguntar ni enseñar nada (la ventana ya pidió la
/// confirmación a su manera).
#[cfg(windows)]
pub fn a_la_papelera(ruta: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE,
        SHFILEOPSTRUCTW,
    };
    // La lista de rutas de esta función acaba en DOS ceros: con uno solo, Windows sigue
    // leyendo memoria como si hubiera más archivos detrás.
    let doble: Vec<u16> = ruta
        .as_os_str()
        .encode_wide()
        .chain([0, 0])
        .collect();
    let mut operacion = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(doble.as_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT).0 as u16,
        ..Default::default()
    };
    let codigo = unsafe { SHFileOperationW(&mut operacion) };
    if codigo != 0 || operacion.fAnyOperationsAborted.as_bool() || ruta.exists() {
        return Err(AppError::Msg(format!("no se ha podido mandar a la papelera ({codigo})")));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn a_la_papelera(_ruta: &Path) -> Result<()> {
    Err(AppError::Unsupported)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("winshotx-historial-{nombre}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tocar(ruta: &Path, segundos_atras: u64) {
        std::fs::write(ruta, b"x").unwrap();
        let cuando = std::time::SystemTime::now() - std::time::Duration::from_secs(segundos_atras);
        std::fs::File::options()
            .write(true)
            .open(ruta)
            .unwrap()
            .set_modified(cuando)
            .unwrap();
    }

    #[test]
    fn salen_las_fotos_y_los_videos_de_lo_mas_nuevo_a_lo_mas_viejo() {
        let dir = carpeta_de_prueba("orden");
        tocar(&dir.join("vieja.png"), 300);
        tocar(&dir.join("video.mp4"), 100);
        tocar(&dir.join("nueva.jpg"), 10);
        tocar(&dir.join("animada.gif"), 200);
        tocar(&dir.join("notas.txt"), 5);
        std::fs::create_dir_all(dir.join("subcarpeta.png")).unwrap();
        let nombres: Vec<String> = leer(&dir, None).into_iter().map(|e| e.nombre).collect();
        assert_eq!(nombres, ["nueva.jpg", "video.mp4", "animada.gif", "vieja.png"]);
    }

    #[test]
    fn el_filtro_separa_fotos_de_videos_y_un_gif_es_un_video() {
        let dir = carpeta_de_prueba("filtro");
        tocar(&dir.join("a.png"), 10);
        tocar(&dir.join("b.mp4"), 20);
        tocar(&dir.join("c.gif"), 30);
        let fotos: Vec<String> = leer(&dir, Some("foto")).into_iter().map(|e| e.nombre).collect();
        let videos: Vec<String> = leer(&dir, Some("video")).into_iter().map(|e| e.nombre).collect();
        assert_eq!(fotos, ["a.png"]);
        assert_eq!(videos, ["b.mp4", "c.gif"]);
    }

    #[test]
    fn una_carpeta_que_no_existe_es_un_historial_vacio() {
        assert!(leer(Path::new(r"C:\no\existe\winshotx"), None).is_empty());
    }

    /// Las órdenes solo tocan archivos del historial: nunca uno de otra carpeta, ni uno que
    /// se sale con `..`, ni uno de una subcarpeta, ni otro tipo de archivo de la misma.
    #[test]
    fn las_ordenes_solo_aceptan_archivos_del_historial() {
        let carpeta = Path::new(r"C:\Users\a\Pictures\winshotx");
        assert!(de_la_carpeta(&carpeta.join("winshotx-1.png"), carpeta));
        assert!(de_la_carpeta(&carpeta.join("winshotx-1.mp4"), carpeta));
        assert!(!de_la_carpeta(Path::new(r"C:\Users\a\Documents\privado.png"), carpeta));
        assert!(!de_la_carpeta(&carpeta.join(r"..\..\Documents\privado.png"), carpeta));
        assert!(!de_la_carpeta(&carpeta.join(r"dentro\winshotx-1.png"), carpeta));
        assert!(!de_la_carpeta(&carpeta.join("desktop.ini"), carpeta));
        assert!(!de_la_carpeta(Path::new(r"C:\Users\a\Pictures\winshotx-viejos\a.png"), carpeta));
    }

    /// Borrar manda a la papelera: el archivo se va de la carpeta y la orden no falla.
    ///
    /// Apagada de serie porque deja un archivo de un byte en la papelera de quien la corre:
    /// `cargo test --lib borrar_lo_saca -- --ignored`.
    #[test]
    #[ignore = "deja un archivo de un byte en la papelera de quien la corre"]
    fn borrar_lo_saca_de_la_carpeta() {
        let dir = carpeta_de_prueba("papelera");
        let ruta = dir.join("winshotx-para-la-papelera.png");
        tocar(&ruta, 1);
        a_la_papelera(&ruta).unwrap();
        assert!(!ruta.exists());
    }

    /// La miniatura de una foto de verdad, por el camino que pide la ventana: o la da
    /// Windows, o se saca de la foto. En los dos casos sale un PNG pequeño y no vacío.
    #[test]
    fn la_miniatura_de_una_foto_sale_pequena() {
        let dir = carpeta_de_prueba("miniatura");
        let foto = dir.join("winshotx-grande.png");
        image::RgbaImage::from_fn(1600, 900, |x, _| image::Rgba([(x % 255) as u8, 40, 200, 255]))
            .save(&foto)
            .unwrap();
        let destino = dir.join("mini.png");
        // Con COM puesto, como en la orden: así se prueba el camino de Windows.
        #[cfg(windows)]
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        hacer_miniatura(&foto, &destino).unwrap();
        let (w, h) = image::image_dimensions(&destino).unwrap();
        // 1600 x 900 metida en 320: 320 x 180, sin deformar.
        assert_eq!((w, h), (320, 180));
    }

    /// La miniatura de un vídeo la da Windows, como en el Explorador: un MP4 escrito con el
    /// codificador de la app, todo de un color, sale de ese color.
    #[cfg(windows)]
    #[test]
    fn la_miniatura_de_un_video_la_da_windows() {
        let dir = carpeta_de_prueba("video");
        let video = dir.join("winshotx-video.mp4");
        let cuadro = image::RgbaImage::from_pixel(320, 240, image::Rgba([230, 120, 20, 255]));
        let indices: Vec<usize> = (0..15).collect();
        let retardos = vec![66u32; 15];
        let mut cargar = |_: usize| Ok(cuadro.clone());
        crate::encode::mp4::encode(
            &indices,
            &retardos,
            &mut cargar,
            &video,
            &crate::encode::mp4::Mp4Options { width: 320, height: 240, fps: 15, quality: 80 },
            None,
            |_, _, _| {},
        )
        .unwrap();
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
        };
        let destino = dir.join("mini.png");
        hacer_miniatura(&video, &destino).expect("Windows tenía que dar la miniatura del MP4");
        let mini = image::open(&destino).unwrap().to_rgba8();
        let centro = mini.get_pixel(mini.width() / 2, mini.height() / 2).0;
        assert!(
            centro[0] > 180 && centro[1] > 80 && centro[1] < 160 && centro[2] < 70,
            "el centro de la miniatura tenía que ser naranja y es {centro:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Un archivo sobrescrito cambia de miniatura: si no, el historial enseñaría la vieja.
    #[test]
    fn el_nombre_de_la_miniatura_cambia_con_el_archivo() {
        let ruta = Path::new(r"C:\x\a.png");
        assert_ne!(nombre_de_miniatura(ruta, 10, 1), nombre_de_miniatura(ruta, 10, 2));
        assert_ne!(nombre_de_miniatura(ruta, 10, 1), nombre_de_miniatura(ruta, 11, 1));
        assert_eq!(nombre_de_miniatura(ruta, 10, 1), nombre_de_miniatura(ruta, 10, 1));
    }
}
