//! La captura con scroll en la pantalla de verdad: las fotos, la rueda, la tecla Esc, la
//! barra y el editor del final. Lo de coser está en el módulo de arriba, sin Windows.
//!
//! **Cómo baja sola.** Se deja el ratón dentro de la zona (lo más cerca posible de donde
//! estaba: al soltar el recorte ya está en su esquina) y se manda la rueda como si la
//! girara quien captura, una muesca o varias. Windows le da la rueda a la ventana que hay
//! debajo del ratón, así que no hace falta saber qué programa es. Se fotografía hasta que
//! la página se queda quieta y se vuelve a girar; cada muesca baja lo que diga ese
//! programa, así que lo que baja la primera se mide y las siguientes se ajustan para bajar
//! algo más de media zona de una vez.
//!
//! **Si quien captura mueve el ratón, la rueda la pone él**: se deja de girar y se sigue
//! fotografiando lo que haga. Y si la página no baja sola (la rueda ha caído en algo que
//! no se desplaza), pasa lo mismo, y la barra se lo dice.
//!
//! **Se para cuando se pide, y se queda con lo que lleve**: «Terminar» en la barra, Esc
//! en cualquier sitio, el final de la página bajando sola, o el alto máximo. Solo la X de
//! la barra lo tira.
//!
//! Las fotos son las de siempre (la captura moderna, o GDI de reserva), que no llevan el
//! ratón dentro. El marco va por fuera de la zona y la barra se aparta de la captura, así
//! que ninguno de los dos sale cosido.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
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

use super::{sitio_de_la_barra, Cosedor, Paso, BARRA_LABEL, EVENTO_PROGRESO};
use crate::capture::{MonitorInfo, Rect};
use crate::error::{AppError, Result};

const SEGUIR: u8 = 0;
pub(super) const TERMINAR: u8 = 1;
pub(super) const DESCARTAR: u8 = 2;

/// La orden de la captura que está en marcha, si hay una. Solo puede haber una.
static EN_MARCHA: Mutex<Option<Arc<AtomicU8>>> = Mutex::new(None);

/// Lo que mide la barra, en píxeles lógicos.
const BARRA_ANCHO: f64 = 420.0;
const BARRA_ALTO: f64 = 52.0;

/// Una muesca de la rueda, hacia abajo.
const MUESCA: i32 = -120;
/// Lo que se espera entre foto y foto mientras la página baja.
const ENTRE_FOTOS: Duration = Duration::from_millis(40);
/// Lo más que se espera a que la página se quede quieta tras girar la rueda: una página
/// con algo animado no se queda quieta nunca.
const ESPERA_MAXIMA: Duration = Duration::from_millis(1500);
/// Lo que se espera antes de dar por quieta la página: el desplazamiento suave tarda
/// un poco en arrancar.
const ARRANQUE: Duration = Duration::from_millis(200);

/// Cómo va la captura, para la barra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progreso {
    /// El alto de la imagen cosida, en píxeles.
    alto: u32,
    /// `bajando` (sola), `tu` (quien captura desplaza), `perdida` (ha bajado demasiado
    /// de golpe), `no-baja` (la rueda no mueve nada) o `cosiendo` (terminando).
    estado: &'static str,
}

/// La primera orden es la que vale: «Terminar» seguido de un clic en la X no tira lo que
/// ya se está guardando.
pub(super) fn pedir(orden: u8) {
    if let Some(actual) = EN_MARCHA.lock().as_ref() {
        let _ = actual.compare_exchange(SEGUIR, orden, Ordering::SeqCst, Ordering::SeqCst);
    }
}

pub(super) fn iniciar(app: &AppHandle, region: Rect) -> Result<()> {
    if region.width < 40 || region.height < 80 {
        return Err(AppError::Msg(
            "Esa zona es muy pequeña para desplazarla: recorta algo más grande.".into(),
        ));
    }
    let monitores = crate::capture::monitors()?;
    let (cx, cy) = region.center();
    let monitor = monitores
        .iter()
        .find(|m| m.contains(cx, cy))
        .cloned()
        .ok_or_else(|| AppError::Msg("la selección queda fuera de la pantalla".into()))?;
    let region = dentro_de(region, &monitor);

    let orden = Arc::new(AtomicU8::new(SEGUIR));
    {
        let mut en_marcha = EN_MARCHA.lock();
        if en_marcha.is_some() {
            return Err(AppError::Msg("Ya hay una captura con scroll en marcha.".into()));
        }
        *en_marcha = Some(orden.clone());
    }

    crate::windows_mgr::close_overlays(app);
    // Sin barra se puede capturar igual (Esc termina), así que no se para por eso.
    if let Err(e) = abrir_barra(app, region) {
        eprintln!("[scroll] no se ha podido abrir la barra: {e}");
    }

    let handle = app.clone();
    let lanzado = std::thread::Builder::new()
        .name("winshotx-scroll".into())
        .spawn(move || {
            let resultado = capturar(&handle, region, &monitor, &orden);
            cerrar_barra(&handle);
            *EN_MARCHA.lock() = None;
            match resultado {
                Ok(Some(imagen)) => entregar(&handle, &imagen, region),
                Ok(None) => {}
                Err(e) => eprintln!("[scroll] la captura ha fallado: {e}"),
            }
        });
    if let Err(e) = lanzado {
        *EN_MARCHA.lock() = None;
        cerrar_barra(app);
        return Err(AppError::Msg(format!("no se ha podido empezar la captura: {e}")));
    }
    Ok(())
}

