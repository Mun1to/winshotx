//! El servidor MCP de winshotx: `winshotx.exe mcp`.
//!
//! MCP (Model Context Protocol) es la forma en que un agente de IA (Claude Code, Codex,
//! Cursor...) descubre herramientas de fuera y las usa. Este servidor habla MCP por la
//! entrada y la salida estándar, en JSON-RPC 2.0 y con un mensaje por línea, y convierte
//! cada herramienta en una petición del canal para agentes (el contrato entre S15 y S16,
//! BUZON del 12 de noviembre de 2026), que manda a la app con `cliente::pedir`. Él no graba
//! ni exporta nada: traduce, espera y devuelve lo que diga la app.
//!
//! Lo que no se ve a simple vista:
//!
//! - **Por la salida estándar solo puede ir MCP.** Un `println!` suelto en este proceso
//!   rompe al cliente. Lo que haya que contar va por `eprintln!`, que el cliente guarda como
//!   registro.
//! - **Las llamadas van a un hilo aparte, y de una en una.** Exportar tarda lo que dura la
//!   grabación, y mientras tanto el cliente puede mandar un `ping` o cancelar: eso lo
//!   contesta enseguida el hilo que lee. De una en una porque la app atiende el canal en
//!   orden, y dos conexiones a la vez solo servirían para que una esperase a la otra.
//! - **Al cerrarse la entrada se terminan las llamadas pendientes antes de salir.** Así
//!   `echo ... | winshotx.exe mcp` contesta todo lo que se le mandó, y no solo lo rápido.
//! - **Las rutas relativas se resuelven aquí**, contra la carpeta en la que el cliente
//!   arrancó este proceso (la del proyecto del agente). La app vive en otra carpeta y, con
//!   una ruta relativa, guardaría el archivo en otro sitio.
//! - **Un argumento mal puesto es un error de la herramienta, no del protocolo.** Así el
//!   modelo lee qué ha hecho mal y lo corrige solo. Solo una herramienta que no existe es
//!   un error de protocolo, como pide la especificación.
//! - **Habla las dos épocas de MCP, petición a petición.** Hasta la 2025-11-25 el cliente
//!   saluda con `initialize` y después pide; desde la 2026-07-28 no hay saludo: cada petición
//!   lleva su versión y lo que sabe hacer el cliente en `params._meta`, y el cliente puede
//!   preguntar antes con `server/discover`. Un cliente que solo hable la nueva no podría usar
//!   un servidor de las de antes («Modern | Legacy: Fails», en `basic/versioning` de la
//!   especificación), así que este atiende las dos: la que trae la clave
//!   `io.modelcontextprotocol/protocolVersion` en su `_meta` va por el camino nuevo, sin
//!   estado y sin saludo, y todo lo demás exactamente como antes. Es como lo deciden los dos
//!   SDK oficiales (el de Python lo explica en `mcp/server/runner.py`: esa clave solo existe
//!   en los mensajes de la 2026-07-28, y un `_meta` a secas no prueba nada, porque los de
//!   antes llevan ahí el `progressToken`).

use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};

/// La revisión de MCP que habla este servidor.
const PROTOCOLO: &str = "2025-06-18";

/// Las revisiones que acepta si el cliente pide una de ellas. Lo que usa este servidor (la
/// lista de herramientas y su resultado en texto) es igual en las dos, y los campos nuevos de
/// la 2025-06-18 (`title`) un cliente viejo los ignora. La 2025-03-26 NO está: obliga al
/// servidor a aceptar lotes de JSON-RPC, y este no los acepta (la 2025-06-18 los quitó). A
/// quien la pida se le contesta con la 2025-06-18, y decide él.
const PROTOCOLOS: &[&str] = &["2025-06-18", "2024-11-05"];

/// Las revisiones de la época sin saludo que habla este servidor. Solo estas van en
/// `supportedVersions` de `server/discover`: las de antes se negocian con `initialize`, y si
/// se anunciaran ahí, los clientes de las dos épocas se irían a `initialize` igual.
const MODERNAS: &[&str] = &["2026-07-28"];

// Las claves de `_meta` de la época sin saludo (`basic/index` de la especificación 2026-07-28).
const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CAPACIDADES: &str = "io.modelcontextprotocol/clientCapabilities";
const META_SERVIDOR: &str = "io.modelcontextprotocol/serverInfo";

/// Cuánto puede guardar un cliente la respuesta de `server/discover` y la de `tools/list`
/// antes de volver a pedirla. Las dos son las mismas para todos y no cambian mientras viva
/// este proceso (una versión nueva de winshotx es otro proceso), así que una hora.
const VIGENCIA_MS: u64 = 3_600_000;

/// Lo que llega a la app por el canal. En la app de verdad es `cliente::pedir`; en las
/// pruebas, uno de mentira.
type Pedir<'a> = dyn Fn(&Value) -> Result<Value, String> + Sync + 'a;

/// `winshotx.exe mcp`: atiende MCP por la entrada y la salida estándar hasta que se cierre
/// la entrada. Devuelve el código de salida del proceso.
pub fn servir() -> i32 {
    let carpeta = std::env::current_dir().unwrap_or_default();
    let entrada = std::io::stdin().lock();
    let mut salida = std::io::stdout();
    servir_con(entrada, &mut salida, &super::cliente::pedir, &carpeta)
}

/// El servidor entero, con la entrada, la salida y el canal que se le den.
fn servir_con<R: BufRead, W: Write + Send>(
    mut entrada: R,
    salida: &mut W,
    pedir: &Pedir<'_>,
    carpeta: &Path,
) -> i32 {
    let salida = Salida { escritor: Mutex::new(salida), rota: AtomicBool::new(false) };
    let cancelados: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    let mut codigo = 0;

    std::thread::scope(|hilos| {
        let (encargos, pendientes) = mpsc::channel::<(Value, Value, bool)>();
        let (salida, cancelados) = (&salida, &cancelados);
        hilos.spawn(move || {
            for (id, peticion, moderna) in pendientes {
                if salida.rota() || quitar(cancelados, &id) {
                    continue;
                }
                // Ojo: esto solo salva un pánico en las pruebas y en `dev`. El perfil de release
                // lleva `panic = "abort"`, así que allí un pánico del canal cierra el proceso
                // entero, y el cliente de MCP lo ve como un servidor que se ha ido.
                let respuesta = match catch_unwind(AssertUnwindSafe(|| pedir(&peticion))) {
                    Ok(respuesta) => respuesta,
                    Err(_) => Err("winshotx failed inside while handling this call.".into()),
                };
                // Si se canceló mientras se hacía, no se contesta (lo pide la especificación).
                if !quitar(cancelados, &id) {
                    salida.escribir(&resultado(id, resultado_de_herramienta(respuesta, moderna)));
                }
            }
        });

        let mut linea = Vec::new();
        let mut iniciado = false;
        loop {
            linea.clear();
            match entrada.read_until(b'\n', &mut linea) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[mcp] no se pudo leer la entrada: {e}");
                    codigo = 1;
                    break;
                }
            }
            match atender(&linea, carpeta, &mut iniciado) {
                Accion::Nada => {}
                Accion::Responder(mensaje) => salida.escribir(&mensaje),
                Accion::Llamar { id, peticion, moderna } => {
                    let _ = encargos.send((id, peticion, moderna));
                }
                Accion::Cancelar(id) => {
                    cancelados.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string());
                }
            }
            if salida.rota() {
                break;
            }
        }
        // Sin más encargos, el hilo termina lo que tenga en cola y se acaba el `scope`.
        drop(encargos);
    });

    if salida.rota() {
        eprintln!("[mcp] el cliente cerró la salida");
        codigo = 1;
    }
    codigo
}

/// La salida estándar, compartida por los dos hilos: un mensaje es una línea entera.
struct Salida<'a, W: Write> {
    escritor: Mutex<&'a mut W>,
    rota: AtomicBool,
}

impl<W: Write> Salida<'_, W> {
    fn escribir(&self, mensaje: &Value) {
        if self.rota() {
            return;
        }
        // `to_string` no deja saltos de línea dentro: los de los textos salen como `\n`.
        let mut linea = mensaje.to_string();
        linea.push('\n');
        let mut escritor = self.escritor.lock().unwrap_or_else(|e| e.into_inner());
        if escritor.write_all(linea.as_bytes()).and_then(|_| escritor.flush()).is_err() {
            self.rota.store(true, Ordering::Relaxed);
        }
    }

    fn rota(&self) -> bool {
        self.rota.load(Ordering::Relaxed)
    }
}

/// Saca un id de los cancelados, y dice si estaba.
fn quitar(cancelados: &Mutex<HashSet<String>>, id: &Value) -> bool {
    cancelados.lock().unwrap_or_else(|e| e.into_inner()).remove(&id.to_string())
}

/// Lo que hay que hacer con una línea recibida.
#[derive(Debug, PartialEq)]
enum Accion {
    /// Una notificación, una respuesta del cliente o una línea en blanco: no se contesta.
    Nada,
    /// Se contesta ya, desde el hilo que lee.
    Responder(Value),
    /// Una herramienta: su petición del contrato va al hilo de las llamadas. `moderna` si llegó
    /// por la época sin saludo, que contesta con lo que esa época pide en cada resultado.
    Llamar { id: Value, peticion: Value, moderna: bool },
    /// El cliente ya no quiere la respuesta de esa petición.
    Cancelar(Value),
}

