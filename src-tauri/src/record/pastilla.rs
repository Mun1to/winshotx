//! Dibujar el nombre de un atajo con la fuente del sistema, para pegarlo en el vídeo.
//!
//! El texto va con GDI porque es lo único a mano que sabe de letras: `image` pinta
//! rectángulos, no tipografía, y traerse un motor de fuentes entero para escribir
//! «Ctrl + C» pesaría más que toda la aplicación.
//!
//! **La pastilla se guarda de un fotograma para otro.** Dibujarla cuesta abrir un contexto
//! de dispositivo, elegir la fuente y medir el texto, y eso treinta veces por segundo
//! durante una grabación larga se nota. Como el texto cambia cuando alguien pulsa un atajo
//! y no en cada fotograma, se guarda la última y se reusa mientras diga lo mismo.

#![cfg(windows)]

use image::{Rgba, RgbaImage};

/// Alto de la letra, en píxeles.
const LETRA: i32 = 22;
/// Aire dentro de la pastilla.
const AIRE_X: i32 = 16;
const AIRE_Y: i32 = 10;

/// La última pastilla dibujada, para no repetir el trabajo en cada fotograma.
#[derive(Default)]
pub struct Cache {
    texto: String,
    imagen: Option<RgbaImage>,
}

impl Cache {
    /// La pastilla de ese texto, dibujándola solo si ha cambiado.
    pub fn pastilla(&mut self, texto: &str) -> Option<&RgbaImage> {
        if self.texto != texto || self.imagen.is_none() {
            self.imagen = dibujar(texto);
            self.texto = texto.to_string();
        }
        self.imagen.as_ref()
    }
}

/// Pega la pastilla abajo y centrada, con la transparencia que se le diga.
pub fn pegar(fondo: &mut RgbaImage, pastilla: &RgbaImage, opacidad: f32) {
    if opacidad <= 0.0 {
        return;
    }
    let (ancho, alto) = fondo.dimensions();
    let (pa, pb) = pastilla.dimensions();
    if pa >= ancho || pb >= alto {
        return;
    }
    let x0 = (ancho - pa) / 2;
    // A un octavo del alto desde abajo, que es donde no tapa lo que se está enseñando.
    let y0 = alto.saturating_sub(pb + alto / 8);

    for y in 0..pb {
        for x in 0..pa {
            let arriba = pastilla.get_pixel(x, y);
            let alfa = arriba.0[3] as f32 / 255.0 * opacidad;
            if alfa <= 0.0 {
                continue;
            }
            let destino = fondo.get_pixel_mut(x0 + x, y0 + y);
            for i in 0..3 {
                destino.0[i] =
                    (destino.0[i] as f32 * (1.0 - alfa) + arriba.0[i] as f32 * alfa).round() as u8;
            }
        }
    }
}

/// Lo transparente que está la pastilla según lo que lleve puesta.
///
/// Entera casi todo el rato y apagándose solo al final: una pastilla que empieza a
/// desvanecerse enseguida se lee peor que una que se queda quieta y luego se va.
pub fn opacidad(edad_ms: u64, duracion_ms: u64) -> f32 {
    if edad_ms >= duracion_ms || duracion_ms == 0 {
        return 0.0;
    }
    let avance = edad_ms as f32 / duracion_ms as f32;
    if avance < 0.75 {
        1.0
    } else {
        1.0 - (avance - 0.75) / 0.25
    }
}

