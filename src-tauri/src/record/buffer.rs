//! El anillo de los ultimos segundos.
//!
//! Grabar lo que ACABA de pasar tiene un problema que la grabacion normal no tiene: no se
//! sabe cuando empieza. Hay que estar grabando siempre y **tirar lo viejo**, y el cache de
//! `record` solo sabe crecer, porque cada fotograma se guarda como el trozo que cambio
//! respecto al anterior y sin los de delante no se puede dibujar ninguno.
//!
//! La solucion sale de algo que ya estaba: se guarda un fotograma **entero** cada treinta.
//! Eso parte la grabacion en trozos que se pueden leer solos. Aqui cada trozo es un
//! archivo suyo (`FrameCache::en_archivo`), y tirar lo viejo es borrar archivos enteros.
//! Al guardar se cosen los que caen dentro de la ventana en un `frames.bin` normal, del
//! que el editor no tiene por que saber que vino de un anillo.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::{calcular_duraciones, FrameCache, FrameEntry};
use crate::error::{AppError, Result};

/// Cuanto dura cada trozo del anillo.
///
/// Es lo unico que se paga por poder tirar lo viejo: al guardar se conserva desde el
/// ultimo fotograma entero que ya habia empezado, asi que sale hasta un segundo de mas
/// (hay un entero por segundo), no un trozo entero de mas. Trozos mas cortos serian mas
/// archivos abiertos y cerrados sin ganar ni un milisegundo de precision.
pub const SEGMENTO_MS: u64 = 5_000;

/// A cuantos fotogramas por segundo graba el anillo de fabrica.
///
/// **Medido el 29 de agosto de 2026, sobre una partida a pantalla completa a 1920x1080**,
/// que es el peor caso porque cambia la pantalla entera en cada fotograma y el recorte de
/// `delta` no ahorra nada: cada fotograma son 2,3 MB en QOI. A 30 fps eso son 53 MB por
/// segundo, o sea un gigabyte y medio por cada treinta segundos de ventana.
///
/// Se probo guardar en JPEG y **sale peor**: ocupa seis veces menos (353 KB al 82 %) pero
/// cuesta 63 ms por fotograma contra 13, o sea cinco veces mas maquina, y esto corre todo
/// el rato. La medicion esta en `medir_formatos`.
///
/// Asi que se baja el ritmo: quince fotogramas por segundo se ven perfectamente para
/// entender que acaba de pasar, y es la mitad de disco y la mitad de maquina. Se puede
/// subir a treinta o sesenta desde los ajustes, con lo que cuesta escrito al lado, pero de
/// fabrica manda que el portatil de nadie sufra por una funcion que corre sola.
pub const FPS_ANILLO: u32 = 15;

/// Lo que ocupa un fotograma de pantalla completa en el PEOR caso.
///
/// Medido el 29 de agosto de 2026 sobre una partida a 1920x1080: 2,3 MB en QOI, porque la
/// pantalla cambia entera y el recorte de `delta` no ahorra nada. Un escritorio de trabajo
/// gasta una decima parte de esto.
const PEOR_FOTOGRAMA: u64 = 2_300_000;

/// Y el techo duro, se pida lo que se pida. Sesenta segundos a sesenta fotogramas serian
/// ocho gigabytes dando vueltas, y eso ya no es una funcion, es un problema.
const TECHO: u64 = 4 * 1_024 * 1_024 * 1_024;

/// Cuanto disco puede llegar a ocupar el anillo con esos segundos y ese ritmo.
///
/// Es a la vez el tope que se aplica y **el numero que se le ensenna a quien lo enciende**:
/// los segundos no dicen nada de lo que cuestan, porque una partida escribe diez veces mas
/// que un escritorio. Si la pantalla cambia poco no se llega ni de lejos, y si cambia mucho
/// se poda antes de tiempo y la interfaz cuenta los segundos que hay de verdad.
pub fn bytes_max(segundos: u32, fps: u32) -> u64 {
    (u64::from(segundos) * u64::from(fps) * PEOR_FOTOGRAMA).min(TECHO)
}

