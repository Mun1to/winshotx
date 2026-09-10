//! El marco que se ve alrededor de lo que se esta grabando.
//!
//! Sin esto, al empezar a grabar el overlay se cierra y **no queda ni un pixel que diga
//! que zona se esta grabando**: solo la barra, debajo. Quien graba un tutorial mueve
//! ventanas, abre menus, y sin marco no sabe si lo que hace cae dentro o fuera hasta que
//! ve el video minutos despues.
//!
//! **Son cuatro ventanas de Windows a pelo, sin webview.** Cada webview cuesta decenas de
//! megabytes y un buen rato en arrancar, y aqui hacen falta cuatro rectangulos de dos
//! pixeles de ancho: una linea arriba, otra abajo y dos a los lados, **por fuera** de la
//! region. Por fuera es lo que las deja fuera del video: la captura recorta exactamente la
//! region, y el marco empieza un pixel mas alla.
//!
//! Las cuatro son de las que **no se pueden pulsar** (`WS_EX_TRANSPARENT`: el clic las
//! atraviesa y llega a lo que haya debajo), **no cogen el foco** (`WS_EX_NOACTIVATE`),
//! no salen en Alt+Tab (`WS_EX_TOOLWINDOW`) y van siempre encima (`WS_EX_TOPMOST`).
//!
//! Viven en **su propio hilo con su propio bucle de mensajes**, como cualquier ventana de
//! Windows: una ventana atiende los mensajes en el hilo que la creo, y el hilo principal
//! esta ocupado con Tauri. Se les habla con `PostThreadMessage` (cambiar el color, cerrar)
//! y el hilo termina cuando se le pide, no antes.

#![cfg(windows)]

use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, InvalidateRect, HBRUSH,
    PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, PostQuitMessage, PostThreadMessageW, RegisterClassW,
    SetLayeredWindowAttributes, SetWindowLongPtrW, ShowWindow, TranslateMessage,
    GWLP_USERDATA, HTTRANSPARENT, LWA_ALPHA, MSG, SW_SHOWNOACTIVATE, WM_APP, WM_ERASEBKGND,
    WM_NCHITTEST, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::capture::Rect;
use crate::error::{AppError, Result};

/// Rojo mientras se graba: es el color que todo el mundo lee como «grabando».
const GRABANDO: u32 = rgb(0xEF, 0x44, 0x44);
/// Ambar en pausa, el mismo del punto de la barra.
const EN_PAUSA: u32 = rgb(0xF5, 0x9E, 0x0B);

/// `COLORREF` va al reves de como se escribe un color: azul en el byte alto.
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

/// El color de cada linea vive en la propia ventana (`GWLP_USERDATA`), no en un global: dos
/// marcos vivos a la vez (el de una grabacion que termina y el de la siguiente, o dos
/// pruebas corriendo juntas) no pueden pisarse el color.
fn color_de_la_ventana(hwnd: HWND) -> u32 {
    match unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as u32 {
        0 => GRABANDO,
        color => color,
    }
}

/// Los dos recados que se le mandan al hilo.
const MSG_REPINTAR: u32 = WM_APP + 1;
const MSG_CERRAR: u32 = WM_APP + 2;

/// El marco en pantalla. Se cierra al soltarlo.
pub struct Marco {
    hilo: Option<JoinHandle<()>>,
    /// El hilo del bucle de mensajes, para poder hablarle.
    id_hilo: u32,
    /// Las cuatro ventanas, como numeros: un `HWND` no puede viajar entre hilos y aqui
    /// solo hacen falta para fotografiarlas en las pruebas.
    #[cfg_attr(not(test), allow(dead_code))]
    ventanas: Vec<isize>,
}

