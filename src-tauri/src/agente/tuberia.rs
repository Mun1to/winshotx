//! La tubería con nombre del canal de agentes: el servidor que vive en la app y la conexión
//! del cliente.
//!
//! **Solo la puede abrir este usuario.** La tubería se crea con un descriptor de seguridad
//! hecho a mano: su dueño es el usuario que arranca la app, la única entrada que deja pasar es
//! la suya, y los que entran por la red se rechazan dos veces (una entrada que les niega todo y
//! `PIPE_REJECT_REMOTE_CLIENTS`). Sin eso, Windows le pone la de por defecto, que deja leer a
//! cualquiera del equipo.
//!
//! **Y el cliente mira de quién es antes de hablar.** Otro usuario del mismo equipo podría
//! crear antes una tubería con el mismo nombre y quedarse con las órdenes (y contestar rutas
//! falsas). El cliente comprueba que el dueño de la tubería a la que se ha conectado es él
//! mismo, y se conecta con `SECURITY_IDENTIFICATION`, para que el otro lado no pueda hacerse
//! pasar por él.
//!
//! El protocolo es una línea por mensaje: una petición JSON terminada en `\n`, una respuesta
//! igual. Por la misma conexión pueden ir varias seguidas, y se contestan en orden. Cada
//! conexión tiene su hilo, así que una exportación larga no deja esperando a un `status`.
//!
//! **En Linux es un socket Unix** (`linux`, más abajo): `winshotx-agent.sock` en la carpeta
//! de los sockets del usuario (`$XDG_RUNTIME_DIR`), con las dos mismas reglas. Solo lo abre
//! este usuario: la carpeta tiene que ser suya y de nadie más, el archivo queda en 0600, y el
//! servidor le pregunta al núcleo quién ha conectado (`SO_PEERCRED`) y corta a quien no sea él.
//! Y el cliente mira lo mismo del otro lado antes de mandar nada.

#[cfg(any(test, not(any(windows, target_os = "linux"))))]
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;

/// Lo más largo que puede ser una petición. Ninguna orden de verdad pasa de unos cientos de
/// bytes; esto es para que alguien que mande basura sin saltos de línea no llene la memoria.
pub const LINEA_MAXIMA: usize = 1 << 20;

/// Lo que contesta el servidor a cada línea que le llega.
pub type Manejador = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// El nombre de la tubería de este usuario: `\\.\pipe\winshotx-agent-<usuario de Windows>`.
///
/// Lleva el usuario para que dos personas con sesión abierta en el mismo equipo (escritorio
/// remoto, cambio rápido de usuario) tengan cada una la suya y no choquen.
///
/// En Linux es la ruta del socket, que ya está en la carpeta de cada usuario (ver `linux`).
pub fn nombre() -> String {
    #[cfg(target_os = "linux")]
    {
        linux::nombre()
    }
    #[cfg(not(target_os = "linux"))]
    nombre_para(&usuario())
}

/// El nombre de la tubería para ese usuario. Una barra no puede ir en el nombre de una tubería
/// (Windows la leería como una carpeta), así que se cambia por `_`, como cualquier carácter de
/// control.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub fn nombre_para(usuario: &str) -> String {
    let limpio: String = usuario
        .trim()
        .chars()
        .map(|c| if c == '\\' || c == '/' || c.is_control() { '_' } else { c })
        .collect();
    let limpio = if limpio.is_empty() { "user".to_string() } else { limpio };
    format!(r"\\.\pipe\winshotx-agent-{limpio}")
}

/// El usuario de Windows de este proceso. Se le pregunta al sistema y no a la variable
/// `USERNAME`, que la puede cambiar cualquiera que lance el proceso; esa queda de respaldo.
#[cfg(not(target_os = "linux"))]
fn usuario() -> String {
    #[cfg(windows)]
    {
        use windows::core::PWSTR;
        use windows::Win32::System::WindowsProgramming::GetUserNameW;
        let mut bufer = [0u16; 257];
        let mut largo = bufer.len() as u32;
        if unsafe { GetUserNameW(Some(PWSTR(bufer.as_mut_ptr())), &mut largo) }.is_ok() && largo > 1 {
            // `largo` cuenta el cero del final.
            return String::from_utf16_lossy(&bufer[..largo as usize - 1]);
        }
    }
    std::env::var("USERNAME").unwrap_or_default()
}

#[cfg(windows)]
pub use win::{abrir, dueno_es_este_usuario, escuchar_con_actividad, esperar_libre};
#[cfg(all(windows, test))]
pub use win::escuchar;

#[cfg(target_os = "linux")]
pub use linux::{abrir, carpeta, dueno_es_este_usuario, escuchar, esperar_libre};