// Los códigos de error de JSON-RPC 2.0.
const JSON_ROTO: i64 = -32700;
const PETICION_MALA: i64 = -32600;
const METODO_QUE_NO_EXISTE: i64 = -32601;
const PARAMETROS_MALOS: i64 = -32602;
/// El de MCP 2026-07-28 para una versión del protocolo que el servidor no habla.
const VERSION_QUE_NO_SE_HABLA: i64 = -32022;

/// Una línea de la entrada, ya leída, convertida en lo que toca hacer. `iniciado` dice si ya
/// llegó el `initialize`: antes de él, de la época del saludo solo se contestan `ping` y el
/// propio saludo. Lo de la época sin saludo no lo necesita (ver `atender_moderna`).
fn atender(linea: &[u8], carpeta: &Path, iniciado: &mut bool) -> Accion {
    let linea = limpiar(linea);
    if linea.is_empty() {
        return Accion::Nada;
    }
    let mensaje: Value = match serde_json::from_slice(linea) {
        Ok(mensaje) => mensaje,
        Err(e) => {
            return Accion::Responder(error(Value::Null, JSON_ROTO, &format!("Parse error: {e}")))
        }
    };
    if mensaje.is_array() {
        return Accion::Responder(error(
            Value::Null,
            PETICION_MALA,
            "Invalid Request: JSON-RPC batches are not supported; send one message per line.",
        ));
    }
    let Some(mensaje) = mensaje.as_object() else {
        return Accion::Responder(error(
            Value::Null,
            PETICION_MALA,
            "Invalid Request: a message must be a JSON-RPC object.",
        ));
    };
    let metodo = mensaje.get("method").and_then(Value::as_str);
    let params = mensaje.get("params");
    let Some(id) = mensaje.get("id") else {
        // Sin id es una notificación: no se contesta nunca, ni aunque no se conozca.
        if metodo == Some("notifications/cancelled") {
            if let Some(id) = params.and_then(|p| p.get("requestId")) {
                return Accion::Cancelar(id.clone());
            }
        }
        return Accion::Nada;
    };
    if !(id.is_string() || id.is_i64() || id.is_u64()) {
        return Accion::Responder(error(
            Value::Null,
            PETICION_MALA,
            "Invalid Request: the id must be a string or an integer.",
        ));
    }
    let id = id.clone();
    let Some(metodo) = metodo else {
        // Con id y sin método es la respuesta a una petición nuestra, y nunca mandamos ninguna.
        return Accion::Nada;
    };
    if mensaje.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Accion::Responder(error(
            id,
            PETICION_MALA,
            "Invalid Request: add \"jsonrpc\": \"2.0\" to the message.",
        ));
    }
    // `initialize` es siempre de la época del saludo (lo dice `basic/versioning`), lleve lo
    // que lleve en `_meta`. Lo demás, si trae la versión en `_meta`, va por la época sin saludo.
    if metodo != "initialize" {
        match epoca_moderna(&id, params) {
            Some(Ok(())) => return atender_moderna(id, metodo, params, carpeta),
            Some(Err(respuesta)) => return Accion::Responder(respuesta),
            None => {}
        }
    }
    match metodo {
        "initialize" => {
            *iniciado = true;
            Accion::Responder(resultado(id, iniciar(params)))
        }
        "ping" => Accion::Responder(resultado(id, json!({}))),
        // Sin la versión en `_meta` y sin saludo, es un cliente de las de antes que se ha
        // saltado el `initialize`, o uno que no se sabe de qué época es. Contestarle como si
        // hubiera saludado sería adivinar; con el error, saluda. Un `server/discover` sin
        // `_meta` cae abajo y recibe «no existe», que es lo que hace volver a `initialize` a un
        // cliente de las dos épocas. Nada de esto cierra el servidor.
        "tools/list" | "tools/call" if !*iniciado => Accion::Responder(error(
            id,
            PETICION_MALA,
            "Invalid Request: send initialize first (MCP 2025-06-18), or put \
             io.modelcontextprotocol/protocolVersion \"2026-07-28\" and \
             io.modelcontextprotocol/clientCapabilities in params._meta (MCP 2026-07-28).",
        )),
        "tools/list" => Accion::Responder(resultado(id, json!({ "tools": herramientas() }))),
        "tools/call" => llamar(id, params, carpeta, false),
        _ => Accion::Responder(error(
            id,
            METODO_QUE_NO_EXISTE,
            &format!("Method not found: {metodo}. This server only has tools."),
        )),
    }
}

/// Quita el BOM de UTF-8 (lo pone PowerShell al pasar texto a otro programa) y los
/// espacios y el `\r` de los extremos.
fn limpiar(linea: &[u8]) -> &[u8] {
    let linea = linea.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(linea);
    linea.trim_ascii()
}

/// La respuesta a `initialize`.
fn iniciar(params: Option<&Value>) -> Value {
    let pedido = params.and_then(|p| p.get("protocolVersion")).and_then(Value::as_str);
    // Si el cliente pide una que se sabe hablar, se le contesta con la misma; si no, con la
    // última, y es el cliente el que decide si sigue.
    let version = pedido.filter(|v| PROTOCOLOS.contains(v)).unwrap_or(PROTOCOLO);
    json!({
        "protocolVersion": version,
        "capabilities": capacidades(),
        "serverInfo": quien_soy(),
        "instructions": INSTRUCCIONES,
    })
}

/// Lo que sabe hacer este servidor: herramientas, y su lista no cambia.
fn capacidades() -> Value {
    json!({ "tools": { "listChanged": false } })
}

/// Quién es este servidor, para `initialize` y para el `_meta` de la época sin saludo.
fn quien_soy() -> Value {
    json!({
        "name": "winshotx",
        "title": "winshotx",
        "version": env!("CARGO_PKG_VERSION"),
    })
}

/// Si la petición es de la época sin saludo (MCP 2026-07-28 en adelante): `None` si no lo es
/// (no trae la versión en `_meta`), `Some(Ok)` si se puede atender, y `Some(Err)` con el error
/// que pide la especificación si no. Se mira en cada petición: en esa época el servidor no
/// guarda nada de una petición para la siguiente (`basic/index`, «Servers MUST NOT rely on
/// prior requests»).
fn epoca_moderna(id: &Value, params: Option<&Value>) -> Option<Result<(), Value>> {
    let meta =params.and_then(|p| p.get("_meta")).and_then(Value::as_object)?;
    let version = meta.get(META_VERSION)?;
    let Some(version) = version.as_str() else {
        return Some(Err(error(
            id.clone(),
            PARAMETROS_MALOS,
            &format!("Invalid params: _meta[\"{META_VERSION}\"] must be a string, like \"2026-07-28\"."),
        )));
    };
    if !MODERNAS.contains(&version) {
        // Con las versiones de las dos épocas: un cliente que también sepa saludar ve que puede
        // volver a `initialize`.
        let sabidas: Vec<&str> = MODERNAS.iter().chain(PROTOCOLOS).copied().collect();
        let mut respuesta = error(id.clone(), VERSION_QUE_NO_SE_HABLA, "Unsupported protocol version");
        respuesta["error"]["data"] = json!({ "supported": sabidas, "requested": version });
        return Some(Err(respuesta));
    }
    if !meta.get(META_CAPACIDADES).is_some_and(Value::is_object) {
        return Some(Err(error(
            id.clone(),
            PARAMETROS_MALOS,
            &format!("Invalid params: _meta[\"{META_CAPACIDADES}\"] is required on every request ({{}} if the client has none)."),
        )));
    }
    Some(Ok(()))
}

/// Una petición de la época sin saludo, ya comprobada: se contesta sin mirar si hubo
/// `initialize`. En esa época ya no existen `initialize`, `ping` ni `logging/setLevel`
/// (`changelog` de la 2026-07-28), así que lo que no sea esto es un método que no existe.
fn atender_moderna(id: Value, metodo: &str, params: Option<&Value>, carpeta: &Path) -> Accion {
    match metodo {
        "server/discover" => Accion::Responder(resultado(
            id,
            moderno(json!({
                "supportedVersions": MODERNAS,
                "capabilities": capacidades(),
                "instructions": INSTRUCCIONES,
                "ttlMs": VIGENCIA_MS,
                "cacheScope": "public",
            })),
        )),
        // Igual para todos y en el mismo orden siempre: por eso «public» (`server/utilities/caching`).
        "tools/list" => Accion::Responder(resultado(
            id,
            moderno(json!({ "tools": herramientas(), "ttlMs": VIGENCIA_MS, "cacheScope": "public" })),
        )),
        "tools/call" => llamar(id, params, carpeta, true),
        _ => Accion::Responder(error(
            id,
            METODO_QUE_NO_EXISTE,
            &format!("Method not found: {metodo}. This server only has tools."),
        )),
    }
}

/// Lo que lleva todo resultado de la época sin saludo: `resultType` (obligatorio) y quién
/// contesta en su `_meta` (recomendado). En la época del saludo no se pone nada de esto.
fn moderno(mut resultado: Value) -> Value {
    resultado["resultType"] = json!("complete");
    resultado["_meta"] = json!({ META_SERVIDOR: quien_soy() });
    resultado
}

