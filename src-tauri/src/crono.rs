//! Cronometro del camino del atajo, para medir en la aplicacion de verdad.
//!
//! Solo escribe si la aplicacion arranco con `--crono`. Cada marca es una linea en
//! `%TEMP%\winshotx\crono.log` con los milisegundos desde el arranque y el nombre de la
//! etapa, y las marcas del frontend llegan por el comando `crono_marca`, asi que todas van
//! con el mismo reloj. `scripts/cronometrar-atajo.mjs` dispara capturas con `--capture`,
//! las cierra con `--cancel` y lee este archivo.
//!
//! Existe porque el camino del atajo hasta ver la seleccion se midio tres veces con
//! `println!` en release (donde no hay consola) o con binarios sin interfaz dentro, y las
//! tres salieron mal. Ver las trampas 33 y 36.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use parking_lot::Mutex;

static ACTIVO: AtomicBool = AtomicBool::new(false);
static ORIGEN: OnceLock<Instant> = OnceLock::new();
/// Un solo escritor a la vez: las tres ventanas del overlay marcan a la vez y sin esto las
/// lineas salian mezcladas unas dentro de otras.
static ESCRITOR: Mutex<()> = Mutex::new(());

/// Se llama una vez al arrancar, si se pidio con `--crono`.
pub fn activar() {
    ORIGEN.get_or_init(Instant::now);
    ACTIVO.store(true, Ordering::Relaxed);
}

pub fn activo() -> bool {
    ACTIVO.load(Ordering::Relaxed)
}

/// Donde se escribe: junto a lo demas de winshotx en la carpeta temporal.
pub fn archivo() -> std::path::PathBuf {
    std::env::temp_dir().join("winshotx").join("crono.log")
}

/// Apunta una etapa. Sin `--crono` no cuesta mas que leer un booleano.
pub fn marca(etapa: &str) {
    if !activo() {
        return;
    }
    let ms = ORIGEN
        .get()
        .map(|o| o.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    // La linea se monta entera antes de escribirla, en UNA sola llamada: `writeln!` escribe
    // cada trozo por separado y con tres hilos a la vez se entrelazaban.
    let linea = format!("{ms:.1}\t{etapa}\n");
    let ruta = archivo();
    if let Some(padre) = ruta.parent() {
        let _ = std::fs::create_dir_all(padre);
    }
    let _guardia = ESCRITOR.lock();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&ruta) {
        let _ = f.write_all(linea.as_bytes());
    }
}