#[cfg(not(any(windows, target_os = "linux")))]
pub fn escuchar(_nombre: &str, _manejador: Manejador) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "solo en Windows y en Linux"))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn abrir(_nombre: &str) -> std::io::Result<File> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "solo en Windows y en Linux"))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn dueno_es_este_usuario(_archivo: &File) -> std::io::Result<bool> {
    Ok(false)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn esperar_libre(_nombre: &str, _ms: u32) {}

/// Atiende una conexión entera: lee líneas, contesta cada una y vuelve cuando el otro lado
/// cuelga. No sabe nada de Windows: sirve igual para una tubería que para cualquier `Read` +
/// `Write`, y así se prueba también sin tubería.
pub fn atender_conexion<C>(conexion: &C, manejador: &Manejador) -> std::io::Result<()>
where
    for<'a> &'a C: std::io::Read + std::io::Write,
{
    let mut lector = BufReader::new(conexion);
    // `BufReader` se queda con una referencia; para escribir se usa otra a la misma conexión,
    // que en una tubería síncrona es el mismo identificador. Se lee y se escribe por turnos,
    // nunca a la vez: Windows pone en fila la E/S síncrona de un mismo objeto.
    let escritor_de = |c: &C, texto: &str| -> std::io::Result<()> {
        let mut w: &C = c;
        w.write_all(texto.as_bytes())?;
        w.write_all(b"\n")?;
        w.flush()
    };
    loop {
        let mut linea = Vec::new();
        let leidos = std::io::Read::take(&mut lector, LINEA_MAXIMA as u64 + 1).read_until(b'\n', &mut linea)?;
        if leidos == 0 {
            return Ok(());
        }
        if linea.len() > LINEA_MAXIMA {
            let error = serde_json::json!({
                "ok": false,
                "error": format!("The request is longer than {LINEA_MAXIMA} bytes. Send one short JSON object per line."),
            });
            escritor_de(&conexion, &error.to_string())?;
            return Ok(());
        }
        let texto = String::from_utf8_lossy(&linea);
        let texto = texto.trim();
        if texto.is_empty() {
            continue;
        }
        let respuesta = manejador(texto);
        escritor_de(&conexion, &respuesta)?;
    }
}

#[cfg(windows)]
mod win {
    use std::fs::File;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};

    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, LocalFree, ERROR_PIPE_CONNECTED, ERROR_SUCCESS, HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
    };
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows::Win32::Security::{
        EqualSid, GetTokenInformation, TokenUser, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
        SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::Storage::FileSystem::{
        FlushFileBuffers, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
        SECURITY_IDENTIFICATION,
    };
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    use super::{atender_conexion, Manejador};

    fn ancho(texto: &str) -> Vec<u16> {
        texto.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn io(error: windows::core::Error) -> std::io::Error {
        std::io::Error::other(error.message())
    }

    /// El SID del usuario de este proceso, en su memoria (`TOKEN_USER` va seguido del SID en
    /// el mismo bloque). Va en `u64` para que quede alineado como pide la estructura.
    pub(super) struct SidPropio {
        bloque: Vec<u64>,
    }

    impl SidPropio {
        pub(super) fn leer() -> std::io::Result<Self> {
            unsafe {
                let mut token = HANDLE::default();
                OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(io)?;
                let mut largo = 0u32;
                let _ = GetTokenInformation(token, TokenUser, None, 0, &mut largo);
                let mut bloque = vec![0u64; (largo as usize).div_ceil(8).max(1)];
                let leido = GetTokenInformation(
                    token,
                    TokenUser,
                    Some(bloque.as_mut_ptr().cast()),
                    (bloque.len() * 8) as u32,
                    &mut largo,
                );
                let _ = CloseHandle(token);
                leido.map_err(io)?;
                Ok(Self { bloque })
            }
        }

        pub(super) fn psid(&self) -> PSID {
            unsafe { (*(self.bloque.as_ptr() as *const TOKEN_USER)).User.Sid }
        }

        /// En texto, `S-1-5-21-...`: es lo que lleva el descriptor de seguridad.
        pub(super) fn texto(&self) -> std::io::Result<String> {
            unsafe {
                let mut texto = PWSTR::null();
                ConvertSidToStringSidW(self.psid(), &mut texto).map_err(io)?;
                let s = texto.to_string().map_err(|e| std::io::Error::other(e.to_string()));
                let _ = LocalFree(Some(HLOCAL(texto.0.cast())));
                s
            }
        }
    }

    /// El descriptor de seguridad de la tubería, en SDDL: dueño, este usuario (`O:`); y en la
    /// lista de acceso, protegida para que no herede nada (`D:P`), primero un «no» a todo para
    /// quien entra por la red (`NU`) y luego un «sí» a todo para este usuario. Nadie más.
    pub(super) fn sddl_para(sid: &str) -> String {
        format!("O:{sid}D:P(D;;GA;;;NU)(A;;GA;;;{sid})")
    }

    /// El descriptor ya convertido. Lo pide Windows en cada instancia de la tubería, así que
    /// vive lo que vive el servidor.
    struct Descriptor(PSECURITY_DESCRIPTOR);

    // Es memoria de `LocalAlloc` que solo se lee: se puede pasar a otro hilo.
    unsafe impl Send for Descriptor {}

    impl Descriptor {
        fn de_este_usuario() -> std::io::Result<Self> {
            let sddl = ancho(&sddl_para(&SidPropio::leer()?.texto()?));
            let mut sd = PSECURITY_DESCRIPTOR::default();
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    PCWSTR(sddl.as_ptr()),
                    SDDL_REVISION_1,
                    &mut sd,
                    None,
                )
                .map_err(io)?;
            }
            Ok(Self(sd))
        }
    }

    impl Drop for Descriptor {
        fn drop(&mut self) {
            unsafe {
                let _ = LocalFree(Some(HLOCAL(self.0 .0)));
            }
        }
    }

    /// Una instancia de la tubería, lista para que se conecte alguien. La primera se crea con
    /// `FILE_FLAG_FIRST_PIPE_INSTANCE`: si ya existe una con ese nombre (de otro programa, o
    /// de otro winshotx), falla en vez de sumarse a la suya.
    fn instancia(nombre: &[u16], descriptor: &Descriptor, primera: bool) -> std::io::Result<File> {
        let atributos = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0 .0,
            bInheritHandle: false.into(),
        };
        let modo = if primera {
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE
        } else {
            PIPE_ACCESS_DUPLEX
        };
        let h = unsafe {
            CreateNamedPipeW(
                PCWSTR(nombre.as_ptr()),
                modo,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                64 * 1024,
                0,
                Some(&atributos),
            )
        };
        if h == INVALID_HANDLE_VALUE || h.is_invalid() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_handle(h.0) })
    }

    /// Abre la tubería y la atiende en un hilo propio. Vuelve cuando ya se puede conectar
    /// alguien, o con el error si no se ha podido crear (por ejemplo, porque otro la tiene).
    #[cfg(test)]
    pub fn escuchar(nombre: &str, manejador: Manejador) -> std::io::Result<()> {
        escuchar_con_actividad(nombre, manejador, || ())
    }

    pub fn escuchar_con_actividad<T: Send + 'static>(
        nombre: &str, manejador: Manejador, al_conectar: impl Fn() -> T + Send + 'static,
    ) -> std::io::Result<()> {
        let nombre = ancho(nombre);
        let descriptor = Descriptor::de_este_usuario()?;
        let primera = instancia(&nombre, &descriptor, true)?;
        std::thread::Builder::new()
            .name("agente-tuberia".into())
            .spawn(move || {
                let mut esperando = Some(primera);
                loop {
                    let actual = match esperando.take() {
                        Some(i) => i,
                        None => match instancia(&nombre, &descriptor, false) {
                            Ok(i) => i,
                            Err(error) => {
                                eprintln!("[agente] no se ha podido abrir otra instancia: {error}");
                                std::thread::sleep(std::time::Duration::from_secs(1));
                                continue;
                            }
                        },
                    };
                    let h = HANDLE(actual.as_raw_handle());
                    let conectado = match unsafe { ConnectNamedPipe(h, None) } {
                        Ok(()) => true,
                        // El cliente llegó entre crear la instancia y esperarlo: ya está dentro.
                        Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => true,
                        Err(error) => {
                            eprintln!("[agente] una conexión ha fallado al entrar: {error}");
                            false
                        }
                    };
                    if !conectado {
                        continue;
                    }
                    let trabajo = al_conectar();
                    let manejador = manejador.clone();
                    let _ = std::thread::Builder::new()
                        .name("agente-conexion".into())
                        .spawn(move || {
                            let _trabajo = trabajo;
                            if let Err(error) = atender_conexion(&actual, &manejador) {
                                // Que el otro lado cuelgue a media respuesta no es un fallo nuestro.
                                if error.kind() != std::io::ErrorKind::BrokenPipe {
                                    eprintln!("[agente] una conexión se ha cortado: {error}");
                                }
                            }
                            let h = HANDLE(actual.as_raw_handle());
                            unsafe {
                                let _ = FlushFileBuffers(h);
                                let _ = DisconnectNamedPipe(h);
                            }
                        });
                }
            })?;
        Ok(())
    }

    /// Se conecta a la tubería como cliente, con `SECURITY_IDENTIFICATION`: el servidor puede
    /// saber quién es, pero no hacerse pasar por él. `NotFound` es que no hay nadie escuchando;
    /// el código 231 (`ERROR_PIPE_BUSY`), que están todas ocupadas un momento.
    pub fn abrir(nombre: &str) -> std::io::Result<File> {
        let FILE_FLAGS_AND_ATTRIBUTES(identificacion) = SECURITY_IDENTIFICATION;
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .security_qos_flags(identificacion)
            .open(nombre)
    }

    /// Espera, como mucho `ms`, a que haya una instancia libre de la tubería.
    pub fn esperar_libre(nombre: &str, ms: u32) {
        let nombre = ancho(nombre);
        unsafe {
            let _ = WaitNamedPipeW(PCWSTR(nombre.as_ptr()), ms);
        }
    }

    /// Si el dueño del objeto de ese identificador (la tubería a la que se ha conectado el
    /// cliente) es el usuario de este proceso. Ver el principio del módulo.
    pub fn dueno_es_este_usuario(archivo: &File) -> std::io::Result<bool> {
        let propio = SidPropio::leer()?;
        let mut dueno = PSID::default();
        let mut sd = PSECURITY_DESCRIPTOR::default();
        let error = unsafe {
            GetSecurityInfo(
                HANDLE(archivo.as_raw_handle()),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                Some(&mut dueno),
                None,
                None,
                None,
                Some(&mut sd),
            )
        };
        if error != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(error.0 as i32));
        }
        let igual = !dueno.0.is_null() && unsafe { EqualSid(dueno, propio.psid()) }.is_ok();
        unsafe {
            let _ = LocalFree(Some(HLOCAL(sd.0)));
        }
        Ok(igual)
    }

    #[cfg(test)]
    pub(super) mod para_pruebas {
        use super::*;
        use windows::Win32::Security::{
            CreateRestrictedToken, ImpersonateLoggedOnUser, RevertToSelf, CREATE_RESTRICTED_TOKEN_FLAGS,
            SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_IMPERSONATE,
        };

        /// Abre la tubería como si fuera OTRO usuario: con el token de este proceso, pero con el
        /// SID del usuario apagado (solo vale para negar). Es lo más parecido a otra persona
        /// del mismo equipo que se puede probar sin crear una cuenta: tiene los mismos grupos
        /// (Usuarios, Todos, la sesión interactiva...), pero no es ÉL. Con `solo_leer`, la abre
        /// solo para leer: la seguridad de serie de Windows deja leer a Todos, la nuestra no.
        pub fn abrir_como_otro(nombre: &str, solo_leer: bool) -> std::io::Result<File> {
            unsafe {
                let mut token = HANDLE::default();
                OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_IMPERSONATE, &mut token)
                    .map_err(io)?;
                let propio = SidPropio::leer()?;
                let apagar = [SID_AND_ATTRIBUTES { Sid: propio.psid(), Attributes: 0 }];
                let mut restringido = HANDLE::default();
                let hecho = CreateRestrictedToken(
                    token,
                    CREATE_RESTRICTED_TOKEN_FLAGS(0),
                    Some(&apagar),
                    None,
                    None,
                    &mut restringido,
                );
                let _ = CloseHandle(token);
                hecho.map_err(io)?;
                ImpersonateLoggedOnUser(restringido).map_err(io)?;
                let resultado = if solo_leer { File::open(nombre) } else { abrir(nombre) };
                let _ = RevertToSelf();
                let _ = CloseHandle(restringido);
                resultado
            }
        }
    }
}

