//! Las pruebas del marco en X11 (`marco_x11.rs`).
//!
//! Las que abren el marco de verdad necesitan un servidor X y pintan en su pantalla, asi que
//! no entran en la tanda normal. En un servidor de pruebas (un `Xvfb`):
//! `DISPLAY=:77 cargo test --lib marco_x11 -- --ignored --test-threads=1`

use super::*;

use x11rb::protocol::xproto::MapState;

/// Cuantas veces cubren esos rectangulos cada pixel.
fn veces_cubierto(rectangulos: &[Rectangle], ancho: usize, alto: usize) -> Vec<u32> {
    let mut veces = vec![0u32; ancho * alto];
    for r in rectangulos {
        for y in r.y as usize..r.y as usize + usize::from(r.height) {
            for x in r.x as usize..r.x as usize + usize::from(r.width) {
                veces[y * ancho + x] += 1;
            }
        }
    }
    veces
}

fn medidas_de(r: &Rectangle) -> (i16, i16, u16, u16) {
    (r.x, r.y, r.width, r.height)
}

/// Los rectangulos con que se recorta una ventana cubren justo lo marcado, cada pixel una sola
/// vez, y son pocos: lo recto sale en un rectangulo, no en uno por fila.
#[test]
fn los_rectangulos_cubren_justo_lo_marcado() {
    let lleno = rectangulos(&[true; 21], 7);
    assert_eq!(lleno.iter().map(medidas_de).collect::<Vec<_>>(), vec![(0, 0, 7, 3)]);
    assert!(rectangulos(&[false; 21], 7).is_empty());
    // Dos tramos en la misma fila, y una fila distinta debajo.
    let dos = rectangulos(&[true, false, true, true, true, false, true, true, false, false, false, true], 4);
    assert_eq!(
        dos.iter().map(medidas_de).collect::<Vec<_>>(),
        vec![(0, 0, 1, 2), (2, 0, 2, 2), (3, 2, 1, 1)],
    );

    // La linea de un marco de verdad, con sus esquinas redondas.
    let region = Rect { x: 100, y: 200, width: 640, height: 480 };
    for escala in [1.0, 1.25, 1.5, 2.0] {
        let forma = Forma::nueva(region, Medidas::a_escala(escala, ESTILO), [false; 4]);
        let mut cuantos = 0;
        for tira in forma.tiras {
            let marcados = linea_de_tira(&forma, tira);
            let (ancho, alto) = (tira.width as usize, tira.height as usize);
            let recorte = rectangulos(&marcados, ancho);
            let veces = veces_cubierto(&recorte, ancho, alto);
            assert!(
                marcados.iter().zip(&veces).all(|(marcado, veces)| u32::from(*marcado) == *veces),
                "el recorte de {tira:?} a {escala} no es la linea"
            );
            cuantos += recorte.len();
        }
        assert!(cuantos <= 100, "a {escala} el marco se recorta con {cuantos} rectangulos");
    }
}

