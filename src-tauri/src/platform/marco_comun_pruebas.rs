//! Las pruebas de la figura del marco y de su dibujo (`marco_comun.rs`): casi todo lo que
//! puede salir mal en el marco se ve en un pixel, y eso se mira igual en cualquier sistema.

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
