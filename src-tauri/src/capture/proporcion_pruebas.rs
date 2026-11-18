//! S27 (proporción fija): lo que sale en el archivo cuando el recorte se hizo con la X.
//!
//! El overlay calcula el recorte en píxeles físicos para que sea un múltiplo exacto de la
//! proporción (`src/lib/proporcion.ts`). Aquí se mira la otra mitad: que Rust no le quite
//! ni un píxel por el camino de «Guardar» (`capture_still`: `crop_from_freeze` y luego
//! `png::save`) ni al grabar (`Rect::to_even`), y que el PNG que se lleva el usuario mida
//! exactamente eso. Se lee el archivo de vuelta del disco, no la imagen en memoria.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;

use super::{crop_from_freeze, Freeze, MonitorInfo, Rect};

/// Una pantalla congelada de ese tamaño en físicos, puesta donde diga el escritorio.
fn pantalla(x: i32, y: i32, ancho: u32, alto: u32, escala: f32) -> Freeze {
    let imagen = image::RgbaImage::from_fn(ancho, alto, |px, py| {
        image::Rgba([(px % 251) as u8, (py % 241) as u8, 90, 255])
    });
    Freeze {
        monitor: MonitorInfo {
            id: 0,
            label: "PRUEBA".into(),
            x,
            y,
            width: ancho,
            height: alto,
            scale: escala,
            is_primary: true,
        },
        png: Arc::new(Vec::new()),
        image: Arc::new(imagen),
    }
}

/// `a:b` en dos números.
fn partes(proporcion: &str) -> (u32, u32) {
    let (a, b) = proporcion.split_once(':').expect("una proporción como 16:9");
    (a.parse().unwrap(), b.parse().unwrap())
}

/// Recorta y guarda como `capture_still` con «Guardar», y devuelve lo que mide el PNG leído
/// del disco.
fn guardar_y_medir(freezes: &[Freeze], region: Rect, destino: &Path) -> (u32, u32) {
    let recorte = crop_from_freeze(freezes, region).expect("el recorte");
    crate::encode::png::save(&recorte, destino, recorte.width(), recorte.height()).expect("guardar el PNG");
    let leido = image::open(destino).expect("leer el PNG guardado");
    (leido.width(), leido.height())
}

fn carpeta(nombre: &str) -> PathBuf {
    let c = std::env::temp_dir().join("winshotx-proporcion").join(nombre);
    let _ = std::fs::remove_dir_all(&c);
    std::fs::create_dir_all(&c).unwrap();
    c
}

/// Los recortes que deja el overlay a 125 % y 150 %, también pegados al borde de abajo a la
/// derecha (donde `crop_from_freeze` recorta lo que se sale) y en una pantalla a la
/// izquierda de la principal, con coordenadas negativas.
#[test]
fn la_proporcion_sale_exacta_en_el_png() {
    let dir = carpeta("fijas");
    // (pantalla en físicos y su origen, región en el escritorio virtual, proporción)
    let casos = [
        // 1920 x 1080 al 125 %: 2400 x 1350 físicos. 16:9 de 100 escalones.
        ((0, 0, 2400, 1350, 1.25), Rect { x: 171, y: 264, width: 1600, height: 900 }, "16:9"),
        // Pegado abajo a la derecha: 2400 - 800 y 1350 - 450.
        ((0, 0, 2400, 1350, 1.25), Rect { x: 1600, y: 900, width: 800, height: 450 }, "16:9"),
        // 150 % a la izquierda de la principal.
        ((-2880, 0, 2880, 1620, 1.5), Rect { x: -2880 + 205, y: 316, width: 711, height: 1264 }, "9:16"),
        ((-2880, 0, 2880, 1620, 1.5), Rect { x: -1000, y: 1620 - 948, width: 948, height: 948 }, "1:1"),
        ((0, 0, 2400, 1350, 1.25), Rect { x: 3, y: 5, width: 1200, height: 900 }, "4:3"),
    ];
    for (i, ((mx, my, ancho, alto, escala), region, proporcion)) in casos.into_iter().enumerate() {
        let freezes = vec![pantalla(mx, my, ancho, alto, escala)];
        let (w, h) = guardar_y_medir(&freezes, region, &dir.join(format!("{i}.png")));
        let (a, b) = partes(proporcion);
        assert_eq!((w, h), (region.width, region.height), "caso {i}: el PNG no mide lo recortado");
        assert_eq!(w * b, h * a, "caso {i}: {w} x {h} no es {proporcion}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Grabando, la región ya llega con los dos lados pares: `to_even` no le quita nada.
#[test]
fn grabando_to_even_no_le_quita_nada() {
    for (region, proporcion) in [
        (Rect { x: 1, y: 1, width: 1600, height: 900 }, "16:9"),
        (Rect { x: 1, y: 1, width: 576, height: 1024 }, "9:16"),
        (Rect { x: 1, y: 1, width: 1200, height: 900 }, "4:3"),
        (Rect { x: 1, y: 1, width: 474, height: 474 }, "1:1"),
    ] {
        let par = region.to_even();
        let (a, b) = partes(proporcion);
        assert_eq!((par.width, par.height), (region.width, region.height));
        assert_eq!(par.width * b, par.height * a);
    }
}

#[derive(Deserialize)]
struct Apuntada {
    escala: f32,
    pantalla: Medida,
    proporcion: String,
    grabando: bool,
    region: Rect,
}

#[derive(Deserialize)]
struct Medida {
    ancho: u32,
    alto: u32,
}

/// De punta a punta con lo que mandó el overlay DE VERDAD: las regiones que apuntan las
/// pruebas de `SelectionCanvas.test.tsx` a 100, 125 y 150 %, recortadas y guardadas aquí.
///
/// ```text
/// ./node_modules/.bin/vitest run src/components/overlay/SelectionCanvas.test.tsx
/// cargo test --lib las_regiones_del_overlay_salen_exactas -- --ignored --nocapture
/// ```
///
/// Va con `--ignored` porque necesita el archivo de la otra mitad; sin él falla, no pasa
/// en verde sin mirar nada.
#[test]
#[ignore]
fn las_regiones_del_overlay_salen_exactas_en_el_png() {
    let ruta = std::env::temp_dir().join("winshotx-proporcion").join("regiones.json");
    let texto = std::fs::read_to_string(&ruta)
        .unwrap_or_else(|_| panic!("falta {}: corre antes las pruebas de vitest del overlay", ruta.display()));
    let apuntadas: Vec<Apuntada> = serde_json::from_str(&texto).expect("el JSON del overlay");
    assert!(apuntadas.len() >= 24, "el overlay apuntó {} regiones, se esperaban 24", apuntadas.len());

    let dir = carpeta("del-overlay");
    for (i, ap) in apuntadas.iter().enumerate() {
        let (a, b) = partes(&ap.proporcion);
        let region = if ap.grabando { ap.region.to_even() } else { ap.region };
        let freezes = vec![pantalla(0, 0, ap.pantalla.ancho, ap.pantalla.alto, ap.escala)];
        let (w, h) = guardar_y_medir(&freezes, region, &dir.join(format!("{i}.png")));
        println!(
            "{:>4} % {:>5} {:<5} -> PNG de {w} x {h}",
            (ap.escala * 100.0).round(),
            ap.proporcion,
            if ap.grabando { "vídeo" } else { "foto" },
        );
        assert_eq!(
            (w, h),
            (ap.region.width, ap.region.height),
            "{i}: el archivo no mide lo que se recortó (grabando: {})",
            ap.grabando
        );
        assert_eq!(w * b, h * a, "{i}: {w} x {h} no es {}", ap.proporcion);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