/// Un trozo del anillo: su archivo y lo que hay dentro.
///
/// Los `offset` de sus fotogramas son de SU archivo, no del cosido final.
#[derive(Debug, Clone)]
pub struct Segmento {
    pub ruta: PathBuf,
    pub frames: Vec<FrameEntry>,
}

impl Segmento {
    fn primer_ms(&self) -> u64 {
        self.frames.first().map(|f| f.timestamp_ms).unwrap_or(0)
    }

    /// Lo que ocupan en el archivo todos los fotogramas desde `desde` hasta el final.
    fn tramo(&self, desde: usize) -> Option<(u64, u64)> {
        let primero = self.frames.get(desde)?;
        let ultimo = self.frames.last()?;
        Some((primero.offset, ultimo.offset + u64::from(ultimo.len)))
    }
}

/// Por donde empieza lo que se guarda: el ultimo fotograma **entero** que ya habia
/// empezado cuando arranca la ventana.
///
/// Tiene que ser entero porque los demas son el trozo que cambio respecto al anterior, y
/// uno de esos no se puede dibujar solo. Y se coge el ultimo que empezo ANTES del corte,
/// no el primero de los de despues, porque **ese es el que se estaba viendo** en ese
/// instante. Una pantalla quieta no genera fotogramas: si se cogiera el primero de
/// despues, treinta segundos mirando algo parado se guardarian como medio segundo.
pub fn punto_de_corte(segmentos: &[Segmento], corte_ms: u64) -> Option<(usize, usize)> {
    let mut primero = None;
    let mut anterior = None;
    for (s, segmento) in segmentos.iter().enumerate() {
        for (i, frame) in segmento.frames.iter().enumerate() {
            if frame.patch.is_some() {
                continue;
            }
            if primero.is_none() {
                primero = Some((s, i));
            }
            if frame.timestamp_ms <= corte_ms {
                anterior = Some((s, i));
            } else {
                return anterior.or(primero);
            }
        }
    }
    anterior.or(primero)
}

/// Cose en `destino` todo lo que hay desde el corte hasta el final, y devuelve la lista de
/// fotogramas ya renumerada y con los tiempos empezando en cero, junto con el momento del
/// anillo por el que se empezo a cortar.
///
/// Ese momento hace falta fuera: los clics y el rastro del raton se anotaron con el reloj
/// del anillo, y hay que restarles lo mismo que a los fotogramas o el zoom se acercaria a
/// destiempo.
///
/// Los fotogramas de un trozo van seguidos dentro de su archivo, asi que cada trozo se
/// copia de una vez en vez de fotograma a fotograma: son megabytes leidos de corrido, que
/// es lo que mejor se le da a un disco.
pub fn ensamblar(
    segmentos: &[Segmento],
    corte_ms: u64,
    ahora_ms: u64,
    fps: u32,
    destino: &Path,
) -> Result<(Vec<FrameEntry>, u64)> {
    let Some((s0, i0)) = punto_de_corte(segmentos, corte_ms) else {
        return Err(AppError::Msg("todavía no hay nada que guardar".into()));
    };
    // El reloj empieza donde se pidio, no donde se dibujo el primer fotograma. Si ese
    // fotograma es anterior al corte es porque llevaba ahi un rato, y lo que se guarda son
    // los segundos que se pidieron, ni uno mas: sale un video que dura exactamente eso.
    // Si ese fotograma es ANTERIOR al corte, el video empieza igualmente en el corte: lo
    // que llevaba en pantalla desde antes no se pidio. Y si es posterior, es que no hay
    // tanta historia guardada, y entonces manda el fotograma.
    let entrada = segmentos[s0].frames[i0].timestamp_ms;
    let t0 = corte_ms.max(entrada);

    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre)?;
    }
    let mut salida = BufWriter::with_capacity(1 << 20, File::create(destino)?);
    let mut entries: Vec<FrameEntry> = Vec::new();
    let mut escrito = 0u64;
    let mut buffer: Vec<u8> = Vec::new();

    for (s, segmento) in segmentos.iter().enumerate().skip(s0) {
        let desde = if s == s0 { i0 } else { 0 };
        let Some((inicio, fin)) = segmento.tramo(desde) else {
            continue;
        };
        let mut file = File::open(&segmento.ruta)?;
        file.seek(SeekFrom::Start(inicio))?;
        buffer.resize((fin - inicio) as usize, 0);
        file.read_exact(&mut buffer)?;
        salida.write_all(&buffer)?;

        for frame in &segmento.frames[desde..] {
            entries.push(FrameEntry {
                index: entries.len() as u32,
                timestamp_ms: frame.timestamp_ms.saturating_sub(t0),
                offset: escrito + (frame.offset - inicio),
                thumb_path: String::new(),
                ..frame.clone()
            });
        }
        escrito += fin - inicio;
    }
    salida.flush()?;

    if entries.is_empty() {
        return Err(AppError::Msg("todavía no hay nada que guardar".into()));
    }
    // El ultimo fotograma dura hasta AHORA, no lo que dura uno suelto. Con la pantalla
    // quieta el ultimo puede llevar segundos en pantalla, y darle treinta milisegundos
    // seria guardar un parpadeo de algo que se estuvo viendo todo el rato.
    calcular_duraciones(&mut entries, ahora_ms.saturating_sub(t0), fps);
    Ok((entries, t0))
}

