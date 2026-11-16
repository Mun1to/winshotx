//! Las pruebas del marco de grabar (`marco.rs`). Van aparte porque son mas largas que el
//! marco: casi todo lo que puede salir mal en el se ve en un pixel.

use super::*;

/// La forma con todos los lados por fuera, que es la de una region en medio de su pantalla.
fn por_fuera(region: Rect, escala: f64, estilo: Estilo) -> Forma {
    Forma::nueva(region, Medidas::a_escala(escala, estilo), [false; 4])
}

fn se_pisan(a: &Rect, b: &Rect) -> bool {
    comun(a, b) > 0
}

/// Ni un pixel del marco cae dentro de la region cuando va por fuera: lo que se graba se
/// recorta exacto, y una linea que se metiera un pixel saldria en todos los fotogramas.
#[test]
fn el_marco_va_por_fuera_de_la_region() {
    let region = Rect { x: 100, y: 200, width: 640, height: 480 };
    let [arriba, abajo, izquierda, derecha] = por_fuera(region, 1.0, Estilo::Rojo).tiras;
    // A 100 %: 2 de hueco, 2 de linea y 5 de sombra.
    assert_eq!((arriba.x, arriba.y, arriba.width, arriba.height), (91, 191, 658, 9));
    assert_eq!((abajo.x, abajo.y, abajo.width, abajo.height), (91, 680, 658, 9));
    assert_eq!((izquierda.x, izquierda.y, izquierda.width, izquierda.height), (91, 200, 9, 480));
    assert_eq!((derecha.x, derecha.y, derecha.width, derecha.height), (740, 200, 9, 480));
    for estilo in Estilo::TODOS {
        for escala in [1.0, 1.25, 1.5, 2.0] {
            for tira in por_fuera(region, escala, estilo).tiras {
                assert!(!se_pisan(&tira, &region), "la tira {tira:?} pisa la region grabada a {escala}");
            }
        }
    }
}

/// Ninguna tira se queda sin tamanno, ni con una escala diminuta ni con una region de un pixel
/// con todos los lados por dentro.
#[test]
fn ninguna_tira_se_queda_sin_tamanno() {
    let region = Rect { x: 0, y: 0, width: 1, height: 1 };
    for dentro in [[false; 4], [true; 4]] {
        for tira in Forma::nueva(region, Medidas::a_escala(0.01, Estilo::Rojo), dentro).tiras {
            assert!(tira.width >= 1 && tira.height >= 1, "{tira:?}");
        }
    }
}

/// La linea crece con la escala de la pantalla, siempre en pixeles enteros (2,5 pixeles
/// no existen y se verian borrosos), y una escala rota se trata como 100 %.
#[test]
fn las_medidas_crecen_con_la_escala_en_pixeles_enteros() {
    let m = |e: f64| {
        let m = Medidas::a_escala(e, Estilo::Rojo);
        (m.hueco, m.linea, m.radio, m.halo)
    };
    assert_eq!(m(1.0), (2, 2, 6, 5));
    assert_eq!(m(1.25), (3, 3, 8, 6));
    assert_eq!(m(1.5), (3, 3, 9, 8));
    assert_eq!(m(1.75), (4, 4, 11, 9));
    assert_eq!(m(2.0), (4, 4, 12, 10));
    assert_eq!(m(0.0), m(1.0));
    assert_eq!(m(f64::NAN), m(1.0));
    // La luz es mas ancha que la sombra: una luz de cinco pixeles no se ve.
    assert_eq!(Medidas::a_escala(1.0, Estilo::AzulConLuz).halo, 9);
}

/// Los pixeles de una columna de la tira de arriba, de la region hacia fuera:
/// (rojo, verde, azul, alfa), premultiplicados.
fn columna(forma: &Forma, estilo: Estilo, pausado: bool, col: i32) -> Vec<[u8; 4]> {
    let tira = forma.tiras[0];
    let dibujo = pintar_tira(forma, tira, estilo, pausado);
    (0..tira.height as i32)
        .rev()
        .map(|fila| {
            let i = ((fila * tira.width as i32 + col) * 4) as usize;
            [dibujo[i + 2], dibujo[i + 1], dibujo[i], dibujo[i + 3]]
        })
        .collect()
}

