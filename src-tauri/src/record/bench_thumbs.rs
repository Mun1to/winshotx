//! Medida de las miniaturas, que es lo que separa al usuario del editor al parar.
//! `cargo test --release --lib medir_miniaturas -- --ignored --nocapture`
#[cfg(test)]
mod tests {
    use crate::record::{generate_thumbnails, FrameCache, SessionData};
    use std::time::Instant;

    fn pantalla(width: u32, height: u32, paso: u32) -> Vec<u8> {
        let mut frame = vec![0u8; (width * height) as usize * 4];
        let mut semilla: u32 = 0x1234_5678;
        for pixel in frame.chunks_exact_mut(4) {
            semilla = semilla.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            pixel[0] = (semilla >> 16) as u8;
            pixel[1] = (semilla >> 8) as u8;
            pixel[2] = semilla as u8;
            pixel[3] = 255;
        }
        let x0 = (paso * 3) % (width - 16);
        for y in 4..20u32 {
            for x in x0..x0 + 16 {
                let p = ((y * width + x) * 4) as usize;
                frame[p..p + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        frame
    }

    #[test]
    #[ignore]
    fn medir_miniaturas() {
        let (ancho, alto, cuantos) = (1920u32, 1080u32, 150u32);
        let dir = std::env::temp_dir().join("winshotx-bench-thumbs");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cache = FrameCache::new(&dir).unwrap();
        let fotogramas: Vec<Vec<u8>> = (0..cuantos).map(|i| pantalla(ancho, alto, i)).collect();
        let t = Instant::now();
        for (i, f) in fotogramas.iter().enumerate() {
            cache.push_rgba(f, ancho, alto, i as u64 * 33).unwrap();
        }
        let ms_push = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "[grabar] {cuantos} fotogramas de {ancho}x{alto} a la cache: {ms_push:.0} ms ({:.1} ms/fotograma)",
            ms_push / cuantos as f64
        );
        drop(fotogramas);
        let frames = cache.finish(u64::from(cuantos) * 33, 30).unwrap();
        let mut session = SessionData {
            id: "bench".into(),
            dir: dir.clone(),
            region: crate::capture::Rect { x: 0, y: 0, width: ancho, height: alto },
            fps: 30,
            format: "mp4".into(),
            has_audio: false,
            width: ancho,
            height: alto,
            mp4_path: None,
            audio: None,
            clics: Vec::new(),
            teclas: Vec::new(),
            cursor: Vec::new(),
            cursor_capturado: false,
            formas: Vec::new(),
            punteros: Vec::new(),
            cambios_puntero: Vec::new(),
            marcas_zoom: Vec::new(),
            camara: None,
            escritura: Vec::new(),
            pistas: None,
            frames,
        };
        let t = Instant::now();
        generate_thumbnails(&mut session).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "[miniaturas] {cuantos} fotogramas de {ancho}x{alto}: {ms:.0} ms ({:.1} ms/fotograma)",
            ms / cuantos as f64
        );

        // Y lo que cuesta leerlos en orden, que es lo que hace exportar.
        let t = Instant::now();
        let mut lector = crate::record::LectorEnOrden::nuevo(&session).unwrap();
        for i in 0..cuantos as usize {
            let _ = lector.en(i).unwrap();
        }
        let ms_orden = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        for i in 0..cuantos as usize {
            let _ = crate::record::read_frame(&session, i).unwrap();
        }
        let ms_suelto = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "[leer] {cuantos} fotogramas: en orden {ms_orden:.0} ms ({:.1}/fotograma), reconstruyendo cada uno {ms_suelto:.0} ms ({:.1}/fotograma)",
            ms_orden / cuantos as f64,
            ms_suelto / cuantos as f64
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Una pagina quieta (cuadros grises, como un documento) con una franja que cambia en
    /// cada fotograma (alguien escribiendo, un contador) y un cuadrado que la cruza: lo que
    /// graba un tutorial, con TODOS los fotogramas distintos, que es el peor caso normal.
    pub(crate) fn tutorial(ancho: u32, alto: u32, paso: u32) -> Vec<u8> {
        let mut f: Vec<u8> = (0..ancho * alto)
            .flat_map(|i| {
                let (x, y) = (i % ancho, i / ancho);
                let v = if (x / 120 + y / 80) % 2 == 0 { 235 } else { 215 };
                [v, v, v, 255]
            })
            .collect();
        let mut semilla = paso.wrapping_mul(2_654_435_761);
        for y in 300..340u32 {
            for x in 200..(200 + 12 * (paso % 60) + 12).min(ancho) {
                semilla = semilla.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let p = ((y * ancho + x) * 4) as usize;
                let v = if semilla >> 31 == 0 { 30 } else { 235 };
                f[p..p + 3].copy_from_slice(&[v, v, v]);
            }
        }
        let x0 = (paso * 7) % (ancho - 64);
        for y in 600..664u32 {
            for x in x0..x0 + 64 {
                let p = ((y * ancho + x) * 4) as usize;
                f[p..p + 3].copy_from_slice(&[40, 90, 220]);
            }
        }
        f
    }

    /// Un minuto de tutorial a 1080p y 30 fps en una sesion de verdad, con su cache en disco.
    pub(crate) fn un_minuto(dir: &std::path::Path) -> SessionData {
        let (ancho, alto) = (1920u32, 1080u32);
        let segundos: u32 = std::env::var("SEGUNDOS").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
        let cuantos = segundos * 30;
        let _ = std::fs::remove_dir_all(dir);
        let mut cache = FrameCache::new(dir).unwrap();
        for i in 0..cuantos {
            cache.push_rgba(&tutorial(ancho, alto, i), ancho, alto, u64::from(i) * 1000 / 30).unwrap();
        }
        let frames = cache.finish(u64::from(segundos) * 1000, 30).unwrap();
        SessionData {
            id: "bench".into(),
            dir: dir.to_path_buf(),
            region: crate::capture::Rect { x: 0, y: 0, width: ancho, height: alto },
            fps: 30,
            format: "mp4".into(),
            has_audio: false,
            width: ancho,
            height: alto,
            mp4_path: None,
            audio: None,
            clics: Vec::new(),
            teclas: Vec::new(),
            cursor: Vec::new(),
            cursor_capturado: false,
            formas: Vec::new(),
            punteros: Vec::new(),
            cambios_puntero: Vec::new(),
            marcas_zoom: Vec::new(),
            camara: None,
            escritura: Vec::new(),
            pistas: None,
            frames,
        }
    }

    /// Lo que cuesta la tira del editor con un minuto: es lo que corre (con todos los
    /// nucleos) justo cuando el editor se esta abriendo.
    ///
    ///     cargo test --release --lib medir_las_miniaturas_de_un_minuto -- --ignored --nocapture
    #[test]
    #[ignore = "mide, no comprueba: pide release"]
    fn medir_las_miniaturas_de_un_minuto() {
        let dir = std::env::temp_dir().join(format!("winshotx-tira-{}", std::process::id()));
        let t = Instant::now();
        let base = un_minuto(&dir);
        println!(
            "[grabar] {} fotogramas en la cache en {:.1} s",
            base.frames.len(),
            t.elapsed().as_secs_f64()
        );
        let mut tiempos = Vec::new();
        for tanda in 1..=3 {
            let _ = std::fs::remove_dir_all(dir.join("thumbs"));
            let mut session = base.clone();
            let t = Instant::now();
            generate_thumbnails(&mut session).unwrap();
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            let hechas = std::fs::read_dir(dir.join("thumbs")).map(|d| d.count()).unwrap_or(0);
            let distintas: std::collections::HashSet<&str> =
                session.frames.iter().map(|f| f.thumb_path.as_str()).collect();
            let json = serde_json::to_string(&session).map(|j| j.len()).unwrap_or(0);
            println!(
                "[tira] tanda {tanda}: {ms:.0} ms, {hechas} PNG en disco, {} rutas distintas, session.json {} KB",
                distintas.len(),
                json / 1024
            );
            tiempos.push(ms);
        }
        tiempos.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("[tira] mediana {:.0} ms", tiempos[1]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
