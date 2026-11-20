//! La captura con scroll en Linux: las fotos, la rueda, el ratón, Esc y la barra. Lo de coser
//! y lo de decidir cuánto se gira está en el módulo de arriba, igual que para Windows
//! (`en_vivo`), y lo que no depende de la pantalla, en `comun`.
//!
//! **Por ahora, X11.** Ahí todo se le pide al servidor X:
//!
//! - **Las fotos**, por memoria compartida (MIT-SHM), con lo mismo que da los fotogramas al
//!   grabar (`record::linux::x11`). El ratón no sale en ellas.
//! - **La rueda**, con XTEST: el servidor la reparte como si la girara quien captura, a la
//!   ventana que hay debajo del ratón, así que no hace falta saber qué programa es. El botón 5
//!   baja y el 4 sube, una pulsación por muesca. Sin XTEST no baja sola, `bajar` lo nota en
//!   dos giros y le deja la rueda a quien captura, como cuando cae en algo que no se desplaza.
//! - **Esc**, cogida en el servidor mientras dura (`XGrabKey`), que llega tenga el foco quien
//!   lo tenga. Si otro programa ya la tiene, se mira el teclado a mano.
//!
//! **Aquí nada nuestro se puede sacar de una foto**: lo que está en pantalla, sale. Por eso el
//! marco va siempre por fuera de la zona (`marco_x11`) y la barra solo se abre si cabe entera
//! fuera de ella. Si no cabe (la pantalla entera, o casi), no hay barra: se para con Esc, con
//! la tecla de capturar o llegando al final, y la pantalla de elegir zona ya lo ha dicho.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use image::RgbaImage;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConnectionExt as _, GrabMode, ModMask, Window, BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::comun::{Aviso, BARRA_ALTO, BARRA_ANCHO, DESCARTAR, SEGUIR, TERMINAR};
use super::{bajar, Cosedor, Pantalla, BARRA_LABEL};
use crate::capture::{MonitorInfo, Rect};
use crate::error::{AppError, Result};
use crate::record::linux::x11::{preparar_fotos, Preparada};

/// Lo que se espera a que la pantalla de elegir zona se vaya de la pantalla antes de la
/// primera foto. Lo mismo que antes de grabar (`recorder/linux.rs`).
const QUE_SE_VAYA_EL_OVERLAY: Duration = Duration::from_millis(250);

/// Los botones del ratón que son la rueda en X11.
const RUEDA_ARRIBA: u8 = 4;
const RUEDA_ABAJO: u8 = 5;

/// El nombre de la tecla Esc en X11 (su keysym).
const KEYSYM_ESC: u32 = 0xFF1B;

/// Una tecla se coge con sus modificadores exactos, y para X11 Bloq Num y Bloq Mayús lo son:
/// Esc se coge sola y con cada uno de los dos encendido, o con ellos puestos no llegaría.
fn candados() -> [ModMask; 4] {
    [ModMask::default(), ModMask::M2, ModMask::LOCK, ModMask::M2 | ModMask::LOCK]
}

/// Cómo se va a saber que se ha pulsado Esc.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Esc {
    /// Cogida en el servidor: cada pulsación llega por la conexión, aunque dure un instante,
    /// y el programa de delante no la recibe.
    Cogida(u8),
    /// Otro programa la tiene cogida: se mira el teclado entre foto y foto, que puede perderse
    /// una pulsación muy corta.
    AMano(u8),
    /// No se sabe ni qué tecla es en este teclado.
    Ninguna,
}

/// Si aquí se puede capturar con scroll. Se pregunta antes de cerrar la pantalla de elegir
/// zona, para que el porqué salga en ella.
pub(super) fn se_puede() -> Result<()> {
    if crate::platform::xdg::es_wayland() {
        return Err(AppError::Msg("En Wayland la captura con scroll todavía no funciona.".into()));
    }
    Ok(())
}