/// El canal en Linux: un socket Unix en la carpeta de los sockets del usuario.
///
/// **Dónde.** `$XDG_RUNTIME_DIR/winshotx-agent.sock`. Esa carpeta es del usuario y de nadie más
/// (la especificación de carpetas de XDG la pide con permisos 0700) y se borra al cerrar la
/// sesión. Si la variable no está, `/run/user/<uid>`, que es donde la pone systemd: muchos
/// agentes lanzan el servidor MCP casi sin variables (el SDK oficial solo pasa `HOME`, `PATH`
/// y cuatro más), y sin esto el cliente no encontraría a la app que ya está abierta.
///
/// **Quién.** Un socket con nombre de archivo obedece los permisos de su carpeta y los suyos,
/// pero eso es cosa de Linux y no de POSIX, así que no se deja solo ahí: cada lado le pregunta
/// al núcleo de quién es el proceso del otro (`SO_PEERCRED`, que no se puede falsear) y no
/// habla con nadie que no sea este mismo usuario. Los sockets «abstractos» no valen para esto:
/// no tienen permisos, y cualquiera del equipo podría coger el nombre antes.
#[cfg(target_os = "linux")]
mod linux {
    use std::fs::File;
    use std::os::fd::{AsRawFd, OwnedFd, RawFd};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};

    use super::{atender_conexion, Manejador};

    const ARCHIVO: &str = "winshotx-agent.sock";

    /// El usuario de este proceso.
    pub(super) fn uid() -> u32 {
        unsafe { libc::getuid() }
    }

    /// La carpeta de los sockets de ese usuario: la de la variable si está y es una ruta
    /// entera, y si no la de systemd.
    pub(super) fn carpeta_con(variable: Option<std::ffi::OsString>, uid: u32) -> PathBuf {
        variable
            .map(PathBuf::from)
            .filter(|carpeta| carpeta.is_absolute())
            .unwrap_or_else(|| PathBuf::from(format!("/run/user/{uid}")))
    }

    /// La carpeta de los sockets de este usuario.
    pub fn carpeta() -> PathBuf {
        carpeta_con(std::env::var_os("XDG_RUNTIME_DIR"), uid())
    }

    /// La ruta del socket de este usuario.
    pub fn nombre() -> String {
        carpeta().join(ARCHIVO).to_string_lossy().into_owned()
    }

    fn mal(clase: std::io::ErrorKind, texto: String) -> std::io::Error {
        std::io::Error::new(clase, texto)
    }

    /// Que la carpeta sea de este usuario y que nadie más pueda entrar en ella. En una que
    /// pueden abrir otros no se pone el canal: es lo que pide la especificación, y así otro
    /// usuario no llega ni a conectar.
    pub(super) fn es_privada(carpeta: &Path, uid: u32) -> std::io::Result<()> {
        let datos = std::fs::metadata(carpeta)
            .map_err(|e| mal(e.kind(), format!("no se puede mirar {} ({e})", carpeta.display())))?;
        if !datos.is_dir() {
            return Err(mal(std::io::ErrorKind::NotFound, format!("{} no es una carpeta", carpeta.display())));
        }
        if datos.uid() != uid {
            return Err(mal(
                std::io::ErrorKind::PermissionDenied,
                format!("{} es de otro usuario ({})", carpeta.display(), datos.uid()),
            ));
        }
        if datos.mode() & 0o077 != 0 {
            return Err(mal(
                std::io::ErrorKind::PermissionDenied,
                format!("{} la pueden abrir otros usuarios (permisos {:o})", carpeta.display(), datos.mode() & 0o777),
            ));
        }
        Ok(())
    }

    /// El usuario del proceso que hay al otro lado de ese socket, según el núcleo: el que
    /// llamó a `connect` o a `listen`. No es algo que el otro lado diga de sí mismo.
    fn uid_del_otro(socket: RawFd) -> std::io::Result<u32> {
        // Con un valor que no es de nadie, por si el núcleo no rellenara nada: a cero sería root.
        let mut otro = libc::ucred { pid: 0, uid: libc::uid_t::MAX, gid: libc::gid_t::MAX };
        let mut largo = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let hecho = unsafe {
            libc::getsockopt(socket, libc::SOL_SOCKET, libc::SO_PEERCRED, (&mut otro as *mut libc::ucred).cast(), &mut largo)
        };
        if hecho != 0 {
            return Err(std::io::Error::last_os_error());
        }
        if largo as usize != std::mem::size_of::<libc::ucred>() {
            return Err(std::io::Error::other("el núcleo no ha dicho quién hay al otro lado"));
        }
        Ok(otro.uid)
    }

    /// El socket, puesto en su sitio. Un archivo con ese nombre puede ser de un winshotx que
    /// murió sin recoger (el archivo se queda): se prueba a conectar, y solo si no contesta
    /// nadie se quita y se pone el nuevo. Si contesta alguien, es otro que ya escucha y no se
    /// le toca: falla, como la segunda tubería de Windows.
    fn enlazar(ruta: &Path) -> std::io::Result<UnixListener> {
        match UnixListener::bind(ruta) {
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                if UnixStream::connect(ruta).is_ok() {
                    return Err(mal(
                        std::io::ErrorKind::AddrInUse,
                        format!("otro programa ya escucha en {}", ruta.display()),
                    ));
                }
                std::fs::remove_file(ruta)?;
                UnixListener::bind(ruta)
            }
            otro => otro,
        }
    }

    /// Abre el socket y lo atiende en un hilo propio. Vuelve cuando ya se puede conectar
    /// alguien, o con el error si no se ha podido poner.
    pub fn escuchar(nombre: &str, manejador: Manejador) -> std::io::Result<()> {
        let ruta = PathBuf::from(nombre);
        let carpeta = ruta
            .parent()
            .ok_or_else(|| mal(std::io::ErrorKind::InvalidInput, format!("{nombre} no está en ninguna carpeta")))?;
        let propio = uid();
        es_privada(carpeta, propio)?;
        let escucha = enlazar(&ruta)?;
        // `bind` lo crea con lo que deje la máscara del proceso; la máscara no se toca (es de
        // todo el proceso, y hay más hilos creando archivos). Mientras tanto lo guarda la carpeta.
        std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o600))?;
        std::thread::Builder::new().name("agente-socket".into()).spawn(move || {
            for conexion in escucha.incoming() {
                let conexion = match conexion {
                    Ok(conexion) => conexion,
                    Err(error) => {
                        eprintln!("[agente] una conexión ha fallado al entrar: {error}");
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        continue;
                    }
                };
                let manejador = manejador.clone();
                let _ = std::thread::Builder::new().name("agente-conexion".into()).spawn(move || {
                    match uid_del_otro(conexion.as_raw_fd()) {
                        Ok(otro) if otro == propio => {}
                        Ok(otro) => {
                            eprintln!("[agente] se corta una conexión del usuario {otro}: el canal es solo de este usuario");
                            return;
                        }
                        Err(error) => {
                            eprintln!("[agente] se corta una conexión: no se sabe de quién es ({error})");
                            return;
                        }
                    }
                    if let Err(error) = atender_conexion(&conexion, &manejador) {
                        // Que el otro lado cuelgue a media respuesta no es un fallo nuestro.
                        let colgo = matches!(
                            error.kind(),
                            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                        );
                        if !colgo {
                            eprintln!("[agente] una conexión se ha cortado: {error}");
                        }
                    }
                });
            }
        })?;
        Ok(())
    }

    /// Se conecta al socket como cliente. `NotFound` es que no hay nadie escuchando: o no hay
    /// archivo, o es el de una app que ya no está (`ConnectionRefused`), que para quien pide es
    /// lo mismo.
    pub fn abrir(nombre: &str) -> std::io::Result<File> {
        match UnixStream::connect(nombre) {
            Ok(conexion) => Ok(File::from(OwnedFd::from(conexion))),
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                Err(mal(std::io::ErrorKind::NotFound, format!("nadie escucha en {nombre}")))
            }
            Err(error) => Err(error),
        }
    }

    /// En un socket no hay «todas las instancias ocupadas»: el que conecta espera en la cola.
    pub fn esperar_libre(_nombre: &str, _ms: u32) {}

    /// Si quien escucha al otro lado de esa conexión es este mismo usuario. Con algo que no
    /// es un socket, el error.
    pub fn dueno_es_este_usuario(archivo: &File) -> std::io::Result<bool> {
        Ok(uid_del_otro(archivo.as_raw_fd())? == uid())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Read;

    /// Un nombre de tubería que no choca con la app ni con otra prueba que corra a la vez.
    #[cfg(not(target_os = "linux"))]
    fn nombre_de_prueba(que: &str) -> String {
        format!(
            r"\\.\pipe\winshotx-agent-prueba-{que}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        )
    }

    /// En Linux, un socket en una carpeta nueva y solo de este usuario, como la de verdad.
    #[cfg(target_os = "linux")]
    fn nombre_de_prueba(que: &str) -> String {
        use std::os::unix::fs::DirBuilderExt;
        let carpeta = std::env::temp_dir().join(format!(
            "winshotx-agent-prueba-{que}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&carpeta).unwrap();
        carpeta.join("canal.sock").to_string_lossy().into_owned()
    }

    /// Un manejador de mentira: contesta con lo que le llega, para ver la ida y la vuelta.
    fn eco() -> Manejador {
        Arc::new(|linea: &str| serde_json::json!({ "ok": true, "eco": linea }).to_string())
    }

    fn una_linea(lector: &mut BufReader<&File>) -> serde_json::Value {
        let mut respuesta = String::new();
        lector.read_line(&mut respuesta).unwrap();
        assert!(respuesta.ends_with('\n'), "una respuesta es una línea entera: {respuesta:?}");
        serde_json::from_str(respuesta.trim()).unwrap()
    }

    #[test]
    fn el_nombre_lleva_el_usuario_y_nunca_una_barra() {
        assert_eq!(nombre_para("Muni"), r"\\.\pipe\winshotx-agent-Muni");
        assert_eq!(nombre_para(r"EQUIPO\Muni"), r"\\.\pipe\winshotx-agent-EQUIPO_Muni");
        assert_eq!(nombre_para("José Pérez"), r"\\.\pipe\winshotx-agent-José Pérez");
        assert_eq!(nombre_para("  "), r"\\.\pipe\winshotx-agent-user");
        // El de verdad sale del sistema y no está vacío.
        #[cfg(not(target_os = "linux"))]
        assert!(nombre().len() > r"\\.\pipe\winshotx-agent-".len());
        // En Linux es una ruta entera, y acaba en el archivo del socket.
        #[cfg(target_os = "linux")]
        assert!(nombre().starts_with('/') && nombre().ends_with("/winshotx-agent.sock"), "{}", nombre());
    }

    /// Lo que no depende de Windows, con una conexión de mentira: dos líneas seguidas por la
    /// misma conexión se contestan en orden, y las vacías no se contestan.
    #[test]
    fn una_conexion_de_mentira_contesta_cada_linea_en_orden() {
        struct Mentira {
            entrada: std::cell::RefCell<std::io::Cursor<Vec<u8>>>,
            salida: std::cell::RefCell<Vec<u8>>,
        }
        impl Read for &Mentira {
            fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                self.entrada.borrow_mut().read(b)
            }
        }
        impl Write for &Mentira {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.salida.borrow_mut().write(b)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mentira = Mentira {
            entrada: std::io::Cursor::new(b"uno\n\n  \r\ndos\r\ntres".to_vec()).into(),
            salida: Vec::new().into(),
        };
        let m: Manejador = Arc::new(|l: &str| format!("<{l}>"));
        atender_conexion(&mentira, &m).unwrap();
        assert_eq!(String::from_utf8(mentira.salida.into_inner()).unwrap(), "<uno>\n<dos>\n<tres>\n");
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn ida_y_vuelta_por_la_tuberia_de_verdad() {
        let nombre = nombre_de_prueba("ida");
        escuchar(&nombre, eco()).unwrap();
        let tuberia = abrir_esperando(&nombre);
        (&tuberia).write_all(b"{\"command\":\"status\"}\n").unwrap();
        let mut lector = BufReader::new(&tuberia);
        let respuesta = una_linea(&mut lector);
        assert_eq!(respuesta["eco"], "{\"command\":\"status\"}");
    }

    /// Como hace el cliente: si están todas las instancias ocupadas (231), espera a una libre.
    /// Pasa con dos clientes seguidos: el servidor abre la siguiente instancia un instante
    /// después de que entre el primero.
    #[cfg(windows)]
    fn abrir_esperando(nombre: &str) -> File {
        for _ in 0..20 {
            match abrir(nombre) {
                Err(e) if e.raw_os_error() == Some(231) => esperar_libre(nombre, 500),
                otro => return otro.unwrap(),
            }
        }
        panic!("la tubería sigue ocupada");
    }

    /// En un socket no hay instancias: el que conecta espera su turno en la cola.
    #[cfg(target_os = "linux")]
    fn abrir_esperando(nombre: &str) -> File {
        abrir(nombre).unwrap()
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn dos_peticiones_por_la_misma_conexion_y_dos_clientes_a_la_vez() {
        let nombre = nombre_de_prueba("dos");
        escuchar(&nombre, eco()).unwrap();
        let a = abrir_esperando(&nombre);
        let b = abrir_esperando(&nombre);
        // Las dos de `a` de golpe, en un solo `write`: se contestan las dos, en orden.
        (&a).write_all(b"primera\nsegunda\n").unwrap();
        (&b).write_all(b"de b\n").unwrap();
        let mut la = BufReader::new(&a);
        let mut lb = BufReader::new(&b);
        assert_eq!(una_linea(&mut lb)["eco"], "de b");
        assert_eq!(una_linea(&mut la)["eco"], "primera");
        assert_eq!(una_linea(&mut la)["eco"], "segunda");
    }

    /// Una línea larguísima sin salto no se lee entera en memoria: se contesta un error y se
    /// cuelga. Se escribe un poco más del tope, lo justo para que el `write` acabe (el servidor
    /// lee hasta el tope y lo que sobra cabe en la tubería) y luego se lee la respuesta: con
    /// E/S síncrona, leer y escribir a la vez en el mismo objeto se pondrían en fila.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn una_linea_demasiado_larga_se_contesta_con_un_error() {
        let nombre = nombre_de_prueba("larga");
        escuchar(&nombre, eco()).unwrap();
        let tuberia = abrir_esperando(&nombre);
        (&tuberia).write_all(&vec![b'a'; LINEA_MAXIMA + 10]).unwrap();
        let mut lector = BufReader::new(&tuberia);
        let respuesta = una_linea(&mut lector);
        assert_eq!(respuesta["ok"], false);
        assert!(respuesta["error"].as_str().unwrap().contains("longer than"));
        // Y después cuelga: no queda nada más que leer.
        let mut resto = String::new();
        assert_eq!(lector.read_line(&mut resto).unwrap_or(0), 0);
    }

    /// La tubería que abre la app es de este usuario, y el cliente lo comprueba.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn la_tuberia_es_de_este_usuario() {
        let nombre = nombre_de_prueba("dueno");
        escuchar(&nombre, eco()).unwrap();
        let tuberia = abrir_esperando(&nombre);
        assert!(dueno_es_este_usuario(&tuberia).unwrap());
    }

    /// Y lo que no es suyo, lo dice: un archivo de Windows es de TrustedInstaller, no suyo.
    /// Es la misma comprobación que haría el cliente con una tubería puesta por otro usuario.
    #[cfg(windows)]
    #[test]
    fn un_objeto_de_otro_dueno_no_pasa_la_comprobacion() {
        let sistema = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let archivo = File::open(format!(r"{sistema}\System32\kernel32.dll")).unwrap();
        assert!(!dueno_es_este_usuario(&archivo).unwrap());
    }

    /// Otro usuario no la puede abrir: con el SID de este usuario apagado en el token (el
    /// resto de grupos siguen), Windows niega el acceso. Ver `abrir_como_otro`.
    #[cfg(windows)]
    #[test]
    fn otro_usuario_no_la_puede_abrir() {
        let nombre = nombre_de_prueba("otro");
        escuchar(&nombre, eco()).unwrap();
        for solo_leer in [false, true] {
            let error = win::para_pruebas::abrir_como_otro(&nombre, solo_leer)
                .expect_err(&format!("otro usuario la ha abierto (solo para leer: {solo_leer})"));
            assert_eq!(error.raw_os_error(), Some(5), "tenía que ser acceso denegado: {error}");
        }
        // Y el de verdad, sí.
        abrir_esperando(&nombre);
    }

    /// Dos servidores con el mismo nombre no se suman: el segundo falla.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn el_segundo_servidor_con_el_mismo_nombre_no_arranca() {
        let nombre = nombre_de_prueba("doble");
        escuchar(&nombre, eco()).unwrap();
        assert!(escuchar(&nombre, eco()).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn el_descriptor_solo_deja_entrar_a_este_usuario() {
        let sid = win::SidPropio::leer().unwrap().texto().unwrap();
        assert!(sid.starts_with("S-1-5-"), "{sid}");
        assert_eq!(win::sddl_para(&sid), format!("O:{sid}D:P(D;;GA;;;NU)(A;;GA;;;{sid})"));
    }

    /// Sin nadie escuchando, abrir dice `NotFound`: es lo que hace que el cliente arranque la app.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn sin_servidor_no_hay_tuberia() {
        let error = abrir(&nombre_de_prueba("nadie")).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    /// Sin la variable, la carpeta de systemd; con una ruta que no es entera, también.
    #[cfg(target_os = "linux")]
    #[test]
    fn la_carpeta_sale_de_la_variable_y_si_no_de_la_de_systemd() {
        use std::path::PathBuf;
        assert_eq!(linux::carpeta_con(Some("/run/user/1000".into()), 7), PathBuf::from("/run/user/1000"));
        assert_eq!(linux::carpeta_con(Some("/tmp/la mía".into()), 7), PathBuf::from("/tmp/la mía"));
        assert_eq!(linux::carpeta_con(None, 1000), PathBuf::from("/run/user/1000"));
        assert_eq!(linux::carpeta_con(Some("".into()), 1000), PathBuf::from("/run/user/1000"));
        assert_eq!(linux::carpeta_con(Some("relativa/x".into()), 0), PathBuf::from("/run/user/0"));
    }

    /// El archivo del socket queda solo para este usuario, sea cual sea la máscara del proceso.
    #[cfg(target_os = "linux")]
    #[test]
    fn el_socket_queda_solo_para_este_usuario() {
        use std::os::unix::fs::PermissionsExt;
        let nombre = nombre_de_prueba("permisos");
        escuchar(&nombre, eco()).unwrap();
        let permisos = std::fs::metadata(&nombre).unwrap().permissions().mode() & 0o777;
        assert_eq!(permisos, 0o600, "{permisos:o}");
    }

    /// En una carpeta en la que pueden entrar otros no se pone el canal, y se dice por qué.
    #[cfg(target_os = "linux")]
    #[test]
    fn en_una_carpeta_que_abren_otros_no_se_escucha() {
        use std::os::unix::fs::PermissionsExt;
        let nombre = nombre_de_prueba("abierta");
        let carpeta = std::path::Path::new(&nombre).parent().unwrap().to_path_buf();
        std::fs::set_permissions(&carpeta, std::fs::Permissions::from_mode(0o755)).unwrap();
        let error = escuchar(&nombre, eco()).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("otros usuarios") && error.to_string().contains("755"), "{error}");
        assert!(!std::path::Path::new(&nombre).exists(), "no ha llegado a poner el socket");
        // Y si la carpeta no es de este usuario, tampoco.
        let error = linux::es_privada(&carpeta, linux::uid().wrapping_add(1)).unwrap_err();
        assert!(error.to_string().contains("otro usuario"), "{error}");
        // Con la carpeta como tiene que estar, sí.
        std::fs::set_permissions(&carpeta, std::fs::Permissions::from_mode(0o700)).unwrap();
        escuchar(&nombre, eco()).unwrap();
    }

    /// El archivo de una app que murió sin recoger no estorba: nadie contesta en él, así que
    /// abrir dice que no hay nadie (y el cliente arranca la app), y la app nueva lo sustituye.
    #[cfg(target_os = "linux")]
    #[test]
    fn el_socket_de_una_app_que_murio_se_sustituye() {
        let nombre = nombre_de_prueba("viejo");
        drop(std::os::unix::net::UnixListener::bind(&nombre).unwrap());
        assert!(std::path::Path::new(&nombre).exists(), "el archivo se queda al morir quien escuchaba");
        assert_eq!(abrir(&nombre).unwrap_err().kind(), std::io::ErrorKind::NotFound);
        escuchar(&nombre, eco()).unwrap();
        let socket = abrir(&nombre).unwrap();
        (&socket).write_all(b"hola\n").unwrap();
        assert_eq!(una_linea(&mut BufReader::new(&socket))["eco"], "hola");
    }

    /// Lo que no es un socket no pasa la comprobación de quién hay al otro lado: da error, y
    /// el cliente no manda nada.
    #[cfg(target_os = "linux")]
    #[test]
    fn lo_que_no_es_un_socket_no_pasa_la_comprobacion() {
        let archivo = File::open("/proc/self/status").unwrap();
        assert!(dueno_es_este_usuario(&archivo).is_err());
    }
}
