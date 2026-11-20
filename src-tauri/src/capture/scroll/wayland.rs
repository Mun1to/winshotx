//! La captura con scroll en Wayland. Lo de coser y lo de decidir cuánto se gira es lo de
//! siempre (el módulo de arriba); aquí está de dónde salen las fotos y quién gira la rueda.
//!
//! En Wayland un programa no puede leer la pantalla ni tocar el puntero: se le pide al
//! escritorio, que le pregunta a la persona. Se pide todo junto, la pantalla y el puntero
//! (`record::linux::wayland::preparar_fotos`), y la respuesta se guarda para que no vuelva a
//! preguntar.
//!
//! - **Las fotos** son el último fotograma que ha mandado el escritorio de esa pantalla,
//!   recortado a la zona. Solo manda uno cuando algo cambia, así que «la foto de ahora» es la
//!   última que llegó. El ratón no va en ellas.
//! - **La rueda** la gira el escritorio cuando se le pide, sobre lo que haya debajo del
//!   puntero. Si la persona deja ver la pantalla pero no controlar, o el escritorio no sabe,
//!   no baja sola: la rueda la pone quien captura, y se cose lo que vaya bajando.
//! - **No hay barra, ni marco, ni Esc.** Una ventana aquí no elige su sitio, así que saldría
//!   en las fotos; y las teclas son del programa que tiene el foco. Se para llegando al
//!   final, con la tecla de capturar (o `winshotx --capture`), o dejando de compartir la
//!   pantalla desde el aviso del propio escritorio. La pantalla de elegir zona lo ha dicho
//!   antes de empezar (`platform::como_parar`).

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use tauri::{AppHandle, Manager};

use super::comun::{DESCARTAR, TERMINAR};
use super::linux::punto_dentro;
use super::{bajar, Cosedor, Pantalla};
use crate::capture::Rect;
use crate::error::{AppError, Result};
use crate::record::linux::wayland::{preparar_fotos, se_puede_pedir_el_puntero, Mando, Preparada, RatonVivo};
use crate::record::linux::{EnMarcha, Fotograma, Marcha, Reciclados};

/// Donde se guarda el permiso del escritorio para ver la pantalla y mover el puntero, junto a
/// los ajustes. Aparte del de grabar (`screencast-token`), que es solo de la pantalla.
const FICHA_DEL_PUNTERO: &str = "remotedesktop-token";

fn hay_ficha_del_puntero(carpeta: &std::path::Path) -> bool {
    std::fs::metadata(carpeta.join(FICHA_DEL_PUNTERO)).is_ok_and(|ficha| ficha.len() > 0)
}

/// Mira, por detrás, si la próxima captura con scroll va a hacer que el escritorio pregunte
/// por el puntero: se le puede pedir y no hay permiso guardado. La pantalla de elegir zona lo
/// dice antes (`platform::como_parar`), porque en la pregunta de GNOME viene apagado y sin él
/// la página no baja sola.
pub(super) fn mirar_si_preguntara(app: &AppHandle) {
    if !crate::platform::xdg::es_wayland() {
        return;
    }
    let Ok(carpeta) = app.path().app_config_dir() else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        let preguntara = !hay_ficha_del_puntero(&carpeta) && se_puede_pedir_el_puntero().await;
        crate::platform::como_parar::apuntar_la_pregunta_del_puntero(preguntara);
    });
}

/// Lo que tiene que pasar sin que el escritorio mande nada para dar la pantalla por quieta
/// antes de la primera foto: la pantalla de elegir zona y la pregunta del escritorio se van
/// con una animación, y la primera foto es el principio de la imagen.
const QUIETA: Duration = Duration::from_millis(300);
/// Lo más que se espera a eso: con algo que se mueve en pantalla no se queda quieta nunca.
const ESPERA_A_QUE_SE_QUEDE_QUIETA: Duration = Duration::from_millis(2500);
/// Lo que se espera a que el puntero llegue adonde se le ha mandado.
const ESPERA_DEL_PUNTERO: Duration = Duration::from_millis(400);
/// Lo que puede moverse el puntero sin que cuente como que lo ha cogido quien captura. Lo
/// mismo que en X11 y en Windows.
const HOLGURA_DEL_PUNTERO: i32 = 6;

/// Las fotos de la zona: lo último que ha mandado el escritorio.
struct Fotos {
    marcha: Marcha,
    recibidos: Receiver<Fotograma>,
    reciclados: Reciclados,
    en_marcha: Option<EnMarcha>,
    medida: (u32, u32),
    ultima: Vec<u8>,
    /// La pantalla ha dejado de llegar: el escritorio ha cortado, o se ha caído.
    cortada: bool,
}

