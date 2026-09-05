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
}
