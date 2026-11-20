//! El sonido donde no lo lleva PipeWire sino PulseAudio (Ubuntu 22.04, Linux Mint 21, los
//! Debian con Xfce): lo que suena por el altavoz y el microfono, con el GStreamer del sistema.
//!
//! Alli PipeWire esta, pero solo para compartir pantalla, y `sonido.rs` no encuentra a que
//! engancharse. Lo que si hay es el `pulsesrc` de GStreamer, que graba de PulseAudio el
//! «monitor» de la salida y el microfono. Va dentro de `gstreamer1.0-plugins-good`, que el
//! .deb ya recomendaba (`gstreamer1.0-pulseaudio` es en Debian y Ubuntu un paquete vacio, de
//! transicion). Sin crate nuevo: es el mismo `gstreamer` que ya escribe el MP4 y lee la camara.
//!
//! **Por que no la orden `parec`.** Tal cual, `parec` recibe el monitor a trozos de hasta dos
//! segundos y sin saber de cuando es cada uno (medido en Ubuntu 22.04: con un reproductor de
//! musica sonando, lo grabado caia hasta 2 s fuera de su sitio). `pulsesrc` le pide a
//! PulseAudio trozos de 10 ms, y cada trozo trae la hora a la que se grabo: aqui se coloca por
//! esa hora y no por cuando llega.
//!
//! De ahi en adelante es lo mismo que con PipeWire: cada trozo va a `Recibir` con el instante
//! del reloj de la grabacion en el que acaba, y `sonido.rs` lo pone en su pista (un archivo, o
//! un anillo en «Grabar siempre lo ultimo»).

use std::time::Duration;

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;

use super::sonido::{Cual, Disponible, Fuentes, Recibir, FORMATO};
use super::Marcha;
use crate::encode::gst as motor;

/// Quien sabe en GStreamer que aparatos tiene PulseAudio. Viene con `pulsesrc`.
const PROVEEDOR: &str = "pulsedeviceprovider";

/// Cada cuanto se mira si ha llegado sonido. Los trozos son de 10 ms y traen su hora: mirar
/// un poco tarde no los descoloca.
const LATIDO: Duration = Duration::from_millis(5);

/// Un trozo que dice haber acabado hace mas que esto no dice la verdad (un reloj que salta):
/// se coloca como recien llegado.
const RETRASO_CREIBLE_MS: u64 = 5_000;

/// El colchon que se le pide a `pulsesrc` (`buffer-time`), en microsegundos. No es retraso: los
/// trozos se siguen pidiendo de 10 ms (`latency-time`). Es lo mas que PulseAudio entrega de una
/// vez sin partirlo, y en el tiene que caber entero un bloque de la fuente, porque no todas
/// entregan de 10 en 10 ms: las hay que dan bloques fijos (la de pruebas de PulseAudio,
/// `module-sine-source`, de 2 s).
///
/// Con un colchon menor que el bloque, PulseAudio lo parte en trozos del tamano del colchon,
/// llegan todos de golpe a un anillo de GStreamer en el que solo cabe uno, y se pierde sonido.
/// Medido en Ubuntu 22.04 con esa fuente: con 200 ms faltaban 190 ms y lo demas caia 1,8 s
/// tarde (y con el procesador ocupado se perdian 5 s de cada 8); con 1 s faltaban 990 ms; con
/// 2 s, 3 s y 6 s llega entero y en su sitio. 2 s es el bloque mas grande que usa PulseAudio:
/// 3 s lo cubre con margen. Con una fuente normal (el monitor de una salida, que da trozos de
/// 10 ms) sale lo mismo con 200 ms que con 3 s.
const COLCHON_US: u32 = 3_000_000;

/// Hay un PulseAudio al que hablarle: su socket esta donde lo deja siempre. No dice si
/// contesta, solo que este sistema lo tiene.
fn hay_pulseaudio() -> bool {
    if let Some(servidor) = std::env::var_os("PULSE_SERVER").filter(|s| !s.is_empty()) {
        let servidor = servidor.to_string_lossy().into_owned();
        return std::path::Path::new(servidor.trim_start_matches("unix:")).exists();
    }
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .is_some_and(|carpeta| carpeta.join("pulse/native").exists())
}

/// Que dice un aparato de PulseAudio: una salida, una entrada o ninguna de las dos. El
/// «monitor» de una salida (por donde se graba lo que suena) aparece como una entrada mas,
/// y no es un microfono.
pub(crate) fn clase_de_pulse(clase: &str, es_monitor: bool) -> Disponible {
    Disponible {
        altavoz: clase == "Audio/Sink",
        microfono: clase == "Audio/Source" && !es_monitor,
        ..Disponible::default()
    }
}

