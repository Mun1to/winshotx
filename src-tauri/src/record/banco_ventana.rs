//! Una pantalla de mentira para medir la captura sin grabar la de nadie.
//!
//! Los bancos del anillo necesitan que Windows entregue fotogramas de verdad (la copia de
//! la GPU, la textura, el mapeo: lo que cuesta de verdad), y hasta S29 eso solo se podia
//! medir grabando la pantalla del raton con un marco parpadeando encima. Aqui se abre una
//! ventana de 1920x1080 casi entera FUERA de las pantallas, se le pinta algo que se mueve
//! y se graba ESA ventana con la misma captura de Windows: los mismos costes, el escritorio
//! de quien lo corre ni se entera.
//!
//! **Trampa medida (S29):** fuera de las pantallas (en -20000) Windows deja de componerla y
//! la captura entrega UN fotograma y nada mas, por mucho que se pinte; y asomando una sola
//! columna por el borde, igual. Lo que funciona es dejarla DENTRO de una pantalla con
//! opacidad 1/255, detras de todas las ventanas y sin coger el raton: a la vista no hay
//! nada y Windows la compone entera.
//!
//! El pintor vive en su propio hilo, que es el dueño de la ventana y el unico que puede
//! atender sus mensajes (ver la trampa de `SetWindowPos` en `win.rs`). Mide su propio
//! tiempo de CPU para que el banco lo pueda restar del proceso: lo que se quiere saber es lo
//! que gasta winshotx capturando, no lo que gasta la prueba pintando.
#![cfg(all(test, windows))]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EndPaint,
    InvalidateRect, SelectObject, UpdateWindow, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP, HDC, PAINTSTRUCT, SRCCOPY,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetWindowLongPtrW,
    PeekMessageW, RegisterClassW, SetWindowLongPtrW, ShowWindow, TranslateMessage, GWLP_USERDATA,
    MSG, PM_REMOVE, SW_SHOWNOACTIVATE, WM_PAINT, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_POPUP,
};

/// Que se mueve en la pantalla de mentira.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escena {
    /// Nada. Windows no deberia entregar casi nada.
    Quieta,
    /// Un cuadrito de 24x24 que se enciende y se apaga en cada vuelta, en una esquina: lo
    /// que hacia el banco de septiembre con el marco, y lo que hace un cursor de texto o un
    /// icono girando en la bandeja.
    Parpadeo,
    /// Un cuadrado de 32x32 que cruza la pantalla, como el raton.
    Raton,
    /// Un trozo de 640x360 que cambia entero en cada vuelta, como un video en una esquina.
    Video,
    /// Las tres cosas a la vez: un raton, un parpadeo lejos y un video. Lo que tiene un
    /// escritorio de trabajo con un terminal escribiendo y el raton moviendose.
    Todo,
}

/// Opacidad de la ventana, de 255. Con 1 no se ve; con 0 Windows la da por invisible y no la
/// compone. Se puede subir con `WINSHOTX_OPACIDAD` para mirarla.
fn opacidad() -> u8 {
    static OPACIDAD: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *OPACIDAD.get_or_init(|| std::env::var("WINSHOTX_OPACIDAD").ok().and_then(|v| v.parse().ok()).unwrap_or(1))
}

pub const ANCHO: u32 = 1920;
pub const ALTO: u32 = 1080;

/// La ventana pintandose, y como pararla.
pub struct Pantalla {
    pub hwnd: isize,
    parar: Arc<AtomicBool>,
    hilo: Option<std::thread::JoinHandle<()>>,
    /// CPU que lleva gastada el pintor, en nanosegundos, para restarla.
    cpu_pintor_ns: Arc<AtomicU64>,
    /// Vueltas pintadas: dice cuantos cambios ha habido.
    pub vueltas: Arc<AtomicU64>,
}

impl Pantalla {
    /// Abre la ventana fuera de las pantallas y empieza a pintar a ~60 vueltas por segundo.
    pub fn abrir(escena: Escena) -> Self {
        let parar = Arc::new(AtomicBool::new(false));
        let cpu = Arc::new(AtomicU64::new(0));
        let vueltas = Arc::new(AtomicU64::new(0));
        let (tx, rx) = std::sync::mpsc::channel::<isize>();
        let hilo = {
            let parar = parar.clone();
            let cpu = cpu.clone();
            let vueltas = vueltas.clone();
            std::thread::spawn(move || pintar(escena, tx, &parar, &cpu, &vueltas))
        };
        let hwnd = rx.recv().expect("el pintor no ha abierto la ventana");
        // Que DWM la tenga compuesta antes de que nadie la grabe.
        std::thread::sleep(Duration::from_millis(300));
        Self {
            hwnd,
            parar,
            hilo: Some(hilo),
            cpu_pintor_ns: cpu,
            vueltas,
        }
    }

    pub fn cpu_pintor(&self) -> Duration {
        Duration::from_nanos(self.cpu_pintor_ns.load(Ordering::Relaxed))
    }
}

