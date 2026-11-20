//! Lo de la webcam que no depende del sistema: qué deja en la sesión, qué formato de los
//! que da se graba, las marcas de tiempo de su archivo y el hilo que la tiene encendida.
//!
//! Cada sistema pone lo suyo (enumerarlas, encenderla y escribir `camara.mp4`): `camara.rs`
//! con Media Foundation en Windows, y `linux/camara.rs` con el GStreamer del sistema. Lo de
//! aquí es lo mismo en los dos, y lo que hace la cámara se cuenta en `camara.rs`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use super::reloj::Reloj;
use crate::error::Result;

/// Cómo se llama el archivo de la cámara dentro de la carpeta de la sesión.
pub const ARCHIVO: &str = "camara.mp4";

/// Cuánto se espera a que la cámara cierre su archivo al parar. Escribir el índice de un MP4
/// largo tarda algo; más que esto es que la cámara se ha quedado colgada, y entonces la
/// grabación se entrega sin ella antes que dejar al editor esperando.
const ESPERA_AL_PARAR: Duration = Duration::from_secs(10);

/// Una cámara conectada, para elegirla en los ajustes.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Dispositivo {
    /// Con lo que el sistema la reconoce aunque cambie de puerto. Es lo que se guarda en los
    /// ajustes (`camera_device`).
    pub id: String,
    /// Como la llama el sistema: «Integrated Webcam», «Logitech C920».
    pub nombre: String,
}

/// Lo que deja la cámara al terminar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pista {
    /// El archivo, sin la carpeta.
    pub archivo: String,
    pub ancho: u32,
    pub alto: u32,
    /// En qué instante del reloj de la grabación se vio su primera imagen.
    pub primer_ms: u64,
}

/// Cuándo empieza la cámara respecto al primer fotograma de la pantalla, como lo pide
/// `CamaraGrabada::desfase_ms`: positivo es que la cámara arrancó más tarde.
///
/// Para el que la dibuje al exportar: el fotograma de la pantalla que se marcó en
/// `timestamp_ms` lleva encima la imagen de la cámara de
/// `timestamp_ms - frames[0].timestamp_ms - desfase_ms`, en milisegundos desde el principio
/// de `camara.mp4`. Si sale negativo, la cámara todavía no había arrancado.
pub fn desfase(primer_ms_camara: u64, primer_fotograma_ms: u64) -> i64 {
    primer_ms_camara as i64 - primer_fotograma_ms as i64
}

// --- Qué formato se graba ---

/// Uno de los formatos que da la cámara.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Formato {
    pub ancho: u32,
    pub alto: u32,
    pub fps: f64,
    /// MJPG y parecidos: hay que descomprimirlos antes de volver a comprimir.
    pub comprimido: bool,
}

/// Cuál de los formatos de la cámara se graba. `None` si no da ninguno.
///
/// La meta es 720p a unos 30 fotogramas: en una esquina del vídeo se ve de sobra, y es lo
/// que casi todas las cámaras dan sin ahogar el USB. Por orden:
///
/// 1. que vaya de 24 a 31 fotogramas por segundo (menos se ve a saltos, y 60 es el doble
///    de máquina para una imagen que va pequeña);
/// 2. que mida lo más cerca posible de 720 de alto, y antes por arriba que por abajo: una
///    imagen grande se puede reducir al exportar, una pequeña no se puede inventar;
/// 3. a igualdad, la que no viene comprimida, que no hay que descomprimir.
pub(crate) fn elegir(formatos: &[Formato]) -> Option<usize> {
    let puntos = |f: &Formato| {
        let fps = if (23.9..=31.0).contains(&f.fps) {
            0
        } else if f.fps > 31.0 {
            1
        } else if f.fps >= 15.0 {
            2
        } else {
            3
        };
        let alto = if f.alto >= 720 { f.alto - 720 } else { (720 - f.alto) * 2 };
        (fps, alto, f.comprimido)
    };
    formatos
        .iter()
        .enumerate()
        .min_by_key(|(_, f)| puntos(f))
        .map(|(i, _)| i)
}

