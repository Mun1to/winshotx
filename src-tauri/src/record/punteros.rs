//! La imagen de verdad del puntero de Windows, leida al grabar.
//!
//! Dibujar el puntero con un poligono hecho a mano se veia mal: una flecha de siete puntos
//! no es la flecha de Windows, y menos la manita. Munir, el 10 de septiembre de 2026: *«el
//! cursor funciona completamente muy mal... en mala calidad»*. Aqui se le pide a Windows
//! el cursor que tiene puesto AHORA, con su transparencia y su punto caliente, y esa imagen
//! es la que se escala y se dibuja al exportar. Sale el puntero de cada uno, sea el de
//! serie, el grande de accesibilidad o uno descargado.
//!
//! Se lee una vez por cursor distinto, no por fotograma: `GetIconInfo` crea dos mapas de
//! bits nuevos cada vez y hay que borrarlos, y una grabacion de diez minutos con tres
//! cursores distintos son tres lecturas.

#![cfg(windows)]

use image::RgbaImage;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, HICON, ICONINFO};

/// Un puntero leido de Windows, con su punto caliente en pixeles de la propia imagen.
#[derive(Debug, Clone)]
pub struct Puntero {
    pub caliente: (u32, u32),
    pub imagen: RgbaImage,
}

/// La imagen del cursor con ese identificador, o `None` si no se puede leer.
///
/// Hay dos clases de cursor. Los de color (los de serie de Windows 11 y casi todos los
/// descargados) traen su mapa de color con transparencia. Los **monocromos** (los esquemas
/// «Windows Black», «Invertido» y los clasicos, y tambien los que Windows entrega a un
/// proceso sin ventana) no tienen mapa de color: son dos mascaras, una que dice que se
/// deja pasar y otra que dice que se pinta, apiladas en un solo mapa del doble de alto.
/// Los dos se leen; la primera version solo leia los de color y en esta maquina la flecha
/// del sistema salio monocroma.
pub fn capturar(hcursor: isize) -> Option<Puntero> {
    let mut info = ICONINFO::default();
    unsafe { GetIconInfo(HICON(hcursor as *mut core::ffi::c_void), &mut info) }.ok()?;
    let resultado = leer(&info);
    // `GetIconInfo` crea los dos mapas de bits para nosotros: hay que soltarlos siempre,
    // se hayan podido leer o no.
    unsafe {
        if !info.hbmColor.is_invalid() {
            let _ = DeleteObject(info.hbmColor.into());
        }
        if !info.hbmMask.is_invalid() {
            let _ = DeleteObject(info.hbmMask.into());
        }
    }
    resultado
}

fn leer(info: &ICONINFO) -> Option<Puntero> {
    if info.hbmColor.is_invalid() {
        return leer_monocromo(info);
    }
    let (ancho, alto, mut color) = bits_de(info.hbmColor)?;
    // Los cursores modernos traen la transparencia en el cuarto byte. Si ninguno lo usa,
    // la transparencia esta en la mascara: blanco donde no hay nada, negro donde si.
    let con_alfa = color.chunks_exact(4).any(|p| p[3] != 0);
    if !con_alfa {
        let (ma, mh, mascara) = bits_de(info.hbmMask)?;
        if (ma, mh) != (ancho, alto) {
            return None;
        }
        for (pixel, m) in color.chunks_exact_mut(4).zip(mascara.chunks_exact(4)) {
            pixel[3] = if m[0] == 0 { 255 } else { 0 };
        }
    }
    // De BGRA a RGBA.
    for pixel in color.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let imagen = RgbaImage::from_raw(ancho, alto, color)?;
    Some(Puntero {
        caliente: (info.xHotspot.min(ancho), info.yHotspot.min(alto)),
        imagen,
    })
}

/// Un cursor sin mapa de color: la mascara lleva arriba lo que se deja pasar (AND) y abajo
/// lo que se pinta (XOR). Dejar pasar y no pintar es transparente; pintar es blanco; ni
/// pasar ni pintar es negro; y pasar y pintar, que en pantalla invierte lo de debajo, se
/// dibuja blanco, que es lo que mas se parece sobre lo que suele haber en un video.
fn leer_monocromo(info: &ICONINFO) -> Option<Puntero> {
    let (ancho, alto_doble, mascara) = bits_de(info.hbmMask)?;
    if alto_doble < 2 || alto_doble % 2 != 0 {
        return None;
    }
    let alto = alto_doble / 2;
    let fila = (ancho * 4) as usize;
    let mut rgba = vec![0u8; (ancho * alto * 4) as usize];
    for y in 0..alto as usize {
        let pasa = &mascara[y * fila..(y + 1) * fila];
        let pinta = &mascara[(y + alto as usize) * fila..(y + alto as usize + 1) * fila];
        for x in 0..ancho as usize {
            let deja_pasar = pasa[x * 4] != 0;
            let se_pinta = pinta[x * 4] != 0;
            let destino = &mut rgba[(y * ancho as usize + x) * 4..(y * ancho as usize + x) * 4 + 4];
            match (deja_pasar, se_pinta) {
                (true, false) => destino.copy_from_slice(&[0, 0, 0, 0]),
                (false, false) => destino.copy_from_slice(&[0, 0, 0, 255]),
                _ => destino.copy_from_slice(&[255, 255, 255, 255]),
            }
        }
    }
    let imagen = RgbaImage::from_raw(ancho, alto, rgba)?;
    Some(Puntero {
        caliente: (info.xHotspot.min(ancho), info.yHotspot.min(alto)),
        imagen,
    })
}

