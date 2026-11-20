use std::path::Path;

use image::RgbaImage;

use crate::error::{AppError, Result};

pub struct Mp4Options {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// 10..100; se traduce a bitrate en funcion de la resolucion y el ritmo.
    pub quality: u8,
}

impl Mp4Options {
    pub fn bitrate(&self) -> u32 {
        let pixels_per_second = self.width as f32 * self.height as f32 * self.fps.max(1) as f32;
        let factor = 0.04 + (self.quality.clamp(10, 100) as f32 / 100.0) * 0.16;
        (pixels_per_second * factor).clamp(400_000.0, 60_000_000.0) as u32
    }
}

/// Media Foundation espera BGRA y las filas de abajo arriba.
///
/// Fila a fila y con trozos de cuatro bytes, sin indices calculados a mano: asi el
/// compilador no tiene que comprobar los limites en cada canal de cada pixel, que a dos
/// millones de pixeles por fotograma se notaba.
///
/// **Secuencial a proposito, y probado.** Medido el 17 de octubre de 2026 en la maquina
/// de Munir: con `par_chunks_mut` esto baja de 2,96 a 1,75 ms por fotograma a 1280x720,
/// pero el camino entero (leer, encoger, voltear y codificar) no mejora ni un
/// milisegundo. El motivo es que no esta solo: el lector del otro hilo ya reparte su
/// trabajo entre todos los nucleos con el mismo rayon, asi que paralelizar aqui no anade
/// maquina, la quita de al lado. Si alguien vuelve a intentarlo, que mida el camino
/// entero y no esta funcion sola.
pub fn rgba_to_bgra_bottom_up(image: &RgbaImage) -> Vec<u8> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let fila = width * 4;
    let source = image.as_raw();
    let mut out = vec![0u8; fila * height];
    let voltear = |(y, destino): (usize, &mut [u8])| {
        let origen = &source[(height - 1 - y) * fila..(height - y) * fila];
        for (d, s) in destino.chunks_exact_mut(4).zip(origen.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
    };
    // Medio millon de pixeles es la raya: por debajo (una miniatura, un icono) el reparto
    // cuesta mas que el trabajo.
    out.chunks_exact_mut(fila).enumerate().for_each(voltear);
    out
}

/// El sonido que acompanna al video, ya recortado al tramo que se exporta.
pub struct Pista {
    pub channels: u16,
    pub sample_rate: u32,
    /// PCM de 16 bits con los canales intercalados, que es lo que quiere el codificador.
    pub datos: Vec<u8>,
}

impl Pista {
    fn bloque(&self) -> usize {
        usize::from(self.channels) * 2
    }

    /// Cuantos bytes de sonido corresponden a ese trozo de tiempo, sin cortar una muestra
    /// por la mitad.
    fn bytes_de(&self, ms: u32) -> usize {
        let bloque = self.bloque().max(1);
        let bytes = u64::from(ms) * u64::from(self.sample_rate) * bloque as u64 / 1000;
        (bytes as usize) / bloque * bloque
    }
}

/// El lado mas corto que Media Foundation acepta codificar.
///
/// **Medido el 17 de octubre de 2026 en la maquina de Munir**, probando tamannos uno a
/// uno contra el codificador de verdad: 96x64 sale, 96x48 no; 64x160 sale, 48x320 no.
/// Por debajo de este lado, `VideoEncoder::new` contesta «Puntero no valido»
/// (0x80004003) y la grabacion se queda sin video sin que el error diga nada util.
///
/// No es un capricho: grabar un trozo pequenno es un caso normal (una fila de una tabla,
/// un boton, un cartel de error), y ahi el editor decia «Sin vista previa» y el MP4
/// exportado no salia.
const LADO_MINIMO: u32 = 64;

/// El tamanno mas parecido al pedido que el codificador si acepta.
///
/// Se agranda lo justo y **manteniendo la proporcion**: un recorte de 96x48 sale a
/// 128x64, no estirado. Lo que ya llega al minimo no se toca, que es el caso de siempre.
pub fn medida_que_acepta(ancho: u32, alto: u32) -> (u32, u32) {
    let corto = ancho.min(alto).max(1);
    if corto >= LADO_MINIMO {
        return (ancho, alto);
    }
    let veces = f64::from(LADO_MINIMO) / f64::from(corto);
    let par = |v: f64| (((v.ceil() as u32).max(LADO_MINIMO) + 1) / 2) * 2;
    (par(f64::from(ancho) * veces), par(f64::from(alto) * veces))
}

