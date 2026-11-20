//! Las pruebas del marco de grabar en Windows (`marco.rs`): sus ventanas de verdad. Las de la
//! figura y su dibujo, que no dependen del sistema, estan en `marco_comun_pruebas.rs`.

use super::*;

/// La forma con todos los lados por fuera, que es la de una region en medio de su pantalla.
fn por_fuera(region: Rect, escala: f64, estilo: Estilo) -> Forma {
    Forma::nueva(region, Medidas::a_escala(escala, estilo), [false; 4])
}

/// Abre el marco de verdad, fuera de todas las pantallas para que nadie lo vea, y lo
/// cierra: comprueba que el hilo arranca, que las ventanas se crean y que cerrar no se
/// queda colgado esperando a nadie. Es lo unico de este archivo que no se puede saber
/// leyendo el codigo.
#[test]
fn se_abre_y_se_cierra_sin_quedarse_colgado() {
    let region = Rect { x: -20_000, y: -20_000, width: 200, height: 100 };
    let empezo = std::time::Instant::now();
    let marco = Marco::abrir(region, 1.0).expect("el marco tenía que abrirse");
    assert_eq!(marco.ventanas.len(), 4, "tienen que ser cuatro tiras");
    // Fuera de toda foto y de todo video (16-nov): ni sale en una captura hecha grabando
    // ni en el video de una captura con scroll.
    for &ventana in &marco.ventanas {
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};
        let mut afinidad = 0u32;
        unsafe { GetWindowDisplayAffinity(HWND(ventana as *mut core::ffi::c_void), &mut afinidad) }
            .expect("la afinidad de la tira");
        assert_eq!(afinidad, WDA_EXCLUDEFROMCAPTURE.0, "una tira del marco sale en las fotos");
    }
    marco.pausado(true);
    marco.pausado(false);
    drop(marco);
    assert!(
        empezo.elapsed() < std::time::Duration::from_secs(5),
        "abrir y cerrar el marco ha tardado {:?}",
        empezo.elapsed()
    );
}

/// Lo que ensenna Windows de cada ventana es su dibujo, en su sitio, y cambia al pausar. Una
/// ventana `WS_EX_LAYERED` a la que no le llega su dibujo no se ve nunca, y eso no lo dice
/// ningun error: por eso se mira la foto. Se compara el color premultiplicado, que es lo que
/// entrega la foto (ver `fotos`).
#[test]
fn se_ve_su_dibujo_y_cambia_en_pausa() {
    let region = Rect { x: -20_000, y: -20_000, width: 200, height: 100 };
    for estilo in [Estilo::Rojo, Estilo::AzulConLuz] {
        let forma = por_fuera(region, 1.5, estilo);
        let marco = Marco::abrir_con(region, 1.5, None, estilo).expect("el marco tenía que abrirse");
        let comprobar = |pausado: bool| {
            for (i, (sitio, bgra)) in fotos(&marco).iter().enumerate() {
                assert_eq!(*sitio, forma.tiras[i], "la tira {i} no esta en su sitio");
                let dibujo = pintar_tira(&forma, forma.tiras[i], estilo, pausado);
                assert_eq!(bgra.len(), dibujo.len());
                let distintos = bgra
                    .chunks_exact(4)
                    .zip(dibujo.chunks_exact(4))
                    .filter(|(a, b)| a[..3] != b[..3])
                    .count();
                assert_eq!(distintos, 0, "la tira {i} no ensenna su dibujo, {estilo:?} pausado {pausado}");
            }
        };
        comprobar(false);
        marco.pausado(true);
        // El repintado es un mensaje al otro hilo: se le da un momento para atenderlo.
        std::thread::sleep(std::time::Duration::from_millis(120));
        comprobar(true);
        marco.pausado(false);
        std::thread::sleep(std::time::Duration::from_millis(120));
        comprobar(false);
    }
}

/// Una ventana de verdad, fuera de todas las pantallas, para que el marco la siga.
struct VentanaDePrueba(HWND);