/// La conexión con el servidor X para lo que no son fotos: girar la rueda, saber dónde está
/// el ratón y moverlo, y la tecla Esc. Aparte de la de las fotos, que es de quien graba.
///
/// **Esc se coge aquí y no con el plugin de los atajos**, que es como se cogen las teclas de
/// capturar. Así se hizo primero, y después de una captura Esc se quedaba cogida: la pantalla
/// de elegir zona recibía la S y no Esc, hasta con un clic dentro (laboratorio de S45, seis
/// pasadas). El plugin (`global-hotkey` 0.8) pide soltar una tecla sin esperar a que el
/// servidor conteste, y como ya no la espera, se la traga. Con conexión propia se suelta al
/// acabar con una ida y vuelta al servidor, y si winshotx muere a media captura la suelta el
/// servidor al cerrarse la conexión. Y las pulsaciones se leen en este mismo hilo, como en
/// Windows.
struct Mando {
    conexion: RustConnection,
    raiz: Window,
    esc: Esc,
}

impl Mando {
    fn abrir() -> Result<Self> {
        let (conexion, numero) = RustConnection::connect(None)
            .map_err(|e| AppError::Msg(format!("no se puede hablar con el servidor X: {e}")))?;
        let raiz = conexion
            .setup()
            .roots
            .get(numero)
            .map(|p| p.root)
            .ok_or_else(|| AppError::Msg("el servidor X no tiene esa pantalla".into()))?;
        if conexion.xtest_get_version(2, 2).ok().and_then(|c| c.reply().ok()).is_none() {
            eprintln!("[scroll] el servidor X no tiene XTEST: no baja sola, la rueda la pone quien captura");
        }
        let mut mando = Self { conexion, raiz, esc: Esc::Ninguna };
        mando.esc = mando.coger_esc();
        Ok(mando)
    }

    /// Coge Esc en todo el escritorio mientras viva este mando.
    fn coger_esc(&self) -> Esc {
        let Some(tecla) = self.tecla_esc() else {
            eprintln!("[scroll] este teclado no dice cuál es su tecla Esc: se para con la barra o con la tecla de capturar");
            return Esc::Ninguna;
        };
        let cogida = candados().into_iter().all(|candado| {
            self.conexion
                .grab_key(false, self.raiz, candado, tecla, GrabMode::ASYNC, GrabMode::ASYNC)
                .is_ok_and(|pedida| pedida.check().is_ok())
        });
        if cogida {
            return Esc::Cogida(tecla);
        }
        // A medias no vale: se suelta lo que sí se haya cogido.
        self.soltar(tecla);
        eprintln!("[scroll] otro programa tiene cogida Esc: se mira el teclado a mano");
        Esc::AMano(tecla)
    }

    fn soltar(&self, tecla: u8) {
        for candado in candados() {
            let _ = self.conexion.ungrab_key(tecla, self.raiz, candado);
        }
        // Ida y vuelta al servidor: al volver de aquí la tecla ya es otra vez de todos.
        let _ = self.conexion.sync();
    }

    /// Si se ha pulsado Esc desde la última vez que se preguntó.
    fn esc_pulsada(&self) -> bool {
        match self.esc {
            Esc::Cogida(tecla) => {
                let mut pulsada = false;
                while let Ok(Some(aviso)) = self.conexion.poll_for_event() {
                    if let Event::KeyPress(pulsacion) = aviso {
                        pulsada |= pulsacion.detail == tecla;
                    }
                }
                pulsada
            }
            Esc::AMano(tecla) => self.esta_abajo(tecla),
            Esc::Ninguna => false,
        }
    }

    /// Dónde está el ratón, en píxeles del escritorio.
    fn raton(&self) -> Option<(i32, i32)> {
        let puntero = self.conexion.query_pointer(self.raiz).ok()?.reply().ok()?;
        puntero.same_screen.then(|| (i32::from(puntero.root_x), i32::from(puntero.root_y)))
    }

    fn poner_el_raton(&self, (x, y): (i32, i32)) {
        let _ = self.conexion.warp_pointer(x11rb::NONE, self.raiz, 0, 0, 0, 0, x as i16, y as i16);
        // Que el servidor lo haya movido ya cuando se le pregunte dónde está.
        let _ = self.conexion.sync();
    }

    fn se_ha_movido(&self, desde: (i32, i32)) -> bool {
        self.raton()
            .is_some_and(|(x, y)| (x - desde.0).abs() + (y - desde.1).abs() > 6)
    }

