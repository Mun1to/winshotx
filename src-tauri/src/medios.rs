//! Un servidor HTTP minimo en `127.0.0.1`, para que el editor de Linux reproduzca video y sonido.
//!
//! En Windows el editor lee los archivos de la sesion por el protocolo `asset:` de Tauri. En
//! Linux las imagenes tambien, pero el video y el sonido no: en WebKitGTK los reproduce
//! GStreamer, que no sabe leer un protocolo que solo existe dentro de la app, y un `<video>` o
//! un `<audio>` con `asset:` se queda sin abrir. HTTP si lo sabe leer.
//!
//! **Lo que deja leer, y a quien.** Solo escucha en `127.0.0.1`, asi que no sale del equipo.
//! Pero en el equipo hay mas programas y mas paginas web que la nuestra, y por eso:
//! - cada arranque inventa una **ficha** que va delante de todas las rutas; sin ella, 404;
//! - solo sirve **archivos de dos carpetas**, la de las grabaciones (`sessions`, en la carpeta
//!   de paso de winshotx) y la de los fondos animados que importa la persona, mirando la ruta
//!   de verdad (resueltos los enlaces y los `..`). Es menos de lo que deja leer `asset:`;
//! - solo sirve **video y sonido**, por la extension: el `session.json` o un fotograma de esa
//!   misma carpeta contestan 404, como lo que no esta;
//! - solo contesta si la peticion va dirigida a `127.0.0.1` o a `localhost`: una pagina de
//!   fuera que haga que su nombre apunte a esta maquina (DNS rebinding) no pasa;
//! - solo `GET` y `HEAD`, y sin las cabeceras que dejarian a otra web leer la respuesta.
//!
//! **Trozos.** Un reproductor no baja el archivo entero: pide trozos (`Range`) para saber lo
//! que dura y para saltar. Se contestan con 206, y con 416 si piden mas alla del final.
//!
//! **Cuanto vive.** Lo que vive la app. Se arranca con el primer editor que se abre y no se
//! para al cerrarlo: el editor siguiente usa el mismo puerto y la misma ficha, y al salir de
//! winshotx el hilo muere con el proceso. Mientras nadie pide nada es un hilo dormido en
//! `accept`, que no gasta. La ficha dura eso mismo: una por arranque de winshotx.
//!
//! **Lo que aguanta.** Quien no tiene la ficha solo puede abrir conexiones: hay un plazo para
//! que llegue la peticion entera (no por lectura), un tope de conexiones a la vez, y una pausa
//! si aceptar falla, para que ni treinta y dos conexiones lentas ni un proceso sin
//! descriptores dejen al editor sin video o a un nucleo girando.
//!
//! Todo es de la biblioteca estandar: un hilo que acepta y un hilo por peticion, que son
//! pocas y cortas. No entra ningun crate. Solo se arranca en Linux; el resto compila en todas
//! partes para que sus pruebas corran tambien en Windows.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// Lo que se espera, EN TOTAL, a que llegue la peticion entera: quien abre y no dice nada, o
/// lo dice letra a letra, no se queda con un hilo. Es un plazo para todas las lecturas juntas
/// y no para cada una: con uno por lectura, treinta y dos conexiones que mandan una letra
/// cada pocos segundos ocupaban todos los hilos y el editor se quedaba sin video.
///
/// Dos segundos y no cinco: quien pide de verdad es el reproductor de esta misma maquina, que
/// manda su peticion entera al conectar. Con cinco, otro usuario del equipo que reabriese
/// conexiones vacias tenia casi siempre llenos los treinta y dos huecos.
const ESPERA_AL_LEER: Duration = Duration::from_secs(2);
/// Lo que se descansa cuando aceptar una conexion falla. Si al proceso no le quedan
/// descriptores, `accept` falla al instante una vez tras otra: sin pausa, el hilo giraria al
/// 100 % de un nucleo hasta que se liberase alguno.
const PAUSA_SI_ACEPTAR_FALLA: Duration = Duration::from_millis(100);
const ESPERA_AL_ESCRIBIR: Duration = Duration::from_secs(30);
const CABECERAS_COMO_MUCHO: usize = 8 * 1024;
const TROZO: usize = 64 * 1024;
/// Peticiones a la vez. Un editor abre unas pocas (el video, dos pistas, la camara).
const A_LA_VEZ: usize = 32;
/// Cuantas peticiones se apuntan en el registro: las primeras dicen si el editor esta leyendo
/// o no, y las demas (una por cada salto en el video) solo lo llenarian.
const SE_APUNTAN: usize = 24;

/// Las que se llevan apuntadas.
static APUNTADAS: AtomicUsize = AtomicUsize::new(0);

/// Apunta una peticion en el registro de la app: el codigo, el nombre del archivo (sin su
/// carpeta, y nunca la ficha) y el trozo pedido.
fn apuntar(codigo: u16, archivo: Option<&Path>, rango: Option<&str>) {
    if APUNTADAS.fetch_add(1, Ordering::Relaxed) >= SE_APUNTAN {
        return;
    }
    let nombre = archivo.and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned());
    eprintln!("[medios] {codigo} {} {}", nombre.as_deref().unwrap_or("-"), rango.unwrap_or("entero"));
}

/// Por donde se le pide (`http://127.0.0.1:PUERTO/FICHA`), o `None` si no ha podido arrancar.
static BASE: OnceLock<Option<String>> = OnceLock::new();

