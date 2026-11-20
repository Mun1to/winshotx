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
//!
//! **En Linux** el arrastre lo lleva GTK (`arrastrar_en_gtk`), que vale para X11 y para
//! Wayland, y lo que viaja es la dirección `file://` del archivo en `text/uri-list`.

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

#[cfg(not(any(windows, target_os = "linux")))]
pub fn arrastrar(_hwnd: isize, _ruta: &Path) -> Result<bool> {
    Err(AppError::Msg("arrastrar solo existe en Windows y en Linux".into()))
}

/// Lo que recibe en Linux la app donde se suelta: el archivo como dirección `file://` (lo que
/// leen los gestores de archivos, los navegadores y los chats, en `text/uri-list`) y, para
/// quien solo entiende texto (una terminal, un editor), la ruta tal cual.
#[cfg(target_os = "linux")]
pub fn lo_que_se_ofrece(ruta: &Path) -> Result<(String, String)> {
    if !ruta.is_file() {
        return Err(AppError::Msg("ese archivo ya no está".into()));
    }
    let uri = gtk::glib::filename_to_uri(ruta, None)
        .map_err(|e| AppError::Msg(format!("no se puede arrastrar ese archivo: {e}")))?;
    Ok((uri.to_string(), ruta.to_string_lossy().to_string()))
}

/// Con qué número se apunta cada formato del arrastre de Linux.
#[cfg(target_os = "linux")]
const COMO_ARCHIVO: u32 = 0;
#[cfg(target_os = "linux")]
const COMO_TEXTO: u32 = 1;

/// Arrastra el archivo desde una ventana de winshotx hasta donde se suelte el ratón (Linux).
///
/// Es el arrastre de GTK, que es quien pinta las ventanas de Tauri y habla las dos cosas que
/// hay: XDND en X11 y `wl_data_device` en Wayland. **Se llama con el botón ya pulsado** y desde
/// el hilo de GTK. A diferencia del de Windows, vuelve en el acto: el arrastre sigue por su
/// cuenta y `al_acabar` dice al final si alguien lo recogió.
///
/// Devuelve `false`, sin empezar nada, si el botón ya no está abajo: ver `sigue_pulsado`.
///
/// Se ofrece copiar, nunca mover, por lo mismo que en Windows: nadie ha pedido que el archivo
/// desaparezca de la carpeta de capturas por arrastrarlo. `icono` es la imagen que va con el
/// ratón mientras se arrastra (la vista pequeña de la captura).
#[cfg(target_os = "linux")]
pub fn arrastrar_en_gtk(
    ventana: &gtk::ApplicationWindow,
    ruta: &Path,
    icono: Option<&Path>,
    al_acabar: impl Fn(bool) + 'static,
) -> Result<bool> {
    use gtk::prelude::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    let (uri, texto) = lo_que_se_ofrece(ruta)?;
    if !sigue_pulsado(ventana) {
        return Ok(false);
    }
    let formatos = gtk::TargetList::new(&[]);
    formatos.add_uri_targets(COMO_ARCHIVO);
    formatos.add_text_targets(COMO_TEXTO);

    // Las tres señales viven lo que dura este arrastre: al acabar se sueltan, o cada
    // miniatura arrastrada dejaría las suyas colgando de la ventana.
    let sennales: Rc<RefCell<Vec<gtk::glib::SignalHandlerId>>> = Rc::default();
    let rechazado = Rc::new(Cell::new(false));
    let dar = ventana.connect_drag_data_get(move |_, _, datos, formato, _| {
        if formato == COMO_ARCHIVO {
            datos.set_uris(&[uri.as_str()]);
        } else {
            datos.set_text(&texto);
        }
    });
    let lo_apunta = rechazado.clone();
    let rechazar = ventana.connect_drag_failed(move |_, _, _| {
        lo_apunta.set(true);
        gtk::glib::Propagation::Proceed
    });
    let para_soltar = sennales.clone();
    let acabar = ventana.connect_drag_end(move |ventana, _| {
        for sennal in para_soltar.borrow_mut().drain(..) {
            ventana.disconnect(sennal);
        }
        devolver_el_raton_a_la_pagina(ventana);
        al_acabar(!rechazado.get());
    });
    sennales.borrow_mut().extend([dar, rechazar, acabar]);

    // Sin evento: la orden llega de la página, no de una señal de GTK. GTK coge entonces el
    // ratón del asiento y la pulsación que sigue abajo.
    let Some(contexto) = ventana.drag_begin_with_coordinates(
        &formatos,
        gtk::gdk::DragAction::COPY,
        1,
        None,
        -1,
        -1,
    ) else {
        for sennal in sennales.borrow_mut().drain(..) {
            ventana.disconnect(sennal);
        }
        return Err(AppError::Msg("el escritorio no ha dejado arrastrar".into()));
    };
    if let Some(imagen) = icono.and_then(|i| gtk::gdk_pixbuf::Pixbuf::from_file_at_size(i, 160, 100).ok()) {
        contexto.drag_set_icon_pixbuf(&imagen, imagen.width() / 2, imagen.height() / 2);
    }
    Ok(true)
}