/// Con un lado que no se pinta (una zona pegada al borde izquierdo de la pantalla), las lineas
/// de arriba y de abajo acaban rectas en el borde de la zona: sin el piquito que la figura
/// comun deja para unirlas con el lado de dentro.
#[test]
fn sin_un_lado_la_linea_acaba_recta_sin_piquito() {
    let region = Rect { x: 0, y: 200, width: 350, height: 320 };
    let medidas = Medidas::a_escala(1.0, ESTILO);
    let dentro = [false, false, true, false];
    let forma = Forma::nueva(region, medidas, dentro);
    let (hueco, linea) = (medidas.hueco as usize, medidas.linea as usize);
    for (n, tira) in [forma.tiras[0], forma.tiras[1]].into_iter().enumerate() {
        let (ancho, alto) = (tira.width as usize, tira.height as usize);
        let mut marcados = linea_de_tira(&forma, tira);
        let sobra = arranques(region, medidas, dentro, tira);
        assert_eq!(sobra, vec![(0, if n == 0 { alto - hueco } else { 0 }, linea, hueco)], "tira {n}");
        // Las filas del hueco: las pegadas a la region.
        let del_hueco = if n == 0 { alto - hueco..alto } else { 0..hueco };
        let en_el_hueco =
            |m: &[bool]| del_hueco.clone().flat_map(|f| (0..ancho / 2).map(move |c| f * ancho + c)).filter(|&i| m[i]).count();
        assert_eq!(en_el_hueco(&marcados), linea * hueco, "tira {n}: la figura ya no deja arranque, y `arranques` sobra");
        vaciar(&mut marcados, 1, ancho, &sobra, false);
        assert_eq!(en_el_hueco(&marcados), 0, "tira {n}: queda piquito");
        // Y la linea sigue llegando al borde de la zona.
        let de_la_linea = if n == 0 { alto - hueco - 1 } else { hueco };
        assert!(marcados[de_la_linea * ancho], "tira {n}: la linea no llega al borde");
    }
    // Con los cuatro lados por fuera no sobra nada.
    let entera = Forma::nueva(Rect { x: 300, ..region }, medidas, [false; 4]);
    assert!(entera.tiras.iter().all(|&tira| arranques(Rect { x: 300, ..region }, medidas, [false; 4], tira).is_empty()));
}

/// Las ventanas de arriba del todo que hay en la pantalla, de abajo arriba.
fn ventanas(conexion: &RustConnection, raiz: Window) -> Vec<Window> {
    conexion.query_tree(raiz).expect("pedir las ventanas").reply().expect("las ventanas").children
}

/// El color de un punto de la pantalla, tal como se ve (rojo, verde, azul).
fn color_en(conexion: &RustConnection, raiz: Window, x: i32, y: i32) -> (u8, u8, u8) {
    let foto = conexion
        .get_image(ImageFormat::Z_PIXMAP, raiz, x as i16, y as i16, 1, 1, !0)
        .expect("pedir el punto")
        .reply()
        .expect("el punto");
    (foto.data[2], foto.data[1], foto.data[0])
}