/// Las carpetas de las que se deja leer: la de las grabaciones, dentro de la carpeta de paso,
/// y la de los fondos animados que importa la persona, si se ha podido saber cual es. Lo demas
/// de la carpeta de paso (fotos fijadas, historial, miniaturas) son imagenes, que el editor
/// lee por `asset:` y no pasan por aqui.
pub fn raices(carpeta_de_paso: &Path, fondos: Option<PathBuf>) -> Vec<PathBuf> {
    std::iter::once(carpeta_de_paso.join("sessions")).chain(fondos).collect()
}

/// Quita la ficha de un texto que va al registro de la app. El mensaje de error de un
/// `<audio>` no lo escribe winshotx sino GStreamer, y puede traer la direccion entera que no
/// ha podido abrir.
pub fn sin_ficha(texto: &str) -> String {
    match BASE.get().and_then(|base| base.as_deref()).and_then(|base| base.rsplit('/').next()) {
        Some(ficha) => quitar(texto, ficha),
        None => texto.to_string(),
    }
}

fn quitar(texto: &str, ficha: &str) -> String {
    if ficha.is_empty() {
        return texto.to_string();
    }
    texto.replace(ficha, "(ficha)")
}

/// Arranca el servidor la primera vez y devuelve por donde se le pide. `raices` son las
/// carpetas de las que deja leer (ver [`raices`]).
pub fn base(raices: &[PathBuf]) -> Option<String> {
    BASE.get_or_init(|| match arrancar(raices) {
        Ok((base, puerto)) => {
            // La ficha no se escribe: quien la tenga lee las grabaciones de esta sesion.
            eprintln!("[medios] el editor lee el vídeo y el sonido por 127.0.0.1:{puerto}");
            Some(base)
        }
        Err(error) => {
            eprintln!("[medios] no se ha podido abrir el servidor del editor: {error}");
            None
        }
    })
    .clone()
}

fn arrancar(raices: &[PathBuf]) -> std::io::Result<(String, u16)> {
    // Con su ruta de verdad, para compararlas con la ruta de verdad de lo que se pida. Una
    // carpeta que no existe o no se puede leer se queda fuera: no se sirve nada de ella.
    let raices: Vec<PathBuf> = raices.iter().filter_map(|raiz| raiz.canonicalize().ok()).collect();
    if raices.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no hay ninguna carpeta de la que leer"));
    }
    let escucha = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    let puerto = escucha.local_addr()?.port();
    let ficha = uuid::Uuid::new_v4().simple().to_string();
    let servidor = Arc::new(Servidor { raices, puerto, ficha: ficha.clone() });
    // El hilo no se guarda ni se para: vive lo que vive la app (ver arriba, «Cuanto vive»).
    std::thread::Builder::new()
        .name("medios".into())
        .spawn(move || aceptar(escucha.incoming(), &servidor, PAUSA_SI_ACEPTAR_FALLA))?;
    Ok((format!("http://127.0.0.1:{puerto}/{ficha}"), puerto))
}

