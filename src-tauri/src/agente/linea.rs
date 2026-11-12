//! `winshotx.exe agent <orden> [opciones]`: la orden de terminal del canal de agentes.
//!
//! Convierte las opciones de guiones en la petición JSON del contrato, se la manda a la app
//! (`cliente::pedir`, que la arranca si hace falta) y escribe la respuesta en UNA línea JSON en
//! la salida estándar. Sale con 0 si la respuesta es `"ok":true` y con 1 si no. Un error de
//! las opciones también sale como JSON, para que un agente siempre tenga algo que leer.

use serde_json::{json, Map, Value};

use super::cliente;

/// `winshotx agent --help`, en inglés y en diez líneas.
pub const AYUDA: &str = "\
winshotx agent <command> [options]: let an AI agent record the screen, export and capture. Prints one JSON line; exit code 0 = ok.
  status | screens | windows     what is going on, the screens, and the visible windows with their ids
  capture <target> [--out shot.png]                    target: --screen N | --window <id or part of the title> | --region x,y,w,h
  record  <target> [--format mp4|gif] [--max-seconds N] [--camera|--no-camera] [--mic|--no-mic] [--system-audio|--no-system-audio] [--cue]
  stop                           stops and saves the recording (no editor opens) and prints its session id and length
  export  <session> [--out demo.mp4|demo.gif] [--style NAME] [--trim start,end] [--zoom-on-clicks 0..1] [--speed 0.25..4]
  open    <session>              opens it in the winshotx editor, for a person to polish
Pixels are physical, on the virtual desktop (see screens). Without --out, files go to the folder in winshotx's settings.
Unset options follow winshotx's settings, except the start sound (--cue), which is off. The red frame shows while recording.
Example: winshotx agent record --screen 0 --max-seconds 20   then   winshotx agent stop   then   winshotx agent export <session> --out demo.mp4";

/// Cómo se lee el valor de una opción.
#[derive(Clone, Copy, PartialEq)]
enum Tipo {
    Entero,
    Numero,
    Texto,
    /// Un número es un id; cualquier otra cosa, un trozo del título.
    Ventana,
    /// `x,y,ancho,alto`.
    Zona,
    /// `inicio,fin`, cualquiera de los dos vacío.
    Recorte,
    /// `--camera` es sí, `--no-camera` es no, y `--camera=false` también.
    SiNo,
}

struct Opcion {
    bandera: &'static str,
    campo: &'static str,
    tipo: Tipo,
    ordenes: &'static [&'static str],
}

const OBJETIVO: &[&str] = &["capture", "record"];

const OPCIONES: &[Opcion] = &[
    Opcion { bandera: "screen", campo: "screen", tipo: Tipo::Entero, ordenes: OBJETIVO },
    Opcion { bandera: "window", campo: "window", tipo: Tipo::Ventana, ordenes: OBJETIVO },
    Opcion { bandera: "region", campo: "region", tipo: Tipo::Zona, ordenes: OBJETIVO },
    Opcion { bandera: "out", campo: "output", tipo: Tipo::Texto, ordenes: &["capture", "export"] },
    Opcion { bandera: "output", campo: "output", tipo: Tipo::Texto, ordenes: &["capture", "export"] },
    Opcion { bandera: "camera", campo: "camera", tipo: Tipo::SiNo, ordenes: &["record"] },
    Opcion { bandera: "mic", campo: "microphone", tipo: Tipo::SiNo, ordenes: &["record"] },
    Opcion { bandera: "microphone", campo: "microphone", tipo: Tipo::SiNo, ordenes: &["record"] },
    Opcion { bandera: "system-audio", campo: "systemAudio", tipo: Tipo::SiNo, ordenes: &["record"] },
    Opcion { bandera: "cue", campo: "cue", tipo: Tipo::SiNo, ordenes: &["record"] },
    Opcion { bandera: "format", campo: "format", tipo: Tipo::Texto, ordenes: &["record", "export"] },
    Opcion { bandera: "max-seconds", campo: "maxSeconds", tipo: Tipo::Numero, ordenes: &["record"] },
    Opcion { bandera: "session", campo: "session", tipo: Tipo::Texto, ordenes: &["export", "open"] },
    Opcion { bandera: "style", campo: "style", tipo: Tipo::Texto, ordenes: &["export"] },
    Opcion { bandera: "trim", campo: "trim", tipo: Tipo::Recorte, ordenes: &["export"] },
    Opcion { bandera: "zoom-on-clicks", campo: "zoomOnClicks", tipo: Tipo::Numero, ordenes: &["export"] },
    Opcion { bandera: "speed", campo: "speed", tipo: Tipo::Numero, ordenes: &["export"] },
];