// --- El archivo ---

/// Cuántos bits por segundo para el H.264 de la cámara: una décima de bit por píxel y
/// fotograma, que en una cara que se mueve poco es de sobra (720p a 30 son 2,8 Mbit/s).
pub(crate) fn bitrate(ancho: u32, alto: u32, fps: f64) -> u32 {
    let bits = f64::from(ancho) * f64::from(alto) * fps.max(1.0) * 0.1;
    bits.clamp(1_000_000.0, 8_000_000.0) as u32
}

// --- Las marcas de tiempo ---

/// Lo que dura una imagen a 30 por segundo, en cien nanosegundos, que es en lo que van las
/// marcas de la cámara (las unidades de Media Foundation; en Linux se pasan a ellas). Se usa
/// hasta saber lo que de verdad da la cámara.
const INTERVALO_DE_SERIE: i64 = 333_333;

/// Pasa las marcas de tiempo de la cámara a las del archivo: empiezan en cero, y lo que dura
/// una pausa de la grabación no está, igual que no está en los fotogramas de la pantalla.
///
/// Mientras la grabación está en pausa, las imágenes de la cámara se tiran. Al volver, el
/// hueco (menos lo que dura una imagen) se descuenta de todo lo que viene detrás, así que
/// el archivo sigue sin saltos y la cámara no se queda atrás de la pantalla.
#[derive(Debug, Default)]
pub(crate) struct Linea {
    primera: Option<i64>,
    ultima_entrada: i64,
    ultima_salida: i64,
    intervalo: i64,
    en_pausa: bool,
    descontado: i64,
}

impl Linea {
    /// La marca que lleva en el archivo una imagen que la cámara marcó con `entrada`, o
    /// `None` si se tira porque la grabación está en pausa.
    pub(crate) fn sello(&mut self, entrada: i64, pausa: bool) -> Option<i64> {
        if pausa {
            self.en_pausa = true;
            return None;
        }
        let Some(primera) = self.primera else {
            self.primera = Some(entrada);
            self.ultima_entrada = entrada;
            self.intervalo = INTERVALO_DE_SERIE;
            self.en_pausa = false;
            return Some(0);
        };
        let hueco = entrada - self.ultima_entrada;
        if self.en_pausa {
            self.descontado += (hueco - self.intervalo).max(0);
            self.en_pausa = false;
        } else if hueco > 0 {
            self.intervalo = hueco;
        }
        self.ultima_entrada = entrada;
        // Siempre hacia delante: dos imágenes con la misma marca no las quiere el MP4.
        let salida = (entrada - primera - self.descontado).max(self.ultima_salida + 1);
        self.ultima_salida = salida;
        Some(salida)
    }
}

// --- La grabación ---

/// Qué imágenes de la cámara entran en el archivo y con qué marca, desde que se le manda
/// grabar.
///
/// Con la cámara encendida de antes (`Encendida`), las primeras que se leen al empezar a
/// grabar pueden haberse sacado ANTES del cero de la grabación: llegan con su retraso. Esas
/// son de antes de grabar y se tiran, así que el archivo empieza en la primera que se sacó
/// desde el cero, y `primer_ms` queda cerca de cero. Lo demás (las pausas, las marcas
/// siempre hacia delante) lo hace `Linea`.
#[derive(Debug, Default)]
pub(crate) struct Toma {
    linea: Linea,
    /// En qué instante del reloj de la grabación se sacó la primera que entró.
    pub(crate) primer_ms: Option<u64>,
}

impl Toma {
    /// La marca en el archivo de una imagen que la cámara marcó con `marca`, o `None` si se
    /// tira. `ahora_ms` es el reloj de la grabación al leerla, y `retraso_ms` lo que tardó en
    /// llegar desde que se sacó (la función `retraso_ms` de cada sistema).
    pub(crate) fn sello(&mut self, marca: i64, ahora_ms: u64, retraso_ms: u64, pausa: bool) -> Option<i64> {
        let sacada = ahora_ms.checked_sub(retraso_ms);
        if self.primer_ms.is_none() && sacada.is_none() {
            return None;
        }
        let sello = self.linea.sello(marca, pausa)?;
        if self.primer_ms.is_none() {
            self.primer_ms = sacada;
        }
        Some(sello)
    }
}

