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
    eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED,
};

use crate::error::{AppError, Result};

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
pub fn empezar() -> Result<Captura> {
    let (formato_tx, formato_rx) = mpsc::channel::<Result<Formato>>();
    let (tx, rx) = mpsc::channel::<Trozo>();
    let parar = Arc::new(AtomicBool::new(false));
    let bandera = Arc::clone(&parar);

    // Todo el trabajo de COM vive en su propio hilo, del principio al fin: los objetos de
    // audio pertenecen al hilo que los crea y no se pueden pasear por otros.
    let hilo = std::thread::spawn(move || {
        let resultado = capturar(&formato_tx, &tx, &bandera);
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
    formato_tx: &Sender<Result<Formato>>,
    tx: &Sender<Trozo>,
    parar: &AtomicBool,
) -> Result<()> {
    // El guardian cierra COM pase lo que pase: sin esto, un error a medio camino deja el
    // hilo con COM inicializado y la siguiente grabacion se encuentra el estropicio.
    let _com = Com::inicializar()?;

    let (cliente, captura, formato) = unsafe { abrir_altavoz() }?;
    if formato_tx.send(Ok(formato)).is_err() {
        return Ok(());
    }

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
    Ok(())
}

/// Abre el dispositivo de salida por defecto en modo loopback y devuelve con que formato
/// esta trabajando.
unsafe fn abrir_altavoz() -> Result<(IAudioClient, IAudioCaptureClient, Formato)> {
    let enumerador: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
        .map_err(|e| AppError::Msg(format!("no se encuentran los dispositivos de sonido: {e}")))?;
    // `eRender` es la salida (los altavoces) y `eConsole` el uso normal, no el de
    // comunicaciones: es el que sigue al altavoz que el usuario tiene puesto.
    let dispositivo = enumerador
        .GetDefaultAudioEndpoint(eRender, eConsole)
        .map_err(|e| AppError::Msg(format!("no hay altavoz por defecto: {e}")))?;

    let cliente: IAudioClient = dispositivo
        .Activate(CLSCTX_ALL, None)
        .map_err(|e| AppError::Msg(format!("no se puede abrir el altavoz: {e}")))?;

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

    let inicio = cliente.Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        AUDCLNT_STREAMFLAGS_LOOPBACK,
        COLCHON_100NS,
        0,
        formato_ptr,
        None,
    );
    CoTaskMemFree(Some(formato_ptr as *const _));
    inicio.map_err(|e| AppError::Msg(format!("no se puede escuchar el altavoz: {e}")))?;

    let captura: IAudioCaptureClient = cliente
        .GetService()
        .map_err(|e| AppError::Msg(format!("no se puede leer del altavoz: {e}")))?;

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
        let captura = empezar().expect("no se ha podido abrir el altavoz");
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
