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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Version {
        /// La 0.4.8: textura nueva y lectura entera en cada fotograma, colores dados la
        /// vuelta enteros y comparacion entera (`viejo_s29`).
        Vieja,
        /// S29: textura guardada, comparar en la GPU, solo las zonas que cambian,
        /// `ImagenDelAnillo`.
        Nueva,
    }

    /// Que se graba: la pantalla de mentira (sin tocar la de nadie) o una pantalla de verdad.
    #[derive(Clone, Copy, Debug)]
    enum Fuente {
        Mentira(crate::record::banco_ventana::Escena),
        /// La pantalla con ese id (`capture::monitors`).
        Pantalla(u32),
    }

    struct Medida {
        cpu: f64,
        recibidos: u64,
        con_zonas: u64,
        guardados: u64,
        bytes: u64,
    }

    /// El anillo a 60 fps y alto nativo, como lo tiene Munito, grabando la fuente durante
    /// `segundos`. Devuelve la CPU del proceso en nucleos, ya restado lo que gasta el pintor
    /// de la pantalla de mentira.
    #[allow(non_snake_case)]
    fn correr_el_anillo(fuente: Fuente, version: Version, segundos: u64) -> Medida {
        use crate::record::banco_ventana::Pantalla;
        use crate::record::buffer::ImagenDelAnillo;
        use crate::record::win::{self, CaptureFlags, CapturedFrame};

        let (pantalla, monitor) = match fuente {
            Fuente::Mentira(escena) => (Some(Pantalla::abrir(escena)), None),
            Fuente::Pantalla(id) => (
                None,
                Some(
                    crate::capture::monitors()
                        .unwrap()
                        .into_iter()
                        .find(|m| m.id == id)
                        .expect("no hay pantalla con ese id"),
                ),
            ),
        };
        let region = match &monitor {
            Some(m) => Rect { x: m.x, y: m.y, width: m.width, height: m.height }.to_even(),
            None => Rect {
                x: 0,
                y: 0,
                width: crate::record::banco_ventana::ANCHO,
                height: crate::record::banco_ventana::ALTO,
            },
        };
        let (ANCHO, ALTO) = (region.width, region.height);
        let fps = 60u32;
        let dir = std::env::temp_dir().join(format!("winshotx-s29-anillo-{version:?}"));
        let _ = std::fs::remove_dir_all(&dir);
        let mut anillo = Anillo::nuevo(&dir, 60_000, fps, crate::record::buffer::bytes_max(60, fps, ANCHO, ALTO)).unwrap();
        let reciclados = win::Reciclados::default();
        let (sender, receiver) = std::sync::mpsc::channel::<CapturedFrame>();
        let stop = Arc::new(AtomicBool::new(false));
        let flags = CaptureFlags {
            sender,
            crop: (0, 0, 0, 0),
            stop: stop.clone(),
            pause: Arc::new(AtomicBool::new(false)),
            paused_ms: Arc::new(AtomicU64::new(0)),
            min_interval_ms: 0,
            start: Instant::now(),
            reciclados: reciclados.clone(),
            solo_cambios: version == Version::Nueva,
        };
        enum Control {
            Vieja(crate::record::viejo_s29::Control),
            Nueva(win::Control),
        }
        let control = match (version, &pantalla, &monitor) {
            (Version::Vieja, Some(p), _) => {
                Control::Vieja(crate::record::viejo_s29::start_window(p.hwnd, ANCHO, ALTO, true, fps, flags))
            }
            (Version::Nueva, Some(p), _) => Control::Nueva(win::start_window(p.hwnd, true, fps, flags).expect("no arranca")),
            (Version::Vieja, None, Some(m)) => {
                Control::Vieja(crate::record::viejo_s29::start(region, (m.x, m.y), true, fps, flags))
            }
            (Version::Nueva, None, Some(m)) => {
                Control::Nueva(win::start(region, (m.x, m.y), true, fps, flags).expect("no arranca"))
            }
            _ => unreachable!(),
        };
        let mut imagen = ImagenDelAnillo::nueva(ANCHO, ALTO, None);
        let mut m = Medida { cpu: 0.0, recibidos: 0, con_zonas: 0, guardados: 0, bytes: 0 };

        // Lo mismo que hace `replay::Cocina::trabajar` con cada tanda, en las dos versiones.
        let mut tragar = |primero: CapturedFrame, anillo: &mut Anillo, m: &mut Medida| {
            let mut lote = vec![primero];
            while let Ok(siguiente) = receiver.try_recv() {
                lote.push(siguiente);
            }
            m.recibidos += lote.len() as u64;
            m.con_zonas += lote.iter().filter(|f| f.zonas.is_some()).count() as u64;
            let ts = lote.last().map(|f| f.ts_ms).unwrap_or(0);
            let _ = crate::record::raton::cursor();
            match version {
                Version::Vieja => {
                    // La 0.4.8: se queda con el ultimo y tira los demas.
                    let ultimo = lote.pop().unwrap();
                    for f in lote {
                        reciclados.devolver(f.bgra);
                    }
                    let mut rgba = ultimo.bgra;
                    crate::record::viejo_s29::bgra_a_rgba_en_sitio(&mut rgba);
                    if anillo.empujar(&rgba, ANCHO, ALTO, ts).unwrap_or(false) {
                        m.guardados += 1;
                    }
                    reciclados.devolver(rgba);
                }
                Version::Nueva => {
                    let desde = lote.iter().rposition(|f| f.zonas.is_none()).unwrap_or(0);
                    for (i, f) in lote.into_iter().enumerate() {
                        let bufer = if i < desde { f.bgra } else { imagen.recibir(f.bgra, f.zonas) };
                        reciclados.devolver(bufer);
                    }
                    if imagen.guardar(anillo, ts).unwrap_or(false) {
                        m.guardados += 1;
                    }
                }
            }
        };

        let calentando = Instant::now();
        while calentando.elapsed() < Duration::from_secs(1) {
            if let Ok(f) = receiver.recv_timeout(Duration::from_millis(50)) {
                let mut basura = Medida { cpu: 0.0, recibidos: 0, con_zonas: 0, guardados: 0, bytes: 0 };
                tragar(f, &mut anillo, &mut basura);
            }
        }
        let bytes0 = anillo.escritos();
        let cpu0 = cpu_del_proceso();
        let pintor = || pantalla.as_ref().map_or(Duration::ZERO, Pantalla::cpu_pintor);
        let pintor0 = pintor();
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(segundos) {
            if let Ok(f) = receiver.recv_timeout(Duration::from_millis(50)) {
                tragar(f, &mut anillo, &mut m);
            }
        }
        let real = t0.elapsed().as_secs_f64();
        let cpu = (cpu_del_proceso() - cpu0).as_secs_f64() - (pintor() - pintor0).as_secs_f64();
        m.cpu = cpu / real;
        m.bytes = anillo.escritos() - bytes0;
        stop.store(true, Ordering::Relaxed);
        match control {
            Control::Vieja(c) => {
                let _ = c.stop();
            }
            Control::Nueva(c) => {
                let _ = c.stop();
            }
        }
        while receiver.try_recv().is_ok() {}
        anillo.limpiar();
        m
    }

    /// **S29: lo que gasta el anillo antes y despues, sin grabar la pantalla de nadie.**
    ///
    /// Las dos versiones graban la misma pantalla de mentira (`banco_ventana`, fuera de las
    /// pantallas) a 60 fps y alto nativo, por turnos y tres tandas, por el ruido del PC.
    ///
    ///     cargo test --release --lib medir_antes_y_despues -- --ignored --nocapture --test-threads=1
    #[test]
    #[ignore = "abre una ventana FUERA de las pantallas y la graba; es una medida"]
    fn medir_antes_y_despues_sin_pantalla() {
        use crate::record::banco_ventana::Escena;
        let segundos = std::env::var("WINSHOTX_SEGUNDOS").ok().and_then(|s| s.parse().ok()).unwrap_or(5);
        println!(
            "{:<10} {:<6} {:>6} {:>9} {:>9} {:>9} {:>10}",
            "escena", "vers.", "tanda", "nucleos", "recib/s", "guard/s", "KB/s disco"
        );
        for escena in [Escena::Quieta, Escena::Parpadeo, Escena::Raton, Escena::Video, Escena::Todo] {
            for tanda in 1..=3 {
                for version in [Version::Vieja, Version::Nueva] {
                    let m = correr_el_anillo(Fuente::Mentira(escena), version, segundos);
                    let s = segundos as f64;
                    println!(
                        "{:<10} {:<6} {:>6} {:>9.3} {:>9.1} {:>9.1} {:>10.0}   (con zonas: {})",
                        format!("{escena:?}"),
                        format!("{version:?}"),
                        tanda,
                        m.cpu,
                        m.recibidos as f64 / s,
                        m.guardados as f64 / s,
                        m.bytes as f64 / 1024.0 / s,
                        m.con_zonas
                    );
                }
            }
        }
    }

    /// **S29: que leer solo las zonas no pierde ni un pixel.** Cada fotograma leido por
    /// zonas se lee tambien entero y se compara (ver `win::verificacion`), en todas las
    /// escenas de la pantalla de mentira.
    ///
    ///     cargo test --release --lib verificar_las_zonas_sin_pantalla -- --ignored --nocapture
    #[test]
    #[ignore = "abre una ventana FUERA de las pantallas y la graba"]
    fn verificar_las_zonas_sin_pantalla() {
        use crate::record::banco_ventana::Escena;
        use crate::record::win::verificacion;
        verificacion::ACTIVA.store(true, Ordering::Relaxed);
        let mut todo_bien = true;
        for escena in [Escena::Parpadeo, Escena::Raton, Escena::Video, Escena::Todo] {
            verificacion::reiniciar();
            let m = correr_el_anillo(Fuente::Mentira(escena), Version::Nueva, 4);
            let (mirados, malos, pixeles) = (
                verificacion::MIRADOS.load(Ordering::Relaxed),
                verificacion::MALOS.load(Ordering::Relaxed),
                verificacion::PIXELES_MALOS.load(Ordering::Relaxed),
            );
            println!(
                "{escena:?}: {mirados} fotogramas por zonas comparados con el entero, {malos} distintos ({pixeles} pixeles); {} recibidos",
                m.recibidos
            );
            todo_bien &= malos == 0 && mirados > 0;
        }
        verificacion::ACTIVA.store(false, Ordering::Relaxed);
        assert!(todo_bien, "leer por zonas ha perdido pixeles, o no se ha leido ninguno por zonas");
    }

    /// La pantalla de verdad que se mide: `WINSHOTX_PANTALLA=<id>` o la principal.
    fn pantalla_de_verdad() -> u32 {
        let monitores = crate::capture::monitors().unwrap();
        for m in &monitores {
            println!(
                "pantalla {}: «{}» {}x{} en ({}, {}){}",
                m.id,
                m.label,
                m.width,
                m.height,
                m.x,
                m.y,
                if m.is_primary { " principal" } else { "" }
            );
        }
        std::env::var("WINSHOTX_PANTALLA")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| monitores.iter().find(|m| m.is_primary).map_or(0, |m| m.id))
    }

    /// **S29, con su pantalla y su permiso:** lo mismo que `verificar_las_zonas_sin_pantalla`
    /// pero sobre una pantalla de verdad, con su raton y lo que tenga delante. Graba en
    /// memoria; no guarda nada fuera de la carpeta temporal y la borra.
    ///
    ///     WINSHOTX_PANTALLA=<id> WINSHOTX_SEGUNDOS=30 cargo test --release --lib verificar_las_zonas_en_pantalla -- --ignored --nocapture
    #[test]
    #[ignore = "graba una pantalla de verdad: solo con permiso de quien esta delante"]
    fn verificar_las_zonas_en_pantalla() {
        use crate::record::win::verificacion;
        let id = pantalla_de_verdad();
        let segundos = std::env::var("WINSHOTX_SEGUNDOS").ok().and_then(|s| s.parse().ok()).unwrap_or(30);
        verificacion::reiniciar();
        verificacion::ACTIVA.store(true, Ordering::Relaxed);
        let m = correr_el_anillo(Fuente::Pantalla(id), Version::Nueva, segundos);
        verificacion::ACTIVA.store(false, Ordering::Relaxed);
        let (mirados, malos, pixeles) = (
            verificacion::MIRADOS.load(Ordering::Relaxed),
            verificacion::MALOS.load(Ordering::Relaxed),
            verificacion::PIXELES_MALOS.load(Ordering::Relaxed),
        );
        println!(
            "pantalla {id}, {segundos} s: {} recibidos ({} por zonas); {mirados} comparados con el entero, {malos} distintos ({pixeles} pixeles)",
            m.recibidos, m.con_zonas
        );
        assert_eq!(malos, 0, "leer por zonas ha perdido pixeles en la pantalla de verdad");
    }

    /// **S29, con su pantalla y su permiso:** antes y despues con lo que tenga delante, por
    /// turnos de `WINSHOTX_SEGUNDOS` (10) y `WINSHOTX_TANDAS` (4) tandas, para que los dos
    /// vean lo mismo de media.
    ///
    ///     WINSHOTX_PANTALLA=<id> cargo test --release --lib medir_antes_y_despues_en_pantalla -- --ignored --nocapture
    #[test]
    #[ignore = "graba una pantalla de verdad: solo con permiso de quien esta delante"]
    fn medir_antes_y_despues_en_pantalla() {
        let id = pantalla_de_verdad();
        let segundos = std::env::var("WINSHOTX_SEGUNDOS").ok().and_then(|s| s.parse().ok()).unwrap_or(10);
        let tandas = std::env::var("WINSHOTX_TANDAS").ok().and_then(|s| s.parse().ok()).unwrap_or(4);
        let mut suma = [0f64; 2];
        for tanda in 1..=tandas {
            for (i, version) in [Version::Vieja, Version::Nueva].into_iter().enumerate() {
                let m = correr_el_anillo(Fuente::Pantalla(id), version, segundos);
                suma[i] += m.cpu;
                println!(
                    "tanda {tanda} {version:?}: {:.3} nucleos, {:.1} recibidos/s, {:.1} guardados/s, {:.0} KB/s, con zonas {}",
                    m.cpu,
                    m.recibidos as f64 / segundos as f64,
                    m.guardados as f64 / segundos as f64,
                    m.bytes as f64 / 1024.0 / segundos as f64,
                    m.con_zonas
                );
            }
        }
        println!("media: vieja {:.3} nucleos, nueva {:.3}", suma[0] / tandas as f64, suma[1] / tandas as f64);
    }

    /// S29: que entrega Windows grabando la pantalla de mentira de `banco_ventana`, fuera de
    /// las pantallas: cuantos fotogramas, y que zonas dice que cambiaron en cada uno.
    ///
    ///     cargo test --release --lib sondear_las_zonas -- --ignored --nocapture
    #[test]
    #[ignore = "abre una ventana FUERA de las pantallas y la graba unos segundos"]
    fn sondear_las_zonas_sucias() {
        use crate::record::banco_ventana::{Escena, Pantalla};
        use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
        use windows_capture::frame::Frame;
        use windows_capture::graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl};
        use windows_capture::settings::{
            ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
            MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
        };

        println!(
            "zonas sucias: {:?}; intervalo minimo: {:?}",
            GraphicsCaptureApi::is_dirty_region_supported(),
            GraphicsCaptureApi::is_minimum_update_interval_supported()
        );

        struct Sonda {
            vistos: Arc<parking_lot::Mutex<Vec<(u32, u32, Vec<(i32, i32, i32, i32)>)>>>,
        }
        impl GraphicsCaptureApiHandler for Sonda {
            type Flags = Arc<parking_lot::Mutex<Vec<(u32, u32, Vec<(i32, i32, i32, i32)>)>>>;
            type Error = Box<dyn std::error::Error + Send + Sync>;
            fn new(ctx: Context<Self::Flags>) -> std::result::Result<Self, Self::Error> {
                Ok(Self { vistos: ctx.flags })
            }
            fn on_frame_arrived(
                &mut self,
                frame: &mut Frame,
                _control: InternalCaptureControl,
            ) -> std::result::Result<(), Self::Error> {
                let zonas = frame
                    .dirty_regions()
                    .map(|v| v.into_iter().map(|r| (r.x, r.y, r.width, r.height)).collect())
                    .unwrap_or_default();
                if self.vistos.lock().is_empty() {
                    let b = frame.buffer()?;
                    let mut apoyo = Vec::new();
                    let px = b.as_nopadding_buffer(&mut apoyo);
                    // (100, 200) es fondo de degradado; (100, 70) es una ventana blanca.
                    let ver = |x: usize, y: usize| {
                        let p = (y * 1920 + x) * 4;
                        px[p..p + 4].to_vec()
                    };
                    println!("   pixel (10,1000) = {:?}; (100,70) = {:?}", ver(10, 1000), ver(100, 70));
                }
                self.vistos.lock().push((frame.width(), frame.height(), zonas));
                Ok(())
            }
        }

        for escena in [Escena::Quieta, Escena::Parpadeo, Escena::Raton, Escena::Todo] {
            let pantalla = Pantalla::abrir(escena);
            let vistos = Arc::new(parking_lot::Mutex::new(Vec::new()));
            let settings = Settings::new(
                windows_capture::window::Window::from_raw_hwnd(pantalla.hwnd as *mut core::ffi::c_void),
                CursorCaptureSettings::WithCursor,
                DrawBorderSettings::WithoutBorder,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::ReportOnly,
                ColorFormat::Bgra8,
                vistos.clone(),
            );
            let control = Sonda::start_free_threaded(settings).expect("no arranca la captura");
            std::thread::sleep(Duration::from_secs(2));
            let _ = control.stop();
            let v = vistos.lock();
            println!("== {escena:?}: {} fotogramas en 2 s, {} vueltas pintadas", v.len(), pantalla.vueltas.load(Ordering::Relaxed));
            for (w, h, zonas) in v.iter().take(6) {
                println!("   {w}x{h} zonas {zonas:?}");
            }
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

        for escenario in [
            "raton sobre escritorio quieto",
            "raton abajo y un icono girando arriba, lejos",
            "pantalla entera cambiando",
        ] {
            let partida = escenario.starts_with("pantalla");
            let lejos = escenario.contains("lejos");
            println!();
            println!("== {escenario}: {cuantos} fotogramas de {ancho}x{alto} ==");
            // Los fotogramas se preparan ANTES, en BGRA como los entrega Windows.
            let fotogramas: Vec<Vec<u8>> = (0..cuantos)
                .map(|i| {
                    let mut f = if partida { ruido(ancho, alto, 100 + i) } else { fondo.clone() };
                    if !partida {
                        cursor_en(&mut f, ancho, 100 + i * 8, 900);
                    }
                    if lejos {
                        // Un icono de 32x32 que cambia cada fotograma en la esquina de
                        // arriba a la derecha: la caja que lo abarca junto al raton es
                        // casi la pantalla entera.
                        cursor_en(&mut f, ancho, 1800 + (i % 2) * 8, 40);
                    }
                    f
                })
                .collect();

            let mut e_copia = Etapa::nueva("copiar 8 MB (to_vec, como win.rs)");
            let mut e_color = Etapa::nueva("BGRA -> RGBA en sitio, entero");
            let mut e_delta = Etapa::nueva("delta::zona_cambiada");
            let mut e_qoi = Etapa::nueva("QOI de UNA caja (como antes)");
            let mut e_anterior = Etapa::nueva("copiar el fotograma a `anterior`");
            let mut e_reducir = Etapa::nueva("escalar::reducir a 1280x720");
            let mut e_cache = Etapa::nueva("FrameCache::push_rgba (zonas, ahora)");

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
    /// medir. El marco, el de una pantalla al 200 % (lineas de cuatro pixeles): lo que cambia
    /// por fotograma es un cursor, no una pelicula.
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
                2.0,
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
        // Los bufers van y vuelven como en la aplicacion; el primer escenario los tira sin
        // devolverlos, que es lo que se hacia antes, para ver lo que cuesta reservarlos.
        let reciclados = win::Reciclados::default();
        // (fotogramas, pixeles de salida reducidos, ms en delta, ms en reducir, ms en empujar)
        let cuentas = Arc::new(parking_lot::Mutex::new((0u64, 0u64, 0f64, 0f64, 0f64)));
        type Tragar = Box<dyn Fn(CapturedFrame, &mut Option<Anillo>)>;
        let escenarios: [(&str, Tragar); 6] = [
            ("captura sola, sin devolver el bufer", Box::new(|_f, _a| {})),
            ("captura sola, devolviendo el bufer", {
                let r = reciclados.clone();
                Box::new(move |f, _a| r.devolver(f.bgra))
            }),
            ("camino entero, alto nativo", {
                let r = reciclados.clone();
                Box::new(move |f, anillo| {
                    let mut rgba = f.bgra;
                    crate::recorder::bgra_a_rgba_en_sitio(&mut rgba);
                    let (w, h) = (region.width, region.height);
                    let _ = anillo.as_mut().unwrap().empujar(&rgba, w, h, f.ts_ms);
                    r.devolver(rgba);
                })
            }),
            ("camino entero, alto nativo, a 30 fps", {
                let r = reciclados.clone();
                Box::new(move |f, anillo| {
                    let mut rgba = f.bgra;
                    crate::recorder::bgra_a_rgba_en_sitio(&mut rgba);
                    let (w, h) = (region.width, region.height);
                    let _ = anillo.as_mut().unwrap().empujar(&rgba, w, h, f.ts_ms);
                    r.devolver(rgba);
                })
            }),
            ("reducido a 720, entero cada vez (como antes)", {
                let r = reciclados.clone();
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
                    r.devolver(imagen.into_raw());
                    let _ = anillo.as_mut().unwrap().empujar(&chica, w, h, f.ts_ms);
                })
            }),
            ("reducido a 720, solo lo que cambia (ahora)", {
                // Lo mismo que hace `replay::Cocina::tragar` con un destino puesto.
                let r = reciclados.clone();
                let Some((w, h)) = crate::encode::escalar::medida_para(region.width, region.height, 720)
                else {
                    panic!("la pantalla ya mide 720 o menos");
                };
                let estado = parking_lot::Mutex::new((
                    crate::encode::escalar::Reduccion::nueva(region.width, region.height, w, h),
                    None::<Vec<u8>>,
                    vec![0u8; (w as usize) * (h as usize) * 4],
                ));
                let cuentas = cuentas.clone();
                Box::new(move |f, anillo| {
                    let mut rgba = f.bgra;
                    crate::recorder::bgra_a_rgba_en_sitio(&mut rgba);
                    let mut guardado = estado.lock();
                    let (reduccion, anterior, escalado) = &mut *guardado;
                    let t_delta = Instant::now();
                    let cambios = match anterior.as_ref() {
                        Some(a) => delta::zonas_cambiadas(a, &rgba, region.width, region.height),
                        None => vec![delta::Parche { x: 0, y: 0, width: region.width, height: region.height }],
                    };
                    let ms_delta = t_delta.elapsed().as_secs_f64() * 1000.0;
                    let mut pixeles = 0u64;
                    let t_red = Instant::now();
                    for cambio in cambios {
                        let zona = reduccion.zona_afectada(cambio);
                        pixeles += zona.pixeles();
                        reduccion.reducir_zona(&rgba, zona, escalado);
                    }
                    let ms_red = t_red.elapsed().as_secs_f64() * 1000.0;
                    let t_emp = Instant::now();
                    let _ = anillo.as_mut().unwrap().empujar(escalado, w, h, f.ts_ms);
                    let ms_emp = t_emp.elapsed().as_secs_f64() * 1000.0;
                    if let Some(viejo) = anterior.replace(rgba) {
                        r.devolver(viejo);
                    }
                    let mut c = cuentas.lock();
                    c.0 += 1;
                    c.1 += pixeles;
                    c.2 += ms_delta;
                    c.3 += ms_red;
                    c.4 += ms_emp;
                })
            }),
        ];

        println!();
        println!("{:<42} {:>8} {:>10} {:>14}", "escenario", "fotogr.", "fps", "CPU (nucleos)");
        for (nombre, tragar) in escenarios.iter() {
            let fps = if nombre.contains("30 fps") { 30 } else { fps };
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
                    reciclados: reciclados.clone(),
                    solo_cambios: false,
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

        {
            let c = cuentas.lock();
            if c.0 > 0 {
                let n = c.0 as f64;
                println!(
                    "  (reducido por zonas: {:.0} px de salida por fotograma; delta {:.2} ms, reducir {:.2} ms, empujar {:.2} ms)",
                    c.1 as f64 / n, c.2 / n, c.3 / n, c.4 / n
                );
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
                            anillo.empujar(&audio::a_pcm16(&trozo.mezcla()));
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