/// Lo que se le manda a la cámara encendida para que empiece a guardar lo que da.
pub(crate) struct Orden {
    pub(crate) reloj: Reloj,
    pub(crate) pausa: Arc<AtomicBool>,
}

/// Cómo está la cámara cuando la grabación deja de esperarla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlEsperar {
    /// Ya da imagen: la cara sale viva desde el primer fotograma.
    Lista,
    /// No ha llegado a encenderse (no hay, la tiene otro programa, Windows no deja usarla):
    /// se graba sin ella, y `Grabadora::terminar` dice por qué.
    Fallida,
    /// Sigue sin dar imagen: se graba igual, y entra en el vídeo cuando llegue, con su
    /// desfase, como antes de encenderla de antemano.
    Tarda,
}

/// La webcam encendida ANTES de grabar, todavía sin guardar nada.
///
/// Encender una cámara tarda (la Insta360 Link 2 Pro de Munito, unos 2 s: 1977 y 2093 ms
/// el 12 de noviembre de 2026), y si se enciende a la vez que la pantalla, la cara sale
/// quieta en su primera imagen todo ese rato. Así que va en dos pasos: `encender` la pone en
/// marcha en su hilo, con la fuente, el lector y el archivo abiertos, leyendo y tirando lo
/// que da (y avisando con la primera imagen, ver `esperar_imagen`); y `grabar`, ya con el
/// reloj de la grabación, empieza a guardar desde ahí.
///
/// Si la grabación no llega a empezar (se suelta sin `grabar`), la cámara se apaga, y con
/// ella su luz: lleva dentro su `Grabadora`, que la para al soltarse.
pub struct Encendida {
    grabadora: Grabadora,
    orden: mpsc::Sender<Orden>,
    aviso: mpsc::Receiver<()>,
    lista: bool,
}

impl Encendida {
    /// `encender`, con lo que corre en el hilo de la cámara puesto aparte: las pruebas le
    /// ponen una cámara de mentira.
    pub(crate) fn con<F>(ruta: PathBuf, hilo: F) -> Self
    where
        F: FnOnce(&Path, &AtomicBool, mpsc::Sender<()>, mpsc::Receiver<Orden>) -> Result<Pista>
            + Send
            + 'static,
    {
        let parar = Arc::new(AtomicBool::new(false));
        let (enviar, resultado) = mpsc::channel();
        let (avisar, aviso) = mpsc::channel();
        let (ordenar, orden) = mpsc::channel();
        let parar_hilo = Arc::clone(&parar);
        std::thread::spawn(move || {
            let hecho = hilo(&ruta, &parar_hilo, avisar, orden);
            if hecho.is_err() {
                let _ = std::fs::remove_file(&ruta);
            }
            let _ = enviar.send(hecho);
        });
        Self {
            grabadora: Grabadora { parar, resultado },
            orden: ordenar,
            aviso,
            lista: false,
        }
    }

    /// Espera como mucho `como_mucho` a que la cámara dé su primera imagen. Vuelve en cuanto
    /// la da, o en cuanto se sabe que no va a dar ninguna (`Fallida`): una cámara que no hay
    /// o que Windows no deja usar no hace esperar a nadie.
    pub fn esperar_imagen(&mut self, como_mucho: Duration) -> AlEsperar {
        if self.lista {
            return AlEsperar::Lista;
        }
        match self.aviso.recv_timeout(como_mucho) {
            Ok(()) => {
                self.lista = true;
                AlEsperar::Lista
            }
            Err(mpsc::RecvTimeoutError::Timeout) => AlEsperar::Tarda,
            // El hilo ha acabado sin dar imagen: ha fallado al encenderla.
            Err(mpsc::RecvTimeoutError::Disconnected) => AlEsperar::Fallida,
        }
    }

