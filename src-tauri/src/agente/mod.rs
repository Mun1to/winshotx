//! El canal para agentes de IA (S15): que un agente (Claude Code, Codex, Cursor...) grabe la
//! pantalla, pare y saque un MP4 o un GIF, o haga una captura, sin que nadie toque el ratón, y
//! que sepa dónde ha quedado el archivo.
//!
//! Grabar y exportar necesitan el `AppHandle` de la app que vive en la bandeja, así que es la
//! app la que abre el canal: una tubería con nombre que solo puede abrir este mismo usuario
//! (`tuberia`), con una orden por línea JSON y una respuesta por línea (`ordenes`), atendidas
//! sobre lo que la app ya hace (`en_la_app`). Dos clientes hablan con ella: la orden de terminal
//! `winshotx.exe agent` (`linea`) y el servidor MCP `winshotx.exe mcp` (`mcp`, de S16). Los dos
//! pasan por `cliente::pedir`, que si la app no está abierta la arranca y espera.
//!
//! **La API es pública y para agentes de cualquier sitio:** los nombres de las órdenes, de los
//! campos y los errores van en inglés. El código de dentro, en español, como el resto.
//!
//! Una grabación pedida por un agente se ve igual que una normal (el marco rojo y la barra),
//! para que quien está delante sepa que se graba. No se esconde a propósito.

pub mod cliente;
mod consola;
#[cfg(windows)]
mod en_la_app;
pub mod linea;
pub mod mcp;
pub mod ordenes;
pub(crate) mod traduccion;
pub mod tuberia;

/// Lo que hace `winshotx.exe` cuando lo llaman como cliente del canal, ANTES de montar la app.
///
/// Tiene que ir antes del `Builder` y de su plugin de una sola instancia: si no, ese plugin se
/// traga los argumentos, se los manda a la app que ya está abierta y este proceso se cierra sin
/// contestar nada. Así `agent` y `mcp` no son «otra instancia»: son clientes, y salen con su
/// código en cuanto acaban. `None` es que no es ninguna de las dos y la app arranca como siempre.
pub fn desde_la_linea_de_ordenes() -> Option<i32> {
    // `args_os` y no `args`: `args` se cae con un argumento que no sea Unicode válido, y esto
    // corre en cada arranque de la app, antes que nada.
    let args: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    match args.first().map(String::as_str) {
        Some("agent") => {
            consola::preparar();
            Some(linea::ejecutar(&args[1..]))
        }
        Some("mcp") => {
            consola::preparar();
            Some(mcp::servir())
        }
        // La que usa `cliente::arrancar_app` para lanzar la app fuera del árbol de procesos de
        // quien la pide. No escribe nada.
        Some(cliente::LANZADERA) => Some(cliente::hacer_de_lanzadera()),
        _ => None,
    }
}

/// Abre el canal con la app. Lo llama `lib.rs::run` en el `setup`, con el estado ya puesto.
///
/// Si no se puede abrir (otro programa tiene ya la tubería con ese nombre), se dice por consola
/// y la app sigue: quedarse sin winshotx por no poder hablar con agentes sería peor.
pub fn arrancar(app: &tauri::AppHandle) {
    #[cfg(windows)]
    {
        let manejador = en_la_app::manejador(app.clone());
        match tuberia::escuchar(&tuberia::nombre(), manejador) {
            Ok(()) => eprintln!("[agente] escuchando en {}", tuberia::nombre()),
            Err(error) => eprintln!("[agente] sin canal para agentes: {error}"),
        }
    }
    #[cfg(not(windows))]
    let _ = app;
}