/// Lo que se le cuenta al agente al conectarse, antes de que mire las herramientas.
///
/// En Linux es otro texto, porque allí no es verdad que siempre se vean un marco y una barra
/// (no se pueden esconder del vídeo, así que solo están si caben fuera de lo que se graba; en
/// Wayland avisa el escritorio), ni que haya lista de ventanas en Wayland.
#[cfg(target_os = "linux")]
const INSTRUCCIONES: &str = "winshotx records and captures the screen on Linux, on this \
computer. To make a video: pick what to record with screens or windows, call record_start, \
do what you want to show, call record_stop, then export to an .mp4 or .gif file (or \
open_in_editor so the person can polish it). For a screenshot, call capture. Coordinates \
are physical pixels of the virtual desktop, the same ones screens and windows return. On \
Wayland there is no window list (use a screen or a region), and the first recording or \
screenshot makes the desktop ask the person for permission: that call waits for their \
answer. A recording is visible to the person at the computer: on X11, winshotx's frame \
around the recorded area and a small bar when they fit outside it (recording a whole screen \
shows neither, only winshotx's tray menu changes); on Wayland, the desktop's own screen \
sharing indicator. Nothing plays a sound unless you pass cue: true. The screen can show \
private things: record and capture only what the person asked for. If winshotx is not \
running, the first call starts it in the background and takes a few seconds longer.";

/// Lo que se le cuenta al agente al conectarse, antes de que mire las herramientas.
#[cfg(not(target_os = "linux"))]
const INSTRUCCIONES: &str = "winshotx records and captures the screen on Windows, on this \
computer. To make a video: pick what to record with screens or windows, call record_start, \
do what you want to show, call record_stop, then export to an .mp4 or .gif file (or \
open_in_editor so the person can polish it). For a screenshot, call capture. Coordinates \
are physical pixels of the virtual desktop, the same ones screens and windows return. A \
recording is always visible to the person at the computer (a frame and a small bar), \
and nothing plays a sound unless you pass cue: true. The screen can show private things: \
record and capture only what the person asked for. If winshotx is not running, the first \
call starts it in the background and takes a few seconds longer.";

/// Lo que ve quien está delante mientras graba un agente, para la descripción de `record_start`.
#[cfg(not(target_os = "linux"))]
const LO_QUE_SE_VE: &str = "The person at the computer sees winshotx's usual frame and recording bar, on purpose.";

/// En Linux, lo que de verdad se ve (ver `INSTRUCCIONES`).
#[cfg(target_os = "linux")]
const LO_QUE_SE_VE: &str = "The person at the computer can see it, on purpose: on X11, winshotx's frame and recording bar when they fit outside the recorded area; on Wayland, the desktop's screen sharing indicator, and the first time the desktop asks them for permission and this call waits for their answer.";

/// Una respuesta buena de JSON-RPC.
fn resultado(id: Value, resultado: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": resultado })
}

/// Una respuesta de error de JSON-RPC. Con `id` nulo (un JSON roto, un mensaje sin id
/// válido) se deja el id fuera en vez de poner `"id": null`: JSON-RPC pide el nulo, pero
/// MCP no lo admite como id y el SDK de TypeScript descarta ese mensaje; la revisión
/// 2025-11-25 de MCP ya permite omitirlo.
fn error(id: Value, codigo: i64, mensaje: &str) -> Value {
    let mut respuesta = json!({ "jsonrpc": "2.0", "error": { "code": codigo, "message": mensaje } });
    if !id.is_null() {
        respuesta["id"] = id;
    }
    respuesta
}

/// `tools/call`: comprueba la herramienta y sus argumentos y la manda al hilo de las llamadas.
/// `moderna` si llegó por la época sin saludo.
fn llamar(id: Value, params: Option<&Value>, carpeta: &Path, moderna: bool) -> Accion {
    let nombre = params.and_then(|p| p.get("name")).and_then(Value::as_str);
    let Some(nombre) = nombre.filter(|n| NOMBRES.contains(n)) else {
        let dicho = nombre.map(|n| format!("Unknown tool: {n}.")).unwrap_or_else(|| {
            "tools/call needs params.name.".into()
        });
        return Accion::Responder(error(
            id,
            PARAMETROS_MALOS,
            &format!("{dicho} The tools are: {}.", NOMBRES.join(", ")),
        ));
    };
    let vacio = Map::new();
    let argumentos = match params.and_then(|p| p.get("arguments")) {
        None | Some(Value::Null) => &vacio,
        Some(Value::Object(argumentos)) => argumentos,
        Some(_) => {
            return Accion::Responder(error(
                id,
                PARAMETROS_MALOS,
                "params.arguments must be a JSON object.",
            ))
        }
    };
    match traducir(nombre, argumentos, carpeta) {
        Ok(peticion) => Accion::Llamar { id, peticion, moderna },
        Err(mensaje) => Accion::Responder(resultado(id, resultado_de_herramienta(Err(mensaje), moderna))),
    }
}

/// El resultado de una herramienta: el JSON de la app como texto, y `isError` si no salió.
/// Un fallo del canal (la app no arranca, por ejemplo) se cuenta con la misma forma que un
/// fallo de la app, `{"ok":false,"error":...}`, para que el agente lea siempre lo mismo. En la
/// época sin saludo (`moderna`), con lo que esa época pide en cada resultado.
fn resultado_de_herramienta(respuesta: Result<Value, String>, moderna: bool) -> Value {
    let (cuerpo, fallo) = match respuesta {
        Ok(cuerpo) => {
            let bien = cuerpo.get("ok") == Some(&Value::Bool(true));
            (cuerpo, !bien)
        }
        Err(mensaje) => (json!({ "ok": false, "error": mensaje }), true),
    };
    let resultado = json!({
        "content": [{ "type": "text", "text": cuerpo.to_string() }],
        "isError": fallo,
    });
    if moderna {
        moderno(resultado)
    } else {
        resultado
    }
}

/// Los nombres de las herramientas, en el orden de la lista.
const NOMBRES: &[&str] = &[
    "status",
    "screens",
    "windows",
    "capture",
    "record_start",
    "record_stop",
    "export",
    "open_in_editor",
];

/// Lo que se elige para capturar o grabar: uno de los cuatro.
fn objetivo() -> Map<String, Value> {
    let objetivo = json!({
        "screen": {
            "type": "integer",
            "minimum": 0,
            "description": "A whole screen: its index from the screens tool."
        },
        "windowId": {
            "type": "integer",
            "description": "One window: its id from the windows tool. Only that window is \
                recorded, even if another one passes in front."
        },
        "windowTitle": {
            "type": "string",
            "description": "One window, by part of its title (case does not matter), when \
                you do not have its id."
        },
        "region": {
            "type": "object",
            "description": "A rectangle of the desktop, in physical pixels of the virtual \
                desktop (the coordinates screens returns; x and y can be negative on a \
                screen left of or above the primary one).",
            "properties": {
                "x": { "type": "integer" },
                "y": { "type": "integer" },
                "width": { "type": "integer", "minimum": 1 },
                "height": { "type": "integer", "minimum": 1 }
            },
            "required": ["x", "y", "width", "height"],
            "additionalProperties": false
        }
    });
    let Value::Object(objetivo) = objetivo else { unreachable!() };
    objetivo
}

/// Un `inputSchema` con estas propiedades y estas obligatorias.
fn esquema(mut propiedades: Map<String, Value>, extra: Value, obligatorias: &[&str]) -> Value {
    if let Value::Object(extra) = extra {
        propiedades.extend(extra);
    }
    let mut esquema = json!({
        "type": "object",
        "properties": propiedades,
        "additionalProperties": false,
    });
    if !obligatorias.is_empty() {
        esquema["required"] = json!(obligatorias);
    }
    esquema
}

