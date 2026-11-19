//! El marco que se ve alrededor de lo que se esta grabando.
//!
//! Sin esto, al empezar a grabar el overlay se cierra y **no queda ni un pixel que diga
//! que zona se esta grabando**: solo la barra, debajo. Quien graba un tutorial mueve
//! ventanas, abre menus, y sin marco no sabe si lo que hace cae dentro o fuera hasta que
//! ve el video minutos despues.
//!
//! **Son cuatro ventanas de Windows a pelo, sin webview.** Cada webview cuesta decenas de
//! megabytes y un buen rato en arrancar, y aqui hacen falta cuatro tiras: una arriba, otra
//! abajo y dos a los lados. Cuatro tiras y no una ventana grande con un agujero: lo que se
//! graba no lleva nada encima, ni siquiera transparente.
//!
//! **Por fuera de la region, salvo donde por fuera no hay pantalla.** Por fuera, la captura
//! recorta exactamente la region y el marco empieza un pixel mas alla. Pero un lado pegado al
//! borde de su pantalla, por fuera, cae en la pantalla de al lado o en ninguna: grabando la
//! pantalla entera, el marco salia en el borde de los OTROS monitores y en la grabada no se
//! veia nada (16-nov, «el marco de grabar se bugea»). Ese lado va por dentro, pegado al borde:
//! las tiras estan fuera de toda foto y de todo video (`fuera_de_la_captura_hwnd`), asi que no
//! salen en lo grabado. En un Windows que no deja sacarlas (antes de la 10 2004), nunca van
//! por dentro.
//!
//! **Grabando una ventana, el marco la sigue**: si se mueve o cambia de tamanno, el marco va
//! con ella, y si se minimiza o se cierra, se esconde. Antes se quedaba donde estaba la
//! ventana al empezar.
//!
//! **Cada tira se dibuja pixel a pixel, con su transparencia** (`UpdateLayeredWindow`): una
//! linea nitida de dos pixeles por la escala de la pantalla, separada de la region por un
//! hueco de otros dos, con las esquinas redondeadas y suaves como las de una ventana de
//! Windows 11, y por fuera un halo corto que la separa de lo de detras (una sombra, o la luz
//! de su color, segun el `Estilo`).
//!
//! Las cuatro son de las que **no se pueden pulsar** (`WS_EX_TRANSPARENT`: el clic las
//! atraviesa y llega a lo que haya debajo), **no cogen el foco** (`WS_EX_NOACTIVATE`),
//! no salen en Alt+Tab (`WS_EX_TOOLWINDOW`) y van siempre encima (`WS_EX_TOPMOST`).
//!
//! Viven en **su propio hilo con su propio bucle de mensajes**, como cualquier ventana de
//! Windows: una ventana atiende los mensajes en el hilo que la creo, y el hilo principal
//! esta ocupado con Tauri. Se les habla con `PostThreadMessage` (pausar, cerrar) y el hilo
//! termina cuando se le pide, no antes.

#![cfg(windows)]

use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplayMonitors, GetDC,
    GetMonitorInfoW, MonitorFromRect, ReleaseDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HDC, HMONITOR,
    MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, IsIconic,
    IsWindow, IsWindowVisible, KillTimer, PostQuitMessage, PostThreadMessageW, RegisterClassW,
    SetTimer, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow, HTTRANSPARENT,
    MSG, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA,
    WM_APP, WM_NCHITTEST, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::capture::Rect;
use crate::error::{AppError, Result};

// La figura del marco y su dibujo no dependen de Windows: viven en `marco_comun`, y desde
// aqui se usan igual que antes.
pub use super::marco_comun::{Estilo, ESTILO};
use super::marco_comun::{lados_por_dentro, pintar_tira, Forma, Medidas};

/// Las pantallas, en pixeles fisicos del escritorio (el hilo del marco los ve asi).
fn pantallas() -> Vec<Rect> {
    unsafe extern "system" fn una(monitor: HMONITOR, _: HDC, _: *mut RECT, datos: LPARAM) -> BOOL {
        let lista = unsafe { &mut *(datos.0 as *mut Vec<Rect>) };
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            let r = info.rcMonitor;
            lista.push(Rect {
                x: r.left,
                y: r.top,
                width: (r.right - r.left).max(0) as u32,
                height: (r.bottom - r.top).max(0) as u32,
            });
        }
        true.into()
    }
    let mut lista: Vec<Rect> = Vec::new();
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(una), LPARAM(&raw mut lista as isize)) };
    lista
}

/// La escala de la pantalla donde cae mas de esa region (1,25 es el 125 % de Windows).
fn escala_en(region: Rect) -> f64 {
    let r = RECT {
        left: region.x,
        top: region.y,
        right: region.x + region.width as i32,
        bottom: region.y + region.height as i32,
    };
    let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
    unsafe {
        let monitor = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    }
    f64::from(dpi_x.max(1)) / 96.0
}

