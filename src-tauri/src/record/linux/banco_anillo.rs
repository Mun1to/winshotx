//! Banco: lo que cuesta en Linux tener encendido «Grabar siempre lo último».
//!
//! Hace lo que haria el anillo: pide la pantalla entera con la captura de grabar (`preparar` y
//! `grabar`, MIT-SHM en X11 y el portal mas PipeWire en Wayland) y mete cada fotograma que llega
//! en un `Anillo` de verdad, en disco. Al acabar dice cuanto procesador ha gastado este proceso,
//! cuanta memoria y cuanto ha escrito. No lleva sonido ni cose nada: es el coste de tenerlo
//! puesto, que es el que se paga toda la tarde.
//!
//! Necesita una pantalla, asi que no entra en la tanda: se lanza a mano dentro de una sesion del
//! laboratorio (`linux-lab/s45/anillo-coste.sh`).
//!   WX_BANCO_PANTALLA=ancho,alto   la pantalla (de 0,0)
//!   WX_BANCO_FPS=15                el ritmo que se pide
//!   WX_BANCO_SEGUNDOS=15           cuanto se mide
//!   WX_BANCO_FICHA=<archivo>       donde se guarda el permiso del escritorio (Wayland)

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{preparar, Marcha, Opciones, Reciclados};
use crate::capture::Rect;
use crate::record::buffer::{bytes_max, Anillo};

/// Lo que lleva gastado de procesador este proceso, con todos sus hilos, en milisegundos.
fn procesador_ms() -> u64 {
    // SAFETY: `getrusage` solo escribe en la estructura que se le pasa.
    let uso = unsafe {
        let mut uso: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut uso);
        uso
    };
    let ms = |t: libc::timeval| t.tv_sec as u64 * 1000 + t.tv_usec as u64 / 1000;
    ms(uso.ru_utime) + ms(uso.ru_stime)
}

/// Un campo de `/proc/self/status` (`VmRSS`, `VmHWM`), en kilobytes.
fn memoria_kb(campo: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|texto| {
            texto.lines().find_map(|linea| {
                linea
                    .strip_prefix(campo)
                    .and_then(|resto| resto.trim_start_matches(':').split_whitespace().next()?.parse().ok())
            })
        })
        .unwrap_or(0)
}

fn de_entorno<T: std::str::FromStr>(nombre: &str, por_defecto: T) -> T {
    std::env::var(nombre).ok().and_then(|v| v.parse().ok()).unwrap_or(por_defecto)
}

#[test]
#[ignore = "banco: necesita una pantalla, se lanza a mano en el laboratorio"]
fn lo_que_cuesta_tener_el_anillo_encendido() {
    let fps: u32 = de_entorno("WX_BANCO_FPS", 15);
    let segundos: u64 = de_entorno("WX_BANCO_SEGUNDOS", 15);
    let pantalla = std::env::var("WX_BANCO_PANTALLA").unwrap_or_else(|_| "1920,1080".into());
    let (ancho, alto) = pantalla
        .split_once(',')
        .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)))
        .expect("WX_BANCO_PANTALLA=ancho,alto");
    let ficha = std::env::var_os("WX_BANCO_FICHA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("winshotx-banco-ficha"));

    // La captura de Wayland mira las pantallas con GDK, que en la app ya esta arrancado.
    gtk::init().expect("arrancar GTK");
    let antes_de_abrir_kb = memoria_kb("VmRSS");
    let region = Rect { x: 0, y: 0, width: ancho, height: alto }.to_even();
    let abrir = Instant::now();
    let preparada = preparar(region, fps, Opciones { ficha, al_cortarse: Box::new(|| eprintln!("BANCO el escritorio ha cortado")) })
        .expect("preparar la captura");
    let region = preparada.region();
    println!("BANCO abierta en {} ms: {}x{} a {fps} por segundo", abrir.elapsed().as_millis(), region.width, region.height);

    let carpeta = std::env::temp_dir().join(format!("winshotx-banco-anillo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&carpeta);
    let ventana_s = 30u32;
    let mut anillo = Anillo::nuevo(
        &carpeta,
        u64::from(ventana_s) * 1000,
        fps,
        bytes_max(ventana_s, fps, region.width, region.height),
    )
    .expect("abrir el anillo");

    let stop = Arc::new(AtomicBool::new(false));
    let marcha = Marcha {
        stop: stop.clone(),
        pause: Arc::new(AtomicBool::new(false)),
        paused_ms: Arc::new(AtomicU64::new(0)),
        start: Instant::now(),
    };
    let (salida, llegada) = channel();
    let reciclados = Reciclados::default();
    let en_marcha = preparada.grabar(marcha, salida, reciclados.clone());

    // El primer segundo no se cuenta: abrir la captura y el primer fotograma entero.
    let calentar = Instant::now();
    let (mut llegados, mut guardados) = (0u64, 0u64);
    let tragar = |anillo: &mut Anillo, hasta: Instant, llegados: &mut u64, guardados: &mut u64| {
        while Instant::now() < hasta {
            match llegada.recv_timeout(Duration::from_millis(150)) {
                Ok(fotograma) => {
                    *llegados += 1;
                    if anillo.empujar(&fotograma.rgba, region.width, region.height, fotograma.ts_ms).unwrap_or(false) {
                        *guardados += 1;
                    }
                    reciclados.devolver(fotograma.rgba);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    };
    tragar(&mut anillo, calentar + Duration::from_secs(1), &mut llegados, &mut guardados);

    let (llegados_0, guardados_0, escritos_0) = (llegados, guardados, anillo.escritos());
    let procesador_0 = procesador_ms();
    let medir = Instant::now();
    tragar(&mut anillo, medir + Duration::from_secs(segundos), &mut llegados, &mut guardados);
    let pasado = medir.elapsed().as_secs_f64();
    let procesador = procesador_ms() - procesador_0;
    let escritos = anillo.escritos() - escritos_0;

    println!(
        "BANCO fps={fps} segundos={pasado:.1} llegados={} guardados={} procesador_ms={procesador} nucleo={:.1}% escritos_kb_s={:.0} en_disco_mb={:.1} memoria_mb={:.1} pico_mb={:.1} antes_de_abrir_mb={:.1}",
        llegados - llegados_0,
        guardados - guardados_0,
        procesador as f64 / (pasado * 10.0),
        escritos as f64 / 1024.0 / pasado,
        anillo.bytes() as f64 / 1_048_576.0,
        memoria_kb("VmRSS") as f64 / 1024.0,
        memoria_kb("VmHWM") as f64 / 1024.0,
        antes_de_abrir_kb as f64 / 1024.0,
    );

    stop.store(true, Ordering::Relaxed);
    drop(en_marcha.terminar());
    anillo.limpiar();
}
