//! Leer el texto en Linux, con el Tesseract que tenga instalado el sistema (S45).
//!
//! En Windows el lector viene con el sistema (`Windows.Media.Ocr`). En Linux no hay ninguno
//! de serie, y uno metido dentro pesa más que winshotx entero: se usa la orden `tesseract`
//! (Apache-2.0), que está en los repositorios de todas las distribuciones y que los paquetes
//! .deb y .rpm recomiendan.
//!
//! **Se llama a la orden, no se enlaza con su biblioteca.** Ubuntu 22.04 trae Tesseract 4
//! (`libtesseract.so.4`) y las demás el 5: un paquete que dependiera de una no se instalaría
//! donde está la otra. La orden hace lo mismo en las dos: lee la imagen de su entrada y
//! devuelve cada palabra con su caja (`tsv`). Medido el 19-nov-2026 en Debian 12 y 13,
//! Ubuntu 22.04, 24.04 y 26.04 y Fedora 44 y 45 (`linux-lab/s45/ocr-distros.sh`).
//!
//! **Lo que se le prepara**, medido con capturas de verdad (`linux-lab/s45/ocr-ajustes.sh`):
//! - La imagen va en gris y sin comprimir (un PGM): prepararla no cuesta nada.
//! - Agrandada: la letra de una interfaz mide nueve o diez píxeles de alto, y Tesseract está
//!   hecho para la de un folio escaneado, que es el triple.
//! - Con el fondo claro: el Tesseract 4 no lee letra clara sobre fondo oscuro.
//!
//! Lo que es texto (elegir idiomas, entender el `tsv`, preparar la imagen) se compila en todos
//! los sistemas para que sus pruebas corran también en Windows; llamar a la orden, solo en Linux.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use image::RgbaImage;

use crate::sensibles::Palabra;

/// Cuánto se agranda la imagen antes de leerla, según los píxeles que tenga.
///
/// Medido con una foto de los Ajustes de 1.600 × 1.000 y 44 palabras a la vista: a su tamaño
/// lee 25, al doble 40 (y se deja la letra pequeña de los avisos) y al triple 40 o 42 con esa
/// letra dentro, en 0,6, 0,8 y 1,4 s. Hasta una pantalla 1080p entera, al triple; hasta una
/// 1440p, al doble; y lo que pasa de ahí se lee tal cual: tardaría segundos, y una captura
/// así suele venir de una pantalla donde la letra ya es grande.
pub fn cuanto_agrandar(pixeles: u64) -> u32 {
    match pixeles {
        0..=2_200_000 => 3,
        2_200_001..=5_000_000 => 2,
        _ => 1,
    }
}

/// El alto de cada tira en que se parte una imagen muy alta (una captura con scroll).
/// Tesseract no acepta lados de más de 32.767 píxeles, y una tira corta se lee antes.
const ALTO_DE_TIRA: u32 = 4_000;

/// Por debajo de este brillo medio (de 0 a 255) la imagen es letra clara sobre fondo
/// oscuro, y se le da la vuelta.
const BRILLO_OSCURO: u64 = 110;

/// Lo que puede faltar para leer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Falta {
    /// No está la orden `tesseract`.
    Programa,
    /// Está, pero sin ningún idioma instalado.
    Idiomas,
}

/// El idioma de Tesseract (tres letras) de un idioma del sistema (`es`, `es_ES.UTF-8`,
/// `zh_TW`). Solo los que se sabe cómo se llaman: de los demás no se inventa nada.
pub fn codigo_de(idioma: &str) -> Option<&'static str> {
    let limpio = idioma.split(['.', '@']).next().unwrap_or_default().to_ascii_lowercase();
    let mut partes = limpio.split(['_', '-']);
    let lengua = partes.next().unwrap_or_default();
    let region = partes.next().unwrap_or_default();
    Some(match lengua {
        "es" => "spa",
        "en" => "eng",
        "zh" if matches!(region, "tw" | "hk" | "mo" | "hant") => "chi_tra",
        "zh" => "chi_sim",
        "ca" => "cat",
        "gl" => "glg",
        "eu" => "eus",
        "pt" => "por",
        "fr" => "fra",
        "de" => "deu",
        "it" => "ita",
        "nl" => "nld",
        "pl" => "pol",
        "ru" => "rus",
        "uk" => "ukr",
        "ja" => "jpn",
        "ko" => "kor",
        "tr" => "tur",
        "sv" => "swe",
        "da" => "dan",
        "nb" | "nn" | "no" => "nor",
        "fi" => "fin",
        "cs" => "ces",
        "el" => "ell",
        "ro" => "ron",
        "hu" => "hun",
        "ar" => "ara",
        "he" => "heb",
        "hi" => "hin",
        _ => return None,
    })
}

/// Con qué idiomas se lee: los del sistema que estén instalados, y el inglés detrás si está,
/// que es lo que llevan los nombres de los programas, las órdenes y las direcciones. Si no
/// hay ninguno de esos, con lo primero que haya instalado: leer con el idioma equivocado
/// pierde algunas tildes, y no leer lo pierde todo.
///
/// `del_sistema` son los idiomas del usuario, del que más manda al que menos; `instalados`,
/// lo que dice `tesseract --list-langs`.
pub fn elegir_idiomas(del_sistema: &[String], instalados: &[String]) -> Option<String> {
    // `osd` no es un idioma: es lo que adivina hacia dónde está girada una página.
    let hay = |codigo: &str| codigo != "osd" && instalados.iter().any(|i| i == codigo);
    let mut elegidos: Vec<&str> = Vec::new();
    for codigo in del_sistema.iter().filter_map(|i| codigo_de(i)).chain(["eng"]) {
        if hay(codigo) && !elegidos.contains(&codigo) {
            elegidos.push(codigo);
        }
    }
    if elegidos.is_empty() {
        elegidos.extend(instalados.iter().map(String::as_str).filter(|i| hay(i)).take(2));
    }
    (!elegidos.is_empty()).then(|| elegidos.join("+"))
}