impl VentanaDePrueba {
    fn nueva(sitio: Rect) -> Self {
        use windows::Win32::UI::WindowsAndMessaging::WS_VISIBLE;
        const CLASE_PRUEBA: &[u16] = &[
            b'w' as u16, b'x' as u16, b'-' as u16, b'p' as u16, b'r' as u16, b'u' as u16, b'e' as u16,
            b'b' as u16, b'a' as u16, 0,
        ];
        unsafe extern "system" fn proc_prueba(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
            unsafe { DefWindowProcW(h, m, w, l) }
        }
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let instancia = GetModuleHandleW(None).expect("el modulo");
            let _ = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(proc_prueba),
                hInstance: instancia.into(),
                lpszClassName: PCWSTR(CLASE_PRUEBA.as_ptr()),
                ..Default::default()
            });
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                PCWSTR(CLASE_PRUEBA.as_ptr()),
                PCWSTR::null(),
                WS_POPUP | WS_VISIBLE,
                sitio.x,
                sitio.y,
                sitio.width as i32,
                sitio.height as i32,
                None,
                None,
                Some(instancia.into()),
                None,
            )
            .expect("la ventana de prueba");
            Self(hwnd)
        }
    }

    fn mover(&self, sitio: Rect) {
        unsafe {
            SetWindowPos(
                self.0,
                None,
                sitio.x,
                sitio.y,
                sitio.width as i32,
                sitio.height as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .expect("mover la ventana de prueba");
    }
}

impl Drop for VentanaDePrueba {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

/// Donde esta y si se ve cada tira del marco.
fn tiras_en_pantalla(marco: &Marco) -> Vec<(Rect, bool)> {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    marco
        .ventanas
        .iter()
        .map(|&v| {
            let hwnd = HWND(v as *mut core::ffi::c_void);
            let mut r = RECT::default();
            unsafe { GetWindowRect(hwnd, &mut r) }.expect("el sitio de la tira");
            let sitio = Rect { x: r.left, y: r.top, width: (r.right - r.left) as u32, height: (r.bottom - r.top) as u32 };
            (sitio, unsafe { IsWindowVisible(hwnd) }.as_bool())
        })
        .collect()
}

/// Espera (poco) a que el hilo del marco se entere de algo.
fn hasta(que: impl Fn() -> bool) -> bool {
    let empezo = std::time::Instant::now();
    while empezo.elapsed() < std::time::Duration::from_secs(2) {
        if que() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    false
}

/// Grabando una ventana, el marco va con ella: si se mueve o crece, el marco la sigue (antes
/// se quedaba donde estaba al empezar), si se esconde se esconde, y si vuelve, vuelve.
#[test]
fn el_marco_sigue_a_la_ventana_que_se_graba() {
    let al_empezar = Rect { x: -20_000, y: -20_000, width: 300, height: 200 };
    let ventana = VentanaDePrueba::nueva(al_empezar);
    let marco = Marco::abrir_en_ventana(al_empezar, 1.0, ventana.0 .0 as isize).expect("el marco");
    let esperado = |r: Rect| por_fuera(r, 1.0, ESTILO).tiras.to_vec();
    let sitios = || tiras_en_pantalla(&marco).into_iter().map(|(r, _)| r).collect::<Vec<_>>();
    let vistas = || tiras_en_pantalla(&marco).iter().all(|(_, v)| *v);
    assert_eq!(sitios(), esperado(al_empezar));
    assert!(vistas());

    let movida = Rect { x: -19_500, y: -19_800, width: 300, height: 200 };
    ventana.mover(movida);
    assert!(hasta(|| sitios() == esperado(movida)), "no ha seguido a la ventana: {:?}", sitios());

    let mas_grande = Rect { x: -19_500, y: -19_800, width: 480, height: 260 };
    ventana.mover(mas_grande);
    assert!(hasta(|| sitios() == esperado(mas_grande)), "no ha crecido con la ventana: {:?}", sitios());

    unsafe { ShowWindow(ventana.0, SW_HIDE) };
    assert!(hasta(|| tiras_en_pantalla(&marco).iter().all(|(_, v)| !*v)), "con la ventana escondida, el marco se ve");
    unsafe { ShowWindow(ventana.0, SW_SHOWNOACTIVATE) };
    assert!(hasta(vistas), "la ventana ha vuelto y el marco no");

    drop(ventana);
    assert!(hasta(|| tiras_en_pantalla(&marco).iter().all(|(_, v)| !*v)), "con la ventana cerrada, el marco se ve");
}

/// Foto de cada ventana del marco: su sitio en la pantalla y sus pixeles en BGRA, tal
/// como los compone Windows (`PW_RENDERFULLCONTENT`), sin que aparezca nada en pantalla.
///
/// Ojo: de una ventana con alfa por pixel, Windows entrega el color ya mezclado sobre
/// negro (el premultiplicado) y **no toca el byte de alfa** (medido el 12-nov-2026: con el
/// lienzo lleno de blanco, la sombra sale negra y el alfa sigue en 255). El color se puede
/// comparar; el alfa, no.
fn fotos(marco: &Marco) -> Vec<(Rect, Vec<u8>)> {
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let mut fotos = Vec::new();
    for &ventana in &marco.ventanas {
        let hwnd = HWND(ventana as *mut core::ffi::c_void);
        let mut r = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut r) }.expect("el sitio de la ventana");
        let (ancho, alto) = (r.right - r.left, r.bottom - r.top);
        unsafe {
            let pantalla = GetDC(None);
            let memoria = CreateCompatibleDC(Some(pantalla));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: ancho,
                    biHeight: -alto,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(memoria), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                .expect("no se ha podido crear el lienzo");
            let anterior = SelectObject(memoria, dib.into());
            // El marco va fuera de toda foto (16-nov), y eso alcanza tambien a
            // `PrintWindow`: para ver su dibujo se le deja entrar un momento, y luego se
            // vuelve a sacar. Que este fuera lo comprueba `se_abre_y_se_cierra_...`.
            crate::platform::window_style::fuera_de_la_captura_hwnd(ventana, false);
            let pintada = PrintWindow(hwnd, memoria, PRINT_WINDOW_FLAGS(2)).as_bool();
            crate::platform::window_style::fuera_de_la_captura_hwnd(ventana, true);
            assert!(pintada, "la ventana no se ha dejado fotografiar");
            let n = (ancho * alto * 4) as usize;
            let pixeles = std::slice::from_raw_parts(bits as *const u8, n).to_vec();
            SelectObject(memoria, anterior);
            let _ = DeleteObject(dib.into());
            let _ = DeleteDC(memoria);
            let _ = ReleaseDC(None, pantalla);
            fotos.push((
                Rect { x: r.left, y: r.top, width: ancho as u32, height: alto as u32 },
                pixeles,
            ));
        }
    }
    fotos
}

/// Un escritorio de mentira: una pagina blanca con renglones, una foto roja y naranja (lo
/// peor para ver un marco rojo) y un editor oscuro.
fn escritorio(ancho: u32, alto: u32) -> image::RgbaImage {
    use image::Rgba;
    let mut img = image::RgbaImage::new(ancho, alto);
    let (a, b) = (ancho * 38 / 100, ancho * 62 / 100);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = if x < a {
            if y % 22 < 7 && x % 260 > 30 && x % 260 < 210 + (y * 7) % 40 {
                Rgba([206, 210, 217, 255])
            } else {
                Rgba([250, 250, 250, 255])
            }
        } else if x < b {
            let t = (x - a) as f32 / (b - a) as f32;
            let u = y as f32 / alto as f32;
            Rgba([
                (230.0 - 60.0 * u) as u8,
                (60.0 + 90.0 * t * (1.0 - u)) as u8,
                (40.0 + 80.0 * u) as u8,
                255,
            ])
        } else if y % 20 < 6 && (x - b) % 300 > 24 && (x - b) % 300 < 120 + (y * 13) % 150 {
            if (y / 20) % 3 == 0 {
                Rgba([122, 162, 247, 255])
            } else {
                Rgba([110, 116, 128, 255])
            }
        } else {
            Rgba([30, 31, 34, 255])
        };
    }
    img
}

