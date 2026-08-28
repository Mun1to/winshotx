//! Los aros de los clics y la pastilla de los atajos, dibujados **al exportar**.
//!
//! Antes se pintaban sobre los fotogramas mientras se grababa, y eso tenía dos problemas.
//!
//! El primero es que quedaban cocidos: había que decidir antes de grabar si se querían, y
//! si te arrepentías, a grabar otra vez. El segundo aparece con el zoom: un aro cocido se
//! amplía con la imagen, así que al acercarse la cámara salía gordo y borroso. Dibujándolo
//! aquí sale del mismo tamaño y nítido, esté la cámara donde esté.
//!
//! Lo que se guarda al grabar es una lista de puntos y de textos. Todo lo demás es
//! aritmética sobre fotogramas que ya están en disco.

use image::RgbaImage;

use crate::encode::recorte::Recorte;
use crate::encode::zoom::Clic;
use crate::record::{pastilla, realce, teclas::Atajo};

/// Qué se dibuja encima. Lo elige quien exporta.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ajustes {
    /// Un aro donde se pulsó.
    pub clics: bool,
    /// Una pastilla abajo con el atajo que se acaba de pulsar.
    pub teclas: bool,
    /// El alto del puntero dibujado, en píxeles del fotograma final. Cero es no dibujarlo.
    ///
    /// Se dibuja aquí en vez de usar el que capturó Windows porque así **se puede hacer
    /// grande sin pixelarlo**: no se amplía una imagen, se dibuja la forma a otra escala.
    pub cursor: f32,
}

impl Ajustes {
    pub fn hay_algo(&self) -> bool {
        self.clics || self.teclas || self.cursor > 0.0
    }
}

/// Lleva un punto de la región grabada al fotograma que se va a escribir.
///
/// Por el mismo camino que las anotaciones: cada recorte lo vuelve a medir, y al final se
/// pasa al tamaño de salida. Sin esto, un aro dibujado con el zoom puesto caería donde
/// estaba el clic en la pantalla, no donde está ahora en el cuadro.
///
/// Devuelve `None` si el punto se quedó fuera de algún recorte: un clic que no se ve no
/// tiene aro.
fn colocar(
    x: i32,
    y: i32,
    origen: (u32, u32),
    recortes: &[Recorte],
    destino: (u32, u32),
) -> Option<(i32, i32)> {
    let (mut u, mut v) = (
        x as f32 / origen.0.max(1) as f32,
        y as f32 / origen.1.max(1) as f32,
    );
    for r in recortes {
        let (x1, y1, x2, y2) = (
            r.x1.min(r.x2),
            r.y1.min(r.y2),
            r.x1.max(r.x2),
            r.y1.max(r.y2),
        );
        u = (u - x1) / (x2 - x1).max(f32::EPSILON);
        v = (v - y1) / (y2 - y1).max(f32::EPSILON);
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            return None;
        }
    }
    Some((
        (u * destino.0 as f32).round() as i32,
        (v * destino.1 as f32).round() as i32,
    ))
}

/// Dibuja sobre el fotograma lo que toque en ese instante.
///
/// `origen` es el tamaño de la región grabada, que es en cuyo sistema están los clics.
/// `recortes` son los que se aplicaron antes de escalar, en orden.
#[allow(clippy::too_many_arguments)]
pub fn pintar(
    imagen: &mut RgbaImage,
    ms: u64,
    clics: &[Clic],
    atajos: &[Atajo],
    rastro: &[(u64, i32, i32)],
    origen: (u32, u32),
    recortes: &[Recorte],
    ajustes: &Ajustes,
    pastillas: &mut pastilla::Cache,
) {
    let destino = (imagen.width(), imagen.height());

    // El puntero va DEBAJO del aro: el aro se abre desde donde se pulsó, y tapar el
    // cursor con él sería tapar justo lo que se está señalando.
    if ajustes.cursor > 0.0 {
        if let Some((_, x, y)) = donde_estaba(rastro, ms) {
            if let Some((px, py)) = colocar(x, y, origen, recortes, destino) {
                super::cursor::pintar(imagen, px, py, ajustes.cursor);
            }
        }
    }

    if ajustes.clics {
        // Solo los que todavía se ven: el aro dura menos de medio segundo.
        let visibles: Vec<realce::Clic> = clics
            .iter()
            .filter(|c| ms >= c.ms && ms - c.ms < realce::DURACION_MS)
            .filter_map(|c| {
                colocar(c.x, c.y, origen, recortes, destino).map(|(x, y)| realce::Clic {
                    x,
                    y,
                    ms: c.ms,
                    derecho: c.derecho,
                })
            })
            .collect();
        if !visibles.is_empty() {
            // Los puntos ya vienen en coordenadas del fotograma, así que no hay que
            // restarle ninguna región: eso ya lo hizo `colocar`.
            realce::pintar(imagen, &visibles, 0, 0, ms);
        }
    }

    if ajustes.teclas {
        // El último atajo que sigue vivo. Dos a la vez se taparían el uno al otro.
        if let Some(a) = atajos
            .iter().rfind(|a| ms >= a.ms && ms - a.ms < teclas_duracion())
        {
            let opaca = pastilla::opacidad(ms - a.ms, teclas_duracion());
            if opaca > 0.0 {
                if let Some(dibujo) = pastillas.pastilla(&a.texto) {
                    pastilla::pegar(imagen, dibujo, opaca);
                }
            }
        }
    }
}

