//! El marco que se ve alrededor de lo que se esta grabando.
//!
//! Sin esto, al empezar a grabar el overlay se cierra y **no queda ni un pixel que diga
//! que zona se esta grabando**: solo la barra, debajo. Quien graba un tutorial mueve
//! ventanas, abre menus, y sin marco no sabe si lo que hace cae dentro o fuera hasta que
//! ve el video minutos despues.
//!
//! **Son cuatro ventanas de Windows a pelo, sin webview.** Cada webview cuesta decenas de
//! megabytes y un buen rato en arrancar, y aqui hacen falta cuatro tiras: una arriba, otra
//! abajo y dos a los lados, **por fuera** de la region. Por fuera es lo que las deja fuera
//! del video: la captura recorta exactamente la region, y el marco empieza un pixel mas
//! alla. Cuatro tiras y no una ventana grande con un agujero: lo que se graba no lleva
//! nada encima, ni siquiera transparente.
//!
//! **Cada tira se dibuja pixel a pixel, con su transparencia** (`UpdateLayeredWindow`): una
//! linea nitida de dos pixeles por la escala de la pantalla, separada de la region por un
//! hueco de otros dos, con las esquinas redondeadas y suaves como las de una ventana de
//! Windows 11, y por fuera una sombra corta que la separa de lo de detras (una linea roja
//! sobre una foto roja, sin sombra, no se ve). El marco de antes eran cuatro rectangulos de
//! color pintados con `FillRect`: esquinas en angulo recto, ninguna separacion del fondo, y
//! un primer fotograma sin pintar al aparecer.
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
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    DIB_RGB_COLORS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostQuitMessage, PostThreadMessageW, RegisterClassW, ShowWindow, TranslateMessage,
    UpdateLayeredWindow, HTTRANSPARENT, MSG, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_APP, WM_NCHITTEST,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::capture::Rect;
use crate::error::{AppError, Result};

/// Rojo mientras se graba: es el color que todo el mundo lee como «grabando», y el mismo del
/// punto de la barra (`bg-red-500`).
const GRABANDO: Color = Color(0xEF, 0x44, 0x44);
/// Ambar en pausa, el mismo del punto de la barra en pausa.
const EN_PAUSA: Color = Color(0xF5, 0x9E, 0x0B);

/// Lo oscura que es la sombra pegada a la linea. Se apaga hacia fuera hasta cero.
const SOMBRA: f32 = 0.30;

/// Un color de la linea, en rojo, verde y azul.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Color(u8, u8, u8);

impl Color {
    /// Para viajar en el `WPARAM` de un mensaje.
    fn empaquetar(self) -> usize {
        (self.0 as usize) | ((self.1 as usize) << 8) | ((self.2 as usize) << 16)
    }
    fn desempaquetar(v: usize) -> Self {
        Color(v as u8, (v >> 8) as u8, (v >> 16) as u8)
    }
}

/// Lo que mide el marco en pixeles de la pantalla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Medidas {
    /// Lo que se separa la linea de la region. Sin hueco, la esquina de dentro tendria que ir
    /// en angulo recto: una esquina redonda pegada a la region se meteria en ella.
    hueco: i32,
    /// La linea de color.
    linea: i32,
    /// El radio de la esquina de dentro de la linea (la de fuera es este mas la linea).
    radio: i32,
    /// La sombra, por fuera de la linea.
    sombra: i32,
}

/// Hasta donde puede llegar el radio con un hueco dado sin que la esquina de la region asome
/// por dentro de la linea: el pico de la region queda a `raiz(2) * (radio - hueco)` del centro
/// de la curva, y tiene que caber en el radio. Sale `radio <= hueco * raiz(2) / (raiz(2) - 1)`.
const RADIO_POR_HUECO: f64 = std::f64::consts::SQRT_2 / (std::f64::consts::SQRT_2 - 1.0);