impl Marco {
    /// Abre el marco alrededor de la region, con lineas de `grosor` pixeles por fuera.
    ///
    /// Vuelve cuando las cuatro ventanas ya existen, o con el error si alguna no se pudo
    /// crear. Un marco que no se abre no impide grabar: quien llama lo apunta y sigue.
    pub fn abrir(region: Rect, grosor: i32) -> Result<Self> {
        let (aviso_tx, aviso_rx) = mpsc::channel::<Aviso>();
        let hilo = std::thread::Builder::new()
            .name("winshotx-marco".into())
            .spawn(move || bucle(region, grosor.max(1), aviso_tx))
            .map_err(|e| AppError::Msg(format!("no se ha podido abrir el marco: {e}")))?;
        let (id_hilo, ventanas) = aviso_rx
            .recv()
            .map_err(|_| AppError::Msg("el hilo del marco no ha llegado a arrancar".into()))?
            .map_err(AppError::Msg)?;
        Ok(Self {
            hilo: Some(hilo),
            id_hilo,
            ventanas,
        })
    }

    /// Cambia el color: ambar en pausa, rojo grabando.
    pub fn pausado(&self, pausado: bool) {
        let color = if pausado { EN_PAUSA } else { GRABANDO };
        let _ = unsafe {
            PostThreadMessageW(self.id_hilo, MSG_REPINTAR, WPARAM(color as usize), LPARAM(0))
        };
    }
}

impl Drop for Marco {
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.id_hilo, MSG_CERRAR, WPARAM(0), LPARAM(0)) };
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

/// Las cuatro lineas, por fuera de la region: arriba, abajo, izquierda y derecha.
///
/// Va aparte para poder probarla sin abrir nada: es la unica cuenta que hay, y si se
/// equivoca en un pixel el marco tapa el borde de lo que se graba o sale en el video.
pub fn lineas(region: Rect, grosor: i32) -> [Rect; 4] {
    let g = grosor.max(1);
    let (x, y) = (region.x, region.y);
    let (w, h) = (region.width as i32, region.height as i32);
    let ancho_con_esquinas = (w + 2 * g) as u32;
    [
        // Arriba y abajo cubren tambien las esquinas.
        Rect { x: x - g, y: y - g, width: ancho_con_esquinas, height: g as u32 },
        Rect { x: x - g, y: y + h, width: ancho_con_esquinas, height: g as u32 },
        Rect { x: x - g, y, width: g as u32, height: h as u32 },
        Rect { x: x + w, y, width: g as u32, height: h as u32 },
    ]
}

/// Lo que el hilo le cuenta a quien lo abrio: su identificador y sus ventanas, o por que no.
type Aviso = std::result::Result<(u32, Vec<isize>), String>;

