//! Arrastrar una captura de winshotx a cualquier otra app: WhatsApp, Discord, un correo, una
//! carpeta (E2 del plan de CleanShot X).
//!
//! Desde una página web no se puede: lo que arrastra el navegador se queda dentro del
//! navegador, o sale como un enlace o un trozo de texto. Para que otra app reciba **un
//! archivo** hay que empezar el arrastre con OLE, la maquinaria de Windows de arrastrar y
//! soltar, desde Rust.
//!
//! **No se fabrica el paquete a mano.** Lo que se arrastra es el mismo objeto que entrega el
//! Explorador cuando arrastras un archivo desde una carpeta (`BHID_DataObject` del archivo).
//! Lleva `CF_HDROP` (la lista de archivos, que es lo que leen casi todas las apps) y además
//! los formatos propios del Explorador, así que lo que acepta un archivo arrastrado desde
//! una carpeta acepta este igual. `SHDoDragDrop` pone encima la imagen del arrastre (la
//! miniatura del archivo) sin dibujar nada aquí.
//!
//! **Lo que no se puede probar aquí** es soltarlo en otra app: eso es un ratón de verdad
//! sobre una ventana de verdad. Lo que sí se prueba es que el paquete lleva el archivo
//! correcto en `CF_HDROP`, que es lo que leerá quien lo reciba.

use std::path::Path;

use crate::error::{AppError, Result};

/// El paquete que recibe la app donde se suelta: el del Explorador para ese archivo.
#[cfg(windows)]
pub fn paquete(ruta: &Path) -> Result<windows::Win32::System::Com::IDataObject> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::UI::Shell::{IShellItem, SHCreateItemFromParsingName, BHID_DataObject};

    if !ruta.is_file() {
        return Err(AppError::Msg("ese archivo ya no está".into()));
    }
    let ancho: Vec<u16> = ruta.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(ancho.as_ptr()), None)
            .map_err(|e| AppError::Msg(format!("no se puede arrastrar ese archivo: {e}")))?;
        item.BindToHandler::<_, IDataObject>(None, &BHID_DataObject)
            .map_err(|e| AppError::Msg(format!("no se puede arrastrar ese archivo: {e}")))
    }
}

/// Arrastra el archivo desde la ventana `hwnd` hasta donde se suelte el ratón.
///
/// **Se llama con el botón del ratón ya pulsado** y desde el hilo de la interfaz, el que
/// tiene OLE arrancado. La llamada no vuelve hasta que se suelta: mientras tanto Windows
/// lleva el bucle de mensajes, así que la ventana sigue pintándose. Devuelve si alguien lo
/// recogió (`true`) o se soltó en un sitio que no lo quería (`false`).
///
/// Se ofrece copiar y enlazar, nunca mover: si la app de destino eligiera «mover», el
/// archivo desaparecería de la carpeta de capturas, y eso nadie lo ha pedido al arrastrar.
#[cfg(windows)]
pub fn arrastrar(hwnd: isize, ruta: &Path) -> Result<bool> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Ole::{
        IDropSource, OleInitialize, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_NONE,
    };
    use windows::Win32::UI::Shell::SHDoDragDrop;

    // En el hilo de la interfaz OLE ya está arrancado (WebView2 lo necesita), y entonces
    // esto no hace nada. Si no lo estuviera, sin esto `SHDoDragDrop` falla sin arrastrar.
    unsafe {
        let _ = OleInitialize(None);
    }
    let datos = paquete(ruta)?;
    let efecto = unsafe {
        SHDoDragDrop(
            Some(HWND(hwnd as *mut core::ffi::c_void)),
            &datos,
            None::<&IDropSource>,
            DROPEFFECT_COPY | DROPEFFECT_LINK,
        )
    }
    .map_err(|e| AppError::Msg(format!("no se ha podido arrastrar: {e}")))?;
    Ok(efecto != DROPEFFECT_NONE)
}

#[cfg(not(windows))]
pub fn arrastrar(_hwnd: isize, _ruta: &Path) -> Result<bool> {
    Err(AppError::Msg("arrastrar solo existe en Windows".into()))
}

/// Las rutas que lleva un paquete en `CF_HDROP`, que es lo que leen las apps al soltar. Es
/// el lado de quien recibe: lo usan las pruebas para mirar lo que llegaría.
#[cfg(all(test, windows))]
pub fn archivos_del_paquete(
    datos: &windows::Win32::System::Com::IDataObject,
) -> Result<Vec<std::path::PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::{ReleaseStgMedium, CF_HDROP};
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

    let formato = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    unsafe {
        let mut medio = datos
            .GetData(&formato)
            .map_err(|e| AppError::Msg(format!("el paquete no lleva la lista de archivos: {e}")))?;
        let global = medio.u.hGlobal;
        let hdrop = HDROP(GlobalLock(global));
        let cuantos = DragQueryFileW(hdrop, u32::MAX, None);
        let mut rutas = Vec::new();
        for i in 0..cuantos {
            let largo = DragQueryFileW(hdrop, i, None) as usize;
            let mut texto = vec![0u16; largo + 1];
            DragQueryFileW(hdrop, i, Some(&mut texto));
            texto.truncate(largo);
            rutas.push(std::ffi::OsString::from_wide(&texto).into());
        }
        let _ = GlobalUnlock(global);
        ReleaseStgMedium(&mut medio);
        Ok(rutas)
    }
}

#[cfg(all(test, windows))]
mod pruebas {
    use super::*;

    /// Lo que recibe la app donde se suelta: `CF_HDROP` con la ruta de la captura, entera,
    /// también con espacios, tildes y eñes en la carpeta (la de muchos usuarios lleva su
    /// nombre).
    #[test]
    fn el_paquete_lleva_la_captura_en_la_lista_de_archivos() {
        let _ = unsafe { windows::Win32::System::Ole::OleInitialize(None) };
        let carpeta = std::env::temp_dir().join("winshotx prueba arrastrar ñandú");
        std::fs::create_dir_all(&carpeta).unwrap();
        let ruta = carpeta.join("winshotx-20261111-120000.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(&ruta)
            .unwrap();

        let datos = paquete(&ruta).expect("el Explorador tenía que dar el paquete");
        let rutas = archivos_del_paquete(&datos).unwrap();
        assert_eq!(rutas, vec![ruta.clone()]);

        let _ = std::fs::remove_dir_all(&carpeta);
    }

    /// Un archivo que ya no existe (se borró por fuera mientras estaba la miniatura) da un
    /// error con su frase, no un arrastre vacío.
    #[test]
    fn un_archivo_que_no_existe_no_se_arrastra() {
        let error = paquete(Path::new(r"C:\no\existe\winshotx.png")).unwrap_err();
        assert!(error.to_string().contains("ya no está"), "{error}");
    }
}