/// Los idiomas que salen de las variables del sistema, del que más manda al que menos:
/// `LANGUAGE` (una lista separada por dos puntos), `LC_ALL`, `LC_MESSAGES` y `LANG`.
pub fn idiomas_de(variable: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let mut lista: Vec<String> = Vec::new();
    for nombre in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        for valor in variable(nombre).unwrap_or_default().split(':') {
            let valor = valor.trim();
            if !valor.is_empty() && valor != "C" && valor != "POSIX" && !lista.iter().any(|v| v == valor) {
                lista.push(valor.to_string());
            }
        }
    }
    lista
}

/// Los idiomas de lo que contesta `tesseract --list-langs`: uno por línea, debajo de una
/// cabecera que cambia de una versión a otra («List of available languages (3):» en la 4,
/// con la carpeta en medio en la 5).
pub fn idiomas_instalados(salida: &str) -> Vec<String> {
    salida
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.contains(' ') && !l.ends_with(':'))
        .map(String::from)
        .collect()
}

/// Las palabras de un `tsv` de Tesseract, línea a línea, con cada caja en píxeles de la
/// imagen original (`escala` es cuánto se agrandó antes de leerla, y `desplazar_y` dónde
/// empezaba la tira que se leyó).
///
/// Cada fila del `tsv` lleva doce columnas: nivel, página, bloque, párrafo, línea, palabra,
/// izquierda, arriba, ancho, alto, confianza y texto. Las palabras son las de nivel 5; lo
/// demás son las cajas de los bloques y de las líneas. La confianza llega entera en la
/// versión 4 y con decimales en la 5, y no se usa.
///
/// Las líneas salen de arriba abajo, y las palabras de cada una, de izquierda a derecha.
pub fn palabras_del_tsv(tsv: &str, escala: f32, desplazar_y: f32) -> Vec<Vec<Palabra>> {
    let mut lineas: Vec<((u32, u32, u32, u32), Vec<Palabra>)> = Vec::new();
    for fila in tsv.lines() {
        let campos: Vec<&str> = fila.splitn(12, '\t').collect();
        if campos.len() < 12 || campos[0] != "5" {
            continue;
        }
        let texto = campos[11].trim();
        if texto.is_empty() {
            continue;
        }
        let numero = |i: usize| campos[i].trim().parse::<u32>().ok();
        let (Some(pagina), Some(bloque), Some(parrafo), Some(linea)) = (numero(1), numero(2), numero(3), numero(4)) else {
            continue;
        };
        let (Some(x), Some(y), Some(ancho), Some(alto)) = (numero(6), numero(7), numero(8), numero(9)) else {
            continue;
        };
        if ancho == 0 || alto == 0 {
            continue;
        }
        let palabra = Palabra {
            texto: texto.to_string(),
            x: x as f32 / escala,
            y: y as f32 / escala + desplazar_y,
            ancho: ancho as f32 / escala,
            alto: alto as f32 / escala,
        };
        let clave = (pagina, bloque, parrafo, linea);
        match lineas.last_mut() {
            Some((ultima, palabras)) if *ultima == clave => palabras.push(palabra),
            _ => lineas.push((clave, vec![palabra])),
        }
    }
    let mut lineas: Vec<Vec<Palabra>> = lineas.into_iter().map(|(_, palabras)| palabras).collect();
    for linea in lineas.iter_mut() {
        linea.sort_by(|a, b| a.x.total_cmp(&b.x));
    }
    en_renglones(lineas)
}

/// Junta en un renglón los trozos que están a la misma altura, y los ordena de arriba abajo.
///
/// Tesseract reparte la imagen en bloques y da las líneas de cada uno por separado: en una
/// ventana de ajustes, «Tecla de zoom» y su «Ctrl + Shift + 9» pueden salir como dos, aunque
/// estén en el mismo renglón. Juntos es como los lee una persona, y como los da el lector
/// de Windows.
fn en_renglones(mut trozos: Vec<Vec<Palabra>>) -> Vec<Vec<Palabra>> {
    let centro = |t: &[Palabra]| {
        let arriba = t.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let abajo = t.iter().map(|p| p.y + p.alto).fold(f32::NEG_INFINITY, f32::max);
        ((arriba + abajo) / 2.0, abajo - arriba)
    };
    trozos.sort_by(|a, b| centro(a).0.total_cmp(&centro(b).0));
    let mut renglones: Vec<Vec<Palabra>> = Vec::new();
    for trozo in trozos {
        let (medio, alto) = centro(&trozo);
        match renglones.last_mut() {
            // Mismo renglón si los centros distan menos de media letra de la más baja.
            Some(ultimo) if (centro(ultimo).0 - medio).abs() < centro(ultimo).1.min(alto) * 0.5 => {
                ultimo.extend(trozo);
                ultimo.sort_by(|a, b| a.x.total_cmp(&b.x));
            }
            _ => renglones.push(trozo),
        }
    }
    renglones
}