/// Si el botón principal del ratón sigue abajo ahora mismo.
///
/// La orden de arrastrar viene de la página y tarda unos milisegundos en llegar al hilo de GTK.
/// En un gesto corto (pulsar, moverse y soltar seguidos, o un clic con el pulso movido) el botón
/// ya está arriba cuando llega, y un arrastre de GTK empezado así no lo acaba nadie: GTK espera
/// un soltar que ya pasó. Medido el 18-nov-2026 en X11 con el gesto entero en 9 ms: la imagen de
/// la captura se quedaba colgando del ratón sin ningún botón pulsado, y el clic siguiente, en
/// cualquier sitio, soltaba el archivo allí.
///
/// En X11 se le pregunta al servidor; en Wayland GDK contesta con lo último que le ha contado
/// el escritorio, que en este hilo ya incluye el soltar.
#[cfg(target_os = "linux")]
fn sigue_pulsado(ventana: &gtk::ApplicationWindow) -> bool {
    use gtk::prelude::*;

    let Some(suya) = ventana.window() else {
        return false;
    };
    let Some(puntero) = suya.display().default_seat().and_then(|asiento| asiento.pointer()) else {
        return false;
    };
    let (_, _, _, botones) = suya.device_position(&puntero);
    botones.contains(gtk::gdk::ModifierType::BUTTON1_MASK)
}

/// Le cuenta a la página que el botón ya se soltó y que el ratón salió de ella.
///
/// El arrastre de GTK se queda con el ratón desde que empieza, así que a WebKitGTK no le llega
/// ni el soltar ni la salida: se queda creyendo que el botón sigue pulsado y que el ratón sigue
/// donde estaba. Medido el 18-nov-2026 en GNOME Wayland y en X11: la miniatura siguiente nacía
/// con sus botones a la vista sin tener el ratón encima (y así no se iba sola), y la pulsación
/// siguiente llegaba sin `pointerdown`.
///
/// Son los dos avisos que GTK le habría dado a la página, con el punto fuera de ella para que
/// el soltar no parezca un clic en nada.
#[cfg(target_os = "linux")]
fn devolver_el_raton_a_la_pagina(ventana: &gtk::ApplicationWindow) {
    use gtk::gdk;
    use gtk::gdk::prelude::*;
    use gtk::glib::translate::{ToGlibPtr, ToGlibPtrMut};
    use gtk::prelude::*;

    fn la_pagina(widget: &gtk::Widget) -> Option<gtk::Widget> {
        if widget.type_().name() == "WebKitWebView" {
            return Some(widget.clone());
        }
        widget
            .downcast_ref::<gtk::Container>()?
            .children()
            .iter()
            .find_map(la_pagina)
    }
    let Some(pagina) = la_pagina(ventana.upcast_ref::<gtk::Widget>()) else {
        return;
    };
    let Some(suya) = pagina.window() else {
        return;
    };
    let puntero = suya.display().default_seat().and_then(|asiento| asiento.pointer());

    for tipo in [gdk::EventType::ButtonRelease, gdk::EventType::LeaveNotify] {
        let mut aviso = gdk::Event::new(tipo);
        // gtk-rs no deja rellenar un evento hecho a mano: se escriben sus campos tal cual.
        // El evento se queda con su propia referencia a la ventana y la suelta al liberarse.
        unsafe {
            let crudo: *mut gdk::ffi::GdkEvent = aviso.to_glib_none_mut().0;
            let de_la_pagina: *mut gdk::ffi::GdkWindow = suya.to_glib_full();
            if tipo == gdk::EventType::ButtonRelease {
                let soltar = crudo as *mut gdk::ffi::GdkEventButton;
                (*soltar).window = de_la_pagina;
                (*soltar).send_event = 1;
                (*soltar).button = 1;
                (*soltar).state = gdk::ffi::GDK_BUTTON1_MASK;
                (*soltar).x = -1.0;
                (*soltar).y = -1.0;
                (*soltar).x_root = -1.0;
                (*soltar).y_root = -1.0;
            } else {
                let salir = crudo as *mut gdk::ffi::GdkEventCrossing;
                (*salir).window = de_la_pagina;
                (*salir).send_event = 1;
                (*salir).mode = gdk::ffi::GDK_CROSSING_NORMAL;
                (*salir).detail = gdk::ffi::GDK_NOTIFY_ANCESTOR;
                (*salir).x = -1.0;
                (*salir).y = -1.0;
                (*salir).x_root = -1.0;
                (*salir).y_root = -1.0;
            }
        }
        aviso.set_device(puntero.as_ref());
        aviso.set_source_device(puntero.as_ref());
        let _ = pagina.event(&aviso);
    }
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

#[cfg(all(test, target_os = "linux"))]
mod pruebas_de_linux {
    use super::*;

    /// Lo que recibe la app donde se suelta: la dirección del archivo, con los espacios, las
    /// tildes y las eñes de la carpeta escritos como pide una URL (la de muchos usuarios
    /// lleva su nombre), y la ruta tal cual para quien solo lee texto.
    #[test]
    fn el_archivo_viaja_como_direccion_y_como_ruta() {
        let carpeta = std::env::temp_dir().join("winshotx prueba arrastrar ñandú");
        std::fs::create_dir_all(&carpeta).unwrap();
        let ruta = carpeta.join("winshotx-20261118-120000.png");
        std::fs::write(&ruta, b"x").unwrap();

        let (uri, texto) = lo_que_se_ofrece(&ruta).unwrap();
        assert!(uri.starts_with("file:///"), "{uri}");
        assert!(
            uri.ends_with("/winshotx%20prueba%20arrastrar%20%C3%B1and%C3%BA/winshotx-20261118-120000.png"),
            "{uri}"
        );
        assert_eq!(texto, ruta.to_string_lossy());

        let _ = std::fs::remove_dir_all(&carpeta);
    }

    /// Un archivo que ya no existe (se borró por fuera con la miniatura puesta) da un error
    /// con su frase, no un arrastre vacío.
    #[test]
    fn un_archivo_que_no_existe_no_se_arrastra() {
        let error = lo_que_se_ofrece(Path::new("/no/existe/winshotx.png")).unwrap_err();
        assert!(error.to_string().contains("ya no está"), "{error}");
    }
}
