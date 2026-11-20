//! La captura con scroll en la pantalla de verdad: las fotos, la rueda, la tecla Esc, la
//! barra y el editor del final. Lo de coser está en el módulo de arriba, sin Windows.
//!
//! **Cómo baja sola.** Se deja el ratón dentro de la zona (lo más cerca posible de donde
//! estaba: al soltar el recorte ya está en su esquina) y se manda la rueda como si la
//! girara quien captura. Windows le da la rueda a la ventana que hay debajo del ratón, así
//! que no hace falta saber qué programa es. Cuánto se gira y qué se hace si se pierde lo
//! decide `bajar`, en el módulo de arriba, que se prueba con una página de mentira.
//!
//! **Si quien captura mueve el ratón, la rueda la pone él**: se deja de girar y se sigue
//! fotografiando lo que haga. Y si la página no baja sola (la rueda ha caído en algo que
//! no se desplaza), pasa lo mismo, y la barra se lo dice.
//!
//! **Se para cuando se pide, y se queda con lo que lleve**: «Parar» en la barra, Esc en
//! cualquier sitio, el final de la página bajando sola, o el alto máximo. Solo la X de la
//! barra lo tira.
//!
//! Las fotos son las de siempre (la captura moderna, o GDI de reserva), que no llevan el
//! ratón dentro. El marco va por fuera de la zona y la barra se aparta de la captura, así
//! que ninguno de los dos sale cosido.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use image::RgbaImage;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, SendInput, UnregisterHotKey, INPUT, INPUT_0, INPUT_MOUSE,
    MOD_NOREPEAT, MOUSEEVENTF_WHEEL, MOUSEINPUT, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetCursorPos, PeekMessageW, SetCursorPos, SetForegroundWindow, WindowFromPoint,
    GA_ROOT, MSG, PM_REMOVE, WM_HOTKEY,
};

use super::comun::{Aviso, BARRA_ALTO, BARRA_ANCHO, DESCARTAR, SEGUIR, TERMINAR};
use super::{bajar, sitio_de_la_barra, Cosedor, Pantalla, BARRA_LABEL};
use crate::capture::{MonitorInfo, Rect};
use crate::error::{AppError, Result};

/// Una muesca de la rueda, hacia abajo.
const MUESCA: i32 = -120;

/// Las fotos de la zona.
///
/// El monitor de `xcap` se busca dentro del hilo que lo usa: lleva un puntero de Windows
/// dentro y no se puede pasar de un hilo a otro.
struct Fotografo {
    monitor: xcap::Monitor,
    info: MonitorInfo,
    region: Rect,
}

impl Fotografo {
    fn nuevo(info: &MonitorInfo, region: Rect) -> Result<Self> {
        let monitor = xcap::Monitor::all()
            .map_err(|e| AppError::Msg(e.to_string()))?
            .into_iter()
            .nth(info.id as usize)
            .ok_or_else(|| AppError::Msg("la pantalla de la captura ha desaparecido".into()))?;
        Ok(Self {
            monitor,
            info: info.clone(),
            region,
        })
    }

    fn foto(&self) -> Result<RgbaImage> {
        let pantalla = super::super::fotografiar(&self.monitor, &self.info)?;
        let x = (self.region.x - self.info.x).max(0) as u32;
        let y = (self.region.y - self.info.y).max(0) as u32;
        let ancho = self.region.width.min(pantalla.width().saturating_sub(x));
        let alto = self.region.height.min(pantalla.height().saturating_sub(y));
        if ancho == 0 || alto == 0 {
            return Err(AppError::Msg("la selección queda fuera de la pantalla".into()));
        }
        Ok(image::imageops::crop_imm(&pantalla, x, y, ancho, alto).to_image())
    }
}

/// La tecla Esc mientras dura la captura, la tenga quien la tenga delante.
///
/// Se registra como atajo del sistema, que llega aunque winshotx no tenga el foco (lo
/// tiene la página que baja). Si otro programa ya la tiene cogida, se mira a mano.
struct Esc {
    registrada: bool,
}

const ID_ESC: i32 = 0x5C01;

impl Esc {
    fn coger() -> Self {
        let registrada =
            unsafe { RegisterHotKey(None, ID_ESC, MOD_NOREPEAT, u32::from(VK_ESCAPE.0)) }.is_ok();
        Self { registrada }
    }