/// Atiende cada conexion que llega en un hilo suyo, hasta `A_LA_VEZ` a la vez. Si aceptar
/// falla, descansa `pausa` antes de volver a intentarlo.
fn aceptar(conexiones: impl Iterator<Item = std::io::Result<TcpStream>>, servidor: &Arc<Servidor>, pausa: Duration) {
    let abiertas = Arc::new(AtomicUsize::new(0));
    for conexion in conexiones {
        let Ok(conexion) = conexion else {
            std::thread::sleep(pausa);
            continue;
        };
        if abiertas.fetch_add(1, Ordering::SeqCst) >= A_LA_VEZ {
            abiertas.fetch_sub(1, Ordering::SeqCst);
            continue;
        }
        let (servidor, abiertas_de_esta) = (servidor.clone(), abiertas.clone());
        let lanzado = std::thread::Builder::new().name("medios-peticion".into()).spawn(move || {
            servidor.atender(conexion);
            abiertas_de_esta.fetch_sub(1, Ordering::SeqCst);
        });
        if lanzado.is_err() {
            abiertas.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

struct Servidor {
    /// Las carpetas de las que se deja leer, con su ruta de verdad.
    raices: Vec<PathBuf>,
    puerto: u16,
    ficha: String,
}

/// Lo que importa de una peticion.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Peticion {
    /// `HEAD`: solo se quieren las cabeceras.
    pub solo_cabeceras: bool,
    /// Lo que va detras del metodo, tal cual: `/FICHA/ruta%20codificada`.
    pub destino: String,
    pub host: Option<String>,
    pub rango: Option<String>,
}

/// La primera linea y las cabeceras que importan. `Err` con el codigo de HTTP con que se contesta.
pub(crate) fn leer_peticion(texto: &str) -> Result<Peticion, u16> {
    let mut lineas = texto.split("\r\n");
    let mut partes = lineas.next().unwrap_or_default().split(' ');
    let (Some(metodo), Some(destino), Some(version), None) =
        (partes.next(), partes.next(), partes.next(), partes.next())
    else {
        return Err(400);
    };
    if !version.starts_with("HTTP/1.") || !destino.starts_with('/') {
        return Err(400);
    }
    let solo_cabeceras = match metodo {
        "GET" => false,
        "HEAD" => true,
        _ => return Err(405),
    };
    let (mut host, mut rango) = (None, None);
    for linea in lineas {
        let Some((nombre, valor)) = linea.split_once(':') else {
            continue;
        };
        match nombre.trim().to_ascii_lowercase().as_str() {
            "host" => host = Some(valor.trim().to_string()),
            "range" => rango = Some(valor.trim().to_string()),
            _ => {}
        }
    }
    Ok(Peticion { solo_cabeceras, destino: destino.to_string(), host, rango })
}

/// Si la peticion va dirigida a este servidor por su nombre de aqui. Una pagina de fuera que
/// haga que `su-nombre.com` apunte a `127.0.0.1` llega con su nombre en `Host`, y no pasa.
pub(crate) fn host_propio(host: Option<&str>, puerto: u16) -> bool {
    let Some(host) = host else {
        return false;
    };
    host == format!("127.0.0.1:{puerto}") || host.eq_ignore_ascii_case(&format!("localhost:{puerto}"))
}

/// De `/FICHA/ruta%20codificada?lo-que-sea` saca la ruta del archivo, si la ficha es la buena
/// y la ruta es entera (no relativa a nada).
pub(crate) fn ruta_pedida(destino: &str, ficha: &str) -> Option<PathBuf> {
    let sin_consulta = destino.split(['?', '#']).next()?;
    let (suya, codificada) = sin_consulta.strip_prefix('/')?.split_once('/')?;
    if !iguales(suya.as_bytes(), ficha.as_bytes()) {
        return None;
    }
    let ruta = PathBuf::from(decodificar(codificada)?);
    ruta.is_absolute().then_some(ruta)
}

/// Compara sin salir antes al primer byte distinto, para que lo que tarda no diga cuanto se
/// ha acertado de la ficha.
fn iguales(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |distinto, (x, y)| distinto | (x ^ y)) == 0
}

/// Deshace los `%20` de una ruta de URL. `None` si esta mal escrita, si no es texto o si lleva
/// un byte nulo (que cortaria la ruta al abrir el archivo).
pub(crate) fn decodificar(texto: &str) -> Option<String> {
    let bytes = texto.as_bytes();
    let mut salida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let par = bytes.get(i + 1..i + 3)?;
            if !par.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            salida.push(u8::from_str_radix(std::str::from_utf8(par).ok()?, 16).ok()?);
            i += 3;
        } else {
            salida.push(bytes[i]);
            i += 1;
        }
    }
    let texto = String::from_utf8(salida).ok()?;
    (!texto.contains('\0')).then_some(texto)
}

/// La ruta de verdad del archivo (resueltos los enlaces y los `..`), si es un archivo y esta
/// dentro de alguna de las `raices`, que tienen que venir ya con su ruta de verdad.
pub(crate) fn dentro(raices: &[PathBuf], pedida: &Path) -> Option<PathBuf> {
    let real = pedida.canonicalize().ok()?;
    (raices.iter().any(|raiz| real.starts_with(raiz)) && real.is_file()).then_some(real)
}

/// Que trozo se pide de un archivo de `total` bytes: `Ok(None)` si va entero,
/// `Ok(Some((primero, ultimo)))` si es un trozo, y `Err(())` si empieza mas alla del final
/// (eso es un 416).
///
/// Solo se entiende un trozo en bytes. Lo que no se entiende (varios trozos, otra unidad) se
/// contesta con el archivo entero, que es lo que manda el estandar.
pub(crate) fn rango(cabecera: Option<&str>, total: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(pedido) = cabecera.and_then(|c| c.trim().strip_prefix("bytes=")) else {
        return Ok(None);
    };
    if pedido.contains(',') {
        return Ok(None);
    }
    let Some((desde, hasta)) = pedido.split_once('-') else {
        return Ok(None);
    };
    let (desde, hasta) = (desde.trim(), hasta.trim());
    if desde.is_empty() {
        // `bytes=-500`: los ultimos quinientos.
        let Ok(ultimos) = hasta.parse::<u64>() else {
            return Ok(None);
        };
        if ultimos == 0 || total == 0 {
            return Err(());
        }
        return Ok(Some((total.saturating_sub(ultimos), total - 1)));
    }
    let Ok(primero) = desde.parse::<u64>() else {
        return Ok(None);
    };
    if primero >= total {
        return Err(());
    }
    let ultimo = if hasta.is_empty() {
        total - 1
    } else {
        match hasta.parse::<u64>() {
            Ok(ultimo) if ultimo >= primero => ultimo.min(total - 1),
            _ => return Ok(None),
        }
    };
    Ok(Some((primero, ultimo)))
}

/// El tipo que se le dice al reproductor, por la extension, o `None` si no es video ni
/// sonido: eso no se sirve. En la carpeta de una grabacion hay mas cosas (el `session.json`,
/// los fotogramas) y nada de eso se reproduce.
pub(crate) fn tipo(ruta: &Path) -> Option<&'static str> {
    let extension = ruta.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("mp4" | "m4v") => Some("video/mp4"),
        Some("webm") => Some("video/webm"),
        Some("wav") => Some("audio/wav"),
        Some("m4a") => Some("audio/mp4"),
        Some("mp3") => Some("audio/mpeg"),
        Some("ogg") => Some("audio/ogg"),
        _ => None,
    }
}

fn frase(codigo: u16) -> &'static str {
    match codigo {
        200 => "OK",
        206 => "Partial Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        _ => "Error",
    }
}