/// La lista de herramientas, tal cual la ve el agente.
fn herramientas() -> Value {
    let solo_mirar = json!({
        "readOnlyHint": true,
        "idempotentHint": true,
        "openWorldHint": false
    });
    let hace_algo = |destructiva: bool| {
        json!({
            "readOnlyHint": false,
            "destructiveHint": destructiva,
            "idempotentHint": false,
            "openWorldHint": false
        })
    };
    let target = "Pass exactly one target: screen, windowId, windowTitle or region.";
    json!([
        {
            "name": "status",
            "title": "winshotx status",
            "description": "Check that winshotx is running and whether it is recording \
                right now. Returns {ok, version, recording, session}. Use it when you are not \
                sure a recording was stopped, or after a recording with maxSeconds to see if \
                it already ended.",
            "inputSchema": esquema(Map::new(), json!({}), &[]),
            "annotations": solo_mirar,
        },
        {
            "name": "screens",
            "title": "List screens",
            "description": "List the screens (monitors) with their index, position and size \
                in physical pixels of the virtual desktop, their scale factor and which one \
                is primary. Use the index as screen in capture or record_start, and these \
                coordinates to build a region.",
            "inputSchema": esquema(Map::new(), json!({}), &[]),
            "annotations": solo_mirar,
        },
        {
            "name": "windows",
            "title": "List windows",
            "description": "List the visible windows (not winshotx's own) with their id, \
                title, process name and bounds in physical pixels. Use the id as windowId, or \
                part of the title as windowTitle, in capture or record_start.",
            "inputSchema": esquema(Map::new(), json!({}), &[]),
            "annotations": solo_mirar,
        },
        {
            "name": "capture",
            "title": "Take a screenshot",
            "description": format!("Take a screenshot of a screen, a window or a region and \
                save it as a PNG file, with no overlay and no sound. {target} Returns {{ok, \
                file, width, height}}: file is the absolute path of the PNG. Use it to show \
                or check what is on screen; to show something happening, record instead."),
            "inputSchema": esquema(objetivo(), json!({
                "output": {
                    "type": "string",
                    "description": "Where to save the PNG (ends in .png). A relative path is \
                        taken from your working directory. Without it, the file goes to the \
                        screenshots folder in winshotx's settings."
                }
            }), &[]),
            "annotations": hace_algo(false),
        },
        {
            "name": "record_start",
            "title": "Start recording the screen",
            "description": format!("Start recording a screen, a window or a region. \
                {target} It returns once the recording has really started (after the webcam \
                warms up, if it is on) with {{ok, session}}; keep the session id for export. \
                {LO_QUE_SE_VE} Only one recording can run at a time. Then do what you want to \
                show and call record_stop."),
            "inputSchema": esquema(objetivo(), json!({
                "camera": {
                    "type": "boolean",
                    "description": "Record the webcam too. Default: what winshotx's \
                        settings say."
                },
                "systemAudio": {
                    "type": "boolean",
                    "description": "Record the computer's sound. Default: winshotx's settings."
                },
                "microphone": {
                    "type": "boolean",
                    "description": "Record the microphone. Default: winshotx's settings."
                },
                "cue": {
                    "type": "boolean",
                    "description": "Play winshotx's short start and stop sound. Default: \
                        false, so nothing makes a sound unless you ask."
                },
                "format": {
                    "type": "string",
                    "enum": ["mp4", "gif"],
                    "description": "What the recording is meant to become. Default: mp4."
                },
                "maxSeconds": {
                    "type": "number",
                    "minimum": 1,
                    "description": "Stop by itself after this many seconds, as a safety \
                        limit. Call record_stop afterwards anyway: it waits until the \
                        recording is saved."
                }
            }), &[]),
            "annotations": hace_algo(false),
        },
        {
            "name": "record_stop",
            "title": "Stop recording",
            "description": "Stop the current recording and save it, without opening the \
                editor. Returns {ok, session, seconds} once it is saved. Then call export to \
                get an MP4 or GIF file, or open_in_editor so the person can edit it.",
            "inputSchema": esquema(Map::new(), json!({}), &[]),
            "annotations": hace_algo(false),
        },
        {
            "name": "export",
            "title": "Export a recording to MP4 or GIF",
            "description": "Turn a saved recording into an MP4 or GIF file with winshotx's \
                look (the drawn cursor, click rings, zoom on clicks, background). The format \
                comes from the extension of output. It waits until the file is written and \
                returns {ok, file, seconds, bytes}; it can take about as long as the \
                recording itself. Without style, it uses the same defaults the editor opens \
                with.",
            "inputSchema": esquema(Map::new(), json!({
                "session": {
                    "type": "string",
                    "description": "The session id from record_start or record_stop."
                },
                "output": {
                    "type": "string",
                    "description": "Where to write the video: a path ending in .mp4 or .gif. \
                        A relative path is taken from your working directory."
                },
                "style": {
                    "type": "string",
                    "description": "The name of a style the person saved in winshotx's \
                        editor (background, cursor, zoom...). Only use one they told you about."
                },
                "trim": {
                    "type": "object",
                    "description": "Keep only this part of the recording, in seconds from \
                        its start.",
                    "properties": {
                        "start": { "type": "number", "minimum": 0 },
                        "end": { "type": "number", "minimum": 0 }
                    },
                    "required": ["start", "end"],
                    "additionalProperties": false
                },
                "zoomOnClicks": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 1,
                    "description": "How much the video zooms in on each click: 0 is no zoom, \
                        1 the most."
                },
                "speed": {
                    "type": "number",
                    "description": "Playback speed: 1 is real time, 2 twice as fast (the \
                        editor offers 0.5 to 4)."
                }
            }), &["session", "output"]),
            "annotations": hace_algo(false),
        },
        {
            "name": "open_in_editor",
            "title": "Open a recording in the editor",
            "description": "Open a saved recording in winshotx's editor window, so the \
                person can trim, zoom and style it by hand and save it themselves. Use it when \
                they want to review or polish the video; it does not write any file.",
            "inputSchema": esquema(Map::new(), json!({
                "session": {
                    "type": "string",
                    "description": "The session id from record_start or record_stop."
                }
            }), &["session"]),
            "annotations": hace_algo(false),
        }
    ])
}

/// Los argumentos de una herramienta convertidos en su petición del contrato. Las
/// propiedades que acepta cada una salen de su propio `inputSchema`: si se añade una ahí y
/// no aquí, o al revés, lo dice la prueba `cada_propiedad_del_esquema_llega_a_la_peticion`.
fn traducir(nombre: &str, args: &Map<String, Value>, carpeta: &Path) -> Result<Value, String> {
    let lista = herramientas();
    let permitidas: Vec<String> = lista
        .as_array()
        .and_then(|lista| lista.iter().find(|h| h["name"] == nombre))
        .and_then(|h| h["inputSchema"]["properties"].as_object())
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default();
    if let Some(sobra) = args.keys().find(|clave| !permitidas.contains(clave)) {
        return Err(if permitidas.is_empty() {
            format!("{nombre} takes no arguments, but got \"{sobra}\".")
        } else {
            format!("Unknown argument \"{sobra}\" for {nombre}. It takes: {}.", permitidas.join(", "))
        });
    }

    let mut p = Map::new();
    let orden = match nombre {
        "status" | "screens" | "windows" => nombre,
        "capture" => {
            poner_objetivo(args, &mut p)?;
            if let Some(ruta) = texto(args, "output")? {
                p.insert("output".into(), json!(ruta_de_salida(&ruta, carpeta, &["png"])?));
            }
            "capture"
        }
        "record_start" => {
            poner_objetivo(args, &mut p)?;
            for clave in ["camera", "systemAudio", "microphone", "cue"] {
                if let Some(valor) = booleano(args, clave)? {
                    p.insert(clave.into(), json!(valor));
                }
            }
            if let Some(formato) = texto(args, "format")? {
                let formato = formato.to_ascii_lowercase();
                if formato != "mp4" && formato != "gif" {
                    return Err("format must be \"mp4\" or \"gif\".".into());
                }
                p.insert("format".into(), json!(formato));
            }
            copiar_numero(args, &mut p, "maxSeconds", |s| s >= 1.0, "a number of seconds, 1 or more")?;
            "record"
        }
        "record_stop" => "stop",
        "export" => {
            p.insert("session".into(), json!(obligatorio(texto(args, "session")?, "session")?));
            let ruta = obligatorio(texto(args, "output")?, "output")?;
            p.insert("output".into(), json!(ruta_de_salida(&ruta, carpeta, &["mp4", "gif"])?));
            if let Some(estilo) = texto(args, "style")? {
                p.insert("style".into(), json!(estilo));
            }
            if let Some(trim) = args.get("trim") {
                p.insert("trim".into(), recorte(trim)?);
            }
            copiar_numero(args, &mut p, "zoomOnClicks", |z| (0.0..=1.0).contains(&z), "a number from 0 to 1")?;
            copiar_numero(args, &mut p, "speed", |v| v > 0.0, "a number above 0, like 2 for twice as fast")?;
            "export"
        }
        "open_in_editor" => {
            p.insert("session".into(), json!(obligatorio(texto(args, "session")?, "session")?));
            "open"
        }
        _ => return Err(format!("Unknown tool: {nombre}.")),
    };
    p.insert("command".into(), json!(orden));
    Ok(Value::Object(p))
}

/// El objetivo de `capture` y `record_start`, con el nombre del contrato: `screen`,
/// `window` (el id o el trozo del título) o `region`.
fn poner_objetivo(args: &Map<String, Value>, p: &mut Map<String, Value>) -> Result<(), String> {
    let mut puestos = Vec::new();
    if let Some(pantalla) = entero(args, "screen")? {
        if pantalla < 0 {
            return Err("screen is an index from the screens tool: 0 or more.".into());
        }
        p.insert("screen".into(), args["screen"].clone());
        puestos.push("screen");
    }
    if entero(args, "windowId")?.is_some() {
        p.insert("window".into(), args["windowId"].clone());
        puestos.push("windowId");
    }
    if let Some(titulo) = texto(args, "windowTitle")? {
        p.insert("window".into(), json!(titulo));
        puestos.push("windowTitle");
    }
    if let Some(region) = args.get("region") {
        p.insert("region".into(), rectangulo(region)?);
        puestos.push("region");
    }
    match puestos.len() {
        1 => Ok(()),
        0 => Err("Say what to capture: pass screen (an index from the screens tool), windowId \
            or windowTitle (from the windows tool), or region {x, y, width, height}."
            .into()),
        _ => Err(format!("Pass only one target, but got {}.", puestos.join(" and "))),
    }
}

fn rectangulo(region: &Value) -> Result<Value, String> {
    let mal = || "region must be {x, y, width, height}, in whole physical pixels, with width \
        and height above 0."
        .to_string();
    let region = region.as_object().ok_or_else(mal)?;
    if region.keys().any(|k| !["x", "y", "width", "height"].contains(&k.as_str())) {
        return Err(mal());
    }
    let mut salida = Map::new();
    for clave in ["x", "y", "width", "height"] {
        let valor = region.get(clave).ok_or_else(mal)?;
        let numero = como_entero(valor).ok_or_else(mal)?;
        if (clave == "width" || clave == "height") && numero < 1 {
            return Err(mal());
        }
        salida.insert(clave.into(), valor.clone());
    }
    Ok(Value::Object(salida))
}