/// El fondo de las fotos: el escritorio de mentira, o un trozo de una captura de verdad con
/// `WINSHOTX_FONDO=<png>` y `WINSHOTX_FONDO_XY=x,y` (su esquina de arriba a la izquierda).
fn fondo(ancho: u32, alto: u32) -> image::RgbaImage {
    let Ok(ruta) = std::env::var("WINSHOTX_FONDO") else { return escritorio(ancho, alto) };
    let captura = image::open(&ruta).expect("WINSHOTX_FONDO no se puede abrir").to_rgba8();
    let (x, y) = std::env::var("WINSHOTX_FONDO_XY")
        .ok()
        .and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .unwrap_or((0u32, 0u32));
    let x = x.min(captura.width().saturating_sub(ancho));
    let y = y.min(captura.height().saturating_sub(alto));
    image::imageops::crop_imm(&captura, x, y, ancho, alto).to_image()
}

/// El marco encima del fondo: cada foto en su sitio, con el color que entrega Windows
/// (premultiplicado) y el alfa de su dibujo, que la foto no trae (ver `fotos`; la prueba
/// `se_ve_su_dibujo_y_cambia_en_pausa` comprueba que el color de las dos es el mismo).
fn componer(fotos: &[(Rect, Vec<u8>)], dibujos: &[Vec<u8>], origen: (i32, i32), fondo: &mut image::RgbaImage) {
    for ((sitio, bgra), dibujo) in fotos.iter().zip(dibujos) {
        for fila in 0..sitio.height as i32 {
            for col in 0..sitio.width as i32 {
                let (x, y) = (sitio.x + col - origen.0, sitio.y + fila - origen.1);
                if x < 0 || y < 0 || x >= fondo.width() as i32 || y >= fondo.height() as i32 {
                    continue;
                }
                let i = ((fila * sitio.width as i32 + col) * 4) as usize;
                let (b, g, r) = (bgra[i] as f32, bgra[i + 1] as f32, bgra[i + 2] as f32);
                let a = dibujo[i + 3] as f32 / 255.0;
                let p = fondo.get_pixel_mut(x as u32, y as u32);
                let mezcla = |s: f32, d: u8| (s + d as f32 * (1.0 - a)).round().min(255.0) as u8;
                *p = image::Rgba([mezcla(r, p[0]), mezcla(g, p[1]), mezcla(b, p[2]), 255]);
            }
        }
    }
}