impl Fotos {
    fn empezar(preparada: Preparada) -> Self {
        let region = preparada.region();
        let marcha = Marcha {
            stop: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
            paused_ms: Arc::new(AtomicU64::new(0)),
            start: Instant::now(),
        };
        let (salida, recibidos) = channel();
        let reciclados = Reciclados::default();
        let en_marcha = preparada.grabar(marcha.clone(), salida, reciclados.clone());
        Self {
            marcha,
            recibidos,
            reciclados,
            en_marcha: Some(en_marcha),
            medida: (region.width, region.height),
            ultima: Vec::new(),
            cortada: false,
        }
    }

    fn guardar(&mut self, fotograma: Fotograma) {
        let anterior = std::mem::replace(&mut self.ultima, fotograma.rgba);
        if anterior.capacity() > 0 {
            self.reciclados.devolver(anterior);
        }
    }

    /// Se queda con lo último que haya llegado.
    fn ponerse_al_dia(&mut self) {
        loop {
            match self.recibidos.try_recv() {
                Ok(fotograma) => self.guardar(fotograma),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.cortada = true;
                    return;
                }
            }
        }
    }

    /// Espera a que el escritorio deje de mandar imágenes: lo que se estuviera yendo de la
    /// pantalla ya se ha ido.
    fn esperar_a_que_se_quede_quieta(&mut self) {
        let tope = Instant::now() + ESPERA_A_QUE_SE_QUEDE_QUIETA;
        loop {
            match self.recibidos.recv_timeout(QUIETA) {
                Ok(fotograma) => self.guardar(fotograma),
                // Sin ninguna imagen todavía no hay nada que dar por quieto.
                Err(RecvTimeoutError::Timeout) if !self.ultima.is_empty() => return,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    self.cortada = true;
                    return;
                }
            }
            if Instant::now() >= tope {
                return;
            }
        }
    }

    fn foto(&mut self) -> Result<RgbaImage> {
        self.ponerse_al_dia();
        if self.cortada {
            return Err(AppError::Msg("el escritorio ha dejado de mandar la pantalla".into()));
        }
        RgbaImage::from_raw(self.medida.0, self.medida.1, self.ultima.clone())
            .ok_or_else(|| AppError::Msg("el escritorio no ha mandado la imagen de la zona".into()))
    }
}

impl Drop for Fotos {
    fn drop(&mut self) {
        // Parar es lo que cierra la sesión con el escritorio, y con ella su aviso de que se
        // está compartiendo la pantalla.
        self.marcha.stop.store(true, Ordering::Relaxed);
        if let Some(en_marcha) = self.en_marcha.take() {
            let _ = en_marcha.terminar();
        }
    }
}

/// Si el puntero se ha ido de donde se dejó. Los dos puntos, contados desde la esquina de la
/// zona.
fn se_ha_ido(ahora: (i32, i32), desde: (i32, i32)) -> bool {
    (ahora.0 - desde.0).abs() + (ahora.1 - desde.1).abs() > HOLGURA_DEL_PUNTERO
}

/// La pantalla de Wayland, para `bajar`.
struct EnWayland<'a> {
    fotos: Fotos,
    /// `None` si la rueda la pone quien captura.
    mando: Option<Mando>,
    raton: RatonVivo,
    /// Dónde se quedó el puntero al empezar, contado desde la esquina de la zona. `None` si el
    /// escritorio no dice por dónde va.
    dejado: Option<(i32, i32)>,
    orden: &'a AtomicU8,
    empezo: Instant,
    estado: &'static str,
}

impl Pantalla for EnWayland<'_> {
    fn foto(&mut self) -> Result<RgbaImage> {
        self.fotos.foto()
    }

    fn rueda(&mut self, muescas: i32) {
        if self.mando.as_ref().is_some_and(|mando| !mando.rueda(muescas)) {
            eprintln!("[scroll] el escritorio ya no deja girar la rueda: la pone quien captura");
            self.mando = None;
        }
    }

    fn esperar(&mut self, tiempo: Duration) {
        std::thread::sleep(tiempo);
    }

    fn reloj(&self) -> Duration {
        self.empezo.elapsed()
    }

    fn raton_movido(&mut self) -> bool {
        if self.mando.is_none() {
            return true;
        }
        match (*self.raton.lock(), self.dejado) {
            (Some(ahora), Some(dejado)) => se_ha_ido(ahora, dejado),
            _ => false,
        }
    }

    fn parar(&mut self) -> bool {
        self.orden.load(Ordering::SeqCst) != super::comun::SEGUIR
    }

    fn avisar(&mut self, _alto: u32, estado: &'static str) {
        // No hay barra a la que contárselo: queda en el registro cuando cambia.
        if estado != self.estado {
            self.estado = estado;
            eprintln!("[scroll] ahora: {estado}");
        }
    }
}