fn recorte(trim: &Value) -> Result<Value, String> {
    let mal = || "trim must be {start, end} in seconds, with end after start.".to_string();
    let trim = trim.as_object().ok_or_else(mal)?;
    if trim.keys().any(|k| k != "start" && k != "end") {
        return Err(mal());
    }
    let desde = trim.get("start").and_then(Value::as_f64).ok_or_else(mal)?;
    let hasta = trim.get("end").and_then(Value::as_f64).ok_or_else(mal)?;
    if desde < 0.0 || hasta <= desde {
        return Err(mal());
    }
    Ok(json!({ "start": trim["start"], "end": trim["end"] }))
}

/// La ruta de un archivo que va a escribir la app: absoluta, y con una de esas extensiones.
fn ruta_de_salida(ruta: &str, carpeta: &Path, extensiones: &[&str]) -> Result<String, String> {
    let ruta = PathBuf::from(ruta);
    let extension = ruta.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if !extension.is_some_and(|e| extensiones.contains(&e.as_str())) {
        let lista: Vec<String> = extensiones.iter().map(|e| format!(".{e}")).collect();
        return Err(format!("output must end in {}.", lista.join(" or ")));
    }
    let ruta = if ruta.is_absolute() || carpeta.as_os_str().is_empty() {
        ruta
    } else {
        carpeta.join(ruta)
    };
    Ok(ruta.to_string_lossy().into_owned())
}

fn obligatorio(valor: Option<String>, clave: &str) -> Result<String, String> {
    valor.ok_or_else(|| format!("{clave} is required."))
}

fn texto(args: &Map<String, Value>, clave: &str) -> Result<Option<String>, String> {
    match args.get(clave) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(Some(s.trim().to_string())),
        Some(Value::String(_)) => Err(format!("{clave} must not be empty.")),
        Some(_) => Err(format!("{clave} must be a string.")),
    }
}

fn booleano(args: &Map<String, Value>, clave: &str) -> Result<Option<bool>, String> {
    match args.get(clave) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("{clave} must be true or false.")),
    }
}

fn entero(args: &Map<String, Value>, clave: &str) -> Result<Option<i64>, String> {
    match args.get(clave) {
        None | Some(Value::Null) => Ok(None),
        Some(valor) => como_entero(valor)
            .map(Some)
            .ok_or_else(|| format!("{clave} must be a whole number.")),
    }
}

/// Un número entero, también si llega como `3.0` (algunos clientes mandan así los enteros).
/// Los ids de ventana caben en un `u64`, así que también se aceptan los que no caben en `i64`.
fn como_entero(valor: &Value) -> Option<i64> {
    if let Some(n) = valor.as_i64() {
        return Some(n);
    }
    if valor.is_u64() {
        return Some(i64::MAX);
    }
    let f = valor.as_f64()?;
    (f.fract() == 0.0 && f.abs() < 9.0e15).then_some(f as i64)
}

