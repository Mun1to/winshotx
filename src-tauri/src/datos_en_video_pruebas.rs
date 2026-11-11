//! Las pruebas de buscar datos en un vídeo.
//!
//! Casi todas van con un lector de mentira que «lee» rectángulos de colores: así se prueba
//! lo que de verdad es de aquí (qué fotogramas se leen, qué trozo, y que cada dato quede
//! tapado en CADA instante en que se ve) sin depender de cómo lea el de Windows. La que pasa
//! por el lector de verdad, con texto pintado y hasta el MP4, está en `exporter_tapar.rs`.

use super::*;
use crate::record::FrameCache;
use image::Rgba;

pub(crate) const ANCHO: u32 = 320;
pub(crate) const ALTO: u32 = 240;
const BLANCO: Rgba<u8> = Rgba([255, 255, 255, 255]);

/// Los «datos» de mentira: cada color es un dato distinto.
const DATOS: [([u8; 3], &str); 3] = [([250, 0, 0], "uno@example.com"), ([0, 0, 250], "dos@example.com"), ([0, 160, 0], "600123456")];

/// Un rectángulo de dato en un fotograma: qué dato, y dónde en píxeles.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Puesto {
    pub dato: usize,
    pub x: u32,
    pub y: u32,
    pub ancho: u32,
    pub alto: u32,
}

pub(crate) fn puesto(dato: usize, x: u32, y: u32) -> Puesto {
    Puesto { dato, x, y, ancho: 60, alto: 12 }
}

