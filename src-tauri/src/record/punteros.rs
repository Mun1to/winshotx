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
/// No se puede leer un cursor monocromo de los antiguos (sin mapa de color): son dos
/// mascaras que se combinan al vuelo, y hoy no los usa nadie. En ese caso el exportador
/// dibuja la flecha de siempre.
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
        return None;
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
        let Some(puntero) = capturar(asa) else {
            eprintln!("[punteros] el cursor actual es monocromo: no se lee, se dibuja la flecha");
            return;
        };
        let (w, h) = puntero.imagen.dimensions();
        assert!(w >= 8 && h >= 8, "un cursor de {w}x{h} no es un cursor");
        assert!(puntero.caliente.0 <= w && puntero.caliente.1 <= h);
        let opacos = puntero.imagen.pixels().filter(|p| p.0[3] > 200).count();
        let transparentes = puntero.imagen.pixels().filter(|p| p.0[3] == 0).count();
        assert!(opacos > 10, "el cursor tiene que tener pixeles que se vean: {opacos}");
        assert!(transparentes > 10, "y pixeles transparentes alrededor: {transparentes}");
    }

    #[test]
    fn un_identificador_inventado_no_revienta() {
        assert!(capturar(0).is_none());
        assert!(capturar(12_345).is_none());
    }
}