impl Drop for Pantalla {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

/// CPU gastada por el hilo que llama, en nanosegundos.
pub fn cpu_del_hilo() -> u64 {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentThread, GetThreadTimes};
    let (mut a, mut b, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe {
        let _ = GetThreadTimes(GetCurrentThread(), &mut a, &mut b, &mut k, &mut u);
    }
    let cien = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    (cien(k) + cien(u)) * 100
}

/// Lo que se pinta: un fondo con algo de textura (ni liso, que QOI lo regala, ni ruido,
/// que es el peor caso de todo) en un DIB que la ventana copia cuando le toca pintar.
struct Lienzo {
    dc: HDC,
    bitmap: HBITMAP,
    bits: *mut u8,
}

impl Lienzo {
    fn nuevo() -> Self {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ANCHO as i32,
                // Negativo: las filas de arriba abajo, como las da la captura.
                biHeight: -(ALTO as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let dc = unsafe { CreateCompatibleDC(None) };
        let bitmap = unsafe { CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) }
            .expect("sin DIB");
        unsafe { SelectObject(dc, bitmap.into()) };
        let lienzo = Self {
            dc,
            bitmap,
            bits: bits.cast(),
        };
        lienzo.fondo();
        lienzo
    }

    fn pixeles(&self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.bits, (ANCHO * ALTO * 4) as usize) }
    }

    /// Un escritorio inventado: degradado suave, «ventanas» lisas y bloques de «texto».
    fn fondo(&self) {
        let px = self.pixeles();
        let mut s = 0x1234_5678u32;
        for y in 0..ALTO {
            for x in 0..ANCHO {
                let p = ((y * ANCHO + x) * 4) as usize;
                let base = [(x / 8) as u8, (y / 5) as u8, 90u8];
                px[p..p + 3].copy_from_slice(&base);
                px[p + 3] = 255;
            }
        }
        // Ventanas lisas con lineas de «texto».
        for ventana in 0..6u32 {
            let (vx, vy) = (80 + ventana * 290, 60 + (ventana % 3) * 300);
            for y in vy..(vy + 260).min(ALTO) {
                for x in vx..(vx + 270).min(ANCHO) {
                    let p = ((y * ANCHO + x) * 4) as usize;
                    s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let texto = (y - vy) % 14 < 9 && (s >> 24) < 90;
                    let v = if texto { 30 } else { 245 };
                    px[p..p + 3].copy_from_slice(&[v, v, v]);
                }
            }
        }
    }

    fn rellenar(&self, x0: u32, y0: u32, w: u32, h: u32, color: [u8; 3]) {
        let px = self.pixeles();
        for y in y0..(y0 + h).min(ALTO) {
            for x in x0..(x0 + w).min(ANCHO) {
                let p = ((y * ANCHO + x) * 4) as usize;
                px[p..p + 3].copy_from_slice(&color);
            }
        }
    }

    fn ruido(&self, x0: u32, y0: u32, w: u32, h: u32, semilla: u32) {
        let px = self.pixeles();
        let mut s = semilla.wrapping_mul(2_654_435_761).wrapping_add(7);
        for y in y0..(y0 + h).min(ALTO) {
            for x in x0..(x0 + w).min(ANCHO) {
                let p = ((y * ANCHO + x) * 4) as usize;
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                // Un «video» no es ruido blanco: bloques de 4x4 con el mismo color.
                let v = ((x / 4 + y / 4 + semilla) * 37 ^ (s >> 28)) as u8;
                px[p..p + 3].copy_from_slice(&[v, v.wrapping_mul(3), v.wrapping_add(80)]);
            }
        }
    }
}