    fn pulsada(&self) -> bool {
        if !self.registrada {
            return unsafe { GetAsyncKeyState(i32::from(VK_ESCAPE.0)) } as u16 & 0x8000 != 0;
        }
        let mut pulsada = false;
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) }.as_bool() {
            if msg.wParam.0 as i32 == ID_ESC {
                pulsada = true;
            }
        }
        pulsada
    }
}

impl Drop for Esc {
    fn drop(&mut self) {
        if self.registrada {
            let _ = unsafe { UnregisterHotKey(None, ID_ESC) };
        }
    }
}

/// Si hay que parar, mirando de paso la tecla Esc.
fn hay_que_parar(orden: &AtomicU8, esc: &Esc) -> bool {
    if esc.pulsada() {
        let _ = orden.compare_exchange(SEGUIR, TERMINAR, Ordering::SeqCst, Ordering::SeqCst);
    }
    orden.load(Ordering::SeqCst) != SEGUIR
}

fn raton() -> POINT {
    let mut punto = POINT::default();
    let _ = unsafe { GetCursorPos(&mut punto) };
    punto
}

/// Donde se deja el ratón para girar la rueda: donde esté, metido 16 píxeles dentro de la
/// zona. Al soltar el recorte ya está en una esquina, así que apenas se mueve.
fn punto_dentro(region: Rect, raton: POINT) -> POINT {
    let margen = 16.min(region.width as i32 / 4).min(region.height as i32 / 4);
    POINT {
        x: raton.x.clamp(region.x + margen, region.x + region.width as i32 - 1 - margen),
        y: raton.y.clamp(region.y + margen, region.y + region.height as i32 - 1 - margen),
    }
}

/// Pone el ratón ahí y le da el foco a la ventana de debajo: con «desplazar las ventanas
/// inactivas» apagado en Windows, la rueda va a la que tiene el foco, y el overlay que lo
/// tenía está aparcado fuera de las pantallas.
fn poner_el_raton(punto: POINT) {
    unsafe {
        let _ = SetCursorPos(punto.x, punto.y);
        let debajo = WindowFromPoint(punto);
        if !debajo.is_invalid() {
            let raiz = GetAncestor(debajo, GA_ROOT);
            if !raiz.is_invalid() {
                let _ = SetForegroundWindow(raiz);
            }
        }
    }
}

fn se_ha_movido(desde: POINT) -> bool {
    let ahora = raton();
    (ahora.x - desde.x).abs() + (ahora.y - desde.y).abs() > 6
}

/// Gira la rueda tantas muescas, una detrás de otra, como una rueda de verdad: hacia abajo
/// si son positivas y hacia arriba si son negativas.
fn rueda(muescas: i32) {
    let paso = if muescas >= 0 { MUESCA } else { -MUESCA };
    let una = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: paso as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let todas = vec![una; muescas.unsigned_abs().max(1) as usize];
    unsafe {
        SendInput(&todas, std::mem::size_of::<INPUT>() as i32);
    }
}

/// La pantalla de verdad, para `bajar`.
struct EnVivo<'a> {
    fotografo: Fotografo,
    orden: &'a AtomicU8,
    esc: Esc,
    objetivo: POINT,
    empezo: Instant,
    avisar: Box<dyn FnMut(u32, &'static str) + 'a>,
}

impl Pantalla for EnVivo<'_> {
    fn foto(&mut self) -> Result<RgbaImage> {
        self.fotografo.foto()
    }

    fn rueda(&mut self, muescas: i32) {
        rueda(muescas);
    }

    fn esperar(&mut self, tiempo: Duration) {
        std::thread::sleep(tiempo);
    }

    fn reloj(&self) -> Duration {
        self.empezo.elapsed()
    }

    fn raton_movido(&mut self) -> bool {
        se_ha_movido(self.objetivo)
    }

    fn parar(&mut self) -> bool {
        hay_que_parar(self.orden, &self.esc)
    }

    fn avisar(&mut self, alto: u32, estado: &'static str) {
        (self.avisar)(alto, estado);
    }
}

