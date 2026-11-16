//! S11 (pistas): el sonido del PC y el micrófono, cada uno en su archivo.
//!
//! Hasta la 0.4.0 el micrófono se sumaba encima del sonido del sistema mientras se grababa
//! (`audio.rs`), y todo acababa en un solo `audio.pcm`: después no había forma de bajar la
//! música sin bajar la voz. Ahora cada fuente va a su archivo, en el mismo reloj y con el
//! mismo largo, y la mezcla se hace al guardar con los volúmenes del editor.
//!
//! Los archivos son WAV y no PCM en crudo para que el editor los pueda poner en un `<audio>`
//! tal cual: la vista previa suena con los dos a la vez y cada uno a su volumen. Por dentro
//! llevan lo mismo que iba a `audio.pcm` (PCM de 16 bits con signo, en el formato del
//! maestro), con la cabecera de 44 bytes de siempre delante.
//!
//! Las grabaciones de antes, y las de la repetición instantánea (que sigue mezclando), no
//! traen `SessionData::pistas`: su pista es `audio.pcm` entera, y se abren y se guardan como
//! siempre.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use super::{AudioInfo, SessionData};

/// El archivo con lo que suena en el PC.
pub const ARCHIVO_PC: &str = "sonido-pc.wav";
/// Y el de la voz.
pub const ARCHIVO_MICROFONO: &str = "microfono.wav";
/// Lo que ocupa la cabecera de un WAV de PCM sin nada más: donde empiezan las muestras.
pub const CABECERA_WAV: u64 = 44;

/// Qué es lo que suena en un archivo de la sesión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueSuena {
    /// Todo junto, como se grababa hasta la 0.4.0 (`audio.pcm`).
    Todo,
    Pc,
    Microfono,
}

/// Un archivo de sonido de la sesión, listo para leer: dónde está, desde qué byte empiezan
/// las muestras y qué lleva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivoDeSonido {
    pub ruta: PathBuf,
    pub desde_byte: u64,
    pub que: QueSuena,
}

