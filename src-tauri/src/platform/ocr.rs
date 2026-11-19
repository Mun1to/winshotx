//! Sacar el texto de una captura, con el motor que Windows ya trae puesto.
//!
//! `Windows.Media.Ocr` viene de serie desde Windows 10, así que esto no engorda el
//! instalador ni un byte y no rompe la promesa de que winshotx funciona sin internet.
//! Un motor de OCR de los que se empaquetan pesa más que la aplicación entera.
//!
//! **El idioma no se elige aquí.** El motor se crea con los idiomas que la persona tiene
//! puestos en Windows, que es lo que va a leer: quien escribe en español lee capturas en
//! español. Si esos idiomas no traen OCR instalado, se prueba con inglés antes de rendirse,
//! porque el paquete inglés está en casi todas las instalaciones.

#[cfg(windows)]
use crate::error::{AppError, Result};

/// Lee el texto de una imagen en PNG y lo devuelve con sus saltos de línea.
///
/// Las líneas salen en el orden en que están en la imagen, de arriba abajo, que es lo que
/// hace falta para pegar el resultado en cualquier sitio y que se entienda. Las palabras
/// de una misma línea van separadas por un espacio: el motor las entrega sueltas, cada una
/// con su rectángulo, y unirlas con lo que había en medio es imposible de saber.
#[cfg(windows)]
pub fn leer_texto(png: &[u8]) -> Result<String> {
    let maximo = windows::Media::Ocr::OcrEngine::MaxImageDimension()?;
    Ok(texto_de(&lineas_de(&decodificador(png)?, &motor()?, maximo)?))
}