const ORDENES: &[&str] = &["status", "screens", "windows", "capture", "record", "stop", "export", "open"];

/// Lo que hay que hacer con la línea de órdenes.
#[derive(Debug, PartialEq)]
pub enum Accion {
    Ayuda,
    Pedir(Value),
}

/// Corre `agent` entero y devuelve el código de salida.
pub fn ejecutar(args: &[String]) -> i32 {
    let (texto, codigo) = match leer(args) {
        Ok(Accion::Ayuda) => (AYUDA.to_string(), 0),
        Ok(Accion::Pedir(peticion)) => {
            let respuesta = cliente::pedir(&peticion).unwrap_or_else(|e| json!({ "ok": false, "error": e }));
            let codigo = if respuesta["ok"] == Value::Bool(true) { 0 } else { 1 };
            (respuesta.to_string(), codigo)
        }
        Err(error) => (json!({ "ok": false, "error": error }).to_string(), 1),
    };
    // Sin `println!`: si quien lee ya ha cerrado la tubería, `println!` se cae, y en release eso
    // aborta el proceso. Aquí se escribe y, si no se puede, el código de salida lo dice igual.
    use std::io::Write;
    let mut salida = std::io::stdout().lock();
    let _ = writeln!(salida, "{texto}");
    let _ = salida.flush();
    codigo
}

/// De los argumentos que van detrás de `agent` a la petición del contrato.
pub fn leer(args: &[String]) -> Result<Accion, String> {
    if args.iter().any(|a| a == "--help" || a == "-h" || a == "/?") {
        return Ok(Accion::Ayuda);
    }
    let Some((orden, resto)) = args.split_first() else {
        return Err("Missing command. Run `winshotx agent --help` to see them.".into());
    };
    let orden = orden.as_str();
    if orden == "help" {
        return Ok(Accion::Ayuda);
    }
    if !ORDENES.contains(&orden) {
        return Err(format!(
            "Unknown command \"{orden}\". Use one of: {}. Run `winshotx agent --help`.",
            ORDENES.join(", ")
        ));
    }
    let mut peticion = Map::new();
    peticion.insert("command".into(), Value::String(orden.into()));
    let mut i = 0;
    while i < resto.len() {
        let arg = &resto[i];
        i += 1;
        let Some(bandera) = arg.strip_prefix("--") else {
            // La sesión puede ir suelta: `export 3f9c0a1b2d --out demo.mp4`.
            if matches!(orden, "export" | "open") && !peticion.contains_key("session") {
                peticion.insert("session".into(), Value::String(arg.clone()));
                continue;
            }
            return Err(format!("Unexpected argument \"{arg}\" for \"{orden}\". Run `winshotx agent --help`."));
        };
        let (nombre, pegado) = match bandera.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (bandera, None),
        };
        let (opcion, negada) = match OPCIONES.iter().find(|o| o.bandera == nombre) {
            Some(o) => (o, false),
            None => match nombre.strip_prefix("no-").and_then(|n| OPCIONES.iter().find(|o| o.bandera == n)) {
                Some(o) if o.tipo == Tipo::SiNo && pegado.is_none() => (o, true),
                _ => return Err(format!("Unknown option --{nombre}. Run `winshotx agent --help`.")),
            },
        };
        if !opcion.ordenes.contains(&orden) {
            return Err(format!(
                "--{nombre} does not apply to \"{orden}\"; it is for: {}.",
                opcion.ordenes.join(", ")
            ));
        }
        let valor = if opcion.tipo == Tipo::SiNo {
            match pegado.as_deref() {
                None => Value::Bool(!negada),
                Some("true" | "yes" | "1") => Value::Bool(true),
                Some("false" | "no" | "0") => Value::Bool(false),
                Some(otro) => return Err(format!("--{nombre} is true or false, not \"{otro}\".")),
            }
        } else {
            let texto = match pegado {
                Some(v) => v,
                None => {
                    let Some(v) = resto.get(i) else {
                        return Err(format!("--{nombre} needs a value. Run `winshotx agent --help`."));
                    };
                    i += 1;
                    v.clone()
                }
            };
            valor_de(opcion, nombre, &texto)?
        };
        peticion.insert(opcion.campo.into(), valor);
    }
    Ok(Accion::Pedir(Value::Object(peticion)))
}

