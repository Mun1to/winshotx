//! El cliente del canal: lo usan la orden `winshotx.exe agent` y el servidor MCP
//! `winshotx.exe mcp` (S16). Manda una petición del contrato y devuelve la respuesta.
//!
//! **Si la app no está abierta, la arranca y espera** (hasta `ESPERA_AL_ARRANCAR`): un agente
//! no tiene por qué saber que winshotx vive en la bandeja. La arranca sin heredarle nada: si la
//! app se quedara con la salida estándar de este proceso (que es la tubería por la que el agente
//! lee la respuesta), el agente se quedaría esperando el final de esa salida hasta que alguien
//! cerrara winshotx.
//!
//! Las órdenes que tardan (`stop`, `export`) no tienen tope: se espera a la respuesta lo que
//! haga falta. Solo conectar tiene tope.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::tuberia;

/// Lo que se espera a que la app conteste después de arrancarla.
pub const ESPERA_AL_ARRANCAR: Duration = Duration::from_secs(15);

/// Manda la petición a la app de este usuario (arrancándola si hace falta) y devuelve la
/// respuesta tal cual, `{"ok":true,...}` o `{"ok":false,"error":...}`. El `Err` es que no se ha
/// podido hablar con ella, ya en inglés y para un agente.
pub fn pedir(peticion: &Value) -> Result<Value, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Could not find winshotx.exe itself ({e})."))?;
    pedir_con(
        &tuberia::nombre(),
        &con_rutas_absolutas(peticion),
        &|| arrancar_app(&exe),
        ESPERA_AL_ARRANCAR,
    )
}

/// `pedir` con todo lo de fuera a mano: el nombre de la tubería, cómo se arranca la app y
/// cuánto se espera. Es lo que usan las pruebas.
pub fn pedir_con(
    nombre: &str,
    peticion: &Value,
    arrancar: &dyn Fn() -> Result<(), String>,
    espera: Duration,
) -> Result<Value, String> {
    let conexion = conectar(nombre, arrancar, espera)?;
    // Ver `tuberia`: una tubería con este nombre puesta por OTRO usuario se quedaría con la
    // petición y podría contestar lo que quisiera.
    match tuberia::dueno_es_este_usuario(&conexion) {
        Ok(true) => {}
        Ok(false) => {
            return Err(format!(
                "{nombre} belongs to another Windows user, so winshotx will not talk to it. Close whatever program created it and try again."
            ))
        }
        Err(e) => return Err(format!("Could not check who owns {nombre} ({e}).")),
    }
    let linea = serde_json::to_string(peticion).map_err(|e| format!("Could not write the request ({e})."))?;
    (&conexion)
        .write_all(format!("{linea}\n").as_bytes())
        .map_err(|e| format!("winshotx closed the connection before the request was sent ({e})."))?;
    let mut respuesta = String::new();
    let leidos = BufReader::new(&conexion)
        .read_line(&mut respuesta)
        .map_err(|e| format!("winshotx closed the connection without answering ({e})."))?;
    if leidos == 0 {
        return Err("winshotx closed the connection without answering. It may have been closed or crashed; try again.".into());
    }
    serde_json::from_str(respuesta.trim()).map_err(|e| format!("winshotx answered something that is not JSON ({e})."))
}