/// Donde esta una ventana, del marco REAL que compone el escritorio (el mismo con que se
/// empieza a grabarla), o nada si ya no se ve: cerrada, minimizada, escondida o en otro
/// escritorio virtual.
fn donde_se_ve(ventana: isize) -> Option<Rect> {
    let hwnd = HWND(ventana as *mut core::ffi::c_void);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() || IsIconic(hwnd).as_bool() || !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        let mut oculta = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut oculta).cast(),
            std::mem::size_of::<u32>() as u32,
        );
        if oculta != 0 {
            return None;
        }
        let mut r = RECT::default();
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut r).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
        .ok()?;
        Some(Rect {
            x: r.left,
            y: r.top,
            width: (r.right - r.left).max(1) as u32,
            height: (r.bottom - r.top).max(1) as u32,
        })
    }
}

/// Los recados que se le mandan al hilo.
const MSG_PAUSAR: u32 = WM_APP + 1;
const MSG_CERRAR: u32 = WM_APP + 2;

/// Cada cuanto se mira donde esta la ventana que se graba: lo que tarda un fotograma a 60, para
/// que al arrastrarla el marco vaya con ella y no detras.
const SEGUIR_CADA_MS: u32 = 16;

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
        Self::abrir_con(region, escala, None, ESTILO)
    }

    /// El marco de una ventana que se graba: empieza alrededor de `region` (donde estaba la
    /// ventana al empezar) y la sigue si se mueve, cambia de tamanno o se esconde.
    pub fn abrir_en_ventana(region: Rect, escala: f64, ventana: isize) -> Result<Self> {
        Self::abrir_con(region, escala, Some(ventana), ESTILO)
    }

    fn abrir_con(region: Rect, escala: f64, seguir: Option<isize>, estilo: Estilo) -> Result<Self> {
        let (aviso_tx, aviso_rx) = mpsc::channel::<Aviso>();
        let hilo = std::thread::Builder::new()
            .name("winshotx-marco".into())
            .spawn(move || bucle(region, escala, seguir, estilo, aviso_tx))
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

    /// Cambia el color: el de pausa en pausa, el de grabar grabando.
    pub fn pausado(&self, pausado: bool) {
        let _ = unsafe {
            PostThreadMessageW(self.id_hilo, MSG_PAUSAR, WPARAM(usize::from(pausado)), LPARAM(0))
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

/// Lo que el hilo le cuenta a quien lo abrio: su identificador y sus ventanas, o por que no.
type Aviso = std::result::Result<(u32, Vec<isize>), String>;

/// Las cuatro ventanas del marco y lo que ensennan, dentro de su hilo.
struct Dibujo {
    ventanas: [HWND; 4],
    estilo: Estilo,
    pausado: bool,
    /// Si las tiras estan fuera de toda foto y de todo video. Sin eso, ningun lado puede ir
    /// por dentro: saldria en lo grabado.
    fuera_del_video: bool,
    forma: Forma,
    a_la_vista: bool,
}

impl Dibujo {
    /// Crea las cuatro ventanas, les da su dibujo y las ensenna.
    unsafe fn crear(region: Rect, escala: f64, estilo: Estilo) -> std::result::Result<Self, String> {
        let instancia = unsafe { GetModuleHandleW(None) }.map_err(|e| e.to_string())?;
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: PCWSTR(CLASE.as_ptr()),
            ..Default::default()
        };
        // Cero es «no se ha podido», salvo que sea porque ya estaba: entonces vale igual.
        let _ = unsafe { RegisterClassW(&clase) };

        let mut ventanas = [HWND::default(); 4];
        let mut fuera_del_video = true;
        for ventana in &mut ventanas {
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
                    PCWSTR(CLASE.as_ptr()),
                    PCWSTR::null(),
                    WS_POPUP,
                    region.x,
                    region.y,
                    1,
                    1,
                    None,
                    None,
                    Some(instancia.into()),
                    None,
                )
            }
            .map_err(|e| format!("no se ha podido crear una linea del marco: {e}"))?;
            // Fuera de toda foto y de todo video: desde el 16-nov se puede capturar grabando
            // (la linea salia en la foto) y bajar con la captura con scroll grabando (salia en
            // el video), y es lo que deja poner por dentro un lado pegado al borde.
            fuera_del_video &= crate::platform::window_style::fuera_de_la_captura_hwnd(hwnd.0 as isize, true);
            *ventana = hwnd;
        }
        let mut dibujo = Self {
            ventanas,
            estilo,
            pausado: false,
            fuera_del_video,
            forma: Forma::nueva(region, Medidas::a_escala(escala, estilo), [false; 4]),
            a_la_vista: false,
        };
        dibujo.forma = dibujo.forma_de(region, escala);
        // El dibujo va en la ventana antes de ensennarla: aparece ya pintada, sin un
        // fotograma vacio, y las cuatro a la vez.
        unsafe { dibujo.pintar() }.map_err(|e| format!("no se ha podido pintar una linea del marco: {e}"))?;
        unsafe { dibujo.ver(true) };
        Ok(dibujo)
    }

    /// La forma del marco para esa region, con los lados por dentro donde por fuera no hay
    /// pantalla (si se puede).
    fn forma_de(&self, region: Rect, escala: f64) -> Forma {
        let medidas = Medidas::a_escala(escala, self.estilo);
        let dentro = if self.fuera_del_video {
            lados_por_dentro(region, medidas.grosor(), &pantallas())
        } else {
            [false; 4]
        };
        Forma::nueva(region, medidas, dentro)
    }

    /// Le da a cada ventana su dibujo, su tamanno y su sitio.
    unsafe fn pintar(&self) -> windows::core::Result<()> {
        for (hwnd, tira) in self.ventanas.iter().zip(self.forma.tiras) {
            unsafe { poner_dibujo(*hwnd, tira, &pintar_tira(&self.forma, tira, self.estilo, self.pausado)) }?;
        }
        Ok(())
    }

    /// Lleva el marco a otra forma: si solo cambia el sitio, mueve las ventanas; si no, las
    /// repinta.
    unsafe fn poner(&mut self, forma: Forma) {
        if forma == self.forma {
            return;
        }
        let mover = forma.misma_figura(&self.forma);
        self.forma = forma;
        if mover {
            for (hwnd, tira) in self.ventanas.iter().zip(forma.tiras) {
                let _ = unsafe {
                    SetWindowPos(*hwnd, None, tira.x, tira.y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)
                };
            }
        } else {
            let _ = unsafe { self.pintar() };
        }
    }

    unsafe fn pausar(&mut self, pausado: bool) {
        if pausado != self.pausado {
            self.pausado = pausado;
            let _ = unsafe { self.pintar() };
        }
    }

    unsafe fn ver(&mut self, si: bool) {
        if si != self.a_la_vista {
            self.a_la_vista = si;
            for hwnd in self.ventanas {
                let _ = unsafe { ShowWindow(hwnd, if si { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
            }
        }
    }

    unsafe fn cerrar(&self) {
        for hwnd in self.ventanas {
            let _ = unsafe { DestroyWindow(hwnd) };
        }
    }
}

/// La ventana que se graba y donde estaba al empezar, para llevar el marco con ella.
struct Seguir {
    ventana: isize,
    /// La region que se graba al empezar, y donde estaba la ventana entonces: la region se
    /// mueve y crece lo mismo que la ventana.
    region: Rect,
    al_empezar: Rect,
}

impl Seguir {
    /// Donde va ahora la region, o nada si la ventana no se ve.
    fn region_ahora(&self) -> Option<Rect> {
        let ahora = donde_se_ve(self.ventana)?;
        let (dw, dh) = (
            ahora.width as i64 - self.al_empezar.width as i64,
            ahora.height as i64 - self.al_empezar.height as i64,
        );
        Some(Rect {
            x: self.region.x + (ahora.x - self.al_empezar.x),
            y: self.region.y + (ahora.y - self.al_empezar.y),
            width: (self.region.width as i64 + dw).max(1) as u32,
            height: (self.region.height as i64 + dh).max(1) as u32,
        })
    }
}

/// Lo que corre en el hilo: crea las ventanas, avisa, y atiende mensajes hasta que le
/// digan que cierre.
fn bucle(region: Rect, escala: f64, seguir: Option<isize>, estilo: Estilo, aviso: mpsc::Sender<Aviso>) {
    // La region llega en pixeles fisicos. La app ya ve las pantallas asi (lo pone tao al
    // arrancar), pero el hilo no depende de eso: un proceso que no lo pida (las pruebas)
    // veria sus ventanas reescaladas por Windows y borrosas.
    let _ = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let mut dibujo = match unsafe { Dibujo::crear(region, escala, estilo) } {
        Ok(d) => d,
        Err(e) => {
            let _ = aviso.send(Err(e));
            return;
        }
    };
    let seguir = seguir.and_then(|ventana| {
        Some(Seguir { ventana, region, al_empezar: donde_se_ve(ventana)? })
    });
    let reloj = if seguir.is_some() { unsafe { SetTimer(None, 0, SEGUIR_CADA_MS, None) } } else { 0 };
    // Desde aqui el hilo ya tiene cola de mensajes (la crea la primera ventana), asi que
    // `PostThreadMessage` desde fuera ya no se pierde.
    let _ = aviso.send(Ok((
        unsafe { GetCurrentThreadId() },
        dibujo.ventanas.iter().map(|h| h.0 as isize).collect(),
    )));

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                MSG_PAUSAR if msg.hwnd.0.is_null() => dibujo.pausar(msg.wParam.0 != 0),
                MSG_CERRAR if msg.hwnd.0.is_null() => {
                    if reloj != 0 {
                        let _ = KillTimer(None, reloj);
                    }
                    dibujo.cerrar();
                    PostQuitMessage(0);
                }
                WM_TIMER if msg.hwnd.0.is_null() => {
                    if let Some(seguir) = &seguir {
                        match seguir.region_ahora() {
                            Some(region) => {
                                let forma = dibujo.forma_de(region, escala_en(region));
                                dibujo.poner(forma);
                                dibujo.ver(true);
                            }
                            None => dibujo.ver(false),
                        }
                    }
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
#[path = "marco_pruebas.rs"]
mod tests;