fn valor_de(opcion: &Opcion, nombre: &str, texto: &str) -> Result<Value, String> {
    let numero = |t: &str| -> Result<f64, String> {
        t.trim().parse::<f64>().ok().filter(|v| v.is_finite()).ok_or_else(|| format!("--{nombre} needs a number, not \"{t}\"."))
    };
    Ok(match opcion.tipo {
        Tipo::Texto => Value::String(texto.to_string()),
        Tipo::Numero => json!(numero(texto)?),
        Tipo::Entero => json!(texto
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("--{nombre} needs a whole number, not \"{texto}\"."))?),
        Tipo::Ventana => match texto.trim().parse::<i64>() {
            Ok(id) => json!(id),
            Err(_) => Value::String(texto.to_string()),
        },
        Tipo::Zona => {
            let partes: Vec<Result<i64, _>> = texto.split(',').map(|p| p.trim().parse::<i64>()).collect();
            match partes.as_slice() {
                [Ok(x), Ok(y), Ok(w), Ok(h)] => json!({ "x": x, "y": y, "width": w, "height": h }),
                _ => return Err(format!("--{nombre} is x,y,width,height in pixels, like 0,0,1280,720; not \"{texto}\".")),
            }
        }
        Tipo::Recorte => {
            let Some((a, b)) = texto.split_once(',') else {
                return Err(format!("--{nombre} is start,end in seconds, like 1.5,9 (either can be empty); not \"{texto}\"."));
            };
            let mut recorte = Map::new();
            if !a.trim().is_empty() {
                recorte.insert("start".into(), json!(numero(a)?));
            }
            if !b.trim().is_empty() {
                recorte.insert("end".into(), json!(numero(b)?));
            }
            Value::Object(recorte)
        }
        Tipo::SiNo => unreachable!("los sí/no se leen aparte"),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn pide(linea: &str) -> Value {
        let args: Vec<String> = linea.split_whitespace().map(String::from).collect();
        match leer(&args) {
            Ok(Accion::Pedir(v)) => v,
            otro => panic!("{linea}: {otro:?}"),
        }
    }

    fn error(linea: &str) -> String {
        let args: Vec<String> = linea.split_whitespace().map(String::from).collect();
        leer(&args).expect_err(linea)
    }

    #[test]
    fn la_ayuda_cabe_en_diez_lineas_y_sale_con_cualquier_forma_de_pedirla() {
        assert_eq!(AYUDA.lines().count(), 10);
        for linea in [vec!["--help"], vec!["help"], vec!["record", "-h"], vec!["export", "x", "/?"]] {
            let args: Vec<String> = linea.into_iter().map(String::from).collect();
            assert_eq!(leer(&args), Ok(Accion::Ayuda));
        }
        // Y nombra cada orden y cada opción que hay.
        for orden in ORDENES {
            assert!(AYUDA.contains(orden), "{orden}");
        }
        for o in OPCIONES.iter().filter(|o| !["output", "microphone", "session"].contains(&o.bandera)) {
            assert!(AYUDA.contains(&format!("--{}", o.bandera)), "--{}", o.bandera);
        }
    }

    #[test]
    fn las_ordenes_sin_opciones() {
        for orden in ["status", "screens", "windows", "stop"] {
            assert_eq!(pide(orden), json!({ "command": orden }));
        }
    }

    #[test]
    fn capture_con_cada_objetivo() {
        assert_eq!(pide("capture --screen 0"), json!({"command":"capture","screen":0}));
        assert_eq!(pide("capture --window 132456 --out a.png"), json!({"command":"capture","window":132456,"output":"a.png"}));
        assert_eq!(pide("capture --window=Edge"), json!({"command":"capture","window":"Edge"}));
        assert_eq!(
            pide("capture --region -1920,0,1280,720 --output C:\\x\\a.png"),
            json!({"command":"capture","region":{"x":-1920,"y":0,"width":1280,"height":720},"output":"C:\\x\\a.png"})
        );
        // Un título con espacios llega entero si va entre comillas (aquí, un solo argumento).
        let args: Vec<String> = ["capture", "--window", "Bloc de notas"].map(String::from).to_vec();
        assert_eq!(leer(&args), Ok(Accion::Pedir(json!({"command":"capture","window":"Bloc de notas"}))));
    }

    #[test]
    fn record_con_todas_sus_opciones() {
        assert_eq!(
            pide("record --screen 1 --camera --no-mic --system-audio=false --cue --format gif --max-seconds 30"),
            json!({"command":"record","screen":1,"camera":true,"microphone":false,"systemAudio":false,"cue":true,"format":"gif","maxSeconds":30.0})
        );
        assert_eq!(pide("record --window Edge --no-camera --mic"), json!({"command":"record","window":"Edge","camera":false,"microphone":true}));
    }

    #[test]
    fn export_y_open_con_la_sesion_suelta_o_con_su_opcion() {
        assert_eq!(
            pide("export 3f9c0a1b2d --out demo.mp4 --style Tutorial --trim 1.5,9 --zoom-on-clicks 0.5 --speed 2"),
            json!({"command":"export","session":"3f9c0a1b2d","output":"demo.mp4","style":"Tutorial","trim":{"start":1.5,"end":9.0},"zoomOnClicks":0.5,"speed":2.0})
        );
        assert_eq!(pide("export --session abc --trim ,4 --format gif"), json!({"command":"export","session":"abc","trim":{"end":4.0},"format":"gif"}));
        assert_eq!(pide("export abc --trim 2,"), json!({"command":"export","session":"abc","trim":{"start":2.0}}));
        assert_eq!(pide("open abc"), json!({"command":"open","session":"abc"}));
    }

    #[test]
    fn lo_que_no_vale_dice_que_hacer() {
        assert!(error("").contains("Missing command"));
        assert!(error("grabar --screen 0").contains("Unknown command \"grabar\""));
        assert!(error("record --screen").contains("needs a value"));
        assert!(error("record --screen uno").contains("whole number"));
        assert!(error("record --screen 0 --camra").contains("Unknown option --camra"));
        assert!(error("capture --screen 0 --camera").contains("does not apply to \"capture\"; it is for: record"));
        assert!(error("record --screen 0 --no-format").contains("Unknown option --no-format"));
        assert!(error("record --screen 0 --camera=quizas").contains("true or false"));
        assert!(error("capture --region 0,0,10").contains("x,y,width,height"));
        assert!(error("export abc --trim 5").contains("start,end"));
        assert!(error("export abc --speed rapido").contains("needs a number"));
        assert!(error("export abc otra").contains("Unexpected argument \"otra\""));
        assert!(error("status ahora").contains("Unexpected argument"));
    }
}
