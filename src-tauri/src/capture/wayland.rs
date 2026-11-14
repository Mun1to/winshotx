//! Congelar la pantalla en Wayland, por el portal `Screenshot` de xdg-desktop-portal.
//!
//! En Wayland una app no puede leer la pantalla: lo que hace `xcap` en X11 (pedirle la
//! imagen al servidor) alli no existe, y por eso la captura fallaba con «Connection closed»
//! (medido por S20 en weston). La puerta que hay es el portal: se le pide una foto de todo
//! (`interactive=false`, sin el selector del escritorio) y devuelve un PNG en un archivo. La
//! primera vez GNOME pregunta si se deja a winshotx hacer capturas, y guarda la respuesta.
//!
//! Lo demas sale de GTK, que si sabe que pantallas hay: donde esta cada una y su escala. Con
//! eso se parte la foto entera en un trozo por pantalla, que es lo que espera el resto de
//! winshotx (`Freeze`). Las coordenadas son las de la foto, que son pixeles de verdad.
//!
//! **Una foto por captura, aunque se pida en dos tandas.** `windows_mgr` congela primero la
//! pantalla del raton y luego las demas (en Windows cada pantalla es una foto aparte); aqui
//! eso serian dos fotos del portal, cada una con su espera. La primera tanda guarda lo que
//! sobra y la segunda se lo lleva.

use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use parking_lot::Mutex;

use super::{Freeze, MonitorInfo};
use crate::error::{AppError, Result};

/// Una pantalla segun GTK, en pixeles logicos (los de las ventanas).
#[derive(Debug, Clone, PartialEq)]
pub struct PantallaGtk {
    pub nombre: String,
    pub x: i32,
    pub y: i32,
    pub ancho: i32,
    pub alto: i32,
    pub escala: i32,
    pub principal: bool,
}

pub fn pantallas() -> Result<Vec<PantallaGtk>> {
    crate::platform::xdg::en_el_hilo_de_gtk(|| {
        use gtk::gdk::prelude::*;
        let Some(pantalla) = gtk::gdk::Display::default() else {
            return Vec::new();
        };
        (0..pantalla.n_monitors())
            .filter_map(|i| pantalla.monitor(i))
            .enumerate()
            .map(|(i, m)| {
                let g = m.geometry();
                PantallaGtk {
                    nombre: m
                        .model()
                        .map(|n| n.to_string())
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| format!("Monitor {}", i + 1)),
                    x: g.x(),
                    y: g.y(),
                    ancho: g.width(),
                    alto: g.height(),
                    escala: m.scale_factor().max(1),
                    principal: m.is_primary(),
                }
            })
            .collect()
    })
}

/// Las pantallas, sin fotografiarlas. Sin la foto todavia no se sabe a cuantos pixeles de
/// verdad sale cada una, asi que se dan con la escala de GTK; `congelar` las da exactas.
pub fn monitores() -> Result<Vec<MonitorInfo>> {
    let pantallas = pantallas()?;
    if pantallas.is_empty() {
        return Err(AppError::Msg("GTK no ve ninguna pantalla".into()));
    }
    Ok(pantallas
        .iter()
        .enumerate()
        .map(|(i, p)| MonitorInfo {
            id: i as u32,
            label: p.nombre.clone(),
            x: p.x * p.escala,
            y: p.y * p.escala,
            width: (p.ancho * p.escala).max(1) as u32,
            height: (p.alto * p.escala).max(1) as u32,
            scale: p.escala as f32,
            is_primary: p.principal || (i == 0 && !pantallas.iter().any(|q| q.principal)),
        })
        .collect())
}

