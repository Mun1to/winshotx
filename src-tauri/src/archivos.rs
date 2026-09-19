//! Dónde cae cada archivo que se lleva el usuario, y cómo se llama.
//!
//! **Un solo sitio.** Hasta ahora lo decidían tres por su cuenta: guardar una captura desde
//! la barra, exportar desde el editor y guardar una captura anclada. Tres sitios que hacen
//! lo mismo son tres sitios que se separan, y ya empezaban a hacerlo.
//!
//! Y los tres tenían el mismo fallo: el nombre lleva la hora **al segundo**, y escribir un
//! PNG sobre uno que ya existe no falla ni avisa. **Dos capturas guardadas dentro del mismo
//! segundo dejaban una sola**, y la que se perdía era la primera. Con el atajo de teclado
//! eso no es un caso raro: es lo que pasa al capturar dos veces seguidas.

use std::path::{Path, PathBuf};

/// La hora, al segundo, como se escribe en el nombre de un archivo.
pub fn marca_de_tiempo() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Un nombre que **no existe todavía** en esa carpeta.
///
/// Si `winshotx-20260827-154810.png` está ocupado prueba con `-2`, luego `-3`, y así. El
/// sufijo va antes de la extensión, no detrás, para que el archivo siga abriéndose con el
/// programa que le toca.
///
/// El tope de mil intentos no es por miedo a un bucle infinito: es para que, si algo va muy
/// mal (una carpeta que dice que todo existe, un disco que no responde), esto devuelva algo
/// en vez de quedarse dando vueltas mientras el usuario mira una captura que no se guarda.
pub fn nombre_libre(dir: &Path, base: &str, extension: &str) -> PathBuf {
    let candidato = dir.join(format!("{base}.{extension}"));
    if !candidato.exists() {
        return candidato;
    }
    for n in 2..1000 {
        let otro = dir.join(format!("{base}-{n}.{extension}"));
        if !otro.exists() {
            return otro;
        }
    }
    // Mil ya ocupados en el mismo segundo no puede pasar de verdad, pero devolver el
    // primero seria pisar un archivo, que es justo lo que este modulo existe para evitar.
    dir.join(format!(
        "{base}-{}.{extension}",
        chrono::Local::now().format("%S%3f")
    ))
}

/// La ruta donde escribir lo siguiente que se guarde, con la carpeta ya creada.
pub fn destino(dir: &Path, extension: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    Ok(nombre_libre(dir, &format!("winshotx-{}", marca_de_tiempo()), extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una carpeta vacia y suya, para que dos pruebas a la vez no se estorben.
    fn carpeta(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("winshotx-test-archivos-{nombre}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tocar(ruta: &Path) {
        std::fs::write(ruta, b"x").unwrap();
    }

    #[test]
    fn en_una_carpeta_vacia_sale_el_nombre_pelado() {
        let dir = carpeta("vacia");
        let ruta = nombre_libre(&dir, "winshotx-20260827-154810", "png");
        assert_eq!(ruta.file_name().unwrap(), "winshotx-20260827-154810.png");
    }

    #[test]
    fn si_ese_ya_existe_no_lo_pisa() {
        // El fallo que este modulo viene a arreglar: dos capturas en el mismo segundo
        // dejaban una sola, y la que se perdia era la primera.
        let dir = carpeta("ocupada");
        let primera = dir.join("winshotx-20260827-154810.png");
        tocar(&primera);
        let segunda = nombre_libre(&dir, "winshotx-20260827-154810", "png");
        assert_ne!(segunda, primera);
        assert_eq!(segunda.file_name().unwrap(), "winshotx-20260827-154810-2.png");
        assert!(primera.exists(), "la primera captura ha desaparecido");
    }

    #[test]
    fn y_va_subiendo_mientras_haga_falta() {
        let dir = carpeta("varias");
        tocar(&dir.join("winshotx-20260827-154810.png"));
        tocar(&dir.join("winshotx-20260827-154810-2.png"));
        tocar(&dir.join("winshotx-20260827-154810-3.png"));
        let ruta = nombre_libre(&dir, "winshotx-20260827-154810", "png");
        assert_eq!(ruta.file_name().unwrap(), "winshotx-20260827-154810-4.png");
    }

    #[test]
    fn el_numero_va_antes_de_la_extension() {
        // Detras de la extension, Windows dejaria de saber con que abrirlo.
        let dir = carpeta("extension");
        tocar(&dir.join("winshotx-x.mp4"));
        let ruta = nombre_libre(&dir, "winshotx-x", "mp4");
        assert_eq!(ruta.extension().unwrap(), "mp4");
    }

    #[test]
    fn cada_formato_lleva_su_cuenta() {
        // Un PNG ocupado no obliga a que el MP4 salga con un -2 detras.
        let dir = carpeta("formatos");
        tocar(&dir.join("winshotx-x.png"));
        let ruta = nombre_libre(&dir, "winshotx-x", "mp4");
        assert_eq!(ruta.file_name().unwrap(), "winshotx-x.mp4");
    }

    #[test]
    fn destino_crea_la_carpeta_si_no_esta() {
        // Quien elige una carpeta en los ajustes puede borrarla despues, y guardar tiene
        // que seguir funcionando en vez de fallar con un error del sistema.
        let dir = carpeta("crear").join("dentro").join("mas");
        assert!(!dir.exists());
        let ruta = destino(&dir, "png").unwrap();
        assert!(dir.exists());
        assert_eq!(ruta.extension().unwrap(), "png");
        assert!(ruta.file_name().unwrap().to_string_lossy().starts_with("winshotx-"));
    }

    #[test]
    fn dos_destinos_seguidos_no_son_el_mismo_archivo() {
        // Sin escribir nada en medio esto SI puede repetirse, porque el nombre solo mira
        // si el archivo existe. Se comprueba el caso de verdad: guardar y volver a pedir.
        let dir = carpeta("seguidos");
        let uno = destino(&dir, "png").unwrap();
        tocar(&uno);
        let dos = destino(&dir, "png").unwrap();
        assert_ne!(uno, dos);
    }

    #[test]
    fn la_marca_de_tiempo_tiene_la_forma_que_espera_el_nombre() {
        let marca = marca_de_tiempo();
        assert_eq!(marca.len(), 15, "{marca}");
        assert_eq!(marca.as_bytes()[8], b'-');
        assert!(marca.bytes().filter(|b| b.is_ascii_digit()).count() == 14);
    }
}