#[cfg(windows)]
pub fn encode<L, P>(
    indices: &[usize],
    delays_ms: &[u32],
    loader: &mut L,
    path: &Path,
    options: &Mp4Options,
    audio: Option<Pista>,
    mut progress: P,
) -> Result<()>
where
    L: FnMut(usize) -> Result<RgbaImage>,
    P: FnMut(&str, usize, usize),
{
    use windows_capture::encoder::{
        AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
        VideoSettingsSubType,
    };

    if indices.is_empty() {
        return Err(AppError::Msg("no hay fotogramas que exportar".into()));
    }
    // Las dimensiones impares rompen a H.264, y las diminutas tambien (ver
    // `medida_que_acepta`): las dos cosas se arreglan aqui, antes de abrir nada.
    let (width, height) = medida_que_acepta((options.width / 2) * 2, (options.height / 2) * 2);

    let mut encoder = VideoEncoder::new(
        VideoSettingsBuilder::new(width, height)
            .sub_type(VideoSettingsSubType::H264)
            .frame_rate(options.fps.max(1))
            .bitrate(options.bitrate()),
        match audio.as_ref() {
            Some(pista) => AudioSettingsBuilder::default()
                .channel_count(u32::from(pista.channels))
                .sample_rate(pista.sample_rate)
                .bit_per_sample(16)
                .disabled(false),
            None => AudioSettingsBuilder::default().disabled(true),
        },
        ContainerSettingsBuilder::default(),
        path,
    )
    .map_err(|e| AppError::Msg(e.to_string()))?;

    let total = indices.len();
    // Media Foundation cuenta el tiempo en unidades de 100 nanosegundos.
    let mut timestamp: i64 = 0;
    let mut enviado: usize = 0;
    // El ultimo que se mando, para poder mandarlo otra vez al final. Ver abajo.
    let mut ultimo: Option<Vec<u8>> = None;
    for (position, index) in indices.iter().enumerate() {
        progress("encoding", position, total);
        let frame = loader(*index)?;
        // Normalmente ya viene del tamanno pedido y esto no hace nada. Cuando no, escala
        // por el camino de casa: con `image` esta linea sola costaba 64 ms por fotograma.
        let frame = if frame.width() == width && frame.height() == height {
            frame
        } else {
            crate::encode::escalar::a_medida(&frame, width, height)
        };
        let buffer = rgba_to_bgra_bottom_up(&frame);
        encoder
            .send_frame_buffer(&buffer, timestamp)
            .map_err(|e| AppError::Msg(e.to_string()))?;
        let dura = delays_ms.get(position).copied().unwrap_or(33);
        // Un fotograma largo (una pantalla quieta) se manda otra vez justo antes de que
        // acabe. El codificador rellena el hueco hasta el siguiente con el SIGUIENTE, no con
        // este: medido el 11 de noviembre de 2026, una pantalla roja de dos segundos y
        // luego una verde salian como 33 ms de rojo y dos segundos de verde. Con el mismo
        // fotograma al final de su rato, el relleno es él. Al comprimirlo no ocupa nada.
        let intervalo = 1000 / options.fps.max(1);
        if dura > intervalo * 2 {
            encoder
                .send_frame_buffer(&buffer, timestamp + i64::from(dura - intervalo) * 10_000)
                .map_err(|e| AppError::Msg(e.to_string()))?;
        }
        timestamp += dura as i64 * 10_000;
        ultimo = Some(buffer);

        // El sonido va detras de cada fotograma, en el trozo que le toca por duracion. De
        // una tacada al final tambien valdria (el codificador coloca el audio contando
        // muestras, no por la marca de tiempo), pero llenaria su cola de golpe.
        if let Some(pista) = audio.as_ref() {
            let hasta = (enviado + pista.bytes_de(dura)).min(pista.datos.len());
            if hasta > enviado {
                encoder
                    .send_audio_buffer(&pista.datos[enviado..hasta], 0)
                    .map_err(|e| AppError::Msg(e.to_string()))?;
                enviado = hasta;
            }
        }
    }

    // El ultimo fotograma se manda DOS veces, la segunda en el instante en que acaba.
    //
    // Media Foundation solo sabe cuando empieza cada fotograma; el ultimo lo termina donde
    // manda la velocidad declarada, o sea a los 33 milisegundos, sin importar lo que
    // durara de verdad. Con mil fotogramas eso se pierde en el redondeo, pero una pantalla
    // quieta se guarda en UN fotograma que dura tres segundos: el video salia de 66
    // milisegundos, o sea vacio. Se ve al rescatar los ultimos segundos de algo parado, y
    // tambien al exportar una grabacion en la que no se movio nada.
    //
    // Mandarlo otra vez en `timestamp` (que ya ha avanzado su duracion) le da a Media
    // Foundation el final de verdad. Es un fotograma identico al anterior, asi que al
    // comprimirlo no ocupa practicamente nada.
    if let Some(buffer) = ultimo {
        encoder
            .send_frame_buffer(&buffer, timestamp)
            .map_err(|e| AppError::Msg(e.to_string()))?;
    }

    // Y lo que quede sonando cuando ya no hay mas fotogramas.
    if let Some(pista) = audio.as_ref() {
        if enviado < pista.datos.len() {
            encoder
                .send_audio_buffer(&pista.datos[enviado..], 0)
                .map_err(|e| AppError::Msg(e.to_string()))?;
        }
    }

    encoder.finish().map_err(|e| AppError::Msg(e.to_string()))?;
    progress("done", total, total);
    Ok(())
}