/// Lo que corre en el hilo: crea las ventanas, avisa, y atiende mensajes hasta que le
/// digan que cierre.
fn bucle(region: Rect, grosor: i32, aviso: mpsc::Sender<Aviso>) {
    let ventanas = match unsafe { crear_ventanas(region, grosor) } {
        Ok(v) => v,
        Err(e) => {
            let _ = aviso.send(Err(e));
            return;
        }
    };
    // Desde aqui el hilo ya tiene cola de mensajes (la crea la primera ventana), asi que
    // `PostThreadMessage` desde fuera ya no se pierde.
    let _ = aviso.send(Ok((
        unsafe { GetCurrentThreadId() },
        ventanas.iter().map(|h| h.0 as isize).collect(),
    )));

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                MSG_REPINTAR if msg.hwnd.0.is_null() => {
                    for &hwnd in &ventanas {
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, msg.wParam.0 as isize);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                MSG_CERRAR if msg.hwnd.0.is_null() => {
                    for &hwnd in &ventanas {
                        let _ = DestroyWindow(hwnd);
                    }
                    PostQuitMessage(0);
                }
                _ => {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}

/// El nombre de la clase de ventana. Se registra una vez por proceso; las siguientes
/// veces `RegisterClassW` falla con «ya existe» y da igual.
const CLASE: &[u16] = &[
    b'w' as u16, b'i' as u16, b'n' as u16, b's' as u16, b'h' as u16, b'o' as u16, b't' as u16,
    b'x' as u16, b'-' as u16, b'm' as u16, b'a' as u16, b'r' as u16, b'c' as u16, b'o' as u16, 0,
];

unsafe fn crear_ventanas(region: Rect, grosor: i32) -> std::result::Result<Vec<HWND>, String> {
    let instancia = unsafe { GetModuleHandleW(None) }.map_err(|e| e.to_string())?;
    let clase = WNDCLASSW {
        lpfnWndProc: Some(procedimiento),
        hInstance: instancia.into(),
        lpszClassName: PCWSTR(CLASE.as_ptr()),
        ..Default::default()
    };
    // Cero es «no se ha podido», salvo que sea porque ya estaba: entonces vale igual.
    let _ = unsafe { RegisterClassW(&clase) };

    let mut ventanas = Vec::with_capacity(4);
    for linea in lineas(region, grosor) {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
                PCWSTR(CLASE.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                linea.x,
                linea.y,
                linea.width as i32,
                linea.height as i32,
                None,
                None,
                Some(instancia.into()),
                None,
            )
        }
        .map_err(|e| format!("no se ha podido crear una linea del marco: {e}"))?;
        // El color va en la ventana antes de ensennarla: la primera pintada ya sale roja.
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, GRABANDO as isize) };
        // Una ventana `WS_EX_LAYERED` no se pinta hasta que se le dan sus atributos. Opaca
        // del todo: el color ya es lo bastante fino como para no tapar nada.
        let _ = unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) };
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        ventanas.push(hwnd);
    }
    Ok(ventanas)
}