/// Los archivos de sonido de una sesión, del PC al micrófono. Vacío si no se grabó sonido.
pub fn de_la_sesion(session: &SessionData) -> Vec<ArchivoDeSonido> {
    if session.audio.is_none() {
        return Vec::new();
    }
    let Some(pistas) = session.pistas.as_ref() else {
        return vec![ArchivoDeSonido {
            ruta: session.audio_path(),
            desde_byte: 0,
            que: QueSuena::Todo,
        }];
    };
    let wav = |nombre: &Option<String>, que| {
        nombre.as_ref().map(|nombre| ArchivoDeSonido {
            ruta: session.dir.join(nombre),
            desde_byte: CABECERA_WAV,
            que,
        })
    };
    [
        wav(&pistas.sistema, QueSuena::Pc),
        wav(&pistas.microfono, QueSuena::Microfono),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// La ruta entera de cada pista que trae la sesión, para que el editor la ponga a sonar.
/// `(None, None)` en las grabaciones de antes: ahí suena el vídeo de vista previa.
pub fn rutas(session: &SessionData) -> (Option<String>, Option<String>) {
    let mut pc = None;
    let mut microfono = None;
    for archivo in de_la_sesion(session) {
        let ruta = Some(archivo.ruta.to_string_lossy().to_string());
        match archivo.que {
            QueSuena::Pc => pc = ruta,
            QueSuena::Microfono => microfono = ruta,
            QueSuena::Todo => {}
        }
    }
    (pc, microfono)
}

/// Todo el sonido de la sesión de una pieza, con las pistas sumadas como se grababa antes.
///
/// Es para la vista previa que se escribe por detrás (`replay::escribir_vista_previa`), que
/// lleva el sonido dentro del MP4 y no separa nada.
pub fn mezcla_entera(session: &SessionData) -> Option<Vec<u8>> {
    let mut mezcla: Option<Vec<u8>> = None;
    for archivo in de_la_sesion(session) {
        let mut datos = std::fs::read(&archivo.ruta).ok()?;
        datos.drain(..(archivo.desde_byte as usize).min(datos.len()));
        mezcla = Some(match mezcla {
            None => datos,
            Some(mut antes) => {
                sumar_pcm16(&mut antes, &datos, 1.0, 1.0);
                antes
            }
        });
    }
    mezcla
}

/// `base = base * a + encima * b`, muestra a muestra en PCM de 16 bits, sin dar la vuelta
/// en los topes: lo que se pasa se queda en el tope, que sonar más alto es mejor que un
/// chasquido. Si `encima` es más larga, `base` crece con silencio para que no se pierda.
pub fn sumar_pcm16(base: &mut Vec<u8>, encima: &[u8], a: f32, b: f32) {
    if encima.len() > base.len() {
        base.resize(encima.len() & !1, 0);
    }
    let mut otra = encima.chunks_exact(2);
    for muestra in base.chunks_exact_mut(2) {
        let de_base = f32::from(i16::from_le_bytes([muestra[0], muestra[1]])) * a;
        let de_encima = otra
            .next()
            .map_or(0.0, |m| f32::from(i16::from_le_bytes([m[0], m[1]])) * b);
        let suma = (de_base + de_encima)
            .round()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
        muestra.copy_from_slice(&suma.to_le_bytes());
    }
}

/// La cabecera de un WAV de PCM de 16 bits con `datos` bytes de sonido detrás.
pub fn cabecera_wav(info: AudioInfo, datos: u64) -> [u8; 44] {
    // Un WAV mide como mucho cuatro gigas; por encima se dice el tope y quien lo lee sigue
    // hasta el final del archivo. A 48 kHz en estéreo son seis horas.
    let datos = datos.min(u64::from(u32::MAX - 36)) as u32;
    let canales = info.channels.max(1);
    let bloque = canales * 2;
    let mut c = [0u8; 44];
    c[0..4].copy_from_slice(b"RIFF");
    c[4..8].copy_from_slice(&(36 + datos).to_le_bytes());
    c[8..12].copy_from_slice(b"WAVE");
    c[12..16].copy_from_slice(b"fmt ");
    c[16..20].copy_from_slice(&16u32.to_le_bytes());
    c[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM entero
    c[22..24].copy_from_slice(&canales.to_le_bytes());
    c[24..28].copy_from_slice(&info.sample_rate.to_le_bytes());
    c[28..32].copy_from_slice(&(info.sample_rate * u32::from(bloque)).to_le_bytes());
    c[32..34].copy_from_slice(&bloque.to_le_bytes());
    c[34..36].copy_from_slice(&16u16.to_le_bytes());
    c[36..40].copy_from_slice(b"data");
    c[40..44].copy_from_slice(&datos.to_le_bytes());
    c
}

/// Un WAV que se va escribiendo mientras se graba. La cabecera se escribe al abrir con el
/// tamaño a cero y se corrige al cerrar, cuando ya se sabe cuánto ha sonado.
pub struct EscritorWav {
    archivo: BufWriter<File>,
    /// Donde está, para poder nivelarlo al cerrar (`cerrar_y_nivelar`).
    ruta: PathBuf,
    info: AudioInfo,
    escritos: u64,
    /// La muestra más alta escrita, en valor absoluto: dice si la pista se oye (`pico`).
    pico: u16,
    /// Lo más alto de cada trozo de 50 ms que sonó (`nivel`), y el trozo que se está llenando.
    picos_de_trozo: Vec<u16>,
    trozo_pico: u16,
    trozo_muestras: u32,
    muestras_por_trozo: u32,
}

/// Lo que dura cada trozo con el que se mide el nivel (`EscritorWav::nivel`).
const MS_POR_TROZO: u32 = 50;
/// Por debajo de esto (-72 dBFS) un trozo es silencio y no cuenta para el nivel: un vídeo en
/// pausa no tiene que bajar la cuenta.
const SONANDO: u16 = 8;

impl EscritorWav {
    pub fn crear(ruta: &Path, info: AudioInfo) -> std::io::Result<Self> {
        let mut archivo = BufWriter::new(File::create(ruta)?);
        archivo.write_all(&cabecera_wav(info, 0))?;
        Ok(Self {
            archivo,
            ruta: ruta.to_path_buf(),
            info,
            escritos: 0,
            pico: 0,
            picos_de_trozo: Vec::new(),
            trozo_pico: 0,
            trozo_muestras: 0,
            muestras_por_trozo: (info.sample_rate * u32::from(info.channels) * MS_POR_TROZO / 1000).max(1),
        })
    }

    /// Añade PCM de 16 bits. Lo que no se pueda escribir se pierde, pero no para la
    /// grabación: la imagen vale más que el sonido.
    pub fn escribir(&mut self, pcm: &[u8]) {
        if self.archivo.write_all(pcm).is_ok() {
            self.escritos += pcm.len() as u64;
            for muestra in pcm.chunks_exact(2) {
                let alto = i16::from_le_bytes([muestra[0], muestra[1]]).unsigned_abs();
                self.pico = self.pico.max(alto);
                self.trozo_pico = self.trozo_pico.max(alto);
                self.trozo_muestras += 1;
                if self.trozo_muestras == self.muestras_por_trozo {
                    if self.trozo_pico >= SONANDO {
                        self.picos_de_trozo.push(self.trozo_pico);
                    }
                    (self.trozo_pico, self.trozo_muestras) = (0, 0);
                }
            }
        }
    }

    /// Lo más alto que sonó la pista, en la escala de 16 bits (0 es silencio exacto y 32768
    /// el tope). Con esto el editor dice «no se grabó nada» en vez de dejar que parezca que
    /// la vista previa está rota: el 14 de noviembre de 2026 el sonido del PC salió a -56 dB
    /// porque Brave estaba al 10 % en el mezclador, y el micro en ceros.
    pub fn pico(&self) -> u16 {
        self.pico
    }

    /// Lo alto que suena la pista casi todo el rato: el pico de los trozos de 50 ms que
    /// suenan, dejando fuera el 10 % más alto. Es con lo que se decide si está casi muda,
    /// y no con `pico`: un aviso de Windows a volumen normal en mitad de un vídeo casi mudo
    /// lo dejaba sin subir. 0 si no sonó nada.
    pub fn nivel(&self) -> u16 {
        let mut picos = self.picos_de_trozo.clone();
        if self.trozo_muestras > 0 && self.trozo_pico >= SONANDO {
            picos.push(self.trozo_pico);
        }
        if picos.is_empty() {
            return 0;
        }
        let i = ((picos.len() as f32 * 0.9).ceil() as usize).clamp(1, picos.len()) - 1;
        *picos.select_nth_unstable(i).1
    }

    /// Cierra el archivo con su tamaño de verdad en la cabecera. Devuelve cuántos bytes de
    /// sonido lleva: sin uno solo, esa pista no existe.
    pub fn cerrar(mut self) -> u64 {
        let cerrado = self.archivo.flush().is_ok()
            && self.archivo.get_mut().seek(SeekFrom::Start(0)).is_ok()
            && self
                .archivo
                .get_mut()
                .write_all(&cabecera_wav(self.info, self.escritos))
                .is_ok()
            && self.archivo.get_mut().flush().is_ok();
        if cerrado {
            self.escritos
        } else {
            0
        }
    }

    /// Cierra como `cerrar`, y si la pista llegó casi muda la sube (`nivelar_si_casi_muda`).
    /// `con_voz`: si la grabación trae una voz que se oye, para no taparla. Devuelve los
    /// bytes de sonido, el pico que queda en el archivo y por cuánto se multiplicó, si se subió.
    pub fn cerrar_y_nivelar(self, con_voz: bool) -> (u64, u16, Option<f32>) {
        let ruta = self.ruta.clone();
        let (pico, nivel) = (self.pico, self.nivel());
        let escritos = self.cerrar();
        if escritos == 0 {
            return (0, pico, None);
        }
        match nivelar_si_casi_muda(&ruta, nivel, con_voz) {
            Some((nuevo, ganancia)) => (escritos, nuevo, Some(ganancia)),
            None => (escritos, pico, None),
        }
    }
}

/// Por debajo de este pico (-40 dBFS) una pista no se oye: es el `CASI_MUDA` del editor.
pub const PICO_CASI_MUDO: u16 = 328;
/// Hasta donde se sube una pista casi muda: su nivel (`EscritorWav::nivel`) a -6 dBFS, como un
/// vídeo normal. Lo que pase de ahí se dobla suave hacia el tope (`limitar`).
const PICO_NIVELADO: f32 = 16_384.0;
/// Y si la grabación trae una voz que se oye, solo hasta -20 dBFS: de fondo, debajo de ella.
/// Alguien que pone la música baja a propósito para hablar encima no quiere que le gane.
const PICO_NIVELADO_CON_VOZ: f32 = 3_277.0;
/// Y como mucho cuánto se multiplica: x100 son +40 dB. Más que eso ya es subir el siseo.
const GANANCIA_MAXIMA: f32 = 100.0;

/// Sube una pista casi muda, en el mismo archivo, para que su `nivel` llegue a `PICO_NIVELADO`
/// (o a `PICO_NIVELADO_CON_VOZ`), y devuelve el pico que queda y por cuánto se multiplicó. Una
/// muda del todo (nivel 0) o una que ya se oye se quedan como están (`None`).
///
/// **Por qué:** winshotx graba lo que suena en el PC DESPUÉS de la rueda de cada app en el
/// mezclador de Windows, y con Brave al 10 % (14-nov) y al 20 % (16-nov) el vídeo de YouTube
/// llegó a -56 y a -44 dB: se grababa, pero no se oía ni en la vista previa ni en el archivo.
/// El aviso del editor estaba dentro del apartado Sonido, plegado, y Munir volvió a pensar
/// que la vista previa estaba rota: «¿por qué sigo sin escuchar el volumen del vídeo?». Lo
/// que sube es casi todo la señal: llega ya en 16 bits, y a x100 el ruido de redondear queda
/// por debajo de -55 dBFS, sin siseo de micrófono que agrandar (salvo que el PC esté sonando
/// una llamada o un micro con «Escuchar este dispositivo»). La voz no se toca: un micrófono
/// bajo trae su siseo, y subirlo lo agranda.
pub fn nivelar_si_casi_muda(ruta: &Path, nivel: u16, con_voz: bool) -> Option<(u16, f32)> {
    if nivel == 0 || nivel >= PICO_CASI_MUDO {
        return None;
    }
    let objetivo = if con_voz { PICO_NIVELADO_CON_VOZ } else { PICO_NIVELADO };
    let ganancia = (objetivo / f32::from(nivel)).min(GANANCIA_MAXIMA);
    match multiplicar(ruta, ganancia) {
        Ok(nuevo) => Some((nuevo, ganancia)),
        Err(error) => {
            eprintln!("[sonido] no se ha podido subir {}: {error}", ruta.display());
            None
        }
    }
}

/// Lo que pasa de -6 dBFS tras subir se dobla suave hacia el tope en vez de cortarse: un aviso
/// de Windows en mitad de un vídeo casi mudo sale x100 y, sin esto, se daría contra el tope.
fn limitar(x: f32) -> f32 {
    const UMBRAL: f32 = 16_384.0;
    const RESTO: f32 = 32_767.0 - UMBRAL;
    let a = x.abs();
    if a <= UMBRAL {
        return x;
    }
    (UMBRAL + RESTO * ((a - UMBRAL) / RESTO).tanh()).copysign(x)
}

/// Multiplica las muestras del WAV por `ganancia`, a trozos de un mega y en su sitio: una
/// grabación de una hora son 690 MB y no tienen por qué pasar enteros por la memoria.
fn multiplicar(ruta: &Path, ganancia: f32) -> std::io::Result<u16> {
    use std::io::Read;
    let mut archivo = std::fs::OpenOptions::new().read(true).write(true).open(ruta)?;
    let mut trozo = vec![0u8; 1 << 20];
    let mut posicion = CABECERA_WAV;
    let mut pico = 0u16;
    loop {
        archivo.seek(SeekFrom::Start(posicion))?;
        let mut leidos = 0;
        while leidos < trozo.len() {
            let n = archivo.read(&mut trozo[leidos..])?;
            if n == 0 {
                break;
            }
            leidos += n;
        }
        let pares = leidos & !1;
        if pares == 0 {
            break;
        }
        for muestra in trozo[..pares].chunks_exact_mut(2) {
            let subida = limitar(f32::from(i16::from_le_bytes([muestra[0], muestra[1]])) * ganancia)
                .round()
                .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
            pico = pico.max(subida.unsigned_abs());
            muestra.copy_from_slice(&subida.to_le_bytes());
        }
        archivo.seek(SeekFrom::Start(posicion))?;
        archivo.write_all(&trozo[..pares])?;
        posicion += pares as u64;
        if leidos < trozo.len() {
            break;
        }
    }
    archivo.flush()?;
    Ok(pico)
}

/// Lee lo que hay en un WAV escrito por `EscritorWav`: su formato y sus muestras. Solo para
/// las pruebas, que comprueban el archivo que se lleva el editor.
#[cfg(test)]
pub fn leer_wav(ruta: &Path) -> Option<(AudioInfo, Vec<u8>)> {
    use std::io::Read;
    let mut todo = Vec::new();
    File::open(ruta).ok()?.read_to_end(&mut todo).ok()?;
    if todo.len() < 44
        || &todo[0..4] != b"RIFF"
        || &todo[8..12] != b"WAVE"
        || &todo[36..40] != b"data"
    {
        return None;
    }
    let u32_en = |i: usize| u32::from_le_bytes([todo[i], todo[i + 1], todo[i + 2], todo[i + 3]]);
    let info = AudioInfo {
        channels: u16::from_le_bytes([todo[22], todo[23]]),
        sample_rate: u32_en(24),
    };
    let datos = u32_en(40) as usize;
    if u32_en(4) as usize != 36 + datos || todo.len() != 44 + datos {
        return None;
    }
    Some((info, todo[44..].to_vec()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn sesion(dir: &Path) -> SessionData {
        let mut s: SessionData = serde_json::from_str(
            r#"{"id":"x","dir":"","region":{"x":0,"y":0,"width":2,"height":2},"fps":30,
                "format":"video","hasAudio":true,"width":2,"height":2,"mp4Path":null,"frames":[]}"#,
        )
        .unwrap();
        s.dir = dir.to_path_buf();
        s.audio = Some(AudioInfo {
            channels: 2,
            sample_rate: 48_000,
        });
        s
    }

    /// Una sesión de antes, sin `pistas`, es `audio.pcm` entero y desde el byte cero; una
    /// nueva, sus WAV desde detrás de la cabecera, el PC primero. Y sin sonido, nada.
    #[test]
    fn cada_sesion_dice_sus_archivos() {
        let dir = std::env::temp_dir().join("winshotx-pistas-archivos");
        let mut vieja = sesion(&dir);
        assert_eq!(
            de_la_sesion(&vieja),
            vec![ArchivoDeSonido {
                ruta: dir.join("audio.pcm"),
                desde_byte: 0,
                que: QueSuena::Todo
            }]
        );
        assert_eq!(
            rutas(&vieja),
            (None, None),
            "lo de antes suena por el vídeo de vista previa"
        );

        vieja.pistas = Some(crate::contrato::PistasGrabadas {
            sistema: Some(ARCHIVO_PC.into()),
            microfono: Some(ARCHIVO_MICROFONO.into()),
            ..Default::default()
        });
        let nueva = vieja;
        let archivos = de_la_sesion(&nueva);
        assert_eq!(archivos.len(), 2);
        assert_eq!(
            (archivos[0].que, archivos[1].que),
            (QueSuena::Pc, QueSuena::Microfono)
        );
        assert!(archivos.iter().all(|a| a.desde_byte == CABECERA_WAV));
        let (pc, mic) = rutas(&nueva);
        assert!(pc.unwrap().ends_with(ARCHIVO_PC) && mic.unwrap().ends_with(ARCHIVO_MICROFONO));

        let mut solo_voz = nueva.clone();
        solo_voz.pistas = Some(crate::contrato::PistasGrabadas {
            sistema: None,
            microfono: Some(ARCHIVO_MICROFONO.into()),
            ..Default::default()
        });
        assert_eq!(de_la_sesion(&solo_voz).len(), 1);
        assert_eq!(
            rutas(&solo_voz).0,
            None,
            "sin sonido del PC no hay mando del PC"
        );

        let mut muda = nueva;
        muda.audio = None;
        assert!(de_la_sesion(&muda).is_empty());
    }

    /// El WAV que se escribe mientras se graba se cierra con su tamaño de verdad, y lo de
    /// dentro es exactamente lo que se le dio. Con el tamaño mal, el editor no lo suena.
    #[test]
    fn el_wav_se_cierra_con_su_tamano() {
        let dir = std::env::temp_dir().join("winshotx-pistas-wav");
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("p.wav");
        let info = AudioInfo {
            channels: 2,
            sample_rate: 44_100,
        };
        let mut wav = EscritorWav::crear(&ruta, info).unwrap();
        wav.escribir(&[1, 0, 2, 0]);
        wav.escribir(&[3, 0, 4, 0, 5, 0, 6, 0]);
        assert_eq!(wav.cerrar(), 12);
        let (leido, datos) = leer_wav(&ruta).expect("el WAV no se lee");
        assert_eq!((leido.channels, leido.sample_rate), (2, 44_100));
        assert_eq!(datos, vec![1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0]);
        // Lo que dice la cabecera del ritmo cuadra con lo que lee cualquier reproductor.
        let todo = std::fs::read(&ruta).unwrap();
        assert_eq!(
            u32::from_le_bytes([todo[28], todo[29], todo[30], todo[31]]),
            44_100 * 4
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// El pico es la muestra más alta en valor absoluto, también la negativa: el silencio
    /// exacto (el micro de Munito el 14-nov) da 0, y eso es lo que le hace decir al editor
    /// «no se grabó nada».
    #[test]
    fn el_pico_dice_lo_alto_que_sono() {
        let dir = std::env::temp_dir().join("winshotx-pistas-pico");
        let _ = std::fs::create_dir_all(&dir);
        let info = AudioInfo { channels: 1, sample_rate: 48_000 };
        let mut muda = EscritorWav::crear(&dir.join("muda.wav"), info).unwrap();
        muda.escribir(&[0, 0, 0, 0]);
        assert_eq!(muda.pico(), 0, "solo ceros es silencio exacto");
        muda.cerrar();
        let mut viva = EscritorWav::crear(&dir.join("viva.wav"), info).unwrap();
        viva.escribir(&[52, 0]);
        viva.escribir(&(-12_345i16).to_le_bytes());
        viva.escribir(&i16::MIN.to_le_bytes()[..1]); // medio byte suelto no cuenta
        assert_eq!(viva.pico(), 12_345);
        viva.escribir(&i16::MIN.to_le_bytes());
        assert_eq!(viva.pico(), 32_768, "el tope negativo no se desborda");
        viva.cerrar();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// El sonido del PC que llega casi mudo (YouTube con Brave al 20 % en el mezclador, el
    /// 16-nov: -44 dB de pico) se sube hasta oírse, con la forma intacta; el que ya se oye y
    /// el silencio exacto se quedan como estaban. Leído del archivo, que es lo que oye el
    /// editor y lo que se mezcla al guardar.
    #[test]
    fn el_sonido_del_pc_casi_mudo_se_sube_al_cerrar() {
        let dir = std::env::temp_dir().join("winshotx-pistas-nivelar");
        let _ = std::fs::create_dir_all(&dir);
        let info = AudioInfo { channels: 2, sample_rate: 48_000 };
        // Más de un trozo de un mega, para que el bucle dé la vuelta: 600.001 muestras son
        // 1.200.002 bytes, un mega entero y algo más.
        let onda: Vec<i16> = (0..600_001).map(|k| ((k as f32 * 0.05).sin() * 200.0).round() as i16).collect();
        assert!(onda.len() * 2 > 1 << 20);
        let bytes: Vec<u8> = onda.iter().flat_map(|m| m.to_le_bytes()).collect();

        let mut baja = EscritorWav::crear(&dir.join("baja.wav"), info).unwrap();
        baja.escribir(&bytes);
        assert_eq!(baja.pico(), 200);
        let (escritos, pico, ganancia) = baja.cerrar_y_nivelar(false);
        assert_eq!(escritos, bytes.len() as u64);
        assert_eq!(ganancia, Some(16_384.0 / 200.0));
        assert!((16_300..=16_384).contains(&pico), "sube hasta -6 dBFS: {pico}");
        let (leido_info, leidas) = leer_wav(&dir.join("baja.wav")).expect("la cabecera sigue bien");
        assert_eq!((leido_info.channels, leido_info.sample_rate), (2, 48_000));
        assert_eq!(leidas.len(), bytes.len());
        let ganancia = 16_384.0 / 200.0;
        for (k, par) in leidas.chunks_exact(2).enumerate().step_by(997) {
            let esperada = (f32::from(onda[k]) * ganancia).round();
            let leida = f32::from(i16::from_le_bytes([par[0], par[1]]));
            assert!((leida - esperada).abs() <= 1.0, "muestra {k}: {leida} en vez de {esperada}");
        }

        let mut alta = EscritorWav::crear(&dir.join("alta.wav"), info).unwrap();
        alta.escribir(&(-12_000i16).to_le_bytes());
        assert_eq!(alta.cerrar_y_nivelar(false), (2, 12_000, None), "la que se oye no se toca");
        assert_eq!(leer_wav(&dir.join("alta.wav")).unwrap().1, (-12_000i16).to_le_bytes());

        let mut muda = EscritorWav::crear(&dir.join("muda.wav"), info).unwrap();
        muda.escribir(&[0, 0, 0, 0]);
        assert_eq!(muda.cerrar_y_nivelar(false), (4, 0, None), "el silencio exacto no se sube");

        // Un clic suelto a -60 dB no se multiplica por más de x100.
        let mut suelta = EscritorWav::crear(&dir.join("suelta.wav"), info).unwrap();
        suelta.escribir(&33i16.to_le_bytes());
        assert_eq!(suelta.cerrar_y_nivelar(false).1, 3_300);

        // Con una voz que se oye, la música baja de fondo solo sube hasta -20 dBFS.
        let mut de_fondo = EscritorWav::crear(&dir.join("fondo.wav"), info).unwrap();
        de_fondo.escribir(&200i16.to_le_bytes());
        assert_eq!(de_fondo.cerrar_y_nivelar(true).1, 3_277);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Un aviso de Windows a volumen normal en mitad de un vídeo casi mudo no impide subirlo:
    /// lo que manda es lo que suena casi todo el rato, no la muestra más alta. Y lo fuerte se
    /// dobla suave hacia el tope en vez de darse la vuelta (revisión, 16-nov).
    #[test]
    fn un_aviso_suelto_no_impide_subir_el_sonido_casi_mudo() {
        let dir = std::env::temp_dir().join("winshotx-pistas-aviso");
        let _ = std::fs::create_dir_all(&dir);
        let info = AudioInfo { channels: 2, sample_rate: 48_000 };
        // Diez segundos de YouTube a -46 dB (160) con un «ding» de 0,3 s a -12 dB (8.000) a los 5 s.
        let total = 48_000 * 2 * 10;
        let aviso = (48_000 * 2 * 5)..(48_000 * 2 * 5 + 48_000 * 2 * 3 / 10);
        let onda: Vec<i16> = (0..total)
            .map(|k| {
                let a = if aviso.contains(&k) { 8_000.0 } else { 160.0 };
                ((k as f32 * 0.05).sin() * a).round() as i16
            })
            .collect();
        let bytes: Vec<u8> = onda.iter().flat_map(|m| m.to_le_bytes()).collect();
        let mut wav = EscritorWav::crear(&dir.join("aviso.wav"), info).unwrap();
        wav.escribir(&bytes);
        assert_eq!(wav.pico(), 8_000, "la muestra más alta es la del aviso");
        assert_eq!(wav.nivel(), 160, "y el nivel, el del vídeo");
        let (_, pico, ganancia) = wav.cerrar_y_nivelar(false);
        assert_eq!(ganancia, Some(100.0), "sube todo lo que puede: x100");
        let leidas: Vec<i16> = leer_wav(&dir.join("aviso.wav"))
            .unwrap()
            .1
            .chunks_exact(2)
            .map(|p| i16::from_le_bytes([p[0], p[1]]))
            .collect();
        let alto = |r: std::ops::Range<usize>| leidas[r].iter().map(|m| m.unsigned_abs()).max().unwrap();
        assert_eq!(alto(0..aviso.start), 16_000, "el vídeo, a -6 dBFS");
        assert!(alto(aviso.clone()) > 30_000 && pico == alto(aviso.clone()), "el aviso, cerca del tope");
        for k in aviso.clone() {
            assert!(
                onda[k] == 0 || (onda[k] > 0) == (leidas[k] > 0),
                "la muestra {k} se ha dado la vuelta: {} pasó a {}",
                onda[k],
                leidas[k]
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// El editor recibe el pico de 0 a 1, y solo de las pistas que tienen archivo.
    #[test]
    fn el_editor_recibe_el_pico_de_cada_pista() {
        let dir = std::env::temp_dir().join("winshotx-pistas-pico-editor");
        let mut s = sesion(&dir);
        s.pistas = Some(crate::contrato::PistasGrabadas {
            sistema: Some(ARCHIVO_PC.into()),
            microfono: Some(ARCHIVO_MICROFONO.into()),
            pico_sistema: Some(52),
            pico_microfono: Some(0),
            subida_sistema: Some(36.5),
        });
        let info = crate::recorder::SessionInfo::from(&s);
        assert!((info.pico_pc.unwrap() - 52.0 / 32_767.0).abs() < 1e-6);
        assert_eq!(info.pico_microfono, Some(0.0));
        assert_eq!(info.subida_pc, Some(36.5), "y cuánto se subió el PC, para decirlo");

        s.pistas = Some(crate::contrato::PistasGrabadas {
            sistema: Some(ARCHIVO_PC.into()),
            pico_sistema: Some(i16::MAX as u16 + 1),
            pico_microfono: Some(0),
            ..Default::default()
        });
        let info = crate::recorder::SessionInfo::from(&s);
        assert_eq!(info.pico_pc, Some(1.0), "el tope se queda en 1");
        assert_eq!(info.pico_microfono, None, "sin archivo de voz, no hay nada que avisar");
        assert_eq!(info.subida_pc, None, "sin subir, nada que decir");

        s.pistas = None;
        let info = crate::recorder::SessionInfo::from(&s);
        assert_eq!((info.pico_pc, info.pico_microfono), (None, None), "las sesiones de antes no avisan");
    }

    /// Sumar dos pistas de 16 bits con su volumen cada una: se escala, se suma, y lo que se
    /// pasa se queda en el tope en vez de dar la vuelta.
    #[test]
    fn sumar_dos_pistas_no_da_la_vuelta() {
        let pcm = |v: &[i16]| v.iter().flat_map(|m| m.to_le_bytes()).collect::<Vec<u8>>();
        let leer = |b: &[u8]| {
            b.chunks_exact(2)
                .map(|m| i16::from_le_bytes([m[0], m[1]]))
                .collect::<Vec<i16>>()
        };
        let mut base = pcm(&[1000, -1000, 30_000, 100]);
        sumar_pcm16(&mut base, &pcm(&[500, 500, 30_000, 100, 7]), 0.5, 2.0);
        assert_eq!(
            leer(&base),
            vec![1500, 500, i16::MAX, 250, 14],
            "la de encima, más larga, se queda entera"
        );
    }

    /// La mezcla entera de una sesión nueva son sus dos WAV sumados; la de una vieja, su
    /// `audio.pcm` tal cual.
    #[test]
    fn la_mezcla_entera_suma_las_dos() {
        let dir = std::env::temp_dir().join("winshotx-pistas-mezcla");
        std::fs::create_dir_all(&dir).unwrap();
        let info = AudioInfo {
            channels: 2,
            sample_rate: 48_000,
        };
        let mut s = sesion(&dir);
        std::fs::write(s.audio_path(), [9, 0, 9, 0]).unwrap();
        assert_eq!(mezcla_entera(&s), Some(vec![9, 0, 9, 0]));

        for (nombre, muestras) in [
            (ARCHIVO_PC, [10u8, 0, 20, 0]),
            (ARCHIVO_MICROFONO, [1, 0, 2, 0]),
        ] {
            let mut wav = EscritorWav::crear(&dir.join(nombre), info).unwrap();
            wav.escribir(&muestras);
            wav.cerrar();
        }
        s.pistas = Some(crate::contrato::PistasGrabadas {
            sistema: Some(ARCHIVO_PC.into()),
            microfono: Some(ARCHIVO_MICROFONO.into()),
            ..Default::default()
        });
        assert_eq!(
            mezcla_entera(&s),
            Some(vec![11, 0, 22, 0]),
            "sin la cabecera de ninguno de los dos"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
