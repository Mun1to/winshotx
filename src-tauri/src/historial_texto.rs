//! Buscar en el historial por el texto de las capturas (S26).
//!
//! Munito lo pidió así: «escribes una palabra y salen las capturas que la llevan escrita».
//!
//! **El texto lo lee el mismo lector de Windows que «Copiar el texto»** (`platform::ocr`):
//! viene con Windows, no engorda el instalador y no necesita internet. Lo leído se apunta en
//! un archivo de la carpeta LOCAL de la app (`%LOCALAPPDATA%`, que a diferencia de
//! `%APPDATA%` no viaja con un perfil móvil de empresa): **el texto de las capturas nunca
//! sale del PC.**
//!
//! **Cuándo se lee.** Cada foto que se guarda, nada más guardarla y en un hilo aparte, para
//! que la captura no espere a nadie. Las que ya había en la carpeta, de la más nueva a la más
//! vieja y con una pausa después de cada una de cuatro veces lo que tardó en leerla: así el
//! lector no pasa de una quinta parte de un núcleo. Mientras se graba no lee nada, y al
//! arrancar espera medio minuto, que es cuando el PC tiene más que hacer (salvo que alguien
//! abra el historial o guarde algo antes).
//!
//! **Qué se lee.** Solo las fotos. Un vídeo cambia de texto a cada segundo y leerlo entero
//! costaría más que grabarlo; un fotograma suelto diría que lleva escrito lo que salió un
//! instante. La búsqueda lo dice en la ventana («busca en el texto de las fotos»).
//!
//! **El índice es un apunte de la carpeta, no la carpeta.** Como el historial, manda lo que
//! hay en el disco: una foto que se borra se olvida en la siguiente vuelta (y su texto sale
//! del archivo), y una que se sobrescribe (otro tamaño u otra fecha) se vuelve a leer.
//!
//! **El archivo es una línea JSON por foto leída**, añadida al final: leer mil fotos no
//! reescribe mil veces un archivo de megas, y si se va la luz a medias se pierde como mucho
//! la última línea. Cuando sobran líneas (una foto olvidada o leída dos veces), se reescribe
//! entero en cuanto el lector se queda sin trabajo.
//!
//! **Sin tildes ni mayúsculas**: «camion» encuentra «Camión», y «CAMIÓN» también. Varias
//! palabras tienen que estar todas, en cualquier orden.

// Donde no hay lector (ni Windows ni Linux) el hilo no arranca y casi todo esto se queda sin
// usar. En Linux lee el Tesseract del sistema (S45).
#![cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::historial::Entrada;

/// El archivo del índice, en la carpeta local de la app.
const ARCHIVO: &str = "texto-de-las-fotos.jsonl";

/// Lo que espera al arrancar antes de ponerse con las fotos viejas.
const ESPERA_AL_ARRANCAR: Duration = Duration::from_secs(30);

/// La pausa más corta entre dos fotos viejas, aunque leer la última fuera instantáneo.
const PAUSA_MINIMA: Duration = Duration::from_millis(400);

/// Cada cuánto mira la carpeta sin que nadie avise: por lo que llegue a ella desde fuera de
/// winshotx (una foto copiada a mano).
const VUELTA_SIN_AVISO: Duration = Duration::from_secs(300);

/// Pasa un texto a la forma en que se busca: minúsculas, sin tildes ni diéresis, y con
/// todos los espacios y saltos de línea seguidos convertidos en uno.
pub fn normalizar(texto: &str) -> String {
    let mut fuera = String::with_capacity(texto.len());
    let mut hueco = false;
    for c in texto.chars().flat_map(char::to_lowercase) {
        if c.is_whitespace() {
            hueco = !fuera.is_empty();
            continue;
        }
        // Las marcas sueltas que van encima de la letra de antes (una «é» escrita como «e»
        // más el acento) se quedan fuera, igual que el acento de una letra ya compuesta.
        if ('\u{300}'..='\u{36f}').contains(&c) {
            continue;
        }
        if hueco {
            fuera.push(' ');
            hueco = false;
        }
        fuera.push(sin_tilde(c));
    }
    fuera
}

