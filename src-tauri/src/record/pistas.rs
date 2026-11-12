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
    info: AudioInfo,
    escritos: u64,
}

impl EscritorWav {
    pub fn crear(ruta: &Path, info: AudioInfo) -> std::io::Result<Self> {
        let mut archivo = BufWriter::new(File::create(ruta)?);
        archivo.write_all(&cabecera_wav(info, 0))?;
        Ok(Self {
            archivo,
            info,
            escritos: 0,
        })
    }

    /// Añade PCM de 16 bits. Lo que no se pueda escribir se pierde, pero no para la
    /// grabación: la imagen vale más que el sonido.
    pub fn escribir(&mut self, pcm: &[u8]) {
        if self.archivo.write_all(pcm).is_ok() {
            self.escritos += pcm.len() as u64;
        }
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
        });
        assert_eq!(
            mezcla_entera(&s),
            Some(vec![11, 0, 22, 0]),
            "sin la cabecera de ninguno de los dos"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