/// Dibuja la pastilla: fondo oscuro redondeado y el texto en blanco encima.
fn dibujar(texto: &str) -> Option<RgbaImage> {
    use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
    use windows::Win32::Graphics::Gdi::*;
    use windows::core::HSTRING;

    if texto.is_empty() {
        return None;
    }
    unsafe {
        let dc = CreateCompatibleDC(None);
        let fuente = CreateFontW(
            LETRA,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            &HSTRING::from("Segoe UI"),
        );
        let fuente_vieja = SelectObject(dc, fuente.into());

        // Medir primero: la pastilla mide lo que mide su texto, ni más ni menos.
        let utf16: Vec<u16> = texto.encode_utf16().collect();
        let mut medida = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, &utf16, &mut medida);
        let ancho = (medida.cx + AIRE_X * 2).max(1);
        let alto = (medida.cy + AIRE_Y * 2).max(1);

        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = ancho;
        // Negativo: filas de arriba abajo, que es como las quiere `image`.
        info.bmiHeader.biHeight = -alto;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;

        let mut pixeles: *mut std::ffi::c_void = std::ptr::null_mut();
        let mapa = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut pixeles, None, 0).ok()?;
        let mapa_viejo = SelectObject(dc, mapa.into());

        // GDI no sabe de transparencia: se pinta el fondo de un gris muy oscuro y el texto
        // en blanco, y al copiar los píxeles se decide cuál es letra y cuál es fondo.
        let pincel = CreateSolidBrush(COLORREF(0x0011_1111));
        let todo = RECT {
            left: 0,
            top: 0,
            right: ancho,
            bottom: alto,
        };
        FillRect(dc, &todo, pincel);
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x00FF_FFFF));
        let _ = TextOutW(dc, AIRE_X, AIRE_Y, &utf16);

        let total = (ancho * alto) as usize;
        let crudo = std::slice::from_raw_parts(pixeles as *const u8, total * 4);
        let mut salida = RgbaImage::new(ancho as u32, alto as u32);
        let radio = (alto / 3).max(4);
        for (i, pixel) in salida.pixels_mut().enumerate() {
            let x = (i as i32) % ancho;
            let y = (i as i32) / ancho;
            let claro = crudo[i * 4 + 2].max(crudo[i * 4 + 1]).max(crudo[i * 4]);
            if !dentro_de_la_pastilla(x, y, ancho, alto, radio) {
                *pixel = Rgba([0, 0, 0, 0]);
            } else if claro > 0x33 {
                // Letra. El suavizado de la fuente deja grises en los bordes, y ese gris
                // se convierte en lo transparente que va la letra ahí.
                *pixel = Rgba([255, 255, 255, claro]);
            } else {
                // Fondo de la pastilla: oscuro y casi opaco, para que se lea encima de lo
                // que haya en la pantalla.
                *pixel = Rgba([16, 16, 20, 225]);
            }
        }

        SelectObject(dc, fuente_vieja);
        SelectObject(dc, mapa_viejo);
        let _ = DeleteObject(fuente.into());
        let _ = DeleteObject(mapa.into());
        let _ = DeleteObject(pincel.into());
        let _ = DeleteDC(dc);
        Some(salida)
    }
}

