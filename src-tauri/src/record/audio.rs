//! El sonido que sale por los altavoces, capturado para meterlo en el MP4.
//!
//! Windows lo llama *loopback*: en vez de grabar de un microfono, se abre el altavoz por
//! defecto y se pide lo que ESTA SONANDO. No hay que pedir permiso ni desviar nada, y el
//! usuario sigue oyendo su musica igual mientras se graba.
//!
//! `windows-capture` no trae el audio: entrega imagen y punto. Lo que se saca de aqui se
//! le pasa despues al codificador con `send_audio_buffer`.

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED,
};

use crate::error::{AppError, Result};

/// De donde sale el sonido que se graba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fuentes {
    /// Lo que suena por los altavoces.
    pub sistema: bool,
    /// Lo que entra por el microfono.
    pub microfono: bool,
}

impl Fuentes {
    pub fn ninguna(&self) -> bool {
        !self.sistema && !self.microfono
    }
}

/// Como viene el sonido: hace falta para decirle al codificador que esta recibiendo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Formato {
    pub canales: u16,
    pub muestras_por_segundo: u32,
    pub bits_por_muestra: u16,
}

impl Formato {
    /// Cuantos bytes ocupa un instante de sonido con todos sus canales.
    pub fn bytes_por_instante(&self) -> u32 {
        u32::from(self.canales) * u32::from(self.bits_por_muestra) / 8
    }

    /// Cuanto dura, en cien nanosegundos, un trozo de sonido de este tamanno. Es la unidad
    /// que usa Windows para los tiempos de los medios, y la que espera el codificador.
    pub fn duracion_de(&self, bytes: usize) -> i64 {
        let por_instante = self.bytes_por_instante().max(1) as u64;
        let instantes = bytes as u64 / por_instante;
        (instantes * 10_000_000 / u64::from(self.muestras_por_segundo.max(1))) as i64
    }
}

/// Un trozo de sonido tal y como lo entrega Windows, con el momento en que empieza.
pub struct Trozo {
    pub datos: Vec<u8>,
    /// Desde que empezo la grabacion, en cien nanosegundos.
    pub desde_el_inicio: i64,
}

/// La captura en marcha. Al soltarla se para el hilo y se cierra todo.
pub struct Captura {
    parar: Arc<AtomicBool>,
    hilo: Option<std::thread::JoinHandle<()>>,
    pub formato: Formato,
    pub trozos: Receiver<Trozo>,
}