impl Drop for Lienzo {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

unsafe extern "system" fn procedimiento(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_PAINT {
        let lienzo = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Lienzo;
        let mut ps = PAINTSTRUCT::default();
        let dc = unsafe { BeginPaint(hwnd, &mut ps) };
        if !lienzo.is_null() {
            let r = ps.rcPaint;
            unsafe {
                let _ = BitBlt(
                    dc,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    Some((*lienzo).dc),
                    r.left,
                    r.top,
                    SRCCOPY,
                );
            }
        }
        unsafe {
            let _ = EndPaint(hwnd, &ps);
        }
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}

fn pintar(
    escena: Escena,
    tx: std::sync::mpsc::Sender<isize>,
    parar: &AtomicBool,
    cpu: &AtomicU64,
    vueltas: &AtomicU64,
) {
    use windows::Win32::UI::HiDpi::{
        SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    // Sin esto, en una pantalla al 125 % Windows estiraria la ventana y lo grabado no
    // mediria 1920x1080.
    unsafe {
        let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    const CLASE: &[u16] = &[
        b'w' as u16, b's' as u16, b'x' as u16, b'-' as u16, b'b' as u16, b'a' as u16, b'n' as u16,
        b'c' as u16, b'o' as u16, 0,
    ];
    let instancia = unsafe { GetModuleHandleW(None) }.expect("sin instancia");
    let clase = WNDCLASSW {
        hInstance: instancia.into(),
        lpszClassName: PCWSTR(CLASE.as_ptr()),
        lpfnWndProc: Some(procedimiento),
        ..Default::default()
    };
    let _ = unsafe { RegisterClassW(&clase) };
    let lienzo = Box::new(Lienzo::nuevo());
    // Dentro de la pantalla que este mas a la izquierda, pero con opacidad 1/255 (ver
    // arriba): a la vista no hay nada, y no coge el raton.
    let (x, y) = crate::capture::monitors()
        .ok()
        .and_then(|m| m.into_iter().min_by_key(|m| m.x))
        .map(|m| (m.x, m.y))
        .unwrap_or((0, 0));
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW
                | WS_EX_NOACTIVATE
                | windows::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED
                | windows::Win32::UI::WindowsAndMessaging::WS_EX_TRANSPARENT,
            PCWSTR(CLASE.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            x,
            y,
            ANCHO as i32,
            ALTO as i32,
            None,
            None,
            Some(instancia.into()),
            None,
        )
    }
    .expect("no se ha podido abrir la pantalla de mentira");
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &*lienzo as *const Lienzo as isize);
        let _ = windows::Win32::UI::WindowsAndMessaging::SetLayeredWindowAttributes(
            hwnd,
            windows::Win32::Foundation::COLORREF(0),
            opacidad(),
            windows::Win32::UI::WindowsAndMessaging::LWA_ALPHA,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        // Detras de todo: la columna que asoma no tapa nada de nadie.
        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowPos(
            hwnd,
            Some(windows::Win32::UI::WindowsAndMessaging::HWND_BOTTOM),
            0,
            0,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                | windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
        );
        let _ = UpdateWindow(hwnd);
    }
    let _ = tx.send(hwnd.0 as isize);

    let cpu0 = cpu_del_hilo();
    let mut vuelta = 0u32;
    let mut siguiente = Instant::now();
    let bombear = || {
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    };
    let invalidar = |x: u32, y: u32, w: u32, h: u32| {
        let r = RECT {
            left: x as i32,
            top: y as i32,
            right: (x + w) as i32,
            bottom: (y + h) as i32,
        };
        // Y se pinta YA, cada trozo por su lado: si se acumularan, `BeginPaint` daria la
        // caja que los abarca a todos y se repintaria media pantalla (que es lo que Windows
        // diria, con razon, que cambio). Una app de verdad pinta lo suyo.
        unsafe {
            let _ = InvalidateRect(Some(hwnd), Some(&r), false);
            let _ = UpdateWindow(hwnd);
        }
    };
    while !parar.load(Ordering::Relaxed) {
        bombear();
        let ahora = Instant::now();
        if ahora < siguiente {
            std::thread::sleep((siguiente - ahora).min(Duration::from_millis(4)));
            continue;
        }
        siguiente += Duration::from_micros(16_667);
        if ahora > siguiente + Duration::from_millis(50) {
            siguiente = ahora;
        }
        vuelta = vuelta.wrapping_add(1);
        let parpadeo = |lienzo: &Lienzo| {
            let color = if vuelta % 2 == 0 { [0, 0, 255] } else { [255, 255, 0] };
            lienzo.rellenar(ANCHO - 60, 30, 24, 24, color);
            invalidar(ANCHO - 60, 30, 24, 24);
        };
        let raton = |lienzo: &Lienzo| {
            // Borra donde estaba (con el fondo de esa franja: se repinta la fila entera de
            // 40 px de alto) y lo pone 6 px mas alla.
            let paso = vuelta % 280;
            let (x, y) = (100 + paso * 6, 700 + (paso % 40) * 3);
            let (xa, ya) = (100 + paso.saturating_sub(1) * 6, 700 + (paso.saturating_sub(1) % 40) * 3);
            lienzo.rellenar(xa, ya, 32, 32, [(xa / 8) as u8, (ya / 5) as u8, 90]);
            invalidar(xa, ya, 32, 32);
            lienzo.rellenar(x, y, 32, 32, [255, 255, 255]);
            invalidar(x, y, 32, 32);
        };
        let video = |lienzo: &Lienzo| {
            lienzo.ruido(1200, 600, 640, 360, vuelta);
            invalidar(1200, 600, 640, 360);
        };
        match escena {
            Escena::Quieta => {}
            Escena::Parpadeo => parpadeo(&lienzo),
            Escena::Raton => raton(&lienzo),
            Escena::Video => video(&lienzo),
            Escena::Todo => {
                parpadeo(&lienzo);
                raton(&lienzo);
                video(&lienzo);
            }
        }
        if escena != Escena::Quieta {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
            vueltas.fetch_add(1, Ordering::Relaxed);
        }
        cpu.store(cpu_del_hilo() - cpu0, Ordering::Relaxed);
    }
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        let _ = DestroyWindow(hwnd);
    }
    drop(lienzo);
}