/// La captura entera. Devuelve la imagen cosida, o nada si se ha descartado.
pub(super) fn capturar(
    app: &AppHandle,
    region: Rect,
    monitor: &MonitorInfo,
    orden: &AtomicU8,
) -> Result<Option<RgbaImage>> {
    // Que el overlay se haya ido de verdad de la pantalla antes de la primera foto: se ha
    // aparcado fuera, pero el compositor puede tener todavía un cuadro con él dentro.
    std::thread::sleep(Duration::from_millis(120));
    let _ = unsafe { DwmFlush() };

    let marco = crate::platform::marco::Marco::abrir(region, f64::from(monitor.scale))
        .map_err(|e| eprintln!("[scroll] sin marco: {e}"))
        .ok();
    let esc = Esc::coger();
    let fotografo = Fotografo::nuevo(monitor, region)?;
    let mut cosedor = Cosedor::nuevo(fotografo.foto()?);
    let mut aviso = Aviso::nuevo(app);

    let objetivo = punto_dentro(region, raton());
    poner_el_raton(objetivo);
    {
        let mut pantalla = EnVivo {
            fotografo,
            orden,
            esc,
            objetivo,
            empezo: Instant::now(),
            avisar: Box::new(|alto, estado| aviso.contar(alto, estado)),
        };
        bajar(&mut pantalla, &mut cosedor);
    }

    drop(marco);
    if orden.load(Ordering::SeqCst) == DESCARTAR {
        return Ok(None);
    }
    aviso.contar(cosedor.alto(), "cosiendo");
    Ok(Some(cosedor.imagen()))
}

/// La barrita de la captura: cómo va, «Terminar» y la X.
pub(super) fn abrir_barra(app: &AppHandle, region: Rect) -> Result<()> {
    let ventana = match app.get_webview_window(BARRA_LABEL) {
        Some(existente) => existente,
        None => {
            let url = WebviewUrl::App("overlay.html?barra=scroll".into());
            let nueva = WebviewWindowBuilder::new(app, BARRA_LABEL, url)
                .additional_browser_args(crate::windows_mgr::NAVEGADOR_ARGS)
                .title("winshotx")
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .shadow(true)
                .focused(false)
                .visible(false)
                .inner_size(BARRA_ANCHO, BARRA_ALTO)
                .build()?;
            crate::platform::window_style::rounded_corners(&nueva);
            // Sin foco nunca: la página que baja tiene que seguir siendo la de delante.
            crate::platform::window_style::never_focus(&nueva);
            if !crate::platform::window_style::fuera_de_la_captura(&nueva, true) {
                eprintln!("[scroll] Windows no ha dejado apartar la barra de las fotos");
            }
            nueva
        }
    };

    let (cx, cy) = region.center();
    let sitio = area_de_trabajo(cx, cy);
    let escala = sitio.map(|(_, escala)| escala).unwrap_or(1.0);
    let (x, y) = sitio_de_la_barra(
        region,
        sitio.map(|(area, _)| area),
        (BARRA_ANCHO * escala) as i32,
        (BARRA_ALTO * escala) as i32,
        (12.0 * escala) as i32,
    );
    let _ = ventana.set_position(PhysicalPosition::new(x, y));
    ventana.show()?;
    Ok(())
}

