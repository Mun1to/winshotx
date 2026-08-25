use std::path::Path;

fn main() {
    // El frontend va EMBEBIDO en el binario. Cargo solo mira los .rs para decidir si hay
    // que recompilar, asi que tocar unicamente la interfaz dejaba el .exe intacto, con la
    // version anterior dentro: la compilacion decia "Finished" y publicaba una interfaz
    // vieja sin que saltara ni un aviso. Pasó el 25 de agosto de 2026 con la barra de
    // modos del overlay.
    //
    // Con esto, cualquier cambio en dist/ obliga a volver a enlazar el binario.
    marcar(Path::new("../dist"));

    tauri_build::build()
}

/// Le dice a cargo que vigile este archivo o carpeta, entero y hacia dentro.
fn marcar(ruta: &Path) {
    println!("cargo:rerun-if-changed={}", ruta.display());
    let Ok(entradas) = std::fs::read_dir(ruta) else {
        return;
    };
    for entrada in entradas.flatten() {
        let camino = entrada.path();
        if camino.is_dir() {
            marcar(&camino);
        } else {
            println!("cargo:rerun-if-changed={}", camino.display());
        }
    }
}
