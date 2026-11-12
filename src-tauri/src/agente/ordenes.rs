//! Las órdenes del canal de agentes: leer cada petición, resolver a qué pantalla, ventana o
//! zona se refiere, pedírselo a la app y contestar. Una petición es una línea JSON con
//! `"command"`; la respuesta, `{"ok":true, ...}` o `{"ok":false,"error":"..."}`.
//!
//! Lo que de verdad graba, captura y exporta está detrás del `trait Hace`: en la app es
//! `en_la_app::EnLaApp`, y en las pruebas, un doble que apunta lo que le piden. Así se prueba
//! que cada orden llega como tiene que llegar sin grabar ni capturar nada.
//!
//! **Los errores son para un agente:** en inglés, y cada uno dice qué hacer a continuación
//! (qué orden da la lista buena, qué campo sobra), porque quien lo lee no puede preguntar.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

/// Las órdenes que hay, para los mensajes de error.
pub const ORDENES: &str = "status, screens, windows, capture, record, stop, export, open";

/// Un rectángulo en píxeles físicos del escritorio virtual: las mismas coordenadas que da
/// `screens`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caja {
    pub x: i32,
    pub y: i32,
    pub ancho: u32,
    pub alto: u32,
}

impl Caja {
    fn derecha(&self) -> i64 {
        i64::from(self.x) + i64::from(self.ancho)
    }

    fn abajo(&self) -> i64 {
        i64::from(self.y) + i64::from(self.alto)
    }

    /// Lo que tienen en común, si tienen algo.
    pub fn cruce(&self, otra: &Caja) -> Option<Caja> {
        let x0 = i64::from(self.x).max(i64::from(otra.x));
        let y0 = i64::from(self.y).max(i64::from(otra.y));
        let x1 = self.derecha().min(otra.derecha());
        let y1 = self.abajo().min(otra.abajo());
        (x1 > x0 && y1 > y0).then(|| Caja {
            x: x0 as i32,
            y: y0 as i32,
            ancho: (x1 - x0) as u32,
            alto: (y1 - y0) as u32,
        })
    }

    fn area(&self) -> u64 {
        u64::from(self.ancho) * u64::from(self.alto)
    }

    /// La caja más pequeña que contiene a las dos.
    fn union(&self, otra: &Caja) -> Caja {
        let x0 = i64::from(self.x).min(i64::from(otra.x));
        let y0 = i64::from(self.y).min(i64::from(otra.y));
        let x1 = self.derecha().max(otra.derecha());
        let y1 = self.abajo().max(otra.abajo());
        Caja { x: x0 as i32, y: y0 as i32, ancho: (x1 - x0) as u32, alto: (y1 - y0) as u32 }
    }