/// Que sonido se puede grabar por PulseAudio. Sin PulseAudio, nada. Con PulseAudio pero sin
/// el plugin de GStreamer que lo graba, nada tampoco, y lo dice (`falta_el_de_pulse`) para
/// que los ajustes digan que instalar.
pub(super) fn disponible() -> Disponible {
    if motor::encender().is_err() {
        return Disponible::default();
    }
    let Some(proveedor) = gst::DeviceProviderFactory::by_name(PROVEEDOR) else {
        return Disponible { falta_el_de_pulse: hay_pulseaudio(), ..Disponible::default() };
    };
    if proveedor.start().is_err() {
        // No contesta ningun PulseAudio.
        return Disponible::default();
    }
    let mut hay = Disponible::default();
    for aparato in proveedor.devices().iter() {
        let es_monitor = aparato
            .properties()
            .is_some_and(|p| p.get::<&str>("device.class").is_ok_and(|clase| clase == "monitor"));
        let este = clase_de_pulse(aparato.device_class().as_str(), es_monitor);
        hay.altavoz |= este.altavoz;
        hay.microfono |= este.microfono;
    }
    proveedor.stop();
    hay
}

/// En que instante del reloj de la grabacion acaba un trozo: ahora, menos lo que hace que se
/// grabo. `ahora_ns` es el reloj de la tuberia en este momento y `acaba_ns` la marca del
/// trozo mas lo que dura, los dos en el mismo reloj. Sin marca, o con una disparatada, el
/// trozo cuenta como recien llegado.
pub(crate) fn acaba_en(reloj_ms: u64, ahora_ns: Option<u64>, acaba_ns: Option<u64>) -> u64 {
    let retraso_ms = match (ahora_ns, acaba_ns) {
        (Some(ahora), Some(acaba)) => ahora.saturating_sub(acaba) / 1_000_000,
        _ => 0,
    };
    if retraso_ms > RETRASO_CREIBLE_MS {
        reloj_ms
    } else {
        reloj_ms.saturating_sub(retraso_ms)
    }
}

/// Un flujo de PulseAudio abierto: lo que suena, o el microfono.
struct Flujo {
    cual: Cual,
    tuberia: gst::Pipeline,
    salida: gst_app::AppSink,
}

impl Flujo {
    fn abrir(cual: Cual) -> std::result::Result<Self, String> {
        let mal = |error: crate::error::AppError| error.to_string();
        // El mismo PCM que se le pide a PipeWire. `pulsesrc` ya pide a PulseAudio trozos de
        // 10 ms (`latency-time`); se escribe para que no dependa de lo que traiga de serie.
        let tuberia = motor::monta(&format!(
            "pulsesrc name=f latency-time=10000 buffer-time={COLCHON_US} ! audioconvert ! audioresample ! \
             audio/x-raw,format=S16LE,rate={},channels={},layout=interleaved ! \
             appsink name=s sync=false max-buffers=1000 drop=true",
            FORMATO.sample_rate, FORMATO.channels
        ))
        .map_err(mal)?;
        if matches!(cual, Cual::Sistema) {
            // El «monitor» de la salida por defecto: lo resuelve el propio PulseAudio.
            motor::pieza::<gst::Element>(&tuberia, "f").map_err(mal)?.set_property("device", "@DEFAULT_MONITOR@");
        }
        // Con el reloj del sistema, la marca de cada trozo y el «ahora» de la tuberia van en
        // el mismo reloj, y no en el de un flujo de sonido que puede tardar en arrancar.
        tuberia.use_clock(Some(&gst::SystemClock::obtain()));
        let salida: gst_app::AppSink = motor::pieza(&tuberia, "s").map_err(mal)?;
        let este = Self { cual, tuberia, salida };
        if este.tuberia.set_state(gst::State::Playing).is_err() {
            let porque = este.tuberia.bus().and_then(|bus| motor::error_de(&bus));
            return Err(porque.unwrap_or_else(|| "PulseAudio no ha querido empezar".into()));
        }
        Ok(este)
    }