/// La pantalla de ese punto sin la barra de tareas, y su escalado. Con la pantalla entera,
/// una zona que llega abajo dejaba la barra encima de la barra de tareas, donde no se ve.
fn area_de_trabajo(x: i32, y: i32) -> Option<((i32, i32, i32, i32), f64)> {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        let r = info.rcWork;
        Some((
            (r.left, r.top, r.right - r.left, r.bottom - r.top),
            f64::from(dpi_x) / 96.0,
        ))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn zona() -> Rect {
        Rect { x: 100, y: 200, width: 800, height: 600 }
    }

    #[test]
    fn el_raton_se_queda_donde_esta_si_ya_esta_dentro() {
        let p = punto_dentro(zona(), POINT { x: 400, y: 500 });
        assert_eq!((p.x, p.y), (400, 500));
    }

    #[test]
    fn desde_la_esquina_del_recorte_apenas_entra() {
        // Al soltar el recorte, el ratón está en su esquina de abajo a la derecha.
        let p = punto_dentro(zona(), POINT { x: 900, y: 800 });
        assert_eq!((p.x, p.y), (883, 783));
    }

    /// Lo que hace la rueda de verdad encima de una página: cuánto baja cada muesca, cuánto
    /// tarda en pararse, y qué le parece cada foto al cosedor. Mueve el ratón y gira la rueda
    /// en la pantalla, así que solo con quien la usa avisado.
    /// `WINSHOTX_ZONA=x,y,ancho,alto cargo test --release --lib traza_de_la_rueda -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn traza_de_la_rueda() {
        crate::capture::wgc::como_la_app();
        let zona: Vec<i32> = std::env::var("WINSHOTX_ZONA")
            .expect("WINSHOTX_ZONA=x,y,ancho,alto")
            .split(',')
            .map(|v| v.trim().parse().expect("número"))
            .collect();
        let region = Rect { x: zona[0], y: zona[1], width: zona[2] as u32, height: zona[3] as u32 };
        let monitores = crate::capture::monitors().unwrap();
        let (cx, cy) = region.center();
        let monitor = monitores.iter().find(|m| m.contains(cx, cy)).unwrap().clone();
        println!("captura moderna: {}", crate::capture::moderna_disponible());
        let fotografo = Fotografo::nuevo(&monitor, region).unwrap();
        poner_el_raton(punto_dentro(region, POINT { x: cx, y: cy }));
        for muescas in [1, 1, 2, 4, 8] {
            let mut cosedor = Cosedor::nuevo(fotografo.foto().unwrap());
            rueda(muescas);
            let inicio = Instant::now();
            let mut linea = Vec::new();
            while inicio.elapsed() < Duration::from_millis(1500) {
                let t = Instant::now();
                let foto = fotografo.foto().unwrap();
                let tarda = t.elapsed().as_millis();
                let paso = cosedor.anadir(foto);
                linea.push(format!("{}+{tarda}:{paso:?}", inicio.elapsed().as_millis()));
                std::thread::sleep(Duration::from_millis(40));
            }
            println!("{muescas} muescas ({} px cosidos): {}", cosedor.alto() - region.height, linea.join(" "));
        }
    }

    /// La captura entera en la pantalla de verdad, con el mismo `bajar` que la app y sin
    /// overlay ni barra: deja la imagen cosida en la carpeta temporal y cuenta cada aviso.
    /// Mueve el ratón y gira la rueda, así que solo con quien usa la pantalla avisado.
    /// `WINSHOTX_ZONA=x,y,ancho,alto cargo test --release --lib la_captura_de_verdad -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn la_captura_de_verdad() {
        crate::capture::wgc::como_la_app();
        let zona: Vec<i32> = std::env::var("WINSHOTX_ZONA")
            .expect("WINSHOTX_ZONA=x,y,ancho,alto")
            .split(',')
            .map(|v| v.trim().parse().expect("número"))
            .collect();
        let region = Rect { x: zona[0], y: zona[1], width: zona[2] as u32, height: zona[3] as u32 };
        let monitores = crate::capture::monitors().unwrap();
        let (cx, cy) = region.center();
        let monitor = monitores.iter().find(|m| m.contains(cx, cy)).unwrap().clone();
        let fotografo = Fotografo::nuevo(&monitor, region).unwrap();
        let mut cosedor = Cosedor::nuevo(fotografo.foto().unwrap());
        let objetivo = punto_dentro(
            region,
            POINT { x: region.x + region.width as i32, y: region.y + region.height as i32 },
        );
        poner_el_raton(objetivo);
        let orden = AtomicU8::new(SEGUIR);
        let empezo = Instant::now();
        let mut ultimo = None;
        let mut pantalla = EnVivo {
            fotografo,
            orden: &orden,
            esc: Esc::coger(),
            objetivo,
            empezo,
            avisar: Box::new(|alto, estado| {
                if ultimo != Some((alto, estado)) {
                    ultimo = Some((alto, estado));
                    println!("{:>6} ms  {estado:<8} {alto} px", empezo.elapsed().as_millis());
                }
            }),
        };
        bajar(&mut pantalla, &mut cosedor);
        drop(pantalla);
        let ruta = std::env::temp_dir().join("winshotx-captura-de-verdad.png");
        cosedor.imagen().save(&ruta).unwrap();
        println!("{} fotos cosidas, {} px -> {}", cosedor.cosidas(), cosedor.alto(), ruta.display());
    }

}