/// Si con esos idiomas hace falta, además, una pasada solo en inglés.
///
/// Medido en Fedora 44 (Tesseract 5.5.4) con letra de interfaz leída al triple: en cuanto
/// entra el español (`spa+eng`, `eng+spa` o `spa` solo) la arroba sale como una «O» y el
/// correo partido en dos («ana. lopezOejemplo.com»), y «Tapar datos» no lo ve. Solo con
/// `eng` el correo sale entero, pero sin tildes ni eñes («camion», «manana»). Así que se
/// lee con los idiomas de la persona y, aparte, en inglés, y de esa segunda pasada se
/// cogen solo los correos (`rescatar_correos`).
pub fn pide_pasada_en_ingles(idiomas: &str) -> bool {
    idiomas != "eng" && idiomas.split('+').any(|idioma| idioma == "eng")
}

/// Algo, una arroba y un dominio con su punto: lo que la pasada en inglés lee como correo.
fn parece_correo(texto: &str) -> bool {
    match texto.split_once('@') {
        Some((antes, despues)) => {
            !antes.is_empty() && !despues.contains('@') && despues.trim_end_matches('.').contains('.') && !despues.starts_with('.')
        }
        None => false,
    }
}

/// Mete en `lineas` los correos que la pasada en inglés ha leído bien y la otra no.
///
/// Cada correo de `en_ingles` busca su renglón por la altura y, dentro, las palabras que
/// pisa. Si ninguna lleva ya su arroba, las sustituye: eran el mismo correo leído a trozos.
/// Lo demás del renglón (con sus tildes) se queda como estaba.
pub fn rescatar_correos(lineas: &mut Vec<Vec<Palabra>>, en_ingles: &[Vec<Palabra>]) {
    for correo in en_ingles.iter().flatten().filter(|p| parece_correo(&p.texto)) {
        let medio = correo.y + correo.alto / 2.0;
        let renglon = lineas.iter_mut().find(|linea| {
            let arriba = linea.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
            let abajo = linea.iter().map(|p| p.y + p.alto).fold(f32::NEG_INFINITY, f32::max);
            medio >= arriba && medio <= abajo
        });
        let Some(linea) = renglon else {
            // Nadie había leído nada a esa altura: el correo es un renglón nuevo, en su sitio.
            let sitio = lineas.iter().position(|l| l.first().is_some_and(|p| p.y > correo.y)).unwrap_or(lineas.len());
            lineas.insert(sitio, vec![correo.clone()]);
            continue;
        };
        // Lo que pisa: más de media palabra dentro del ancho del correo.
        let pisa = |p: &Palabra| {
            let comun = (p.x + p.ancho).min(correo.x + correo.ancho) - p.x.max(correo.x);
            comun > p.ancho * 0.5
        };
        if linea.iter().any(|p| pisa(p) && p.texto.contains('@')) {
            continue;
        }
        linea.retain(|p| !pisa(p));
        linea.push(correo.clone());
        linea.sort_by(|a, b| a.x.total_cmp(&b.x));
    }
}