/// La letra sin su tilde, diéresis, virgulilla o cedilla. Lo que no es una letra latina con
/// marca (un número, un ideograma, una «ß») se queda como está.
fn sin_tilde(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' | 'ŧ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        otra => otra,
    }
}

/// Las palabras de lo que se escribe en la caja, ya normalizadas.
pub fn palabras(busqueda: &str) -> Vec<String> {
    normalizar(busqueda).split(' ').filter(|p| !p.is_empty()).map(String::from).collect()
}

/// Una línea del archivo: la foto, con qué tamaño y fecha se leyó, y su texto tal cual.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Linea {
    ruta: String,
    bytes: u64,
    cuando_ms: u64,
    texto: String,
}

/// Una foto leída, con su texto ya preparado para buscar.
#[derive(Debug, Clone)]
struct Leida {
    bytes: u64,
    cuando_ms: u64,
    texto: String,
    normal: String,
}

/// Lo que se sabe del texto de las fotos, por ruta.
#[derive(Debug, Default)]
pub struct Indice {
    leidas: HashMap<String, Leida>,
    /// El archivo tiene líneas de más (fotos olvidadas o leídas dos veces) y toca reescribirlo.
    sobran: bool,
}

impl Indice {
    /// Lo que hay en el archivo. Un archivo que no existe es un índice vacío, y una línea
    /// rota (la última, si se fue la luz mientras se escribía) se salta.
    pub fn cargar(archivo: &Path) -> Self {
        let mut indice = Indice::default();
        let Ok(todo) = std::fs::read_to_string(archivo) else {
            return indice;
        };
        let mut lineas = 0;
        for linea in todo.lines().filter(|l| !l.trim().is_empty()) {
            lineas += 1;
            if let Ok(l) = serde_json::from_str::<Linea>(linea) {
                indice.meter(l);
            }
        }
        indice.sobran = lineas != indice.leidas.len();
        indice
    }

    fn meter(&mut self, l: Linea) {
        let normal = normalizar(&l.texto);
        self.leidas.insert(
            l.ruta,
            Leida { bytes: l.bytes, cuando_ms: l.cuando_ms, texto: l.texto, normal },
        );
    }

    pub fn len(&self) -> usize {
        self.leidas.len()
    }

    /// Si esa foto ya está leída tal como está ahora en el disco.
    pub fn al_dia(&self, entrada: &Entrada) -> bool {
        self.leidas
            .get(&entrada.ruta)
            .is_some_and(|l| l.bytes == entrada.bytes && l.cuando_ms == entrada.cuando_ms)
    }

    /// Cuántas fotos de la lista faltan por leer.
    pub fn cuantas_por_leer(&self, entradas: &[Entrada]) -> usize {
        entradas.iter().filter(|e| e.tipo == "foto" && !self.al_dia(e)).count()
    }

    /// Las fotos de la lista que faltan por leer, en el mismo orden (el historial las da de
    /// la más nueva a la más vieja, que es el orden en que se buscan).
    pub fn por_leer(&self, entradas: &[Entrada]) -> Vec<Entrada> {
        entradas
            .iter()
            .filter(|e| e.tipo == "foto" && !self.al_dia(e))
            .cloned()
            .collect()
    }

    /// Apunta el texto de una foto, y si hay archivo, lo añade al final.
    pub fn apuntar(&mut self, archivo: Option<&Path>, entrada: &Entrada, texto: String) -> std::io::Result<()> {
        let linea = Linea {
            ruta: entrada.ruta.clone(),
            bytes: entrada.bytes,
            cuando_ms: entrada.cuando_ms,
            texto,
        };
        if self.leidas.contains_key(&linea.ruta) {
            self.sobran = true;
        }
        let escrita = match archivo {
            Some(archivo) => añadir(archivo, &linea),
            None => Ok(()),
        };
        self.meter(linea);
        escrita
    }

    /// Olvida las fotos de las que `sigue` dice que ya no están, y dice cuántas.
    pub fn olvidar(&mut self, sigue: impl Fn(&str) -> bool) -> usize {
        let antes = self.leidas.len();
        self.leidas.retain(|ruta, _| sigue(ruta));
        let olvidadas = antes - self.leidas.len();
        if olvidadas > 0 {
            self.sobran = true;
        }
        olvidadas
    }