    /// Pasa a `recibir` todo lo que haya llegado. Dice si habia algo.
    fn vaciar(&self, marcha: &Marcha, recibir: &Recibir) -> bool {
        let mut habia = false;
        while let Some(muestra) = self.salida.try_pull_sample(gst::ClockTime::ZERO) {
            habia = true;
            if marcha.en_pausa() || marcha.parada() {
                continue;
            }
            let Some(bufer) = muestra.buffer() else { continue };
            let Ok(pcm) = bufer.map_readable() else { continue };
            let marcos = (pcm.len() / (2 * usize::from(FORMATO.channels))) as u64;
            let dura_ns = marcos * 1_000_000_000 / u64::from(FORMATO.sample_rate);
            let ms = acaba_en(
                marcha.ms(),
                self.tuberia.current_running_time().map(|t| t.nseconds()),
                bufer.pts().map(|t| t.nseconds() + dura_ns),
            );
            recibir(self.cual, &pcm, ms);
        }
        habia
    }
}

impl Drop for Flujo {
    fn drop(&mut self) {
        let _ = self.tuberia.set_state(gst::State::Null);
    }
}

/// Graba de PulseAudio lo pedido hasta que `Marcha::stop` se pone a cierto. Lo mismo que
/// `sonido::correr_en_pipewire`: si uno de los dos flujos no se puede abrir, el otro se
/// graba igual.
pub(super) fn correr(fuentes: Fuentes, marcha: &Marcha, recibir: Recibir) -> std::result::Result<(), String> {
    motor::encender().map_err(|error| error.to_string())?;
    let mut flujos = Vec::new();
    if fuentes.sistema {
        match Flujo::abrir(Cual::Sistema) {
            Ok(flujo) => flujos.push(flujo),
            Err(error) => eprintln!("[grabar] sin el sonido del sistema: {error}"),
        }
    }
    if fuentes.microfono {
        match Flujo::abrir(Cual::Microfono) {
            Ok(flujo) => flujos.push(flujo),
            Err(error) => eprintln!("[grabar] sin el micrófono: {error}"),
        }
    }
    if flujos.is_empty() {
        return Err("PulseAudio no deja grabar nada de lo pedido".into());
    }
    while !marcha.parada() {
        let mut habia = false;
        for flujo in &flujos {
            habia |= flujo.vaciar(marcha, &recibir);
        }
        if !habia {
            std::thread::sleep(LATIDO);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_monitor_de_una_salida_no_es_un_microfono() {
        let nada = Disponible::default();
        assert_eq!(clase_de_pulse("Audio/Sink", false), Disponible { altavoz: true, ..nada });
        assert_eq!(clase_de_pulse("Audio/Source", false), Disponible { microfono: true, ..nada });
        // Lo que hay en un PC sin microfono: la salida y su monitor. No hay microfono.
        assert_eq!(clase_de_pulse("Audio/Source", true), nada);
        assert_eq!(clase_de_pulse("Video/Source", false), nada);
    }

    /// Un trozo se coloca por la hora a la que se grabo, no por cuando llega: si se mira
    /// 300 ms tarde, sigue cayendo en su sitio.
    #[test]
    fn un_trozo_se_coloca_por_su_marca_y_no_por_cuando_llega() {
        let s = 1_000_000_000u64;
        // El reloj de la grabacion va por 4000 ms; la tuberia, por 10 s; el trozo acabo en
        // 9,7 s de la tuberia: hace 300 ms.
        assert_eq!(acaba_en(4_000, Some(10 * s), Some(10 * s - 300_000_000)), 3_700);
        // Recien grabado: ahora mismo.
        assert_eq!(acaba_en(4_000, Some(10 * s), Some(10 * s)), 4_000);
        // Una marca por delante del reloj (el vaiven de dos relojes): ahora, nunca despues.
        assert_eq!(acaba_en(4_000, Some(10 * s), Some(10 * s + 5_000_000)), 4_000);
    }

    #[test]
    fn sin_marca_o_con_una_disparatada_cuenta_como_recien_llegado() {
        let s = 1_000_000_000u64;
        assert_eq!(acaba_en(4_000, Some(10 * s), None), 4_000);
        assert_eq!(acaba_en(4_000, None, Some(10 * s)), 4_000);
        assert_eq!(acaba_en(4_000, Some(100 * s), Some(10 * s)), 4_000, "noventa segundos de retraso no se creen");
        // Y al principio de la grabacion no se va por debajo de cero.
        assert_eq!(acaba_en(100, Some(10 * s), Some(10 * s - 300_000_000)), 0);
    }

    /// Preguntar no puede reventar ni quedarse colgado, haya o no PulseAudio y este o no el
    /// plugin (en la CI no hay ninguno de los dos).
    #[test]
    fn preguntar_que_hay_en_pulseaudio_no_revienta() {
        let hay = disponible();
        eprintln!("[sonido] PulseAudio dice: {hay:?}");
        assert!(!(hay.falta_el_de_pulse && (hay.altavoz || hay.microfono)), "si falta el plugin no puede haber visto nada");
    }
}
