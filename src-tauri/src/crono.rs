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
//!
//! **Desde S29b los milisegundos cuentan desde que Windows creo el proceso**, no desde que
//! el cronometro se encendio: asi «arrancar hasta la bandeja» incluye cargar el exe y sus
//! DLL, que es parte de lo que espera quien lo abre. Y cada linea lleva una tercera columna
//! con la hora del reloj de pared (milisegundos desde 1970), para poner en la misma regla
//! las marcas de dos procesos: la segunda instancia que pide los ajustes y la que los abre.

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

/// La hora de pared (milisegundos desde 1970) a la que nacio el proceso: el cero de `ORIGEN`.
static NACIO_MS: OnceLock<f64> = OnceLock::new();

/// Se llama una vez al arrancar, si se pidio con `--crono`.
pub fn activar() {
    let edad = edad_del_proceso();
    ORIGEN.get_or_init(|| Instant::now().checked_sub(edad).unwrap_or_else(Instant::now));
    NACIO_MS.get_or_init(|| ahora_ms() - edad.as_secs_f64() * 1000.0);
    ACTIVO.store(true, Ordering::Relaxed);
}

/// Milisegundos desde 1970 segun el reloj de pared.
fn ahora_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// Lo que lleva vivo este proceso, segun la hora de creacion que guarda Windows. Fuera de
/// Windows, o si no contesta, cero: el cronometro cuenta entonces desde que se encendio.
fn edad_del_proceso() -> std::time::Duration {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        let (mut nacio, mut salio, mut kernel, mut usuario) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let bien = unsafe {
            GetProcessTimes(GetCurrentProcess(), &mut nacio, &mut salio, &mut kernel, &mut usuario)
        }
        .is_ok();
        if bien {
            // FILETIME cuenta de 100 en 100 nanosegundos desde 1601.
            const DE_1601_A_1970: u64 = 116_444_736_000_000_000;
            let nacio = (u64::from(nacio.dwHighDateTime) << 32) | u64::from(nacio.dwLowDateTime);
            let ahora = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64 / 100 + DE_1601_A_1970)
                .unwrap_or(0);
            if ahora > nacio {
                return std::time::Duration::from_nanos((ahora - nacio) * 100);
            }
        }
    }
    std::time::Duration::ZERO
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
    let pared = NACIO_MS.get().copied().unwrap_or(0.0) + ms;
    let linea = format!("{ms:.1}\t{etapa}\t{pared:.1}\n");
    let ruta = archivo();
    if let Some(padre) = ruta.parent() {
        let _ = std::fs::create_dir_all(padre);
    }
    let _guardia = ESCRITOR.lock();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&ruta) {
        let _ = f.write_all(linea.as_bytes());
    }
}