    /// Si la foto lleva escritas todas las palabras. Una que no se ha leído no lleva nada.
    pub fn encaja(&self, ruta: &str, palabras: &[String]) -> bool {
        self.leidas
            .get(ruta)
            .is_some_and(|l| palabras.iter().all(|p| l.normal.contains(p.as_str())))
    }

    /// Reescribe el archivo con una línea por foto que sigue, si sobraban. Se escribe al
    /// lado y se cambia de nombre al final: un corte a medias deja el de antes entero.
    pub fn ordenar_el_archivo(&mut self, archivo: &Path) -> std::io::Result<()> {
        if !self.sobran {
            return Ok(());
        }
        let nuevo = archivo.with_extension("jsonl.nuevo");
        {
            let mut escritor = std::io::BufWriter::new(std::fs::File::create(&nuevo)?);
            for (ruta, l) in &self.leidas {
                let linea = Linea {
                    ruta: ruta.clone(),
                    bytes: l.bytes,
                    cuando_ms: l.cuando_ms,
                    texto: l.texto.clone(),
                };
                serde_json::to_writer(&mut escritor, &linea)?;
                escritor.write_all(b"\n")?;
            }
            escritor.flush()?;
        }
        std::fs::rename(&nuevo, archivo)?;
        self.sobran = false;
        Ok(())
    }
}

/// Añade una línea al final del archivo, creándolo si hace falta.
fn añadir(archivo: &Path, linea: &Linea) -> std::io::Result<()> {
    let mut texto = serde_json::to_string(linea)?;
    texto.push('\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(archivo)?
        .write_all(texto.as_bytes())
}

/// Lo que ha cambiado en la carpeta desde la última vuelta: olvida lo que ya no está y
/// devuelve lo que falta por leer, de lo más nuevo a lo más viejo.
///
/// Lo de otra carpeta (la de capturas de antes de cambiarla en los ajustes) se queda
/// mientras exista, por si se vuelve a ella; lo que se ha borrado se olvida esté donde esté.
pub fn mirar_la_carpeta(indice: &mut Indice, carpeta: &Path) -> Vec<Entrada> {
    // Una carpeta que no está (un disco USB desenchufado) no es una carpeta vacía: si se
    // olvidara todo, al volver el disco habría que leer otra vez cada foto.
    if !carpeta.is_dir() {
        return Vec::new();
    }
    let fotos = crate::historial::leer(carpeta, Some("foto"));
    let presentes: HashSet<&str> = fotos.iter().map(|e| e.ruta.as_str()).collect();
    indice.olvidar(|ruta| {
        presentes.contains(ruta) || {
            let ruta = Path::new(ruta);
            ruta.parent() != Some(carpeta) && ruta.is_file()
        }
    });
    indice.por_leer(&fotos)
}

/// Deja en la lista solo lo que lleva escrito lo buscado. Con la caja vacía (o solo con
/// espacios), la lista se queda entera: el historial de siempre.
pub fn filtrar(indice: &Indice, entradas: &mut Vec<Entrada>, busqueda: &str) {
    let palabras = palabras(busqueda);
    if palabras.is_empty() {
        return;
    }
    entradas.retain(|e| indice.encaja(&e.ruta, &palabras));
}

/// Lo que se le dice al lector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aviso {
    /// Se acaba de guardar algo en la carpeta: que lo lea ya, sin acabar la pausa.
    Guardada,
    /// Alguien está mirando el historial: que mire la carpeta cuando le toque, por si ha
    /// llegado algo por fuera, pero sin saltarse la pausa.
    Mirar,
}

/// El buscador, como estado de la app: el índice, el canal para avisar al lector y si se
/// puede buscar.
pub struct Buscador {
    indice: Mutex<Indice>,
    archivo: Option<PathBuf>,
    aviso: Mutex<Sender<Aviso>>,
    /// Si hay lector en este PC: en Linux, o en un Windows sin ningún idioma con lector
    /// instalado, la caja no sale.
    disponible: AtomicBool,
}

impl Buscador {
    fn avisar(&self, aviso: Aviso) {
        let _ = self.aviso.lock().send(aviso);
    }

    pub fn disponible(&self) -> bool {
        self.disponible.load(Ordering::Relaxed)
    }