    /// Empieza a guardar, con el reloj y la pausa de la grabación: desde aquí, lo que da la
    /// cámara va a `camara.mp4`.
    pub fn grabar(self, reloj: Reloj, pausa: Arc<AtomicBool>) -> Grabadora {
        // Si el hilo ya acabó (la cámara falló), la orden no llega a nadie y `terminar` dice
        // por qué.
        let _ = self.orden.send(Orden { reloj, pausa });
        self.grabadora
    }
}

/// La cámara grabando en su hilo, hasta que se le diga que pare.
pub struct Grabadora {
    parar: Arc<AtomicBool>,
    resultado: mpsc::Receiver<Result<Pista>>,
}

impl Grabadora {
    /// Le dice a la cámara que pare, sin esperarla. Así va cerrando su archivo mientras la
    /// grabación cierra el suyo.
    pub fn avisar(&self) {
        self.parar.store(true, Ordering::Relaxed);
    }

    /// Para la cámara y devuelve lo grabado. `None` si no hubo cámara, y entonces dice por
    /// qué en la consola.
    pub fn terminar(self) -> Option<Pista> {
        self.avisar();
        match self.resultado.recv_timeout(ESPERA_AL_PARAR) {
            Ok(Ok(pista)) => Some(pista),
            Ok(Err(error)) => {
                eprintln!("[winshotx] sin cámara: {error}");
                None
            }
            Err(_) => {
                eprintln!("[winshotx] la cámara no ha cerrado su archivo a tiempo: se entrega sin ella");
                None
            }
        }
    }
}