/// Espera (poco) a que el hilo del marco se entere de algo.
fn hasta(que: impl Fn() -> bool) -> bool {
    let empezo = std::time::Instant::now();
    while empezo.elapsed() < Duration::from_secs(3) {
        if que() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// El marco de verdad en un servidor X: las cuatro tiras estan en su sitio y el gestor de
/// ventanas no las toca; en la pantalla se ve la linea de su color y dentro de la region no
/// hay nada; en pausa cambia de color; el raton las atraviesa; y al soltarlo no queda nada.
#[test]
#[ignore = "necesita un servidor X (DISPLAY) y pinta en su pantalla"]
fn en_un_servidor_x_el_marco_rodea_la_region_y_no_la_pisa() {
    let (conexion, numero) = RustConnection::connect(None).expect("hace falta un servidor X");
    let raiz = conexion.setup().roots[numero].root;
    let antes = ventanas(&conexion, raiz);
    let region = Rect { x: 300, y: 200, width: 400, height: 300 };
    let dentro_antes = color_en(&conexion, raiz, region.x, region.y);

    let marco = Marco::abrir(region, 1.0).expect("el marco tenía que abrirse");
    let nuevas: Vec<Window> = ventanas(&conexion, raiz).into_iter().filter(|v| !antes.contains(v)).collect();
    assert_eq!(nuevas.len(), 4, "tienen que ser cuatro tiras");
    let medidas = Medidas::a_escala(1.0, ESTILO);
    let forma = Forma::nueva(region, medidas, [false; 4]);
    for tira in forma.tiras {
        let esta = nuevas.iter().any(|&ventana| {
            let sitio = conexion.get_geometry(ventana).unwrap().reply().unwrap();
            let como = conexion.get_window_attributes(ventana).unwrap().reply().unwrap();
            (i32::from(sitio.x), i32::from(sitio.y), u32::from(sitio.width), u32::from(sitio.height))
                == (tira.x, tira.y, tira.width, tira.height)
                && como.override_redirect
                && como.map_state == MapState::VIEWABLE
        });
        assert!(esta, "no hay una tira en {tira:?}");
    }

    // En medio de cada lado, la linea: a `hueco` pixeles de la region, hacia fuera.
    let fuera = medidas.hueco + 1;
    let (medio_x, medio_y) = (region.x + 200, region.y + 150);
    let (derecha, abajo) = (region.x + region.width as i32, region.y + region.height as i32);
    let en_la_linea = [
        (medio_x, region.y - fuera),
        (medio_x, abajo + fuera - 1),
        (region.x - fuera, medio_y),
        (derecha + fuera - 1, medio_y),
    ];
    let de_color = |pausado: bool| {
        let Color(r, g, b) = ESTILO.linea(pausado);
        en_la_linea.iter().all(|&(x, y)| color_en(&conexion, raiz, x, y) == (r, g, b))
    };
    assert!(hasta(|| de_color(false)), "la linea no se ve en la pantalla");
    // Lo que se graba queda limpio: ni en la esquina ni pegado al borde hay nada del marco.
    for (x, y) in [(region.x, region.y), (derecha - 1, abajo - 1), (medio_x, region.y), (region.x, medio_y)] {
        assert_eq!(color_en(&conexion, raiz, x, y), dentro_antes, "el marco pisa la region en {x},{y}");
    }

    marco.pausado(true);
    assert!(hasta(|| de_color(true)), "en pausa la linea no cambia de color");
    marco.pausado(false);
    assert!(hasta(|| de_color(false)), "al seguir, la linea no vuelve a su color");

    // Con el raton encima de la linea, lo que tiene debajo no es una de las tiras.
    let (x, y) = en_la_linea[0];
    conexion.warp_pointer(x11rb::NONE, raiz, 0, 0, 0, 0, x as i16, y as i16).unwrap();
    conexion.sync().unwrap();
    let raton = conexion.query_pointer(raiz).unwrap().reply().unwrap();
    assert_eq!((i32::from(raton.root_x), i32::from(raton.root_y)), (x, y), "el raton no ha ido a la linea");
    assert!(!nuevas.contains(&raton.child), "la tira se queda con el raton");

    drop(marco);
    let quedan: Vec<Window> = ventanas(&conexion, raiz).into_iter().filter(|v| !antes.contains(v)).collect();
    assert!(quedan.is_empty(), "al cerrar el marco quedan ventanas: {quedan:?}");
}

/// Un lado que por fuera no cabe en la pantalla no se pinta (saldria en el video si fuera por
/// dentro), y grabando la pantalla entera no hay marco y abrirlo no es un error.
#[test]
#[ignore = "necesita un servidor X (DISPLAY) y pinta en su pantalla"]
fn en_un_servidor_x_un_lado_que_no_cabe_por_fuera_no_se_pinta() {
    let (conexion, numero) = RustConnection::connect(None).expect("hace falta un servidor X");
    let (raiz, ancho, alto) = {
        let p = &conexion.setup().roots[numero];
        (p.root, u32::from(p.width_in_pixels), u32::from(p.height_in_pixels))
    };
    let antes = ventanas(&conexion, raiz);
    let nuevas = || ventanas(&conexion, raiz).into_iter().filter(|v| !antes.contains(v)).count();

    // Pegada a la esquina de arriba a la izquierda: solo caben el lado de abajo y el derecho.
    let esquina = Marco::abrir(Rect { x: 0, y: 0, width: 400, height: 300 }, 1.0).expect("el marco");
    assert_eq!(nuevas(), 2, "pegada a la esquina tienen que quedar dos tiras");
    drop(esquina);
    assert_eq!(nuevas(), 0);

    let entera = Marco::abrir(Rect { x: 0, y: 0, width: ancho, height: alto }, 1.0).expect("el marco");
    assert_eq!(nuevas(), 0, "grabando la pantalla entera no hay marco");
    entera.pausado(true);
    drop(entera);
}