/// La zona recortada a su pantalla: la foto es de una pantalla y no puede salirse.
fn dentro_de(region: Rect, monitor: &MonitorInfo) -> Rect {
    let x0 = region.x.max(monitor.x);
    let y0 = region.y.max(monitor.y);
    let x1 = (region.x + region.width as i32).min(monitor.x + monitor.width as i32);
    let y1 = (region.y + region.height as i32).min(monitor.y + monitor.height as i32);
    Rect {
        x: x0,
        y: y0,
        width: (x1 - x0).max(1) as u32,
        height: (y1 - y0).max(1) as u32,
    }
}

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

/// Gira la rueda hacia abajo tantas muescas, una detrás de otra, como una rueda de verdad.
fn rueda(muescas: u32) {
    let una = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: MUESCA as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let todas = vec![una; muescas.max(1) as usize];
    unsafe {
        SendInput(&todas, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Le cuenta a la barra cómo va, sin inundarla: cuando cambia algo, y si no, dos veces
/// por segundo (la primera puede llegar antes de que la barra haya cargado).
struct Aviso<'a> {
    app: &'a AppHandle,
    ultimo: Option<(u32, &'static str)>,
    cuando: Instant,
}

impl<'a> Aviso<'a> {
    fn nuevo(app: &'a AppHandle) -> Self {
        Self {
            app,
            ultimo: None,
            cuando: Instant::now(),
        }
    }

    fn contar(&mut self, alto: u32, estado: &'static str) {
        let ahora = (alto, estado);
        if self.ultimo == Some(ahora) && self.cuando.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.ultimo = Some(ahora);
        self.cuando = Instant::now();
        let _ = self
            .app
            .emit_to(BARRA_LABEL, EVENTO_PROGRESO, Progreso { alto, estado });
    }
}

/// La captura entera. Devuelve la imagen cosida, o nada si se ha descartado.
fn capturar(
    app: &AppHandle,
    region: Rect,
    monitor: &MonitorInfo,
    orden: &AtomicU8,
) -> Result<Option<RgbaImage>> {
    // Que el overlay se haya ido de verdad de la pantalla antes de la primera foto: se ha
    // aparcado fuera, pero el compositor puede tener todavía un cuadro con él dentro.
    std::thread::sleep(Duration::from_millis(120));
    let _ = unsafe { DwmFlush() };

    let marco = crate::platform::marco::Marco::abrir(region, 2)
        .map_err(|e| eprintln!("[scroll] sin marco: {e}"))
        .ok();
    let esc = Esc::coger();
    let fotografo = Fotografo::nuevo(monitor, region)?;
    let mut cosedor = Cosedor::nuevo(fotografo.foto()?);
    let mut aviso = Aviso::nuevo(app);

    let objetivo = punto_dentro(region, raton());
    poner_el_raton(objetivo);

    let mut sola = true;
    let mut estado: &'static str = "bajando";
    let mut muescas: u32 = 1;
    let mut giros_en_vano = 0;
    aviso.contar(cosedor.alto(), estado);

    'captura: loop {
        if hay_que_parar(orden, &esc) {
            break;
        }
        if sola && se_ha_movido(objetivo) {
            sola = false;
            estado = "tu";
        }
        aviso.contar(cosedor.alto(), estado);

        if !sola {
            // La rueda la pone quien captura: se mira lo que haya y se cose si ha bajado.
            std::thread::sleep(Duration::from_millis(60));
            let Ok(foto) = fotografo.foto() else { break };
            match cosedor.anadir(foto) {
                Paso::Cosida(_) => estado = "tu",
                Paso::Perdida => estado = "perdida",
                Paso::Llena => break,
                Paso::Quieta | Paso::Atras => {}
            }
            continue;
        }

        rueda(muescas);
        let inicio = Instant::now();
        let mut bajado = 0u32;
        let mut quietas = 0;
        loop {
            std::thread::sleep(ENTRE_FOTOS);
            if hay_que_parar(orden, &esc) {
                break 'captura;
            }
            let Ok(foto) = fotografo.foto() else { break 'captura };
            match cosedor.anadir(foto) {
                Paso::Cosida(d) => {
                    bajado += d;
                    quietas = 0;
                    aviso.contar(cosedor.alto(), estado);
                }
                Paso::Quieta => quietas += 1,
                Paso::Atras | Paso::Perdida => quietas = 0,
                Paso::Llena => break 'captura,
            }
            if (quietas >= 2 && inicio.elapsed() > ARRANQUE) || inicio.elapsed() > ESPERA_MAXIMA {
                break;
            }
        }

        if bajado > 0 {
            giros_en_vano = 0;
            // Lo que baja cada muesca lo decide el programa de debajo: se mide, y la
            // siguiente vez se gira lo justo para bajar algo más de media zona.
            let por_muesca = (bajado / muescas).max(1);
            muescas = (cosedor.zona() * 3 / 5 / por_muesca).clamp(1, 8);
        } else {
            giros_en_vano += 1;
            if giros_en_vano >= 2 {
                if cosedor.cosidas() > 0 {
                    // Ha bajado y ya no baja: es el final de la página.
                    break;
                }
                // No ha bajado nunca: la rueda ha caído en algo que no se desplaza.
                sola = false;
                estado = "no-baja";
            } else {
                // Una página que carga más al llegar abajo tarda un poco en crecer.
                std::thread::sleep(Duration::from_millis(400));
            }
        }
    }

    drop(esc);
    drop(marco);
    if orden.load(Ordering::SeqCst) == DESCARTAR {
        return Ok(None);
    }
    aviso.contar(cosedor.alto(), "cosiendo");
    Ok(Some(cosedor.imagen()))
}

/// La imagen al editor, como una foto cualquiera: desde ahí se copia, se guarda o se
/// anota, y guardar va por el mismo camino que el resto.
fn entregar(app: &AppHandle, imagen: &RgbaImage, region: Rect) {
    let con_sonido = app
        .state::<crate::state::AppState>()
        .settings
        .read()
        .play_sound;
    if con_sonido {
        crate::platform::sonido::obturador();
    }
    let zona = Rect {
        x: region.x,
        y: region.y,
        width: imagen.width(),
        height: imagen.height(),
    };
    let abierto = crate::recorder::session_from_image(app, imagen, zona)
        .and_then(|sesion| crate::windows_mgr::open_editor(app, &sesion.id));
    if let Err(e) = abierto {
        eprintln!("[scroll] no se ha podido abrir la captura en el editor: {e}");
    }
}

/// La barrita de la captura: cómo va, «Terminar» y la X.
fn abrir_barra(app: &AppHandle, region: Rect) -> Result<()> {
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
    let sitio = crate::windows_mgr::monitor_de(cx, cy);
    let escala = sitio.map(|m| m.escala).unwrap_or(1.0);
    let (x, y) = sitio_de_la_barra(
        region,
        sitio.map(|m| (m.x, m.y, m.ancho, m.alto)),
        (BARRA_ANCHO * escala) as i32,
        (BARRA_ALTO * escala) as i32,
        (12.0 * escala) as i32,
    );
    let _ = ventana.set_position(PhysicalPosition::new(x, y));
    ventana.show()?;
    Ok(())
}

fn cerrar_barra(app: &AppHandle) {
    if let Some(ventana) = app.get_webview_window(BARRA_LABEL) {
        let _ = ventana.close();
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

    #[test]
    fn la_zona_no_se_sale_de_su_pantalla() {
        let monitor = MonitorInfo {
            id: 0,
            label: "PRUEBA".into(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
            is_primary: true,
        };
        let r = dentro_de(Rect { x: 1800, y: -10, width: 400, height: 500 }, &monitor);
        assert_eq!((r.x, r.y, r.width, r.height), (1800, 0, 120, 490));
    }

    #[test]
    fn la_primera_orden_es_la_que_vale() {
        let orden = Arc::new(AtomicU8::new(SEGUIR));
        *EN_MARCHA.lock() = Some(orden.clone());
        pedir(TERMINAR);
        pedir(DESCARTAR);
        assert_eq!(orden.load(Ordering::SeqCst), TERMINAR);
        *EN_MARCHA.lock() = None;
    }
}