/// Donde cae cada pantalla dentro de la foto entera.
///
/// El portal fotografia el escritorio entero, que en GNOME y KDE va en pixeles logicos
/// multiplicados por la escala: el trozo de cada pantalla se saca con la proporcion entre
/// la foto y el rectangulo que cubren todas. Con escalas distintas en cada pantalla eso es
/// una aproximacion (NO PROBADO con pantallas de verdad).
pub fn repartir(pantallas: &[PantallaGtk], ancho_foto: u32, alto_foto: u32) -> Vec<MonitorInfo> {
    let Some(x0) = pantallas.iter().map(|p| p.x).min() else {
        return Vec::new();
    };
    let y0 = pantallas.iter().map(|p| p.y).min().unwrap_or(0);
    let x1 = pantallas.iter().map(|p| p.x + p.ancho).max().unwrap_or(x0 + 1);
    let y1 = pantallas.iter().map(|p| p.y + p.alto).max().unwrap_or(y0 + 1);
    let sx = f64::from(ancho_foto) / f64::from((x1 - x0).max(1));
    let sy = f64::from(alto_foto) / f64::from((y1 - y0).max(1));
    let ninguna_principal = !pantallas.iter().any(|p| p.principal);
    pantallas
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let x = (f64::from(p.x - x0) * sx).round() as i32;
            let y = (f64::from(p.y - y0) * sy).round() as i32;
            let ancho = ((f64::from(p.ancho) * sx).round() as i32)
                .min(ancho_foto as i32 - x)
                .max(1) as u32;
            let alto = ((f64::from(p.alto) * sy).round() as i32)
                .min(alto_foto as i32 - y)
                .max(1) as u32;
            MonitorInfo {
                id: i as u32,
                label: p.nombre.clone(),
                x,
                y,
                width: ancho,
                height: alto,
                scale: sx as f32,
                is_primary: p.principal || (i == 0 && ninguna_principal),
            }
        })
        .collect()
}

/// Lo que sobro de la ultima foto: la imagen entera, donde cae cada pantalla y cuales no se
/// han entregado todavia.
struct Sobrante {
    cuando: Instant,
    foto: Arc<RgbaImage>,
    monitores: Vec<MonitorInfo>,
    pendientes: Vec<usize>,
}

static SOBRANTE: Mutex<Option<Sobrante>> = Mutex::new(None);

/// Lo que puede tardar la segunda tanda en pedir lo suyo. Pasado esto, la foto es vieja.
const VIDA_DEL_SOBRANTE: Duration = Duration::from_secs(5);

pub fn congelar(indices: &[usize]) -> Result<Vec<Freeze>> {
    let (foto, monitores) = {
        let mut sobrante = SOBRANTE.lock();
        let vale = sobrante.as_ref().is_some_and(|s| {
            s.cuando.elapsed() < VIDA_DEL_SOBRANTE && indices.iter().all(|i| s.pendientes.contains(i))
        });
        if vale {
            let s = sobrante.as_mut().expect("comprobado justo arriba");
            s.pendientes.retain(|i| !indices.contains(i));
            let datos = (Arc::clone(&s.foto), s.monitores.clone());
            if s.pendientes.is_empty() {
                *sobrante = None;
            }
            datos
        } else {
            *sobrante = None;
            drop(sobrante);
            let pantallas = pantallas()?;
            let foto = Arc::new(foto_del_portal()?);
            let monitores = repartir(&pantallas, foto.width(), foto.height());
            let pendientes: Vec<usize> = (0..monitores.len()).filter(|i| !indices.contains(i)).collect();
            if !pendientes.is_empty() {
                *SOBRANTE.lock() = Some(Sobrante {
                    cuando: Instant::now(),
                    foto: Arc::clone(&foto),
                    monitores: monitores.clone(),
                    pendientes,
                });
            }
            (foto, monitores)
        }
    };

    indices
        .iter()
        .map(|&i| {
            let monitor = monitores
                .get(i)
                .cloned()
                .ok_or_else(|| AppError::Msg("una pantalla ha desaparecido".into()))?;
            let trozo = image::imageops::crop_imm(
                foto.as_ref(),
                monitor.x.max(0) as u32,
                monitor.y.max(0) as u32,
                monitor.width,
                monitor.height,
            )
            .to_image();
            let png = crate::encode::png::to_bytes(&trozo)?;
            Ok(Freeze {
                monitor,
                image: Arc::new(trozo),
                png: Arc::new(png),
            })
        })
        .collect()
}