    /// Gira la rueda tantas muescas, una detrás de otra, como una rueda de verdad: hacia abajo
    /// si son positivas y hacia arriba si son negativas.
    fn rueda(&self, muescas: i32) {
        let boton = if muescas >= 0 { RUEDA_ABAJO } else { RUEDA_ARRIBA };
        for _ in 0..muescas.unsigned_abs().max(1) {
            let _ = self.conexion.xtest_fake_input(BUTTON_PRESS_EVENT, boton, 0, x11rb::NONE, 0, 0, 0);
            let _ = self.conexion.xtest_fake_input(BUTTON_RELEASE_EVENT, boton, 0, x11rb::NONE, 0, 0, 0);
        }
        let _ = self.conexion.flush();
    }

    /// El número de tecla (keycode) de Esc en este teclado.
    fn tecla_esc(&self) -> Option<u8> {
        let (primera, ultima) = (self.conexion.setup().min_keycode, self.conexion.setup().max_keycode);
        let cuantas = ultima.checked_sub(primera)?.checked_add(1)?;
        let teclado = self.conexion.get_keyboard_mapping(primera, cuantas).ok()?.reply().ok()?;
        let por_tecla = usize::from(teclado.keysyms_per_keycode).max(1);
        let n = teclado.keysyms.chunks(por_tecla).position(|keysyms| keysyms.contains(&KEYSYM_ESC))?;
        primera.checked_add(u8::try_from(n).ok()?)
    }

    fn esta_abajo(&self, tecla: u8) -> bool {
        self.conexion
            .query_keymap()
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|abajo| abajo.keys[usize::from(tecla / 8)] & (1 << (tecla % 8)) != 0)
    }
}

impl Drop for Mando {
    fn drop(&mut self) {
        if let Esc::Cogida(tecla) = self.esc {
            self.soltar(tecla);
        }
    }
}

/// Donde se deja el ratón para girar la rueda: donde esté, metido 16 píxeles dentro de la
/// zona. Al soltar el recorte ya está en una esquina, así que apenas se mueve. La misma
/// cuenta que en Windows (`en_vivo::punto_dentro`).
fn punto_dentro(region: Rect, raton: (i32, i32)) -> (i32, i32) {
    let margen = 16.min(region.width as i32 / 4).min(region.height as i32 / 4);
    (
        raton.0.clamp(region.x + margen, region.x + region.width as i32 - 1 - margen),
        raton.1.clamp(region.y + margen, region.y + region.height as i32 - 1 - margen),
    )
}

/// La pantalla de X11, para `bajar`.
struct EnX11<'a> {
    fotos: Preparada,
    mando: Mando,
    orden: &'a AtomicU8,
    objetivo: (i32, i32),
    empezo: Instant,
    avisar: Box<dyn FnMut(u32, &'static str) + 'a>,
}