/// Copia un número tal cual llegó (sin pasarlo por `f64`, que cambia `30` por `30.0`) si
/// cumple la condición.
fn copiar_numero(
    args: &Map<String, Value>,
    p: &mut Map<String, Value>,
    clave: &str,
    vale: impl Fn(f64) -> bool,
    que_es: &str,
) -> Result<(), String> {
    match args.get(clave) {
        None | Some(Value::Null) => Ok(()),
        Some(valor) => match valor.as_f64() {
            Some(n) if vale(n) => {
                p.insert(clave.into(), valor.clone());
                Ok(())
            }
            _ => Err(format!("{clave} must be {que_es}.")),
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Cursor;
    use std::time::Duration;

    /// Un canal de mentira que apunta lo que le piden y contesta con `respuesta`.
    struct Canal {
        pedido: Mutex<Vec<Value>>,
        respuesta: Box<dyn Fn(&Value) -> Result<Value, String> + Sync>,
    }

    impl Canal {
        fn que_contesta(respuesta: impl Fn(&Value) -> Result<Value, String> + Sync + 'static) -> Self {
            Canal { pedido: Mutex::new(Vec::new()), respuesta: Box::new(respuesta) }
        }
        fn pedir(&self, peticion: &Value) -> Result<Value, String> {
            self.pedido.lock().unwrap().push(peticion.clone());
            (self.respuesta)(peticion)
        }
        fn pedido(&self) -> Vec<Value> {
            self.pedido.lock().unwrap().clone()
        }
    }

    fn carpeta() -> PathBuf {
        PathBuf::from(if cfg!(windows) { r"C:\proyecto" } else { "/proyecto" })
    }

    /// Pasa estas líneas por el servidor entero y devuelve cada línea que escribió, leída.
    fn conversar(lineas: &[&str], canal: &Canal) -> (i32, Vec<Value>) {
        let entrada = lineas.iter().map(|l| format!("{l}\n")).collect::<String>();
        conversar_bytes(entrada.into_bytes(), canal)
    }

    fn conversar_bytes(entrada: Vec<u8>, canal: &Canal) -> (i32, Vec<Value>) {
        let mut salida = Vec::new();
        let pedir = |p: &Value| canal.pedir(p);
        let codigo = servir_con(Cursor::new(entrada), &mut salida, &pedir, &carpeta());
        let texto = String::from_utf8(salida).expect("la salida es UTF-8");
        assert!(texto.is_empty() || texto.ends_with('\n'), "cada mensaje acaba su línea");
        let mensajes = texto
            .lines()
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("línea que no es JSON: {l} ({e})")))
            .collect();
        (codigo, mensajes)
    }

    /// Como `conversar`, pero saludando antes, como hace un cliente de verdad. La respuesta al
    /// saludo no se devuelve.
    fn conversar_iniciado(lineas: &[&str], canal: &Canal) -> (i32, Vec<Value>) {
        let mut todas = vec![INICIO, INICIADO];
        todas.extend_from_slice(lineas);
        let (codigo, mensajes) = conversar(&todas, canal);
        (codigo, mensajes.into_iter().filter(|m| m["id"] != json!("init")).collect())
    }

    fn con_id(mensajes: &[Value], id: Value) -> Value {
        let con: Vec<&Value> = mensajes.iter().filter(|m| m["id"] == id).collect();
        assert_eq!(con.len(), 1, "una sola respuesta con id {id}: {mensajes:?}");
        con[0].clone()
    }

    /// El JSON que devolvió una herramienta, leído de su bloque de texto.
    fn texto_de(respuesta: &Value) -> Value {
        let contenido = &respuesta["result"]["content"];
        assert_eq!(contenido.as_array().map(Vec::len), Some(1), "{respuesta}");
        assert_eq!(contenido[0]["type"], "text");
        serde_json::from_str(contenido[0]["text"].as_str().unwrap()).unwrap()
    }

    fn llamada(id: i64, nombre: &str, argumentos: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
                "params": { "name": nombre, "arguments": argumentos } })
        .to_string()
    }

    const INICIO: &str = r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"prueba","version":"1"}}}"#;
    const INICIADO: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;

    fn sin_canal() -> Canal {
        Canal::que_contesta(|_| panic!("esta prueba no debería llegar a la app"))
    }

    #[test]
    fn el_saludo_dice_quien_es_y_que_sabe_hacer() {
        let canal = sin_canal();
        let ping = r#"{"jsonrpc":"2.0","id":"p","method":"ping"}"#;
        let (codigo, mensajes) = conversar(&[INICIO, INICIADO, ping], &canal);
        assert_eq!(codigo, 0);
        assert_eq!(mensajes.len(), 2, "la notificación no se contesta: {mensajes:?}");
        let inicio = con_id(&mensajes, json!("init"));
        assert_eq!(inicio["jsonrpc"], "2.0");
        let r = &inicio["result"];
        assert_eq!(r["protocolVersion"], "2025-06-18");
        assert_eq!(r["serverInfo"]["name"], "winshotx");
        assert_eq!(r["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(r["capabilities"]["tools"]["listChanged"], false);
        assert!(r["instructions"].as_str().unwrap().contains("record_start"));
        assert_eq!(con_id(&mensajes, json!("p"))["result"], json!({}));
    }

    #[test]
    fn la_version_del_protocolo_se_negocia() {
        let pedir = |v: &str| {
            let r = iniciar(Some(&json!({ "protocolVersion": v })));
            r["protocolVersion"].as_str().unwrap().to_string()
        };
        assert_eq!(pedir("2024-11-05"), "2024-11-05");
        assert_eq!(pedir("2025-03-26"), "2025-06-18", "la 2025-03-26 obliga a aceptar lotes");
        assert_eq!(pedir("2099-01-01"), "2025-06-18");
        assert_eq!(iniciar(None)["protocolVersion"], "2025-06-18");
    }

    #[test]
    fn la_lista_trae_las_ocho_herramientas_bien_descritas() {
        let canal = sin_canal();
        let lista = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
        let (_, mensajes) = conversar(&[INICIO, INICIADO, lista], &canal);
        let herramientas = con_id(&mensajes, json!(2))["result"]["tools"].clone();
        let nombres: Vec<&str> =
            herramientas.as_array().unwrap().iter().map(|h| h["name"].as_str().unwrap()).collect();
        assert_eq!(
            nombres,
            ["status", "screens", "windows", "capture", "record_start", "record_stop", "export", "open_in_editor"]
        );
        assert_eq!(nombres, NOMBRES);
        for h in herramientas.as_array().unwrap() {
            let nombre = h["name"].as_str().unwrap();
            let descripcion = h["description"].as_str().unwrap();
            assert!(descripcion.len() > 80, "{nombre}: la descripción dice cuándo usarla");
            assert!(!descripcion.contains("  "), "{nombre}: sin espacios dobles de las líneas partidas");
            assert!(h["title"].is_string(), "{nombre}");
            let esquema = &h["inputSchema"];
            assert_eq!(esquema["type"], "object", "{nombre}");
            assert_eq!(esquema["additionalProperties"], false, "{nombre}");
            for (clave, propiedad) in esquema["properties"].as_object().unwrap() {
                assert!(propiedad["type"].is_string(), "{nombre}.{clave} lleva un solo tipo");
            }
            for obligatoria in esquema["required"].as_array().into_iter().flatten() {
                assert!(esquema["properties"].get(obligatoria.as_str().unwrap()).is_some());
            }
            assert_eq!(h["annotations"]["openWorldHint"], false, "{nombre}");
        }
    }

    #[test]
    fn grabar_manda_su_peticion_y_devuelve_lo_que_dice_la_app() {
        let canal = Canal::que_contesta(|_| Ok(json!({ "ok": true, "session": "s-123" })));
        let grabar = llamada(3, "record_start", json!({ "screen": 0, "camera": true, "maxSeconds": 30 }));
        let (_, mensajes) = conversar(&[INICIO, INICIADO, &grabar], &canal);
        assert_eq!(
            canal.pedido(),
            [json!({ "command": "record", "screen": 0, "camera": true, "maxSeconds": 30 })]
        );
        let respuesta = con_id(&mensajes, json!(3));
        assert_eq!(respuesta["result"]["isError"], false);
        assert_eq!(texto_de(&respuesta), json!({ "ok": true, "session": "s-123" }));
    }

    #[test]
    fn cada_herramienta_se_traduce_al_contrato() {
        // Las rutas enteras de cada sistema; una relativa se junta con la carpeta de quien pide.
        let (fotos, shots, demo, gif) = if cfg!(windows) {
            (r"D:\fotos\a.PNG", r"C:\proyecto\shots/a.png", r"C:\proyecto\demo.mp4", r"C:\videos\demo.gif")
        } else {
            ("/fotos/a.PNG", "/proyecto/shots/a.png", "/proyecto/demo.mp4", "/videos/demo.gif")
        };
        let casos = [
            ("status", json!({}), json!({ "command": "status" })),
            ("screens", json!({}), json!({ "command": "screens" })),
            ("windows", json!({}), json!({ "command": "windows" })),
            ("capture", json!({ "windowTitle": " Edge " }), json!({ "command": "capture", "window": "Edge" })),
            (
                "capture",
                json!({ "windowId": 9_007_199_254_740_993u64, "output": fotos }),
                json!({ "command": "capture", "window": 9_007_199_254_740_993u64, "output": fotos }),
            ),
            (
                "capture",
                json!({ "region": { "x": -1920, "y": 0, "width": 800, "height": 600 }, "output": "shots/a.png" }),
                json!({ "command": "capture", "region": { "x": -1920, "y": 0, "width": 800, "height": 600 },
                        "output": shots }),
            ),
            (
                "record_start",
                json!({ "windowId": 4242, "systemAudio": false, "microphone": true, "cue": true, "format": "GIF" }),
                json!({ "command": "record", "window": 4242, "systemAudio": false, "microphone": true,
                        "cue": true, "format": "gif" }),
            ),
            ("record_stop", json!({}), json!({ "command": "stop" })),
            (
                "export",
                json!({ "session": "s-1", "output": "demo.mp4", "style": "Tutorial",
                        "trim": { "start": 1.5, "end": 9 }, "zoomOnClicks": 0.5, "speed": 2 }),
                json!({ "command": "export", "session": "s-1", "output": demo,
                        "style": "Tutorial", "trim": { "start": 1.5, "end": 9 }, "zoomOnClicks": 0.5,
                        "speed": 2 }),
            ),
            (
                "export",
                json!({ "session": "s-1", "output": gif }),
                json!({ "command": "export", "session": "s-1", "output": gif }),
            ),
            ("open_in_editor", json!({ "session": "s-1" }), json!({ "command": "open", "session": "s-1" })),
        ];
        for (nombre, args, esperada) in casos {
            let args = args.as_object().unwrap().clone();
            assert_eq!(traducir(nombre, &args, &carpeta()), Ok(esperada), "{nombre} {args:?}");
        }
    }

    #[test]
    fn cada_propiedad_del_esquema_llega_a_la_peticion() {
        // Un valor válido para cada propiedad que anuncia la lista: si una se anuncia y la
        // traducción la tira o la rechaza, el agente la pondría para nada.
        let valido = |clave: &str| match clave {
            "screen" => json!(1),
            "windowId" => json!(77),
            "windowTitle" => json!("Notepad"),
            "region" => json!({ "x": 0, "y": 0, "width": 10, "height": 10 }),
            "output" => json!(r"C:\a\b.gif"),
            "camera" | "systemAudio" | "microphone" | "cue" => json!(true),
            "format" => json!("gif"),
            "maxSeconds" | "speed" => json!(2),
            "session" => json!("s"),
            "style" => json!("Mío"),
            "trim" => json!({ "start": 0, "end": 1 }),
            "zoomOnClicks" => json!(1),
            otra => panic!("propiedad sin valor de prueba: {otra}"),
        };
        let objetivos = ["screen", "windowId", "windowTitle", "region"];
        for h in herramientas().as_array().unwrap() {
            let nombre = h["name"].as_str().unwrap();
            for (clave, _) in h["inputSchema"]["properties"].as_object().unwrap() {
                let mut args = Map::new();
                args.insert(clave.clone(), valido(clave));
                if !objetivos.contains(&clave.as_str()) && h["inputSchema"]["properties"].get("screen").is_some() {
                    args.insert("screen".into(), json!(0));
                }
                for obligatoria in h["inputSchema"]["required"].as_array().into_iter().flatten() {
                    let obligatoria = obligatoria.as_str().unwrap();
                    args.entry(obligatoria).or_insert_with(|| valido(obligatoria));
                }
                let output = if nombre == "capture" { json!(r"C:\a\b.png") } else { valido("output") };
                if args.contains_key("output") {
                    args.insert("output".into(), output);
                }
                let peticion = traducir(nombre, &args, &carpeta())
                    .unwrap_or_else(|e| panic!("{nombre}.{clave} se rechaza: {e}"));
                let en_el_contrato = match clave.as_str() {
                    "windowId" | "windowTitle" => "window",
                    otra => otra,
                };
                assert!(peticion.get(en_el_contrato).is_some(), "{nombre}.{clave} no llega: {peticion}");
            }
        }
    }

    #[test]
    fn un_argumento_mal_puesto_vuelve_como_error_de_la_herramienta() {
        let canal = sin_canal();
        let casos = [
            ("capture", json!({}), "Say what to capture"),
            ("capture", json!({ "screen": 0, "region": { "x": 0, "y": 0, "width": 5, "height": 5 } }), "only one target"),
            ("capture", json!({ "screen": -1 }), "0 or more"),
            ("capture", json!({ "screen": "0" }), "whole number"),
            ("capture", json!({ "screen": 0, "output": "a.jpg" }), ".png"),
            ("capture", json!({ "region": { "x": 0, "y": 0, "width": 0, "height": 5 } }), "width"),
            ("record_start", json!({ "screen": 0, "max_seconds": 5 }), "Unknown argument \"max_seconds\""),
            ("record_start", json!({ "screen": 0, "format": "webm" }), "mp4"),
            ("record_start", json!({ "screen": 0, "maxSeconds": 0 }), "maxSeconds"),
            ("record_start", json!({ "screen": 0, "camera": "yes" }), "true or false"),
            ("record_stop", json!({ "now": true }), "takes no arguments"),
            ("export", json!({ "output": "a.mp4" }), "session is required"),
            ("export", json!({ "session": "s" }), "output is required"),
            ("export", json!({ "session": "s", "output": "a.mov" }), ".mp4 or .gif"),
            ("export", json!({ "session": "s", "output": "a.mp4", "trim": { "start": 5, "end": 2 } }), "end after start"),
            ("export", json!({ "session": "s", "output": "a.mp4", "zoomOnClicks": 2 }), "0 to 1"),
            ("export", json!({ "session": " ", "output": "a.mp4" }), "must not be empty"),
            ("open_in_editor", json!({}), "session is required"),
        ];
        let lineas: Vec<String> =
            casos.iter().enumerate().map(|(i, (n, a, _))| llamada(10 + i as i64, n, a.clone())).collect();
        let lineas: Vec<&str> = lineas.iter().map(String::as_str).collect();
        let (_, mensajes) = conversar_iniciado(&lineas, &canal);
        for (i, (nombre, _, debe_decir)) in casos.iter().enumerate() {
            let respuesta = con_id(&mensajes, json!(10 + i as i64));
            assert_eq!(respuesta["result"]["isError"], true, "{nombre}: {respuesta}");
            let cuerpo = texto_de(&respuesta);
            assert_eq!(cuerpo["ok"], false);
            let error = cuerpo["error"].as_str().unwrap();
            assert!(error.contains(debe_decir), "{nombre}: «{error}» no dice «{debe_decir}»");
        }
        assert!(canal.pedido().is_empty(), "nada mal puesto llega a la app");
    }

    #[test]
    fn un_fallo_de_la_app_o_del_canal_es_un_error_de_la_herramienta() {
        let canal = Canal::que_contesta(|p| match p["command"].as_str() {
            Some("stop") => Ok(json!({ "ok": false, "error": "Nothing is being recorded." })),
            Some("status") => Err("winshotx did not start within 15 s.".into()),
            _ => panic!("el canal de mentira entra en pánico"),
        });
        let lineas = [
            llamada(1, "record_stop", json!({})),
            llamada(2, "status", json!({})),
            llamada(3, "screens", json!({})),
            llamada(4, "windows", json!(null)),
        ];
        let lineas: Vec<&str> = lineas.iter().map(String::as_str).collect();
        let (codigo, mensajes) = conversar_iniciado(&lineas, &canal);
        assert_eq!(codigo, 0);
        let parar = con_id(&mensajes, json!(1));
        assert_eq!(parar["result"]["isError"], true);
        assert_eq!(texto_de(&parar), json!({ "ok": false, "error": "Nothing is being recorded." }));
        let estado = con_id(&mensajes, json!(2));
        assert_eq!(estado["result"]["isError"], true);
        assert_eq!(texto_de(&estado), json!({ "ok": false, "error": "winshotx did not start within 15 s." }));
        // Un pánico dentro del canal no tira el servidor: se contesta y se sigue.
        for id in [3, 4] {
            let respuesta = con_id(&mensajes, json!(id));
            assert_eq!(respuesta["result"]["isError"], true);
            assert!(texto_de(&respuesta)["error"].as_str().unwrap().contains("failed inside"));
        }
    }

    #[test]
    fn lo_que_no_es_del_protocolo_recibe_su_error_y_el_servidor_sigue() {
        let canal = sin_canal();
        let lineas = [
            "{esto no es json",
            r#"{"jsonrpc":"2.0","id":5,"method":"resources/list"}"#,
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"borrar_todo"}}"#,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"status","arguments":[1]}}"#,
            r#"[{"jsonrpc":"2.0","id":8,"method":"ping"}]"#,
            r#"{"id":9,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/algo_nuevo","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":99,"result":{}}"#,
            "",
            r#"{"jsonrpc":"2.0","id":10,"method":"ping"}"#,
        ];
        let (codigo, mensajes) = conversar_iniciado(&lineas, &canal);
        assert_eq!(codigo, 0);
        assert!(
            mensajes.iter().all(|m| m.get("id") != Some(&Value::Null)),
            "un error sin id deja el id fuera, sin \"id\": null: {mensajes:?}"
        );
        let codigos: Vec<(Value, i64)> = mensajes
            .iter()
            .filter(|m| m.get("error").is_some())
            .map(|m| (m["id"].clone(), m["error"]["code"].as_i64().unwrap()))
            .collect();
        assert_eq!(
            codigos,
            [
                (Value::Null, -32700),
                (json!(5), -32601),
                (json!(6), -32602),
                (json!(7), -32602),
                (Value::Null, -32600),
                (json!(9), -32600),
                (Value::Null, -32600),
            ]
        );
        let herramienta = con_id(&mensajes, json!(6));
        assert!(herramienta["error"]["message"].as_str().unwrap().contains("record_start"));
        assert_eq!(con_id(&mensajes, json!(10))["result"], json!({}), "después de todo, sigue contestando");
        assert_eq!(mensajes.len(), 8, "ni la notificación ni la respuesta del cliente se contestan");
    }

    #[test]
    fn antes_del_saludo_solo_contesta_el_ping_y_lo_nuevo_recibe_su_error() {
        // Sin la versión en `_meta` nada de esto es de la época nueva: un `server/discover` así
        // recibe «no existe» (un cliente de las dos épocas vuelve con eso a `initialize`) y lo
        // demás pide el saludo. Si el servidor se callara o se cerrara, ese cliente no lo podría
        // usar. Lo que sí trae la versión, en `un_cliente_que_solo_habla_la_2026_07_28_...`.
        let canal = sin_canal();
        let lineas = [
            r#"{"jsonrpc":"2.0","id":"d","method":"server/discover","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            &llamada(3, "status", json!({})),
            r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#,
            INICIO,
            INICIADO,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/list"}"#,
        ];
        let (codigo, mensajes) = conversar(&lineas, &canal);
        assert_eq!(codigo, 0, "sigue vivo");
        assert_eq!(con_id(&mensajes, json!("d"))["error"]["code"], -32601);
        for id in [2, 3] {
            let antes = con_id(&mensajes, json!(id));
            assert_eq!(antes["error"]["code"], -32600, "{antes}");
            assert!(antes["error"]["message"].as_str().unwrap().contains("initialize"));
        }
        assert!(canal.pedido().is_empty(), "nada de antes del saludo llega a la app");
        assert_eq!(con_id(&mensajes, json!(4))["result"], json!({}));
        assert_eq!(con_id(&mensajes, json!("init"))["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(con_id(&mensajes, json!(5))["result"]["tools"].as_array().map(Vec::len), Some(8));
    }

    /// El `_meta` que lleva cada petición de la época sin saludo, con esta versión.
    fn meta(version: &str) -> Value {
        json!({
            "io.modelcontextprotocol/protocolVersion": version,
            "io.modelcontextprotocol/clientCapabilities": {},
            "io.modelcontextprotocol/clientInfo": { "name": "prueba", "version": "2" },
        })
    }

    /// Una petición de la época sin saludo (MCP 2026-07-28).
    fn moderna(id: Value, metodo: &str, mut params: Value) -> String {
        params["_meta"] = meta("2026-07-28");
        json!({ "jsonrpc": "2.0", "id": id, "method": metodo, "params": params }).to_string()
    }

    /// Lo que todo resultado de la época sin saludo tiene que llevar.
    fn es_moderno(respuesta: &Value) {
        let r = &respuesta["result"];
        assert_eq!(r["resultType"], "complete", "{respuesta}");
        let yo = &r["_meta"]["io.modelcontextprotocol/serverInfo"];
        assert_eq!(yo["name"], "winshotx", "{respuesta}");
        assert_eq!(yo["version"], env!("CARGO_PKG_VERSION"), "{respuesta}");
    }

    /// La fila «Modern | Dual-era» de la especificación (`basic/versioning`): un cliente que
    /// solo habla MCP 2026-07-28 no saluda nunca. Pregunta con `server/discover`, pide la lista y
    /// llama, cada petición con su versión en `_meta`, y todo funciona sin `initialize`.
    #[test]
    fn un_cliente_que_solo_habla_la_2026_07_28_lo_usa_sin_saludar() {
        let canal = Canal::que_contesta(|_| Ok(json!({ "ok": true, "recording": false })));
        let lineas = [
            moderna(json!("d"), "server/discover", json!({})),
            moderna(json!(2), "tools/list", json!({})),
            moderna(json!(3), "tools/call", json!({ "name": "status", "arguments": {} })),
        ];
        let lineas: Vec<&str> = lineas.iter().map(String::as_str).collect();
        let (codigo, mensajes) = conversar(&lineas, &canal);
        assert_eq!(codigo, 0);

        let descubrir = con_id(&mensajes, json!("d"));
        es_moderno(&descubrir);
        let r = &descubrir["result"];
        assert_eq!(r["supportedVersions"], json!(["2026-07-28"]), "solo las de la época sin saludo");
        assert_eq!(r["capabilities"], json!({ "tools": { "listChanged": false } }));
        assert!(r["instructions"].as_str().unwrap().contains("record_start"));
        assert!(r["ttlMs"].as_u64().is_some(), "ttlMs es obligatorio y >= 0: {r}");
        assert_eq!(r["cacheScope"], "public");

        let lista = con_id(&mensajes, json!(2));
        es_moderno(&lista);
        assert_eq!(lista["result"]["tools"], herramientas(), "la misma lista que con saludo");
        assert!(lista["result"]["ttlMs"].as_u64().is_some());
        assert_eq!(lista["result"]["cacheScope"], "public");

        let estado = con_id(&mensajes, json!(3));
        es_moderno(&estado);
        assert_eq!(estado["result"]["isError"], false);
        assert_eq!(texto_de(&estado), json!({ "ok": true, "recording": false }));
        assert_eq!(canal.pedido(), [json!({ "command": "status" })], "la llamada llega a la app");
    }

    /// La fila «Dual-era | Dual-era»: un cliente de las dos épocas prueba primero con
    /// `server/discover` en la misma tubería y, al ver la 2026-07-28, sigue en ella. Y un cliente
    /// que ya saludó a la antigua puede mandar luego una petición nueva: cada una va por la suya.
    #[test]
    fn las_dos_epocas_en_la_misma_tuberia_contestan_cada_una_a_su_manera() {
        let canal = Canal::que_contesta(|_| Ok(json!({ "ok": true })));
        let lineas = [
            INICIO.to_string(),
            INICIADO.to_string(),
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_string(),
            moderna(json!(2), "tools/list", json!({})),
            llamada(3, "status", json!({})),
            moderna(json!(4), "tools/call", json!({ "name": "status" })),
        ];
        let lineas: Vec<&str> = lineas.iter().map(String::as_str).collect();
        let (_, mensajes) = conversar(&lineas, &canal);
        // Lo de la época del saludo sale exactamente como antes: sin nada de la nueva.
        for id in [1, 3] {
            let r = &con_id(&mensajes, json!(id))["result"];
            for nueva in ["resultType", "_meta", "ttlMs", "cacheScope"] {
                assert!(r.get(nueva).is_none(), "la respuesta {id} de la época del saludo lleva {nueva}: {r}");
            }
        }
        es_moderno(&con_id(&mensajes, json!(2)));
        es_moderno(&con_id(&mensajes, json!(4)));
        // `initialize` con `_meta` nuevo sigue siendo el saludo de antes, y nunca dice 2026-07-28.
        let saludo = json!({ "jsonrpc": "2.0", "id": "i", "method": "initialize",
            "params": { "protocolVersion": "2026-07-28", "capabilities": {}, "clientInfo": { "name": "p", "version": "1" },
                        "_meta": meta("2026-07-28") } })
        .to_string();
        let (_, mensajes) = conversar(&[&saludo], &sin_canal());
        assert_eq!(con_id(&mensajes, json!("i"))["result"]["protocolVersion"], "2025-06-18");
    }

    /// Lo que la 2026-07-28 manda contestar cuando una petición suya no se puede atender: la
    /// versión que no se habla (`-32022`, con las que sí), el `_meta` incompleto (`-32602`), y lo
    /// que ya no existe en esa época (`ping`, `-32601`). Una herramienta que no existe o unos
    /// argumentos malos, como siempre, pero con lo que esa época pide en cada resultado.
    #[test]
    fn la_epoca_sin_saludo_dice_lo_que_falta_y_que_versiones_habla() {
        let canal = sin_canal();
        let lineas = [
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list",
                    "params": { "_meta": { "io.modelcontextprotocol/protocolVersion": "2026-07-28" } } })
            .to_string(),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "server/discover", "params": { "_meta": meta("2099-01-01") } })
                .to_string(),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list",
                    "params": { "_meta": { "io.modelcontextprotocol/protocolVersion": 2026,
                                           "io.modelcontextprotocol/clientCapabilities": {} } } })
            .to_string(),
            moderna(json!(4), "ping", json!({})),
            moderna(json!(5), "tools/call", json!({ "name": "borrar_todo" })),
            moderna(json!(6), "tools/call", json!({ "name": "capture", "arguments": { "screen": "0" } })),
            moderna(json!(7), "logging/setLevel", json!({ "level": "info" })),
        ];
        let lineas: Vec<&str> = lineas.iter().map(String::as_str).collect();
        let (codigo, mensajes) = conversar(&lineas, &canal);
        assert_eq!(codigo, 0, "sigue vivo");
        let codigo_de = |id: i64| con_id(&mensajes, json!(id))["error"]["code"].as_i64();
        assert_eq!(codigo_de(1), Some(-32602), "sin clientCapabilities");
        assert!(con_id(&mensajes, json!(1))["error"]["message"].as_str().unwrap().contains("clientCapabilities"));
        assert_eq!(codigo_de(2), Some(-32022), "una versión que no habla");
        let datos = &con_id(&mensajes, json!(2))["error"]["data"];
        assert_eq!(datos["requested"], "2099-01-01");
        assert_eq!(datos["supported"], json!(["2026-07-28", "2025-06-18", "2024-11-05"]));
        assert_eq!(codigo_de(3), Some(-32602), "una versión que no es texto");
        assert_eq!(codigo_de(4), Some(-32601), "ping ya no existe en esta época");
        assert_eq!(codigo_de(5), Some(-32602), "una herramienta que no existe");
        assert_eq!(codigo_de(7), Some(-32601));
        let mala = con_id(&mensajes, json!(6));
        es_moderno(&mala);
        assert_eq!(mala["result"]["isError"], true);
        assert!(texto_de(&mala)["error"].as_str().unwrap().contains("whole number"), "{mala}");
        assert!(canal.pedido().is_empty(), "nada de esto llega a la app");
    }

    /// Un `_meta` de los de antes (el `progressToken`) no convierte una petición en nueva: sin la
    /// versión en `_meta`, sin saludo, se pide el saludo como siempre.
    #[test]
    fn un_meta_de_los_de_antes_no_es_de_la_epoca_nueva() {
        let lista = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"progressToken":"t"}}}"#;
        let (_, mensajes) = conversar(&[lista], &sin_canal());
        assert_eq!(con_id(&mensajes, json!(1))["error"]["code"], -32600);
    }

    #[test]
    fn el_bom_y_los_finales_de_linea_de_windows_no_molestan() {
        let canal = sin_canal();
        let entrada = format!("\u{FEFF}{INICIO}\r\n{}\r\n", r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
        let (_, mensajes) = conversar_bytes(entrada.into_bytes(), &canal);
        assert_eq!(con_id(&mensajes, json!("init"))["result"]["serverInfo"]["name"], "winshotx");
        assert_eq!(con_id(&mensajes, json!(2))["result"], json!({}));
    }

    #[test]
    fn una_llamada_larga_no_tapa_el_ping_y_al_cerrar_se_contesta_entera() {
        let canal = Canal::que_contesta(|_| {
            std::thread::sleep(Duration::from_millis(300));
            Ok(json!({ "ok": true, "file": r"C:\v.mp4", "seconds": 3.0, "bytes": 1000 }))
        });
        let exportar = llamada(1, "export", json!({ "session": "s", "output": r"C:\v.mp4" }));
        let ping = r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#;
        let (_, mensajes) = conversar_iniciado(&[&exportar, ping], &canal);
        let ids: Vec<Value> = mensajes.iter().map(|m| m["id"].clone()).collect();
        assert_eq!(ids, [json!(2), json!(1)], "el ping sale antes que la exportación");
        assert_eq!(con_id(&mensajes, json!(1))["result"]["isError"], false);
    }

    #[test]
    fn una_llamada_cancelada_antes_de_empezar_no_llega_a_la_app() {
        let canal = Canal::que_contesta(|_| Ok(json!({ "ok": true })));
        let cancelar = r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#;
        let exportar = llamada(1, "export", json!({ "session": "s", "output": r"C:\v.mp4" }));
        let estado = llamada(2, "status", json!({}));
        // La cancelación llega antes que la llamada (el cliente se arrepiente al momento).
        let (_, mensajes) = conversar_iniciado(&[cancelar, &exportar, &estado], &canal);
        assert!(mensajes.iter().all(|m| m["id"] != json!(1)), "la cancelada no tiene respuesta: {mensajes:?}");
        assert_eq!(canal.pedido(), [json!({ "command": "status" })], "ni se le pide a la app");
        assert_eq!(con_id(&mensajes, json!(2))["result"]["isError"], false, "la siguiente sí");
    }

    /// Una entrada que da sus líneas de una en una, y antes de cada una espera a su señal
    /// (si la tiene): así una prueba decide qué llega mientras se hace una llamada.
    struct EntradaConPausas {
        lineas: std::collections::VecDeque<(Option<mpsc::Receiver<()>>, Vec<u8>)>,
    }

    impl std::io::Read for EntradaConPausas {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let Some((senal, linea)) = self.lineas.front_mut() else { return Ok(0) };
            if let Some(senal) = senal.take() {
                senal.recv_timeout(Duration::from_secs(5)).expect("la señal llega");
            }
            let n = linea.len().min(buf.len());
            buf[..n].copy_from_slice(&linea[..n]);
            linea.drain(..n);
            if linea.is_empty() {
                self.lineas.pop_front();
            }
            Ok(n)
        }
    }

    #[test]
    fn una_llamada_cancelada_mientras_se_hace_no_se_contesta() {
        let (empezada, al_empezar) = mpsc::channel();
        let empezada = Mutex::new(empezada);
        let canal = Canal::que_contesta(move |p| {
            if p["command"] == "export" {
                empezada.lock().unwrap().send(()).unwrap();
                // Lo que tarde la exportación: da tiempo de sobra a que llegue la cancelación.
                std::thread::sleep(Duration::from_millis(200));
            }
            Ok(json!({ "ok": true }))
        });
        let linea = |s: String| format!("{s}\n").into_bytes();
        let cancelar = r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1,"reason":"user"}}"#;
        let entrada = EntradaConPausas {
            lineas: [
                (None, linea(INICIO.to_string())),
                (None, linea(llamada(1, "export", json!({ "session": "s", "output": r"C:\v.mp4" })))),
                // La cancelación no se lee hasta que la app ya está exportando.
                (Some(al_empezar), linea(cancelar.to_string())),
                (None, linea(llamada(2, "status", json!({})))),
            ]
            .into(),
        };
        let mut salida = Vec::new();
        let pedir = |p: &Value| canal.pedir(p);
        let codigo = servir_con(std::io::BufReader::new(entrada), &mut salida, &pedir, &carpeta());
        assert_eq!(codigo, 0);
        let mensajes: Vec<Value> =
            String::from_utf8(salida).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(canal.pedido().len(), 2, "la exportación sí se pidió");
        assert!(mensajes.iter().all(|m| m["id"] != json!(1)), "la cancelada no tiene respuesta: {mensajes:?}");
        assert_eq!(con_id(&mensajes, json!(2))["result"]["isError"], false, "la siguiente sí");
    }

    #[test]
    fn si_el_cliente_cierra_la_salida_el_servidor_acaba_con_error() {
        struct Cerrada;
        impl Write for Cerrada {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let canal = sin_canal();
        let pedir = |p: &Value| canal.pedir(p);
        let entrada = format!("{INICIO}\n{INICIO}\n");
        let codigo = servir_con(Cursor::new(entrada.into_bytes()), &mut Cerrada, &pedir, &carpeta());
        assert_eq!(codigo, 1);
    }
}