/// Una pantalla blanca con los datos puestos y una marca de `ruido` en la esquina de abajo
/// a la izquierda que cambia con cada número: es lo que hace que dos fotogramas con los
/// mismos datos sean distintos (un reloj, un cursor que parpadea).
pub(crate) fn pantalla(datos: &[Puesto], ruido: u32) -> RgbaImage {
    let mut imagen = RgbaImage::from_pixel(ANCHO, ALTO, BLANCO);
    for p in datos {
        let [r, g, b] = DATOS[p.dato].0;
        for y in p.y..(p.y + p.alto).min(ALTO) {
            for x in p.x..(p.x + p.ancho).min(ANCHO) {
                imagen.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
    }
    for x in 0..(ruido % 40) {
        imagen.put_pixel(x, ALTO - 1, Rgba([90, 90, 90, 255]));
    }
    imagen
}

pub(crate) fn sesion(nombre: &str, fotogramas: &[(u64, RgbaImage)], total_ms: u64) -> SessionData {
    let dir = std::env::temp_dir().join(format!("winshotx-tapar-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cache = FrameCache::new(&dir).expect("cache");
    let (ancho, alto) = fotogramas[0].1.dimensions();
    for (ms, imagen) in fotogramas {
        assert!(cache.push_rgba(imagen.as_raw(), ancho, alto, *ms).unwrap(), "el fotograma de los {ms} ms repite el anterior");
    }
    let mut session: SessionData = serde_json::from_value(serde_json::json!({
        "id": nombre, "dir": dir, "region": {"x": 0, "y": 0, "width": ancho, "height": alto},
        "fps": 30, "format": "mp4", "hasAudio": false, "width": ancho, "height": alto,
        "mp4Path": null, "frames": []
    }))
    .unwrap();
    session.frames = cache.finish(total_ms, 30).expect("cerrar la cache");
    session
}

/// El lector de mentira: dentro de la franja, cada color de `DATOS` es un dato con la caja
/// de sus píxeles. Apunta qué franjas le han pedido leer.
pub(crate) struct LectorDeColores {
    pub franjas: Vec<Franja>,
    /// Los números de lectura en que se hace el ciego, para probar que un fallo suelto del
    /// lector no deja el dato al aire.
    pub ciego_en: Vec<usize>,
}

impl LectorDeColores {
    pub fn nuevo() -> Self {
        Self { franjas: Vec::new(), ciego_en: Vec::new() }
    }

    pub fn leer(&mut self, imagen: &RgbaImage, f: Franja) -> Result<Vec<Dato>> {
        let lectura = self.franjas.len();
        self.franjas.push(f);
        if self.ciego_en.contains(&lectura) {
            return Ok(Vec::new());
        }
        let mut datos = Vec::new();
        for (color, texto) in DATOS {
            let mut caja: Option<Caja> = None;
            for y in f.y1..f.y2.min(imagen.height()) {
                for x in 0..imagen.width() {
                    let p = imagen.get_pixel(x, y).0;
                    if [p[0], p[1], p[2]] == color {
                        let c = Caja { x1: x as f32, y1: y as f32, x2: x as f32 + 1.0, y2: y as f32 + 1.0 };
                        caja = Some(caja.map_or(c, |k| k.union(&c)));
                    }
                }
            }
            if let Some(caja) = caja {
                datos.push(Dato { clase: Clase::Correo, texto: texto.into(), caja });
            }
        }
        Ok(datos)
    }
}

fn buscar_con(session: &SessionData, lector: &mut LectorDeColores) -> DatosEnVideo {
    buscar(session, &mut |imagen, franja| lector.leer(imagen, franja), &mut |_, _| {}).expect("buscar")
}

/// Lo que importa de verdad: en CADA instante, cada dato que se ve está dentro de una tapa
/// puesta en ese instante. Se mira cada 5 ms de toda la grabación.
fn todo_tapado(tapas: &[Tapa], verdad: &[(u64, Vec<Puesto>)], total_ms: u64) {
    for (i, (desde, puestos)) in verdad.iter().enumerate() {
        let hasta = verdad.get(i + 1).map_or(total_ms, |s| s.0);
        let mut ms = *desde;
        while ms < hasta {
            for p in puestos {
                let tapado = tapas.iter().any(|t| {
                    crate::encode::tapar::puesta(t, ms)
                        && t.x * ANCHO as f32 <= p.x as f32 + 0.01
                        && t.y * ALTO as f32 <= p.y as f32 + 0.01
                        && (t.x + t.ancho) * ANCHO as f32 >= (p.x + p.ancho) as f32 - 0.01
                        && (t.y + t.alto) * ALTO as f32 >= (p.y + p.alto) as f32 - 0.01
                });
                assert!(tapado, "a los {ms} ms el dato {} en ({}, {}) se ve sin tapar: {tapas:?}", p.dato, p.x, p.y);
            }
            ms += 5;
        }
    }
}

/// Monta la sesión de una «verdad» (qué datos hay desde cada instante) y la busca.
fn probar(nombre: &str, verdad: &[(u64, Vec<Puesto>)], total_ms: u64, lector: &mut LectorDeColores) -> DatosEnVideo {
    let fotogramas: Vec<(u64, RgbaImage)> =
        verdad.iter().enumerate().map(|(i, (ms, puestos))| (*ms, pantalla(puestos, i as u32))).collect();
    let session = sesion(nombre, &fotogramas, total_ms);
    let salida = buscar_con(&session, lector);
    let _ = std::fs::remove_dir_all(&session.dir);
    todo_tapado(&salida.tapas, verdad, total_ms);
    salida
}

#[test]
fn un_dato_quieto_se_tapa_justo_mientras_se_ve() {
    let verdad = vec![(0, vec![]), (1_000, vec![puesto(0, 100, 50)]), (3_000, vec![])];
    let salida = probar("quieto", &verdad, 4_000, &mut LectorDeColores::nuevo());
    assert_eq!(salida.tapas.len(), 1, "{:?}", salida.tapas);
    let t = salida.tapas[0];
    assert_eq!((t.desde_ms, t.hasta_ms), (1_000, 3_000));
    assert!((t.x - 100.0 / ANCHO as f32).abs() < 1e-4 && (t.ancho - 60.0 / ANCHO as f32).abs() < 1e-4);
    assert_eq!(salida.resumen, vec![("correo".to_string(), 1)]);
}

/// Sin ningún dato no sale nada, y una pantalla quieta se lee una vez.
#[test]
fn sin_datos_no_sale_nada() {
    let verdad = vec![(0, vec![])];
    let mut lector = LectorDeColores::nuevo();
    let salida = probar("vacio", &verdad, 5_000, &mut lector);
    assert!(salida.tapas.is_empty());
    assert_eq!(salida.lecturas, 1);
}

/// Mientras la pantalla cambia deprisa no se leen todos los fotogramas; y un dato que sale
/// en uno de los que no se leen se tapa desde ESE, no desde el siguiente que se lee.
#[test]
fn un_dato_que_sale_entre_dos_lecturas_se_tapa_desde_que_sale() {
    // Un fotograma cada 40 ms (como un vídeo), y el dato sale a los 120 ms.
    let mut verdad: Vec<(u64, Vec<Puesto>)> = (0..30).map(|i| (i * 40, vec![])).collect();
    for (ms, puestos) in verdad.iter_mut() {
        if *ms >= 120 {
            puestos.push(puesto(0, 100, 50));
        }
    }
    let mut lector = LectorDeColores::nuevo();
    let salida = probar("deprisa", &verdad, 1_200, &mut lector);
    assert!(salida.lecturas < 15, "se han leído {} de 30 fotogramas", salida.lecturas);
    // Se tapa desde el primero que cambió y no se miró: puede que antes de los 120 ms (de
    // más no tapa nada que importe), pero nunca después.
    let desde = salida.tapas[0].desde_ms;
    assert!((40..=120).contains(&desde), "empieza a los {desde} ms: {:?}", salida.tapas);
}

/// Solo se lee lo que cambia: con un dato arriba y algo que cambia sin parar abajo, el
/// lector lee la franja de abajo y no vuelve a leer la de arriba.
#[test]
fn solo_se_lee_la_franja_que_cambia() {
    let verdad: Vec<(u64, Vec<Puesto>)> = (0..10).map(|i| (i * 500, vec![puesto(0, 100, 20)])).collect();
    let mut lector = LectorDeColores::nuevo();
    let salida = probar("franjas", &verdad, 5_000, &mut lector);
    assert_eq!(salida.tapas.len(), 1, "{:?}", salida.tapas);
    assert_eq!((salida.tapas[0].desde_ms, salida.tapas[0].hasta_ms), (0, 5_000));
    // La primera, entera; las demás, solo la franja del ruido, al pie.
    assert_eq!(lector.franjas[0], Franja { y1: 0, y2: ALTO });
    for f in &lector.franjas[1..] {
        assert!(f.y1 >= ALTO - 1 - MARGEN_FRANJA && f.y2 == ALTO, "ha leído {f:?}");
    }
}

/// Lo que cambia encima de un dato se relee con el dato entero dentro, aunque solo haya
/// cambiado una fila: un dato cortado por la mitad no se reconoce, y se perdería.
#[test]
fn lo_que_cambia_encima_de_un_dato_se_relee_con_el_dato_entero() {
    let dato = puesto(0, 100, 100);
    let mut cambiado = pantalla(&[dato], 0);
    // Una sola fila cambia, a la mitad del dato (un subrayado que aparece).
    for x in 100..160 {
        cambiado.put_pixel(x, 106, Rgba([255, 255, 255, 255]));
    }
    let session = sesion("encima", &[(0, pantalla(&[dato], 0)), (1_000, cambiado)], 2_000);
    let mut lector = LectorDeColores::nuevo();
    let salida = buscar_con(&session, &mut lector);
    let _ = std::fs::remove_dir_all(&session.dir);
    let segunda = lector.franjas[1];
    assert!(segunda.y1 <= 100 && segunda.y2 >= 112, "la relectura no coge el dato entero: {segunda:?}");
    assert_eq!(salida.tapas.len(), 1);
    assert_eq!(salida.tapas[0].hasta_ms, 2_000, "el dato sigue ahí");
}

/// Un scroll: el dato sube 6 píxeles cada 33 ms. En ningún instante queda al aire, y la
/// línea de tiempo no se llena de un bloque por fotograma.
#[test]
fn un_dato_que_se_mueve_va_tapado_en_todo_su_camino() {
    let mut verdad: Vec<(u64, Vec<Puesto>)> = vec![(0, vec![puesto(0, 100, 200)])];
    for i in 1..=25u32 {
        verdad.push((1_000 + u64::from(i) * 33, vec![puesto(0, 100, 200 - i * 6)]));
    }
    verdad.push((3_000, vec![puesto(0, 100, 50), puesto(1, 20, 150)]));
    let salida = probar("scroll", &verdad, 5_000, &mut LectorDeColores::nuevo());
    assert!(salida.tapas.len() <= 8, "demasiados bloques para un scroll: {}", salida.tapas.len());
    assert_eq!(salida.resumen, vec![("correo".to_string(), 2)], "el mismo correo movido es un correo");
}

/// Si el lector se salta el dato en una lectura (le pasa al de Windows), el dato no queda
/// al aire en ese hueco: sigue siendo un solo bloque.
#[test]
fn un_fallo_suelto_del_lector_no_deja_el_dato_al_aire() {
    let verdad: Vec<(u64, Vec<Puesto>)> = (0..8).map(|i| (i * 400, vec![puesto(0, 100, 100)])).collect();
    // Que el ruido toque el dato, para que cada fotograma lo relea.
    let fotogramas: Vec<(u64, RgbaImage)> = verdad
        .iter()
        .enumerate()
        .map(|(i, (ms, p))| {
            let mut imagen = pantalla(p, 0);
            imagen.put_pixel(100 + i as u32 % 3, 113, Rgba([10 * i as u8, 0, 0, 255]));
            (*ms, imagen)
        })
        .collect();
    let session = sesion("ciego", &fotogramas, 3_200);
    let mut lector = LectorDeColores::nuevo();
    lector.ciego_en = vec![3];
    let salida = buscar_con(&session, &mut lector);
    let _ = std::fs::remove_dir_all(&session.dir);
    assert_eq!(salida.tapas.len(), 1, "{:?}", salida.tapas);
    assert_eq!((salida.tapas[0].desde_ms, salida.tapas[0].hasta_ms), (0, 3_200));
}

/// Dos datos a la vez son dos bloques, y se cuentan los dos.
#[test]
fn dos_datos_a_la_vez_son_dos_bloques() {
    let verdad = vec![(0, vec![]), (500, vec![puesto(0, 10, 10), puesto(2, 200, 180)]), (2_000, vec![puesto(2, 200, 180)])];
    let salida = probar("dos", &verdad, 3_000, &mut LectorDeColores::nuevo());
    assert_eq!(salida.tapas.len(), 2, "{:?}", salida.tapas);
    assert_eq!(salida.resumen, vec![("correo".to_string(), 2)]);
}

#[test]
fn las_franjas_que_cambian() {
    let a = pantalla(&[], 0);
    assert!(franjas_cambiadas(&a, &a).is_empty(), "iguales, nada que leer");
    let b = pantalla(&[puesto(0, 10, 100)], 0);
    assert_eq!(franjas_cambiadas(&a, &b), vec![Franja { y1: 100 - MARGEN_FRANJA, y2: 112 + MARGEN_FRANJA }]);
    // Arriba y abajo, lejos: dos franjas.
    let c = pantalla(&[puesto(0, 10, 10), puesto(1, 10, 200)], 0);
    assert_eq!(franjas_cambiadas(&a, &c).len(), 2);
    // Casi todo: la imagen entera.
    let d = RgbaImage::from_pixel(ANCHO, ALTO, Rgba([0, 0, 0, 255]));
    assert_eq!(franjas_cambiadas(&a, &d), vec![Franja { y1: 0, y2: ALTO }]);
    // Otro tamaño: entera.
    let e = RgbaImage::from_pixel(10, 10, BLANCO);
    assert_eq!(franjas_cambiadas(&a, &e), vec![Franja { y1: 0, y2: 10 }]);
}

/// La caja de un dato leído lleva su margen, y con la franja y la escala del lector vuelve
/// a píxeles del fotograma entero.
#[test]
fn la_caja_de_un_dato_vuelve_al_fotograma_entero() {
    let palabra = |texto: &str, x: f32| crate::sensibles::Palabra { texto: texto.into(), x, y: 10.0, ancho: 100.0, alto: 20.0 };
    let linea = vec![palabra("Contacto:", 0.0), palabra("Munir@Example.com", 120.0)];
    let datos = datos_de_linea(&linea, 300.0, 2.0);
    assert_eq!(datos.len(), 1);
    let d = &datos[0];
    assert_eq!(d.texto, "munir@example.com");
    // 20 de alto: 3,6 de margen; a escala 2 y bajada 300.
    assert!((d.caja.x1 - (120.0 - 3.6) * 2.0).abs() < 0.01, "{:?}", d.caja);
    assert!((d.caja.y1 - ((10.0 - 3.6) * 2.0 + 300.0)).abs() < 0.01, "{:?}", d.caja);
}

/// Texto negro sobre blanco con la fuente del sistema, cada línea en su sitio: lo más
/// parecido a una pantalla de verdad para el lector de Windows, que con rectángulos de
/// colores no lee nada. Lo usan la prueba de la factura y la medida de un minuto.
#[cfg(windows)]
pub(crate) fn pintar_texto(lineas: &[(i32, i32, String)], ancho: u32, alto: u32, letra: i32) -> RgbaImage {
    use windows::Win32::Foundation::{COLORREF, RECT};
    use windows::Win32::Graphics::Gdi::*;
    use windows::core::HSTRING;

    unsafe {
        let dc = CreateCompatibleDC(None);
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = ancho as i32;
        // Negativo: filas de arriba abajo, como las quiere `image`.
        info.bmiHeader.biHeight = -(alto as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;
        let mut pixeles: *mut std::ffi::c_void = std::ptr::null_mut();
        let mapa = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut pixeles, None, 0).expect("el lienzo");
        let anterior = SelectObject(dc, mapa.into());
        let blanco = CreateSolidBrush(COLORREF(0x00FF_FFFF));
        FillRect(dc, &RECT { left: 0, top: 0, right: ancho as i32, bottom: alto as i32 }, blanco);
        let fuente = CreateFontW(
            letra, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
            DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            &HSTRING::from("Segoe UI"),
        );
        let fuente_anterior = SelectObject(dc, fuente.into());
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x0000_0000));
        for (x, y, texto) in lineas {
            let utf16: Vec<u16> = texto.encode_utf16().collect();
            let _ = TextOutW(dc, *x, *y, &utf16);
        }
        let total = (ancho * alto) as usize;
        let crudo = std::slice::from_raw_parts(pixeles as *const u8, total * 4);
        let mut lienzo = RgbaImage::new(ancho, alto);
        for (i, p) in lienzo.pixels_mut().enumerate() {
            // El DIB viene en BGRA.
            *p = Rgba([crudo[i * 4 + 2], crudo[i * 4 + 1], crudo[i * 4], 255]);
        }
        SelectObject(dc, fuente_anterior);
        SelectObject(dc, anterior);
        let _ = DeleteObject(fuente.into());
        let _ = DeleteObject(mapa.into());
        let _ = DeleteObject(blanco.into());
        let _ = DeleteDC(dc);
        lienzo
    }
}
