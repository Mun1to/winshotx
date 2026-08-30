//! Los cuatro sitios de fuera a los que winshotx abre el navegador.
//!
//! La lista vive aqui y no en la interfaz porque abrir una direccion es lo unico que hace
//! esta aplicacion que se sale de su propia ventana: quien pide abrir algo manda una
//! cadena, y sin una lista cerrada esa cadena podria ser cualquier cosa, incluida una ruta
//! del disco o un `.exe`. Asi el peor caso es que no se abra nada.
//!
//! La interfaz tiene las mismas cuatro en `src/lib/enlaces.ts`, para poder ensennarlas
//! escritas debajo del boton. Si un dia dejan de coincidir, la direccion nueva no se abre
//! y la prueba de abajo dice cual falta.

/// Invitar a un cafe. Es la unica forma de apoyar winshotx: no hay version de pago.
pub const CAFE: &str = "https://buymeacoffee.com/munito";

/// El codigo, que es de donde sale todo lo demas.
pub const REPO: &str = "https://github.com/Mun1to/winshotx";

/// Contar un fallo o pedir algo, con el formulario que ya pregunta lo que hace falta.
pub const FALLOS: &str = "https://github.com/Mun1to/winshotx/issues/new";

/// La pagina, que es donde se prueba sin instalar nada.
pub const WEB: &str = "https://winshotx.com";

/// Todas, para comprobar contra ellas lo que llegue desde la ventana.
pub const TODOS: &[&str] = &[CAFE, REPO, FALLOS, WEB];

/// Si esa direccion es una de las nuestras.
pub fn permitido(url: &str) -> bool {
    TODOS.contains(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_pasan_las_cuatro_de_la_lista() {
        for url in TODOS {
            assert!(permitido(url), "{url} deberia estar permitida");
        }
        assert!(!permitido("https://buymeacoffee.com/otro"));
        assert!(!permitido("https://github.com/Mun1to/winshotx/../../otro"));
        assert!(!permitido("file:///C:/Windows/System32/cmd.exe"));
        assert!(!permitido("C:\\Windows\\System32\\cmd.exe"));
        assert!(!permitido(""));
    }

    #[test]
    fn todas_son_https_y_sin_espacios() {
        // Un espacio al final de una constante no se ve leyendo el archivo, y convierte la
        // direccion en otra que no existe.
        for url in TODOS {
            assert!(url.starts_with("https://"), "{url} no es https");
            assert_eq!(url.trim(), *url, "{url} lleva espacios");
        }
    }

    #[test]
    fn la_interfaz_tiene_las_mismas() {
        // Las direcciones se ensennan escritas en los ajustes, asi que estan tambien en
        // TypeScript. Si una de las dos listas cambia sola, el boton deja de abrir nada.
        let ts = include_str!("../../src/lib/enlaces.ts");
        for url in TODOS {
            assert!(ts.contains(url), "a src/lib/enlaces.ts le falta {url}");
        }
    }
}