/// Un trozo ampliado sin suavizar, para ver los pixeles de una esquina.
fn ampliar(img: &image::RgbaImage, x: u32, y: u32, lado: u32, veces: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(lado * veces, lado * veces, |i, j| {
        *img.get_pixel((x + i / veces).min(img.width() - 1), (y + j / veces).min(img.height() - 1))
    })
}

/// La hoja de un estilo a una escala: el marco de verdad sobre el fondo y, a la derecha, sus
/// dos esquinas de arriba ampliadas diez veces.
fn hoja(estilo: Estilo, escala: f64, pausado: bool) -> image::RgbaImage {
    let e = |v: f64| (v * escala).round() as i32;
    let region = Rect { x: -20_000, y: -20_000, width: e(520.0) as u32, height: e(300.0) as u32 };
    let margen = e(36.0);
    let marco = Marco::abrir_con(region, escala, None, estilo).expect("el marco tenía que abrirse");
    marco.pausado(pausado);
    std::thread::sleep(std::time::Duration::from_millis(150));
    let fotos = fotos(&marco);
    drop(marco);

    let (ancho, alto) = (region.width + 2 * margen as u32, region.height + 2 * margen as u32);
    let mut lienzo = fondo(ancho, alto);
    let forma = por_fuera(region, escala, estilo);
    let dibujos: Vec<_> = forma.tiras.iter().map(|&tira| pintar_tira(&forma, tira, estilo, pausado)).collect();
    componer(&fotos, &dibujos, (region.x - margen, region.y - margen), &mut lienzo);

    let lado = e(20.0) as u32;
    let veces = 10;
    let izquierda = ampliar(&lienzo, margen as u32 - lado / 2, margen as u32 - lado / 2, lado, veces);
    let derecha = ampliar(&lienzo, margen as u32 + region.width - lado / 2, margen as u32 - lado / 2, lado, veces);
    let hueco = 16;
    let hoja_ancho = ancho + hueco + izquierda.width() + hueco + derecha.width();
    let hoja_alto = alto.max(izquierda.height());
    let mut hoja = image::RgbaImage::from_pixel(hoja_ancho, hoja_alto, image::Rgba([120, 120, 124, 255]));
    image::imageops::replace(&mut hoja, &lienzo, 0, 0);
    image::imageops::replace(&mut hoja, &izquierda, (ancho + hueco) as i64, 0);
    image::imageops::replace(&mut hoja, &derecha, (ancho + hueco + izquierda.width() + hueco) as i64, 0);
    hoja
}