/// Las líneas leídas, de arriba abajo, con las palabras de cada una separadas por un espacio.
#[cfg(windows)]
fn texto_de(lineas: &[Vec<crate::sensibles::Palabra>]) -> String {
    lineas
        .iter()
        .map(|linea| linea.iter().map(|p| p.texto.as_str()).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// El único camino por el que una imagen llega al lector de Windows (issue #16): la tecla T,
/// la captura anclada, el fotograma del editor, tapar datos y el texto del historial.
///
/// - **Siempre en BGRA de 8 bits**, se abra lo que se abra (un PNG de 16 bits por canal, uno
///   gris, un JPEG). Medido: el decodificador ya lo entregaba así sin pedírselo, y ninguno de
///   esos formatos fallaba; pedirlo quita la duda y hace que todos los caminos lean lo mismo.
/// - **Nada más grande que `maximo`.** El motor no lee imágenes de más de
///   `OcrEngine::MaxImageDimension` píxeles de lado (10 000 en el Windows 11 de las pruebas,
///   y contesta «Image dimensions are too large!»): una captura con scroll larga se lee a
///   tiras que se solapan, y una más ancha que eso se reduce.
/// - **Cada línea, una vez.** Lo que cae en el solape de dos tiras lo ven las dos, y se queda
///   la que lo ve mejor (`Vista`): sale entero y no repetido.
/// - **Las cajas, en píxeles de la imagen de verdad**, no de la reducida, ni de su tira, ni
///   de la que el lector endereza por su cuenta cuando ve el texto torcido (`Giro`): con
///   ellas se tapa un correo, y una caja corrida tapa otra cosa.
///
/// `join` espera a que cada operación de WinRT termine sin montar ningún runtime asíncrono.
/// Ningún hilo hace falta iniciarlo a mano: `windows-core` mete en el apartamento MTA del
/// proceso al que pide una clase de WinRT sin haber iniciado COM (`CoIncrementMTAUsage`).
#[cfg(windows)]
fn lineas_de(
    decodificador: &windows::Graphics::Imaging::BitmapDecoder,
    motor: &windows::Media::Ocr::OcrEngine,
    maximo: u32,
) -> Result<Vec<Vec<crate::sensibles::Palabra>>> {
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapBounds, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform,
        ColorManagementMode, ExifOrientationMode,
    };

    let (ancho, alto) = (decodificador.PixelWidth()?, decodificador.PixelHeight()?);
    let maximo = maximo.max(SOLAPE_DE_TIRAS * 2);
    let escala = (maximo as f64 / ancho.max(1) as f64).min(1.0);
    let (ancho_leido, alto_leido) = if escala < 1.0 {
        (
            ((ancho as f64 * escala).round() as u32).clamp(1, maximo),
            ((alto as f64 * escala).round() as u32).max(1),
        )
    } else {
        (ancho, alto)
    };
    // De los píxeles leídos a los de la imagen de verdad.
    let (kx, ky) = (ancho as f32 / ancho_leido as f32, alto as f32 / alto_leido as f32);

    let lista = tiras(alto_leido, maximo, SOLAPE_DE_TIRAS);
    let mut vistas: Vec<Vista> = Vec::new();
    // Las líneas de la tira anterior, que todavía puede perder alguna contra esta, y dónde
    // acababa esa tira.
    let mut de_antes: Vec<Vista> = Vec::new();
    let mut fondo_de_antes = 0.0;
    for &(y, alto_tira) in &lista {
        let mapa = if lista.len() == 1 && escala >= 1.0 {
            decodificador
                .GetSoftwareBitmapConvertedAsync(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Premultiplied)?
                .join()?
        } else {
            let transformacion = BitmapTransform::new()?;
            if escala < 1.0 {
                transformacion.SetScaledWidth(ancho_leido)?;
                transformacion.SetScaledHeight(alto_leido)?;
                // De fábrica Windows reduce quedándose con un píxel de cada tantos, y eso
                // deja la letra pequeña a medio dibujar. Promediando se sigue leyendo.
                transformacion.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
            }
            // El recorte se aplica después de escalar, en los píxeles ya reducidos.
            transformacion.SetBounds(BitmapBounds { X: 0, Y: y, Width: ancho_leido, Height: alto_tira })?;
            decodificador
                .GetSoftwareBitmapTransformedAsync(
                    BitmapPixelFormat::Bgra8,
                    BitmapAlphaMode::Premultiplied,
                    &transformacion,
                    ExifOrientationMode::IgnoreExifOrientation,
                    ColorManagementMode::DoNotColorManage,
                )?
                .join()?
        };
        let resultado = motor.RecognizeAsync(&mapa)?.join()?;
        let grados = resultado.TextAngle().and_then(|angulo| angulo.Value()).unwrap_or(0.0);
        let giro = Giro::nuevo(grados, ancho_leido, alto_tira);

        let mut de_esta = Vec::new();
        for linea in resultado.Lines()? {
            let mut palabras = Vec::new();
            for palabra in linea.Words()? {
                let caja = palabra.BoundingRect()?;
                let (x, y_en_la_tira, ancho, alto) = giro.caja(caja.X, caja.Y, caja.Width, caja.Height);
                palabras.push(crate::sensibles::Palabra {
                    texto: palabra.Text()?.to_string_lossy(),
                    x,
                    y: y_en_la_tira + y as f32,
                    ancho,
                    alto,
                });
            }
            let Some(nueva) = Vista::de(palabras) else {
                continue;
            };
            // Si la tira de antes ya vio esta línea, se queda una: la que tiene su centro más
            // lejos del borde por el que su tira la corta, que es la que más ve de ella.
            let holgura = nueva.centro() - y as f32;
            let gana = de_antes
                .iter()
                .filter(|vieja| vieja.es_la_misma(&nueva))
                .all(|vieja| fondo_de_antes - vieja.centro() < holgura);
            if gana {
                de_antes.retain(|vieja| !vieja.es_la_misma(&nueva));
                de_esta.push(nueva);
            }
        }
        vistas.append(&mut de_antes);
        de_antes = de_esta;
        fondo_de_antes = (y + alto_tira) as f32;
    }
    vistas.append(&mut de_antes);

    Ok(vistas
        .into_iter()
        .map(|vista| {
            let mut palabras = vista.palabras;
            for p in &mut palabras {
                p.x *= kx;
                p.y *= ky;
                p.ancho *= kx;
                p.alto *= ky;
            }
            palabras
        })
        .collect())
}

/// El giro con el que el lector ha enderezado una imagen antes de leerla.
///
/// Cuando ve el texto torcido (`OcrResult::TextAngle`, en grados y en el sentido de las
/// agujas), el lector gira la imagen alrededor de su centro y da las cajas de las palabras
/// **en la imagen enderezada**. Para saber dónde está cada palabra en la que se le pasó hay
/// que deshacer ese giro; el ejemplo de OCR de Microsoft lo hace girando su capa de cajas
/// ese mismo ángulo alrededor del centro. Medido con texto girado a propósito: sin esto las
/// cajas caían a 30 px de su palabra a tres grados y a 68 a seis; con esto, a uno.
///
/// Pasa también con texto recto: en alguna tira el lector dice dos grados donde no los hay.
#[cfg(windows)]
struct Giro {
    seno: f32,
    coseno: f32,
    cx: f32,
    cy: f32,
}

#[cfg(windows)]
impl Giro {
    fn nuevo(grados: f64, ancho: u32, alto: u32) -> Self {
        let (seno, coseno) = (grados as f32).to_radians().sin_cos();
        Self { seno, coseno, cx: ancho as f32 / 2.0, cy: alto as f32 / 2.0 }
    }

    /// La caja de una palabra en la imagen que se le pasó al lector: el rectángulo recto que
    /// envuelve a la suya ya girada. Sin giro es la misma, sin tocar.
    fn caja(&self, x: f32, y: f32, ancho: f32, alto: f32) -> (f32, f32, f32, f32) {
        if self.seno == 0.0 {
            return (x, y, ancho, alto);
        }
        let esquinas = [(x, y), (x + ancho, y), (x, y + alto), (x + ancho, y + alto)].map(|(px, py)| {
            let (dx, dy) = (px - self.cx, py - self.cy);
            (self.cx + dx * self.coseno - dy * self.seno, self.cy + dx * self.seno + dy * self.coseno)
        });
        let x1 = esquinas.iter().map(|e| e.0).fold(f32::MAX, f32::min);
        let y1 = esquinas.iter().map(|e| e.1).fold(f32::MAX, f32::min);
        let x2 = esquinas.iter().map(|e| e.0).fold(f32::MIN, f32::max);
        let y2 = esquinas.iter().map(|e| e.1).fold(f32::MIN, f32::max);
        (x1, y1, x2 - x1, y2 - y1)
    }
}

/// Una línea tal como la ha visto una tira: sus palabras y el rectángulo que las envuelve,
/// en píxeles leídos y contados desde arriba de la imagen entera.
///
/// Dos tiras seguidas ven las dos lo que cae en su solape, y no siempre en el mismo sitio
/// exacto: el lector da cajas un poco distintas según lo que haya alrededor. Repartir por
/// «a qué lado de una raya cae el centro» deja que cada tira se la quede por su lado y salga
/// dos veces, o que ninguna se la quede. Se comparan las dos vistas una con otra.
#[cfg(windows)]
struct Vista {
    palabras: Vec<crate::sensibles::Palabra>,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

#[cfg(windows)]
impl Vista {
    /// `None` si la línea no trae palabras.
    fn de(palabras: Vec<crate::sensibles::Palabra>) -> Option<Self> {
        let x1 = palabras.iter().map(|p| p.x).reduce(f32::min)?;
        let y1 = palabras.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let x2 = palabras.iter().map(|p| p.x + p.ancho).fold(x1, f32::max);
        let y2 = palabras.iter().map(|p| p.y + p.alto).fold(y1, f32::max);
        Some(Self { palabras, x1, y1, x2, y2 })
    }

    fn centro(&self) -> f32 {
        (self.y1 + self.y2) / 2.0
    }

    /// Si es la misma línea de la imagen que `otra`: comparten al menos la mitad del alto de
    /// la más baja de las dos, y algo a lo ancho. Dos renglones seguidos apenas se rozan, y
    /// dos columnas a la misma altura no comparten nada a lo ancho.
    fn es_la_misma(&self, otra: &Vista) -> bool {
        let comun = self.y2.min(otra.y2) - self.y1.max(otra.y1);
        let la_mas_baja = (self.y2 - self.y1).min(otra.y2 - otra.y1);
        comun >= la_mas_baja / 2.0 && self.x2.min(otra.x2) > self.x1.max(otra.x1)
    }
}

/// Lo que se solapan dos tiras seguidas de una foto demasiado alta, en píxeles: una línea
/// de texto que cae justo en el corte sale entera en una de las dos.
#[cfg_attr(not(windows), allow(dead_code))]
pub const SOLAPE_DE_TIRAS: u32 = 120;

/// Cómo se parte en tiras una foto de `alto` píxeles para que ninguna pase de `maximo`:
/// dónde empieza cada una y cuánto mide. Una foto que cabe es una sola tira.
///
/// Hace falta por las capturas con scroll: el lector de Windows no acepta imágenes de más
/// de `OcrEngine::MaxImageDimension` píxeles de lado, y una página larga cosida pasa de eso.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn tiras(alto: u32, maximo: u32, solape: u32) -> Vec<(u32, u32)> {
    if alto <= maximo {
        return vec![(0, alto)];
    }
    let paso = maximo.saturating_sub(solape).max(1);
    let mut lista = Vec::new();
    let mut y = 0;
    loop {
        let trozo = (alto - y).min(maximo);
        lista.push((y, trozo));
        if y + trozo >= alto {
            return lista;
        }
        y += paso;
    }
}

/// Un lector para muchas fotos seguidas, con el motor creado una sola vez (S26: el texto
/// del historial, que lee la carpeta de capturas entera poco a poco).
#[cfg(windows)]
pub struct Lector {
    motor: windows::Media::Ocr::OcrEngine,
    /// El lado más grande que acepta el motor.
    maximo: u32,
}

#[cfg(windows)]
impl Lector {
    pub fn nuevo() -> Result<Self> {
        let maximo = windows::Media::Ocr::OcrEngine::MaxImageDimension()?;
        Ok(Self { motor: motor()?, maximo })
    }

    /// Lee una foto del disco (PNG, JPEG o lo que sepa abrir Windows) y devuelve su texto.
    ///
    /// La imagen la abre Windows directamente del archivo, sin pasar por Rust: no hay que
    /// descomprimirla aquí ni volver a comprimirla para el lector. Una foto más ancha de lo
    /// que acepta el motor se reduce, y una más alta (una captura con scroll de veinte
    /// pantallas) se lee a tiras que se solapan un poco.
    pub fn leer_archivo(&self, ruta: &std::path::Path) -> Result<String> {
        let bytes = std::fs::read(ruta)?;
        Ok(texto_de(&lineas_de(&decodificador(&bytes)?, &self.motor, self.maximo)?))
    }
}

/// Un decodificador de Windows sobre unos bytes de imagen que ya están en memoria.
#[cfg(windows)]
fn decodificador(bytes: &[u8]) -> Result<windows::Graphics::Imaging::BitmapDecoder> {
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    let flujo = InMemoryRandomAccessStream::new()?;
    let escritor = DataWriter::CreateDataWriter(&flujo.GetOutputStreamAt(0)?)?;
    escritor.WriteBytes(bytes)?;
    escritor.StoreAsync()?.join()?;
    escritor.FlushAsync()?.join()?;
    flujo.Seek(0)?;
    Ok(BitmapDecoder::CreateAsync(&flujo)?.join()?)
}

/// Lee la imagen y devuelve sus líneas, cada una con sus palabras y el rectángulo de cada
/// palabra en píxeles.
///
/// Es la misma lectura que `leer_texto`, pero quedándose con lo que aquella tira: dónde
/// estaba cada palabra. Sin eso se puede copiar el texto de una captura, pero no tapar un
/// correo, porque no se sabe qué trozo de la imagen hay que cubrir.
///
/// Las líneas salen como las ve el motor, de arriba abajo, y las palabras dentro de cada
/// línea en el orden en que están escritas. Eso importa: un dato partido en trozos (una
/// tarjeta en grupos de cuatro) solo se reconoce si sus trozos llegan seguidos.
#[cfg(windows)]
pub fn leer_palabras(png: &[u8]) -> Result<Vec<Vec<crate::sensibles::Palabra>>> {
    let maximo = windows::Media::Ocr::OcrEngine::MaxImageDimension()?;
    lineas_de(&decodificador(png)?, &motor()?, maximo)
}

/// El motor de OCR, con los idiomas de la persona y el inglés como red. También lo usa
/// `datos_en_video`, que lo crea una vez para leer muchos fotogramas.
#[cfg(windows)]
pub(crate) fn motor() -> Result<windows::Media::Ocr::OcrEngine> {
    use windows::Globalization::Language;
    use windows::Media::Ocr::OcrEngine;
    use windows::core::HSTRING;

    if let Ok(motor) = OcrEngine::TryCreateFromUserProfileLanguages() {
        return Ok(motor);
    }
    // Los idiomas del usuario pueden no tener OCR instalado. El inglés lo tiene casi
    // cualquier Windows, y leer con el motor equivocado sigue siendo mucho mejor que no
    // leer nada: el alfabeto es el mismo y solo se pierden algunas tildes.
    let ingles = Language::CreateLanguage(&HSTRING::from("en-US"))?;
    OcrEngine::TryCreateFromLanguage(&ingles).map_err(|_| {
        AppError::Msg(
            "Windows no tiene ningún idioma con lector de texto instalado. Se añade en \
             Configuración → Hora e idioma → Idioma, en «Opciones» de tu idioma."
                .into(),
        )
    })
}

#[cfg(not(windows))]
pub fn leer_texto(_png: &[u8]) -> crate::error::Result<String> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(not(windows))]
pub fn leer_palabras(_png: &[u8]) -> crate::error::Result<Vec<Vec<crate::sensibles::Palabra>>> {
    Err(crate::error::AppError::Unsupported)
}