/// Una respuesta sin nada dentro: todos los «no».
fn sin_cuerpo(salida: &mut impl Write, codigo: u16, total: Option<u64>) -> std::io::Result<()> {
    let mut cabeza = format!("HTTP/1.1 {codigo} {}\r\nContent-Length: 0\r\nConnection: close\r\n", frase(codigo));
    if let Some(total) = total {
        cabeza.push_str(&format!("Content-Range: bytes */{total}\r\n"));
    }
    if codigo == 405 {
        cabeza.push_str("Allow: GET, HEAD\r\n");
    }
    cabeza.push_str("\r\n");
    salida.write_all(cabeza.as_bytes())?;
    salida.flush()
}

/// Lee hasta la linea en blanco que cierra las cabeceras. `None` si no llega entera antes de
/// `plazo` (contado desde ahora, para todas las lecturas juntas), si se corta o si es mas
/// larga de lo que cabe esperar.
fn leer_cabeceras(conexion: &mut TcpStream, plazo: Duration) -> Option<String> {
    let limite = Instant::now() + plazo;
    let mut leido = Vec::with_capacity(1024);
    let mut trozo = [0u8; 1024];
    loop {
        // A cada lectura se le da lo que queda del plazo, no el plazo entero otra vez.
        let queda = limite.checked_duration_since(Instant::now()).filter(|queda| !queda.is_zero())?;
        conexion.set_read_timeout(Some(queda)).ok()?;
        let cuantos = conexion.read(&mut trozo).ok()?;
        if cuantos == 0 {
            return None;
        }
        leido.extend_from_slice(&trozo[..cuantos]);
        if let Some(fin) = leido.windows(4).position(|ventana| ventana == b"\r\n\r\n") {
            leido.truncate(fin);
            break;
        }
        if leido.len() > CABECERAS_COMO_MUCHO {
            return None;
        }
    }
    String::from_utf8(leido).ok()
}

impl Servidor {
    fn atender(&self, mut conexion: TcpStream) {
        let _ = conexion.set_write_timeout(Some(ESPERA_AL_ESCRIBIR));
        if let Some(texto) = leer_cabeceras(&mut conexion, ESPERA_AL_LEER) {
            // Si el reproductor corta a medias (salta a otro sitio del video), no pasa nada.
            let _ = self.contestar(&texto, &mut conexion);
        }
        let _ = conexion.shutdown(Shutdown::Both);
    }