/// El anillo: los trozos vivos y el que se esta escribiendo.
pub struct Anillo {
    dir: PathBuf,
    ventana_ms: u64,
    fps: u32,
    segmentos: VecDeque<Segmento>,
    cache: Option<FrameCache>,
    ruta_actual: PathBuf,
    inicio_ms: u64,
    numero: u32,
    /// Archivos que ya no caben en la ventana pero que todavia no se pueden borrar porque
    /// hay un guardado leyendolos. Sin esto, pulsar dos veces seguidas se comeria el
    /// archivo a mitad de la copia.
    por_borrar: Vec<PathBuf>,
    copiando: Arc<AtomicUsize>,
    bytes_cerrados: u64,
    bytes_max: u64,
}

impl Anillo {
    pub fn nuevo(dir: &Path, ventana_ms: u64, fps: u32, bytes_max: u64) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let ruta_actual = dir.join("trozo-0.bin");
        Ok(Self {
            dir: dir.to_path_buf(),
            ventana_ms,
            fps,
            segmentos: VecDeque::new(),
            cache: Some(FrameCache::en_archivo(&ruta_actual)?),
            ruta_actual,
            inicio_ms: 0,
            numero: 0,
            por_borrar: Vec::new(),
            copiando: Arc::new(AtomicUsize::new(0)),
            bytes_cerrados: 0,
            bytes_max,
        })
    }

    /// Guarda el fotograma y, si toca, cambia de trozo y tira lo que ya no cabe.
    pub fn empujar(&mut self, rgba: &[u8], width: u32, height: u32, ts_ms: u64) -> Result<bool> {
        if ts_ms.saturating_sub(self.inicio_ms) >= SEGMENTO_MS {
            self.rotar(ts_ms)?;
            self.podar(ts_ms);
        }
        let cache = self
            .cache
            .as_mut()
            .ok_or_else(|| AppError::Msg("el anillo está cerrado".into()))?;
        cache.push_rgba(rgba, width, height, ts_ms)
    }

    /// Cierra el trozo que se estaba escribiendo y abre otro.
    ///
    /// El nuevo empieza por un fotograma entero sin que nadie se lo pida: un cache recien
    /// abierto no tiene con que comparar. Eso es justo lo que hace que el trozo se pueda
    /// leer cuando los de delante ya no existan.
    fn rotar(&mut self, ts_ms: u64) -> Result<()> {
        if let Some(cache) = self.cache.take() {
            self.bytes_cerrados += cache.bytes_written();
            let frames = cache.finish(ts_ms, self.fps)?;
            if frames.is_empty() {
                // Un trozo sin un solo fotograma es una pantalla que no se movio en cinco
                // segundos. No entra en la lista, asi que hay que borrarlo aqui.
                let _ = std::fs::remove_file(&self.ruta_actual);
            } else {
                self.segmentos.push_back(Segmento {
                    ruta: self.ruta_actual.clone(),
                    frames,
                });
            }
        }
        self.numero += 1;
        self.ruta_actual = self.dir.join(format!("trozo-{}.bin", self.numero));
        self.cache = Some(FrameCache::en_archivo(&self.ruta_actual)?);
        self.inicio_ms = ts_ms;
        Ok(())
    }

    /// Tira los trozos que se salen de la ventana.
    ///
    /// Se conserva el que CONTIENE el corte, no solo los de despues: dentro de el estan
    /// los primeros instantes de lo que se va a guardar.
    fn podar(&mut self, ahora_ms: u64) {
        let corte = ahora_ms.saturating_sub(self.ventana_ms);
        while self.segmentos.len() >= 2
            && (self.segmentos[1].primer_ms() <= corte || self.bytes() > self.bytes_max)
        {
            if let Some(viejo) = self.segmentos.pop_front() {
                self.bytes_cerrados = self
                    .bytes_cerrados
                    .saturating_sub(viejo.tramo(0).map(|(i, f)| f - i).unwrap_or(0));
                self.por_borrar.push(viejo.ruta);
            }
        }
        if self.copiando.load(Ordering::Acquire) == 0 {
            for ruta in self.por_borrar.drain(..) {
                let _ = std::fs::remove_file(ruta);
            }
        }
    }

    /// Lo que hay ahora mismo en la ventana, listo para coserlo en otro hilo.
    ///
    /// Cierra el trozo en curso para que lo ultimo que se grabo entre tambien: mientras se
    /// esta escribiendo, sus ultimos fotogramas viven en un bufer que nadie mas ve.
    ///
    /// Devuelve tambien el momento por el que hay que cortar y un guardian: mientras ese
    /// guardian viva, el anillo no borra ningun archivo aunque se salga de la ventana.
    pub fn instantanea(&mut self, ahora_ms: u64) -> Result<(Vec<Segmento>, u64, Copia)> {
        self.rotar(ahora_ms)?;
        if self.segmentos.is_empty() {
            return Err(AppError::Msg("todavía no hay nada que guardar".into()));
        }
        let copia = Copia::nueva(&self.copiando);
        let corte = ahora_ms.saturating_sub(self.ventana_ms);
        let segmentos = self.segmentos.iter().cloned().collect();
        self.podar(ahora_ms);
        Ok((segmentos, corte, copia))
    }

    /// Lo que ocupa en disco lo que hay guardado ahora mismo.
    pub fn bytes(&self) -> u64 {
        self.bytes_cerrados + self.cache.as_ref().map_or(0, |c| c.bytes_written())
    }

    /// Cuantos milisegundos hay guardados DE VERDAD.
    ///
    /// No tiene por que ser la ventana: al principio porque todavia se esta llenando, y
    /// con una pantalla que cambia mucho porque el tope de disco tira lo viejo antes de
    /// tiempo. La interfaz ensenna este numero, no el del ajuste: prometer treinta
    /// segundos y dar nueve seria mentir justo cuando alguien va a pulsar la tecla.
    pub fn guardado_ms(&self, ahora_ms: u64) -> u64 {
        let desde = self
            .segmentos
            .front()
            .map(Segmento::primer_ms)
            .unwrap_or(self.inicio_ms);
        ahora_ms.saturating_sub(desde).min(self.ventana_ms)
    }

    /// Cierra el anillo y borra todo lo que dejo en disco.
    pub fn limpiar(mut self) {
        self.cache = None;
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Un guardado en curso. Mientras exista, el anillo apunta lo que habria que borrar pero
/// no lo borra: los archivos que se estan copiando tienen que seguir ahi.
pub struct Copia(Arc<AtomicUsize>);

impl Copia {
    fn nueva(contador: &Arc<AtomicUsize>) -> Self {
        contador.fetch_add(1, Ordering::AcqRel);
        Self(Arc::clone(contador))
    }
}

impl Drop for Copia {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// El sonido de los ultimos segundos, en memoria.
///
/// Aqui no hacen falta trozos ni archivos: el sonido en crudo es una tira de bytes donde
/// cada milisegundo ocupa lo mismo, asi que quedarse con el final es cortar por delante.
/// Treinta segundos en estereo a 48 kHz son seis megabytes.
pub struct AnilloAudio {
    datos: VecDeque<u8>,
    max: usize,
    bytes_por_ms: usize,
    bloque: usize,
}

impl AnilloAudio {
    pub fn nuevo(bytes_por_ms: u64, canales: u16, ventana_ms: u64) -> Self {
        // Un poco mas que la ventana, porque al guardar se conserva desde el ultimo
        // fotograma entero y ese puede caer hasta un segundo antes del corte.
        let max = ((ventana_ms + 2_000) * bytes_por_ms.max(1)) as usize;
        Self {
            datos: VecDeque::new(),
            max,
            bytes_por_ms: bytes_por_ms.max(1) as usize,
            bloque: usize::from(canales.max(1)) * 2,
        }
    }

    pub fn empujar(&mut self, pcm: &[u8]) {
        self.datos.extend(pcm.iter().copied());
        let sobra = self.datos.len().saturating_sub(self.max);
        self.datos.drain(..sobra);
    }

    /// Los ultimos `ms` milisegundos, alineados a muestra entera: media muestra suena a
    /// chasquido. Si no hay tanto guardado, se devuelve lo que haya.
    pub fn ultimos(&self, ms: u64) -> Vec<u8> {
        let quiero = (ms as usize).saturating_mul(self.bytes_por_ms);
        let tengo = self.datos.len().min(quiero);
        let alineado = tengo / self.bloque * self.bloque;
        self.datos
            .iter()
            .skip(self.datos.len() - alineado)
            .copied()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::Rect;
    use crate::record::SessionData;

    /// Una pantalla de mentira: fondo fijo y un cuadrado que se mueve, que es lo que pasa
    /// de verdad al grabar (casi todo quieto y una cosa moviendose).
    fn pantalla(ancho: u32, alto: u32, paso: u32) -> Vec<u8> {
        let mut frame = vec![18u8; (ancho * alto) as usize * 4];
        for (i, byte) in frame.iter_mut().enumerate() {
            if i % 4 == 3 {
                *byte = 255;
            }
        }
        let x0 = (paso * 2) % (ancho - 6);
        for y in 3..9u32 {
            for x in x0..x0 + 6 {
                let p = ((y * ancho + x) * 4) as usize;
                frame[p] = 200;
                frame[p + 1] = 30;
                frame[p + 2] = 90;
            }
        }
        frame
    }

    fn temporal(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("winshotx-anillo-{nombre}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn frame(index: u32, ts: u64, entero: bool) -> FrameEntry {
        FrameEntry {
            index,
            timestamp_ms: ts,
            len: 10,
            offset: u64::from(index) * 10,
            patch: (!entero).then_some(crate::record::delta::Parche {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }),
            ..Default::default()
        }
    }

    /// Lo que no se puede negociar: se empieza por uno entero. Empezar por un parche es
    /// empezar por la diferencia con un fotograma que ya se borro.
    #[test]
    fn el_corte_cae_siempre_en_un_fotograma_entero() {
        let segmentos = vec![
            Segmento {
                ruta: PathBuf::new(),
                frames: vec![frame(0, 0, true), frame(1, 1000, false), frame(2, 2000, true)],
            },
            Segmento {
                ruta: PathBuf::new(),
                frames: vec![frame(0, 5000, true), frame(1, 6000, false)],
            },
        ];
        // El corte cae en mitad del primer trozo, entre dos enteros: se coge el de antes.
        assert_eq!(punto_de_corte(&segmentos, 2500), Some((0, 2)));
        // Justo encima de uno entero: ese mismo.
        assert_eq!(punto_de_corte(&segmentos, 5000), Some((1, 0)));
        // Y si se pide mas de lo que hay guardado, todo lo que hay.
        assert_eq!(punto_de_corte(&segmentos, 0), Some((0, 0)));
    }

    /// Un corte posterior a todo lo guardado no puede devolver un parche suelto.
    #[test]
    fn con_el_corte_pasado_el_final_se_coge_el_ultimo_entero() {
        let segmentos = vec![Segmento {
            ruta: PathBuf::new(),
            frames: vec![frame(0, 0, true), frame(1, 100, false), frame(2, 200, true)],
        }];
        assert_eq!(punto_de_corte(&segmentos, 99_000), Some((0, 2)));
    }

    /// **La prueba que importa**: se cose de verdad y el fotograma cosido se vuelve a
    /// dibujar. Que los trozos existan no dice nada; lo que hay que ver es que la imagen
    /// que sale del archivo cosido es la misma que entro, pixel a pixel.
    #[test]
    fn lo_cosido_se_vuelve_a_dibujar_igual() {
        let (ancho, alto) = (64u32, 32u32);
        let dir = temporal("cose");
        let mut anillo = Anillo::nuevo(&dir.join("anillo"), 10_000, 30, bytes_max(10, 30)).unwrap();

        // Veinte segundos a un fotograma cada 250 ms: cuatro trozos de cinco segundos.
        let mut ultimo = Vec::new();
        for paso in 0..80u32 {
            ultimo = pantalla(ancho, alto, paso);
            anillo.empujar(&ultimo, ancho, alto, u64::from(paso) * 250).unwrap();
        }

        let (segmentos, corte, _copia) = anillo.instantanea(20_000).unwrap();
        let destino = dir.join("sesion").join("frames.bin");
        let (frames, t0) = ensamblar(&segmentos, corte, 20_000, 30, &destino).unwrap();
        assert!(t0 >= 9_000, "el corte tiene que caer cerca de la ventana, y cayo en {t0}");

        // La ventana son diez segundos: cuarenta fotogramas, mas lo que sobre por empezar
        // en uno entero. Ni los ochenta enteros ni un punado suelto.
        assert!(
            (40..=60).contains(&frames.len()),
            "han salido {} fotogramas",
            frames.len()
        );
        assert_eq!(frames[0].timestamp_ms, 0, "el tiempo tiene que empezar en cero");
        assert!(frames[0].patch.is_none(), "el primero tiene que ser entero");

        let session = SessionData {
            id: "prueba".into(),
            dir: dir.join("sesion"),
            region: Rect { x: 0, y: 0, width: ancho, height: alto },
            fps: 30,
            format: "mp4".into(),
            has_audio: false,
            width: ancho,
            height: alto,
            mp4_path: None,
            audio: None,
            clics: Vec::new(),
            teclas: Vec::new(),
            cursor: Vec::new(),
            cursor_capturado: false,
            frames,
        };
        let dibujado = crate::record::read_frame(&session, session.frames.len() - 1).unwrap();
        assert_eq!(dibujado.as_raw(), &ultimo, "el ultimo fotograma no sobrevivio al cosido");

        anillo.limpiar();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Un anillo que corre toda la tarde no puede llenar el disco.
    #[test]
    fn los_trozos_viejos_se_tiran() {
        let (ancho, alto) = (32u32, 16u32);
        let dir = temporal("poda");
        let mut anillo = Anillo::nuevo(&dir, 10_000, 30, bytes_max(10, 30)).unwrap();
        for paso in 0..400u32 {
            let frame = pantalla(ancho, alto, paso);
            anillo.empujar(&frame, ancho, alto, u64::from(paso) * 250).unwrap();
        }
        let vivos = std::fs::read_dir(&dir).unwrap().count();
        // Diez segundos de ventana en trozos de cinco: dos o tres archivos, no cien.
        assert!(vivos <= 4, "han quedado {vivos} archivos vivos");
        anillo.limpiar();
    }

    /// Una pantalla que cambia entera (una partida) escribe treinta veces mas que un
    /// escritorio, asi que la ventana en segundos no dice nada de lo que ocupa. El tope de
    /// disco manda por encima del tiempo.
    #[test]
    fn el_tope_de_disco_manda_sobre_los_segundos() {
        let (ancho, alto) = (64u32, 64u32);
        let dir = temporal("tope");
        // Una ventana larguisima, pero medio megabyte de tope.
        let mut anillo = Anillo::nuevo(&dir, 600_000, 30, 512 * 1024).unwrap();
        for paso in 0..200u32 {
            anillo
                .empujar(&pantalla(ancho, alto, paso), ancho, alto, u64::from(paso) * 250)
                .unwrap();
        }
        assert!(
            anillo.bytes() <= 512 * 1024 * 2,
            "el anillo ocupa {} KB con medio mega de tope",
            anillo.bytes() / 1024
        );
        // Y sigue teniendo algo dentro: podar no puede dejarlo vacio.
        let (segmentos, _, _) = anillo.instantanea(50_000).unwrap();
        assert!(!segmentos.is_empty());
        anillo.limpiar();
    }

    /// Que el sonido tampoco crezca sin fin, y que lo que salga sean los ULTIMOS
    /// milisegundos y no los primeros.
    #[test]
    fn el_sonido_se_queda_con_el_final() {
        // Un canal a 1.000 muestras por segundo: dos bytes por milisegundo.
        let mut anillo = AnilloAudio::nuevo(2, 1, 1_000);
        for i in 0..5_000u16 {
            anillo.empujar(&i.to_le_bytes());
        }
        let ultimos = anillo.ultimos(500);
        assert_eq!(ultimos.len(), 1_000);
        let ultima = u16::from_le_bytes([ultimos[998], ultimos[999]]);
        assert_eq!(ultima, 4_999, "lo que sale tiene que acabar en lo ultimo que entro");
    }
}

#[cfg(test)]
mod medir {
    use std::time::Instant;

    /// Cuanto cuesta guardar un fotograma de pantalla en cada formato, sobre una imagen de
    /// verdad (una partida a pantalla completa, que es el peor caso: cambia entera).
    ///
    /// `cargo test --release --lib medir_formatos -- --ignored --nocapture`
    #[test]
    #[ignore = "es una medicion, no una comprobacion"]
    fn medir_formatos() {
        let ruta = std::env::temp_dir().join("winshotx-replay-pantalla/ultimo.png");
        let Ok(imagen) = image::open(&ruta) else {
            println!("no hay imagen en {}; corre antes la prueba con pantalla", ruta.display());
            return;
        };
        let rgba = imagen.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        println!("imagen de {w}x{h}");

        let ahora = Instant::now();
        let qoi = qoi::encode_to_vec(rgba.as_raw(), w, h).unwrap();
        println!("QOI      {:>7} KB en {:>4} ms", qoi.len() / 1024, ahora.elapsed().as_millis());

        for calidad in [70u8, 82, 90] {
            let rgb = image::DynamicImage::ImageRgba8(rgba.clone()).to_rgb8();
            let ahora = Instant::now();
            let mut salida = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut salida, calidad)
                .encode(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
                .unwrap();
            println!(
                "JPEG {calidad}  {:>7} KB en {:>4} ms",
                salida.len() / 1024,
                ahora.elapsed().as_millis()
            );
        }
    }
}
