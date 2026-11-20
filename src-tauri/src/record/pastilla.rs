//! Dibujar el nombre de un atajo con la fuente del sistema, para pegarlo en el vídeo.
//!
//! El texto va con GDI porque es lo único a mano que sabe de letras: `image` pinta
//! rectángulos, no tipografía, y traerse un motor de fuentes entero para escribir
//! «Ctrl + C» pesaría más que toda la aplicación.
//!
//! En Linux no hay GDI: ahí las letras salen de las fuentes del sistema (`letras.rs`), y el
//! resto se hace igual que con GDI para que la pastilla mida y se vea lo mismo. En macOS
//! todavía no hay con qué, y no sale.
//!
//! **La pastilla se guarda de un fotograma para otro.** Dibujarla cuesta abrir un contexto
//! de dispositivo, elegir la fuente y medir el texto, y eso treinta veces por segundo
//! durante una grabación larga se nota. Como el texto cambia cuando alguien pulsa un atajo
//! y no en cada fotograma, se guarda la última y se reusa mientras diga lo mismo.

use image::{Rgba, RgbaImage};

/// Alto de la letra, en píxeles.
const LETRA: i32 = 22;
/// Aire dentro de la pastilla.
#[cfg(any(windows, target_os = "linux"))]
const AIRE_X: i32 = 16;
#[cfg(any(windows, target_os = "linux"))]
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

/// Escribe un texto suelto, sin pastilla detrás, del alto y el color que se pida.
///
/// Lo usa la anotación de texto del editor. Comparte con la pastilla todo el trabajo con
/// GDI, que es la parte que cuesta: abrir el contexto, elegir la fuente y medir.
pub fn escribir(texto: &str, alto_letra: i32, color: [u8; 3]) -> Option<RgbaImage> {
    pintar_texto(texto, alto_letra, Some(color))
}

/// Dibuja la pastilla: fondo oscuro redondeado y el texto en blanco encima.
fn dibujar(texto: &str) -> Option<RgbaImage> {
    pintar_texto(texto, LETRA, None)
}