fn carpeta_de_fotos() -> std::path::PathBuf {
    std::env::var("WINSHOTX_FOTOS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("winshotx-marco"))
}

/// Fotos del marco que se ve al grabar (`ESTILO`), sin abrir nada en pantalla, a 100, 125 y
/// 150 %.
///
/// `WINSHOTX_FOTOS=<carpeta> WINSHOTX_FOTOS_NOMBRE=despues cargo test --lib ver_el_marco -- --ignored --nocapture`
#[test]
#[ignore = "no comprueba nada: deja un PNG por escala para mirar"]
fn ver_el_marco() {
    let carpeta = carpeta_de_fotos();
    let nombre = std::env::var("WINSHOTX_FOTOS_NOMBRE").unwrap_or_else(|_| "marco".into());
    for escala in [1.0f64, 1.25, 1.5] {
        let ruta = carpeta.join(format!("{nombre}-{}.png", (escala * 100.0).round() as u32));
        let hoja = hoja(ESTILO, escala, false);
        crate::encode::png::save(&hoja, &ruta, hoja.width(), hoja.height()).expect("guardar");
        println!("{}", ruta.display());
    }
}

/// Las variaciones para que Munito elija (16-nov): cada estilo grabando a 100 y a 125 %, y en
/// pausa a 100 %. `variacion-<n>-<escala>.png` y `variacion-<n>-pausa.png`, con `n` el numero
/// de `Estilo` (0 es el de siempre).
///
/// `WINSHOTX_FOTOS=<carpeta> [WINSHOTX_FONDO=<png> WINSHOTX_FONDO_XY=x,y] cargo test --lib ver_las_variaciones_del_marco -- --ignored --nocapture`
#[test]
#[ignore = "no comprueba nada: deja los PNG de las variaciones para mirar"]
fn ver_las_variaciones_del_marco() {
    let carpeta = carpeta_de_fotos();
    std::fs::create_dir_all(&carpeta).expect("la carpeta de las fotos");
    for (n, estilo) in Estilo::TODOS.into_iter().enumerate() {
        for (escala, pausado, sufijo) in [(1.0, false, "100"), (1.25, false, "125"), (1.0, true, "pausa")] {
            let ruta = carpeta.join(format!("variacion-{n}-{sufijo}.png"));
            let hoja = hoja(estilo, escala, pausado);
            crate::encode::png::save(&hoja, &ruta, hoja.width(), hoja.height()).expect("guardar");
            println!("{}", ruta.display());
        }
    }
}