impl Captura {
    /// Pide que pare y espera a que el hilo cierre COM antes de seguir.
    pub fn parar(mut self) {
        self.parar.store(true, Ordering::Relaxed);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

impl Drop for Captura {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

/// Cada cuanto se recoge lo que haya sonando. Con menos, el hilo se pasa el dia
/// despertandose para nada; con mucho mas, el bufer de Windows se llena y se pierde audio.
const CADA: std::time::Duration = std::time::Duration::from_millis(10);

/// Lo que se le pide a Windows de colchon: dos decimas. Si el hilo se retrasa mas que
/// esto, Windows tira lo viejo y avisa con su marca de discontinuidad.
const COLCHON_100NS: i64 = 2_000_000;

/// Abre el altavoz por defecto y empieza a recoger lo que suena.
///
/// Devuelve el formato antes de arrancar el hilo a proposito: quien codifica necesita
/// saber cuantos canales y a que frecuencia ANTES de que llegue el primer trozo.
pub fn empezar(fuentes: Fuentes) -> Result<Captura> {
    let (formato_tx, formato_rx) = mpsc::channel::<Result<Formato>>();
    let (tx, rx) = mpsc::channel::<Trozo>();
    let parar = Arc::new(AtomicBool::new(false));
    let bandera = Arc::clone(&parar);

    // Todo el trabajo de COM vive en su propio hilo, del principio al fin: los objetos de
    // audio pertenecen al hilo que los crea y no se pueden pasear por otros.
    let hilo = std::thread::spawn(move || {
        let resultado = capturar(fuentes, &formato_tx, &tx, &bandera);
        if let Err(error) = resultado {
            // Si falla despues de haber dado el formato, ya no hay a quien contarselo por
            // el canal: se deja escrito y la grabacion sigue, muda.
            eprintln!("[winshotx] el audio del sistema se ha cortado: {error}");
        }
    });

    let formato = formato_rx
        .recv()
        .map_err(|_| AppError::Msg("el hilo de audio no ha llegado a arrancar".into()))??;

    Ok(Captura {
        parar,
        hilo: Some(hilo),
        formato,
        trozos: rx,
    })
}

fn capturar(
    fuentes: Fuentes,
    formato_tx: &Sender<Result<Formato>>,
    tx: &Sender<Trozo>,
    parar: &AtomicBool,
) -> Result<()> {
    // El guardian cierra COM pase lo que pase: sin esto, un error a medio camino deja el
    // hilo con COM inicializado y la siguiente grabacion se encuentra el estropicio.
    let _com = Com::inicializar()?;

    // El maestro es el que marca el ritmo y el formato del MP4: el sistema si esta, y si
    // no, el microfono. Solo el maestro decide cuando sale un trozo; lo del otro se le
    // suma encima. Con dos relojes independientes marcando el compas, el sonido se
    // desalinearia sin remedio a los pocos segundos.
    let maestro = if fuentes.sistema {
        Dispositivo::Altavoz
    } else {
        Dispositivo::Microfono
    };
    let (cliente, captura, formato) = match unsafe { abrir(maestro) } {
        Ok(abierto) => abierto,
        Err(error) => {
            let _ = formato_tx.send(Err(error));
            return Ok(());
        }
    };
    if formato_tx.send(Ok(formato)).is_err() {
        return Ok(());
    }

    // El acompannante, si se han pedido los dos. Que falle no para la grabacion: es peor
    // quedarse sin video por no tener microfono que grabar sin voz.
    let mut acompannante = if fuentes.sistema && fuentes.microfono {
        match unsafe { abrir(Dispositivo::Microfono) } {
            Ok((cliente_mic, captura_mic, formato_mic)) => {
                match unsafe { cliente_mic.Start() } {
                    Ok(()) => Some(Acompannante {
                        cliente: cliente_mic,
                        captura: captura_mic,
                        formato: formato_mic,
                        pendiente: Vec::new(),
                    }),
                    Err(error) => {
                        eprintln!("[winshotx] el microfono no arranca: {error}");
                        None
                    }
                }
            }
            Err(error) => {
                eprintln!("[winshotx] sin microfono: {error}");
                None
            }
        }
    } else {
        None
    };

    unsafe { cliente.Start() }.map_err(|e| AppError::Msg(format!("no arranca el audio: {e}")))?;

    let mut escritos: usize = 0;
    while !parar.load(Ordering::Relaxed) {
        std::thread::sleep(CADA);
        loop {
            let disponible = unsafe { captura.GetNextPacketSize() }
                .map_err(|e| AppError::Msg(format!("no se puede leer el audio: {e}")))?;
            if disponible == 0 {
                break;
            }

            let mut datos = std::ptr::null_mut();
            let mut instantes = 0u32;
            let mut banderas = 0u32;
            unsafe {
                captura.GetBuffer(&mut datos, &mut instantes, &mut banderas, None, None)
            }
            .map_err(|e| AppError::Msg(format!("no se puede coger el audio: {e}")))?;

            let bytes = instantes as usize * formato.bytes_por_instante() as usize;
            // AUDCLNT_BUFFERFLAGS_SILENT vale 2: Windows dice "aqui no suena nada" y no se
            // molesta en rellenar el bufer. Hay que meter el silencio a mano, porque si no
            // el sonido se adelanta y se despega de la imagen.
            let trozo = if banderas & 0x2 != 0 || datos.is_null() {
                vec![0u8; bytes]
            } else {
                unsafe { std::slice::from_raw_parts(datos, bytes) }.to_vec()
            };
            unsafe { captura.ReleaseBuffer(instantes) }
                .map_err(|e| AppError::Msg(format!("no se puede soltar el audio: {e}")))?;

            // Y encima, la voz. `mezclar_encima` coge del microfono justo lo que dura
            // este trozo; si el microfono va con retraso, lo que falte se queda en
            // silencio y el sistema se sigue oyendo entero.
            let trozo = match acompannante.as_mut() {
                Some(mic) => mezclar_encima(trozo, formato, mic),
                None => trozo,
            };

            let desde_el_inicio = formato.duracion_de(escritos);
            escritos += trozo.len();
            if tx
                .send(Trozo {
                    datos: trozo,
                    desde_el_inicio,
                })
                .is_err()
            {
                // Ya no hay nadie escuchando: la grabacion ha terminado.
                break;
            }
        }
    }

    let _ = unsafe { cliente.Stop() };
    if let Some(mic) = acompannante.as_ref() {
        let _ = unsafe { mic.cliente.Stop() };
    }
    Ok(())
}

/// El microfono cuando acompanna al sistema, con lo que le sobro de la ultima vuelta.
struct Acompannante {
    cliente: IAudioClient,
    captura: IAudioCaptureClient,
    formato: Formato,
    /// Muestras ya adaptadas al formato del maestro y todavia sin gastar.
    pendiente: Vec<f32>,
}

/// Suma sobre el trozo del maestro lo que haya llegado del microfono.
///
/// El microfono se lee entero cada vez y se guarda en `pendiente`, ya convertido al
/// formato del maestro. De ahi se gasta exactamente lo que dura este trozo: ni mas, para
/// no adelantar la voz, ni menos, para no irla acumulando.
fn mezclar_encima(trozo: Vec<u8>, formato: Formato, mic: &mut Acompannante) -> Vec<u8> {
    if let Some(nuevas) = unsafe { leer_todo(&mic.captura, mic.formato) } {
        mic.pendiente
            .extend(super::mezcla::adaptar(&nuevas, mic.formato, formato));
    }
    let mut muestras = super::mezcla::como_flotantes(&trozo);
    let cuantas = muestras.len().min(mic.pendiente.len());
    if cuantas == 0 {
        return trozo;
    }
    super::mezcla::sumar(&mut muestras, &mic.pendiente[..cuantas]);
    mic.pendiente.drain(..cuantas);
    super::mezcla::como_bytes(&muestras)
}

/// Vacia el bufer de un dispositivo y devuelve sus muestras, o `None` si no habia nada.
unsafe fn leer_todo(captura: &IAudioCaptureClient, formato: Formato) -> Option<Vec<f32>> {
    let mut todo: Vec<f32> = Vec::new();
    loop {
        let disponible = unsafe { captura.GetNextPacketSize() }.ok()?;
        if disponible == 0 {
            break;
        }
        let mut datos = std::ptr::null_mut();
        let mut instantes = 0u32;
        let mut banderas = 0u32;
        unsafe { captura.GetBuffer(&mut datos, &mut instantes, &mut banderas, None, None) }.ok()?;
        let bytes = instantes as usize * formato.bytes_por_instante() as usize;
        if banderas & 0x2 != 0 || datos.is_null() {
            todo.extend(std::iter::repeat_n(
                0.0,
                bytes / std::mem::size_of::<f32>(),
            ));
        } else {
            let crudo = unsafe { std::slice::from_raw_parts(datos, bytes) };
            todo.extend(super::mezcla::como_flotantes(crudo));
        }
        let _ = unsafe { captura.ReleaseBuffer(instantes) };
    }
    (!todo.is_empty()).then_some(todo)
}

/// Que aparato se abre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dispositivo {
    /// Los altavoces, en modo loopback: se graba lo que ESTA SONANDO.
    Altavoz,
    /// El microfono, que se graba como cualquier entrada, sin loopback.
    Microfono,
}

/// Abre el aparato que se le pida y devuelve con que formato esta trabajando.
unsafe fn abrir(que: Dispositivo) -> Result<(IAudioClient, IAudioCaptureClient, Formato)> {
    let enumerador: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
        .map_err(|e| AppError::Msg(format!("no se encuentran los dispositivos de sonido: {e}")))?;
    // `eRender` es la salida (los altavoces) y `eCapture` la entrada (el microfono).
    // `eConsole` es el uso normal, no el de comunicaciones: es el que sigue al aparato que
    // el usuario tiene puesto en Windows.
    let (flujo, nombre) = match que {
        Dispositivo::Altavoz => (eRender, "altavoz"),
        Dispositivo::Microfono => (eCapture, "micrófono"),
    };
    let dispositivo = enumerador
        .GetDefaultAudioEndpoint(flujo, eConsole)
        .map_err(|e| AppError::Msg(format!("no hay {nombre} por defecto: {e}")))?;

    let cliente: IAudioClient = dispositivo
        .Activate(CLSCTX_ALL, None)
        .map_err(|e| AppError::Msg(format!("no se puede abrir el {nombre}: {e}")))?;

    let formato_ptr = cliente
        .GetMixFormat()
        .map_err(|e| AppError::Msg(format!("no se sabe en qué formato suena: {e}")))?;
    // El formato lo reserva Windows y hay que devolverlo, tanto si esto sale bien como si
    // no: se copia lo que interesa y se suelta en el acto.
    let formato = Formato {
        canales: (*formato_ptr).nChannels,
        muestras_por_segundo: (*formato_ptr).nSamplesPerSec,
        bits_por_muestra: (*formato_ptr).wBitsPerSample,
    };

    // El loopback ("dame lo que esta sonando") solo existe para una salida. Pedirselo a
    // un microfono lo rechaza: una entrada ya se graba de por si.
    let banderas = match que {
        Dispositivo::Altavoz => AUDCLNT_STREAMFLAGS_LOOPBACK,
        Dispositivo::Microfono => 0,
    };
    let inicio = cliente.Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        banderas,
        COLCHON_100NS,
        0,
        formato_ptr,
        None,
    );
    CoTaskMemFree(Some(formato_ptr as *const _));
    inicio.map_err(|e| AppError::Msg(format!("no se puede escuchar el {nombre}: {e}")))?;

    let captura: IAudioCaptureClient = cliente
        .GetService()
        .map_err(|e| AppError::Msg(format!("no se puede leer del {nombre}: {e}")))?;

    Ok((cliente, captura, formato))
}

/// COM inicializado mientras viva, y cerrado al soltarlo.
struct Com;

impl Com {
    fn inicializar() -> Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|e| AppError::Msg(format!("no se puede preparar el sonido: {e}")))?;
        Ok(Self)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// Pasa el sonido de coma flotante a enteros de 16 bits.
///
/// El mezclador de Windows entrega el sonido en coma flotante de 32 bits, y el
/// codificador AAC quiere enteros de 16. Sin esta conversion el MP4 sale con la pista
/// muda o con un chirrido, que es peor que no tener sonido.
pub fn a_pcm16(flotantes: &[u8]) -> Vec<u8> {
    let mut salida = Vec::with_capacity(flotantes.len() / 2);
    for muestra in flotantes.chunks_exact(4) {
        let valor = f32::from_le_bytes([muestra[0], muestra[1], muestra[2], muestra[3]]);
        // Se recorta a [-1, 1] antes de escalar: una muestra por encima de uno daria la
        // vuelta al convertirla y sonaria como un chasquido.
        let recortado = valor.clamp(-1.0, 1.0);
        let entero = (recortado * i16::MAX as f32) as i16;
        salida.extend_from_slice(&entero.to_le_bytes());
    }
    salida
}

/// El formato que de verdad se le entrega al codificador, ya en enteros de 16 bits.
pub fn formato_para_el_mp4(origen: Formato) -> Formato {
    Formato {
        bits_por_muestra: 16,
        ..origen
    }
}

/// Deja el formato en la estructura que espera Windows para los medios. La necesita el
/// codificador para saber que le esta entrando.
pub fn como_waveformat(formato: Formato) -> WAVEFORMATEX {
    WAVEFORMATEX {
        wFormatTag: 3, // WAVE_FORMAT_IEEE_FLOAT, que es lo que da el mezclador de Windows
        nChannels: formato.canales,
        nSamplesPerSec: formato.muestras_por_segundo,
        nAvgBytesPerSec: formato.muestras_por_segundo * formato.bytes_por_instante(),
        nBlockAlign: formato.bytes_por_instante() as u16,
        wBitsPerSample: formato.bits_por_muestra,
        cbSize: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Las cuentas del tiempo, que son las que sincronizan el sonido con la imagen. Un
    /// error aqui no se ve: se OYE, y tarde, cuando alguien reproduce el video.
    #[test]
    fn un_segundo_de_sonido_dura_un_segundo() {
        let formato = Formato {
            canales: 2,
            muestras_por_segundo: 48_000,
            bits_por_muestra: 32,
        };
        // Dos canales de cuatro bytes: ocho bytes por instante.
        assert_eq!(formato.bytes_por_instante(), 8);
        // Un segundo entero son 48.000 instantes, o sea 384.000 bytes.
        assert_eq!(formato.duracion_de(384_000), 10_000_000);
        // Y medio segundo, la mitad.
        assert_eq!(formato.duracion_de(192_000), 5_000_000);
        assert_eq!(formato.duracion_de(0), 0);
    }

    /// El mono a 44.100 tambien tiene que salir bien: no todo el mundo tiene el altavoz
    /// en estereo a 48 kHz.
    #[test]
    fn el_mono_a_cuarenta_y_cuatro_mil_cien_tambien_cuadra() {
        let formato = Formato {
            canales: 1,
            muestras_por_segundo: 44_100,
            bits_por_muestra: 16,
        };
        assert_eq!(formato.bytes_por_instante(), 2);
        assert_eq!(formato.duracion_de(88_200), 10_000_000);
    }

    /// El silencio, el maximo y el minimo, que son los tres sitios donde una conversion
    /// mal hecha se oye: un chasquido en cada extremo.
    #[test]
    fn el_sonido_pasa_a_enteros_sin_chasquidos() {
        let mut bytes = Vec::new();
        for valor in [0.0f32, 1.0, -1.0, 0.5, 2.0, -3.0] {
            bytes.extend_from_slice(&valor.to_le_bytes());
        }
        let pcm = a_pcm16(&bytes);
        let leidos: Vec<i16> = pcm
            .chunks_exact(2)
            .map(|p| i16::from_le_bytes([p[0], p[1]]))
            .collect();
        assert_eq!(leidos[0], 0, "el silencio tiene que quedarse en cero");
        assert_eq!(leidos[1], i16::MAX, "el máximo se va al tope");
        assert_eq!(leidos[2], -i16::MAX, "y el mínimo, al otro tope");
        assert_eq!(leidos[3], i16::MAX / 2, "la mitad, a la mitad");
        assert_eq!(leidos[4], i16::MAX, "por encima de uno se recorta, no da la vuelta");
        assert_eq!(leidos[5], -i16::MAX, "y por debajo de menos uno, igual");
        assert_eq!(pcm.len(), bytes.len() / 2, "ocupa la mitad, que es el objetivo");
    }

    /// Abre el microfono de verdad y comprueba que entrega sonido.
    ///
    /// Va con `--ignored` porque necesita un microfono conectado:
    /// `cargo test --lib escuchar_el_microfono -- --ignored --nocapture`
    ///
    /// Lo que comprueba y no se puede saber leyendo el codigo: que una ENTRADA se abre sin
    /// la bandera de loopback. Con ella puesta, Windows rechaza el dispositivo y el
    /// microfono no graba nada, sin mas explicacion que un codigo de error.
    #[test]
    #[ignore]
    fn escuchar_el_microfono() {
        let captura = empezar(Fuentes {
            sistema: false,
            microfono: true,
        })
        .expect("abrir el micrófono");
        println!(
            "el micrófono da {} canales a {} Hz y {} bits",
            captura.formato.canales,
            captura.formato.muestras_por_segundo,
            captura.formato.bits_por_muestra
        );
        std::thread::sleep(std::time::Duration::from_millis(600));

        let mut trozos = 0;
        let mut bytes = 0usize;
        while let Ok(trozo) = captura.trozos.try_recv() {
            trozos += 1;
            bytes += trozo.datos.len();
        }
        captura.parar();
        println!(
            "{trozos} trozos, {bytes} bytes, {:.2} s de sonido",
            bytes as f64 / (48_000.0 * 8.0)
        );
        assert!(trozos > 0, "el micrófono no ha entregado ni un trozo");
    }

    /// Los dos a la vez, que es el caso de un tutorial narrado.
    ///
    /// Comprueba lo unico que aqui puede salir mal de verdad: que el trozo mezclado sigue
    /// midiendo lo que medía. Si la mezcla alargara o acortara los trozos, el sonido se
    /// iria desplazando poco a poco y a los dos minutos ya no cuadraria con la imagen.
    #[test]
    #[ignore]
    fn el_sistema_y_el_microfono_a_la_vez_no_cambian_la_duracion() {
        let captura = empezar(Fuentes {
            sistema: true,
            microfono: true,
        })
        .expect("abrir los dos");
        let formato = captura.formato;
        std::thread::sleep(std::time::Duration::from_millis(600));

        let mut bytes = 0usize;
        let mut ultimo_inicio = 0i64;
        while let Ok(trozo) = captura.trozos.try_recv() {
            // Cada trozo empieza justo donde acababa el anterior: la marca de tiempo se
            // calcula contando bytes, así que si la mezcla cambiara el tamaño, aquí se
            // vería un salto.
            assert_eq!(
                trozo.desde_el_inicio,
                formato.duracion_de(bytes),
                "el trozo mezclado no empieza donde acababa el anterior"
            );
            assert!(trozo.desde_el_inicio >= ultimo_inicio, "el tiempo va hacia atrás");
            ultimo_inicio = trozo.desde_el_inicio;
            bytes += trozo.datos.len();
        }
        captura.parar();
        println!("{bytes} bytes mezclados, {:.2} s", bytes as f64 / (48_000.0 * 8.0));
        assert!(bytes > 0, "no ha llegado sonido de ninguna de las dos fuentes");
    }

    /// La estructura que se le pasa a Windows tiene que ser coherente consigo misma, o el
    /// codificador la rechaza sin decir por que.
    #[test]
    fn el_formato_para_windows_cuadra_consigo_mismo() {
        let formato = Formato {
            canales: 2,
            muestras_por_segundo: 48_000,
            bits_por_muestra: 32,
        };
        let wave = como_waveformat(formato);
        // `WAVEFORMATEX` viene empaquetada, asi que no se puede coger una referencia a sus
        // campos: se copian a variables sueltas antes de compararlos.
        let (canales, bits, alineacion) = (wave.nChannels, wave.wBitsPerSample, wave.nBlockAlign);
        let (por_segundo, muestras) = (wave.nAvgBytesPerSec, wave.nSamplesPerSec);
        assert_eq!(u32::from(alineacion), u32::from(canales) * u32::from(bits) / 8);
        assert_eq!(por_segundo, muestras * u32::from(alineacion));
    }
}

#[cfg(test)]
mod prueba_de_verdad {
    use super::*;

    /// Abre el altavoz de verdad y comprueba que Windows entrega sonido. No corre sola
    /// porque necesita una tarjeta de sonido y medio segundo:
    /// `cargo test --lib escuchar_el_altavoz -- --ignored --nocapture`.
    ///
    /// Con el equipo en silencio tambien pasa: WASAPI en loopback entrega silencio, pero
    /// lo entrega. Lo que se comprueba es que el grifo esta abierto.
    #[test]
    #[ignore]
    fn escuchar_el_altavoz_de_verdad() {
        let captura = empezar(Fuentes {
            sistema: true,
            microfono: false,
        })
        .expect("no se ha podido abrir el altavoz");
        println!(
            "altavoz: {} canales a {} Hz, {} bits",
            captura.formato.canales,
            captura.formato.muestras_por_segundo,
            captura.formato.bits_por_muestra
        );
        std::thread::sleep(std::time::Duration::from_millis(600));

        let mut bytes = 0usize;
        let mut trozos = 0usize;
        let mut con_sonido = 0usize;
        while let Ok(trozo) = captura.trozos.try_recv() {
            bytes += trozo.datos.len();
            trozos += 1;
            if trozo.datos.iter().any(|b| *b != 0) {
                con_sonido += 1;
            }
        }
        let segundos = captura.formato.duracion_de(bytes) as f64 / 10_000_000.0;
        println!("{trozos} trozos, {bytes} bytes, {segundos:.2} s de sonido, {con_sonido} con algo sonando");
        captura.parar();

        assert!(trozos > 0, "Windows no ha entregado ni un trozo de sonido");
        assert!(
            segundos > 0.3,
            "en 0,6 s deberían llegar más de 0,3 s de sonido y han llegado {segundos:.2}"
        );
    }
}