/// El trabajo de verdad. Con `color` puesto sale el texto suelto, y sin él, la pastilla.
#[cfg(windows)]
fn pintar_texto(texto: &str, alto_letra: i32, color: Option<[u8; 3]>) -> Option<RgbaImage> {
    use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
    use windows::Win32::Graphics::Gdi::*;
    use windows::core::HSTRING;

    if texto.is_empty() {
        return None;
    }
    unsafe {
        let dc = CreateCompatibleDC(None);
        let fuente = CreateFontW(
            alto_letra,
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
        // El texto suelto va sin aire: lo pega quien lo pide, donde lo pida.
        let (aire_x, aire_y) = if color.is_some() { (2, 2) } else { (AIRE_X, AIRE_Y) };
        let ancho = (medida.cx + aire_x * 2).max(1);
        let alto = (medida.cy + aire_y * 2).max(1);

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
        let _ = TextOutW(dc, aire_x, aire_y, &utf16);

        let total = (ancho * alto) as usize;
        let crudo = std::slice::from_raw_parts(pixeles as *const u8, total * 4);
        let mut salida = RgbaImage::new(ancho as u32, alto as u32);
        let radio = (alto / 3).max(4);
        for (i, pixel) in salida.pixels_mut().enumerate() {
            let x = (i as i32) % ancho;
            let y = (i as i32) / ancho;
            let claro = crudo[i * 4 + 2].max(crudo[i * 4 + 1]).max(crudo[i * 4]);
            // Texto suelto: solo la letra, del color que se pidió, y el resto transparente.
            if let Some(rgb) = color {
                *pixel = if claro > 0x33 {
                    Rgba([rgb[0], rgb[1], rgb[2], claro])
                } else {
                    Rgba([0, 0, 0, 0])
                };
                continue;
            }
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

/// Lo mismo en Linux, con las fuentes del sistema en vez de GDI.
///
/// Se imita a GDI paso a paso para que salga igual en los dos sistemas: se mide, se le pone
/// el mismo aire, se escribe en blanco sobre el mismo gris muy oscuro y se decide qué es
/// letra y qué es fondo con el mismo umbral. La única diferencia es de dónde sale la letra.
#[cfg(target_os = "linux")]
fn pintar_texto(texto: &str, alto_letra: i32, color: Option<[u8; 3]>) -> Option<RgbaImage> {
    /// El gris sobre el que escribe el camino de GDI.
    const FONDO: u8 = 0x11;

    if texto.is_empty() {
        return None;
    }
    let renglon = super::letras::componer(texto, alto_letra)?;
    let (aire_x, aire_y) = if color.is_some() { (2, 2) } else { (AIRE_X, AIRE_Y) };
    let ancho = (renglon.ancho + aire_x * 2).max(1);
    let alto = (renglon.alto + aire_y * 2).max(1);

    // Lo que dejaría GDI en el mapa: el fondo gris y la letra blanca encima, mezclada con el
    // gris según cuánto del píxel tapa el glifo.
    let mut claros = vec![FONDO; (ancho * alto) as usize];
    renglon.pintar(|x, y, cobertura| {
        let (x, y) = (x + aire_x, y + aire_y);
        if x < 0 || y < 0 || x >= ancho || y >= alto {
            return;
        }
        let tinta = cobertura.clamp(0.0, 1.0) * f32::from(255 - FONDO);
        let valor = (f32::from(FONDO) + tinta).round() as u8;
        // Dos letras que se tocan (el kerning las junta) se quedan con lo más claro, como
        // letra blanca pintada encima de letra blanca.
        let i = (y * ancho + x) as usize;
        claros[i] = claros[i].max(valor);
    });

    let mut salida = RgbaImage::new(ancho as u32, alto as u32);
    let radio = (alto / 3).max(4);
    for (i, pixel) in salida.pixels_mut().enumerate() {
        let x = (i as i32) % ancho;
        let y = (i as i32) / ancho;
        let claro = claros[i];
        // Texto suelto: solo la letra, del color que se pidió, y el resto transparente.
        if let Some(rgb) = color {
            *pixel = if claro > 0x33 {
                Rgba([rgb[0], rgb[1], rgb[2], claro])
            } else {
                Rgba([0, 0, 0, 0])
            };
            continue;
        }
        *pixel = if !dentro_de_la_pastilla(x, y, ancho, alto, radio) {
            Rgba([0, 0, 0, 0])
        } else if claro > 0x33 {
            // Letra, con el borde suavizado convertido en transparencia.
            Rgba([255, 255, 255, claro])
        } else {
            // Fondo de la pastilla, el mismo oscuro casi opaco que con GDI.
            Rgba([16, 16, 20, 225])
        };
    }
    Some(salida)
}

/// En macOS todavía no hay con qué escribir letras: sin texto no sale pastilla, y quien la
/// pide se queda sin ella en vez de reventar.
#[cfg(not(any(windows, target_os = "linux")))]
fn pintar_texto(_texto: &str, _alto_letra: i32, _color: Option<[u8; 3]>) -> Option<RgbaImage> {
    None
}

/// Si el punto cae dentro del rectángulo con las esquinas redondeadas.
#[cfg(any(windows, target_os = "linux"))]
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

#[cfg(all(test, any(windows, target_os = "linux")))]
mod tests {
    use super::*;

    /// Las filas de arriba abajo que tienen algo de letra.
    fn filas_con_letra(dibujo: &RgbaImage) -> u32 {
        (0..dibujo.height())
            .filter(|&y| (0..dibujo.width()).any(|x| dibujo.get_pixel(x, y).0[3] > 0))
            .count() as u32
    }

    /// La pastilla es su texto más el aire, ni más ni menos, y el texto mide de alto lo que
    /// se pidió (más los dos píxeles de margen del texto suelto por cada lado).
    #[test]
    fn la_pastilla_mide_su_texto_mas_el_aire() {
        let texto = "Ctrl + Mayús + 2";
        let pastilla = dibujar(texto).expect("dibujar la pastilla");
        let suelto = escribir(texto, LETRA, [255, 255, 255]).expect("escribir el texto");
        assert_eq!(
            pastilla.width() as i32 - suelto.width() as i32,
            2 * (AIRE_X - 2),
            "a lo ancho, la pastilla tiene que ser el texto más su aire"
        );
        assert_eq!(
            pastilla.height() as i32 - suelto.height() as i32,
            2 * (AIRE_Y - 2),
            "y a lo alto, igual"
        );
        let alto_texto = suelto.height() as i32 - 4;
        assert!(
            (alto_texto - LETRA).abs() <= 1,
            "se pidió letra de {LETRA} y la celda mide {alto_texto}"
        );
        assert!(suelto.width() > suelto.height() * 3, "«{texto}» es mucho más ancho que alto");
    }

    /// El alto que se pide es el de la celda (ascendente más descendente), no el em: es lo
    /// que hace `CreateFontW` con un alto positivo. Una H mide algo más de la mitad de la
    /// celda en todas las fuentes de texto; medida contra el em pasaría del 70 %.
    #[test]
    fn el_alto_de_la_letra_es_el_de_la_celda() {
        let hache = escribir("H", 100, [255, 255, 255]).expect("escribir la H");
        let alto = filas_con_letra(&hache);
        assert!(
            (45..=68).contains(&alto),
            "una H en letra de 100 mide {alto} px de alto: la escala no es la de la celda"
        );
    }

    /// El texto suelto sale del color pedido, con la letra opaca por dentro y el resto
    /// transparente: es lo que se pega encima de la captura.
    #[test]
    fn el_texto_suelto_sale_del_color_pedido() {
        let azul = [10, 155, 255];
        let dibujo = escribir("Hola", 30, azul).expect("escribir");
        let visibles: Vec<_> = dibujo.pixels().filter(|p| p.0[3] > 0).collect();
        assert!(
            visibles.iter().all(|p| p.0[..3] == azul),
            "hay letra de otro color que el pedido"
        );
        let opacos = visibles.iter().filter(|p| p.0[3] > 200).count();
        assert!(opacos > 40, "la letra tiene que ser opaca por dentro: {opacos} píxeles");
        assert_eq!(dibujo.get_pixel(0, 0).0[3], 0, "y el margen, transparente");
    }

    /// Un carácter que no está en la fuente (chino, sobre una latina) no revienta, y no se
    /// come lo que va delante.
    #[test]
    fn un_caracter_chino_no_revienta() {
        let latino = escribir("Ctrl + ", LETRA, [255, 255, 255]).expect("lo latino");
        let mezclado = escribir("Ctrl + 中", LETRA, [255, 255, 255]).expect("con el chino");
        assert!(mezclado.width() >= latino.width(), "el chino se ha comido el principio");
        // En Linux la celda es siempre la de la fuente principal. En Windows el alto con
        // una fuente enlazada lo decide GDI, y eso no se ha medido.
        if cfg!(target_os = "linux") {
            assert_eq!(mezclado.height(), latino.height(), "y no cambia el alto de la celda");
        }
        assert!(dibujar("中文").is_some(), "una pastilla solo en chino también sale");
    }

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

    /// Texto suelto de varios colores y tamaños sobre claro, con tildes y chino, para mirarlo.
    /// `cargo test --lib ver_el_texto_suelto -- --ignored --nocapture`
    #[test]
    #[ignore = "no comprueba nada: deja un PNG para mirar"]
    fn ver_el_texto_suelto() {
        let mut fondo = RgbaImage::from_pixel(640, 300, Rgba([246, 247, 250, 255]));
        let lineas: [(&str, i32, [u8; 3]); 4] = [
            ("Texto suelto: «ñandú», pingüino", 34, [17, 24, 39]),
            ("Cuidado con este botón", 26, [239, 68, 68]),
            ("Ajustes > Grabación > 60 fps", 22, [10, 155, 255]),
            ("Mezclado: Ctrl + 中文 ok", 28, [22, 163, 74]),
        ];
        let mut y = 16i64;
        for (texto, alto, color) in lineas {
            let dibujo = escribir(texto, alto, color).expect("escribir");
            image::imageops::overlay(&mut fondo, &dibujo, 16, y);
            y += i64::from(dibujo.height()) + 18;
        }
        let pastilla = dibujar("Ctrl + Mayús + 2").expect("dibujar");
        image::imageops::overlay(&mut fondo, &pastilla, 16, y);
        let ruta = std::env::temp_dir().join("winshotx-texto-suelto.png");
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