/// Si la grabación se cae por el camino y nadie llega a llamar a `terminar`, la cámara se
/// para igual: si no, se quedaría encendida, con su luz, hasta cerrar winshotx.
impl Drop for Grabadora {
    fn drop(&mut self) {
        self.avisar();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formato(ancho: u32, alto: u32, fps: f64, comprimido: bool) -> Formato {
        Formato {
            ancho,
            alto,
            fps,
            comprimido,
        }
    }

    /// Lo que da una webcam corriente por USB 2: en YUY2 solo llega a 720p a 10 fotogramas,
    /// y en MJPG a 30. Se queda con el MJPG a 720p, que es lo que se ve bien.
    #[test]
    fn se_queda_con_720p_a_30_aunque_venga_comprimido() {
        let formatos = [
            formato(640, 480, 30.0, false),
            formato(1280, 720, 10.0, false),
            formato(1280, 720, 30.0, true),
            formato(1920, 1080, 30.0, true),
            formato(320, 240, 30.0, false),
        ];
        assert_eq!(elegir(&formatos), Some(2));
    }

    #[test]
    fn a_igualdad_prefiere_el_que_no_hay_que_descomprimir() {
        let formatos = [formato(1280, 720, 30.0, true), formato(1280, 720, 30.0, false)];
        assert_eq!(elegir(&formatos), Some(1));
    }

    #[test]
    fn sin_720p_prefiere_pasarse_que_quedarse_corto() {
        // 1080p se reduce al exportar; 480p no se puede agrandar sin verse borroso.
        let formatos = [formato(640, 480, 30.0, false), formato(1920, 1080, 30.0, true)];
        assert_eq!(elegir(&formatos), Some(1));
    }

    #[test]
    fn antes_30_fotogramas_que_60() {
        let formatos = [formato(1280, 720, 60.0, true), formato(1280, 720, 30.0, true)];
        assert_eq!(elegir(&formatos), Some(1));
    }

    #[test]
    fn sin_formatos_no_elige_nada() {
        assert_eq!(elegir(&[]), None);
    }

    /// 30000/1001 son los 29,97 de las cámaras que siguen la norma americana: valen como 30.
    #[test]
    fn veintinueve_con_noventa_y_siete_cuenta_como_treinta() {
        let formatos = [formato(1280, 720, 15.0, false), formato(1280, 720, 30_000.0 / 1001.0, true)];
        assert_eq!(elegir(&formatos), Some(1));
    }

    #[test]
    fn el_desfase_es_positivo_si_la_camara_arranca_tarde() {
        assert_eq!(desfase(850, 40), 810);
        assert_eq!(desfase(10, 40), -30);
    }

    const FOTO: i64 = 333_333;

    #[test]
    fn el_archivo_empieza_en_cero() {
        let mut linea = Linea::default();
        assert_eq!(linea.sello(77_000_000, false), Some(0));
        assert_eq!(linea.sello(77_000_000 + FOTO, false), Some(FOTO));
        assert_eq!(linea.sello(77_000_000 + 2 * FOTO, false), Some(2 * FOTO));
    }

    /// Cinco segundos de pausa en la grabación no pueden ser cinco segundos de cámara: la
    /// pantalla no los tiene, y desde ahí la cara iría por detrás de lo que se ve.
    #[test]
    fn la_pausa_no_deja_hueco_en_el_archivo() {
        let mut linea = Linea::default();
        let mut entrada = 0;
        for _ in 0..30 {
            linea.sello(entrada, false);
            entrada += FOTO;
        }
        let antes = entrada - FOTO;
        // Cinco segundos en pausa: las imágenes llegan y se tiran.
        for _ in 0..150 {
            assert_eq!(linea.sello(entrada, true), None);
            entrada += FOTO;
        }
        let despues = linea.sello(entrada, false).unwrap();
        // Al volver, la imagen va justo detrás de la última de antes de la pausa.
        assert_eq!(despues, antes + FOTO, "la cámara se ha quedado atrás");
        assert_eq!(linea.sello(entrada + FOTO, false), Some(despues + FOTO));
    }

    #[test]
    fn una_pausa_antes_de_la_primera_imagen_no_cuenta() {
        let mut linea = Linea::default();
        assert_eq!(linea.sello(0, true), None);
        assert_eq!(linea.sello(FOTO, true), None);
        assert_eq!(linea.sello(2 * FOTO, false), Some(0));
    }

    #[test]
    fn las_marcas_siempre_van_hacia_delante() {
        // Una cámara que repite una marca (pasa con algunas al cambiar la luz).
        let mut linea = Linea::default();
        linea.sello(1_000, false);
        let a = linea.sello(1_000 + FOTO, false).unwrap();
        let b = linea.sello(1_000 + FOTO, false).unwrap();
        assert!(b > a, "{b} no va detrás de {a}");
    }

    /// Lo que se sacó antes del cero de la grabación (la cámara ya estaba encendida, y la
    /// imagen llega con su retraso) no entra en el archivo: la primera que entra es la
    /// primera sacada desde el cero, y de ella sale `primer_ms`.
    #[test]
    fn con_la_camara_encendida_de_antes_no_entra_lo_sacado_antes_de_grabar() {
        let mut toma = Toma::default();
        // Leída a los 10 ms de empezar, pero sacada 40 ms antes: es de antes de grabar.
        assert_eq!(toma.sello(5_000_000, 10, 40, false), None);
        assert_eq!(toma.primer_ms, None);
        // Leída a los 43 y sacada a los 3: la primera del archivo.
        assert_eq!(toma.sello(5_000_000 + FOTO, 43, 40, false), Some(0));
        assert_eq!(toma.primer_ms, Some(3));
        // Y desde ahí, como siempre.
        assert_eq!(toma.sello(5_000_000 + 2 * FOTO, 76, 40, false), Some(FOTO));
        assert_eq!(toma.primer_ms, Some(3));
    }

    #[test]
    fn una_pausa_antes_de_la_primera_imagen_tampoco_cuenta_en_la_toma() {
        let mut toma = Toma::default();
        assert_eq!(toma.sello(0, 50, 30, true), None);
        assert_eq!(toma.primer_ms, None);
        assert_eq!(toma.sello(FOTO, 80, 30, false), Some(0));
        assert_eq!(toma.primer_ms, Some(50));
    }
}