/// Si el punto cae dentro del rectángulo con las esquinas redondeadas.
fn dentro_de_la_pastilla(x: i32, y: i32, ancho: i32, alto: i32, radio: i32) -> bool {
    let cx = if x < radio {
        radio
    } else if x >= ancho - radio {
        ancho - radio - 1
    } else {
        return true;
    };
    let cy = if y < radio {
        radio
    } else if y >= alto - radio {
        alto - radio - 1
    } else {
        return true;
    };
    let (dx, dy) = ((x - cx) as f32, (y - cy) as f32);
    (dx * dx + dy * dy).sqrt() <= radio as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_pastilla_mide_lo_que_mide_su_texto() {
        let corta = dibujar("Ctrl + C").expect("dibujar la corta");
        let larga = dibujar("Ctrl + Mayús + Alt + Supr").expect("dibujar la larga");
        assert!(
            larga.width() > corta.width(),
            "la larga tenía que ser más ancha"
        );
        assert_eq!(larga.height(), corta.height(), "y las dos igual de altas");
    }

    #[test]
    fn un_texto_vacio_no_dibuja_nada() {
        assert!(dibujar("").is_none());
    }

    #[test]
    fn las_esquinas_salen_transparentes_y_el_centro_no() {
        let pastilla = dibujar("Ctrl + C").expect("dibujar");
        let (ancho, alto) = pastilla.dimensions();
        assert_eq!(pastilla.get_pixel(0, 0).0[3], 0, "la esquina es redonda");
        assert!(
            pastilla.get_pixel(ancho / 2, alto / 2).0[3] > 0,
            "el centro se ve"
        );
    }

    #[test]
    fn hay_letra_blanca_dentro_de_la_pastilla() {
        let pastilla = dibujar("Ctrl + C").expect("dibujar");
        let blancos = pastilla.pixels().filter(|p| p.0[0] > 200).count();
        assert!(blancos > 20, "no se ha escrito el texto: {blancos} píxeles");
    }

    #[test]
    fn el_cache_no_vuelve_a_dibujar_lo_mismo() {
        let mut cache = Cache::default();
        let primera = cache.pastilla("Ctrl + C").expect("la primera").clone();
        let segunda = cache.pastilla("Ctrl + C").expect("la segunda").clone();
        assert_eq!(primera.as_raw(), segunda.as_raw());
        let otra = cache.pastilla("Alt + Tab").expect("otra distinta").clone();
        assert_ne!(otra.dimensions(), primera.dimensions());
    }

    #[test]
    fn la_pastilla_se_pega_abajo_y_centrada() {
        let mut fondo = RgbaImage::from_pixel(400, 300, Rgba([0, 0, 0, 255]));
        let pastilla = dibujar("Ctrl + C").expect("dibujar");
        pegar(&mut fondo, &pastilla, 1.0);
        let tocados: Vec<(u32, u32)> = fondo
            .enumerate_pixels()
            .filter(|(_, _, p)| p.0[0] != 0)
            .map(|(x, y, _)| (x, y))
            .collect();
        assert!(!tocados.is_empty(), "no ha pegado nada");
        let arriba = tocados.iter().map(|(_, y)| *y).min().unwrap();
        assert!(
            arriba > 150,
            "tenía que estar en la mitad de abajo, y está en {arriba}"
        );
    }

    #[test]
    fn una_pastilla_mas_grande_que_el_video_no_se_pega() {
        let mut fondo = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 255]));
        let pastilla = dibujar("Ctrl + Mayús + Supr").expect("dibujar");
        pegar(&mut fondo, &pastilla, 1.0);
        assert!(
            fondo.pixels().all(|p| p.0[0] == 0),
            "se ha salido del vídeo en vez de no pintarse"
        );
    }

    /// Deja la pastilla sobre un fondo con textura, para mirarla con los ojos.
    /// `cargo test --lib ver_la_pastilla -- --ignored --nocapture`
    #[test]
    #[ignore = "no comprueba nada: deja un PNG para mirar"]
    fn ver_la_pastilla() {
        let mut fondo = RgbaImage::new(560, 260);
        for (x, y, p) in fondo.enumerate_pixels_mut() {
            // Un degradado con rejilla, que es lo peor para leer texto encima.
            let base = 70 + ((x + y) / 6) as u8 % 120;
            *p = if x % 40 == 0 || y % 40 == 0 {
                Rgba([base + 30, base + 20, base, 255])
            } else {
                Rgba([base, base / 2 + 40, base / 3 + 60, 255])
            };
        }
        let pastilla = dibujar("Ctrl + Mayús + P").expect("dibujar");
        pegar(&mut fondo, &pastilla, 1.0);
        let ruta = std::env::temp_dir().join("winshotx-pastilla.png");
        let (a, b) = fondo.dimensions();
        crate::encode::png::save(&fondo, &ruta, a, b).expect("guardar");
        println!("{}", ruta.display());
    }

    #[test]
    fn esta_entera_casi_todo_el_rato_y_se_apaga_al_final() {
        assert_eq!(opacidad(0, 1000), 1.0);
        assert_eq!(opacidad(700, 1000), 1.0);
        assert!(opacidad(900, 1000) < 1.0);
        assert_eq!(opacidad(1000, 1000), 0.0);
        assert_eq!(opacidad(5000, 1000), 0.0);
    }
}