fn teclas_duracion() -> u64 {
    crate::record::teclas::DURACION_MS
}

/// Dónde estaba el ratón en ese instante, según el rastro anotado al grabar.
///
/// Se coge la última anotación que no sea posterior: el rastro lleva una por fotograma, así
/// que la que toca es la del propio fotograma. Con una búsqueda binaria porque esto se
/// pregunta una vez por cada fotograma exportado.
pub fn donde_estaba(rastro: &[(u64, i32, i32)], ms: u64) -> Option<(u64, i32, i32)> {
    match rastro.binary_search_by_key(&ms, |(t, _, _)| *t) {
        Ok(i) => rastro.get(i).copied(),
        Err(0) => None,
        Err(i) => rastro.get(i - 1).copied(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entera() -> Vec<Recorte> {
        Vec::new()
    }

    fn mitad_derecha() -> Vec<Recorte> {
        vec![Recorte {
            x1: 0.5,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        }]
    }

    #[test]
    fn sin_recortes_ni_escalado_el_punto_se_queda_donde_estaba() {
        let p = colocar(100, 50, (400, 300), &entera(), (400, 300));
        assert_eq!(p, Some((100, 50)));
    }

    #[test]
    fn al_exportar_al_doble_el_punto_va_al_doble() {
        // El fotograma se escala, y el aro tiene que seguir encima de lo que se pulso.
        let p = colocar(100, 50, (400, 300), &entera(), (800, 600));
        assert_eq!(p, Some((200, 100)));
    }

    #[test]
    fn con_un_recorte_el_punto_se_mide_sobre_el_trozo() {
        // El clic en 300 de una captura de 400 es el 100 de la mitad derecha.
        let p = colocar(300, 150, (400, 300), &mitad_derecha(), (200, 300));
        assert_eq!(p, Some((100, 150)));
    }

    #[test]
    fn un_punto_que_se_quedo_fuera_no_se_dibuja() {
        // Un clic que no se ve no tiene aro. Pegarlo al borde lo pondria donde nadie pulso.
        assert_eq!(colocar(20, 150, (400, 300), &mitad_derecha(), (200, 300)), None);
    }

    #[test]
    fn los_recortes_se_encadenan_igual_que_al_recortar_la_imagen() {
        // El del usuario y despues el de la camara del zoom, que se mide sobre el primero.
        let camara = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 0.5,
            y2: 1.0,
        };
        let mut recortes = mitad_derecha();
        recortes.push(camara);
        // El clic en 250 cae en el 50 de la mitad derecha, que es el 50 de su mitad
        // izquierda: justo el centro de un trozo de 100 de ancho.
        let p = colocar(250, 150, (400, 300), &recortes, (100, 300));
        assert_eq!(p, Some((50, 150)));
    }

    #[test]
    fn un_aro_solo_se_dibuja_mientras_dura() {
        let clic = Clic {
            ms: 1000,
            x: 200,
            y: 150,
            derecho: false,
        };
        let ajustes = Ajustes {
            clics: true,
            teclas: false,
            cursor: 0.0,
        };
        let mut cache = pastilla::Cache::default();
        let mut tocado = |ms: u64| {
            let mut imagen = RgbaImage::from_pixel(400, 300, image::Rgba([0, 0, 0, 255]));
            pintar(
                &mut imagen,
                ms,
                std::slice::from_ref(&clic),
                &[],
                &[],
                (400, 300),
                &entera(),
                &ajustes,
                &mut cache,
            );
            imagen.pixels().any(|p| p.0 != [0, 0, 0, 255])
        };
        assert!(tocado(1000), "en el momento del clic tendria que verse");
        assert!(
            !tocado(1000 + realce::DURACION_MS + 1),
            "medio segundo despues ya no tendria que quedar nada"
        );
        assert!(!tocado(500), "antes del clic no puede haber aro");
    }

    #[test]
    fn apagado_no_dibuja_nada() {
        let clic = Clic {
            ms: 1000,
            x: 200,
            y: 150,
            derecho: false,
        };
        let mut imagen = RgbaImage::from_pixel(400, 300, image::Rgba([0, 0, 0, 255]));
        let mut cache = pastilla::Cache::default();
        pintar(
            &mut imagen,
            1000,
            std::slice::from_ref(&clic),
            &[],
            &[],
            (400, 300),
            &entera(),
            &Ajustes::default(),
            &mut cache,
        );
        assert!(imagen.pixels().all(|p| p.0 == [0, 0, 0, 255]));
    }

    #[test]
    fn el_aro_no_se_agranda_con_el_zoom() {
        // Es la razon de dibujarlo aqui y no al grabar: cocido, la camara lo ampliaba y
        // salia gordo y borroso. Dibujado ahora, mide lo mismo con zoom y sin el.
        let clic = Clic {
            ms: 1000,
            x: 200,
            y: 150,
            derecho: false,
        };
        let ajustes = Ajustes {
            clics: true,
            teclas: false,
            cursor: 0.0,
        };
        let mut cache = pastilla::Cache::default();
        let mut cuantos = |recortes: &[Recorte]| {
            let mut imagen = RgbaImage::from_pixel(400, 300, image::Rgba([0, 0, 0, 255]));
            pintar(
                &mut imagen,
                1000,
                std::slice::from_ref(&clic),
                &[],
                &[],
                (400, 300),
                recortes,
                &ajustes,
                &mut cache,
            );
            imagen.pixels().filter(|p| p.0 != [0, 0, 0, 255]).count()
        };
        let sin_zoom = cuantos(&entera());
        let con_zoom = cuantos(&[Recorte {
            x1: 0.25,
            y1: 0.25,
            x2: 0.75,
            y2: 0.75,
        }]);
        assert_eq!(
            sin_zoom, con_zoom,
            "el aro cambia de tamanno con la camara: {sin_zoom} vs {con_zoom}"
        );
    }

    /// Cuanto cuesta vestir un fotograma, que es lo que se paga en CADA uno al exportar.
    ///
    /// Munir, el 27 de agosto de 2026: «por que tarda tanto en guardar el video?». La
    /// exportacion de un minuto son mil ochocientos fotogramas, asi que un milisegundo de
    /// mas por fotograma son dos segundos de espera mirando una barra.
    #[test]
    #[ignore]
    fn medir_lo_que_cuesta_vestir_un_fotograma() {
        use std::time::Instant;

        let fotogramas = 1800usize;
        // Un rastro de raton de un minuto a 30 fps, que es lo que se anota al grabar.
        let rastro: Vec<(u64, i32, i32)> = (0..fotogramas as u64)
            .map(|i| (i * 33, 200 + (i % 400) as i32, 150 + (i % 300) as i32))
            .collect();
        let clics: Vec<Clic> = (0..30)
            .map(|i| Clic {
                ms: i * 2000,
                x: 300,
                y: 200,
                derecho: false,
            })
            .collect();
        let ajustes = Ajustes {
            clics: true,
            teclas: false,
            cursor: 40.0,
        };
        let mut cache = pastilla::Cache::default();
        let mut imagen = RgbaImage::from_pixel(1280, 800, image::Rgba([30, 30, 30, 255]));

        let t = Instant::now();
        for f in 0..fotogramas {
            pintar(
                &mut imagen,
                f as u64 * 33,
                &clics,
                &[],
                &rastro,
                (1280, 800),
                &[],
                &ajustes,
                &mut cache,
            );
        }
        let vestir = t.elapsed();

        // Y lo que cuesta preguntar donde mira la camara, que es lo otro que se hace en
        // cada fotograma.
        let za = super::super::zoom::Ajustes::default();
        let zclics: Vec<super::super::zoom::Clic> = clics.clone();
        let tramos = super::super::zoom::tramos(&zclics, &za);
        let t = Instant::now();
        for f in 0..fotogramas {
            let _ = super::super::zoom::siguiendo(&tramos, &rastro, f as u64 * 33, 1280, 800, &za);
        }
        let camara = t.elapsed();

        eprintln!(
            "[estudio] {fotogramas} fotogramas: vestir {:?} ({:.3} ms/fotograma), camara {:?} ({:.3} ms/fotograma)",
            vestir,
            vestir.as_secs_f64() * 1000.0 / fotogramas as f64,
            camara,
            camara.as_secs_f64() * 1000.0 / fotogramas as f64,
        );
    }

    /// Lo que cuesta ESCALAR un fotograma, que es lo que el zoom obliga a hacer.
    ///
    /// Sin zoom, un fotograma que ya mide lo que se pide no se escala: pasa tal cual. Con
    /// zoom, cada fotograma se recorta a un trozo y hay que estirarlo al tamanno final, o
    /// sea que aparece un escalado que antes no existia. Aqui se mide cuanto cuesta con
    /// cada filtro, para elegir con datos y no a ojo.
    #[test]
    #[ignore]
    fn medir_lo_que_cuesta_escalar_un_fotograma() {
        use image::imageops::FilterType;
        use std::time::Instant;

        let trozo = RgbaImage::from_fn(640, 400, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
        });
        for (nombre, filtro) in [
            ("Lanczos3", FilterType::Lanczos3),
            ("CatmullRom", FilterType::CatmullRom),
            ("Triangle", FilterType::Triangle),
            ("Nearest", FilterType::Nearest),
        ] {
            let t = Instant::now();
            let veces = 20;
            for _ in 0..veces {
                let _ = image::imageops::resize(&trozo, 1280, 800, filtro);
            }
            let cada = t.elapsed().as_secs_f64() * 1000.0 / veces as f64;
            eprintln!("[escalar] 640x400 -> 1280x800 con {nombre}: {cada:.1} ms por fotograma");
        }
    }

    /// Lo que tarda de verdad una exportacion, con zoom y sin el.
    ///
    /// Las dos medidas de arriba miden piezas. Esta mide lo que espera el usuario mirando
    /// la barra, que es lo unico que le importa.
    #[test]
    #[ignore]
    fn medir_una_exportacion_entera() {
        use image::imageops::FilterType;
        use std::time::Instant;

        let fotogramas = 300usize;
        let fuente = RgbaImage::from_fn(1280, 800, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255])
        });

        // Sin zoom: el fotograma ya mide lo que se pide, asi que no se escala.
        let t = Instant::now();
        for _ in 0..fotogramas {
            let salida = fuente.clone();
            std::hint::black_box(salida);
        }
        let sin = t.elapsed();

        // Con zoom: cada fotograma se recorta a un trozo y se estira al tamanno final.
        // Por el camino de verdad, que es el que usa el exportador.
        let t = Instant::now();
        for i in 0..fotogramas {
            let x = (i % 200) as u32;
            let trozo = image::imageops::crop_imm(&fuente, x, 0, 640, 400).to_image();
            let salida = super::super::escalar::ampliar(&trozo, 1280, 800);
            std::hint::black_box(salida);
        }
        let con = t.elapsed();

        // Y lo que costaba antes, para que la diferencia quede escrita.
        let t = Instant::now();
        for i in 0..30 {
            let x = (i % 200) as u32;
            let trozo = image::imageops::crop_imm(&fuente, x, 0, 640, 400).to_image();
            std::hint::black_box(image::imageops::resize(&trozo, 1280, 800, FilterType::Lanczos3));
        }
        let antes = t.elapsed().as_secs_f64() * 1000.0 / 30.0;
        eprintln!("[exportar] con `image`, lo de antes: {antes:.0} ms/fotograma");

        eprintln!(
            "[exportar] {fotogramas} fotogramas de 1280x800: sin zoom {:?}, con zoom {:?} ({:.0} ms/fotograma)",
            sin,
            con,
            con.as_secs_f64() * 1000.0 / fotogramas as f64
        );
    }

    /// Exporta unos fotogramas de una grabacion DE VERDAD, con el zoom y el estudio puestos.
    ///
    /// Todo lo demas prueba piezas con imagenes inventadas. Esto abre una sesion que dejo
    /// una grabacion real, le aplica lo mismo que aplicaria el exportador, y deja los PNG
    /// en el temporal para mirarlos. Es la unica forma de ver si el zoom se acerca a donde
    /// tiene que acercarse, porque eso no lo dice ningun `assert`.
    ///
    /// Se le pasa la carpeta de la sesion por variable de entorno:
    ///
    /// ```text
    /// SESION=<carpeta> cargo test --release --lib ver_una_grabacion_de_verdad -- --ignored
    /// ```
    #[test]
    #[ignore]
    fn ver_una_grabacion_de_verdad() {
        use crate::encode::{escalar, zoom};
        use crate::record::{self, SessionData};

        let Ok(carpeta) = std::env::var("SESION") else {
            eprintln!("[ver] falta SESION=<carpeta de la sesion>");
            return;
        };
        let carpeta = std::path::PathBuf::from(carpeta);
        let crudo = std::fs::read_to_string(carpeta.join("session.json")).unwrap();
        let mut sesion: SessionData = serde_json::from_str(&crudo).unwrap();
        // El json guarda la carpeta de donde se grabo; aqui manda donde esta ahora.
        sesion.dir = carpeta.clone();

        eprintln!(
            "[ver] {} fotogramas, {} clics, {} teclas, {} posiciones de raton, region {}x{}",
            sesion.frames.len(),
            sesion.clics.len(),
            sesion.teclas.len(),
            sesion.cursor.len(),
            sesion.width,
            sesion.height
        );

        let za = zoom::Ajustes {
            escala: 2.0,
            ..zoom::Ajustes::default()
        };
        let tramos = zoom::tramos(&sesion.clics, &za);
        eprintln!("[ver] tramos de zoom: {tramos:?}");

        let ajustes = Ajustes {
            clics: true,
            teclas: true,
            cursor: 44.0,
        };
        let mut cache = pastilla::Cache::default();
        let destino = std::env::temp_dir().join("winshotx-ver-zoom");
        let _ = std::fs::remove_dir_all(&destino);
        std::fs::create_dir_all(&destino).unwrap();

        // Seis momentos repartidos por el primer tramo de zoom, para ver el acercamiento.
        let momentos: Vec<u64> = match tramos.first() {
            Some(t) => (0..6)
                .map(|i| t.desde_ms + (t.hasta_ms - t.desde_ms) * i / 5)
                .collect(),
            None => (0..6).map(|i| i * 1000).collect(),
        };

        for (n, ms) in momentos.iter().enumerate() {
            // El fotograma que toca en ese instante.
            let indice = sesion
                .frames
                .iter()
                .position(|f| f.timestamp_ms >= *ms)
                .unwrap_or(0);
            let imagen = record::read_frame(&sesion, indice).unwrap();
            let camara = zoom::siguiendo(&tramos, &sesion.cursor, *ms, sesion.width, sesion.height, &za);
            let recortes: Vec<Recorte> = if camara.escala > 1.001 {
                vec![camara.como_recorte(sesion.width, sesion.height)]
            } else {
                Vec::new()
            };
            let trozo = match recortes.first() {
                Some(r) => r.aplicar(&imagen),
                None => imagen,
            };
            let mut salida = escalar::ampliar(&trozo, sesion.width, sesion.height);
            pintar(
                &mut salida,
                *ms,
                &sesion.clics,
                &sesion.teclas,
                &sesion.cursor,
                (sesion.width, sesion.height),
                &recortes,
                &ajustes,
                &mut cache,
            );
            let ruta = destino.join(format!("{n}-{ms}ms-x{:.2}.png", camara.escala));
            salida.save(&ruta).unwrap();
            eprintln!("[ver] {ms} ms, escala {:.2} -> {}", camara.escala, ruta.display());
        }
        eprintln!("[ver] mira la carpeta {}", destino.display());
    }
}