/// Lo unico que saben hacer estas ventanas: pintarse de un color y dejar pasar el raton.
unsafe extern "system" fn procedimiento(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let brocha: HBRUSH = unsafe { CreateSolidBrush(COLORREF(color_de_la_ventana(hwnd))) };
            unsafe {
                FillRect(hdc, &ps.rcPaint, brocha);
                let _ = DeleteObject(brocha.into());
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        // El fondo lo pinta WM_PAINT entero; decir que ya esta borrado evita el parpadeo.
        WM_ERASEBKGND => LRESULT(1),
        // Por si `WS_EX_TRANSPARENT` no bastara: el raton no es nuestro.
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ni un pixel del marco cae dentro de la region: lo que se graba se recorta exacto, y
    /// una linea que se metiera un pixel saldria en todos los fotogramas del video.
    #[test]
    fn el_marco_va_por_fuera_de_la_region() {
        let region = Rect { x: 100, y: 200, width: 640, height: 480 };
        let [arriba, abajo, izquierda, derecha] = lineas(region, 2);
        assert_eq!((arriba.x, arriba.y, arriba.width, arriba.height), (98, 198, 644, 2));
        assert_eq!((abajo.x, abajo.y, abajo.width, abajo.height), (98, 680, 644, 2));
        assert_eq!((izquierda.x, izquierda.y, izquierda.width, izquierda.height), (98, 200, 2, 480));
        assert_eq!((derecha.x, derecha.y, derecha.width, derecha.height), (740, 200, 2, 480));

        let dentro = |r: &Rect| {
            let (x1, y1) = (r.x, r.y);
            let (x2, y2) = (r.x + r.width as i32, r.y + r.height as i32);
            let (rx2, ry2) = (region.x + region.width as i32, region.y + region.height as i32);
            x1 < rx2 && x2 > region.x && y1 < ry2 && y2 > region.y
        };
        for linea in lineas(region, 2) {
            assert!(!dentro(&linea), "la linea {linea:?} pisa la region grabada");
        }
    }

    /// Un grosor de cero o negativo no puede dejar una ventana sin tamanno.
    #[test]
    fn el_grosor_nunca_baja_de_un_pixel() {
        let region = Rect { x: 0, y: 0, width: 10, height: 10 };
        for linea in lineas(region, 0) {
            assert!(linea.width >= 1 && linea.height >= 1);
        }
    }

    /// Abre el marco de verdad, fuera de todas las pantallas para que nadie lo vea, y lo
    /// cierra: comprueba que el hilo arranca, que las ventanas se crean y que cerrar no se
    /// queda colgado esperando a nadie. Es lo unico de este archivo que no se puede saber
    /// leyendo el codigo.
    #[test]
    fn se_abre_y_se_cierra_sin_quedarse_colgado() {
        let region = Rect { x: -20_000, y: -20_000, width: 200, height: 100 };
        let empezo = std::time::Instant::now();
        let marco = Marco::abrir(region, 2).expect("el marco tenía que abrirse");
        assert_eq!(marco.ventanas.len(), 4, "tienen que ser cuatro lineas");
        marco.pausado(true);
        marco.pausado(false);
        drop(marco);
        assert!(
            empezo.elapsed() < std::time::Duration::from_secs(5),
            "abrir y cerrar el marco ha tardado {:?}",
            empezo.elapsed()
        );
    }

    /// El pixel de una linea, pidiendole a la ventana que se pinte en memoria.
    ///
    /// `PrintWindow` le manda a la ventana que se dibuje en un lienzo nuestro, asi que
    /// funciona aunque este fuera de las pantallas. Es la misma tecnica que usa
    /// `scripts/fotografiar-ventanas.ps1` para mirar la app sin tocarle el raton a nadie.
    fn color_de(ventana: isize) -> (u8, u8, u8) {
        use windows::Win32::Graphics::Gdi::{
            CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
            SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        };
        use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};

        let hwnd = HWND(ventana as *mut core::ffi::c_void);
        let (ancho, alto) = (8i32, 4i32);
        unsafe {
            let pantalla = GetDC(None);
            let memoria = CreateCompatibleDC(Some(pantalla));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: ancho,
                    // Negativo: filas de arriba abajo, como las lee cualquiera.
                    biHeight: -alto,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(memoria), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                .expect("no se ha podido crear el lienzo");
            let anterior = SelectObject(memoria, dib.into());
            let pintada = PrintWindow(hwnd, memoria, PRINT_WINDOW_FLAGS(0)).as_bool();
            assert!(pintada, "la ventana no se ha dejado fotografiar");
            let pixel = std::slice::from_raw_parts(bits as *const u8, 4);
            // El DIB va en BGRA.
            let color = (pixel[2], pixel[1], pixel[0]);
            SelectObject(memoria, anterior);
            let _ = DeleteObject(dib.into());
            let _ = DeleteDC(memoria);
            let _ = ReleaseDC(None, pantalla);
            color
        }
    }

    /// Que las lineas se pinten de verdad del color que toca, y que cambien al pausar:
    /// una ventana `WS_EX_LAYERED` sin sus atributos puestos no se pinta nunca, y eso no
    /// lo dice ningun error.
    #[test]
    fn se_pinta_de_rojo_y_en_pausa_de_ambar() {
        let region = Rect { x: -20_000, y: -20_000, width: 200, height: 100 };
        let marco = Marco::abrir(region, 4).expect("el marco tenía que abrirse");
        assert_eq!(color_de(marco.ventanas[0]), (0xEF, 0x44, 0x44), "grabando tiene que ser rojo");
        marco.pausado(true);
        // El repintado es un mensaje al otro hilo: se le da un momento para atenderlo.
        std::thread::sleep(std::time::Duration::from_millis(80));
        assert_eq!(color_de(marco.ventanas[3]), (0xF5, 0x9E, 0x0B), "en pausa tiene que ser ámbar");
        marco.pausado(false);
        std::thread::sleep(std::time::Duration::from_millis(80));
        assert_eq!(color_de(marco.ventanas[1]), (0xEF, 0x44, 0x44));
    }
}