/// En el tramo recto, de la region hacia fuera: el hueco limpio, la linea de pixeles
/// enteros y del grosor que toca (tantas filas SEGUIDAS de su color exacto y opacas como dice
/// la escala, y ni una mas) y el halo, que se apaga hacia fuera hasta casi nada. La sombra no
/// pone color, solo oscurece; la luz es del color del estado.
#[test]
fn la_linea_es_nitida_y_de_su_grosor_y_el_halo_se_apaga() {
    let region = Rect { x: 1000, y: 1000, width: 400, height: 300 };
    for estilo in Estilo::TODOS {
        for pausado in [false, true] {
            for escala in [1.0, 1.25, 1.5, 2.0] {
                let forma = por_fuera(region, escala, estilo);
                let medidas = forma.medidas;
                let px = columna(&forma, estilo, pausado, 200);
                let (hueco, linea) = (medidas.hueco as usize, medidas.linea as usize);
                let que = format!("{estilo:?}, pausado {pausado}, a {escala}: {px:?}");
                assert!(px[..hueco].iter().all(|p| *p == [0, 0, 0, 0]), "el hueco, limpio, {que}");
                let Color(r, g, b) = estilo.linea(pausado);
                let opacas = px[hueco..].iter().take_while(|p| **p == [r, g, b, 255]).count();
                assert_eq!(opacas, linea, "{que}");
                let halo = &px[hueco + linea..];
                assert_eq!(halo.len() as i32, medidas.halo);
                let color = estilo.halo(pausado).color;
                if color == NEGRO {
                    assert!(halo.iter().all(|p| p[..3] == [0, 0, 0]), "la sombra no pone color, {que}");
                } else {
                    // Premultiplicado: cada canal es su color por el alfa, y el de la luz manda.
                    let Color(lr, lg, lb) = color;
                    let pegado = halo[0];
                    let cerca = |c: u8, l: u8| (c as f32 - l as f32 * pegado[3] as f32 / 255.0).abs() <= 2.0;
                    assert!(cerca(pegado[0], lr) && cerca(pegado[1], lg) && cerca(pegado[2], lb), "la luz, de su color, {que}");
                }
                assert!(halo[0][3] >= 50, "pegado a la linea se tiene que notar, {que}");
                assert!(halo.windows(2).all(|w| w[1][3] < w[0][3]), "se apaga hacia fuera, {que}");
                assert!(halo.last().unwrap()[3] <= 12, "y acaba en casi nada, {que}");
            }
        }
    }
}

/// La esquina de la region nunca asoma por dentro de la linea, a ninguna escala: si
/// asomara, la linea se cortaria en la esquina (las tiras no pueden pintar dentro de la
/// region). Es lo que limita el radio con el hueco que hay (`RADIO_POR_HUECO`): a 120 %,
/// por ejemplo, el hueco redondea a 2 y el radio a 7, y con 7 la esquina asomaria.
#[test]
fn la_region_queda_dentro_de_la_linea() {
    let region = Rect { x: 0, y: 0, width: 400, height: 300 };
    let mut escala = 0.5;
    while escala <= 3.0 {
        let forma = por_fuera(region, escala, Estilo::Rojo);
        for (x, y) in [(0.0, 0.0), (400.0, 0.0), (0.0, 300.0), (400.0, 300.0)] {
            let d = distancia(&forma, x, y);
            assert!(d <= 0.0, "a {escala} la esquina ({x}, {y}) asoma {d} pixeles: {:?}", forma.medidas);
        }
        escala += 0.05;
    }
}

/// Las esquinas son redondas de verdad y suaves: el pixel donde iria la esquina de un
/// rectangulo queda sin color, el pico de fuera de la tira vacio, la curva tiene pixeles a
/// medias (sin escalones) y la linea mide en la diagonal lo mismo que en los lados. El
/// marco de antes del 12-nov era un rectangulo de color: en su esquina, rojo entero.
#[test]
fn la_esquina_es_redonda_y_suave() {
    let region = Rect { x: 0, y: 0, width: 400, height: 300 };
    for escala in [1.0, 1.25, 1.5, 2.0] {
        let forma = por_fuera(region, escala, Estilo::Rojo);
        let m = forma.medidas;
        let g = m.grosor();
        let tira = forma.tiras[0];
        let dibujo = pintar_tira(&forma, tira, Estilo::Rojo, false);
        // Pixel contado desde la esquina de la region (negativo hacia fuera).
        let pixel = |x: i32, y: i32| {
            let i = (((y + g) * tira.width as i32 + (x + g)) * 4) as usize;
            [dibujo[i + 2], dibujo[i + 1], dibujo[i], dibujo[i + 3]]
        };
        assert_eq!(pixel(-g, -g)[3], 0, "el pico de fuera, vacio, a {escala}");
        let esquina = -(m.hueco + m.linea);
        assert!(
            pixel(esquina, esquina)[0] < 0xEF / 2,
            "donde iria la esquina en angulo recto hay rojo a {escala}: {:?}",
            pixel(esquina, esquina)
        );
        let a_medias = (-g..0)
            .flat_map(|y| (-g..0).map(move |x| (x, y)))
            .filter(|&(x, y)| (1..0xEF).contains(&pixel(x, y)[0]))
            .count();
        assert!(a_medias >= 4, "la curva no tiene borde suave a {escala}");
        let rojos = (1..=g).filter(|&k| pixel(-k, -k)[0] as f32 >= 0xEF as f32 * 0.5).count();
        let diagonal = m.linea as f32 / std::f32::consts::SQRT_2;
        assert!(
            (rojos as f32 - diagonal).abs() <= 1.0,
            "la esquina no tiene el grosor de los lados a {escala}: {rojos} frente a {diagonal}"
        );
    }
}

