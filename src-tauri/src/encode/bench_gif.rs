//! Medida del codificador GIF, para no optimizar a ciegas: tiempo, tamanno y cuanto se
//! aleja el color del original.
//! `cargo test --release --lib medir_gif -- --ignored --nocapture`
//!
//! Medido el 5 de septiembre de 2026 en release, 90 fotogramas de 1280x720, contra el
//! codificador de antes (que se trajo de git a un modulo de pruebas solo para esto):
//!
//! | calidad | antes | ahora |
//! |---|---|---|
//! | 50 | 1594 ms, 176 KB, error 2,01 | 590 ms, 219 KB, error 1,26 |
//! | 80 | 3344 ms, 222 KB, error 1,35 | 798 ms, 222 KB, error 1,27 |
//! | 100 | 27708 ms, 222 KB, error 1,54 | 758 ms, 221 KB, error 1,30 |
//!
//! El error es la media de la diferencia absoluta por canal entre lo que se ve en el GIF y
//! el fotograma original: mas bajo es mas fiel.
#[cfg(test)]
mod tests {
    use crate::encode::gif::{encode, GifOptions};
    use image::RgbaImage;
    use std::time::Instant;

    /// Una pantalla de mentira parecida a una de verdad: fondo con degradado, una ventana
    /// con texto y un cursor que se mueve. Casi todo quieto, como al grabar.
    pub(crate) fn fotograma(ancho: u32, alto: u32, paso: u32) -> RgbaImage {
        let mut img = RgbaImage::from_fn(ancho, alto, |x, y| {
            let franja = (y / 28) % 2 == 0;
            let base = if franja { 246 } else { 255 };
            let ventana = x > ancho / 6 && x < ancho * 5 / 6 && y > alto / 8 && y < alto * 7 / 8;
            let texto = ventana && (x / 3 + y / 11) % 17 == 0;
            let ruido = (x.wrapping_mul(2654435761).wrapping_add(y.wrapping_mul(40503)) % 7) as u8;
            if texto {
                image::Rgba([32, 34, 38, 255])
            } else if ventana {
                image::Rgba([base - ruido, base - ruido, base, 255])
            } else {
                // Un degradado de verdad, que es lo que hace trabajar al dithering.
                image::Rgba([(x * 255 / ancho) as u8, (y * 255 / alto) as u8, 120, 255])
            }
        });
        let cx = (paso * 7) % (ancho - 24);
        let cy = (paso * 5) % (alto - 24);
        for y in cy..cy + 20 {
            for x in cx..cx + 20 {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let filas = 20 + (paso / 10) * 14;
        for y in filas..(filas + 12).min(alto) {
            for x in (ancho / 6 + 10)..(ancho / 6 + 300) {
                if (x + y) % 3 == 0 {
                    img.put_pixel(x, y, image::Rgba([40, 40, 40, 255]));
                }
            }
        }
        img
    }

    /// Decodifica el GIF entero y devuelve la media de la diferencia absoluta por canal
    /// entre cada fotograma reconstruido y el original, y cuantos fotogramas trae.
    fn error_medio(bytes: &[u8], originales: &[RgbaImage]) -> (f64, usize) {
        let mut opciones = gif::DecodeOptions::new();
        opciones.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = opciones.read_info(std::io::Cursor::new(bytes)).unwrap();
        let (w, h) = (decoder.width() as usize, decoder.height() as usize);
        let mut lienzo = vec![0u8; w * h * 4];
        let mut suma = 0u64;
        let mut cuenta = 0u64;
        let mut leidos = 0usize;
        // Cada fotograma del GIF puede cubrir varios originales (los que no cambiaron), asi
        // que se compara con el original que le toca por posicion de escritura.
        let mut original = 0usize;
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            leidos += 1;
            for y in 0..frame.height as usize {
                for x in 0..frame.width as usize {
                    let s = (y * frame.width as usize + x) * 4;
                    if frame.buffer[s + 3] == 0 {
                        continue;
                    }
                    let d = ((y + frame.top as usize) * w + x + frame.left as usize) * 4;
                    lienzo[d..d + 4].copy_from_slice(&frame.buffer[s..s + 4]);
                }
            }
            if let Some(orig) = originales.get(original) {
                for (a, b) in lienzo.chunks_exact(4).zip(orig.as_raw().chunks_exact(4)) {
                    suma += (a[0] as i64 - b[0] as i64).unsigned_abs()
                        + (a[1] as i64 - b[1] as i64).unsigned_abs()
                        + (a[2] as i64 - b[2] as i64).unsigned_abs();
                    cuenta += 3;
                }
            }
            original += 1;
        }
        (suma as f64 / cuenta.max(1) as f64, leidos)
    }

    #[test]
    #[ignore]
    fn medir_gif() {
        let (ancho, alto, cuantos) = (1280u32, 720u32, 90usize);
        let frames: Vec<RgbaImage> = (0..cuantos as u32).map(|i| fotograma(ancho, alto, i)).collect();
        let indices: Vec<usize> = (0..cuantos).collect();
        let delays = vec![33u32; cuantos];
        let dir = std::env::temp_dir().join("winshotx-bench");
        std::fs::create_dir_all(&dir).unwrap();

        for calidad in [50u8, 80, 100] {
            let nuevo = dir.join("nuevo.gif");
            let mut loader = |i: usize| Ok(frames[i].clone());
            let t = Instant::now();
            encode(
                &indices,
                &delays,
                &mut loader,
                &nuevo,
                &GifOptions { width: ancho, height: alto, quality: calidad, loop_forever: true },
                |_, _, _| {},
            )
            .unwrap();
            let ms_nuevo = t.elapsed().as_millis();
            let bytes_nuevo = std::fs::read(&nuevo).unwrap();
            let (err_nuevo, n_nuevo) = error_medio(&bytes_nuevo, &frames);

            println!(
                "[gif] {ancho}x{alto} x{cuantos} calidad {calidad}: {ms_nuevo} ms ({:.1} ms/fotograma), {} KB, error {err_nuevo:.2}, {n_nuevo} fotogramas",
                ms_nuevo as f64 / cuantos as f64,
                bytes_nuevo.len() / 1024,
            );
        }
    }
}
