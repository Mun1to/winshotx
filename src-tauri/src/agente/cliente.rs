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

/// El primer argumento con el que `winshotx.exe` hace solo de lanzadera: arranca la app y sale
/// enseguida. Es interno (no sale en `--help`); ver `arrancar_app`.
pub const LANZADERA: &str = "--agent-launcher";

/// Arranca winshotx sin argumentos (se queda en la bandeja, como al encender el PC), en la
/// carpeta de su exe, sin heredarle ningún identificador (sus entrada, salida y errores quedan
/// a nulo) y **fuera del árbol y del job de quien la pide**.
///
/// Lo de fuera es lo que importa. Claude Code, al cerrarse en Windows, mata a los 300 ms el árbol
/// entero de procesos del servidor MCP, y un cliente puede meterlo además en un job que mata todo
/// lo suyo al cerrarse. Si la app fuera hija de `winshotx.exe mcp`, cerrar el agente cerraría la
/// app de la bandeja con la grabación en marcha. Por eso no la lanza este proceso, sino una
/// lanzadera (este mismo exe con `LANZADERA`) que sale del job, la lanza y se va: la app queda
/// colgando de un proceso que ya no existe, y no la alcanza nadie que mate «el árbol». Si el job
/// no deja salir a nadie, la lanza el Explorador (lo de siempre para abrir un programa); y si
/// tampoco se puede, se arranca dentro, que es mejor que nada.
pub fn arrancar_app(exe: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let fallo = |e: String| format!("winshotx is not running and could not be started from {} ({e}).", exe.display());
        match proceso::crear(exe, &[LANZADERA], true) {
            Ok(lanzadera) => match lanzadera.esperar(Duration::from_secs(10)) {
                Some(0) => return Ok(()),
                otro => eprintln!("[agente] la lanzadera ha acabado con {otro:?}; la arranca el Explorador"),
            },
            // El job no deja salir: lo dice con «acceso denegado».
            Err(e) if e.code() == windows::Win32::Foundation::ERROR_ACCESS_DENIED.to_hresult() => {}
            Err(e) => return Err(fallo(e.message())),
        }
        if proceso::por_el_explorador(exe).is_ok() {
            return Ok(());
        }
        proceso::crear(exe, &[], false).map(drop).map_err(|e| fallo(e.message()))
    }
    #[cfg(not(windows))]
    Err(format!("winshotx only runs on Windows ({}).", exe.display()))
}

/// `winshotx.exe --agent-launcher`: arranca la app (este mismo exe, sin argumentos) fuera de
/// cualquier job y sale en el acto. Devuelve el código de salida: 0 si la ha lanzado.
pub fn hacer_de_lanzadera() -> i32 {
    #[cfg(windows)]
    {
        let Ok(exe) = std::env::current_exe() else {
            return 1;
        };
        match proceso::crear(&exe, &[], true).or_else(|_| proceso::crear(&exe, &[], false)) {
            Ok(_) => 0,
            Err(_) => 1,
        }
    }
    #[cfg(not(windows))]
    1
}