impl Pantalla for EnX11<'_> {
    fn foto(&mut self) -> Result<RgbaImage> {
        self.fotos.foto()
    }

    fn rueda(&mut self, muescas: i32) {
        self.mando.rueda(muescas);
    }

    fn esperar(&mut self, tiempo: Duration) {
        std::thread::sleep(tiempo);
    }

    fn reloj(&self) -> Duration {
        self.empezo.elapsed()
    }

    fn raton_movido(&mut self) -> bool {
        self.mando.se_ha_movido(self.objetivo)
    }

    fn parar(&mut self) -> bool {
        if self.mando.esc_pulsada() {
            let _ = self.orden.compare_exchange(SEGUIR, TERMINAR, Ordering::SeqCst, Ordering::SeqCst);
        }
        self.orden.load(Ordering::SeqCst) != SEGUIR
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
    // Que la pantalla de elegir zona se haya ido de verdad antes de la primera foto: aquí se
    // esconde, y el servidor tarda un momento en repintar lo que tapaba.
    std::thread::sleep(QUE_SE_VAYA_EL_OVERLAY);

    let fotos = preparar_fotos(region)?;
    let mando = Mando::abrir()?;
    let marco = crate::platform::marco_x11::Marco::abrir(region, f64::from(monitor.scale))
        .map_err(|e| eprintln!("[scroll] sin marco: {e}"))
        .ok();
    let mut cosedor = Cosedor::nuevo(fotos.foto()?);
    let mut aviso = Aviso::nuevo(app);

    let zona = fotos.region();
    let (cx, cy) = zona.center();
    let objetivo = punto_dentro(zona, mando.raton().unwrap_or((cx, cy)));
    mando.poner_el_raton(objetivo);
    let empezo = Instant::now();
    {
        let mut pantalla = EnX11 {
            fotos,
            mando,
            orden,
            objetivo,
            empezo,
            avisar: Box::new(|alto, estado| aviso.contar(alto, estado)),
        };
        bajar(&mut pantalla, &mut cosedor);
    }

    drop(marco);
    let como = match orden.load(Ordering::SeqCst) {
        DESCARTAR => "descartada",
        TERMINAR => "parada a mano",
        _ => "hasta donde ha llegado sola",
    };
    eprintln!(
        "[scroll] {} fotos cosidas, {} px de alto, en {} ms ({como})",
        cosedor.cosidas(),
        cosedor.alto(),
        empezo.elapsed().as_millis()
    );
    if orden.load(Ordering::SeqCst) == DESCARTAR {
        return Ok(None);
    }
    aviso.contar(cosedor.alto(), "cosiendo");
    Ok(Some(cosedor.imagen()))
}

/// La barrita de la captura: cómo va, «Parar» y la X. Solo si cabe entera fuera de la zona.
pub(super) fn abrir_barra(app: &AppHandle, region: Rect) -> Result<()> {
    let (cx, cy) = region.center();
    let sitio = crate::windows_mgr::monitor_de(cx, cy)
        .and_then(|m| crate::windows_mgr::sitio_fuera(region, m, BARRA_ANCHO, BARRA_ALTO));
    let Some((x, y)) = sitio else {
        eprintln!(
            "[scroll] no hay barra (no cabe fuera de la zona, y dentro saldría en las fotos): se para con Esc, con la tecla de capturar o al llegar al final"
        );
        return Ok(());
    };
    let ventana = match app.get_webview_window(BARRA_LABEL) {
        Some(existente) => existente,
        None => {
            let url = WebviewUrl::App("overlay.html?barra=scroll".into());
            WebviewWindowBuilder::new(app, BARRA_LABEL, url)
                .title("winshotx")
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                // En GTK una ventana que no se puede estirar no hace caso del tamaño que se le
                // pide (`windows_mgr::open_recorder_fuera`): con el mínimo y el máximo iguales
                // mide lo suyo y tampoco se puede estirar.
                .resizable(true)
                // Sin foco nunca: la página que baja tiene que seguir siendo la de delante.
                .focusable(false)
                .focused(false)
                .visible(false)
                .inner_size(BARRA_ANCHO, BARRA_ALTO)
                .min_inner_size(BARRA_ANCHO, BARRA_ALTO)
                .max_inner_size(BARRA_ANCHO, BARRA_ALTO)
                .build()?
        }
    };
    let _ = ventana.set_position(PhysicalPosition::new(x, y));
    ventana.show()?;
    eprintln!("[scroll] la barra, fuera de la zona, en {x},{y}");
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn zona() -> Rect {
        Rect { x: 100, y: 200, width: 800, height: 600 }
    }

    #[test]
    fn el_raton_se_queda_donde_esta_si_ya_esta_dentro() {
        assert_eq!(punto_dentro(zona(), (400, 500)), (400, 500));
    }

    #[test]
    fn desde_la_esquina_del_recorte_apenas_entra() {
        // Al soltar el recorte, el ratón está en su esquina de abajo a la derecha.
        assert_eq!(punto_dentro(zona(), (900, 800)), (883, 783));
    }

    #[test]
    fn en_una_zona_estrecha_el_raton_sigue_cayendo_dentro() {
        let estrecha = Rect { x: 10, y: 10, width: 40, height: 80 };
        let (x, y) = punto_dentro(estrecha, (500, 500));
        assert!((10..50).contains(&x) && (10..90).contains(&y), "{x},{y}");
    }
}