    /// Cuántas fotos de la lista faltan por leer. Se cuenta al pedirlo, sobre lo que hay en
    /// la carpeta: recién arrancada la app el lector todavía no ha dado su primera vuelta, y
    /// una cuenta suya diría cero justo cuando faltan todas.
    pub fn cuantas_por_leer(&self, entradas: &[Entrada]) -> usize {
        self.indice.lock().cuantas_por_leer(entradas)
    }

    pub fn filtrar(&self, entradas: &mut Vec<Entrada>, busqueda: &str) {
        filtrar(&self.indice.lock(), entradas, busqueda);
    }
}

/// Pone el buscador como estado de la app y arranca el lector en su hilo. El índice se
/// carga dentro del hilo: con miles de fotos son unos megas, y el arranque no los espera.
pub fn arrancar(app: &AppHandle) {
    let archivo = app.path().app_local_data_dir().ok().map(|d| d.join(ARCHIVO));
    let (aviso, avisos) = mpsc::channel();
    if !app.manage(Buscador {
        indice: Mutex::new(Indice::default()),
        archivo,
        aviso: Mutex::new(aviso),
        disponible: AtomicBool::new(false),
    }) { return; }
    #[cfg(any(windows, target_os = "linux"))]
    {
        let app = app.clone();
        let _ = std::thread::Builder::new()
            .name("texto-del-historial".into())
            .spawn(move || trabajar(app, avisos));
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    drop(avisos);
}

/// Se acaba de guardar algo en la carpeta de capturas: que el lector lo lea ya.
pub fn avisar(app: &AppHandle) {
    if let Some(buscador) = app.try_state::<Buscador>() {
        buscador.avisar(Aviso::Guardada);
    }
}

/// La ventana está mirando el historial: que el lector mire la carpeta cuando pueda.
pub fn mirar(app: &AppHandle) {
    if let Some(buscador) = app.try_state::<Buscador>() {
        buscador.avisar(Aviso::Mirar);
    }
}

/// Espera lo que se le diga, salvo que llegue un aviso de algo recién guardado. Lo que
/// llegue entretanto se apunta en `mirar`. Devuelve `false` si el canal se ha cerrado.
fn esperar(avisos: &Receiver<Aviso>, cuanto: Duration, mirar: &mut bool) -> bool {
    let hasta = Instant::now() + cuanto;
    loop {
        let queda = hasta.saturating_duration_since(Instant::now());
        if queda.is_zero() {
            return true;
        }
        match avisos.recv_timeout(queda) {
            Ok(Aviso::Guardada) => {
                *mirar = true;
                return true;
            }
            Ok(Aviso::Mirar) => *mirar = true,
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

/// El lector: mira la carpeta, lee la foto más nueva que falte, descansa, y vuelta.
#[cfg(any(windows, target_os = "linux"))]
fn trabajar(app: AppHandle, avisos: Receiver<Aviso>) {
    use crate::state::AppState;

    let buscador = app.state::<Buscador>();
    if let Some(archivo) = &buscador.archivo {
        if let Some(carpeta) = archivo.parent() {
            let _ = std::fs::create_dir_all(carpeta);
        }
        *buscador.indice.lock() = Indice::cargar(archivo);
    }
    let lector = loop {
        match crate::platform::ocr::Lector::nuevo() {
            Ok(lector) => break lector,
            Err(e) => {
                // Sin lector no hay nada que buscar: la caja no sale y el historial sigue igual.
                eprintln!("[historial] sin lector de texto: {e}");
                // En Windows el lector es del sistema: si no está, no va a aparecer.
                #[cfg(windows)]
                return;
                // En Linux es un programa que se instala cuando la app dice que falta: se
                // vuelve a mirar cada vez que alguien guarda una foto o abre el historial.
                #[cfg(not(windows))]
                if avisos.recv().is_err() {
                    return;
                }
            }
        }
    };
    buscador.disponible.store(true, Ordering::Relaxed);

    let carpeta = || PathBuf::from(app.state::<AppState>().settings.read().save_directory.clone());
    let grabando = || app.state::<AppState>().recording.lock().is_some();

    // Al arrancar espera, salvo que alguien guarde algo o se ponga a mirar el historial: ahí
    // ya hay alguien delante que quiere encontrar sus fotos.
    if let Err(RecvTimeoutError::Disconnected) = avisos.recv_timeout(ESPERA_AL_ARRANCAR) {
        return;
    }
    let mut mirar = true;
    let mut cola: Vec<Entrada> = Vec::new();
    loop {
        // Mientras se graba, nada: grabar ya pide todo lo que tiene la máquina.
        if grabando() {
            match avisos.recv_timeout(Duration::from_secs(2)) {
                Err(RecvTimeoutError::Disconnected) => return,
                Ok(_) => mirar = true,
                Err(RecvTimeoutError::Timeout) => {}
            }
            continue;
        }
        if mirar {
            mirar = false;
            while avisos.try_recv().is_ok() {}
            let mut indice = buscador.indice.lock();
            let antes = indice.len();
            cola = mirar_la_carpeta(&mut indice, &carpeta());
            // Una foto borrada se lleva su texto del disco ya, no cuando se acaben las viejas.
            if indice.len() < antes {
                if let Some(archivo) = &buscador.archivo {
                    if let Err(e) = indice.ordenar_el_archivo(archivo) {
                        eprintln!("[historial] no se ha podido reescribir el índice: {e}");
                    }
                }
            }
        }
        if cola.is_empty() {
            if let Some(archivo) = &buscador.archivo {
                if let Err(e) = buscador.indice.lock().ordenar_el_archivo(archivo) {
                    eprintln!("[historial] no se ha podido reescribir el índice: {e}");
                }
            }
            match avisos.recv_timeout(VUELTA_SIN_AVISO) {
                Err(RecvTimeoutError::Disconnected) => return,
                _ => mirar = true,
            }
            continue;
        }
        let entrada = cola.remove(0);
        let empezo = Instant::now();
        // Una foto que no se deja leer (rota, o de un formato que Windows no abre) se apunta
        // sin texto: si no, se intentaría otra vez en cada vuelta. Si cambia, se relee.
        let texto = lector.leer_archivo(Path::new(&entrada.ruta)).unwrap_or_else(|e| {
            eprintln!("[historial] no se ha podido leer {}: {e}", entrada.nombre);
            String::new()
        });
        let tardo = empezo.elapsed();
        if let Err(e) = buscador.indice.lock().apuntar(buscador.archivo.as_deref(), &entrada, texto) {
            eprintln!("[historial] no se ha podido apuntar {}: {e}", entrada.nombre);
        }
        if !esperar(&avisos, (tardo * 4).max(PAUSA_MINIMA), &mut mirar) {
            return;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entrada(ruta: &str, bytes: u64, cuando_ms: u64) -> Entrada {
        Entrada {
            ruta: ruta.into(),
            // A mano y no con `Path`: estas rutas son de Windows, y en Linux `\` no separa nada.
            nombre: ruta.rsplit(['\\', '/']).next().unwrap().into(),
            tipo: if ruta.ends_with(".mp4") || ruta.ends_with(".gif") { "video" } else { "foto" },
            bytes,
            cuando_ms,
        }
    }

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("winshotx-historial-texto-{nombre}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sin_tildes_ni_mayusculas() {
        assert_eq!(normalizar("Camión"), "camion");
        assert_eq!(normalizar("CAMIÓN DE REPARTO"), "camion de reparto");
        assert_eq!(normalizar("Ñandú, pingüino y garçon"), "nandu, pinguino y garcon");
        // Una «é» escrita como «e» y el acento suelto detrás.
        assert_eq!(normalizar("cafe\u{301}"), "cafe");
        // Lo que no lleva tildes se queda igual: números, ideogramas, signos.
        assert_eq!(normalizar("Factura 2026-104 截图"), "factura 2026-104 截图");
    }

    #[test]
    fn los_saltos_y_espacios_seguidos_son_un_espacio() {
        assert_eq!(normalizar("  Hola\n\n\tmundo   feliz  "), "hola mundo feliz");
        assert_eq!(palabras("  Camión   ROJO "), ["camion", "rojo"]);
        assert!(palabras("   ").is_empty());
    }

    #[test]
    fn camion_encuentra_camion_con_tilde_y_todas_las_palabras_tienen_que_estar() {
        let mut indice = Indice::default();
        let a = entrada(r"C:\c\a.png", 10, 1);
        let b = entrada(r"C:\c\b.png", 10, 2);
        indice.apuntar(None, &a, "El Camión de reparto\nllega a las 10:30".into()).unwrap();
        indice.apuntar(None, &b, "Factura de la luz".into()).unwrap();
        assert!(indice.encaja(&a.ruta, &palabras("camion")));
        assert!(indice.encaja(&a.ruta, &palabras("CAMIÓN")));
        assert!(indice.encaja(&a.ruta, &palabras("reparto camion")), "en cualquier orden");
        assert!(indice.encaja(&a.ruta, &palabras("repar")), "un trozo de palabra también");
        assert!(!indice.encaja(&a.ruta, &palabras("camion factura")), "tienen que estar todas");
        assert!(indice.encaja(&b.ruta, &palabras("factura")));
        assert!(!indice.encaja(r"C:\c\sin-leer.png", &palabras("camion")));
    }

    #[test]
    fn filtrar_deja_lo_que_lo_lleva_y_sin_texto_no_toca_nada() {
        let mut indice = Indice::default();
        let fotos = vec![
            entrada(r"C:\c\a.png", 1, 3),
            entrada(r"C:\c\b.png", 1, 2),
            entrada(r"C:\c\v.mp4", 1, 1),
        ];
        indice.apuntar(None, &fotos[0], "camión".into()).unwrap();
        indice.apuntar(None, &fotos[1], "otra cosa".into()).unwrap();
        let mut todas = fotos.clone();
        filtrar(&indice, &mut todas, "   ");
        assert_eq!(todas, fotos);
        filtrar(&indice, &mut todas, "Camion");
        assert_eq!(todas, [fotos[0].clone()]);
    }

    #[test]
    fn una_foto_cambiada_se_vuelve_a_leer_y_los_videos_no_se_leen() {
        let mut indice = Indice::default();
        let a = entrada(r"C:\c\a.png", 10, 1);
        indice.apuntar(None, &a, "x".into()).unwrap();
        assert!(indice.al_dia(&a));
        assert!(!indice.al_dia(&entrada(r"C:\c\a.png", 11, 1)), "otro tamaño");
        assert!(!indice.al_dia(&entrada(r"C:\c\a.png", 10, 2)), "otra fecha");
        let lista = [entrada(r"C:\c\n.png", 1, 9), entrada(r"C:\c\v.mp4", 1, 8), a.clone(), entrada(r"C:\c\m.jpg", 1, 0)];
        let nombres: Vec<String> = indice.por_leer(&lista).into_iter().map(|e| e.nombre).collect();
        assert_eq!(nombres, ["n.png", "m.jpg"], "lo nuevo primero, sin el vídeo ni la ya leída");
        assert_eq!(indice.cuantas_por_leer(&lista), 2);
    }

    /// El archivo se lee como se escribió; una línea rota no tumba el resto; leer dos veces
    /// la misma foto se queda con la última; y al reescribirlo queda una línea por foto.
    #[test]
    fn el_archivo_sobrevive_a_un_corte_y_se_reescribe_sin_lo_que_sobra() {
        let dir = carpeta_de_prueba("archivo");
        let archivo = dir.join(ARCHIVO);
        let mut indice = Indice::default();
        let a = entrada(r"C:\c\a.png", 10, 1);
        let b = entrada(r"C:\c\b.png", 20, 2);
        indice.apuntar(Some(&archivo), &a, "primera vez".into()).unwrap();
        indice.apuntar(Some(&archivo), &b, "Camión".into()).unwrap();
        indice.apuntar(Some(&archivo), &entrada(r"C:\c\a.png", 11, 3), "segunda vez".into()).unwrap();
        // Lo que deja un corte de luz a media línea.
        std::fs::OpenOptions::new().append(true).open(&archivo).unwrap().write_all(b"{\"ruta\":\"C:\\\\c\\\\z.p").unwrap();

        let mut leido = Indice::cargar(&archivo);
        assert_eq!(leido.len(), 2);
        assert!(leido.sobran);
        assert!(leido.encaja(&a.ruta, &palabras("segunda")));
        assert!(!leido.encaja(&a.ruta, &palabras("primera")));
        assert!(leido.encaja(&b.ruta, &palabras("camion")));

        assert_eq!(leido.olvidar(|ruta| ruta != b.ruta), 1);
        leido.ordenar_el_archivo(&archivo).unwrap();
        let texto = std::fs::read_to_string(&archivo).unwrap();
        assert_eq!(texto.lines().count(), 1, "una línea por foto que sigue:\n{texto}");
        assert!(!texto.contains("Camión"), "el texto de la foto olvidada sale del disco");
        let otra_vez = Indice::cargar(&archivo);
        assert_eq!(otra_vez.len(), 1);
        assert!(!otra_vez.sobran);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_archivo_que_no_existe_es_un_indice_vacio() {
        let indice = Indice::cargar(Path::new(r"C:\no\existe\texto.jsonl"));
        assert_eq!(indice.len(), 0);
        assert!(!indice.sobran);
    }

    fn escribir(ruta: &Path, segundos_atras: u64) {
        std::fs::write(ruta, b"x").unwrap();
        let cuando = std::time::SystemTime::now() - Duration::from_secs(segundos_atras);
        std::fs::File::options().write(true).open(ruta).unwrap().set_modified(cuando).unwrap();
    }

    /// Una vuelta por la carpeta: lo borrado se olvida, lo de otra carpeta que sigue
    /// existiendo se queda, y sale lo que falta de lo más nuevo a lo más viejo, sin vídeos.
    #[test]
    fn mirar_la_carpeta_olvida_lo_borrado_y_ordena_lo_que_falta() {
        let dir = carpeta_de_prueba("carpeta");
        let otra = carpeta_de_prueba("carpeta-de-antes");
        escribir(&dir.join("vieja.png"), 300);
        escribir(&dir.join("nueva.png"), 10);
        escribir(&dir.join("leida.png"), 100);
        escribir(&dir.join("video.mp4"), 5);
        escribir(&otra.join("de-antes.png"), 50);

        let mut indice = Indice::default();
        let leida = crate::historial::leer(&dir, None).into_iter().find(|e| e.nombre == "leida.png").unwrap();
        indice.apuntar(None, &leida, "ya leída".into()).unwrap();
        indice.apuntar(None, &entrada(&dir.join("borrada.png").to_string_lossy(), 1, 1), "se fue".into()).unwrap();
        let de_antes = crate::historial::leer(&otra, None).remove(0);
        indice.apuntar(None, &de_antes, "sigue".into()).unwrap();
        indice.apuntar(None, &entrada(&otra.join("perdida.png").to_string_lossy(), 1, 1), "no".into()).unwrap();

        let cola = mirar_la_carpeta(&mut indice, &dir);
        let nombres: Vec<&str> = cola.iter().map(|e| e.nombre.as_str()).collect();
        assert_eq!(nombres, ["nueva.png", "vieja.png"]);
        assert_eq!(indice.len(), 2, "quedan la leída y la de la otra carpeta");
        assert!(indice.encaja(&de_antes.ruta, &palabras("sigue")));
        assert!(indice.sobran);

        // Si la carpeta desaparece (el disco desenchufado), no se olvida nada.
        let _ = std::fs::remove_dir_all(&dir);
        assert!(mirar_la_carpeta(&mut indice, &dir).is_empty());
        assert_eq!(indice.len(), 2, "con la carpeta fuera no se olvida lo leído");
        let _ = std::fs::remove_dir_all(&otra);
    }

    /// Un aviso de algo recién guardado corta la pausa; uno de «mira la carpeta» no.
    #[test]
    fn solo_lo_recien_guardado_corta_la_pausa() {
        let (aviso, avisos) = mpsc::channel();
        aviso.send(Aviso::Mirar).unwrap();
        let mut mirar = false;
        let empezo = Instant::now();
        assert!(esperar(&avisos, Duration::from_millis(150), &mut mirar));
        assert!(empezo.elapsed() >= Duration::from_millis(140), "«mirar» no corta la pausa");
        assert!(mirar);

        aviso.send(Aviso::Guardada).unwrap();
        let mut mirar = false;
        let empezo = Instant::now();
        assert!(esperar(&avisos, Duration::from_secs(5), &mut mirar));
        assert!(empezo.elapsed() < Duration::from_secs(1), "lo recién guardado se lee ya");
        assert!(mirar);

        drop(aviso);
        assert!(!esperar(&avisos, Duration::from_secs(5), &mut mirar), "canal cerrado, se acaba");
    }

    /// El camino entero con el lector de Windows de verdad: unas capturas con texto en una
    /// carpeta, una vuelta del lector como la hace el hilo, el índice en su archivo, y la
    /// búsqueda por donde la pide la ventana (`filtrar` sobre lo que da el historial).
    ///
    /// Va con `--ignored` como las demás del lector: necesita su paquete de idioma.
    /// `cargo test --lib el_texto_de_las_capturas_se_encuentra -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore = "necesita el lector de texto de Windows instalado"]
    fn el_texto_de_las_capturas_se_encuentra() {
        use crate::platform::ocr::pruebas::pintar_textos;
        let dir = carpeta_de_prueba("de-verdad");
        let archivo = dir.join("indice").join(ARCHIVO);
        std::fs::create_dir_all(archivo.parent().unwrap()).unwrap();
        let fotos = [
            ("winshotx-camion.png", vec![(24, "El Camión de reparto"), (120, "llega el martes")]),
            ("winshotx-factura.png", vec![(24, "Factura de la luz"), (120, "Total 54,20 euros")]),
            ("winshotx-blanca.png", vec![]),
        ];
        for (i, (nombre, lineas)) in fotos.iter().enumerate() {
            let ruta = dir.join(nombre);
            std::fs::write(&ruta, pintar_textos(lineas, 900, 240)).unwrap();
            let cuando = std::time::SystemTime::now() - Duration::from_secs(10 * (i as u64 + 1));
            std::fs::File::options().write(true).open(&ruta).unwrap().set_modified(cuando).unwrap();
        }
        // Una captura con scroll más alta de lo que acepta el lector, con una palabra arriba,
        // otra justo en el corte entre las dos primeras tiras y otra al final.
        let lector = crate::platform::ocr::Lector::nuevo().expect("el lector de Windows");
        let maximo = windows::Media::Ocr::OcrEngine::MaxImageDimension().unwrap() as i32;
        let alto = maximo + 3000;
        let alta = dir.join("winshotx-scroll.png");
        let en_el_corte = maximo - 60;
        std::fs::write(
            &alta,
            pintar_textos(&[(24, "Arriba del todo"), (en_el_corte, "Justo en el corte"), (alto - 120, "Abajo del todo")], 700, alto),
        )
        .unwrap();
        println!("el lector acepta {maximo} px de lado; la alta mide {alto}");

        let mut indice = Indice::default();
        let cola = mirar_la_carpeta(&mut indice, &dir);
        assert_eq!(cola.len(), 4);
        for entrada in &cola {
            let empezo = Instant::now();
            let texto = lector.leer_archivo(Path::new(&entrada.ruta)).unwrap();
            println!("{} ({} ms): {texto:?}", entrada.nombre, empezo.elapsed().as_millis());
            indice.apuntar(Some(&archivo), entrada, texto).unwrap();
        }
        assert!(mirar_la_carpeta(&mut indice, &dir).is_empty(), "ya no falta ninguna");

        // Lo que hace `historial_lista` con la caja escrita, con el índice recién cargado
        // del archivo, como al día siguiente.
        let guardado = Indice::cargar(&archivo);
        let buscar = |busqueda: &str| {
            let mut todas = crate::historial::leer(&dir, None);
            filtrar(&guardado, &mut todas, busqueda);
            todas.into_iter().map(|e| e.nombre).collect::<Vec<_>>()
        };
        assert_eq!(buscar("camion"), ["winshotx-camion.png"]);
        assert_eq!(buscar("REPARTO martes"), ["winshotx-camion.png"]);
        assert_eq!(buscar("factura"), ["winshotx-factura.png"]);
        assert_eq!(buscar("arriba"), ["winshotx-scroll.png"]);
        assert_eq!(buscar("corte"), ["winshotx-scroll.png"], "la línea del corte sale entera en una tira");
        assert_eq!(buscar("abajo"), ["winshotx-scroll.png"]);
        assert!(buscar("hipopotamo").is_empty());
        assert_eq!(buscar("").len(), 4, "sin texto, el historial entero");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