/// El texto de unas líneas ya leídas: una por renglón, con sus palabras separadas por un
/// espacio. Lo mismo que hace el lector de Windows en `ocr::texto_de`.
pub fn texto_de(lineas: &[Vec<Palabra>]) -> String {
    lineas
        .iter()
        .map(|linea| linea.iter().map(|p| p.texto.as_str()).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Un trozo de la imagen listo para Tesseract: en gris, con el fondo claro y agrandado.
pub struct Preparada {
    pub ancho: u32,
    pub alto: u32,
    pub gris: Vec<u8>,
    /// Cuánto se ha agrandado: las cajas que salgan se dividen entre esto.
    pub escala: f32,
}

impl Preparada {
    /// El archivo que se le da a la orden: un PGM, que es la cabecera y los bytes tal cual.
    pub fn pgm(&self) -> Vec<u8> {
        let mut archivo = format!("P5\n{} {}\n255\n", self.ancho, self.alto).into_bytes();
        archivo.extend_from_slice(&self.gris);
        archivo
    }
}

/// Prepara las filas `y1..y2` de la imagen, a todo el ancho.
pub fn preparar(imagen: &RgbaImage, y1: u32, y2: u32) -> Option<Preparada> {
    let ancho = imagen.width();
    let y2 = y2.min(imagen.height());
    if ancho == 0 || y2 <= y1 {
        return None;
    }
    let alto = y2 - y1;
    let fila = ancho as usize * 4;
    let crudo = &imagen.as_raw()[y1 as usize * fila..y2 as usize * fila];
    let mut gris: Vec<u8> = crudo
        .chunks_exact(4)
        .map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8)
        .collect();
    let brillo = gris.iter().map(|&v| v as u64).sum::<u64>() / gris.len() as u64;
    if brillo < BRILLO_OSCURO {
        gris.iter_mut().for_each(|v| *v = 255 - *v);
    }
    let veces = cuanto_agrandar(u64::from(ancho) * u64::from(alto));
    if veces == 1 {
        return Some(Preparada { ancho, alto, gris, escala: 1.0 });
    }
    let pequenna = image::GrayImage::from_raw(ancho, alto, gris)?;
    let grande = image::imageops::resize(&pequenna, ancho * veces, alto * veces, image::imageops::FilterType::Triangle);
    Some(Preparada { ancho: ancho * veces, alto: alto * veces, gris: grande.into_raw(), escala: veces as f32 })
}

/// El paquete que trae Tesseract y, si el sistema no está en inglés, el de su idioma, según
/// lo que diga `/etc/os-release`. Solo donde se ha comprobado en un contenedor limpio cómo
/// se llaman: de las demás distribuciones no se dice ninguna orden.
pub fn orden_para(os_release: &str, falta: &Falta, del_sistema: &[String]) -> Option<String> {
    let campo = |nombre: &str| {
        os_release
            .lines()
            .find_map(|linea| linea.strip_prefix(nombre)?.strip_prefix('='))
            .map(|valor| valor.trim().trim_matches('"').to_ascii_lowercase())
            .unwrap_or_default()
    };
    let familia = format!("{} {}", campo("ID"), campo("ID_LIKE"));
    let es = |nombre: &str| familia.split_whitespace().any(|f| f == nombre);
    let suyo = del_sistema.iter().filter_map(|i| codigo_de(i)).find(|c| *c != "eng");
    if es("debian") || es("ubuntu") {
        // El paquete del programa ya trae el inglés. Los de idioma llevan guiones.
        let mut orden = String::from("sudo apt install");
        if *falta == Falta::Programa {
            orden.push_str(" tesseract-ocr");
        }
        match suyo {
            Some(codigo) => orden.push_str(&format!(" tesseract-ocr-{}", codigo.replace('_', "-"))),
            None if *falta == Falta::Idiomas => orden.push_str(" tesseract-ocr-eng"),
            None => {}
        }
        Some(orden)
    } else if es("fedora") {
        let mut orden = String::from("sudo dnf install");
        if *falta == Falta::Programa {
            orden.push_str(" tesseract");
        }
        match suyo {
            Some(codigo) => orden.push_str(&format!(" tesseract-langpack-{codigo}")),
            None if *falta == Falta::Idiomas => orden.push_str(" tesseract-langpack-eng"),
            None => {}
        }
        Some(orden)
    } else {
        None
    }
}

/// Lo que se le dice a quien quiere leer un texto y no tiene con qué. La frase acaba en la
/// orden, detrás de «Se instala con:», que es por donde la interfaz la separa para traducir
/// lo demás (`src/lib/i18n.ts`).
pub fn frase_de_lo_que_falta(falta: &Falta, orden: Option<&str>) -> String {
    // Cada frase entera y de una pieza: son las claves con que la interfaz las traduce.
    match (falta, orden) {
        (Falta::Programa, Some(orden)) => format!("Para leer el texto falta Tesseract. Se instala con: {orden}"),
        (Falta::Programa, None) => {
            "Para leer el texto falta Tesseract. Instala el paquete tesseract de tu distribución.".to_string()
        }
        (Falta::Idiomas, Some(orden)) => format!("Tesseract no tiene ningún idioma instalado. Se instala con: {orden}"),
        (Falta::Idiomas, None) => {
            "Tesseract no tiene ningún idioma instalado. Instala el de tu idioma con los paquetes de tu distribución."
                .to_string()
        }
    }
}

#[cfg(target_os = "linux")]
pub use en_linux::Motor;

#[cfg(target_os = "linux")]
mod en_linux {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use image::RgbaImage;

    use super::*;
    use crate::error::{AppError, Result};

    /// Lo más que se espera a una lectura. Una pantalla entera tarda uno o dos segundos; si
    /// pasa de esto es que la orden se ha quedado colgada, y no se queda la app con ella.
    const PLAZO: Duration = Duration::from_secs(60);

    /// La orden lista para leer: con qué idiomas, y con cuántos hilos.
    pub struct Motor {
        idiomas: String,
        /// Para lo que se lee de fondo (el texto del historial): un solo hilo, que nadie
        /// está esperando y el PC es de quien lo está usando.
        un_hilo: bool,
    }

    impl Motor {
        /// Mira que la orden está y con qué idiomas. Si falta algo, el error ya dice qué y
        /// cómo se instala.
        pub fn nuevo() -> Result<Self> {
            let del_sistema = idiomas_de(|nombre| std::env::var(nombre).ok());
            let fallo = |falta: Falta| {
                let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
                let orden = orden_para(&os_release, &falta, &del_sistema);
                AppError::Msg(frase_de_lo_que_falta(&falta, orden.as_deref()))
            };
            let salida = match orden().arg("--list-langs").stdin(Stdio::null()).output() {
                Ok(salida) => salida,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(fallo(Falta::Programa)),
                Err(e) => return Err(e.into()),
            };
            // La lista sale por la salida normal; alguna versión vieja la sacaba por la de errores.
            let dicho = format!("{}\n{}", String::from_utf8_lossy(&salida.stdout), String::from_utf8_lossy(&salida.stderr));
            let instalados = idiomas_instalados(&dicho);
            let idiomas = elegir_idiomas(&del_sistema, &instalados).ok_or_else(|| fallo(Falta::Idiomas))?;
            Ok(Self { idiomas, un_hilo: false })
        }

        /// El mismo, leyendo con un solo hilo.
        pub fn de_fondo(mut self) -> Self {
            self.un_hilo = true;
            self
        }

        /// Para las pruebas: con esos idiomas, sin mirar los del sistema.
        #[cfg(test)]
        pub fn con_idiomas(idiomas: &str) -> Self {
            Self { idiomas: idiomas.to_string(), un_hilo: false }
        }

        /// Lee la imagen entera: sus líneas de arriba abajo, cada palabra con su caja en
        /// píxeles de la imagen. Una imagen muy alta se lee a tiras que se solapan, y cada
        /// línea sale una sola vez.
        pub fn leer(&self, imagen: &RgbaImage) -> Result<Vec<Vec<Palabra>>> {
            let solape = crate::platform::ocr::SOLAPE_DE_TIRAS;
            let tiras = crate::platform::ocr::tiras(imagen.height(), ALTO_DE_TIRA, solape);
            let mut lineas = Vec::new();
            for (i, (y, alto)) in tiras.iter().copied().enumerate() {
                // De lo que se solapa con la tira de antes, la mitad de arriba es suya.
                let desde = if i == 0 { 0.0 } else { y as f32 + solape as f32 / 2.0 };
                let hasta = if i + 1 == tiras.len() { f32::INFINITY } else { (y + alto) as f32 - solape as f32 / 2.0 };
                for linea in self.leer_franja(imagen, y, y + alto)? {
                    let centro = linea.iter().map(|p| p.y + p.alto / 2.0).sum::<f32>() / linea.len() as f32;
                    if centro >= desde && centro < hasta {
                        lineas.push(linea);
                    }
                }
            }
            Ok(lineas)
        }

        /// Lee solo las filas `y1..y2`, con las cajas en píxeles de la imagen entera.
        pub fn leer_franja(&self, imagen: &RgbaImage, y1: u32, y2: u32) -> Result<Vec<Vec<Palabra>>> {
            let Some(preparada) = preparar(imagen, y1, y2) else {
                return Ok(Vec::new());
            };
            let pgm = preparada.pgm();
            let pasada = |idiomas: &str| -> Result<Vec<Vec<Palabra>>> {
                let mut orden = orden();
                // `--psm 3`, que es como viene: reparte la imagen en bloques y lee cada uno. Buscando
                // texto suelto (`11`) lee las mismas palabras, pero trae además los iconos leídos
                // como letras («EJE», «MO», «<r»), y eso acaba en lo que se copia.
                orden.args(["stdin", "stdout", "-l", idiomas, "--psm", "3", "tsv"]);
                if self.un_hilo {
                    orden.env("OMP_THREAD_LIMIT", "1");
                }
                let tsv = ejecutar(orden, &pgm)?;
                Ok(palabras_del_tsv(&tsv, preparada.escala, y1 as f32))
            };
            if !pide_pasada_en_ingles(&self.idiomas) {
                return pasada(&self.idiomas);
            }
            // La pasada en inglés, para los correos. Las dos a la vez, que hay alguien
            // esperando; de fondo, una detrás de otra y con un solo núcleo.
            let (lineas, en_ingles) = if self.un_hilo {
                (pasada(&self.idiomas), pasada("eng").ok())
            } else {
                std::thread::scope(|hilos| {
                    let otra = hilos.spawn(|| pasada("eng").ok());
                    (pasada(&self.idiomas), otra.join().ok().flatten())
                })
            };
            let mut lineas = lineas?;
            // Si la pasada en inglés falla se queda lo leído: es un repaso, no la lectura.
            if let Some(en_ingles) = en_ingles {
                rescatar_correos(&mut lineas, &en_ingles);
            }
            Ok(lineas)
        }
    }

    /// La orden `tesseract`, sin lo que un AppImage le cambia a sus hijos: dentro de uno,
    /// `LD_LIBRARY_PATH` apunta a las bibliotecas que lleva dentro, y el Tesseract del
    /// sistema tiene que cargar las del sistema. Es la misma limpieza que para abrir un
    /// enlace o el gestor de archivos.
    fn orden() -> Command {
        let mut orden = Command::new("tesseract");
        crate::platform::xdg::sin_el_entorno_del_appimage(&mut orden);
        orden
    }

    /// Lanza la orden con esa entrada y devuelve lo que escribe. La entrada va por un hilo y
    /// la salida por otro: las dos son tuberías, y llenando una sin vaciar la otra se quedan
    /// esperándose para siempre.
    fn ejecutar(mut orden: Command, entrada: &[u8]) -> Result<String> {
        let mut hijo = orden.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let (mut tubo, mut salida, mut errores) = match (hijo.stdin.take(), hijo.stdout.take(), hijo.stderr.take()) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return Err(AppError::Msg("no se ha podido hablar con Tesseract".into()));
            }
        };
        let (estado, dicho, queja) = std::thread::scope(|hilos| {
            hilos.spawn(move || {
                // Si la orden muere antes de leerlo todo (un idioma que no está), escribir
                // falla: lo que cuenta es con qué sale ella. Al soltar el tubo se cierra.
                let _ = tubo.write_all(entrada);
            });
            let leer = hilos.spawn(move || {
                let mut todo = Vec::new();
                let _ = salida.read_to_end(&mut todo);
                todo
            });
            let quejas = hilos.spawn(move || {
                let mut todo = Vec::new();
                let _ = errores.read_to_end(&mut todo);
                todo
            });
            let limite = Instant::now() + PLAZO;
            let estado = loop {
                match hijo.try_wait() {
                    Ok(Some(estado)) => break Some(estado),
                    Ok(None) if Instant::now() < limite => std::thread::sleep(Duration::from_millis(4)),
                    // Fuera de plazo, o no se puede saber cómo va: se la mata, y con eso se
                    // cierran sus tuberías y acaban los tres hilos.
                    _ => {
                        let _ = hijo.kill();
                        let _ = hijo.wait();
                        break None;
                    }
                }
            };
            (estado, leer.join().unwrap_or_default(), quejas.join().unwrap_or_default())
        });
        match estado {
            Some(estado) if estado.success() => Ok(String::from_utf8_lossy(&dicho).into_owned()),
            Some(_) => {
                let queja = String::from_utf8_lossy(&queja);
                let ultima = queja.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("sin explicación");
                Err(AppError::Msg(format!("Tesseract no ha podido leer la imagen: {}", ultima.trim())))
            }
            None => Err(AppError::Msg("Tesseract ha tardado demasiado en leer la imagen".into())),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn lista(valores: &[&str]) -> Vec<String> {
        valores.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn cada_idioma_del_sistema_tiene_su_codigo() {
        assert_eq!(codigo_de("es_ES.UTF-8"), Some("spa"));
        assert_eq!(codigo_de("es"), Some("spa"));
        assert_eq!(codigo_de("en_US.UTF-8"), Some("eng"));
        assert_eq!(codigo_de("zh_CN.UTF-8"), Some("chi_sim"));
        assert_eq!(codigo_de("zh_TW"), Some("chi_tra"));
        assert_eq!(codigo_de("ca_ES@valencia"), Some("cat"));
        // De lo que no se sabe no se inventa.
        assert_eq!(codigo_de("tlh"), None);
        assert_eq!(codigo_de(""), None);
    }

    #[test]
    fn se_lee_con_el_idioma_del_sistema_y_con_el_ingles_detras() {
        let instalados = lista(&["eng", "osd", "spa"]);
        assert_eq!(elegir_idiomas(&lista(&["es_ES.UTF-8"]), &instalados).as_deref(), Some("spa+eng"));
        assert_eq!(elegir_idiomas(&lista(&["en_US.UTF-8"]), &instalados).as_deref(), Some("eng"));
        // El que no está instalado no se pide: la orden saldría con un error.
        assert_eq!(elegir_idiomas(&lista(&["fr_FR.UTF-8", "es"]), &instalados).as_deref(), Some("spa+eng"));
        assert_eq!(elegir_idiomas(&lista(&["es"]), &lista(&["eng", "osd"])).as_deref(), Some("eng"));
        // Sin ninguno de los suyos ni inglés, con lo que haya. `osd` no es un idioma.
        assert_eq!(elegir_idiomas(&lista(&["es"]), &lista(&["osd", "deu", "fra", "ita"])).as_deref(), Some("deu+fra"));
        assert_eq!(elegir_idiomas(&lista(&["es"]), &lista(&["osd"])), None);
        assert_eq!(elegir_idiomas(&[], &[]), None);
    }

    #[test]
    fn los_idiomas_salen_de_las_variables_por_orden() {
        let variables = |nombre: &str| match nombre {
            "LANGUAGE" => Some("es_ES:en".to_string()),
            "LANG" => Some("es_ES.UTF-8".to_string()),
            "LC_ALL" => Some("C".to_string()),
            _ => None,
        };
        assert_eq!(idiomas_de(variables), ["es_ES", "en", "es_ES.UTF-8"]);
        assert!(idiomas_de(|_| None).is_empty());
        assert!(idiomas_de(|n| (n == "LANG").then(|| "POSIX".to_string())).is_empty());
    }

    #[test]
    fn la_lista_de_idiomas_se_entiende_en_la_version_4_y_en_la_5() {
        let la_4 = "List of available languages (3):\neng\nosd\nspa\n";
        let la_5 = "List of available languages in \"/usr/share/tesseract-ocr/5/tessdata/\" (3):\neng\nosd\nspa\n";
        let fedora = "List of available languages in \"/usr/share/tesseract/tessdata/\" (2):\neng\nchi_sim\n";
        assert_eq!(idiomas_instalados(la_4), ["eng", "osd", "spa"]);
        assert_eq!(idiomas_instalados(la_5), ["eng", "osd", "spa"]);
        assert_eq!(idiomas_instalados(fedora), ["eng", "chi_sim"]);
        assert!(idiomas_instalados("").is_empty());
    }

    /// Lo que devolvió de verdad el Tesseract 5.3.0 de Debian 12 y el 4.1.1 de Ubuntu 22.04
    /// (etapa `o1` del laboratorio): la confianza llega con decimales en uno y entera en otro.
    const TSV_5: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t1100\t420\t-1\t\n\
2\t1\t1\t0\t0\t0\t28\t59\t760\t40\t-1\t\n\
4\t1\t1\t1\t1\t0\t28\t59\t760\t40\t-1\t\n\
5\t1\t1\t1\t1\t1\t28\t59\t146\t31\t92.779404\tFactura\n\
5\t1\t1\t1\t1\t2\t191\t59\t188\t31\t92.779404\t2026-104\n\
5\t1\t1\t1\t1\t3\t395\t66\t67\t35\t96.731834\tpor\n\
4\t1\t1\t1\t2\t0\t28\t149\t600\t40\t-1\t\n\
5\t1\t1\t1\t2\t1\t28\t149\t190\t31\t95.1\tContacto:\n\
5\t1\t1\t1\t2\t2\t236\t149\t420\t40\t91.0\tmunir@example.com\n";

    #[test]
    fn el_tsv_sale_en_lineas_con_cada_palabra_en_su_caja() {
        let lineas = palabras_del_tsv(TSV_5, 1.0, 0.0);
        assert_eq!(texto_de(&lineas), "Factura 2026-104 por\nContacto: munir@example.com");
        let correo = &lineas[1][1];
        assert_eq!((correo.x, correo.y, correo.ancho, correo.alto), (236.0, 149.0, 420.0, 40.0));
    }

    #[test]
    fn las_cajas_vuelven_al_tamanno_de_la_imagen_y_a_su_altura() {
        // Leída al doble de grande y en una tira que empezaba en la fila 1.000.
        let lineas = palabras_del_tsv(TSV_5, 2.0, 1_000.0);
        let factura = &lineas[0][0];
        assert_eq!((factura.x, factura.y, factura.ancho, factura.alto), (14.0, 1_029.5, 73.0, 15.5));
    }

    #[test]
    fn la_confianza_entera_de_la_version_4_se_lee_igual() {
        let de_la_4 = TSV_5.replace("92.779404", "92").replace("96.731834", "96").replace("95.1", "95").replace("91.0", "91");
        assert_eq!(texto_de(&palabras_del_tsv(&de_la_4, 1.0, 0.0)), texto_de(&palabras_del_tsv(TSV_5, 1.0, 0.0)));
    }

    #[test]
    fn un_tsv_roto_o_vacio_no_da_palabras() {
        assert!(palabras_del_tsv("", 1.0, 0.0).is_empty());
        assert!(palabras_del_tsv("level\tpage_num\n", 1.0, 0.0).is_empty());
        // Una palabra sin texto, una sin tamaño y una fila con letras donde van números.
        let raro = "5\t1\t1\t1\t1\t1\t10\t10\t50\t20\t90\t \n5\t1\t1\t1\t1\t2\t10\t10\t0\t20\t90\thola\n5\t1\tx\t1\t1\t3\t10\t10\t50\t20\t90\thola\n";
        assert!(palabras_del_tsv(raro, 1.0, 0.0).is_empty());
    }

    /// Un texto con tabuladores dentro no existe (Tesseract parte por espacios), pero la
    /// última columna se coge entera por si acaso.
    #[test]
    fn dos_trozos_a_la_misma_altura_son_un_renglon() {
        let tsv = "5\t1\t2\t1\t1\t1\t600\t102\t60\t20\t90\tCtrl\n\
5\t1\t1\t1\t1\t1\t40\t100\t90\t22\t90\tGrabar\n\
5\t1\t1\t1\t1\t2\t140\t100\t90\t22\t90\tregión\n\
5\t1\t3\t1\t1\t1\t40\t160\t80\t22\t90\tTecla\n";
        let lineas = palabras_del_tsv(tsv, 1.0, 0.0);
        assert_eq!(texto_de(&lineas), "Grabar región Ctrl\nTecla");
    }

    #[test]
    fn con_otro_idioma_delante_se_repasa_en_ingles() {
        assert!(pide_pasada_en_ingles("spa+eng"));
        assert!(pide_pasada_en_ingles("eng+spa"));
        assert!(pide_pasada_en_ingles("fra+spa+eng"));
        assert!(!pide_pasada_en_ingles("eng"), "ya es la pasada en inglés");
        assert!(!pide_pasada_en_ingles("spa"), "sin el inglés instalado no hay con qué repasar");
    }

    fn palabra(texto: &str, x: f32, y: f32, ancho: f32) -> Palabra {
        Palabra { texto: texto.into(), x, y, ancho, alto: 20.0 }
    }

    #[test]
    fn el_correo_que_el_espannol_parte_se_coge_de_la_pasada_en_ingles() {
        // Lo medido en el laboratorio: con `spa`, «ana. lopezOejemplo.com»; con `eng`, el correo
        // entero y lo demás sin tildes.
        let mut lineas = vec![
            vec![palabra("El", 16.0, 50.0, 14.0), palabra("camión", 36.0, 50.0, 60.0)],
            vec![
                palabra("Correo:", 16.0, 86.0, 52.0),
                palabra("ana.", 72.0, 86.0, 30.0),
                palabra("lopezOejemplo.com", 106.0, 86.0, 135.0),
            ],
        ];
        let en_ingles = vec![
            vec![palabra("El", 16.0, 50.0, 14.0), palabra("camion", 36.0, 50.0, 60.0)],
            vec![palabra("Correo:", 16.0, 86.0, 52.0), palabra("ana.lopez@ejemplo.com", 72.0, 87.0, 169.0)],
        ];
        rescatar_correos(&mut lineas, &en_ingles);
        assert_eq!(texto_de(&lineas), "El camión
Correo: ana.lopez@ejemplo.com");
        let correo = &lineas[1][1];
        assert_eq!((correo.x, correo.ancho), (72.0, 169.0), "con la caja entera, que es lo que se tapa");
    }

    #[test]
    fn lo_que_ya_estaba_bien_leido_no_se_toca() {
        let bien = vec![
            vec![palabra("Correo:", 16.0, 86.0, 52.0), palabra("ana.lopez@ejemplo.com", 72.0, 86.0, 169.0)],
            vec![palabra("mañana", 16.0, 120.0, 60.0)],
        ];
        let mut lineas = bien.clone();
        // La pasada en inglés trae el mismo correo, «manana» y una arroba suelta que no es un correo.
        let en_ingles = vec![
            vec![palabra("ana.lopez@ejemplo.com", 73.0, 86.0, 168.0)],
            vec![palabra("manana", 16.0, 120.0, 60.0), palabra("@", 90.0, 120.0, 12.0), palabra("a@b", 110.0, 120.0, 30.0)],
        ];
        rescatar_correos(&mut lineas, &en_ingles);
        assert_eq!(lineas, bien);
    }

    #[test]
    fn un_correo_que_solo_vio_la_pasada_en_ingles_entra_en_su_sitio() {
        let mut lineas = vec![vec![palabra("Arriba", 16.0, 10.0, 60.0)], vec![palabra("Abajo", 16.0, 200.0, 60.0)]];
        rescatar_correos(&mut lineas, &[vec![palabra("a@b.org", 16.0, 100.0, 70.0)]]);
        assert_eq!(texto_de(&lineas), "Arriba
a@b.org
Abajo");
    }

    #[test]
    fn la_imagen_se_prepara_en_gris_agrandada_y_con_el_fondo_claro() {
        // Letra clara sobre fondo oscuro: sale con el fondo claro.
        let mut oscura = RgbaImage::from_pixel(40, 20, image::Rgba([17, 17, 19, 255]));
        oscura.put_pixel(5, 5, image::Rgba([240, 240, 240, 255]));
        let lista = preparar(&oscura, 0, 20).expect("la imagen preparada");
        assert_eq!((lista.ancho, lista.alto, lista.escala), (120, 60, 3.0));
        assert_eq!(lista.gris.len(), 120 * 60);
        assert!(lista.gris[0] > 200, "el fondo ha quedado en {}", lista.gris[0]);
        let pgm = lista.pgm();
        assert!(pgm.starts_with(b"P5\n120 60\n255\n"));
        assert_eq!(pgm.len(), b"P5\n120 60\n255\n".len() + 120 * 60);
        // Una clara se queda como está, y solo las filas que se piden.
        let clara = RgbaImage::from_pixel(40, 20, image::Rgba([246, 246, 247, 255]));
        let trozo = preparar(&clara, 5, 15).expect("el trozo");
        assert_eq!((trozo.ancho, trozo.alto), (120, 30));
        assert!(trozo.gris[0] > 200);
        assert!(preparar(&clara, 15, 15).is_none());
        assert!(preparar(&clara, 30, 40).is_none());
    }

    #[test]
    fn cuanto_mas_grande_la_imagen_menos_se_agranda() {
        assert_eq!(cuanto_agrandar(800 * 400), 3);
        assert_eq!(cuanto_agrandar(1_920 * 1_080), 3);
        assert_eq!(cuanto_agrandar(2_560 * 1_440), 2);
        assert_eq!(cuanto_agrandar(3_840 * 2_160), 1);
        let enorme = RgbaImage::from_pixel(3_000, 1_800, image::Rgba([255, 255, 255, 255]));
        let lista = preparar(&enorme, 0, 1_800).expect("la imagen preparada");
        assert_eq!((lista.ancho, lista.alto, lista.escala), (3_000, 1_800, 1.0));
    }

    #[test]
    fn a_cada_distribucion_se_le_dice_su_paquete_y_el_de_su_idioma() {
        let debian = "PRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"\nID=debian\n";
        let mint = "NAME=\"Linux Mint\"\nID=linuxmint\nID_LIKE=\"ubuntu debian\"\n";
        let fedora = "NAME=\"Fedora Linux\"\nVERSION_ID=44\nID=fedora\n";
        let es = lista(&["es_ES.UTF-8"]);
        let en = lista(&["en_US.UTF-8"]);
        let zh = lista(&["zh_CN.UTF-8"]);
        for de_apt in [debian, mint] {
            assert_eq!(orden_para(de_apt, &Falta::Programa, &es).as_deref(), Some("sudo apt install tesseract-ocr tesseract-ocr-spa"));
            assert_eq!(orden_para(de_apt, &Falta::Programa, &en).as_deref(), Some("sudo apt install tesseract-ocr"));
            assert_eq!(orden_para(de_apt, &Falta::Programa, &zh).as_deref(), Some("sudo apt install tesseract-ocr tesseract-ocr-chi-sim"));
            assert_eq!(orden_para(de_apt, &Falta::Idiomas, &es).as_deref(), Some("sudo apt install tesseract-ocr-spa"));
            assert_eq!(orden_para(de_apt, &Falta::Idiomas, &en).as_deref(), Some("sudo apt install tesseract-ocr-eng"));
        }
        assert_eq!(orden_para(fedora, &Falta::Programa, &es).as_deref(), Some("sudo dnf install tesseract tesseract-langpack-spa"));
        assert_eq!(orden_para(fedora, &Falta::Programa, &zh).as_deref(), Some("sudo dnf install tesseract tesseract-langpack-chi_sim"));
        assert_eq!(orden_para(fedora, &Falta::Idiomas, &[]).as_deref(), Some("sudo dnf install tesseract-langpack-eng"));
        assert_eq!(orden_para("ID=arch\n", &Falta::Programa, &es), None);
        assert_eq!(orden_para("", &Falta::Programa, &es), None);
    }

    #[test]
    fn lo_que_falta_se_dice_con_la_orden_al_final() {
        assert_eq!(
            frase_de_lo_que_falta(&Falta::Programa, Some("sudo apt install tesseract-ocr tesseract-ocr-spa")),
            "Para leer el texto falta Tesseract. Se instala con: sudo apt install tesseract-ocr tesseract-ocr-spa"
        );
        let sin = frase_de_lo_que_falta(&Falta::Programa, None);
        assert!(sin.starts_with("Para leer el texto falta Tesseract.") && !sin.contains("sudo"), "{sin}");
        assert!(frase_de_lo_que_falta(&Falta::Idiomas, None).starts_with("Tesseract no tiene ningún idioma instalado."));
    }
}

// De punta a punta, con el Tesseract de verdad.
#[cfg(all(test, target_os = "linux"))]
#[path = "tesseract_pruebas.rs"]
pub(crate) mod pruebas_de_verdad;