/// Lo que puede faltar en un Linux para escribir un MP4. En Windows no falta nunca: el
/// codificador es del sistema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum Falta {
    /// El codificador de H.264 (`openh264enc`) o lo que lo mete en el MP4.
    Video,
    /// Un codificador de AAC.
    Sonido,
}

/// La orden que instala lo que falta, segun lo que diga `/etc/os-release`.
///
/// Solo estan las distribuciones donde se ha comprobado en un contenedor limpio que ese
/// paquete trae el codificador: de las demas no se dice ninguna orden, que una orden
/// inventada es peor que ninguna.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn orden_para(os_release: &str, falta: Falta) -> Option<&'static str> {
    let campo = |nombre: &str| {
        os_release
            .lines()
            .find_map(|linea| linea.strip_prefix(nombre)?.strip_prefix('='))
            .map(|valor| valor.trim().trim_matches('"').to_ascii_lowercase())
            .unwrap_or_default()
    };
    let familia = format!("{} {}", campo("ID"), campo("ID_LIKE"));
    let es = |nombre: &str| familia.split_whitespace().any(|f| f == nombre);
    if es("debian") || es("ubuntu") {
        // El mismo paquete trae los dos: `openh264enc` y `voaacenc`.
        Some("sudo apt install gstreamer1.0-plugins-bad")
    } else if es("fedora") {
        Some(match falta {
            Falta::Video => "sudo dnf install gstreamer1-plugin-openh264",
            Falta::Sonido => "sudo dnf install gstreamer1-plugins-bad-free",
        })
    } else {
        None
    }
}

/// Como se arregla lo que falta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum Arreglo {
    /// Con esta orden.
    Orden(&'static str),
    /// Con los plugins de GStreamer de la distribucion, sin saber decir cuales.
    Plugins,
    /// Instalando nada: es un AppImage cuyo GStreamer no carga los plugins del sistema
    /// (`gst::appimage_a_ciegas`). Lo que lo arregla es el paquete .deb o .rpm.
    OtroPaquete,
}

/// Lo que arregla lo que falta, segun la distribucion y segun si esto es un AppImage que no
/// ve los plugins del sistema: ahi la orden de instalar no cambiaria nada, y decirla seria
/// mandar a alguien a instalar algo para nada.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn arreglo_para(os_release: &str, falta: Falta, appimage_a_ciegas: bool) -> Arreglo {
    if appimage_a_ciegas {
        return Arreglo::OtroPaquete;
    }
    match orden_para(os_release, falta) {
        Some(orden) => Arreglo::Orden(orden),
        None => Arreglo::Plugins,
    }
}

/// Lo que arregla lo que falta en este equipo.
#[cfg(target_os = "linux")]
pub fn arreglo(falta: Falta) -> Arreglo {
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    arreglo_para(&os_release, falta, super::gst::appimage_a_ciegas())
}