/// Los pixeles de un mapa de bits, en BGRA de 32 bits y con las filas de arriba abajo.
fn bits_de(hbm: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut bm = BITMAP::default();
    let leidos = unsafe {
        GetObjectW(
            hbm.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&raw mut bm).cast()),
        )
    };
    if leidos == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
        return None;
    }
    let (ancho, alto) = (bm.bmWidth as u32, bm.bmHeight as u32);
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ancho as i32,
            // Negativo: las filas vienen de arriba abajo, como las quiere `image`.
            biHeight: -(alto as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bytes = vec![0u8; (ancho * alto * 4) as usize];
    let hdc = unsafe { GetDC(None) };
    let filas = unsafe {
        GetDIBits(
            hdc,
            hbm,
            0,
            alto,
            Some(bytes.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    unsafe { ReleaseDC(None, hdc) };
    (filas as u32 == alto).then_some((ancho, alto, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lee el cursor que hay puesto ahora mismo en la maquina que corre las pruebas. Es lo
    /// unico que no se puede saber leyendo el codigo: que Windows entrega la imagen, con
    /// transparencia, y que el punto caliente cae dentro de ella.
    #[test]
    fn el_cursor_de_ahora_mismo_se_lee_con_su_transparencia() {
        let Some((asa, _)) = crate::record::raton::puntero_actual() else {
            eprintln!("[punteros] no hay cursor a la vista (¿sesion sin escritorio?)");
            return;
        };
        let puntero = capturar(asa).expect("el cursor de ahora mismo tiene que leerse");
        let (w, h) = puntero.imagen.dimensions();
        assert!(w >= 8 && h >= 8, "un cursor de {w}x{h} no es un cursor");
        assert!(puntero.caliente.0 <= w && puntero.caliente.1 <= h);
        let opacos = puntero.imagen.pixels().filter(|p| p.0[3] > 200).count();
        let transparentes = puntero.imagen.pixels().filter(|p| p.0[3] == 0).count();
        assert!(opacos > 10, "el cursor tiene que tener pixeles que se vean: {opacos}");
        assert!(transparentes > 10, "y pixeles transparentes alrededor: {transparentes}");
    }

    /// Los cursores del sistema se pueden cargar por su identificador fijo, sin que haya
    /// ninguno a la vista: la flecha y la barra de texto de Windows se leen enteros, con su
    /// transparencia y con el punto caliente dentro de la imagen. Es lo que comprueba la
    /// lectura de verdad aunque quien corre las pruebas tenga el puntero escondido por
    /// estar tecleando. En esta maquina la flecha del sistema sale monocroma desde el
    /// proceso de pruebas, asi que esto ejercita justo el camino de las dos mascaras.
    #[test]
    fn la_flecha_y_la_barra_del_sistema_se_leen_enteras() {
        use windows::Win32::UI::WindowsAndMessaging::{LoadCursorW, IDC_ARROW, IDC_IBEAM};

        for (nombre, id) in [("flecha", IDC_ARROW), ("barra", IDC_IBEAM)] {
            let asa = unsafe { LoadCursorW(None, id) }.expect("cursor del sistema");
            let p = capturar(asa.0 as isize).unwrap_or_else(|| panic!("la {nombre} tiene que leerse"));
            let (w, h) = p.imagen.dimensions();
            assert!(w >= 16 && h >= 16, "{nombre}: {w}x{h}");
            assert!(p.caliente.0 <= w && p.caliente.1 <= h, "{nombre}: {:?}", p.caliente);
            let opacos = p.imagen.pixels().filter(|px| px.0[3] > 200).count();
            let transparentes = p.imagen.pixels().filter(|px| px.0[3] == 0).count();
            assert!(opacos > 30, "la {nombre} tiene que verse: {opacos}");
            assert!(
                transparentes > (w * h / 3) as usize,
                "y sobrar sitio transparente alrededor de la {nombre}: {transparentes}"
            );
        }
    }

    #[test]
    fn un_identificador_inventado_no_revienta() {
        assert!(capturar(0).is_none());
        assert!(capturar(12_345).is_none());
    }
}