    fn contestar(&self, texto: &str, salida: &mut impl Write) -> std::io::Result<()> {
        let peticion = match leer_peticion(texto) {
            Ok(peticion) => peticion,
            Err(codigo) => return sin_cuerpo(salida, codigo, None),
        };
        if !host_propio(peticion.host.as_deref(), self.puerto) {
            apuntar(403, None, None);
            return sin_cuerpo(salida, 403, None);
        }
        // La ficha mala, la ruta de fuera y el archivo que no esta contestan lo mismo: no se
        // le dice a nadie cual de las tres ha sido.
        let Some(real) = ruta_pedida(&peticion.destino, &self.ficha).and_then(|ruta| dentro(&self.raices, &ruta)) else {
            apuntar(404, None, None);
            return sin_cuerpo(salida, 404, None);
        };
        // Lo que no es video ni sonido contesta igual que lo que no esta.
        let Some(tipo) = tipo(&real) else {
            apuntar(404, None, None);
            return sin_cuerpo(salida, 404, None);
        };
        let Ok(mut archivo) = std::fs::File::open(&real) else {
            apuntar(404, Some(&real), None);
            return sin_cuerpo(salida, 404, None);
        };
        let total = archivo.metadata()?.len();
        let (codigo, primero, cuanto) = match rango(peticion.rango.as_deref(), total) {
            Ok(None) => (200, 0, total),
            Ok(Some((primero, ultimo))) => (206, primero, ultimo - primero + 1),
            Err(()) => {
                apuntar(416, Some(&real), peticion.rango.as_deref());
                return sin_cuerpo(salida, 416, Some(total));
            }
        };
        apuntar(codigo, Some(&real), peticion.rango.as_deref());
        let mut cabeza = format!(
            "HTTP/1.1 {codigo} {}\r\nContent-Type: {}\r\nContent-Length: {cuanto}\r\nAccept-Ranges: bytes\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n",
            frase(codigo),
            tipo
        );
        if codigo == 206 {
            cabeza.push_str(&format!("Content-Range: bytes {primero}-{}/{total}\r\n", primero + cuanto - 1));
        }
        cabeza.push_str("\r\n");
        salida.write_all(cabeza.as_bytes())?;
        if peticion.solo_cabeceras || cuanto == 0 {
            return salida.flush();
        }
        archivo.seek(SeekFrom::Start(primero))?;
        let mut quedan = cuanto;
        let mut trozo = vec![0u8; TROZO];
        while quedan > 0 {
            let ahora = quedan.min(TROZO as u64) as usize;
            let leidos = archivo.read(&mut trozo[..ahora])?;
            if leidos == 0 {
                break;
            }
            salida.write_all(&trozo[..leidos])?;
            quedan -= leidos as u64;
        }
        salida.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn de_una_peticion_se_saca_el_metodo_el_destino_y_las_dos_cabeceras_que_importan() {
        let peticion = leer_peticion("GET /abc/%2Ftmp%2Fa.wav HTTP/1.1\r\nHost: 127.0.0.1:4000\r\nrange: bytes=0-9\r\nAccept: */*")
            .expect("peticion");
        assert_eq!(
            peticion,
            Peticion {
                solo_cabeceras: false,
                destino: "/abc/%2Ftmp%2Fa.wav".into(),
                host: Some("127.0.0.1:4000".into()),
                rango: Some("bytes=0-9".into()),
            }
        );
        assert!(leer_peticion("HEAD /x HTTP/1.1\r\nHost: a").expect("head").solo_cabeceras);
    }

    #[test]
    fn solo_se_puede_leer_no_escribir_ni_borrar() {
        for metodo in ["POST", "PUT", "DELETE", "OPTIONS", "PATCH"] {
            assert_eq!(leer_peticion(&format!("{metodo} /x HTTP/1.1\r\nHost: a")), Err(405), "{metodo}");
        }
        assert_eq!(leer_peticion("hola"), Err(400));
        assert_eq!(leer_peticion("GET /x y HTTP/1.1"), Err(400));
        assert_eq!(leer_peticion("GET http://otro/x HTTP/1.1"), Err(400));
        assert_eq!(leer_peticion("GET /x SPDY/3"), Err(400));
    }

    #[test]
    fn una_pagina_de_fuera_con_su_nombre_apuntando_aqui_no_pasa() {
        assert!(host_propio(Some("127.0.0.1:4000"), 4000));
        assert!(host_propio(Some("LocalHost:4000"), 4000));
        assert!(!host_propio(Some("malo.example.com:4000"), 4000));
        assert!(!host_propio(Some("127.0.0.1:4001"), 4000));
        assert!(!host_propio(Some("127.0.0.1"), 4000));
        assert!(!host_propio(None, 4000));
    }

    #[test]
    fn sin_la_ficha_no_hay_ruta() {
        let absoluta = std::env::temp_dir().join("a b.wav");
        let codificada: String = absoluta
            .to_string_lossy()
            .bytes()
            .map(|b| if b.is_ascii_alphanumeric() || b == b'.' { (b as char).to_string() } else { format!("%{b:02X}") })
            .collect();
        assert_eq!(ruta_pedida(&format!("/buena/{codificada}"), "buena"), Some(absoluta.clone()));
        assert_eq!(ruta_pedida(&format!("/buena/{codificada}?t=1#x"), "buena"), Some(absoluta));
        assert_eq!(ruta_pedida(&format!("/mala0/{codificada}"), "buena"), None);
        assert_eq!(ruta_pedida(&format!("/buen/{codificada}"), "buena"), None);
        assert_eq!(ruta_pedida(&format!("/{codificada}"), "buena"), None);
        // Una ruta que no es entera se leeria desde donde este la app: no.
        assert_eq!(ruta_pedida("/buena/a.wav", "buena"), None);
        assert_eq!(ruta_pedida("/buena/..%2F..%2Fa.wav", "buena"), None);
    }

    #[test]
    fn decodificar_no_se_traga_lo_mal_escrito() {
        assert_eq!(decodificar("a%20b%C3%B1").as_deref(), Some("a bñ"));
        assert_eq!(decodificar("sin-nada").as_deref(), Some("sin-nada"));
        assert_eq!(decodificar("%"), None);
        assert_eq!(decodificar("%2"), None);
        assert_eq!(decodificar("%zz"), None);
        assert_eq!(decodificar("%+f"), None);
        assert_eq!(decodificar("%FF"), None, "no es texto");
        assert_eq!(decodificar("a%00b"), None, "un nulo cortaria la ruta");
    }

    #[test]
    fn los_trozos_que_pide_un_reproductor() {
        assert_eq!(rango(None, 1000), Ok(None));
        assert_eq!(rango(Some("bytes=0-"), 1000), Ok(Some((0, 999))));
        assert_eq!(rango(Some("bytes=10-19"), 1000), Ok(Some((10, 19))));
        assert_eq!(rango(Some("bytes=990-2000"), 1000), Ok(Some((990, 999))), "se recorta al final");
        assert_eq!(rango(Some("bytes=-100"), 1000), Ok(Some((900, 999))));
        assert_eq!(rango(Some("bytes=-5000"), 1000), Ok(Some((0, 999))));
        // Mas alla del final: 416.
        assert_eq!(rango(Some("bytes=1000-"), 1000), Err(()));
        assert_eq!(rango(Some("bytes=-0"), 1000), Err(()));
        assert_eq!(rango(Some("bytes=0-"), 0), Err(()));
        // Lo que no se entiende, entero.
        assert_eq!(rango(Some("bytes=0-9,20-29"), 1000), Ok(None));
        assert_eq!(rango(Some("bytes=9-3"), 1000), Ok(None));
        assert_eq!(rango(Some("paginas=1-2"), 1000), Ok(None));
        assert_eq!(rango(Some("bytes=a-b"), 1000), Ok(None));
    }

    #[test]
    fn el_tipo_sale_de_la_extension_y_solo_hay_video_y_sonido() {
        assert_eq!(tipo(Path::new("/x/vista.MP4")), Some("video/mp4"));
        assert_eq!(tipo(Path::new("/x/sonido-pc.wav")), Some("audio/wav"));
        assert_eq!(tipo(Path::new("/x/session.json")), None);
        assert_eq!(tipo(Path::new("/x/000001.png")), None);
        assert_eq!(tipo(Path::new("/x/sin-extension")), None);
    }

    #[test]
    fn la_ficha_no_llega_al_registro_aunque_venga_en_un_mensaje_de_error() {
        let ficha = "0123456789abcdef0123456789abcdef";
        let de_gstreamer = format!("pc: Could not open resource for reading: http://127.0.0.1:4000/{ficha}/%2Ftmp%2Fa.wav");
        let apuntado = quitar(&de_gstreamer, ficha);
        assert!(!apuntado.contains(ficha), "{apuntado}");
        assert!(apuntado.contains("Could not open resource") && apuntado.contains("a.wav"), "{apuntado}");
        assert_eq!(quitar("pc: no se puede abrir", ficha), "pc: no se puede abrir");
    }

    /// Una carpeta de paso y una de fondos de mentira: una pista de sonido en una grabacion,
    /// un fondo animado, y lo que no se sirve (un archivo de fuera, uno de otra carpeta de la
    /// de paso, y el `session.json` de la grabacion).
    struct Carpetas {
        base: PathBuf,
        /// La carpeta de paso entera.
        paso: PathBuf,
        /// La de los fondos, que vive en otra parte del disco.
        fondos: PathBuf,
        /// Las grabaciones, con su ruta de verdad.
        raiz: PathBuf,
        dentro: PathBuf,
        fuera: PathBuf,
        fondo: PathBuf,
        fijada: PathBuf,
        ficha_de_la_sesion: PathBuf,
    }

    impl Carpetas {
        fn nuevas(nombre: &str) -> Self {
            let base = std::env::temp_dir().join(format!("winshotx-medios-{nombre}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            std::fs::create_dir_all(base.join("paso").join("sessions")).expect("carpeta");
            let dentro = base.join("paso").join("sessions").join("sonido pc.wav");
            let fuera = base.join("secreto.wav");
            let contenido: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
            std::fs::write(&dentro, &contenido).expect("archivo de dentro");
            std::fs::write(&fuera, b"no se toca").expect("archivo de fuera");
            let paso = base.join("paso");
            let fondos = base.join("datos").join("fondos");
            std::fs::create_dir_all(&fondos).expect("carpeta de fondos");
            std::fs::create_dir_all(paso.join("pins")).expect("carpeta de fijadas");
            let fondo = fondos.join("video-0123.mp4");
            let fijada = paso.join("pins").join("otra cosa.mp4");
            let ficha_de_la_sesion = paso.join("sessions").join("session.json");
            std::fs::write(&fondo, b"un fondo animado").expect("fondo");
            std::fs::write(&fijada, b"no es una grabacion").expect("fijada");
            std::fs::write(&ficha_de_la_sesion, b"{}").expect("session.json");
            let raiz = paso.join("sessions").canonicalize().expect("raiz");
            Self { base, paso, fondos, raiz, dentro, fuera, fondo, fijada, ficha_de_la_sesion }
        }
    }

    impl Drop for Carpetas {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn solo_se_leen_archivos_de_las_carpetas_que_se_sirven() {
        let c = Carpetas::nuevas("dentro");
        let raices = [c.raiz.clone()];
        assert!(dentro(&raices, &c.dentro).is_some());
        assert!(dentro(&raices, &c.fuera).is_none(), "un archivo de fuera");
        assert!(dentro(&raices, &c.fondo).is_none(), "los fondos, solo si su carpeta esta entre las que se sirven");
        // Entrar y volver a salir con `..`: la ruta de verdad esta fuera.
        let con_vuelta = c.raiz.join("..").join("..").join("secreto.wav");
        assert!(con_vuelta.exists());
        assert!(dentro(&raices, &con_vuelta).is_none(), "salir con ..");
        assert!(dentro(&raices, &c.raiz).is_none(), "una carpeta no es un archivo");
        assert!(dentro(&raices, &c.raiz.join("no-esta.wav")).is_none());
        let con_fondos = [c.raiz.clone(), c.fondos.canonicalize().expect("fondos")];
        assert!(dentro(&con_fondos, &c.fondo).is_some());
        assert!(dentro(&con_fondos, &c.dentro).is_some());
        assert!(dentro(&con_fondos, &c.fijada).is_none());
    }

    #[test]
    fn se_sirven_las_grabaciones_y_los_fondos_y_nada_mas_de_la_carpeta_de_paso() {
        let c = Carpetas::nuevas("raices");
        let (base, puerto) = arrancar(&raices(&c.paso, Some(c.fondos.clone()))).expect("servidor");
        let ficha = base.rsplit('/').next().expect("ficha").to_string();
        let pedir_archivo = |ruta: &Path| {
            let (cabeceras, cuerpo) =
                pedir(puerto, &format!("GET /{ficha}/{} HTTP/1.1\r\nHost: 127.0.0.1:{puerto}\r\n\r\n", codificar(ruta)));
            (cabeceras.split("\r\n").next().unwrap_or_default().to_string(), cabeceras, cuerpo)
        };
        let (estado, _, _) = pedir_archivo(&c.dentro);
        assert_eq!(estado, "http/1.1 200 ok", "una pista de la grabacion");
        // Un fondo animado importado vive en la carpeta de datos, no en la de paso.
        let (estado, cabeceras, cuerpo) = pedir_archivo(&c.fondo);
        assert_eq!(estado, "http/1.1 200 ok", "un fondo animado");
        assert!(cabeceras.contains("content-type: video/mp4"), "{cabeceras}");
        assert_eq!(cuerpo, b"un fondo animado");
        // Lo demas de la carpeta de paso no es de este servidor, aunque sea un video.
        let (estado, _, cuerpo) = pedir_archivo(&c.fijada);
        assert_eq!(estado, "http/1.1 404 not found", "otra carpeta de la de paso");
        assert!(cuerpo.is_empty());
        // Y de una grabacion, solo lo que se reproduce.
        let (estado, cabeceras, cuerpo) = pedir_archivo(&c.ficha_de_la_sesion);
        assert_eq!(estado, "http/1.1 404 not found", "el session.json: {cabeceras}");
        assert!(cuerpo.is_empty());
    }

    #[test]
    fn sin_ninguna_carpeta_que_exista_el_servidor_no_arranca() {
        let c = Carpetas::nuevas("ninguna");
        assert!(arrancar(&[c.base.join("no-esta")]).is_err());
        assert!(arrancar(&[]).is_err());
        // Con una que no esta y otra que si, se sirve la que si.
        assert!(arrancar(&[c.base.join("no-esta"), c.raiz.clone()]).is_ok());
    }

    #[test]
    fn quien_manda_la_peticion_a_gotas_no_se_queda_con_el_hilo() {
        let escucha = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).expect("escucha");
        let puerto = escucha.local_addr().expect("puerto").port();
        // Una letra cada 100 ms durante tres segundos: cada lectura llega a tiempo, la
        // peticion entera no llega nunca.
        let gotas = std::thread::spawn(move || {
            let mut conexion = TcpStream::connect(("127.0.0.1", puerto)).expect("conectar");
            for _ in 0..30 {
                if conexion.write_all(b"G").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        let (mut conexion, _) = escucha.accept().expect("aceptar");
        let empieza = Instant::now();
        assert_eq!(leer_cabeceras(&mut conexion, Duration::from_millis(500)), None);
        let tardo = empieza.elapsed();
        assert!(tardo >= Duration::from_millis(400), "se ha rendido antes del plazo: {tardo:?}");
        assert!(tardo < Duration::from_millis(1500), "el plazo es para la peticion entera, no para cada letra: {tardo:?}");
        drop(conexion);
        gotas.join().expect("el de las gotas");
    }

    #[test]
    fn una_peticion_que_llega_entera_a_tiempo_se_lee_aunque_venga_en_dos_veces() {
        let escucha = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).expect("escucha");
        let puerto = escucha.local_addr().expect("puerto").port();
        let cliente = std::thread::spawn(move || {
            let mut conexion = TcpStream::connect(("127.0.0.1", puerto)).expect("conectar");
            conexion.write_all(b"GET /x HTTP/1.1\r\nHo").expect("primera mitad");
            std::thread::sleep(Duration::from_millis(150));
            conexion.write_all(b"st: a\r\n\r\n").expect("segunda mitad");
            std::thread::sleep(Duration::from_millis(200));
        });
        let (mut conexion, _) = escucha.accept().expect("aceptar");
        assert_eq!(leer_cabeceras(&mut conexion, Duration::from_secs(5)).as_deref(), Some("GET /x HTTP/1.1\r\nHost: a"));
        cliente.join().expect("cliente");
    }

    #[test]
    fn si_aceptar_falla_se_descansa_en_vez_de_girar() {
        let c = Carpetas::nuevas("aceptar");
        let servidor = Arc::new(Servidor { raices: vec![c.raiz.clone()], puerto: 1, ficha: "f".into() });
        // Lo que pasa cuando al proceso no le quedan descriptores: `accept` falla una y otra vez.
        let fallos = (0..4).map(|_| Err(std::io::Error::new(std::io::ErrorKind::Other, "sin descriptores")));
        let empieza = Instant::now();
        aceptar(fallos, &servidor, Duration::from_millis(50));
        let tardo = empieza.elapsed();
        assert!(tardo >= Duration::from_millis(200), "cuatro fallos sin descansar: {tardo:?}");
    }

    fn codificar(ruta: &Path) -> String {
        ruta.to_string_lossy()
            .bytes()
            .map(|b| if b.is_ascii_alphanumeric() || b == b'.' || b == b'-' { (b as char).to_string() } else { format!("%{b:02X}") })
            .collect()
    }

    /// Pide por la red de verdad y devuelve las cabeceras (en minusculas) y el cuerpo.
    fn pedir(puerto: u16, peticion: &str) -> (String, Vec<u8>) {
        let mut conexion = TcpStream::connect(("127.0.0.1", puerto)).expect("conectar");
        conexion.set_read_timeout(Some(Duration::from_secs(10))).expect("espera");
        conexion.write_all(peticion.as_bytes()).expect("pedir");
        let mut todo = Vec::new();
        conexion.read_to_end(&mut todo).expect("leer");
        let corte = todo.windows(4).position(|v| v == b"\r\n\r\n").expect("fin de las cabeceras");
        (String::from_utf8_lossy(&todo[..corte]).to_ascii_lowercase(), todo[corte + 4..].to_vec())
    }

    #[test]
    fn el_servidor_de_verdad_suelta_a_los_dos_segundos_a_quien_abre_y_calla() {
        let c = Carpetas::nuevas("callado");
        let (_, puerto) = arrancar(&[c.raiz.clone()]).expect("servidor");
        let mut conexion = TcpStream::connect(("127.0.0.1", puerto)).expect("conectar");
        conexion.set_read_timeout(Some(Duration::from_secs(10))).expect("espera");
        let empieza = Instant::now();
        // Sin pedir nada: cuando al servidor se le acaba el plazo cierra, y aqui se ve como
        // el final de la conexion.
        let mut resto = Vec::new();
        let _ = conexion.read_to_end(&mut resto);
        let tardo = empieza.elapsed();
        assert!(resto.is_empty(), "a quien no pide nada no se le contesta nada");
        assert!(tardo >= Duration::from_millis(1500), "ha soltado antes de tiempo: {tardo:?}");
        // Con cinco segundos, quien reabria conexiones vacias tenia casi siempre ocupados los
        // treinta y dos huecos: a dos segundos cada una, le cuesta mucho mas llenarlos.
        assert!(tardo < Duration::from_millis(3500), "un hueco ocupado sin pedir nada tanto rato: {tardo:?}");
    }

    #[test]
    fn el_servidor_de_verdad_sirve_el_archivo_entero_y_a_trozos_y_nada_mas() {
        let c = Carpetas::nuevas("servidor");
        let (base, puerto) = arrancar(&[c.raiz.clone()]).expect("servidor");
        let ficha = base.rsplit('/').next().expect("ficha").to_string();
        let host = format!("Host: 127.0.0.1:{puerto}\r\n");
        let ruta = format!("/{ficha}/{}", codificar(&c.dentro));
        let esperado: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();

        let (cabeceras, cuerpo) = pedir(puerto, &format!("GET {ruta} HTTP/1.1\r\n{host}\r\n"));
        assert!(cabeceras.starts_with("http/1.1 200 "), "{cabeceras}");
        assert!(cabeceras.contains("content-type: audio/wav"), "{cabeceras}");
        assert!(cabeceras.contains("content-length: 1000"), "{cabeceras}");
        assert!(cabeceras.contains("accept-ranges: bytes"), "{cabeceras}");
        assert!(!cabeceras.contains("access-control"), "otra web no puede leer la respuesta: {cabeceras}");
        assert_eq!(cuerpo, esperado);

        let (cabeceras, cuerpo) = pedir(puerto, &format!("GET {ruta} HTTP/1.1\r\n{host}Range: bytes=10-19\r\n\r\n"));
        assert!(cabeceras.starts_with("http/1.1 206 "), "{cabeceras}");
        assert!(cabeceras.contains("content-range: bytes 10-19/1000"), "{cabeceras}");
        assert!(cabeceras.contains("content-length: 10"), "{cabeceras}");
        assert_eq!(cuerpo, &esperado[10..20]);

        let (cabeceras, cuerpo) = pedir(puerto, &format!("GET {ruta} HTTP/1.1\r\n{host}Range: bytes=-4\r\n\r\n"));
        assert!(cabeceras.contains("content-range: bytes 996-999/1000"), "{cabeceras}");
        assert_eq!(cuerpo, &esperado[996..]);

        let (cabeceras, cuerpo) = pedir(puerto, &format!("GET {ruta} HTTP/1.1\r\n{host}Range: bytes=5000-\r\n\r\n"));
        assert!(cabeceras.starts_with("http/1.1 416 "), "{cabeceras}");
        assert!(cabeceras.contains("content-range: bytes */1000"), "{cabeceras}");
        assert!(cuerpo.is_empty());

        let (cabeceras, cuerpo) = pedir(puerto, &format!("HEAD {ruta} HTTP/1.1\r\n{host}\r\n"));
        assert!(cabeceras.starts_with("http/1.1 200 "), "{cabeceras}");
        assert!(cabeceras.contains("content-length: 1000"), "{cabeceras}");
        assert!(cuerpo.is_empty(), "HEAD no trae el archivo");

        // Los «no»: sin la ficha, un archivo de fuera, otro nombre en Host, y escribir.
        let mala = format!("/{}/{}", "0".repeat(ficha.len()), codificar(&c.dentro));
        let (cabeceras, cuerpo) = pedir(puerto, &format!("GET {mala} HTTP/1.1\r\n{host}\r\n"));
        assert!(cabeceras.starts_with("http/1.1 404 "), "sin la ficha: {cabeceras}");
        assert!(cuerpo.is_empty());
        let de_fuera = format!("/{ficha}/{}", codificar(&c.fuera));
        let (cabeceras, cuerpo) = pedir(puerto, &format!("GET {de_fuera} HTTP/1.1\r\n{host}\r\n"));
        assert!(cabeceras.starts_with("http/1.1 404 "), "un archivo de fuera: {cabeceras}");
        assert!(cuerpo.is_empty());
        let (cabeceras, _) = pedir(puerto, &format!("GET {ruta} HTTP/1.1\r\nHost: malo.example.com:{puerto}\r\n\r\n"));
        assert!(cabeceras.starts_with("http/1.1 403 "), "con otro nombre: {cabeceras}");
        let (cabeceras, _) = pedir(puerto, &format!("DELETE {ruta} HTTP/1.1\r\n{host}\r\n"));
        assert!(cabeceras.starts_with("http/1.1 405 "), "{cabeceras}");
        assert!(c.dentro.exists());
    }
}