/// Lo que se le dice a quien quiere un MP4 y no tiene con que escribirlo: que falta y como
/// se arregla. El GIF no lo necesita.
#[cfg(target_os = "linux")]
pub fn que_falta(falta: Falta) -> String {
    frase_de_lo_que_falta(falta, arreglo(falta))
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn frase_de_lo_que_falta(falta: Falta, arreglo: Arreglo) -> String {
    let que = match falta {
        Falta::Video => "Para guardar en MP4 falta el codificador de vídeo H.264 del sistema (openh264, de GStreamer).",
        Falta::Sonido => "Para guardar un MP4 con sonido falta un codificador de AAC del sistema (de GStreamer).",
    };
    match arreglo {
        Arreglo::Orden(orden) => format!("{que} Se instala con: {orden}"),
        Arreglo::Plugins => format!("{que} Instala los plugins «bad» de GStreamer de tu distribución."),
        Arreglo::OtroPaquete => {
            "Esta AppImage no puede usar los codificadores del sistema, y sin ellos no hay MP4. Instala winshotx con el paquete .deb o .rpm."
                .to_string()
        }
    }
}

/// En Linux escribe el GStreamer del sistema (`encode::gst`). Hace lo mismo que la de
/// Windows, con una diferencia que la simplifica: aqui cada fotograma se manda con lo que
/// dura, asi que una pantalla quieta no hay que mandarla dos veces para que llegue al final.
#[cfg(target_os = "linux")]
pub fn encode<L, P>(
    indices: &[usize],
    delays_ms: &[u32],
    loader: &mut L,
    path: &Path,
    options: &Mp4Options,
    audio: Option<Pista>,
    mut progress: P,
) -> Result<()>
where
    L: FnMut(usize) -> Result<RgbaImage>,
    P: FnMut(&str, usize, usize),
{
    use crate::encode::gst::{Escritor, Medidas};

    if indices.is_empty() {
        return Err(AppError::Msg("no hay fotogramas que exportar".into()));
    }
    let (width, height) = medida_que_acepta((options.width / 2) * 2, (options.height / 2) * 2);
    let medidas = Medidas { ancho: width, alto: height, fps: options.fps.max(1), bitrate: options.bitrate() };
    let mut escritor = Escritor::abrir(path, &medidas, audio.as_ref().map(|p| (p.channels, p.sample_rate)))?;

    let total = indices.len();
    let mut desde_ms: u64 = 0;
    let mut enviado: usize = 0;
    for (position, index) in indices.iter().enumerate() {
        progress("encoding", position, total);
        let frame = loader(*index)?;
        let frame = if frame.width() == width && frame.height() == height {
            frame
        } else {
            crate::encode::escalar::a_medida(&frame, width, height)
        };
        let dura = delays_ms.get(position).copied().unwrap_or(33);
        escritor.imagen(frame.into_raw(), desde_ms, u64::from(dura))?;
        desde_ms += u64::from(dura);

        // El sonido va detras de cada fotograma, en el trozo que le toca por duracion.
        if let Some(pista) = audio.as_ref() {
            let hasta = (enviado + pista.bytes_de(dura)).min(pista.datos.len());
            if hasta > enviado {
                escritor.sonido(&pista.datos[enviado..hasta])?;
                enviado = hasta;
            }
            // Si el sonido se acaba antes que la imagen, se dice ya: quien los junta espera
            // a los dos, y esperaria a un sonido que no va a llegar.
            if enviado >= pista.datos.len() {
                escritor.fin_del_sonido();
            }
        }
    }
    // Y lo que quede sonando cuando ya no hay mas fotogramas.
    if let Some(pista) = audio.as_ref() {
        if enviado < pista.datos.len() {
            escritor.sonido(&pista.datos[enviado..])?;
        }
    }
    escritor.cerrar()?;
    progress("done", total, total);
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn encode<L, P>(
    _indices: &[usize],
    _delays_ms: &[u32],
    _loader: &mut L,
    _path: &Path,
    _options: &Mp4Options,
    _audio: Option<Pista>,
    _progress: P,
) -> Result<()>
where
    L: FnMut(usize) -> Result<RgbaImage>,
    P: FnMut(&str, usize, usize),
{
    Err(AppError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn la_conversion_invierte_filas_y_canales() {
        let mut image = RgbaImage::from_pixel(1, 2, Rgba([0, 0, 0, 255]));
        image.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
        image.put_pixel(0, 1, Rgba([40, 50, 60, 255]));
        let out = rgba_to_bgra_bottom_up(&image);
        // La ultima fila del original tiene que quedar la primera, y en BGRA.
        assert_eq!(&out[0..4], &[60, 50, 40, 255]);
        assert_eq!(&out[4..8], &[30, 20, 10, 255]);
    }

    #[test]
    fn lo_diminuto_se_agranda_sin_deformarse() {
        // 96x48 es un recorte normal (una fila de una tabla) y el codificador lo rechaza.
        let (ancho, alto) = medida_que_acepta(96, 48);
        assert!(ancho >= 64 && alto >= 64, "ha salido {ancho}x{alto}");
        // La proporcion se mantiene: lo que entra 2 a 1 sale 2 a 1.
        let antes = 96.0 / 48.0;
        let ahora = f64::from(ancho) / f64::from(alto);
        assert!((antes - ahora).abs() < 0.05, "{antes} se ha vuelto {ahora}");
        // Y los dos lados pares, que H.264 no admite impares.
        assert_eq!((ancho % 2, alto % 2), (0, 0));
    }

    #[test]
    fn y_lo_que_ya_vale_no_se_toca() {
        assert_eq!(medida_que_acepta(1920, 1080), (1920, 1080));
        assert_eq!(medida_que_acepta(64, 64), (64, 64));
    }

    #[test]
    fn a_cada_distribucion_se_le_dice_el_paquete_que_trae_su_codificador() {
        let debian = "PRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"\nID=debian\n";
        let ubuntu = "NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n";
        let mint = "NAME=\"Linux Mint\"\nID=linuxmint\nID_LIKE=\"ubuntu debian\"\n";
        let fedora = "NAME=\"Fedora Linux\"\nVERSION_ID=44\nID=fedora\n";
        for de_apt in [debian, ubuntu, mint] {
            for falta in [Falta::Video, Falta::Sonido] {
                assert_eq!(orden_para(de_apt, falta), Some("sudo apt install gstreamer1.0-plugins-bad"));
            }
        }
        assert_eq!(orden_para(fedora, Falta::Video), Some("sudo dnf install gstreamer1-plugin-openh264"));
        assert_eq!(orden_para(fedora, Falta::Sonido), Some("sudo dnf install gstreamer1-plugins-bad-free"));
        // De donde no se ha comprobado no se inventa ninguna orden.
        assert_eq!(orden_para("ID=arch\n", Falta::Video), None);
        assert_eq!(orden_para("", Falta::Sonido), None);
        // Y solo cuentan `ID` e `ID_LIKE`, no cualquier linea que acabe igual.
        assert_eq!(orden_para("BUILD_ID=debian\nVERSION_ID=fedora\n", Falta::Video), None);
    }

    #[test]
    fn lo_que_falta_se_dice_en_llano_y_con_la_orden_si_se_sabe() {
        let con = frase_de_lo_que_falta(Falta::Video, Arreglo::Orden("sudo apt install gstreamer1.0-plugins-bad"));
        assert_eq!(
            con,
            "Para guardar en MP4 falta el codificador de vídeo H.264 del sistema (openh264, de GStreamer). Se instala con: sudo apt install gstreamer1.0-plugins-bad"
        );
        let sin = frase_de_lo_que_falta(Falta::Sonido, Arreglo::Plugins);
        assert!(sin.starts_with("Para guardar un MP4 con sonido falta un codificador de AAC"), "{sin}");
        assert!(sin.contains("GStreamer") && !sin.contains("sudo"), "{sin}");
    }

    /// El AppImage lleva su propio GStreamer, y si ese no carga los plugins del sistema,
    /// instalar el codificador no cambia nada: no se dice ninguna orden, se dice lo que si
    /// lo arregla. Donde si los carga (o fuera de un AppImage), la orden de siempre.
    #[test]
    fn en_un_appimage_que_no_ve_los_plugins_del_sistema_no_se_manda_instalar_nada() {
        let ubuntu = "NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n";
        let fedora = "ID=fedora\n";
        for os_release in [ubuntu, fedora, "ID=arch\n", ""] {
            for falta in [Falta::Video, Falta::Sonido] {
                assert_eq!(arreglo_para(os_release, falta, true), Arreglo::OtroPaquete, "{os_release:?}");
            }
        }
        assert_eq!(
            arreglo_para(ubuntu, Falta::Video, false),
            Arreglo::Orden("sudo apt install gstreamer1.0-plugins-bad")
        );
        assert_eq!(arreglo_para("ID=arch\n", Falta::Video, false), Arreglo::Plugins);
        let frase = frase_de_lo_que_falta(Falta::Video, Arreglo::OtroPaquete);
        assert!(frase.contains("AppImage") && frase.contains(".deb o .rpm"), "{frase}");
        assert!(!frase.contains("sudo") && !frase.contains("Se instala con"), "{frase}");
    }

    /// El camino entero de exportar un MP4 en Linux: `encode` lo escribe y se vuelve a leer,
    /// con su sonido.
    #[cfg(target_os = "linux")]
    #[test]
    fn en_linux_el_mp4_sale_con_cada_fotograma_en_su_tiempo_y_con_su_sonido() {
        let dir = std::env::temp_dir().join(format!("winshotx-mp4-linux-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("salida.mp4");
        let colores = [[230u8, 20, 20], [20, 200, 20], [20, 20, 230]];
        let duraciones = [1500u32, 500, 1000];
        // El segundo llega al doble de grande: se encoge al tamanno pedido.
        let mut cargar = |i: usize| -> Result<RgbaImage> {
            let lado = if i == 1 { 2 } else { 1 };
            let [r, g, b] = colores[i];
            Ok(RgbaImage::from_pixel(320 * lado, 240 * lado, Rgba([r, g, b, 255])))
        };
        // Tres segundos de tono, lo mismo que duran los tres fotogramas.
        let datos: Vec<u8> = (0..48_000u32 * 3)
            .flat_map(|n| {
                let v = ((n as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 12_000.0) as i16;
                [v.to_le_bytes(), v.to_le_bytes()].concat()
            })
            .collect();
        let mut fases = Vec::new();
        encode(
            &[0, 1, 2],
            &duraciones,
            &mut cargar,
            &ruta,
            // 321 de ancho: los lados impares se quedan en pares.
            &Mp4Options { width: 321, height: 240, fps: 30, quality: 75 },
            Some(Pista { channels: 2, sample_rate: 48_000, datos }),
            |fase, hechos, total| fases.push((fase.to_string(), hechos, total)),
        )
        .expect("el MP4 no ha salido");
        assert_eq!(fases.last(), Some(&("done".to_string(), 3, 3)));

        use crate::encode::fondo_video::Lector;
        let mut lector = crate::encode::gst::LectorVideo::abrir(&ruta).expect("leer el MP4");
        let mut desde = 0u64;
        for (color, dura) in colores.iter().zip(duraciones) {
            let cuadro = lector.siguiente().expect("fotograma").expect("faltan fotogramas");
            assert_eq!(cuadro.imagen.dimensions(), (320, 240));
            let visto = cuadro.imagen.get_pixel(160, 120).0;
            assert!(color.iter().zip(visto).all(|(a, b)| a.abs_diff(b) < 40), "se esperaba {color:?} y se lee {visto:?}");
            assert!(cuadro.desde_ms.abs_diff(desde) <= 2, "empieza en {} y no en {desde}", cuadro.desde_ms);
            assert!(cuadro.dura_ms.abs_diff(u64::from(dura)) <= 2, "dura {} y no {dura}", cuadro.dura_ms);
            desde += u64::from(dura);
        }
        assert!(lector.siguiente().expect("el final").is_none(), "sobran fotogramas");

        let (muestras, frecuencia) = crate::encode::gst::leer_sonido(&ruta, 10.0).expect("leer el sonido");
        let segundos = muestras.len() as f64 / f64::from(frecuencia);
        assert!((segundos - 3.0).abs() < 0.1, "el sonido dura {segundos} s");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_bitrate_sube_con_la_calidad() {
        let base = Mp4Options {
            width: 1280,
            height: 720,
            fps: 30,
            quality: 20,
        };
        let alta = Mp4Options {
            quality: 100,
            ..base
        };
        assert!(alta.bitrate() > base.bitrate());
    }
}