#[cfg(test)]
mod tiras_de_una_foto {
    use super::tiras;

    #[test]
    fn una_foto_que_cabe_es_una_sola_tira() {
        assert_eq!(tiras(1080, 10_000, 120), [(0, 1080)]);
        assert_eq!(tiras(10_000, 10_000, 120), [(0, 10_000)]);
    }

    /// Ninguna pasa del máximo, se solapan lo dicho, y la última acaba justo en el borde.
    #[test]
    fn una_foto_alta_se_parte_sin_dejarse_nada() {
        let lista = tiras(25_000, 10_000, 120);
        assert_eq!(lista, [(0, 10_000), (9_880, 10_000), (19_760, 5_240)]);
        for par in lista.windows(2) {
            assert_eq!(par[0].0 + par[0].1 - par[1].0, 120, "solape entre {par:?}");
        }
        let ultima = lista.last().unwrap();
        assert_eq!(ultima.0 + ultima.1, 25_000);
    }
}

#[cfg(all(test, windows))]
pub(crate) mod pruebas {
    use super::*;

    /// Escribe una frase con la fuente del sistema y la lee de vuelta.
    ///
    /// El texto se dibuja con GDI y no a base de rectángulos negros: probé eso primero y
    /// el motor no leyó nada. Un OCR está entrenado con letras de verdad, con sus curvas y
    /// su grosor variable, y unas barras gordas no se le parecen a ninguna. Dibujarlo con
    /// la fuente que tiene Windows puesta es además lo más cercano a una captura real, que
    /// es lo que va a leer de verdad.
    ///
    /// Necesita el paquete de OCR instalado, así que va con `--ignored`: en una máquina
    /// sin él, la prueba diría que el código está mal cuando lo que falta es Windows.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn leer_una_frase_de_una_imagen() {
        let png = escribir_con_gdi("winshotx lee texto", 720, 160);
        let texto = leer_texto(&png).expect("el motor de OCR");
        println!("el lector ha visto: {texto:?}");
        let leido = texto.to_lowercase();
        assert!(
            leido.contains("winshotx"),
            "no ha leído la primera palabra: {texto:?}"
        );
        assert!(
            leido.contains("texto"),
            "no ha llegado al final de la línea: {texto:?}"
        );
    }

    /// Dos líneas se leen como dos líneas, no como una frase pegada.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn dos_lineas_salen_con_su_salto_en_medio() {
        let png = escribir_dos_lineas("primera linea", "segunda linea", 600, 260);
        let texto = leer_texto(&png).expect("el motor de OCR");
        println!("el lector ha visto: {texto:?}");
        assert!(
            texto.contains('\n'),
            "las dos líneas han salido pegadas: {texto:?}"
        );
        let (arriba, abajo) = texto.split_once('\n').unwrap();
        assert!(arriba.to_lowercase().contains("primera"), "arriba: {arriba:?}");
        assert!(abajo.to_lowercase().contains("segunda"), "abajo: {abajo:?}");
    }

    /// El camino entero de tapar datos: se pinta una pantalla con un correo y un DNI
    /// dentro, la lee el motor de Windows, y el detector tiene que sacar los dos.
    ///
    /// Las dos mitades estaban verdes por separado (el lector lee, y el detector detecta
    /// sobre palabras escritas a mano), y eso no es la función hecha: entre las dos hay
    /// una pantalla de verdad, un motor que parte las palabras donde quiere y una
    /// puntuación que llega pegada. Esto es lo único que dice si funciona.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn el_camino_entero_de_tapar_datos() {
        let png = pintar_lineas(
            &[
                "Factura 2026-104 por 1.234,56 euros",
                "Contacto: munir@example.com",
                "DNI 12345678Z telefono 600123456",
            ],
            900,
            320,
        );
        let lineas = leer_palabras(&png).expect("el motor de OCR");
        println!("lineas leidas: {}", lineas.len());
        let mut clases = Vec::new();
        for linea in &lineas {
            let dicho: Vec<&str> = linea.iter().map(|p| p.texto.as_str()).collect();
            println!("  {:?}", dicho.join(" "));
            for hallazgo in crate::sensibles::buscar_en_linea(linea) {
                println!("    -> {} en {:?}", hallazgo.clase.nombre(), hallazgo.palabras);
                clases.push(hallazgo.clase);
            }
        }
        assert!(
            clases.contains(&crate::sensibles::Clase::Correo),
            "no ha encontrado el correo: {clases:?}"
        );
        assert!(
            clases.len() >= 2,
            "de tres datos en pantalla ha encontrado {}: {clases:?}",
            clases.len()
        );
        // Y lo que no es un dato se queda: la factura y su importe no se tapan.
        assert!(
            clases.len() <= 4,
            "ha encontrado datos de mas, y eso tapa media pantalla: {clases:?}"
        );
    }

    /// Las cajas que devuelve el lector tienen que caer DENTRO de la imagen y tener
    /// tamanno: una caja de cero no tapa nada, y una fuera de la imagen no se pinta.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn las_cajas_de_las_palabras_caen_dentro_de_la_imagen() {
        let (ancho, alto) = (720.0f32, 200.0f32);
        let png = pintar_lineas(&["winshotx lee donde esta cada palabra"], 720, 200);
        let lineas = leer_palabras(&png).expect("el motor de OCR");
        let mut cuantas = 0;
        for linea in &lineas {
            for p in linea {
                assert!(p.ancho > 0.0 && p.alto > 0.0, "caja sin tamanno: {p:?}");
                assert!(p.x >= 0.0 && p.y >= 0.0, "caja en negativo: {p:?}");
                assert!(
                    p.x + p.ancho <= ancho + 1.0 && p.y + p.alto <= alto + 1.0,
                    "caja fuera de la imagen: {p:?}"
                );
                cuantas += 1;
            }
        }
        assert!(cuantas >= 4, "ha leido {cuantas} palabras de seis");
    }

    fn escribir_con_gdi(frase: &str, ancho: i32, alto: i32) -> Vec<u8> {
        pintar_lineas(&[frase], ancho, alto)
    }

    fn escribir_dos_lineas(una: &str, otra: &str, ancho: i32, alto: i32) -> Vec<u8> {
        pintar_lineas(&[una, otra], ancho, alto)
    }

    /// Texto negro sobre blanco con la fuente del sistema, devuelto como PNG.
    fn pintar_lineas(lineas: &[&str], ancho: i32, alto: i32) -> Vec<u8> {
        let colocadas: Vec<(i32, &str)> =
            lineas.iter().enumerate().map(|(i, l)| (24 + (i as i32) * 96, *l)).collect();
        pintar_textos(&colocadas, ancho, alto)
    }

    /// Lo mismo, con cada línea a la altura que se le diga: para pintar una captura muy alta
    /// con texto arriba, abajo y justo en el corte entre dos tiras (S26).
    pub(crate) fn pintar_textos(lineas: &[(i32, &str)], ancho: i32, alto: i32) -> Vec<u8> {
        pintar_con_letra(lineas, ancho, alto, 64)
    }

    /// Lo mismo, con la letra del alto que se diga: 64 es la de las demás pruebas, 24 la de
    /// una interfaz en una pantalla al 150 %, y 240 la de un titular.
    fn pintar_con_letra(lineas: &[(i32, &str)], ancho: i32, alto: i32, letra: i32) -> Vec<u8> {
        use windows::Win32::Foundation::COLORREF;
        use windows::Win32::Graphics::Gdi::*;
        use windows::core::HSTRING;

        unsafe {
            let dc = CreateCompatibleDC(None);
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = ancho;
            // Negativo: filas de arriba abajo, que es como las quiere `image`.
            info.bmiHeader.biHeight = -alto;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB.0;

            let mut pixeles: *mut std::ffi::c_void = std::ptr::null_mut();
            let mapa = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut pixeles, None, 0)
                .expect("el lienzo de la prueba");
            let anterior = SelectObject(dc, mapa.into());

            let blanco = CreateSolidBrush(COLORREF(0x00FF_FFFF));
            let todo = windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: ancho,
                bottom: alto,
            };
            FillRect(dc, &todo, blanco);

            // Una letra bien grande: a tamaño de interfaz el OCR también lee, pero una
            // prueba que falla por dos píxeles no dice nada útil.
            let fuente = CreateFontW(
                letra, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                &HSTRING::from("Segoe UI"),
            );
            let fuente_anterior = SelectObject(dc, fuente.into());
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, COLORREF(0x0000_0000));

            for (y, linea) in lineas {
                let utf16: Vec<u16> = linea.encode_utf16().collect();
                let _ = TextOutW(dc, 24, *y, &utf16);
            }

            // Los píxeles del DIB vienen en BGRA; `image` los quiere en RGBA.
            let total = (ancho * alto) as usize;
            let crudo = std::slice::from_raw_parts(pixeles as *const u8, total * 4);
            let mut lienzo = image::RgbaImage::new(ancho as u32, alto as u32);
            for (i, pixel) in lienzo.pixels_mut().enumerate() {
                let b = crudo[i * 4];
                let g = crudo[i * 4 + 1];
                let r = crudo[i * 4 + 2];
                *pixel = image::Rgba([r, g, b, 255]);
            }

            SelectObject(dc, fuente_anterior);
            SelectObject(dc, anterior);
            let _ = DeleteObject(fuente.into());
            let _ = DeleteObject(mapa.into());
            let _ = DeleteObject(blanco.into());
            let _ = DeleteDC(dc);

            crate::encode::png::to_bytes(&lienzo).expect("el PNG de prueba")
        }
    }

    /// Una imagen en blanco no es un error: es una captura sin texto, y hay que decirlo
    /// con una cadena vacía en vez de reventar.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn una_imagen_sin_texto_devuelve_cadena_vacia() {
        let blanca = image::RgbaImage::from_pixel(200, 120, image::Rgba([255, 255, 255, 255]));
        let png = crate::encode::png::to_bytes(&blanca).expect("el PNG de prueba");
        assert_eq!(leer_texto(&png).expect("el motor de OCR"), "");
    }

    // Issue #16: lo que no cabe en el lector de Windows. Hasta aquí solo el texto del
    // historial miraba `MaxImageDimension`; la tecla T, la captura anclada, el fotograma
    // del editor y tapar datos le pasaban la imagen tal cual.

    /// El lado más grande que acepta el lector en este Windows.
    fn maximo() -> u32 {
        windows::Media::Ocr::OcrEngine::MaxImageDimension().expect("el máximo del lector")
    }

    /// Un lienzo blanco de `ancho` x `alto` con cada frase pegada donde se dice, escrita
    /// con la fuente del sistema. Devuelve el PNG.
    fn lienzo_con_frases(ancho: u32, alto: u32, frases: &[(u32, u32, &str)]) -> Vec<u8> {
        let mut lienzo = image::RgbaImage::from_pixel(ancho, alto, image::Rgba([255, 255, 255, 255]));
        for (x, y, frase) in frases {
            let png = pintar_textos(&[(16, frase)], 1100, 110);
            let trozo = image::load_from_memory(&png).expect("el trozo de prueba").to_rgba8();
            image::imageops::overlay(&mut lienzo, &trozo, *x as i64, *y as i64);
        }
        crate::encode::png::to_bytes(&lienzo).expect("el PNG de prueba")
    }

    /// Una captura más ancha que lo que acepta el lector (tres pantallas 4K de lado a lado
    /// pasan de los 10 000 px de Windows 11) se lee igual, también lo que está a la derecha.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn una_captura_mas_ancha_que_el_lector_se_lee() {
        let ancho = maximo() * 3 / 2;
        let png = lienzo_con_frases(ancho, 240, &[(ancho - 1150, 60, "winshotx lee tres pantallas")]);
        let texto = leer_texto(&png).expect("el motor de OCR");
        println!("el lector ha visto: {texto:?}");
        assert!(texto.to_lowercase().contains("pantallas"), "no ha leído la frase: {texto:?}");
    }

    /// Una captura con scroll más alta que lo que acepta el lector se lee entera, en orden,
    /// y la línea que cae en el solape de dos tiras sale una sola vez.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn una_captura_con_scroll_se_lee_entera_y_en_orden() {
        let maximo = maximo();
        let alto = maximo * 2 + 400;
        let corte = maximo - SOLAPE_DE_TIRAS / 2 - 40;
        let png = lienzo_con_frases(
            1200,
            alto,
            &[(20, 40, "arriba del todo"), (20, corte, "justo en el corte"), (20, alto - 140, "abajo del todo")],
        );
        let texto = leer_texto(&png).expect("el motor de OCR").to_lowercase();
        println!("el lector ha visto: {texto:?}");
        let arriba = texto.find("arriba").expect("falta la línea de arriba");
        let corte_en = texto.find("corte").expect("falta la línea del corte");
        let abajo = texto.find("abajo").expect("falta la línea de abajo");
        assert!(arriba < corte_en && corte_en < abajo, "fuera de orden: {texto:?}");
        assert_eq!(texto.matches("corte").count(), 1, "la línea del corte sale dos veces: {texto:?}");
    }

    /// En una captura ancha, la caja de cada palabra cae donde está la palabra en la imagen
    /// de verdad, no en la reducida: tapar un correo tiene que tapar el correo.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn las_cajas_de_una_captura_ancha_caen_sobre_su_palabra() {
        let ancho = maximo() * 3 / 2;
        let (x, y) = (ancho - 1150, 60);
        let png = lienzo_con_frases(ancho, 240, &[(x, y, "munir@example.com")]);
        let lineas = leer_palabras(&png).expect("el motor de OCR");
        let palabras: Vec<_> = lineas.iter().flatten().collect();
        println!("palabras: {palabras:?}");
        assert!(!palabras.is_empty(), "no ha leído nada");
        for p in palabras {
            // El texto empieza en x + 24 y mide unos 64 px de alto a partir de y + 16.
            assert!(p.x >= x as f32 && p.x + p.ancho <= (x + 1100) as f32, "caja fuera en x: {p:?}");
            assert!(p.y >= y as f32 && p.y + p.alto <= (y + 110) as f32, "caja fuera en y: {p:?}");
        }
    }

    /// En una captura alta, la caja de lo que está en la segunda tira lleva la altura de la
    /// imagen entera, no la de su tira.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn las_cajas_de_una_captura_alta_llevan_su_altura_de_verdad() {
        let maximo = maximo();
        let alto = maximo * 2;
        let y = maximo + 500;
        let png = lienzo_con_frases(1200, alto, &[(20, y, "munir@example.com")]);
        let lineas = leer_palabras(&png).expect("el motor de OCR");
        let palabras: Vec<_> = lineas.iter().flatten().collect();
        println!("palabras: {palabras:?}");
        assert_eq!(lineas.len(), 1, "esperaba una sola línea: {lineas:?}");
        for p in palabras {
            assert!(p.y >= y as f32 && p.y + p.alto <= (y + 110) as f32, "caja fuera en y: {p:?}");
        }
    }

    /// Una imagen en cualquiera de los formatos que puede haber en la carpeta de capturas o
    /// anclarse se lee igual: PNG de 8 y de 16 bits por canal, en color, en gris y con
    /// transparencia, JPEG y BMP.
    ///
    /// Medido el 19 de noviembre de 2026 en Windows 11 (26200): con todos ellos el
    /// decodificador ya entregaba BGRA de 8 bits también sin pedírselo
    /// (`GetSoftwareBitmapAsync()` a secas), así que ninguno fallaba antes de la issue #16.
    /// Pedirlo siempre igual solo quita la duda.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn cualquier_formato_de_imagen_se_lee() {
        use image::DynamicImage as D;
        use image::ImageFormat::{Bmp, Jpeg, Png};
        let png8 = pintar_lineas(&["winshotx lee formatos"], 1100, 160);
        let base = image::load_from_memory(&png8).expect("el PNG de prueba");
        let variantes = [
            ("PNG de 8 bits sin alfa", D::ImageRgb8(base.to_rgb8()), Png),
            ("PNG gris de 8 bits", D::ImageLuma8(base.to_luma8()), Png),
            ("PNG gris con alfa", D::ImageLumaA8(base.to_luma_alpha8()), Png),
            ("PNG de 16 bits", D::ImageRgba16(base.to_rgba16()), Png),
            ("PNG de 16 bits sin alfa", D::ImageRgb16(base.to_rgb16()), Png),
            ("PNG gris de 16 bits", D::ImageLuma16(base.to_luma16()), Png),
            ("JPEG", D::ImageRgb8(base.to_rgb8()), Jpeg),
            ("JPEG gris", D::ImageLuma8(base.to_luma8()), Jpeg),
            ("BMP", D::ImageRgb8(base.to_rgb8()), Bmp),
        ];
        for (nombre, imagen, formato) in variantes {
            let mut bytes = std::io::Cursor::new(Vec::new());
            imagen.write_to(&mut bytes, formato).expect("la imagen de prueba");
            let texto = leer_texto(bytes.get_ref()).unwrap_or_else(|e| panic!("{nombre}: {e}"));
            assert_eq!(texto, "winshotx lee formatos", "{nombre}");
        }
    }

    /// En qué apartamento de COM está este hilo: `None` si en ninguno, o su tipo y su matiz.
    fn apartamento() -> Option<(i32, i32)> {
        use windows::Win32::System::Com::{APTTYPE, APTTYPEQUALIFIER, CoGetApartmentType};
        let (mut tipo, mut matiz) = (APTTYPE(0), APTTYPEQUALIFIER(0));
        unsafe { CoGetApartmentType(&mut tipo, &mut matiz) }.ok().map(|()| (tipo.0, matiz.0))
    }

    /// Un hilo nuevo, sin iniciar nada de COM ni de WinRT, lee igual (issue #16, punto 1).
    ///
    /// `windows-core` mete el hilo en el apartamento MTA del proceso la primera vez que pide
    /// una clase de WinRT y Windows contesta `CO_E_NOTINITIALIZED` (`CoIncrementMTAUsage`, en
    /// `imp/factory_cache.rs`), y desde ahí todo hilo que no haya iniciado COM está en ese
    /// MTA «implícito». Es lo que hace el hilo del historial y cada comando de Tauri, que
    /// corren en hilos de tokio. Sola en su proceso
    /// (`cargo test --lib un_hilo_recien_creado -- --ignored --nocapture`) enseña el antes:
    /// «sin apartamento».
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn un_hilo_recien_creado_lee_sin_iniciar_nada() {
        use windows::Win32::System::Com::{APTTYPE_MTA, APTTYPEQUALIFIER_IMPLICIT_MTA};
        let png = pintar_lineas(&["winshotx lee en otro hilo"], 1100, 160);
        let (antes, texto, despues) = std::thread::spawn(move || (apartamento(), leer_texto(&png), apartamento()))
            .join()
            .expect("el hilo");
        println!("apartamento del hilo: antes {antes:?}, después {despues:?}");
        assert!(texto.expect("el motor de OCR").to_lowercase().contains("hilo"));
        assert_eq!(
            despues,
            Some((APTTYPE_MTA.0, APTTYPEQUALIFIER_IMPLICIT_MTA.0)),
            "el hilo tiene que acabar en el MTA del proceso sin haberlo iniciado él"
        );
    }

    /// Y un hilo que sí tiene su apartamento, de un solo hilo (como el de la ventana) o de
    /// varios, lee igual: el motor es ágil y vale desde cualquiera.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn un_hilo_con_su_apartamento_tambien_lee() {
        use windows::Win32::System::Com::{
            COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize,
        };
        for modelo in [COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED] {
            let png = pintar_lineas(&["winshotx lee con apartamento"], 1100, 160);
            let texto = std::thread::spawn(move || unsafe {
                CoInitializeEx(None, modelo).ok().expect("iniciar COM");
                let texto = leer_texto(&png);
                CoUninitialize();
                texto
            })
            .join()
            .expect("el hilo")
            .expect("el motor de OCR");
            assert!(texto.to_lowercase().contains("apartamento"), "{modelo:?}: {texto:?}");
        }
    }

    /// Varios hilos leen a la vez, cada uno con su motor: la tecla T no falla porque el
    /// historial esté leyendo una foto en ese momento.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn varios_hilos_leen_a_la_vez() {
        let png = std::sync::Arc::new(pintar_lineas(&["winshotx lee a la vez"], 1100, 160));
        let hilos: Vec<_> = (0..8)
            .map(|_| {
                let png = png.clone();
                std::thread::spawn(move || {
                    let lector = Lector::nuevo().expect("el lector");
                    let ruta = std::env::temp_dir()
                        .join(format!("winshotx-a-la-vez-{:?}.png", std::thread::current().id()));
                    std::fs::write(&ruta, &*png).expect("la foto de prueba");
                    for _ in 0..5 {
                        assert_eq!(leer_texto(&png).expect("leer_texto"), "winshotx lee a la vez");
                        assert_eq!(lector.leer_archivo(&ruta).expect("leer_archivo"), "winshotx lee a la vez");
                    }
                    let _ = std::fs::remove_file(&ruta);
                })
            })
            .collect();
        for hilo in hilos {
            hilo.join().expect("un hilo ha fallado");
        }
    }

    // Lo que se ha intentado romper después (sesión ocr-16).

    /// Lee con el máximo que se diga en vez del de este Windows, para probar el reparto en
    /// tiras con imágenes pequeñas (y lo que pasaría con un lector que acepte menos).
    fn con_maximo(png: &[u8], maximo: u32) -> Vec<Vec<crate::sensibles::Palabra>> {
        lineas_de(&decodificador(png).expect("abrir"), &motor().expect("el motor"), maximo).expect("leer")
    }

    /// Justo en el máximo y un píxel por encima, a lo ancho y a lo alto: se lee igual.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn justo_en_el_maximo_y_un_pixel_por_encima() {
        let maximo = maximo();
        for (ancho, alto) in [(maximo, 200), (maximo + 1, 200), (1200, maximo), (1200, maximo + 1)] {
            let png = lienzo_con_frases(ancho, alto, &[(20, 40, "justo en el limite")]);
            let texto = leer_texto(&png).unwrap_or_else(|e| panic!("{ancho} x {alto}: {e}"));
            assert_eq!(texto, "justo en el limite", "{ancho} x {alto}");
        }
    }

    /// Lo más pequeño que hay, un píxel, y otras imágenes sin sitio para una letra: no hay
    /// texto, y se dice con una cadena vacía, no con un error.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn una_imagen_de_un_pixel_no_tiene_texto() {
        for (ancho, alto) in [(1, 1), (1, 200), (200, 1), (8, 8)] {
            let blanca = image::RgbaImage::from_pixel(ancho, alto, image::Rgba([255, 255, 255, 255]));
            let png = crate::encode::png::to_bytes(&blanca).expect("el PNG de prueba");
            assert_eq!(leer_texto(&png).unwrap_or_else(|e| panic!("{ancho} x {alto}: {e}")), "");
            assert!(leer_palabras(&png).expect("leer_palabras").is_empty());
        }
    }

    /// Ancha Y alta a la vez (se reduce y además se parte en tiras): cada frase sale una vez,
    /// en orden, y su caja cae sobre ella en la imagen de verdad.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn una_captura_ancha_y_alta_a_la_vez() {
        let (ancho, alto, maximo) = (4000u32, 9000u32, 2600u32);
        let nombres =
            ["uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve", "diez", "once", "doce"];
        let frases: Vec<(u32, u32, String)> = nombres
            .iter()
            .enumerate()
            .map(|(i, n)| ((i as u32 % 3) * 1400 + 30, i as u32 * 740 + 20, format!("frase numero {n} aqui")))
            .collect();
        let sitios: Vec<(u32, u32, &str)> = frases.iter().map(|(x, y, f)| (*x, *y, f.as_str())).collect();
        let lineas = con_maximo(&lienzo_con_frases(ancho, alto, &sitios), maximo);
        let leido: Vec<String> = lineas
            .iter()
            .map(|l| l.iter().map(|p| p.texto.as_str()).collect::<Vec<_>>().join(" "))
            .collect();
        let esperado: Vec<&str> = frases.iter().map(|(_, _, f)| f.as_str()).collect();
        assert_eq!(leido, esperado);
        for (linea, (x, y, frase)) in lineas.iter().zip(&frases) {
            for p in linea {
                // La frase se pinta en un trozo de 1100 x 110 puesto en (x, y).
                assert!(
                    p.x >= *x as f32 && p.x + p.ancho <= (x + 1100) as f32,
                    "{frase:?}, caja fuera en x: {p:?}"
                );
                assert!(
                    p.y >= *y as f32 && p.y + p.alto <= (y + 110) as f32,
                    "{frase:?}, caja fuera en y: {p:?}"
                );
            }
        }
    }

    /// Una captura más ancha que el lector se reduce, y la letra de una interfaz tiene que
    /// seguir leyéndose después. Con la interpolación de fábrica de Windows (el vecino más
    /// cercano, que tira píxeles enteros) de estas diez palabras se leían cinco y tres.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn la_letra_de_interfaz_de_una_captura_ancha_se_lee() {
        let frases = ["Pedido 48213 para Zaragoza", "Sale martes desde Bilbao", "Contacto munir@example.com"];
        // Una pantalla 4K con un lector que acepte 2600 px, y tres 4K con el de Windows 11.
        for (ancho, maximo) in [(3840, 2600), (11_520, 10_000)] {
            let png = pintar_con_letra(&[(40, frases[0]), (140, frases[1]), (240, frases[2])], ancho, 400, 24);
            let texto = texto_de(&con_maximo(&png, maximo));
            let leidas = frases
                .iter()
                .flat_map(|f| f.split(' '))
                .filter(|palabra| texto.split_whitespace().any(|leida| leida == *palabra))
                .count();
            assert!(leidas >= 9, "{ancho} px leídos a {maximo}: {leidas} palabras de 10 en {texto:?}");
        }
    }

    /// La misma línea, bajando píxel a píxel por el corte entre dos tiras: sale una vez, esté
    /// a la altura que esté.
    ///
    /// Las dos tiras la ven, y no siempre en el mismo sitio: a tres de estas alturas el lector
    /// dice que la primera tira está torcida dos grados (no lo está) y da sus cajas corridas
    /// una docena de píxeles. Repartiendo por dónde cae el centro, cada tira se la quedaba
    /// por su lado y salía dos veces.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn la_linea_del_corte_sale_una_vez_a_cualquier_altura() {
        // 700 px de ancho leídos a 600: se reduce y se parte en tres tiras.
        for y in 560..=620 {
            let png = pintar_textos(&[(20, "arriba queda"), (y, "justo corte"), (980, "abajo queda")], 700, 1100);
            let texto = texto_de(&con_maximo(&png, 600));
            assert_eq!(texto, "arriba queda\njusto corte\nabajo queda", "con la línea a la altura {y}");
        }
    }

    /// Gira una imagen los grados que se digan, en el sentido de las agujas y alrededor de su
    /// centro, sobre blanco: la foto de un papel que no se puso recto.
    fn girar(imagen: &image::RgbaImage, grados: f64) -> image::RgbaImage {
        let (ancho, alto) = imagen.dimensions();
        let (cx, cy) = (ancho as f64 / 2.0, alto as f64 / 2.0);
        let (seno, coseno) = grados.to_radians().sin_cos();
        let gris = |x: f64, y: f64| -> f64 {
            if x < 0.0 || y < 0.0 || x >= ancho as f64 || y >= alto as f64 {
                255.0
            } else {
                imagen.get_pixel(x as u32, y as u32).0[0] as f64
            }
        };
        image::RgbaImage::from_fn(ancho, alto, |x, y| {
            // De dónde sale este píxel: el giro al revés, mezclando los cuatro de alrededor.
            let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
            let (ox, oy) = (cx + dx * coseno + dy * seno - 0.5, cy - dx * seno + dy * coseno - 0.5);
            let (x0, y0) = (ox.floor(), oy.floor());
            let (fx, fy) = (ox - x0, oy - y0);
            let v = gris(x0, y0) * (1.0 - fx) * (1.0 - fy)
                + gris(x0 + 1.0, y0) * fx * (1.0 - fy)
                + gris(x0, y0 + 1.0) * (1.0 - fx) * fy
                + gris(x0 + 1.0, y0 + 1.0) * fx * fy;
            let v = v.round() as u8;
            image::Rgba([v, v, v, 255])
        })
    }

    /// Cuando el texto está torcido (la foto de un papel, anclada), el lector endereza la
    /// imagen antes de leerla y da las cajas en la enderezada, no en la que se le pasó: a
    /// seis grados caían a más de 60 px de su palabra, y la tapa de un correo tapaba otra
    /// cosa. La caja tiene que caer sobre la palabra en la imagen de verdad.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn las_cajas_de_un_texto_torcido_caen_sobre_su_palabra() {
        let (ancho, alto) = (1400u32, 800u32);
        let png = pintar_textos(
            &[
                (60, "Factura numero 2026 para Zaragoza"),
                (330, "Contacto munir@example.com"),
                (600, "Pedido 48213 sale martes Bilbao"),
            ],
            ancho as i32,
            alto as i32,
        );
        let recta = image::load_from_memory(&png).expect("el PNG de prueba").to_rgba8();
        let en_su_sitio: Vec<_> = leer_palabras(&png).expect("leer").into_iter().flatten().collect();
        assert_eq!(en_su_sitio.len(), 12, "la imagen recta: {en_su_sitio:?}");

        let (cx, cy) = (ancho as f32 / 2.0, alto as f32 / 2.0);
        for grados in [-6.0f32, -3.0, 3.0, 6.0] {
            let torcida = crate::encode::png::to_bytes(&girar(&recta, grados as f64)).expect("el PNG");
            let palabras: Vec<_> = leer_palabras(&torcida).expect("leer").into_iter().flatten().collect();
            assert_eq!(palabras.len(), en_su_sitio.len(), "a {grados} grados: {palabras:?}");
            let (seno, coseno) = grados.to_radians().sin_cos();
            for (p, recta) in palabras.iter().zip(&en_su_sitio) {
                // Dónde ha ido a parar el centro de esta palabra al girar la imagen.
                let (dx, dy) = (recta.x + recta.ancho / 2.0 - cx, recta.y + recta.alto / 2.0 - cy);
                let (x, y) = (cx + dx * coseno - dy * seno, cy + dx * seno + dy * coseno);
                assert!(
                    x >= p.x && x <= p.x + p.ancho && y >= p.y && y <= p.y + p.alto,
                    "a {grados} grados, {:?} está en ({x:.0}, {y:.0}) y su caja es {p:?}",
                    recta.texto
                );
                let (ex, ey) = (p.x + p.ancho / 2.0 - x, p.y + p.alto / 2.0 - y);
                assert!(
                    ex.hypot(ey) < 15.0,
                    "a {grados} grados, la caja de {:?} está a {:.0} px de su palabra",
                    recta.texto,
                    ex.hypot(ey)
                );
            }
        }
    }

    /// Un titular más alto que lo que se solapan dos tiras no cabe entero en ninguna de las
    /// dos cuando cae en el corte: aun así sale una vez, y bien leído.
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn un_titular_mas_alto_que_el_solape_sale_una_vez() {
        for letra in [180, 320] {
            for y in (600..1000).step_by(16) {
                let png = pintar_con_letra(&[(y, "Titular Grande")], 1800, 1800, letra);
                let texto = texto_de(&con_maximo(&png, 1000));
                assert_eq!(texto, "Titular Grande", "letra de {letra} px a la altura {y}");
            }
        }
    }
}