impl Medidas {
    /// A 100 %: dos pixeles de hueco, dos de linea, esquina de seis por dentro (ocho por
    /// fuera, la de las ventanas de Windows 11) y cinco de sombra. Crecen con la escala de la
    /// pantalla y se redondean a pixeles enteros: una linea de 2,5 pixeles no existe, se ve
    /// borrosa.
    fn a_escala(escala: f64) -> Self {
        let e = if escala.is_finite() && escala > 0.0 { escala } else { 1.0 };
        let px = |v: f64, minimo: f64| (v * e).round().max(minimo) as i32;
        let hueco = px(2.0, 1.0);
        Self {
            hueco,
            linea: px(2.0, 1.0),
            radio: px(6.0, 0.0).min((hueco as f64 * RADIO_POR_HUECO).floor() as i32),
            sombra: px(5.0, 2.0),
        }
    }

    /// Lo que sale el marco por fuera de la region, por cada lado.
    fn grosor(self) -> i32 {
        self.hueco + self.linea + self.sombra
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
    /// Abre el marco alrededor de la region, con las medidas de una pantalla a esa `escala`
    /// (1,25 es el 125 % de Windows).
    ///
    /// Vuelve cuando las cuatro ventanas ya existen y estan pintadas, o con el error si
    /// alguna no se pudo crear. Un marco que no se abre no impide grabar: quien llama lo
    /// apunta y sigue.
    pub fn abrir(region: Rect, escala: f64) -> Result<Self> {
        let medidas = Medidas::a_escala(escala);
        let (aviso_tx, aviso_rx) = mpsc::channel::<Aviso>();
        let hilo = std::thread::Builder::new()
            .name("winshotx-marco".into())
            .spawn(move || bucle(region, medidas, aviso_tx))
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
            PostThreadMessageW(self.id_hilo, MSG_REPINTAR, WPARAM(color.empaquetar()), LPARAM(0))
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

/// Las cuatro tiras, por fuera de la region: arriba, abajo, izquierda y derecha.
///
/// Va aparte para poder probarla sin abrir nada: si se equivoca en un pixel el marco tapa
/// el borde de lo que se graba o sale en el video.
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

/// La distancia de un punto al borde de dentro de la linea, en pixeles: negativa por dentro
/// (la region y el hueco) y positiva por fuera. Ese borde es la region agrandada `hueco`
/// pixeles por cada lado, con las esquinas redondeadas de radio `radio`. El punto va contado
/// desde la esquina de arriba a la izquierda de la region, para no hacer cuentas finas con
/// coordenadas de pantalla grandes (el marco de las pruebas vive en -20000).
fn distancia(region: Rect, medidas: Medidas, x: f32, y: f32) -> f32 {
    let (medio_ancho, medio_alto) = (region.width as f32 / 2.0, region.height as f32 / 2.0);
    let (hueco, radio) = (medidas.hueco as f32, medidas.radio as f32);
    let qx = (x - medio_ancho).abs() - (medio_ancho + hueco) + radio;
    let qy = (y - medio_alto).abs() - (medio_alto + hueco) + radio;
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radio
}

/// El dibujo de una tira: cada pixel segun lo lejos que este del borde de dentro de la linea.
/// Sale en BGRA premultiplicado, que es lo que pide `UpdateLayeredWindow`.
///
/// En los tramos rectos, el hueco y la linea son pixeles enteros y sus bordes caen justo
/// entre dos pixeles: la linea sale nitida, sin medio pixel borroso a ningun lado. En las
/// esquinas cada pixel se mira en 4x4 puntos y la curva sale suave, sin escalones.
fn pintar_tira(region: Rect, tira: Rect, medidas: Medidas, color: Color) -> Vec<u8> {
    let linea = medidas.linea as f32;
    let sombra = medidas.sombra.max(1) as f32;
    let (dx, dy) = (tira.x - region.x, tira.y - region.y);
    // Donde empieza la curva de cada esquina, contado desde el centro de la region.
    let (recto_x, recto_y) = (
        region.width as f32 / 2.0 + (medidas.hueco - medidas.radio) as f32,
        region.height as f32 / 2.0 + (medidas.hueco - medidas.radio) as f32,
    );
    let mut bgra = Vec::with_capacity(tira.width as usize * tira.height as usize * 4);
    for fila in 0..tira.height as i32 {
        for col in 0..tira.width as i32 {
            let (x, y) = ((dx + col) as f32, (dy + fila) as f32);
            let d = distancia(region, medidas, x + 0.5, y + 0.5);
            let en_curva = (x + 0.5 - region.width as f32 / 2.0).abs() > recto_x - 1.0
                && (y + 0.5 - region.height as f32 / 2.0).abs() > recto_y - 1.0;
            let cubre = if !en_curva {
                // Tramo recto: el centro del pixel decide, y la linea sale de pixeles enteros.
                if (0.0..=linea).contains(&d) { 1.0 } else { 0.0 }
            } else {
                let mut dentro = 0u32;
                for my in 0..4 {
                    for mx in 0..4 {
                        let (sx, sy) = (x + (mx as f32 + 0.5) / 4.0, y + (my as f32 + 0.5) / 4.0);
                        if (0.0..=linea).contains(&distancia(region, medidas, sx, sy)) {
                            dentro += 1;
                        }
                    }
                }
                dentro as f32 / 16.0
            };
            // La sombra solo va por fuera de la linea: el hueco de dentro queda limpio.
            let oscuro = if d > linea / 2.0 {
                let t = ((d - linea) / sombra).clamp(0.0, 1.0);
                SOMBRA * (1.0 - t) * (1.0 - t)
            } else {
                0.0
            };
            // La linea encima de la sombra. La sombra es negra: solo pone alfa, no color.
            let alfa = cubre + oscuro * (1.0 - cubre);
            let canal = |v: u8| (v as f32 * cubre).round() as u8;
            bgra.extend_from_slice(&[
                canal(color.2),
                canal(color.1),
                canal(color.0),
                (alfa * 255.0).round() as u8,
            ]);
        }
    }
    bgra
}

/// Lo que el hilo le cuenta a quien lo abrio: su identificador y sus ventanas, o por que no.
type Aviso = std::result::Result<(u32, Vec<isize>), String>;

/// Lo que corre en el hilo: crea las ventanas, avisa, y atiende mensajes hasta que le
/// digan que cierre.
fn bucle(region: Rect, medidas: Medidas, aviso: mpsc::Sender<Aviso>) {
    // La region llega en pixeles fisicos. La app ya ve las pantallas asi (lo pone tao al
    // arrancar), pero el hilo no depende de eso: un proceso que no lo pida (las pruebas)
    // veria sus ventanas reescaladas por Windows y borrosas.
    let _ = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let mut color = GRABANDO;
    let ventanas = match unsafe { crear_ventanas(region, medidas, color) } {
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
        ventanas.iter().map(|(h, _)| h.0 as isize).collect(),
    )));

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                MSG_REPINTAR if msg.hwnd.0.is_null() => {
                    let nuevo = Color::desempaquetar(msg.wParam.0);
                    if nuevo != color {
                        color = nuevo;
                        for &(hwnd, tira) in &ventanas {
                            let _ = poner_dibujo(hwnd, tira, &pintar_tira(region, tira, medidas, color));
                        }
                    }
                }
                MSG_CERRAR if msg.hwnd.0.is_null() => {
                    for &(hwnd, _) in &ventanas {
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

unsafe fn crear_ventanas(
    region: Rect,
    medidas: Medidas,
    color: Color,
) -> std::result::Result<Vec<(HWND, Rect)>, String> {
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
    for tira in lineas(region, medidas.grosor()) {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
                PCWSTR(CLASE.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                tira.x,
                tira.y,
                tira.width as i32,
                tira.height as i32,
                None,
                None,
                Some(instancia.into()),
                None,
            )
        }
        .map_err(|e| format!("no se ha podido crear una linea del marco: {e}"))?;
        // El dibujo va en la ventana antes de ensennarla: aparece ya pintada, sin un
        // fotograma vacio, y las cuatro a la vez.
        unsafe { poner_dibujo(hwnd, tira, &pintar_tira(region, tira, medidas, color)) }
            .map_err(|e| format!("no se ha podido pintar una linea del marco: {e}"))?;
        ventanas.push((hwnd, tira));
    }
    for &(hwnd, _) in &ventanas {
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
    }
    Ok(ventanas)
}

/// Le da a la ventana su dibujo (BGRA premultiplicado, del tamanno de la tira) y su sitio.
unsafe fn poner_dibujo(hwnd: HWND, tira: Rect, bgra: &[u8]) -> windows::core::Result<()> {
    let (ancho, alto) = (tira.width as i32, tira.height as i32);
    unsafe {
        let pantalla = GetDC(None);
        let memoria = CreateCompatibleDC(Some(pantalla));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ancho,
                // Negativo: filas de arriba abajo, en el orden en que se han pintado.
                biHeight: -alto,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let resultado = match CreateDIBSection(Some(memoria), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(dib) => {
                let n = (ancho * alto * 4) as usize;
                std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, n.min(bgra.len()));
                let anterior = SelectObject(memoria, dib.into());
                let mezcla = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let sitio = POINT { x: tira.x, y: tira.y };
                let tamanno = SIZE { cx: ancho, cy: alto };
                let origen = POINT { x: 0, y: 0 };
                let hecho = UpdateLayeredWindow(
                    hwnd,
                    Some(pantalla),
                    Some(&sitio),
                    Some(&tamanno),
                    Some(memoria),
                    Some(&origen),
                    COLORREF(0),
                    Some(&mezcla),
                    ULW_ALPHA,
                );
                SelectObject(memoria, anterior);
                let _ = DeleteObject(dib.into());
                hecho
            }
            Err(e) => Err(e),
        };
        let _ = DeleteDC(memoria);
        let _ = ReleaseDC(None, pantalla);
        resultado
    }
}

/// Lo unico que saben hacer estas ventanas: dejar pasar el raton. El dibujo lo guarda
/// Windows desde `UpdateLayeredWindow`, asi que no hay `WM_PAINT` que atender.
unsafe extern "system" fn procedimiento(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
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
        for escala in [1.0, 1.25, 1.5, 2.0] {
            for linea in lineas(region, Medidas::a_escala(escala).grosor()) {
                assert!(!dentro(&linea), "la tira {linea:?} pisa la region grabada a {escala}");
            }
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

    /// La linea crece con la escala de la pantalla, siempre en pixeles enteros (2,5 pixeles
    /// no existen y se verian borrosos), y una escala rota se trata como 100 %.
    #[test]
    fn las_medidas_crecen_con_la_escala_en_pixeles_enteros() {
        let m = |e: f64| {
            let m = Medidas::a_escala(e);
            (m.hueco, m.linea, m.radio, m.sombra)
        };
        assert_eq!(m(1.0), (2, 2, 6, 5));
        assert_eq!(m(1.25), (3, 3, 8, 6));
        assert_eq!(m(1.5), (3, 3, 9, 8));
        assert_eq!(m(1.75), (4, 4, 11, 9));
        assert_eq!(m(2.0), (4, 4, 12, 10));
        assert_eq!(m(0.0), m(1.0));
        assert_eq!(m(f64::NAN), m(1.0));
    }

    /// Los pixeles de una columna de la tira de arriba, de la region hacia fuera:
    /// (rojo, verde, azul, alfa), premultiplicados.
    fn columna(region: Rect, medidas: Medidas, color: Color, col: i32) -> Vec<[u8; 4]> {
        let tira = lineas(region, medidas.grosor())[0];
        let dibujo = pintar_tira(region, tira, medidas, color);
        (0..tira.height as i32)
            .rev()
            .map(|fila| {
                let i = ((fila * tira.width as i32 + col) * 4) as usize;
                [dibujo[i + 2], dibujo[i + 1], dibujo[i], dibujo[i + 3]]
            })
            .collect()
    }

    /// En el tramo recto, de la region hacia fuera: el hueco limpio, la linea de pixeles
    /// enteros y del grosor que toca (tantas filas SEGUIDAS del rojo exacto y opacas como dice
    /// la escala, y ni una mas) y la sombra, que no pone color, solo oscurece, y se apaga hacia
    /// fuera hasta casi nada.
    #[test]
    fn la_linea_es_nitida_y_de_su_grosor_y_la_sombra_se_apaga() {
        let region = Rect { x: 1000, y: 1000, width: 400, height: 300 };
        for escala in [1.0, 1.25, 1.5, 2.0] {
            let medidas = Medidas::a_escala(escala);
            let px = columna(region, medidas, GRABANDO, 200);
            let (hueco, linea) = (medidas.hueco as usize, medidas.linea as usize);
            assert!(px[..hueco].iter().all(|p| *p == [0, 0, 0, 0]), "el hueco, limpio, a {escala}: {px:?}");
            let opacas = px[hueco..].iter().take_while(|p| **p == [0xEF, 0x44, 0x44, 255]).count();
            assert_eq!(opacas, linea, "a {escala}: {px:?}");
            let sombra = &px[hueco + linea..];
            assert_eq!(sombra.len() as i32, medidas.sombra);
            assert!(sombra.iter().all(|p| p[..3] == [0, 0, 0]), "la sombra no pone color: {sombra:?}");
            assert!(sombra[0][3] >= 50, "pegada a la linea se tiene que notar: {sombra:?}");
            assert!(sombra.windows(2).all(|w| w[1][3] < w[0][3]), "se apaga hacia fuera: {sombra:?}");
            assert!(sombra.last().unwrap()[3] <= 10, "y acaba en casi nada: {sombra:?}");
        }
    }

    /// La esquina de la region nunca asoma por dentro de la linea, a ninguna escala: si
    /// asomara, la linea se cortaria en la esquina (las tiras no pueden pintar dentro de la
    /// region). Es lo que limita el radio con el hueco que hay (`RADIO_POR_HUECO`): a 120 %,
    /// por ejemplo, el hueco redondea a 2 y el radio a 7, y con 7 la esquina asomaria.
    #[test]
    fn la_region_queda_dentro_de_la_linea() {
        let region = Rect { x: 0, y: 0, width: 400, height: 300 };
        let mut escala = 0.5;
        while escala <= 3.0 {
            let m = Medidas::a_escala(escala);
            for (x, y) in [(0.0, 0.0), (400.0, 0.0), (0.0, 300.0), (400.0, 300.0)] {
                let d = distancia(region, m, x, y);
                assert!(d <= 0.0, "a {escala} la esquina ({x}, {y}) asoma {d} pixeles: {m:?}");
            }
            escala += 0.05;
        }
    }

    /// Las esquinas son redondas de verdad y suaves: el pixel donde iria la esquina de un
    /// rectangulo queda sin rojo, el pico de fuera de la tira vacio, la curva tiene pixeles a
    /// medias (sin escalones) y la linea mide en la diagonal lo mismo que en los lados. El
    /// marco de antes era un rectangulo de color: en su esquina, rojo entero.
    #[test]
    fn la_esquina_es_redonda_y_suave() {
        let region = Rect { x: 0, y: 0, width: 400, height: 300 };
        for escala in [1.0, 1.25, 1.5, 2.0] {
            let m = Medidas::a_escala(escala);
            let g = m.grosor();
            let tira = lineas(region, g)[0];
            let dibujo = pintar_tira(region, tira, m, GRABANDO);
            // Pixel contado desde la esquina de la region (negativo hacia fuera).
            let pixel = |x: i32, y: i32| {
                let i = (((y + g) * tira.width as i32 + (x + g)) * 4) as usize;
                [dibujo[i + 2], dibujo[i + 1], dibujo[i], dibujo[i + 3]]
            };
            assert_eq!(pixel(-g, -g)[3], 0, "el pico de fuera, vacio, a {escala}");
            let esquina = -(m.hueco + m.linea);
            assert!(
                pixel(esquina, esquina)[0] < 0xEF / 2,
                "donde iria la esquina en angulo recto hay rojo a {escala}: {:?}",
                pixel(esquina, esquina)
            );
            let a_medias = (-g..0)
                .flat_map(|y| (-g..0).map(move |x| (x, y)))
                .filter(|&(x, y)| (1..0xEF).contains(&pixel(x, y)[0]))
                .count();
            assert!(a_medias >= 4, "la curva no tiene borde suave a {escala}");
            let rojos = (1..=g).filter(|&k| pixel(-k, -k)[0] as f32 >= 0xEF as f32 * 0.5).count();
            let diagonal = m.linea as f32 / std::f32::consts::SQRT_2;
            assert!(
                (rojos as f32 - diagonal).abs() <= 1.0,
                "la esquina no tiene el grosor de los lados a {escala}: {rojos} frente a {diagonal}"
            );
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
        let marco = Marco::abrir(region, 1.0).expect("el marco tenía que abrirse");
        assert_eq!(marco.ventanas.len(), 4, "tienen que ser cuatro tiras");
        marco.pausado(true);
        marco.pausado(false);
        drop(marco);
        assert!(
            empezo.elapsed() < std::time::Duration::from_secs(5),
            "abrir y cerrar el marco ha tardado {:?}",
            empezo.elapsed()
        );
    }

    /// Lo que ensenna Windows de cada ventana es su dibujo, en su sitio, y cambia al pausar:
    /// la linea roja, sus esquinas suaves, y ambar en pausa. Una ventana `WS_EX_LAYERED` a la
    /// que no le llega su dibujo no se ve nunca, y eso no lo dice ningun error: por eso se
    /// mira la foto. Se compara el color premultiplicado, que es lo que entrega la foto (ver
    /// `fotos`).
    #[test]
    fn se_ve_su_dibujo_rojo_y_en_pausa_ambar() {
        let region = Rect { x: -20_000, y: -20_000, width: 200, height: 100 };
        let medidas = Medidas::a_escala(1.5);
        let marco = Marco::abrir(region, 1.5).expect("el marco tenía que abrirse");
        let tiras = lineas(region, medidas.grosor());
        let comprobar = |color: Color| {
            for (i, (sitio, bgra)) in fotos(&marco).iter().enumerate() {
                let medida = |r: &Rect| (r.x, r.y, r.width, r.height);
                assert_eq!(medida(sitio), medida(&tiras[i]), "la tira {i} no esta en su sitio");
                let dibujo = pintar_tira(region, tiras[i], medidas, color);
                assert_eq!(bgra.len(), dibujo.len());
                let distintos = bgra
                    .chunks_exact(4)
                    .zip(dibujo.chunks_exact(4))
                    .filter(|(a, b)| a[..3] != b[..3])
                    .count();
                assert_eq!(distintos, 0, "la tira {i} no ensenna su dibujo en {color:?}");
            }
        };
        comprobar(GRABANDO);
        marco.pausado(true);
        // El repintado es un mensaje al otro hilo: se le da un momento para atenderlo.
        std::thread::sleep(std::time::Duration::from_millis(120));
        comprobar(EN_PAUSA);
        marco.pausado(false);
        std::thread::sleep(std::time::Duration::from_millis(120));
        comprobar(GRABANDO);
    }

    /// Foto de cada ventana del marco: su sitio en la pantalla y sus pixeles en BGRA, tal
    /// como los compone Windows (`PW_RENDERFULLCONTENT`), sin que aparezca nada en pantalla.
    ///
    /// Ojo: de una ventana con alfa por pixel, Windows entrega el color ya mezclado sobre
    /// negro (el premultiplicado) y **no toca el byte de alfa** (medido el 12-nov-2026: con el
    /// lienzo lleno de blanco, la sombra sale negra y el alfa sigue en 255). El color se puede
    /// comparar; el alfa, no.
    fn fotos(marco: &Marco) -> Vec<(Rect, Vec<u8>)> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{
            CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
            SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        };
        use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
        use windows::Win32::UI::HiDpi::{
            SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let mut fotos = Vec::new();
        for &ventana in &marco.ventanas {
            let hwnd = HWND(ventana as *mut core::ffi::c_void);
            let mut r = RECT::default();
            unsafe { GetWindowRect(hwnd, &mut r) }.expect("el sitio de la ventana");
            let (ancho, alto) = (r.right - r.left, r.bottom - r.top);
            unsafe {
                let pantalla = GetDC(None);
                let memoria = CreateCompatibleDC(Some(pantalla));
                let info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: ancho,
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
                let pintada = PrintWindow(hwnd, memoria, PRINT_WINDOW_FLAGS(2)).as_bool();
                assert!(pintada, "la ventana no se ha dejado fotografiar");
                let n = (ancho * alto * 4) as usize;
                let pixeles = std::slice::from_raw_parts(bits as *const u8, n).to_vec();
                SelectObject(memoria, anterior);
                let _ = DeleteObject(dib.into());
                let _ = DeleteDC(memoria);
                let _ = ReleaseDC(None, pantalla);
                fotos.push((
                    Rect { x: r.left, y: r.top, width: ancho as u32, height: alto as u32 },
                    pixeles,
                ));
            }
        }
        fotos
    }

    /// Un escritorio de mentira: una pagina blanca con renglones, una foto roja y naranja (lo
    /// peor para ver un marco rojo) y un editor oscuro.
    fn escritorio(ancho: u32, alto: u32) -> image::RgbaImage {
        use image::Rgba;
        let mut img = image::RgbaImage::new(ancho, alto);
        let (a, b) = (ancho * 38 / 100, ancho * 62 / 100);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = if x < a {
                if y % 22 < 7 && x % 260 > 30 && x % 260 < 210 + (y * 7) % 40 {
                    Rgba([206, 210, 217, 255])
                } else {
                    Rgba([250, 250, 250, 255])
                }
            } else if x < b {
                let t = (x - a) as f32 / (b - a) as f32;
                let u = y as f32 / alto as f32;
                Rgba([
                    (230.0 - 60.0 * u) as u8,
                    (60.0 + 90.0 * t * (1.0 - u)) as u8,
                    (40.0 + 80.0 * u) as u8,
                    255,
                ])
            } else if y % 20 < 6 && (x - b) % 300 > 24 && (x - b) % 300 < 120 + (y * 13) % 150 {
                if (y / 20) % 3 == 0 {
                    Rgba([122, 162, 247, 255])
                } else {
                    Rgba([110, 116, 128, 255])
                }
            } else {
                Rgba([30, 31, 34, 255])
            };
        }
        img
    }

    /// El marco encima del escritorio de mentira: cada foto en su sitio, con el color que
    /// entrega Windows (premultiplicado) y el alfa de su dibujo, que la foto no trae (ver
    /// `fotos`; la prueba `se_ve_su_dibujo_rojo_y_en_pausa_ambar` comprueba que el color de
    /// las dos es el mismo).
    fn componer(
        fotos: &[(Rect, Vec<u8>)],
        dibujos: &[Vec<u8>],
        origen: (i32, i32),
        fondo: &mut image::RgbaImage,
    ) {
        for ((sitio, bgra), dibujo) in fotos.iter().zip(dibujos) {
            for fila in 0..sitio.height as i32 {
                for col in 0..sitio.width as i32 {
                    let (x, y) = (sitio.x + col - origen.0, sitio.y + fila - origen.1);
                    if x < 0 || y < 0 || x >= fondo.width() as i32 || y >= fondo.height() as i32 {
                        continue;
                    }
                    let i = ((fila * sitio.width as i32 + col) * 4) as usize;
                    let (b, g, r) = (bgra[i] as f32, bgra[i + 1] as f32, bgra[i + 2] as f32);
                    let a = dibujo[i + 3] as f32 / 255.0;
                    let p = fondo.get_pixel_mut(x as u32, y as u32);
                    let mezcla = |s: f32, d: u8| (s + d as f32 * (1.0 - a)).round().min(255.0) as u8;
                    *p = image::Rgba([mezcla(r, p[0]), mezcla(g, p[1]), mezcla(b, p[2]), 255]);
                }
            }
        }
    }

    /// Un trozo ampliado sin suavizar, para ver los pixeles de una esquina.
    fn ampliar(img: &image::RgbaImage, x: u32, y: u32, lado: u32, veces: u32) -> image::RgbaImage {
        image::RgbaImage::from_fn(lado * veces, lado * veces, |i, j| {
            *img.get_pixel((x + i / veces).min(img.width() - 1), (y + j / veces).min(img.height() - 1))
        })
    }

    /// Fotos del marco de verdad, sin abrir nada en pantalla, a 100, 125 y 150 %: el marco
    /// entero sobre el escritorio de mentira y, a la derecha, sus dos esquinas de arriba
    /// ampliadas diez veces (la de la pagina blanca y la del editor oscuro).
    ///
    /// `WINSHOTX_FOTOS=<carpeta> WINSHOTX_FOTOS_NOMBRE=despues cargo test --lib ver_el_marco -- --ignored --nocapture`
    #[test]
    #[ignore = "no comprueba nada: deja un PNG por escala para mirar"]
    fn ver_el_marco() {
        let carpeta = std::env::var("WINSHOTX_FOTOS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("winshotx-marco"));
        let nombre = std::env::var("WINSHOTX_FOTOS_NOMBRE").unwrap_or_else(|_| "marco".into());
        for escala in [1.0f64, 1.25, 1.5] {
            let e = |v: f64| (v * escala).round() as i32;
            let region = Rect { x: -20_000, y: -20_000, width: e(520.0) as u32, height: e(300.0) as u32 };
            let margen = e(36.0);
            let marco = Marco::abrir(region, escala).expect("el marco tenía que abrirse");
            std::thread::sleep(std::time::Duration::from_millis(120));
            let fotos = fotos(&marco);
            drop(marco);

            let (ancho, alto) = (region.width + 2 * margen as u32, region.height + 2 * margen as u32);
            let mut lienzo = escritorio(ancho, alto);
            let medidas = Medidas::a_escala(escala);
            let dibujos: Vec<_> = lineas(region, medidas.grosor())
                .iter()
                .map(|&tira| pintar_tira(region, tira, medidas, GRABANDO))
                .collect();
            componer(&fotos, &dibujos, (region.x - margen, region.y - margen), &mut lienzo);

            let lado = e(20.0) as u32;
            let veces = 10;
            let izquierda = ampliar(&lienzo, margen as u32 - lado / 2, margen as u32 - lado / 2, lado, veces);
            let derecha = ampliar(
                &lienzo,
                margen as u32 + region.width - lado / 2,
                margen as u32 - lado / 2,
                lado,
                veces,
            );
            let hueco = 16;
            let hoja_ancho = ancho + hueco + izquierda.width() + hueco + derecha.width();
            let hoja_alto = alto.max(izquierda.height());
            let mut hoja = image::RgbaImage::from_pixel(hoja_ancho, hoja_alto, image::Rgba([120, 120, 124, 255]));
            image::imageops::replace(&mut hoja, &lienzo, 0, 0);
            image::imageops::replace(&mut hoja, &izquierda, (ancho + hueco) as i64, 0);
            image::imageops::replace(
                &mut hoja,
                &derecha,
                (ancho + hueco + izquierda.width() + hueco) as i64,
                0,
            );
            let ruta = carpeta.join(format!("{nombre}-{}.png", (escala * 100.0).round() as u32));
            crate::encode::png::save(&hoja, &ruta, hoja.width(), hoja.height()).expect("guardar");
            println!("{}", ruta.display());
        }
    }
}