/// La foto de todo el escritorio, por el portal.
///
/// El portal la deja en un archivo y devuelve su direccion. Ese archivo lo ha creado para
/// nosotros y no lo quiere nadie mas (GNOME lo deja en la carpeta de imagenes si no se
/// borra), asi que se borra en cuanto se ha leido, pero solo si es de ahora mismo: nunca se
/// borra algo que ya estuviera ahi.
fn foto_del_portal() -> Result<RgbaImage> {
    let inicio = Instant::now();
    let hilo = std::thread::spawn(|| {
        tauri::async_runtime::block_on(async {
            let respuesta = ashpd::desktop::screenshot::Screenshot::request()
                .interactive(false)
                .modal(false)
                .send()
                .await?
                .response()?;
            Ok::<_, ashpd::Error>(respuesta.uri().clone())
        })
    });
    let uri = hilo
        .join()
        .map_err(|_| AppError::Msg("el portal de capturas ha reventado".into()))?
        .map_err(|error| AppError::Msg(format!("el sistema no ha dejado hacer la foto: {error}")))?;
    let ruta = uri
        .to_file_path()
        .map_err(|_| AppError::Msg(format!("el portal ha devuelto algo que no es un archivo: {uri}")))?;
    let imagen = image::open(&ruta)
        .map_err(|error| AppError::Msg(format!("no se ha podido leer la foto del portal: {error}")))?
        .to_rgba8();
    let es_de_ahora = std::fs::metadata(&ruta)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|edad| edad < Duration::from_secs(60));
    if es_de_ahora {
        let _ = std::fs::remove_file(&ruta);
    }
    eprintln!(
        "[captura] foto del portal: {}x{} en {} ms ({})",
        imagen.width(),
        imagen.height(),
        inicio.elapsed().as_millis(),
        ruta.display()
    );
    Ok(imagen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pantalla(x: i32, y: i32, ancho: i32, alto: i32, escala: i32) -> PantallaGtk {
        PantallaGtk {
            nombre: "p".into(),
            x,
            y,
            ancho,
            alto,
            escala,
            principal: false,
        }
    }

    #[test]
    fn una_pantalla_es_la_foto_entera() {
        let m = repartir(&[pantalla(0, 0, 1920, 1080, 1)], 1920, 1080);
        assert_eq!((m[0].x, m[0].y, m[0].width, m[0].height), (0, 0, 1920, 1080));
        assert!(m[0].is_primary, "sin ninguna marcada, la primera hace de principal");
    }

    #[test]
    fn con_escala_dos_la_foto_es_el_doble_y_cada_trozo_tambien() {
        // GNOME al 200 %: la pantalla mide 1280x800 para las ventanas y la foto 2560x1600.
        let m = repartir(&[pantalla(0, 0, 1280, 800, 2)], 2560, 1600);
        assert_eq!((m[0].width, m[0].height), (2560, 1600));
        assert_eq!(m[0].scale, 2.0);
    }

    #[test]
    fn dos_pantallas_lado_a_lado_se_parten_por_donde_toca() {
        let m = repartir(
            &[pantalla(0, 0, 1920, 1080, 1), pantalla(1920, 0, 1280, 1024, 1)],
            3200,
            1080,
        );
        assert_eq!((m[0].x, m[0].width, m[0].height), (0, 1920, 1080));
        assert_eq!((m[1].x, m[1].width, m[1].height), (1920, 1280, 1024));
    }

    #[test]
    fn una_pantalla_a_la_izquierda_en_negativo_empieza_en_cero() {
        let m = repartir(
            &[pantalla(0, 0, 1920, 1080, 1), pantalla(-1080, -400, 1080, 1920, 1)],
            3000,
            1920,
        );
        assert_eq!((m[1].x, m[1].y), (0, 0));
        assert_eq!((m[0].x, m[0].y), (1080, 400));
    }

    #[test]
    fn ningun_trozo_se_sale_de_la_foto() {
        // Una foto que mide un pixel menos de lo que dicen las pantallas (redondeos).
        let m = repartir(
            &[pantalla(0, 0, 1920, 1080, 1), pantalla(1920, 0, 1920, 1080, 1)],
            3839,
            1079,
        );
        for monitor in &m {
            assert!(monitor.x as u32 + monitor.width <= 3839);
            assert!(monitor.y as u32 + monitor.height <= 1079);
        }
    }
}