    pub fn a_json(&self) -> Value {
        json!({ "x": self.x, "y": self.y, "width": self.ancho, "height": self.alto })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pantalla {
    /// Su número en la enumeración del sistema, empezando por 0: el que se pide en `"screen"`.
    pub indice: u32,
    pub caja: Caja,
    pub escala: f32,
    pub principal: bool,
    pub nombre: String,
}

impl Pantalla {
    fn a_json(&self) -> Value {
        json!({
            "index": self.indice,
            "x": self.caja.x,
            "y": self.caja.y,
            "width": self.caja.ancho,
            "height": self.caja.alto,
            "scale": (f64::from(self.escala) * 1000.0).round() / 1000.0,
            "primary": self.principal,
            "name": self.nombre,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ventana {
    /// El HWND, como número.
    pub id: isize,
    pub titulo: String,
    /// El nombre del ejecutable, `msedge.exe`.
    pub proceso: String,
    pub caja: Caja,
}

impl Ventana {
    fn a_json(&self) -> Value {
        json!({
            "id": self.id,
            "title": self.titulo,
            "process": self.proceso,
            "x": self.caja.x,
            "y": self.caja.y,
            "width": self.caja.ancho,
            "height": self.caja.alto,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueVentana {
    Id(isize),
    /// Un trozo del título (o del nombre del programa), sin mirar mayúsculas.
    Titulo(String),
}

/// Lo que se captura o se graba, tal cual lo pide el agente.
#[derive(Debug, Clone, PartialEq)]
pub enum Objetivo {
    Pantalla(u32),
    Ventana(QueVentana),
    Zona(Caja),
}

/// El objetivo ya resuelto: la caja de píxeles y, si es una ventana, cuál.
#[derive(Debug, Clone, PartialEq)]
pub struct Donde {
    pub caja: Caja,
    pub ventana: Option<isize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Mp4,
    Gif,
}

impl Formato {
    pub fn extension(self) -> &'static str {
        match self {
            Formato::Mp4 => "mp4",
            Formato::Gif => "gif",
        }
    }

    fn de(texto: &str) -> Option<Self> {
        match texto.to_ascii_lowercase().as_str() {
            "mp4" => Some(Formato::Mp4),
            "gif" => Some(Formato::Gif),
            _ => None,
        }
    }
}

/// Lo que se puede pedir al grabar. `None` es «lo que diga el ajuste de la app».
#[derive(Debug, Clone, PartialEq)]
pub struct OpcionesDeGrabar {
    pub camara: Option<bool>,
    pub sonido_pc: Option<bool>,
    pub microfono: Option<bool>,
    /// La marimba de empezar. Apagada si no se pide: un agente no hace sonar nada sin que se
    /// lo digan.
    pub marimba: bool,
    pub formato: Formato,
    /// Para sola al llegar, como si se hubiera mandado `stop`.
    pub max_segundos: Option<f64>,
}

impl Default for OpcionesDeGrabar {
    fn default() -> Self {
        Self { camara: None, sonido_pc: None, microfono: None, marimba: false, formato: Formato::Mp4, max_segundos: None }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PedidoDeExportar {
    pub sesion: String,
    /// Absoluta. Sin ella, la carpeta de los ajustes con un nombre nuevo.
    pub salida: Option<PathBuf>,
    pub formato: Option<Formato>,
    /// El nombre de un estilo guardado en el editor.
    pub estilo: Option<String>,
    /// El trozo que se queda, en segundos de la grabación.
    pub desde: Option<f64>,
    pub hasta: Option<f64>,
    /// Cuánto se acerca a cada clic: 0 nada, 1 lo más (tres veces).
    pub zoom_en_clics: Option<f64>,
    pub velocidad: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Orden {
    Estado,
    Pantallas,
    Ventanas,
    Capturar { objetivo: Objetivo, salida: Option<PathBuf> },
    Grabar { objetivo: Objetivo, opciones: OpcionesDeGrabar },
    Parar,
    Exportar(PedidoDeExportar),
    Abrir { sesion: String },
}

pub struct Estado {
    pub version: String,
    /// La sesión que se está grabando, si hay una.
    pub grabando: Option<String>,
}

pub struct Captura {
    pub archivo: PathBuf,
    pub ancho: u32,
    pub alto: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parada {
    pub sesion: String,
    pub segundos: f64,
    /// La paró `maxSeconds` antes de que llegara el `stop`.
    pub por_tope: bool,
}

pub struct Exportado {
    pub archivo: PathBuf,
    pub segundos: f64,
    pub bytes: u64,
}

/// Lo que la app sabe hacer, visto desde el canal. Los errores ya van en inglés y para un
/// agente.
pub trait Hace {
    fn estado(&self) -> Result<Estado, String>;
    fn pantallas(&self) -> Result<Vec<Pantalla>, String>;
    /// Las ventanas visibles, sin las de winshotx, en orden de delante a atrás.
    fn ventanas(&self) -> Result<Vec<Ventana>, String>;
    fn capturar(&self, donde: &Donde, salida: Option<&Path>) -> Result<Captura, String>;
    /// Vuelve cuando la grabación YA ha empezado, con el id de su sesión.
    fn grabar(&self, donde: &Donde, opciones: &OpcionesDeGrabar) -> Result<String, String>;
    /// Vuelve cuando la sesión está guardada, sin abrir el editor.
    fn parar(&self) -> Result<Parada, String>;
    fn exportar(&self, pedido: &PedidoDeExportar) -> Result<Exportado, String>;
    fn abrir(&self, sesion: &str) -> Result<(), String>;
}

/// Atiende una línea y devuelve la respuesta, también en una línea. Nunca falla: lo que va
/// mal sale como `{"ok":false,"error":...}`.
pub fn atender(linea: &str, hace: &dyn Hace) -> String {
    let respuesta = leer(linea).and_then(|orden| ejecutar(orden, hace));
    match respuesta {
        Ok(mut valor) => {
            valor["ok"] = Value::Bool(true);
            valor
        }
        Err(error) => json!({ "ok": false, "error": error }),
    }
    .to_string()
}

/// La petición, leída y comprobada. Todo lo que no cuadra se rechaza aquí con un error que
/// dice cómo arreglarlo, antes de tocar nada de la app.
pub fn leer(linea: &str) -> Result<Orden, String> {
    let valor: Value = serde_json::from_str(linea).map_err(|e| {
        format!("The request is not valid JSON ({e}). Send one JSON object per line, like {{\"command\":\"status\"}}.")
    })?;
    let obj = valor
        .as_object()
        .ok_or_else(|| "The request must be a JSON object, like {\"command\":\"status\"}.".to_string())?;
    let orden = match obj.get("command") {
        Some(Value::String(orden)) => orden.as_str(),
        Some(_) => return Err(format!("\"command\" must be a string: one of {ORDENES}.")),
        None => return Err(format!("Missing \"command\". Use one of: {ORDENES}.")),
    };
    const OBJETIVO: [&str; 3] = ["screen", "window", "region"];
    match orden {
        "status" => solo(obj, orden, &[]).map(|_| Orden::Estado),
        "screens" => solo(obj, orden, &[]).map(|_| Orden::Pantallas),
        "windows" => solo(obj, orden, &[]).map(|_| Orden::Ventanas),
        "stop" => solo(obj, orden, &[]).map(|_| Orden::Parar),
        "capture" => {
            solo(obj, orden, &[&OBJETIVO[..], &["output"][..]].concat())?;
            Ok(Orden::Capturar { objetivo: objetivo(obj)?, salida: salida_png(obj)? })
        }
        "record" => {
            solo(
                obj,
                orden,
                &[&OBJETIVO[..], &["camera", "systemAudio", "microphone", "cue", "format", "maxSeconds"][..]].concat(),
            )?;
            let max_segundos = match numero(obj, "maxSeconds")? {
                Some(s) if s > 0.0 => Some(s),
                Some(_) => return Err("\"maxSeconds\" must be a number of seconds greater than 0.".into()),
                None => None,
            };
            Ok(Orden::Grabar {
                objetivo: objetivo(obj)?,
                opciones: OpcionesDeGrabar {
                    camara: booleano(obj, "camera")?,
                    sonido_pc: booleano(obj, "systemAudio")?,
                    microfono: booleano(obj, "microphone")?,
                    marimba: booleano(obj, "cue")?.unwrap_or(false),
                    formato: formato(obj)?.unwrap_or(Formato::Mp4),
                    max_segundos,
                },
            })
        }
        "export" => {
            solo(obj, orden, &["session", "output", "format", "style", "trim", "zoomOnClicks", "speed"])?;
            let salida = ruta_absoluta(obj, "output")?;
            let por_extension = match &salida {
                Some(ruta) => Some(
                    ruta.extension()
                        .and_then(|e| e.to_str())
                        .and_then(Formato::de)
                        .ok_or_else(|| "\"output\" must end in .mp4 or .gif.".to_string())?,
                ),
                None => None,
            };
            let formato = match (por_extension, formato(obj)?) {
                (Some(a), Some(b)) if a != b => {
                    return Err(format!(
                        "\"output\" ends in .{} but \"format\" is \"{}\": make them match, or leave \"format\" out.",
                        a.extension(),
                        b.extension()
                    ))
                }
                (a, b) => a.or(b),
            };
            let (desde, hasta) = recorte(obj)?;
            let zoom_en_clics = match numero(obj, "zoomOnClicks")? {
                Some(z) if (0.0..=1.0).contains(&z) => Some(z),
                Some(_) => return Err("\"zoomOnClicks\" goes from 0 (no zoom) to 1 (the most, 3x).".into()),
                None => None,
            };
            let velocidad = match numero(obj, "speed")? {
                Some(v) if (0.25..=4.0).contains(&v) => Some(v),
                Some(_) => return Err("\"speed\" goes from 0.25 to 4 (1 is real time, 2 twice as fast).".into()),
                None => None,
            };
            let estilo = match obj.get("style") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
                Some(_) => return Err("\"style\" must be the name of a style saved in the winshotx editor.".into()),
            };
            Ok(Orden::Exportar(PedidoDeExportar {
                sesion: sesion(obj)?,
                salida,
                formato,
                estilo,
                desde,
                hasta,
                zoom_en_clics,
                velocidad,
            }))
        }
        "open" => {
            solo(obj, orden, &["session"])?;
            Ok(Orden::Abrir { sesion: sesion(obj)? })
        }
        otra => Err(format!("Unknown command \"{otra}\". Use one of: {ORDENES}.")),
    }
}

/// Que no venga ningún campo que esa orden no conoce: un `"systemaudio"` mal escrito se
/// ignoraría en silencio y el agente creería que lo ha pedido.
fn solo(obj: &Map<String, Value>, orden: &str, permitidos: &[&str]) -> Result<(), String> {
    for clave in obj.keys() {
        if clave != "command" && !permitidos.contains(&clave.as_str()) {
            return Err(if permitidos.is_empty() {
                format!("\"{orden}\" takes no fields, and got \"{clave}\".")
            } else {
                format!("Unknown field \"{clave}\" for \"{orden}\". Valid fields: {}.", permitidos.join(", "))
            });
        }
    }
    Ok(())
}

fn booleano(obj: &Map<String, Value>, campo: &str) -> Result<Option<bool>, String> {
    match obj.get(campo) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("\"{campo}\" must be true or false.")),
    }
}

fn numero(obj: &Map<String, Value>, campo: &str) -> Result<Option<f64>, String> {
    match obj.get(campo) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_f64()
            .filter(|v| v.is_finite())
            .map(Some)
            .ok_or_else(|| format!("\"{campo}\" must be a number.")),
        Some(_) => Err(format!("\"{campo}\" must be a number.")),
    }
}

/// Un entero, aunque venga como `2.0`.
fn entero(valor: &Value) -> Option<i64> {
    valor.as_i64().or_else(|| valor.as_f64().filter(|v| v.fract() == 0.0 && v.abs() < 9e15).map(|v| v as i64))
}

fn formato(obj: &Map<String, Value>) -> Result<Option<Formato>, String> {
    match obj.get("format") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Formato::de(s).map(Some).ok_or_else(|| "\"format\" must be \"mp4\" or \"gif\".".into()),
        Some(_) => Err("\"format\" must be \"mp4\" or \"gif\".".into()),
    }
}

/// El id de una sesión: el que devuelven `record` y `stop`. Solo letras, números, `-` y `_`,
/// porque con él se arma la ruta de su carpeta y un `..\` saldría de ella.
fn sesion(obj: &Map<String, Value>) -> Result<String, String> {
    let texto = match obj.get("session") {
        Some(Value::String(s)) => s.trim(),
        Some(_) => return Err("\"session\" must be the id string that \"record\" or \"stop\" returned.".into()),
        None => return Err("Missing \"session\": use the id that \"record\" or \"stop\" returned.".into()),
    };
    let valido = !texto.is_empty()
        && texto.len() <= 64
        && texto.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valido {
        return Err(format!("\"{texto}\" is not a session id. Use the id that \"record\" or \"stop\" returned."));
    }
    Ok(texto.to_string())
}

/// Una ruta de salida, que tiene que ser absoluta: la app no sabe en qué carpeta está el
/// agente. Los dos clientes (`agent` y `mcp`) ya convierten las relativas antes de mandarlas
/// (`cliente::con_rutas_absolutas`), así que esto solo salta con un cliente hecho a mano.
fn ruta_absoluta(obj: &Map<String, Value>, campo: &str) -> Result<Option<PathBuf>, String> {
    match obj.get(campo) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if !s.trim().is_empty() => {
            let ruta = PathBuf::from(s.trim());
            if ruta.is_absolute() {
                Ok(Some(ruta))
            } else {
                Err(format!(
                    "\"{campo}\" must be an absolute path, like C:\\Users\\you\\Videos\\demo.mp4 (winshotx runs in its own folder, not yours)."
                ))
            }
        }
        Some(_) => Err(format!("\"{campo}\" must be a file path.")),
    }
}

/// La salida de una captura: siempre PNG. Sin extensión se le pone; con otra, error.
fn salida_png(obj: &Map<String, Value>) -> Result<Option<PathBuf>, String> {
    let Some(mut ruta) = ruta_absoluta(obj, "output")? else {
        return Ok(None);
    };
    match ruta.extension().and_then(|e| e.to_str()) {
        None => {
            ruta.set_extension("png");
            Ok(Some(ruta))
        }
        Some(e) if e.eq_ignore_ascii_case("png") => Ok(Some(ruta)),
        Some(_) => Err("A capture is saved as PNG: \"output\" must end in .png.".into()),
    }
}

/// `"trim": {"start": 1.5, "end": 9}`, en segundos. Los dos son opcionales: sin `end`, hasta
/// el final.
fn recorte(obj: &Map<String, Value>) -> Result<(Option<f64>, Option<f64>), String> {
    let mal = || "\"trim\" must look like {\"start\": 1.5, \"end\": 9} (seconds of the recording).".to_string();
    let t = match obj.get("trim") {
        None | Some(Value::Null) => return Ok((None, None)),
        Some(Value::Object(t)) => t,
        Some(_) => return Err(mal()),
    };
    if t.keys().any(|k| k != "start" && k != "end") {
        return Err(mal());
    }
    let segundos = |campo: &str| -> Result<Option<f64>, String> {
        match t.get(campo) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v.as_f64().filter(|s| s.is_finite() && *s >= 0.0).map(Some).ok_or_else(mal),
        }
    };
    let (desde, hasta) = (segundos("start")?, segundos("end")?);
    if let (Some(a), Some(b)) = (desde, hasta) {
        if b <= a {
            return Err(format!("\"trim\" ends at {b} s, which is not after its start ({a} s)."));
        }
    }
    Ok((desde, hasta))
}

/// Uno y solo uno de `screen`, `window` o `region`.
fn objetivo(obj: &Map<String, Value>) -> Result<Objetivo, String> {
    let puestos: Vec<&str> = ["screen", "window", "region"]
        .into_iter()
        .filter(|c| obj.get(*c).is_some_and(|v| !v.is_null()))
        .collect();
    match puestos.as_slice() {
        [] => Err("Say what to use: \"screen\": <index> (see \"screens\"), \"window\": <id or part of its title> (see \"windows\"), or \"region\": {\"x\":..,\"y\":..,\"width\":..,\"height\":..}.".into()),
        ["screen"] => {
            let v = &obj["screen"];
            entero(v)
                .and_then(|n| u32::try_from(n).ok())
                .map(Objetivo::Pantalla)
                .ok_or_else(|| "\"screen\" must be a screen index: 0, 1... (see \"screens\").".into())
        }
        ["window"] => match &obj["window"] {
            Value::String(s) if !s.trim().is_empty() => Ok(Objetivo::Ventana(QueVentana::Titulo(s.trim().to_string()))),
            v => entero(v)
                .and_then(|n| isize::try_from(n).ok())
                .filter(|n| *n != 0)
                .map(|n| Objetivo::Ventana(QueVentana::Id(n)))
                .ok_or_else(|| "\"window\" must be a window id or part of its title (see \"windows\").".into()),
        },
        ["region"] => {
            let mal = || "\"region\" must look like {\"x\":0,\"y\":0,\"width\":1280,\"height\":720} in physical pixels (see \"screens\").".to_string();
            let r = obj["region"].as_object().ok_or_else(mal)?;
            if r.keys().any(|k| !["x", "y", "width", "height"].contains(&k.as_str())) {
                return Err(mal());
            }
            let campo = |c: &str| r.get(c).and_then(entero).ok_or_else(mal);
            let (x, y, ancho, alto) = (campo("x")?, campo("y")?, campo("width")?, campo("height")?);
            let caja = Caja {
                x: i32::try_from(x).map_err(|_| mal())?,
                y: i32::try_from(y).map_err(|_| mal())?,
                ancho: u32::try_from(ancho).map_err(|_| mal())?,
                alto: u32::try_from(alto).map_err(|_| mal())?,
            };
            if caja.ancho < 2 || caja.alto < 2 {
                return Err("\"region\" needs a width and a height of at least 2 pixels.".into());
            }
            Ok(Objetivo::Zona(caja))
        }
        varios => Err(format!(
            "Use only one of \"screen\", \"window\" or \"region\", not {}.",
            varios.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(" and ")
        )),
    }
}

/// Para qué es el objetivo: una captura puede cruzar varias pantallas (se juntan), y una
/// grabación de zona no (se graba de una pantalla).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uso {
    Capturar,
    Grabar,
}

/// De lo que pidió el agente a una caja de píxeles. Una zona se recorta a lo que cae dentro de
/// las pantallas (al grabar, a la pantalla donde cae más), para no grabar ni capturar negro.
pub fn resolver(objetivo: &Objetivo, uso: Uso, hace: &dyn Hace) -> Result<Donde, String> {
    match objetivo {
        Objetivo::Pantalla(indice) => {
            let pantallas = hace.pantallas()?;
            let pantalla = pantallas.iter().find(|p| p.indice == *indice).ok_or_else(|| {
                format!(
                    "There is no screen {indice}. This PC has {} screen(s), numbered from 0; see \"screens\".",
                    pantallas.len()
                )
            })?;
            Ok(Donde { caja: pantalla.caja, ventana: None })
        }
        Objetivo::Ventana(que) => {
            let ventanas = hace.ventanas()?;
            let elegida = match que {
                QueVentana::Id(id) => ventanas.iter().find(|v| v.id == *id).ok_or_else(|| {
                    format!("No visible window has id {id}: it may have closed or be minimized. List them with \"windows\".")
                })?,
                QueVentana::Titulo(trozo) => {
                    let buscado = trozo.to_lowercase();
                    let sin_exe = buscado.strip_suffix(".exe").unwrap_or(&buscado).to_string();
                    ventanas
                        .iter()
                        .find(|v| v.titulo.to_lowercase().contains(&buscado))
                        .or_else(|| ventanas.iter().find(|v| v.proceso.to_lowercase().contains(&sin_exe)))
                        .ok_or_else(|| {
                            let algunas: Vec<String> =
                                ventanas.iter().take(8).map(|v| format!("\"{}\"", v.titulo)).collect();
                            format!(
                                "No visible window has \"{trozo}\" in its title or program name. Some open windows: {}. Use \"windows\" for the full list with ids.",
                                if algunas.is_empty() { "none".into() } else { algunas.join(", ") }
                            )
                        })?
                }
            };
            Ok(Donde { caja: elegida.caja, ventana: Some(elegida.id) })
        }
        Objetivo::Zona(caja) => {
            let pantallas = hace.pantallas()?;
            let fuera = || {
                "That region is outside every screen. See \"screens\" for where they are (physical pixels of the virtual desktop).".to_string()
            };
            let caja = match uso {
                // A la pantalla que más tenga de ella: una grabación de zona sale de UNA pantalla.
                Uso::Grabar => pantallas
                    .iter()
                    .filter_map(|p| caja.cruce(&p.caja))
                    .max_by_key(Caja::area)
                    .ok_or_else(fuera)?,
                // A lo que cubren las pantallas que toca: la captura las junta.
                Uso::Capturar => {
                    let tocadas: Vec<Caja> = pantallas.iter().filter(|p| caja.cruce(&p.caja).is_some()).map(|p| p.caja).collect();
                    let escritorio = tocadas.iter().skip(1).fold(*tocadas.first().ok_or_else(fuera)?, |a, b| a.union(b));
                    caja.cruce(&escritorio).ok_or_else(fuera)?
                }
            };
            if caja.ancho < 2 || caja.alto < 2 {
                return Err(fuera());
            }
            Ok(Donde { caja, ventana: None })
        }
    }
}

/// Segundos con tres decimales, que es lo que se puede medir.
fn redondear(segundos: f64) -> f64 {
    (segundos * 1000.0).round() / 1000.0
}

fn ejecutar(orden: Orden, hace: &dyn Hace) -> Result<Value, String> {
    match orden {
        Orden::Estado => {
            let estado = hace.estado()?;
            Ok(json!({
                "version": estado.version,
                "recording": estado.grabando.is_some(),
                "session": estado.grabando,
            }))
        }
        Orden::Pantallas => Ok(json!({ "screens": hace.pantallas()?.iter().map(Pantalla::a_json).collect::<Vec<_>>() })),
        Orden::Ventanas => Ok(json!({ "windows": hace.ventanas()?.iter().map(Ventana::a_json).collect::<Vec<_>>() })),
        Orden::Capturar { objetivo, salida } => {
            let donde = resolver(&objetivo, Uso::Capturar, hace)?;
            let captura = hace.capturar(&donde, salida.as_deref())?;
            Ok(json!({
                "file": captura.archivo.to_string_lossy(),
                "width": captura.ancho,
                "height": captura.alto,
            }))
        }
        Orden::Grabar { objetivo, opciones } => {
            let donde = resolver(&objetivo, Uso::Grabar, hace)?;
            let sesion = hace.grabar(&donde, &opciones)?;
            Ok(json!({ "session": sesion, "region": donde.caja.a_json() }))
        }
        Orden::Parar => {
            let parada = hace.parar()?;
            let mut valor = json!({ "session": parada.sesion, "seconds": redondear(parada.segundos) });
            if parada.por_tope {
                valor["maxSecondsReached"] = Value::Bool(true);
            }
            Ok(valor)
        }
        Orden::Exportar(pedido) => {
            let hecho = hace.exportar(&pedido)?;
            Ok(json!({
                "file": hecho.archivo.to_string_lossy(),
                "seconds": redondear(hecho.segundos),
                "bytes": hecho.bytes,
            }))
        }
        Orden::Abrir { sesion } => {
            hace.abrir(&sesion)?;
            Ok(json!({ "session": sesion }))
        }
    }
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;
    use std::cell::RefCell;

    /// Un doble de la app: dos pantallas (la principal de 1920x1080 al 125 % y otra a su
    /// derecha), tres ventanas, y apunta cada cosa que le piden.
    #[derive(Default)]
    pub(crate) struct Doble {
        pub pedido: RefCell<Vec<String>>,
        pub grabando: RefCell<Option<String>>,
    }

    pub(crate) fn caja(x: i32, y: i32, ancho: u32, alto: u32) -> Caja {
        Caja { x, y, ancho, alto }
    }

    impl Hace for Doble {
        fn estado(&self) -> Result<Estado, String> {
            Ok(Estado { version: "0.4.5".into(), grabando: self.grabando.borrow().clone() })
        }
        fn pantallas(&self) -> Result<Vec<Pantalla>, String> {
            Ok(vec![
                Pantalla { indice: 0, caja: caja(0, 0, 1920, 1080), escala: 1.25, principal: true, nombre: "UNO".into() },
                Pantalla { indice: 1, caja: caja(1920, 0, 1080, 1920), escala: 1.0, principal: false, nombre: "DOS".into() },
            ])
        }
        fn ventanas(&self) -> Result<Vec<Ventana>, String> {
            Ok(vec![
                Ventana { id: 111, titulo: "Notas - Bloc de notas".into(), proceso: "notepad.exe".into(), caja: caja(10, 20, 800, 600) },
                Ventana { id: 222, titulo: "winshotx - Microsoft Edge".into(), proceso: "msedge.exe".into(), caja: caja(100, 100, 1280, 720) },
                Ventana { id: 333, titulo: "Otra pestaña - Microsoft Edge".into(), proceso: "msedge.exe".into(), caja: caja(0, 0, 640, 480) },
            ])
        }
        fn capturar(&self, donde: &Donde, salida: Option<&Path>) -> Result<Captura, String> {
            self.pedido.borrow_mut().push(format!("capturar {donde:?} en {salida:?}"));
            Ok(Captura {
                archivo: salida.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(r"C:\Capturas\winshotx-1.png")),
                ancho: donde.caja.ancho,
                alto: donde.caja.alto,
            })
        }
        fn grabar(&self, donde: &Donde, opciones: &OpcionesDeGrabar) -> Result<String, String> {
            if self.grabando.borrow().is_some() {
                return Err("A recording is already running.".into());
            }
            self.pedido.borrow_mut().push(format!("grabar {donde:?} con {opciones:?}"));
            *self.grabando.borrow_mut() = Some("abc123".into());
            Ok("abc123".into())
        }
        fn parar(&self) -> Result<Parada, String> {
            let sesion = self.grabando.borrow_mut().take().ok_or("Nothing is being recorded.")?;
            self.pedido.borrow_mut().push("parar".into());
            Ok(Parada { sesion, segundos: 12.34567, por_tope: false })
        }
        fn exportar(&self, pedido: &PedidoDeExportar) -> Result<Exportado, String> {
            self.pedido.borrow_mut().push(format!("exportar {pedido:?}"));
            Ok(Exportado { archivo: pedido.salida.clone().unwrap_or_default(), segundos: 7.5, bytes: 1234 })
        }
        fn abrir(&self, sesion: &str) -> Result<(), String> {
            self.pedido.borrow_mut().push(format!("abrir {sesion}"));
            Ok(())
        }
    }

    fn pide(doble: &Doble, linea: &str) -> Value {
        serde_json::from_str(&atender(linea, doble)).unwrap()
    }

    fn error_de(doble: &Doble, linea: &str) -> String {
        let r = pide(doble, linea);
        assert_eq!(r["ok"], false, "tenía que fallar: {linea} -> {r}");
        r["error"].as_str().unwrap().to_string()
    }

    #[test]
    fn status_dice_la_version_y_si_se_graba() {
        let d = Doble::default();
        let r = pide(&d, r#"{"command":"status"}"#);
        assert_eq!(r, json!({"ok": true, "version": "0.4.5", "recording": false, "session": null}));
        *d.grabando.borrow_mut() = Some("s1".into());
        let r = pide(&d, r#"{"command":"status"}"#);
        assert_eq!((r["recording"].clone(), r["session"].clone()), (json!(true), json!("s1")));
    }

    #[test]
    fn screens_y_windows_con_los_nombres_del_contrato() {
        let d = Doble::default();
        let r = pide(&d, r#"{"command":"screens"}"#);
        assert_eq!(
            r["screens"][0],
            json!({"index":0,"x":0,"y":0,"width":1920,"height":1080,"scale":1.25,"primary":true,"name":"UNO"})
        );
        assert_eq!(r["screens"].as_array().unwrap().len(), 2);
        let r = pide(&d, r#"{"command":"windows"}"#);
        assert_eq!(
            r["windows"][1],
            json!({"id":222,"title":"winshotx - Microsoft Edge","process":"msedge.exe","x":100,"y":100,"width":1280,"height":720})
        );
    }

    #[test]
    fn json_roto_o_sin_orden_da_un_error_que_dice_que_hacer() {
        let d = Doble::default();
        assert!(error_de(&d, "{\"command\": ").contains("not valid JSON"));
        assert!(error_de(&d, "[1,2]").contains("JSON object"));
        assert!(error_de(&d, "{}").contains("Missing \"command\""));
        let e = error_de(&d, r#"{"command":"grabar"}"#);
        assert!(e.contains("Unknown command \"grabar\"") && e.contains("record"), "{e}");
        // Nada de eso ha llegado a la app.
        assert!(d.pedido.borrow().is_empty());
    }

    #[test]
    fn un_campo_mal_escrito_no_se_ignora() {
        let d = Doble::default();
        let e = error_de(&d, r#"{"command":"record","screen":0,"systemaudio":true}"#);
        assert!(e.contains("\"systemaudio\"") && e.contains("systemAudio"), "{e}");
        assert!(error_de(&d, r#"{"command":"status","verbose":true}"#).contains("takes no fields"));
        assert!(d.pedido.borrow().is_empty());
    }

    #[test]
    fn capture_de_una_pantalla_va_a_su_caja_y_sin_salida_a_la_carpeta_de_siempre() {
        let d = Doble::default();
        let r = pide(&d, r#"{"command":"capture","screen":1}"#);
        assert_eq!(r, json!({"ok":true,"file":r"C:\Capturas\winshotx-1.png","width":1080,"height":1920}));
        assert_eq!(
            d.pedido.borrow()[0],
            format!("capturar {:?} en None", Donde { caja: caja(1920, 0, 1080, 1920), ventana: None })
        );
    }

    #[test]
    fn capture_de_una_ventana_por_trozo_de_titulo_coge_la_de_delante() {
        let d = Doble::default();
        let r = pide(&d, r#"{"command":"capture","window":"microsoft EDGE","output":"C:\\x\\v.png"}"#);
        assert_eq!(r["file"], r"C:\x\v.png");
        assert_eq!(
            d.pedido.borrow()[0],
            format!(
                "capturar {:?} en {:?}",
                Donde { caja: caja(100, 100, 1280, 720), ventana: Some(222) },
                Some(Path::new(r"C:\x\v.png"))
            )
        );
        // Por el programa, si ningún título lo tiene; y por id.
        pide(&d, r#"{"command":"capture","window":"notepad.exe"}"#);
        assert!(d.pedido.borrow()[1].contains("ventana: Some(111)"));
        pide(&d, r#"{"command":"capture","window":333}"#);
        assert!(d.pedido.borrow()[2].contains("ventana: Some(333)"));
    }

    #[test]
    fn una_ventana_que_no_esta_lo_dice_y_ensenna_cuales_hay() {
        let d = Doble::default();
        let e = error_de(&d, r#"{"command":"capture","window":"Photoshop"}"#);
        assert!(e.contains("\"Photoshop\"") && e.contains("\"Notas - Bloc de notas\""), "{e}");
        assert!(error_de(&d, r#"{"command":"record","window":999}"#).contains("id 999"));
        assert!(error_de(&d, r#"{"command":"capture","screen":7}"#).contains("2 screen(s)"));
    }

    #[test]
    fn la_salida_de_una_captura_es_absoluta_y_png() {
        let d = Doble::default();
        assert!(error_de(&d, r#"{"command":"capture","screen":0,"output":"foto.png"}"#).contains("absolute path"));
        assert!(error_de(&d, r#"{"command":"capture","screen":0,"output":"C:\\x\\foto.jpg"}"#).contains(".png"));
        let r = pide(&d, r#"{"command":"capture","screen":0,"output":"C:\\x\\foto"}"#);
        assert_eq!(r["file"], r"C:\x\foto.png");
    }

    #[test]
    fn una_zona_se_recorta_a_las_pantallas() {
        let d = Doble::default();
        // Cruza de la 0 a la 1 y se sale por arriba: la captura se queda con lo que cubren las dos.
        pide(&d, r#"{"command":"capture","region":{"x":1800,"y":-50,"width":300,"height":200}}"#);
        assert!(d.pedido.borrow()[0].contains(&format!("{:?}", caja(1800, 0, 300, 150))), "{:?}", d.pedido.borrow());
        // La grabación, solo con la pantalla donde cae más (la 1: 180 de ancho contra 120).
        pide(&d, r#"{"command":"record","region":{"x":1800,"y":0,"width":300,"height":200}}"#);
        assert!(d.pedido.borrow()[1].contains(&format!("{:?}", caja(1920, 0, 180, 200))), "{:?}", d.pedido.borrow());
        // Fuera de todo, error; y demasiado pequeña, también.
        assert!(error_de(&d, r#"{"command":"capture","region":{"x":-900,"y":0,"width":100,"height":100}}"#).contains("outside every screen"));
        assert!(error_de(&d, r#"{"command":"capture","region":{"x":0,"y":0,"width":1,"height":100}}"#).contains("at least 2"));
        assert!(error_de(&d, r#"{"command":"capture","region":{"x":0,"y":0,"w":10,"h":10}}"#).contains("\"region\" must look like"));
    }

    #[test]
    fn un_objetivo_y_solo_uno() {
        let d = Doble::default();
        assert!(error_de(&d, r#"{"command":"capture"}"#).contains("Say what to use"));
        let e = error_de(&d, r#"{"command":"record","screen":0,"window":"Edge"}"#);
        assert!(e.contains("only one") && e.contains("\"screen\" and \"window\""), "{e}");
        assert!(error_de(&d, r#"{"command":"record","screen":-1}"#).contains("screen index"));
    }

    #[test]
    fn record_sin_opciones_es_mp4_callado_y_con_lo_de_los_ajustes() {
        let d = Doble::default();
        let r = pide(&d, r#"{"command":"record","screen":0}"#);
        assert_eq!(r, json!({"ok":true,"session":"abc123","region":{"x":0,"y":0,"width":1920,"height":1080}}));
        assert_eq!(
            d.pedido.borrow()[0],
            format!("grabar {:?} con {:?}", Donde { caja: caja(0, 0, 1920, 1080), ventana: None }, OpcionesDeGrabar::default())
        );
        // La marimba, apagada si no se pide.
        assert!(!OpcionesDeGrabar::default().marimba);
    }

    #[test]
    fn record_con_todas_sus_opciones() {
        let d = Doble::default();
        pide(
            &d,
            r#"{"command":"record","window":"Edge","camera":true,"systemAudio":false,"microphone":true,"cue":true,"format":"gif","maxSeconds":30}"#,
        );
        let esperado = OpcionesDeGrabar {
            camara: Some(true),
            sonido_pc: Some(false),
            microfono: Some(true),
            marimba: true,
            formato: Formato::Gif,
            max_segundos: Some(30.0),
        };
        assert_eq!(
            d.pedido.borrow()[0],
            format!("grabar {:?} con {esperado:?}", Donde { caja: caja(100, 100, 1280, 720), ventana: Some(222) })
        );
        assert!(error_de(&d, r#"{"command":"record","screen":0,"maxSeconds":0}"#).contains("greater than 0"));
        assert!(error_de(&d, r#"{"command":"record","screen":0,"format":"webm"}"#).contains("\"mp4\" or \"gif\""));
        assert!(error_de(&d, r#"{"command":"record","screen":0,"camera":"yes"}"#).contains("true or false"));
    }

    #[test]
    fn con_una_grabacion_en_marcha_record_falla_y_stop_la_devuelve() {
        let d = Doble::default();
        pide(&d, r#"{"command":"record","screen":0}"#);
        assert!(error_de(&d, r#"{"command":"record","screen":1}"#).contains("already running"));
        let r = pide(&d, r#"{"command":"stop"}"#);
        assert_eq!(r, json!({"ok":true,"session":"abc123","seconds":12.346}));
        assert!(error_de(&d, r#"{"command":"stop"}"#).contains("Nothing is being recorded"));
    }

    #[test]
    fn export_entero() {
        let d = Doble::default();
        let r = pide(
            &d,
            r#"{"command":"export","session":"abc123","output":"C:\\v\\demo.MP4","style":" Tutorial ","trim":{"start":1.5,"end":9},"zoomOnClicks":0.5,"speed":2}"#,
        );
        assert_eq!(r, json!({"ok":true,"file":r"C:\v\demo.MP4","seconds":7.5,"bytes":1234}));
        let esperado = PedidoDeExportar {
            sesion: "abc123".into(),
            salida: Some(PathBuf::from(r"C:\v\demo.MP4")),
            formato: Some(Formato::Mp4),
            estilo: Some("Tutorial".into()),
            desde: Some(1.5),
            hasta: Some(9.0),
            zoom_en_clics: Some(0.5),
            velocidad: Some(2.0),
        };
        assert_eq!(d.pedido.borrow()[0], format!("exportar {esperado:?}"));
    }

    #[test]
    fn export_lo_minimo_y_lo_que_no_vale() {
        let d = Doble::default();
        pide(&d, r#"{"command":"export","session":"abc123"}"#);
        assert_eq!(
            d.pedido.borrow()[0],
            format!("exportar {:?}", PedidoDeExportar { sesion: "abc123".into(), ..Default::default() })
        );
        pide(&d, r#"{"command":"export","session":"abc123","format":"gif","trim":{"start":2}}"#);
        assert!(d.pedido.borrow()[1].contains("formato: Some(Gif)") && d.pedido.borrow()[1].contains("desde: Some(2.0), hasta: None"));
        for (linea, trozo) in [
            (r#"{"command":"export"}"#, "Missing \"session\""),
            (r#"{"command":"export","session":"..\\..\\x"}"#, "not a session id"),
            (r#"{"command":"export","session":"a","output":"C:\\v\\demo.webm"}"#, ".mp4 or .gif"),
            (r#"{"command":"export","session":"a","output":"C:\\v\\demo.gif","format":"mp4"}"#, "make them match"),
            (r#"{"command":"export","session":"a","output":"demo.mp4"}"#, "absolute path"),
            (r#"{"command":"export","session":"a","trim":{"start":5,"end":2}}"#, "not after its start"),
            (r#"{"command":"export","session":"a","trim":[1,2]}"#, "\"trim\" must look like"),
            (r#"{"command":"export","session":"a","zoomOnClicks":2}"#, "from 0 (no zoom) to 1"),
            (r#"{"command":"export","session":"a","speed":10}"#, "0.25 to 4"),
            (r#"{"command":"export","session":"a","style":""}"#, "name of a style"),
        ] {
            assert!(error_de(&d, linea).contains(trozo), "{linea}");
        }
        assert_eq!(d.pedido.borrow().len(), 2, "lo que no vale no llega a la app");
    }

    #[test]
    fn open_abre_esa_sesion() {
        let d = Doble::default();
        assert_eq!(pide(&d, r#"{"command":"open","session":"abc123"}"#), json!({"ok":true,"session":"abc123"}));
        assert_eq!(d.pedido.borrow()[0], "abrir abc123");
    }

    #[test]
    fn la_respuesta_es_una_sola_linea() {
        let d = Doble::default();
        for linea in [r#"{"command":"screens"}"#, r#"{"command":"windows"}"#, "roto"] {
            assert!(!atender(linea, &d).contains('\n'));
        }
    }
}