/// Pone el puntero dentro de la zona, que es donde tiene que caer la rueda, y devuelve dónde
/// se ha quedado (contado desde la esquina de la zona), si el escritorio lo dice. `Err` si el
/// escritorio no deja moverlo.
fn dejar_el_puntero(mando: &Mando, raton: &RatonVivo, zona: Rect) -> std::result::Result<Option<(i32, i32)>, ()> {
    let ahora = (*raton.lock()).map(|(x, y)| (zona.x + x, zona.y + y)).unwrap_or_else(|| zona.center());
    let destino = punto_dentro(zona, ahora);
    if !mando.poner(destino) {
        return Err(());
    }
    // El escritorio lo mueve cuando le toca: se espera a que diga que ya está allí. Si no lo
    // dice a tiempo, vale donde diga que está: desde ahí se mira si lo coge quien captura.
    let en_la_zona = (destino.0 - zona.x, destino.1 - zona.y);
    let tope = Instant::now() + ESPERA_DEL_PUNTERO;
    loop {
        let visto = *raton.lock();
        let ha_llegado = visto.is_some_and(|p| (p.0 - en_la_zona.0).abs() <= 2 && (p.1 - en_la_zona.1).abs() <= 2);
        if ha_llegado || Instant::now() >= tope {
            return Ok(visto);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// La captura entera. Devuelve la imagen cosida, o nada si se ha descartado.
pub(super) fn capturar(app: &AppHandle, region: Rect, orden: &AtomicU8) -> Result<Option<RgbaImage>> {
    let carpeta = app.path().app_config_dir().map_err(|e| AppError::Msg(e.to_string()))?;
    // Aquí se espera a que la persona conteste al escritorio, si es que pregunta.
    let pedida = preparar_fotos(
        region,
        &carpeta.join(FICHA_DEL_PUNTERO),
        &carpeta.join("screencast-token"),
        // «Dejar de compartir» en el aviso del escritorio: se queda con lo que lleve.
        Box::new(|| super::comun::pedir(TERMINAR)),
    )?;
    // Para la próxima: si no ha quedado permiso guardado, el escritorio volverá a preguntar.
    crate::platform::como_parar::apuntar_la_pregunta_del_puntero(
        pedida.puntero_pedido && !hay_ficha_del_puntero(&carpeta),
    );
    let zona = pedida.preparada.region();
    let raton = pedida.preparada.raton_vivo();
    let mut fotos = Fotos::empezar(pedida.preparada);
    fotos.esperar_a_que_se_quede_quieta();

    let mut dejado = None;
    let mando = pedida.mando.and_then(|mando| match dejar_el_puntero(&mando, &raton, zona) {
        Ok(visto) => {
            dejado = visto;
            Some(mando)
        }
        Err(()) => None,
    });
    match &mando {
        Some(_) => eprintln!("[scroll] el escritorio deja ver la pantalla y mover el puntero: baja sola"),
        None => eprintln!("[scroll] el escritorio deja ver la pantalla, pero no mover el puntero: la rueda la pone quien captura"),
    }

    let mut cosedor = Cosedor::nuevo(fotos.foto()?);
    let empezo = Instant::now();
    {
        let mut pantalla = EnWayland { fotos, mando, raton, dejado, orden, empezo, estado: "" };
        bajar(&mut pantalla, &mut cosedor);
    }

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
    Ok(Some(cosedor.imagen()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_temblor_del_puntero_no_es_cogerlo() {
        // Lo que mueve el propio escritorio al colocar el puntero, o una mano apoyada.
        assert!(!se_ha_ido((334, 304), (334, 304)));
        assert!(!se_ha_ido((337, 301), (334, 304)));
    }

    #[test]
    fn moverlo_de_verdad_si() {
        assert!(se_ha_ido((350, 304), (334, 304)));
        assert!(se_ha_ido((330, 300), (334, 304)));
    }
}
