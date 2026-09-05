//! Medida de congelar las pantallas de verdad y de recortar de lo congelado.
//! `cargo test --release --lib medir_freeze -- --ignored --nocapture`
#[cfg(test)]
mod tests {
    use std::time::Instant;
    use image::{ExtendedColorType, ImageEncoder};

    fn ms(t: Instant) -> f64 {
        t.elapsed().as_secs_f64() * 1000.0
    }

    #[test]
    #[ignore]
    fn medir_freeze() {
        let dir = std::env::temp_dir().join("winshotx-bench-freeze");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let _ = crate::capture::freeze_all().unwrap();
        for vuelta in 0..4 {
            let t = Instant::now();
            let freezes = crate::capture::freeze_all().unwrap();
            let ms_freeze = ms(t);
            let t = Instant::now();
            let region = crate::capture::Rect { x: 100, y: 100, width: 800, height: 600 };
            let _img = crate::capture::crop_from_freeze(&freezes, region).unwrap();
            let ms_crop = ms(t);
            let t = Instant::now();
            let _ = crate::capture::stitch_all(&freezes).unwrap();
            let ms_stitch = ms(t);
            println!(
                "[freeze] vuelta {vuelta}: freeze_all {} pantallas {ms_freeze:.0} ms ({} KB de PNG); recorte {ms_crop:.0} ms; juntar {ms_stitch:.0} ms",
                freezes.len(),
                freezes.iter().map(|f| f.png.len()).sum::<usize>() / 1024
            );
        }
        // Las partes por separado.
        for vuelta in 0..3 {
            let monitors = xcap::Monitor::all().unwrap();
            let t = Instant::now();
            let imgs: Vec<_> = monitors.iter().map(|m| m.capture_image().unwrap()).collect();
            let ms_cap_fila = ms(t);

            let cuantos = monitors.len();
            let t = Instant::now();
            let _paralelo: Vec<image::RgbaImage> = std::thread::scope(|s| {
                let hs: Vec<_> = (0..cuantos)
                    .map(|i| s.spawn(move || xcap::Monitor::all().unwrap()[i].capture_image().unwrap()))
                    .collect();
                hs.into_iter().map(|h| h.join().unwrap()).collect()
            });
            let ms_cap_par = ms(t);

            let t = Instant::now();
            let mut bmps = Vec::new();
            for img in &imgs {
                let mut v = Vec::with_capacity(img.as_raw().len() + 64);
                image::codecs::bmp::BmpEncoder::new(&mut v)
                    .write_image(img.as_raw(), img.width(), img.height(), ExtendedColorType::Rgba8)
                    .unwrap();
                bmps.push(v);
            }
            let ms_bmp = ms(t);

            let t = Instant::now();
            let a_mano: Vec<Vec<u8>> = imgs.iter().map(crate::capture::bmp_bytes).collect();
            let ms_mano = ms(t);
            println!("[freeze]   BMP a mano, {} pantallas: {ms_mano:.0} ms", a_mano.len());

            let t = Instant::now();
            for (i, b) in bmps.iter().enumerate() {
                std::fs::write(dir.join(format!("x-{i}.bmp")), b).unwrap();
            }
            let ms_write = ms(t);

            let t = Instant::now();
            let _ = image::open(dir.join("x-0.bmp")).unwrap().to_rgba8();
            let ms_decode = ms(t);

            println!(
                "[freeze] vuelta {vuelta}: capturar en fila {ms_cap_fila:.0} ms, en paralelo {ms_cap_par:.0} ms; codificar {} BMP {ms_bmp:.0} ms; escribir {ms_write:.0} ms; decodificar uno {ms_decode:.0} ms",
                bmps.len()
            );
        }
    }
}