/// Las pantallas de Munito, medidas el 16-nov: la principal de 1920x1080, otra igual a su
/// izquierda y mas baja (empieza en y=483) y el portatil (1920x1200 fisicos al 125 %) debajo de
/// la principal.
fn sus_pantallas() -> Vec<Rect> {
    vec![
        Rect { x: 0, y: 0, width: 1920, height: 1080 },
        Rect { x: -1920, y: 483, width: 1920, height: 1080 },
        Rect { x: 0, y: 1080, width: 1920, height: 1200 },
    ]
}

/// Un lado va por dentro cuando por fuera caeria en otra pantalla o en ninguna. Grabando la
/// principal entera, el marco salia en el borde del monitor de la izquierda y en el de arriba
/// del portatil, y en la principal no se veia (16-nov, «el marco de grabar se bugea»).
#[test]
fn un_lado_pegado_al_borde_de_su_pantalla_va_por_dentro() {
    let pantallas = sus_pantallas();
    let g = Medidas::a_escala(1.0, Estilo::Rojo).grosor();
    let dentro = |x, y, width, height| lados_por_dentro(Rect { x, y, width, height }, g, &pantallas);
    // Arriba, abajo, izquierda, derecha.
    assert_eq!(dentro(0, 0, 1920, 1080), [true; 4], "la principal entera");
    assert_eq!(dentro(300, 200, 800, 500), [false; 4], "en medio de la principal");
    assert_eq!(dentro(0, 200, 800, 500), [false, false, true, false], "pegada a la izquierda de la principal");
    assert_eq!(dentro(1120, 580, 800, 500), [false, true, false, true], "en la esquina de abajo a la derecha");
    assert_eq!(dentro(4, 4, 800, 500), [true, false, true, false], "a menos del marco del borde");
    assert_eq!(dentro(-400, 600, 800, 400), [false; 4], "cruzando a la de la izquierda: las dos son suyas");
    assert_eq!(dentro(-400, 300, 800, 400), [true, false, true, false], "por encima de la izquierda no hay pantalla, ni a la izquierda de su trozo de arriba");
    assert_eq!(dentro(-1920, 483, 1920, 1080), [true; 4], "la de la izquierda entera: su derecha da a las otras dos");
    assert_eq!(dentro(0, 1080, 1920, 1200), [true; 4], "el portatil entero");
    // Una region que no toca ninguna pantalla (las pruebas, en -20000) va toda por fuera.
    assert_eq!(dentro(-20_000, -20_000, 200, 100), [false; 4]);
}

/// Por dentro, la linea va pegada al borde de la region, de su grosor y sin un pixel fuera: las
/// tiras no salen de la region, y la de arriba es la linea entera, de su color.
#[test]
fn por_dentro_la_linea_va_pegada_al_borde_sin_salirse() {
    let region = Rect { x: 0, y: 0, width: 1920, height: 1080 };
    for escala in [1.0, 1.25, 1.5] {
        let forma = Forma::nueva(region, Medidas::a_escala(escala, Estilo::Rojo), [true; 4]);
        let linea = forma.medidas.linea as u32;
        for tira in forma.tiras {
            assert_eq!(comun(&tira, &region), i64::from(tira.width) * i64::from(tira.height), "{tira:?} sale de la region");
        }
        assert_eq!(forma.tiras[0].height, linea);
        let dibujo = pintar_tira(&forma, forma.tiras[0], Estilo::Rojo, false);
        // Lejos de las esquinas, toda la tira es la linea; y las esquinas, en angulo recto y
        // llenas (redondas se cortarian dentro de la region).
        for col in [0, 1, 500, 1918, 1919] {
            for fila in 0..linea {
                let i = ((fila * 1920 + col) * 4) as usize;
                assert_eq!(&dibujo[i..i + 4], &[0x44, 0x44, 0xEF, 255], "fila {fila}, columna {col} a {escala}");
            }
        }
    }
}

/// Con un lado por dentro y el de al lado por fuera, la esquina se cierra: las dos lineas se
/// tocan, no queda un pico sin pintar y nada cae del lado de la otra pantalla.
#[test]
fn una_esquina_mitad_por_dentro_se_cierra() {
    // Pegada a la izquierda de su pantalla: la izquierda por dentro, el resto por fuera.
    let region = Rect { x: 0, y: 200, width: 800, height: 500 };
    let forma = Forma::nueva(region, Medidas::a_escala(1.0, Estilo::Rojo), [false, false, true, false]);
    for tira in forma.tiras {
        assert!(tira.x >= 0, "{tira:?} cae en la pantalla de la izquierda");
    }
    let m = forma.medidas;
    // El pico de la esquina de arriba a la izquierda: donde la linea de la izquierda (x de 0 a
    // 2) sube hasta la de arriba (y de -4 a -2), todo de color.
    for (x, y) in [(0.5, -m.hueco as f32 - 0.5), (0.5, -(m.hueco + m.linea) as f32 + 0.5), (1.5, -0.5)] {
        let d = distancia(&forma, x, y);
        assert!((0.0..=m.linea as f32).contains(&d), "({x}, {y}) no es linea: {d}");
    }
    assert_eq!(forma.radios, [0.0, m.radio as f32, 0.0, m.radio as f32]);
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
