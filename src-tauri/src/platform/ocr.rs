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
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    // El motor quiere un flujo del sistema, no un puñado de bytes: se le monta uno en
    // memoria y se le vuelca el PNG dentro.
    let flujo = InMemoryRandomAccessStream::new()?;
    let escritor = DataWriter::CreateDataWriter(&flujo.GetOutputStreamAt(0)?)?;
    escritor.WriteBytes(png)?;
    escritor.StoreAsync()?.join()?;
    escritor.FlushAsync()?.join()?;
    flujo.Seek(0)?;

    let decodificador = BitmapDecoder::CreateAsync(&flujo)?.join()?;
    let mapa = decodificador.GetSoftwareBitmapAsync()?.join()?;

    // `join` espera a que la operacion de WinRT termine sin montar ningun runtime
    // asincrono: leer una captura tarda decimas y esto ya corre en su propio comando.
    let motor = motor()?;
    let resultado = motor.RecognizeAsync(&mapa)?.join()?;

    let mut lineas = Vec::new();
    for linea in resultado.Lines()? {
        let palabras: Vec<String> = linea
            .Words()?
            .into_iter()
            .map(|p| p.Text().map(|t| t.to_string_lossy()))
            .collect::<std::result::Result<_, _>>()?;
        if !palabras.is_empty() {
            lineas.push(palabras.join(" "));
        }
    }
    Ok(lineas.join("\n"))
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
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    let flujo = InMemoryRandomAccessStream::new()?;
    let escritor = DataWriter::CreateDataWriter(&flujo.GetOutputStreamAt(0)?)?;
    escritor.WriteBytes(png)?;
    escritor.StoreAsync()?.join()?;
    escritor.FlushAsync()?.join()?;
    flujo.Seek(0)?;

    let decodificador = BitmapDecoder::CreateAsync(&flujo)?.join()?;
    let mapa = decodificador.GetSoftwareBitmapAsync()?.join()?;
    let resultado = motor()?.RecognizeAsync(&mapa)?.join()?;

    let mut lineas = Vec::new();
    for linea in resultado.Lines()? {
        let mut palabras = Vec::new();
        for palabra in linea.Words()? {
            let caja = palabra.BoundingRect()?;
            palabras.push(crate::sensibles::Palabra {
                texto: palabra.Text()?.to_string_lossy(),
                x: caja.X,
                y: caja.Y,
                ancho: caja.Width,
                alto: caja.Height,
            });
        }
        if !palabras.is_empty() {
            lineas.push(palabras);
        }
    }
    Ok(lineas)
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

#[cfg(all(test, windows))]
mod pruebas {
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
                64, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                &HSTRING::from("Segoe UI"),
            );
            let fuente_anterior = SelectObject(dc, fuente.into());
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, COLORREF(0x0000_0000));

            for (i, linea) in lineas.iter().enumerate() {
                let utf16: Vec<u16> = linea.encode_utf16().collect();
                let _ = TextOutW(dc, 24, 24 + (i as i32) * 96, &utf16);
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
}
