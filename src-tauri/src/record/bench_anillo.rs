//! Lo que cuesta el anillo de los ultimos segundos, medido en vez de supuesto.
//!
//! El 31 de agosto de 2026 se midio en la maquina de Munir que el anillo a 60 fps y alto
//! nativo se come el **86 % de un nucleo** todo el rato. Este archivo parte ese numero en
//! sus trozos, que es lo unico que permite atacar el trozo gordo en vez de suponerlo. Dos
//! bancos, los dos con `--ignored`:
//!
//! - `medir_el_anillo_en_seco`: fotogramas de mentira, cada etapa cronometrada aparte.
//!   Determinista, corre en cualquier maquina.
//! - `medir_el_anillo_con_pantalla`: la captura de Windows de verdad sobre la pantalla
//!   del raton, con un marco parpadeando en una esquina para que lleguen fotogramas aunque
//!   nadie mueva nada. Mide el tiempo de CPU del proceso entero, que es lo que ve el
//!   Administrador de tareas.
//!
//! Los dos se corren en release, que es donde valen los numeros:
//!
//!     cargo test --release --lib medir_el_anillo -- --ignored --nocapture --test-threads=1
#[cfg(all(test, windows))]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use crate::capture::Rect;
    use crate::record::buffer::Anillo;
    use crate::record::{delta, FrameCache};

    /// Cuanta CPU lleva gastada este proceso, en todos sus hilos y nucleos juntos.
    fn cpu_del_proceso() -> Duration {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

        let mut creacion = FILETIME::default();
        let mut salida = FILETIME::default();
        let mut nucleo = FILETIME::default();
        let mut usuario = FILETIME::default();
        unsafe {
            GetProcessTimes(GetCurrentProcess(), &mut creacion, &mut salida, &mut nucleo, &mut usuario)
                .expect("no se ha podido leer el tiempo de CPU");
        }
        let cien_ns = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
        Duration::from_nanos((cien_ns(nucleo) + cien_ns(usuario)) * 100)
    }

    /// Una pantalla llena de ruido, que es lo peor para QOI y para el recorte de `delta`.
    fn ruido(ancho: u32, alto: u32, semilla: u32) -> Vec<u8> {
        let mut frame = vec![0u8; (ancho * alto) as usize * 4];
        let mut s = semilla.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9);
        for pixel in frame.chunks_exact_mut(4) {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            pixel[0] = (s >> 16) as u8;
            pixel[1] = (s >> 8) as u8;
            pixel[2] = s as u8;
            pixel[3] = 255;
        }
        frame
    }

    /// Un cuadrado blanco de 32x32 en esa posicion, como un cursor que se mueve.
    fn cursor_en(frame: &mut [u8], ancho: u32, x: u32, y: u32) {
        for dy in 0..32u32 {
            for dx in 0..32u32 {
                let p = (((y + dy) * ancho + x + dx) * 4) as usize;
                frame[p..p + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
    }

    struct Etapa {
        nombre: &'static str,
        total: Duration,
        veces: u32,
    }

    impl Etapa {
        fn nueva(nombre: &'static str) -> Self {
            Self { nombre, total: Duration::ZERO, veces: 0 }
        }
        fn medir<T>(&mut self, f: impl FnOnce() -> T) -> T {
            let t = Instant::now();
            let r = f();
            self.total += t.elapsed();
            self.veces += 1;
            r
        }
        fn linea(&self) -> String {
            let ms = self.total.as_secs_f64() * 1000.0 / f64::from(self.veces.max(1));
            format!(
                "  {:<36} {:>7.2} ms/fotograma  ({:>5.1} % de un nucleo a 60 fps)",
                self.nombre,
                ms,
                ms * 60.0 / 10.0
            )
        }
    }

    /// Cada etapa del camino de un fotograma, cronometrada aparte, en dos escenarios: el
    /// raton moviendose sobre un escritorio quieto (lo normal toda la tarde) y una pantalla
    /// que cambia entera en cada fotograma (una partida, el peor caso).
    #[test]
    #[ignore = "es un banco de medida, no una prueba"]
    fn medir_el_anillo_en_seco() {
        let (ancho, alto) = (1920u32, 1080u32);
        let cuantos = 120u32;
        let fondo = ruido(ancho, alto, 7);

        for escenario in ["raton sobre escritorio quieto", "pantalla entera cambiando"] {
            let partida = escenario.starts_with("pantalla");
            println!();
            println!("== {escenario}: {cuantos} fotogramas de {ancho}x{alto} ==");
            // Los fotogramas se preparan ANTES, en BGRA como los entrega Windows.
            let fotogramas: Vec<Vec<u8>> = (0..cuantos)
                .map(|i| {
                    let mut f = if partida { ruido(ancho, alto, 100 + i) } else { fondo.clone() };
                    if !partida {
                        cursor_en(&mut f, ancho, 100 + i * 8, 300);
                    }
                    f
                })
                .collect();

            let mut e_copia = Etapa::nueva("copiar 8 MB (to_vec, como win.rs)");
            let mut e_color = Etapa::nueva("BGRA -> RGBA en sitio, entero");
            let mut e_delta = Etapa::nueva("delta::zona_cambiada");
            let mut e_qoi = Etapa::nueva("QOI de lo que se guarda");
            let mut e_anterior = Etapa::nueva("copiar el fotograma a `anterior`");
            let mut e_reducir = Etapa::nueva("escalar::reducir a 1280x720");
            let mut e_cache = Etapa::nueva("FrameCache::push_rgba entero");

            let dir = std::env::temp_dir().join("winshotx-bench-anillo");
            let _ = std::fs::remove_dir_all(&dir);
            let mut cache = FrameCache::new(&dir).unwrap();
            let mut anterior: Option<Vec<u8>> = None;
            let mut desde_entero = 0u32;

            for (i, original) in fotogramas.iter().enumerate() {
                let bgra: Vec<u8> = e_copia.medir(|| original.to_vec());
                let mut rgba = bgra;
                e_color.medir(|| crate::recorder::bgra_a_rgba_en_sitio(&mut rgba));

                let parche = e_delta.medir(|| {
                    anterior.as_ref().map(|p| delta::zona_cambiada(p, &rgba, ancho, alto))
                });
                // Las mismas reglas que `push_rgba`: entero cada 30, o si cambio mas del 60 %.
                let entero = u64::from(ancho) * u64::from(alto);
                let recorte = parche
                    .flatten()
                    .filter(|_| desde_entero + 1 < 30)
                    .filter(|p| (p.pixeles() as f64) < entero as f64 * 0.6);
                let guardado = e_qoi.medir(|| match recorte {
                    Some(p) => {
                        qoi::encode_to_vec(delta::recortar(&rgba, ancho, p), p.width, p.height).unwrap()
                    }
                    None => qoi::encode_to_vec(&rgba, ancho, alto).unwrap(),
                });
                std::hint::black_box(&guardado);
                desde_entero = if recorte.is_some() { desde_entero + 1 } else { 0 };
                e_anterior.medir(|| match anterior.as_mut() {
                    Some(p) => p.copy_from_slice(&rgba),
                    None => anterior = Some(rgba.clone()),
                });

                let imagen = image::RgbaImage::from_raw(ancho, alto, rgba.clone()).unwrap();
                let chica = e_reducir.medir(|| crate::encode::escalar::reducir(&imagen, 1280, 720));
                std::hint::black_box(&chica);

                e_cache.medir(|| cache.push_rgba(&rgba, ancho, alto, i as u64 * 16).unwrap());
            }
            for e in [&e_copia, &e_color, &e_delta, &e_qoi, &e_anterior, &e_reducir, &e_cache] {
                println!("{}", e.linea());
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// Cuanta CPU gasta cada escenario con la captura de Windows DE VERDAD, sobre la
    /// pantalla del raton, a 60 fps y con el cursor, que es como lo tiene Munir.
    ///
    /// Pinta un marco pequenno parpadeando en la esquina de abajo a la derecha de esa
    /// pantalla: sin algo que cambie, Windows no entrega fotogramas y no habria nada que
    /// medir. El marco son cuatro lineas de cuatro pixeles: lo que cambia por fotograma es
    /// un cursor, no una pelicula.
    #[test]
    #[ignore = "graba la pantalla unos segundos y pinta un marco en una esquina"]
    fn medir_el_anillo_con_pantalla() {
        use crate::platform::marco::Marco;
        use crate::record::win::{self, CaptureFlags, CapturedFrame};

        let monitor = crate::replay::pantalla_para_medir().expect("no hay pantalla");
        let region = Rect {
            x: monitor.x,
            y: monitor.y,
            width: monitor.width,
            height: monitor.height,
        }
        .to_even();
        println!(
            "pantalla «{}», {}x{} en ({}, {})",
            monitor.label, region.width, region.height, region.x, region.y
        );

        // El marco que parpadea, en la esquina. Vive en un Arc para que el hilo que lo hace
        // parpadear y este puedan tenerlo a la vez.
        let marco = Arc::new(
            Marco::abrir(
                Rect {
                    x: region.x + region.width as i32 - 160,
                    y: region.y + region.height as i32 - 120,
                    width: 100,
                    height: 60,
                },
                4,
            )
            .expect("el marco tenia que abrirse"),
        );
        let parar_parpadeo = Arc::new(AtomicBool::new(false));
        let parpadeo = {
            let marco = marco.clone();
            let parar = parar_parpadeo.clone();
            std::thread::spawn(move || {
                let mut pausado = false;
                while !parar.load(Ordering::Relaxed) {
                    pausado = !pausado;
                    marco.pausado(pausado);
                    std::thread::sleep(Duration::from_millis(16));
                }
            })
        };

        const SEGUNDOS: u64 = 6;
        let fps = 60u32;
        let dir = std::env::temp_dir().join("winshotx-bench-anillo-pantalla");

        // Cada escenario arranca su propia captura, la deja correr SEGUNDOS y mide cuanta
        // CPU ha gastado el proceso entero mientras tanto (captura, copias y anillo).
        type Tragar = Box<dyn Fn(CapturedFrame, &mut Option<Anillo>)>;
        let escenarios: [(&str, Tragar); 3] = [
            ("captura sola, tirando los fotogramas", Box::new(|_f, _a| {})),
            (
                "camino entero, alto nativo",
                Box::new(move |f, anillo| {
                    let mut rgba = f.bgra;
                    crate::recorder::bgra_a_rgba_en_sitio(&mut rgba);
                    let (w, h) = (region.width, region.height);
                    let _ = anillo.as_mut().unwrap().empujar(&rgba, w, h, f.ts_ms);
                }),
            ),
            (
                "camino entero, reducido a 720 de alto",
                Box::new(move |f, anillo| {
                    let mut rgba = f.bgra;
                    crate::recorder::bgra_a_rgba_en_sitio(&mut rgba);
                    let Some((w, h)) =
                        crate::encode::escalar::medida_para(region.width, region.height, 720)
                    else {
                        return;
                    };
                    let imagen = image::RgbaImage::from_raw(region.width, region.height, rgba).unwrap();
                    let chica = crate::encode::escalar::reducir(&imagen, w, h).into_raw();
                    let _ = anillo.as_mut().unwrap().empujar(&chica, w, h, f.ts_ms);
                }),
            ),
        ];

        println!();
        println!("{:<42} {:>8} {:>10} {:>14}", "escenario", "fotogr.", "fps", "CPU (nucleos)");
        for (nombre, tragar) in escenarios.iter() {
            let _ = std::fs::remove_dir_all(&dir);
            let mut anillo = Some(
                Anillo::nuevo(
                    &dir,
                    30_000,
                    fps,
                    crate::record::buffer::bytes_max(30, fps, region.width, region.height),
                )
                .unwrap(),
            );
            let (sender, receiver) = std::sync::mpsc::channel::<CapturedFrame>();
            let stop = Arc::new(AtomicBool::new(false));
            let control = win::start(
                region,
                (monitor.x, monitor.y),
                true,
                fps,
                CaptureFlags {
                    sender,
                    crop: (0, 0, 0, 0),
                    stop: stop.clone(),
                    pause: Arc::new(AtomicBool::new(false)),
                    paused_ms: Arc::new(AtomicU64::new(0)),
                    min_interval_ms: 0,
                    start: Instant::now(),
                },
            )
            .expect("no ha arrancado la captura");
            // Un segundo de calentamiento, que la captura tarda en coger el ritmo.
            let calentando = Instant::now();
            while calentando.elapsed() < Duration::from_secs(1) {
                if let Ok(f) = receiver.recv_timeout(Duration::from_millis(50)) {
                    tragar(f, &mut anillo);
                }
            }
            let cpu0 = cpu_del_proceso();
            let t0 = Instant::now();
            let mut fotogramas = 0u64;
            while t0.elapsed() < Duration::from_secs(SEGUNDOS) {
                if let Ok(f) = receiver.recv_timeout(Duration::from_millis(50)) {
                    fotogramas += 1;
                    tragar(f, &mut anillo);
                }
            }
            let cpu = cpu_del_proceso() - cpu0;
            let real = t0.elapsed();
            stop.store(true, Ordering::Relaxed);
            let _ = control.stop();
            // Lo que quede en el canal se tira: no cuenta.
            while receiver.try_recv().is_ok() {}
            println!(
                "{:<42} {:>8} {:>10.1} {:>14.2}",
                nombre,
                fotogramas,
                fotogramas as f64 / real.as_secs_f64(),
                cpu.as_secs_f64() / real.as_secs_f64()
            );
            if let Some(a) = anillo.take() {
                a.limpiar();
            }
        }

        // Y el sonido, que en su configuracion corre tambien toda la tarde: altavoz y
        // microfono a la vez, mezclados. Ademas comprueba que el loopback entrega de verdad
        // con el altavoz callado (trozos > 0), que es lo que arregla el chorro de silencio.
        {
            use crate::record::audio::{self, Fuentes};
            use crate::record::buffer::AnilloAudio;
            match audio::empezar(
                Fuentes { sistema: true, microfono: true },
                Arc::new(AtomicBool::new(false)),
            ) {
                Ok(captura) => {
                    let info = crate::record::AudioInfo {
                        channels: captura.formato.canales,
                        sample_rate: captura.formato.muestras_por_segundo,
                    };
                    let mut anillo = AnilloAudio::nuevo(info.bytes_por_ms(), info.channels, 30_000);
                    std::thread::sleep(Duration::from_secs(1));
                    while captura.trozos.try_recv().is_ok() {}
                    let cpu0 = cpu_del_proceso();
                    let t0 = Instant::now();
                    let mut trozos = 0u64;
                    while t0.elapsed() < Duration::from_secs(SEGUNDOS) {
                        if let Ok(trozo) = captura.trozos.recv_timeout(Duration::from_millis(50)) {
                            trozos += 1;
                            anillo.empujar(&audio::a_pcm16(&trozo.datos));
                        }
                    }
                    let cpu = cpu_del_proceso() - cpu0;
                    let real = t0.elapsed();
                    captura.parar();
                    println!(
                        "{:<42} {:>8} {:>10} {:>14.2}",
                        "sonido: altavoz + microfono, mezclados",
                        trozos,
                        "-",
                        cpu.as_secs_f64() / real.as_secs_f64()
                    );
                }
                Err(e) => println!("sonido: no se ha podido abrir ({e})"),
            }
        }

        parar_parpadeo.store(true, Ordering::Relaxed);
        let _ = parpadeo.join();
        drop(marco);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
