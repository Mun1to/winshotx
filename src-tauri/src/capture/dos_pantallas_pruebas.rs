//! Lo que sale en el archivo cuando la seleccion cruza de una pantalla a otra (16-nov).
//!
//! El overlay manda la region en pixeles fisicos del escritorio virtual, aunque abarque dos
//! pantallas (`src/lib/seleccionCompartida.ts`). Aqui se mira la otra mitad: que por el
//! camino de «Guardar» (`capture_still`: `recortar_escritorio` y luego `png::save`) el PNG
//! que se lleva el usuario mida lo que abarca la region y que cada trozo salga de su
//! pantalla. Se lee el archivo de vuelta del disco, no la imagen en memoria.

use std::sync::Arc;

use serde::Deserialize;

use super::{interseccion, recortar_escritorio, Freeze, MonitorInfo, Rect};

/// Lo que el overlay apunta en `%TEMP%\winshotx-dos-pantallas\regiones.json` por cada region
/// que manda a `capture_still` cruzando pantallas.
#[derive(Deserialize)]
struct Apuntada {
    nombre: String,
    monitores: Vec<MonitorInfo>,
    region: Rect,
}

/// El color de cada pantalla, por su orden: es la firma de donde sale cada pixel.
fn color(i: usize) -> [u8; 4] {
    [[230, 40, 40, 255], [40, 200, 60, 255], [40, 90, 230, 255], [220, 200, 40, 255]][i % 4]
}

/// Los recortes que deja el overlay al cruzar de una pantalla a otra, pasados a PNG por el
/// camino de «Guardar»: el archivo mide lo que abarca la region, cada pantalla que toca pone
/// su trozo en su sitio, y lo que no es de ninguna va transparente.
///
/// ```text
/// ./node_modules/.bin/vitest run src/components/overlay/SelectionCanvas.test.tsx
/// cargo test --lib las_regiones_que_cruzan_salen_enteras -- --ignored --nocapture
/// ```
///
/// Va con `--ignored` porque necesita el archivo de la otra mitad; sin el falla, no pasa en
/// verde sin mirar nada.
#[test]
#[ignore]
fn las_regiones_que_cruzan_salen_enteras_en_el_png() {
    let ruta = std::env::temp_dir().join("winshotx-dos-pantallas").join("regiones.json");
    let texto = std::fs::read_to_string(&ruta)
        .unwrap_or_else(|_| panic!("falta {}: corre antes las pruebas de vitest del overlay", ruta.display()));
    let apuntadas: Vec<Apuntada> = serde_json::from_str(&texto).expect("el JSON del overlay");
    assert!(!apuntadas.is_empty(), "el overlay no apunto ninguna region");

    let dir = std::env::temp_dir().join("winshotx-dos-pantallas").join("png");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cruzan = 0;
    for (i, ap) in apuntadas.iter().enumerate() {
        let freezes: Vec<Freeze> = ap
            .monitores
            .iter()
            .enumerate()
            .map(|(n, m)| Freeze {
                monitor: m.clone(),
                png: Arc::new(Vec::new()),
                image: Arc::new(image::RgbaImage::from_pixel(m.width, m.height, image::Rgba(color(n)))),
            })
            .collect();
        let sitios: Vec<Rect> =
            ap.monitores.iter().map(|m| Rect { x: m.x, y: m.y, width: m.width, height: m.height }).collect();
        let trozos: Vec<(usize, Rect)> =
            sitios.iter().enumerate().filter_map(|(n, &s)| Some((n, interseccion(ap.region, s)?))).collect();
        // La del iman (pasarse unos pixeles del borde) se queda a proposito en una pantalla.
        cruzan += usize::from(trozos.len() >= 2);

        let (recorte, abarca) = recortar_escritorio(&freezes, ap.region).expect("el recorte");
        let destino = dir.join(format!("{i}.png"));
        crate::encode::png::save(&recorte, &destino, recorte.width(), recorte.height()).expect("guardar el PNG");
        let leido = image::open(&destino).expect("leer el PNG guardado").to_rgba8();
        println!("{:<40} {:?} -> PNG de {} x {}", ap.nombre, ap.region, leido.width(), leido.height());

        assert_eq!(abarca, ap.region, "{}: la region se sale de las pantallas", ap.nombre);
        assert_eq!((leido.width(), leido.height()), (ap.region.width, ap.region.height), "{}", ap.nombre);
        for (n, t) in trozos {
            let (cx, cy) = ((t.x + t.width as i32 / 2 - abarca.x) as u32, (t.y + t.height as i32 / 2 - abarca.y) as u32);
            assert_eq!(leido.get_pixel(cx, cy).0, color(n), "{}: el trozo de la pantalla {n} no es suyo", ap.nombre);
        }
    }
    assert!(cruzan >= 3, "solo {cruzan} regiones cruzan de pantalla");
    let _ = std::fs::remove_dir_all(&dir);
}