/// Crear procesos sin heredarles nada.
#[cfg(windows)]
pub(crate) mod proceso {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::time::Duration;

    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        CreateProcessW, GetExitCodeProcess, WaitForSingleObject, CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP,
        DETACHED_PROCESS, PROCESS_INFORMATION, STARTUPINFOW,
    };

    fn ancho(texto: &OsStr) -> Vec<u16> {
        texto.encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Un proceso recién creado. Soltarlo no lo mata: solo cierra el identificador.
    pub struct Proceso(HANDLE);

    impl Proceso {
        /// Su código de salida, si acaba antes de `cuanto`.
        pub fn esperar(&self, cuanto: Duration) -> Option<u32> {
            unsafe {
                if WaitForSingleObject(self.0, cuanto.as_millis().min(u128::from(u32::MAX - 1)) as u32) != WAIT_OBJECT_0 {
                    return None;
                }
                let mut codigo = 0u32;
                GetExitCodeProcess(self.0, &mut codigo).ok()?;
                Some(codigo)
            }
        }
    }

    impl Drop for Proceso {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    /// Arranca `exe` con esos argumentos, en la carpeta del exe, sin consola, en su propio grupo
    /// (un Ctrl+C de quien lo lanza no le llega) y **sin heredar ningún identificador**
    /// (`bInheritHandles` a falso: ni la salida estándar de este proceso ni nada). Con
    /// `fuera_del_job`, además sale del job de este proceso, si lo hay y lo deja.
    pub fn crear(exe: &Path, args: &[&str], fuera_del_job: bool) -> windows::core::Result<Proceso> {
        let aplicacion = ancho(exe.as_os_str());
        let mut linea = format!("\"{}\"", exe.display());
        for arg in args {
            linea.push(' ');
            linea.push_str(arg);
        }
        let mut linea = ancho(OsStr::new(&linea));
        let carpeta = exe.parent().map(|c| ancho(c.as_os_str()));
        let inicio = STARTUPINFOW { cb: std::mem::size_of::<STARTUPINFOW>() as u32, ..Default::default() };
        let mut flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        if fuera_del_job {
            flags |= CREATE_BREAKAWAY_FROM_JOB;
        }
        let mut info = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessW(
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
            )?;
            let _ = CloseHandle(info.hThread);
        }
        Ok(Proceso(info.hProcess))
    }

    /// Que lo abra el Explorador, como un doble clic: `explorer.exe "<exe>"` le pasa el encargo
    /// al Explorador que ya está abierto y se cierra, así que el programa cuelga de él y no de
    /// quien lo pide (ni de su job). Solo se usa si el job no deja salir.
    pub fn por_el_explorador(exe: &Path) -> windows::core::Result<()> {
        let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        let explorador = Path::new(&windows).join("explorer.exe");
        let argumento = format!("\"{}\"", exe.display());
        crear(&explorador, &[&argumento], false).map(drop)
    }
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

    fn de_sistema(exe: &str) -> std::path::PathBuf {
        let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        Path::new(&windows).join("System32").join(exe)
    }

    /// Lo que arranca el cliente no hereda nada: aquí, el extremo de escribir de una tubería
    /// marcada para heredarse, como la salida estándar que un agente le pasa a `winshotx.exe
    /// mcp`. Si el hijo (un `ping` de dos segundos, sin ventana ni sonido) se la quedara, leer
    /// no acabaría hasta que él saliera; sin heredarla, acaba en cuanto se cierra la nuestra.
    #[cfg(windows)]
    #[test]
    fn lo_que_arranca_no_se_queda_con_la_salida_de_quien_lo_pide() {
        use std::io::Read;
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};
        let (mut lector, escritor) = std::io::pipe().unwrap();
        unsafe {
            SetHandleInformation(HANDLE(escritor.as_raw_handle()), HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT).unwrap();
        }
        let ping = proceso::crear(&de_sistema("PING.EXE"), &["-n", "3", "127.0.0.1"], false).unwrap();
        drop(escritor);
        let empezado = Instant::now();
        let mut resto = Vec::new();
        lector.read_to_end(&mut resto).unwrap();
        assert!(empezado.elapsed() < Duration::from_millis(1500), "el hijo se quedó con la tubería: {:?}", empezado.elapsed());
        assert_eq!(ping.esperar(Duration::from_secs(10)), Some(0), "y el hijo sí ha corrido entero");
    }

    /// El padre de ese proceso y el nombre del exe del padre, de una foto de los procesos.
    #[cfg(windows)]
    fn padre_de(pid: u32) -> Option<(u32, String)> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
        };
        let mut todos = Vec::new();
        unsafe {
            let foto = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
            let mut p = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            let mut hay = Process32FirstW(foto, &mut p).is_ok();
            while hay {
                let largo = p.szExeFile.iter().position(|c| *c == 0).unwrap_or(p.szExeFile.len());
                todos.push((p.th32ProcessID, p.th32ParentProcessID, String::from_utf16_lossy(&p.szExeFile[..largo])));
                hay = Process32NextW(foto, &mut p).is_ok();
            }
            let _ = CloseHandle(foto);
        }
        let padre = todos.iter().find(|(p, _, _)| *p == pid)?.1;
        let nombre = todos.iter().find(|(p, _, _)| *p == padre).map(|t| t.2.clone()).unwrap_or_default();
        Some((padre, nombre))
    }

    /// El arranque de verdad, con una «sonda» en lugar de la app: un exe de subsistema Windows
    /// SIN ventanas que apunta en `sonda.txt` (junto a él) su pid y sus argumentos, espera cuatro
    /// segundos y sale. Lanzada con `arrancar_app` desde un job que mata todo al cerrarse y no
    /// deja salir (la terminal de Claude Code lo es: medido el 12-nov-2026), tiene que acabar
    /// fuera de ese job y sin colgar de este proceso. La sonda es cualquier exe así; la de la
    /// prueba del 12-nov está en el informe de S15.
    ///
    /// `WINSHOTX_SONDA=C:\...\sonda.exe cargo test --lib arrancar_fuera_del_job_con_una_sonda -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn arrancar_fuera_del_job_con_una_sonda() {
        let sonda = std::path::PathBuf::from(std::env::var_os("WINSHOTX_SONDA").expect("WINSHOTX_SONDA"));
        let nota = sonda.with_file_name("sonda.txt");
        let _ = std::fs::remove_file(&nota);
        arrancar_app(&sonda).unwrap();
        let empezado = Instant::now();
        let texto = loop {
            if let Some(t) = std::fs::read_to_string(&nota).ok().filter(|t| t.ends_with('\n')) {
                break t;
            }
            assert!(empezado.elapsed() < Duration::from_secs(10), "la sonda no ha arrancado");
            std::thread::sleep(Duration::from_millis(50));
        };
        println!("la sonda dice: {}", texto.trim());
        let pid: u32 = texto.split_whitespace().next().unwrap().trim_start_matches("pid=").parse().unwrap();
        let (padre, nombre) = padre_de(pid).expect("la sonda sigue viva");
        println!("su padre: {padre} ({nombre}); este proceso: {}", std::process::id());
        // En Windows 11 casi todo programa va en ALGÚN job (la app instalada, arrancada a mano,
        // también): lo que cuenta es que no esté en el de este proceso, que es el que lo mataría.
        let de_este_job = pids_de_este_job();
        println!("en el job de este proceso: {de_este_job:?}");
        assert!(de_este_job.contains(&std::process::id()), "la lista del job es la de este proceso");
        assert!(!de_este_job.contains(&pid), "la sonda ha quedado en el job de quien la pidió");
        assert!(!de_este_job.contains(&padre) && padre != std::process::id(), "la sonda cuelga de quien la pidió");
    }

    /// Los procesos que hay en el job de este proceso (vacío si no va en ninguno).
    #[cfg(windows)]
    fn pids_de_este_job() -> Vec<u32> {
        use windows::Win32::System::JobObjects::{JobObjectBasicProcessIdList, QueryInformationJobObject};
        // JOBOBJECT_BASIC_PROCESS_ID_LIST: dos u32 (cuántos hay y cuántos caben) y los pid, de
        // a `usize` cada uno.
        let mut bloque = vec![0usize; 1024];
        let leido = unsafe {
            QueryInformationJobObject(None, JobObjectBasicProcessIdList, bloque.as_mut_ptr().cast(), (bloque.len() * 8) as u32, None)
        };
        if leido.is_err() {
            return Vec::new();
        }
        let en_la_lista = (bloque[0] >> 32) as usize;
        bloque[1..=en_la_lista.min(bloque.len() - 1)].iter().map(|p| *p as u32).collect()
    }

    #[cfg(windows)]
    #[test]
    fn se_lee_el_codigo_con_el_que_sale_lo_que_arranca() {
        let cmd = proceso::crear(&de_sistema("cmd.exe"), &["/d", "/c", "exit", "7"], false).unwrap();
        assert_eq!(cmd.esperar(Duration::from_secs(10)), Some(7));
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