/// Se conecta, y si no hay nadie escuchando arranca la app una vez y reintenta hasta `espera`.
fn conectar(nombre: &str, arrancar: &dyn Fn() -> Result<(), String>, espera: Duration) -> Result<std::fs::File, String> {
    let empezado = Instant::now();
    let mut arrancada = false;
    loop {
        match tuberia::abrir(nombre) {
            Ok(conexion) => return Ok(conexion),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if !arrancada {
                    arrancar()?;
                    arrancada = true;
                }
            }
            // Todas las instancias ocupadas un momento: la app abre otra en cuanto entra alguien.
            Err(e) if e.raw_os_error() == Some(231) => tuberia::esperar_libre(nombre, 500),
            Err(e) => return Err(format!("Could not connect to winshotx at {nombre} ({e}).")),
        }
        if empezado.elapsed() >= espera {
            return Err(format!(
                "winshotx did not answer within {} s of starting it. If an older winshotx (before 0.4.5) is open, quit it from its tray icon and try again; if not, open winshotx once by hand and retry.",
                espera.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Las rutas de salida relativas, contra la carpeta de quien pide: la app corre en la suya y
/// no sabe cuál es la del agente. Las dos clientes pasan por aquí, así que `"output":"demo.mp4"`
/// funciona igual desde `agent` que desde `mcp`.
pub fn con_rutas_absolutas(peticion: &Value) -> Value {
    let mut peticion = peticion.clone();
    if let Some(Value::String(ruta)) = peticion.get("output") {
        let ruta = Path::new(ruta.trim());
        if !ruta.as_os_str().is_empty() && !ruta.is_absolute() {
            if let Ok(aqui) = std::env::current_dir() {
                peticion["output"] = Value::String(aqui.join(ruta).to_string_lossy().into_owned());
            }
        }
    }
    peticion
}

/// Arranca winshotx sin argumentos (se queda en la bandeja, como al encender el PC), sin
/// heredarle ningún identificador y en la carpeta de su exe. Primero intenta sacarla del «job»
/// de quien la arranca: algunos agentes meten sus órdenes en uno que mata todo lo que cuelga de
/// ellas al acabar, y winshotx tiene que seguir viva. Si ese job no deja salir, se arranca dentro.
pub fn arrancar_app(exe: &Path) -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        use windows::core::{PCWSTR, PWSTR};
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            CreateProcessW, CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS, PROCESS_INFORMATION,
            STARTUPINFOW,
        };
        let ancho = |t: &std::ffi::OsStr| -> Vec<u16> {
            use std::os::windows::ffi::OsStrExt;
            t.encode_wide().chain(std::iter::once(0)).collect()
        };
        let aplicacion = ancho(exe.as_os_str());
        let mut linea = ancho(std::ffi::OsStr::new(&format!("\"{}\"", exe.display())));
        let carpeta = exe.parent().map(|c| ancho(c.as_os_str()));
        let inicio = STARTUPINFOW { cb: std::mem::size_of::<STARTUPINFOW>() as u32, ..Default::default() };
        let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        let mut ultimo = None;
        for flags in [base | CREATE_BREAKAWAY_FROM_JOB, base] {
            let mut info = PROCESS_INFORMATION::default();
            match CreateProcessW(
                PCWSTR(aplicacion.as_ptr()),
                Some(PWSTR(linea.as_mut_ptr())),
                None,
                None,
                false,
                flags,
                None,
                carpeta.as_ref().map_or(PCWSTR::null(), |c| PCWSTR(c.as_ptr())),
                &inicio,
                &mut info,
            ) {
                Ok(()) => {
                    let _ = CloseHandle(info.hThread);
                    let _ = CloseHandle(info.hProcess);
                    return Ok(());
                }
                Err(e) => ultimo = Some(e),
            }
        }
        Err(format!(
            "winshotx is not running and could not be started from {} ({}).",
            exe.display(),
            ultimo.map(|e| e.message()).unwrap_or_default()
        ))
    }
    #[cfg(not(windows))]
    Err(format!("winshotx only runs on Windows ({}).", exe.display()))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::agente::ordenes;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn nombre_de_prueba(que: &str) -> String {
        format!(r"\\.\pipe\winshotx-agent-cliente-{que}-{}-{}", std::process::id(), uuid::Uuid::new_v4().simple())
    }

    /// El servidor con el doble de `ordenes` detrás, como la app pero sin pantalla.
    fn servidor_con_doble(nombre: &str) {
        let doble = Arc::new(Mutex::new(ordenes::pruebas::Doble::default()));
        let manejador: tuberia::Manejador = Arc::new(move |linea: &str| ordenes::atender(linea, &*doble.lock().unwrap()));
        tuberia::escuchar(nombre, manejador).unwrap();
    }

    fn sin_arrancar() -> Result<(), String> {
        panic!("la app ya estaba escuchando: no había que arrancarla")
    }

    #[cfg(windows)]
    #[test]
    fn de_punta_a_punta_por_la_tuberia_con_el_doble() {
        let nombre = nombre_de_prueba("punta");
        servidor_con_doble(&nombre);
        let pide = |v: Value| pedir_con(&nombre, &v, &sin_arrancar, Duration::from_secs(2)).unwrap();
        assert_eq!(pide(json!({"command":"status"}))["version"], "0.4.5");
        let grabando = pide(json!({"command":"record","screen":0}));
        assert_eq!(grabando["session"], "abc123");
        assert_eq!(pide(json!({"command":"status"}))["session"], "abc123");
        let parada = pide(json!({"command":"stop"}));
        assert_eq!((parada["ok"].clone(), parada["seconds"].clone()), (json!(true), json!(12.346)));
        // Un error de la app llega como respuesta, no como fallo del canal.
        let otra = pide(json!({"command":"stop"}));
        assert_eq!(otra["ok"], false);
    }

    /// Sin nadie escuchando, arranca la app UNA vez y espera a que conteste.
    #[cfg(windows)]
    #[test]
    fn si_la_app_no_esta_la_arranca_y_espera() {
        let nombre = nombre_de_prueba("arranca");
        let veces = AtomicUsize::new(0);
        let n = nombre.clone();
        let arrancar = || {
            veces.fetch_add(1, Ordering::SeqCst);
            let n = n.clone();
            // Como la app de verdad: tarda un rato en abrir la tubería.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(600));
                servidor_con_doble(&n);
            });
            Ok(())
        };
        let r = pedir_con(&nombre, &json!({"command":"status"}), &arrancar, Duration::from_secs(5)).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(veces.load(Ordering::SeqCst), 1);
    }

    #[cfg(windows)]
    #[test]
    fn si_no_contesta_a_tiempo_lo_dice() {
        let nombre = nombre_de_prueba("nunca");
        let e = pedir_con(&nombre, &json!({"command":"status"}), &|| Ok(()), Duration::from_millis(400)).unwrap_err();
        assert!(e.contains("did not answer within") && e.contains("older winshotx"), "{e}");
        let e = pedir_con(&nombre, &json!({"command":"status"}), &|| Err("no está".into()), Duration::from_secs(1)).unwrap_err();
        assert_eq!(e, "no está");
    }

    #[cfg(windows)]
    #[test]
    fn arrancar_un_exe_que_no_existe_da_un_error_legible() {
        let e = arrancar_app(Path::new(r"C:\no\existe\winshotx.exe")).unwrap_err();
        assert!(e.contains("could not be started") && e.contains(r"C:\no\existe\winshotx.exe"), "{e}");
    }

    #[test]
    fn las_rutas_relativas_se_hacen_absolutas_con_la_carpeta_de_quien_pide() {
        let aqui = std::env::current_dir().unwrap();
        let r = con_rutas_absolutas(&json!({"command":"export","session":"a","output":"videos\\demo.mp4"}));
        assert_eq!(r["output"], aqui.join("videos\\demo.mp4").to_string_lossy().as_ref());
        let absoluta = json!({"command":"capture","screen":0,"output":"C:\\x\\a.png"});
        assert_eq!(con_rutas_absolutas(&absoluta), absoluta);
        let sin = json!({"command":"status"});
        assert_eq!(con_rutas_absolutas(&sin), sin);
    }

    /// Un servidor de mentira con el nombre DE VERDAD de la tubería (con el doble de `ordenes`
    /// detrás), para probar el exe (`winshotx.exe agent ...`) de punta a punta sin la app: el
    /// exe se conecta a esto y no arranca nada. Si la tubería ya es de otro (la app nueva
    /// abierta), falla al empezar y no hay que lanzar el exe. Vive `WINSHOTX_MENTIRA_SEGUNDOS`
    /// (60 si no):
    ///
    /// `cargo test --lib servidor_de_mentira_con_el_nombre_de_verdad -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn servidor_de_mentira_con_el_nombre_de_verdad() {
        servidor_con_doble(&tuberia::nombre());
        println!("ESCUCHANDO en {}", tuberia::nombre());
        let segundos = std::env::var("WINSHOTX_MENTIRA_SEGUNDOS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
        std::thread::sleep(Duration::from_secs(segundos));
    }

    /// Lo que dura cada pista según ffprobe (si está): la de vídeo, en segundos.
    fn duracion_del_video(mp4: &Path) -> Option<f64> {
        let salida = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=duration", "-of", "csv=p=0"])
            .arg(mp4)
            .output()
            .ok()?;
        String::from_utf8_lossy(&salida.stdout).trim().parse().ok()
    }

    /// DE VERDAD, con la app: graba 2 s de la pantalla 0, para y exporta un MP4. Se ve en la
    /// pantalla (el marco rojo y la barra, como cualquier grabación) y NO hace sonar nada: sin
    /// marimba, sin sonido del PC, sin micro y sin cámara. Necesita la app NUEVA abierta (con el
    /// canal), o `WINSHOTX_EXE` con la ruta de su exe para que la arranque:
    ///
    /// `WINSHOTX_EXE=C:\...\winshotx.exe cargo test --lib un_agente_graba_dos_segundos -- --ignored --nocapture`
    ///
    /// Solo con quien está delante avisado: graba su pantalla.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn un_agente_graba_dos_segundos() {
        let arrancar = || match std::env::var_os("WINSHOTX_EXE") {
            Some(exe) => arrancar_app(Path::new(&exe)),
            None => Err("abre winshotx (la versión con el canal) o pon WINSHOTX_EXE con la ruta de su exe".into()),
        };
        let pide = |v: Value| {
            let r = pedir_con(&tuberia::nombre(), &v, &arrancar, ESPERA_AL_ARRANCAR).expect("hablar con winshotx");
            println!("{v} -> {r}");
            assert_eq!(r["ok"], true, "{r}");
            r
        };
        pide(json!({"command":"status"}));
        let grabando = pide(json!({
            "command":"record","screen":0,"camera":false,"systemAudio":false,"microphone":false,"cue":false
        }));
        std::thread::sleep(Duration::from_secs(2));
        let parada = pide(json!({"command":"stop"}));
        assert_eq!(parada["session"], grabando["session"]);
        let segundos = parada["seconds"].as_f64().unwrap();
        assert!((1.5..4.0).contains(&segundos), "la sesión dura {segundos} s");
        let salida = std::env::temp_dir().join("winshotx-un-agente-graba-dos-segundos.mp4");
        let _ = std::fs::remove_file(&salida);
        let hecho = pide(json!({"command":"export","session":parada["session"],"output":salida}));
        assert_eq!(hecho["file"], salida.to_string_lossy().as_ref());
        let bytes = std::fs::metadata(&salida).expect("el MP4 está en el disco").len();
        assert!(bytes > 1000 && hecho["bytes"].as_u64() == Some(bytes), "{bytes} bytes");
        if let Some(duracion) = duracion_del_video(&salida) {
            assert!((1.5..4.0).contains(&duracion), "el MP4 dura {duracion} s");
            println!("MP4 de {duracion} s y {bytes} bytes en {}", salida.display());
        } else {
            println!("sin ffprobe: MP4 de {bytes} bytes en {}", salida.display());
        }
        assert_eq!(pide(json!({"command":"status"}))["recording"], false);
    }
}
