//! Buscar datos personales en un vídeo para taparlos (D4 del plan de CleanShot X).
//!
//! En una captura, «Tapar datos» lee la imagen con el lector de texto de Windows y
//! `sensibles` dice qué de lo leído es un correo, un teléfono, un DNI, una tarjeta o una
//! cuenta. En un vídeo eso mismo, hecho fotograma a fotograma, tardaría minutos: leer una
//! pantalla 1080p cuesta del orden de una décima de segundo, y un minuto de grabación con
//! algo moviéndose son cientos de fotogramas.
//!
//! **Solo se lee lo que cambia.** Una grabación ya guarda solo los fotogramas en los que
//! algo cambió (Windows no manda otro mientras la pantalla está quieta), y aquí se va más
//! lejos: de cada fotograma se compara con el último que se leyó y se leen solo las
//! franjas que han cambiado, a todo el ancho para no partir una línea. Lo que no ha
//! cambiado sigue diciendo lo mismo: si había un correo, sigue ahí.
//!
//! **Y nunca se tapa de menos por ir deprisa.** Mientras la pantalla cambia sin parar (un
//! vídeo, un scroll) no se lee cada fotograma, sino unos pocos por segundo; los de en medio
//! no se miran, y por eso lo que se encuentra se tapa también en ellos: un dato que aparece
//! se tapa desde el primer fotograma sin leer que pudo enseñarlo, y uno que se mueve se tapa
//! con el rectángulo que va de donde estaba a donde ha llegado. Tapar de más un instante no
//! le cuesta nada a nadie; dejar al aire un correo un instante, sí.
//!
//! Lo que sale son `Tapa` del contrato, que el editor enseña como bloques en la línea de
//! tiempo: se ven, se quitan y se alargan. **Aquí no se tapa nada**: eso lo hace el
//! exportador con lo que el usuario ha dejado (`encode::tapar`).

use image::RgbaImage;
use serde::Serialize;

use crate::contrato::Tapa;
use crate::error::Result;
use crate::record::{self, SessionData};
use crate::sensibles::Clase;

/// Un rectángulo en píxeles de la imagen grabada, con el final sin incluir.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caja {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Caja {
    fn area(&self) -> f32 {
        (self.x2 - self.x1).max(0.0) * (self.y2 - self.y1).max(0.0)
    }

    fn union(&self, otra: &Caja) -> Caja {
        Caja {
            x1: self.x1.min(otra.x1),
            y1: self.y1.min(otra.y1),
            x2: self.x2.max(otra.x2),
            y2: self.y2.max(otra.y2),
        }
    }

    /// Lo que se pisan, sobre lo que mide la más pequeña de las dos: 1 es que una está
    /// dentro de la otra. Sobre la pequeña y no sobre la unión porque el lector unas veces
    /// coge el punto final de un correo y otras no, y eso no lo convierte en otro dato.
    fn solape(&self, otra: &Caja) -> f32 {
        let dentro = Caja {
            x1: self.x1.max(otra.x1),
            y1: self.y1.max(otra.y1),
            x2: self.x2.min(otra.x2),
            y2: self.y2.min(otra.y2),
        };
        let menor = self.area().min(otra.area());
        if menor <= 0.0 { 0.0 } else { dentro.area() / menor }
    }
    /// Si es la misma caja con el temblor del lector: cada borde a menos de 3 píxeles, o de
    /// la quinta parte del alto si la letra es grande. Más que eso es que se ha movido.
    fn casi_igual(&self, otra: &Caja) -> bool {
        let alto = (self.y2 - self.y1).min(otra.y2 - otra.y1).max(0.0);
        let margen = (alto * 0.2).max(3.0);
        (self.x1 - otra.x1).abs() <= margen
            && (self.y1 - otra.y1).abs() <= margen
            && (self.x2 - otra.x2).abs() <= margen
            && (self.y2 - otra.y2).abs() <= margen
    }
}

/// Un dato encontrado en un fotograma: qué clase de dato es, lo que pone (para seguirlo si
/// se mueve) y dónde está, ya con el margen que lo tapa entero.
#[derive(Debug, Clone, PartialEq)]
pub struct Dato {
    pub clase: Clase,
    pub texto: String,
    pub caja: Caja,
}

/// Una franja de la imagen, a todo el ancho: de la fila `y1` a la `y2`, sin incluirla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Franja {
    pub y1: u32,
    pub y2: u32,
}

impl Franja {
    fn toca(&self, caja: &Caja) -> bool {
        caja.y1 < self.y2 as f32 && caja.y2 > self.y1 as f32
    }
}

/// Como mucho una lectura cada tanto mientras la pantalla cambia sin parar. Cuatro por
/// segundo: un dato que se queda en pantalla menos de esto no se lee a ojo, y lo que hay
/// entre dos lecturas se tapa igual (ver arriba).
const ENTRE_LECTURAS_MS: u64 = 250;

/// Y mientras un dato se mueve (un scroll), más a menudo: así el rectángulo que cubre su
/// camino entre dos lecturas es corto y no tapa media pantalla.
const ENTRE_LECTURAS_MOVIENDOSE_MS: u64 = 100;

/// Un fotograma que se queda en pantalla al menos esto se lee siempre: ahí se puede leer.
const QUIETO_MS: u32 = 300;

/// Lo que se añade por arriba y por abajo a cada franja que ha cambiado, para que la línea
/// entera entre en lo que se lee aunque solo haya cambiado el trozo de una letra.
const MARGEN_FRANJA: u32 = 24;

/// Dos franjas cambiadas a menos de esto se leen como una: cada lectura tiene su coste
/// fijo, y dos líneas seguidas suelen ser el mismo párrafo.
const JUNTAR_FRANJAS: u32 = 48;

/// Si lo que ha cambiado pasa de esta parte de la imagen, se lee la imagen entera.
const FRANJAS_COMO_ENTERA: f32 = 0.5;

/// Lo que puede durar un tramo «en movimiento» antes de cerrarse y empezar otro: su
/// rectángulo es el camino entero del dato en ese rato, y en un scroll largo no se quiere
/// una banda de media pantalla.
const TRAMO_EN_MOVIMIENTO_MS: u64 = 500;

/// Un dato que el lector deja de ver y vuelve a ver antes de esto sigue siendo el mismo
/// bloque: el lector de Windows a veces se salta una palabra en un fotograma, y sin esto
/// el dato quedaría al aire en ese hueco.
const PUENTE_MS: u64 = 1_000;

/// Cuánto se agranda la caja de un dato antes de taparlo, en veces su alto. Lo mismo que
/// en las capturas (`commands::MARGEN`): las colas y los acentos se salen de la caja que
/// da el lector.
const MARGEN_DATO: f32 = 0.18;

/// Lo que ha salido de buscar: las tapas, y lo que se cuenta al usuario.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatosEnVideo {
    pub tapas: Vec<Tapa>,
    /// Cuántos datos distintos de cada clase: «2 correos, 1 teléfono».
    pub resumen: Vec<(String, usize)>,
    /// Cuántas veces se ha pasado el lector, y sobre cuántos fotogramas.
    pub lecturas: usize,
    pub fotogramas: usize,
    pub ms: u64,
}

/// Lo que se está siguiendo: un dato que se ha visto y todavía no se ha ido.
#[derive(Debug, Clone)]
struct Activo {
    clase: Clase,
    texto: String,
    /// Lo que tapa: donde está, o el camino entero si se está moviendo.
    caja: Caja,
    /// Donde se vio la última vez, exacto.
    ultima: Caja,
    desde: u64,
    moviendose: bool,
}

/// Un tramo ya cerrado, en píxeles y con su dato.
#[derive(Debug, Clone)]
struct Hecho {
    clase: Clase,
    texto: String,
    caja: Caja,
    desde: u64,
    hasta: u64,
}

#[derive(Default)]
struct Rastreo {
    activos: Vec<Activo>,
    hechos: Vec<Hecho>,
}

impl Rastreo {
    fn cerrar(&mut self, a: Activo, hasta: u64) {
        if hasta > a.desde {
            self.hechos.push(Hecho { clase: a.clase, texto: a.texto, caja: a.caja, desde: a.desde, hasta });
        }
    }

    /// Junta lo que se acaba de leer en `leidas` con lo que se venía siguiendo.
    ///
    /// `ms` es el fotograma que se ha leído; `sin_mirar` es el primero que cambió después de
    /// la lectura anterior, y que no se miró: lo que aparece se tapa desde ahí.
    ///
    /// `sin_tocar` dice si los píxeles de una caja son los mismos que en la lectura
    /// anterior: si lo son, el dato sigue ahí aunque el lector no lo haya visto esta vez.
    fn juntar(
        &mut self,
        leidas: &[Franja],
        datos: Vec<Dato>,
        ms: u64,
        sin_mirar: u64,
        sin_tocar: &dyn Fn(&Caja) -> bool,
    ) {
        let mut usados = vec![false; datos.len()];
        let mut siguen = Vec::with_capacity(self.activos.len());
        for mut a in std::mem::take(&mut self.activos) {
            // Lo que no se ha vuelto a leer no ha cambiado: sigue ahí, igual.
            if !leidas.iter().any(|f| f.toca(&a.caja) || f.toca(&a.ultima)) {
                siguen.push(a);
                continue;
            }
            // En el mismo sitio, con el temblor del lector.
            let quieto = datos
                .iter()
                .enumerate()
                .position(|(j, d)| !usados[j] && d.clase == a.clase && d.caja.casi_igual(&a.ultima))
                // O en el mismo sitio leído distinto («examp1e»): sigue siendo el mismo
                // dato, salvo que sea él mismo movido, que se busca antes.
                .or_else(|| {
                    let mismo_texto = datos.iter().enumerate().any(|(j, d)| !usados[j] && d.texto == a.texto);
                    (!mismo_texto)
                        .then(|| {
                            datos.iter().enumerate().position(|(j, d)| {
                                !usados[j] && d.clase == a.clase && d.caja.solape(&a.ultima) >= 0.5
                            })
                        })
                        .flatten()
                });
            if let Some(j) = quieto {
                usados[j] = true;
                let caja = datos[j].caja;
                if a.moviendose {
                    // Se ha parado: lo que se movía acaba donde empezó lo que no se miró,
                    // y desde ahí el dato quieto, con su rectángulo justo.
                    let corte = sin_mirar.max(a.desde);
                    let ultima = a.ultima;
                    self.cerrar(a.clone(), corte);
                    siguen.push(Activo { caja: caja.union(&ultima), ultima: caja, desde: corte, moviendose: false, ..a });
                } else {
                    // El lector no da dos veces la misma caja al píxel: se tapa lo que cubren las dos.
                    a.caja = a.caja.union(&caja);
                    a.ultima = caja;
                    siguen.push(a);
                }
                continue;
            }
            // Lo mismo en otro sitio: se ha movido.
            let movido = datos
                .iter()
                .enumerate()
                .position(|(j, d)| !usados[j] && d.clase == a.clase && d.texto == a.texto);
            if let Some(j) = movido {
                usados[j] = true;
                let caja = datos[j].caja;
                if a.moviendose && ms.saturating_sub(a.desde) < TRAMO_EN_MOVIMIENTO_MS {
                    a.caja = a.caja.union(&caja);
                    a.ultima = caja;
                    siguen.push(a);
                } else {
                    // Lo de antes acaba donde empezó lo que no se miró, y el camino de ahí
                    // hasta aquí se tapa entero: en medio, el dato pasó por algún sitio.
                    let corte = sin_mirar.max(a.desde);
                    let camino = a.ultima.union(&caja);
                    self.cerrar(a.clone(), corte);
                    siguen.push(Activo { caja: camino, ultima: caja, desde: corte, moviendose: true, ..a });
                }
                continue;
            }
            // Si debajo no ha cambiado ni un píxel, sigue ahí: es el lector el que no lo ha
            // visto esta vez (le pasa al de Windows con una palabra de cada tanto).
            if !a.moviendose && sin_tocar(&a.ultima) {
                siguen.push(a);
                continue;
            }
            // Ya no está. Se tapa hasta el fotograma leído: en los de en medio, que no se
            // miraron, podía seguir.
            self.cerrar(a, ms);
        }
        for (j, d) in datos.into_iter().enumerate() {
            if !usados[j] {
                siguen.push(Activo {
                    clase: d.clase,
                    texto: d.texto,
                    caja: d.caja,
                    ultima: d.caja,
                    desde: sin_mirar,
                    moviendose: false,
                });
            }
        }
        self.activos = siguen;
    }

    /// Cierra lo que siga abierto al final y junta los tramos del mismo dato que se tocan.
    fn terminar(mut self, fin: u64) -> Vec<Hecho> {
        for a in std::mem::take(&mut self.activos) {
            self.cerrar(a, fin);
        }
        juntar_tramos(self.hechos)
    }
}

/// Junta los tramos del mismo dato en el mismo sitio que se pisan o casi se tocan: el
/// lector a veces no ve en un fotograma lo que sí ve en el anterior y en el siguiente, y
/// sin esto el hueco quedaría al aire y la línea de tiempo, llena de trocitos.
fn juntar_tramos(mut hechos: Vec<Hecho>) -> Vec<Hecho> {
    hechos.sort_by_key(|h| (h.desde, h.hasta));
    let mut salen: Vec<Hecho> = Vec::with_capacity(hechos.len());
    'fuera: for h in hechos {
        for s in salen.iter_mut().rev() {
            let cerca = h.desde <= s.hasta + PUENTE_MS && s.desde <= h.hasta + PUENTE_MS;
            if cerca && s.clase == h.clase && s.caja.solape(&h.caja) >= 0.5 {
                s.desde = s.desde.min(h.desde);
                s.hasta = s.hasta.max(h.hasta);
                s.caja = s.caja.union(&h.caja);
                continue 'fuera;
            }
        }
        salen.push(h);
    }
    salen
}

/// Si los píxeles de esa caja son los mismos en las dos imágenes.
fn caja_igual(antes: &RgbaImage, ahora: &RgbaImage, caja: &Caja) -> bool {
    let (ancho, alto) = ahora.dimensions();
    if antes.dimensions() != (ancho, alto) {
        return false;
    }
    let x1 = caja.x1.floor().clamp(0.0, ancho as f32) as usize;
    let x2 = caja.x2.ceil().clamp(0.0, ancho as f32) as usize;
    let y1 = caja.y1.floor().clamp(0.0, alto as f32) as u32;
    let y2 = caja.y2.ceil().clamp(0.0, alto as f32) as u32;
    let fila = ancho as usize * 4;
    (y1..y2).all(|y| {
        let i = y as usize * fila;
        antes.as_raw()[i + x1 * 4..i + x2 * 4] == ahora.as_raw()[i + x1 * 4..i + x2 * 4]
    })
}

/// Las franjas en que `ahora` es distinta de `antes`, ya con su margen y juntas. Vacío si
/// son iguales. Una sola a toda la altura si cambian de tamaño o si ha cambiado casi todo.
pub fn franjas_cambiadas(antes: &RgbaImage, ahora: &RgbaImage) -> Vec<Franja> {
    let (ancho, alto) = ahora.dimensions();
    let entera = vec![Franja { y1: 0, y2: alto }];
    if antes.dimensions() != (ancho, alto) {
        return entera;
    }
    let fila = ancho as usize * 4;
    let (a, b) = (antes.as_raw(), ahora.as_raw());
    let mut cambiadas: Vec<Franja> = Vec::new();
    for y in 0..alto {
        let i = y as usize * fila;
        if a[i..i + fila] != b[i..i + fila] {
            match cambiadas.last_mut() {
                Some(f) if y <= f.y2 + JUNTAR_FRANJAS => f.y2 = y + 1,
                _ => cambiadas.push(Franja { y1: y, y2: y + 1 }),
            }
        }
    }
    if cambiadas.is_empty() {
        return cambiadas;
    }
    let franjas = ensanchar(cambiadas, MARGEN_FRANJA, &[], alto);
    let alto_leido: u32 = franjas.iter().map(|f| f.y2 - f.y1).sum();
    if alto_leido as f32 > alto as f32 * FRANJAS_COMO_ENTERA { entera } else { franjas }
}

/// Les da a las franjas su margen y las estira para que cada dato que tocan entre entero
/// (un dato cortado por la mitad no se reconoce), y junta las que se pisan.
fn ensanchar(mut franjas: Vec<Franja>, margen: u32, cajas: &[Caja], alto: u32) -> Vec<Franja> {
    for f in franjas.iter_mut() {
        f.y1 = f.y1.saturating_sub(margen);
        f.y2 = (f.y2 + margen).min(alto);
    }
    loop {
        let mut cambia = false;
        for f in franjas.iter_mut() {
            for c in cajas {
                if !f.toca(c) {
                    continue;
                }
                let y1 = (c.y1.floor().max(0.0) as u32).saturating_sub(4);
                let y2 = ((c.y2.ceil() as u32) + 4).min(alto);
                if y1 < f.y1 || y2 > f.y2 {
                    f.y1 = f.y1.min(y1);
                    f.y2 = f.y2.max(y2);
                    cambia = true;
                }
            }
        }
        franjas.sort_by_key(|f| f.y1);
        let mut juntas: Vec<Franja> = Vec::with_capacity(franjas.len());
        for f in franjas {
            match juntas.last_mut() {
                Some(j) if f.y1 <= j.y2 => {
                    j.y2 = j.y2.max(f.y2);
                    cambia = true;
                }
                _ => juntas.push(f),
            }
        }
        franjas = juntas;
        if !cambia {
            return franjas;
        }
    }
}

/// Busca los datos de toda la sesión y devuelve las tapas que los cubren.
///
/// `leer` es el lector: recibe el fotograma y la franja que tiene que leer, y devuelve los
/// datos que hay en ella con sus cajas en píxeles del fotograma entero. En la app es el
/// lector de texto de Windows (`lector_de_windows`); en las pruebas, uno de mentira que
/// sabe dónde están.
pub fn buscar(
    session: &SessionData,
    leer: &mut dyn FnMut(&RgbaImage, Franja) -> Result<Vec<Dato>>,
    progreso: &mut dyn FnMut(usize, usize),
) -> Result<DatosEnVideo> {
    let empieza = std::time::Instant::now();
    let frames = &session.frames;
    let total = frames.len();
    let mut lector = record::LectorPorDelante::nuevo(session.clone(), (0..total).collect());
    let mut rastreo = Rastreo::default();
    let mut ultima: Option<RgbaImage> = None;
    let mut ultima_ms = 0u64;
    let mut sin_mirar: Option<u64> = None;
    let mut lecturas = 0usize;
    let (mut ancho, mut alto) = (session.width.max(1), session.height.max(1));

    for (i, frame) in frames.iter().enumerate() {
        let imagen = lector.en(i)?;
        progreso(i + 1, total);
        let ms = frame.timestamp_ms;
        let cambiadas = match &ultima {
            None => vec![Franja { y1: 0, y2: imagen.height() }],
            Some(antes) => franjas_cambiadas(antes, &imagen),
        };
        if cambiadas.is_empty() {
            continue;
        }
        let desde = *sin_mirar.get_or_insert(ms);
        let moviendose = rastreo.activos.iter().any(|a| a.moviendose);
        let espera = if moviendose { ENTRE_LECTURAS_MOVIENDOSE_MS } else { ENTRE_LECTURAS_MS };
        let leer_ahora = ultima.is_none()
            || ms.saturating_sub(ultima_ms) >= espera
            || frame.duration_ms >= QUIETO_MS
            || i + 1 == total;
        if !leer_ahora {
            continue;
        }
        (ancho, alto) = imagen.dimensions();
        // Lo que se pisa con un dato que se viene siguiendo se relee con el dato entero dentro.
        let cajas: Vec<Caja> = rastreo.activos.iter().flat_map(|a| [a.caja, a.ultima]).collect();
        let franjas = if cambiadas.len() == 1 && cambiadas[0].y1 == 0 && cambiadas[0].y2 == alto {
            cambiadas
        } else {
            ensanchar(cambiadas, 0, &cajas, alto)
        };
        let mut datos = Vec::new();
        for f in &franjas {
            datos.extend(leer(&imagen, *f)?);
            lecturas += 1;
        }
        let sin_tocar = |c: &Caja| ultima.as_ref().is_some_and(|antes| caja_igual(antes, &imagen, c));
        rastreo.juntar(&franjas, datos, ms, desde, &sin_tocar);
        ultima = Some(imagen);
        ultima_ms = ms;
        sin_mirar = None;
    }

    let fin = frames.last().map_or(0, |f| f.timestamp_ms + u64::from(f.duration_ms.max(1)));
    let hechos = rastreo.terminar(fin);

    // Cuántos datos distintos de cada clase: el mismo correo en dos sitios es un correo.
    let mut vistos: Vec<(Clase, String)> = Vec::new();
    let mut resumen: Vec<(String, usize)> = Vec::new();
    for h in &hechos {
        if vistos.iter().any(|(c, t)| *c == h.clase && *t == h.texto) {
            continue;
        }
        vistos.push((h.clase, h.texto.clone()));
        let nombre = h.clase.nombre().to_string();
        match resumen.iter_mut().find(|(n, _)| *n == nombre) {
            Some((_, cuantos)) => *cuantos += 1,
            None => resumen.push((nombre, 1)),
        }
    }
    let (w, h) = (ancho as f32, alto as f32);
    let tapas = hechos
        .iter()
        .map(|t| {
            let x1 = (t.caja.x1 / w).clamp(0.0, 1.0);
            let y1 = (t.caja.y1 / h).clamp(0.0, 1.0);
            Tapa {
                desde_ms: t.desde,
                hasta_ms: t.hasta,
                x: x1,
                y: y1,
                ancho: (t.caja.x2 / w).clamp(0.0, 1.0) - x1,
                alto: (t.caja.y2 / h).clamp(0.0, 1.0) - y1,
            }
        })
        .filter(|t| t.ancho > 0.0 && t.alto > 0.0)
        .collect();
    Ok(DatosEnVideo { tapas, resumen, lecturas, fotogramas: total, ms: empieza.elapsed().as_millis() as u64 })
}

/// Los datos de una línea leída, con su caja agrandada para taparlos enteros. Lo mismo que
/// hace `commands::caja_de` en las capturas, en píxeles y con lo que pone.
pub fn datos_de_linea(linea: &[crate::sensibles::Palabra], desplazar_y: f32, escala: f32) -> Vec<Dato> {
    crate::sensibles::buscar_en_linea(linea)
        .into_iter()
        .filter_map(|hallazgo| {
            let partes: Vec<&crate::sensibles::Palabra> =
                hallazgo.palabras.iter().filter_map(|i| linea.get(*i)).collect();
            let primera = partes.first()?;
            let mut caja = Caja {
                x1: primera.x,
                y1: primera.y,
                x2: primera.x + primera.ancho,
                y2: primera.y + primera.alto,
            };
            for p in &partes[1..] {
                caja = caja.union(&Caja { x1: p.x, y1: p.y, x2: p.x + p.ancho, y2: p.y + p.alto });
            }
            let margen = (caja.y2 - caja.y1) * MARGEN_DATO;
            // Lo que pone, para reconocerlo si se mueve: junto, en minúsculas y sin la
            // puntuación de los bordes, que el lector unas veces trae y otras no.
            let junto: String = partes.iter().map(|p| p.texto.to_lowercase()).collect();
            let texto = junto.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
            Some(Dato {
                clase: hallazgo.clase,
                texto,
                caja: Caja {
                    x1: (caja.x1 - margen) * escala,
                    y1: (caja.y1 - margen) * escala + desplazar_y,
                    x2: (caja.x2 + margen) * escala,
                    y2: (caja.y2 + margen) * escala + desplazar_y,
                },
            })
        })
        .collect()
}

/// El lector de verdad: el de texto de Windows, creado una vez para toda la búsqueda.
///
/// Se le pasa la franja en gris y en crudo, sin pasar por PNG: con cientos de lecturas, lo
/// de comprimir y descomprimir cada una se notaba más que leerla.
#[cfg(windows)]
pub fn lector_de_windows() -> Result<impl FnMut(&RgbaImage, Franja) -> Result<Vec<Dato>>> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    let motor = crate::platform::ocr::motor()?;
    let maximo = OcrEngine::MaxImageDimension().unwrap_or(2_600).max(64);
    Ok(move |imagen: &RgbaImage, franja: Franja| -> Result<Vec<Dato>> {
        let ancho = imagen.width();
        let y2 = franja.y2.min(imagen.height());
        if y2 <= franja.y1 || ancho == 0 {
            return Ok(Vec::new());
        }
        let trozo = image::imageops::crop_imm(imagen, 0, franja.y1, ancho, y2 - franja.y1).to_image();
        // El lector no acepta imágenes de más de `maximo` de lado: una pantalla 4K se lee
        // a menos tamaño, que a esa resolución las letras siguen siendo grandes.
        let lado = trozo.width().max(trozo.height());
        let (trozo, escala) = if lado > maximo {
            let f = maximo as f32 / lado as f32;
            let w = ((trozo.width() as f32 * f) as u32).max(1);
            let h = ((trozo.height() as f32 * f) as u32).max(1);
            (image::imageops::resize(&trozo, w, h, image::imageops::FilterType::Triangle), 1.0 / f)
        } else {
            (trozo, 1.0)
        };
        let gris: Vec<u8> = trozo
            .pixels()
            .map(|p| ((p.0[0] as u32 * 299 + p.0[1] as u32 * 587 + p.0[2] as u32 * 114) / 1000) as u8)
            .collect();
        let escritor = DataWriter::new()?;
        escritor.WriteBytes(&gris)?;
        let buffer = escritor.DetachBuffer()?;
        let mapa = SoftwareBitmap::CreateCopyFromBuffer(
            &buffer,
            BitmapPixelFormat::Gray8,
            trozo.width() as i32,
            trozo.height() as i32,
        )?;
        let resultado = motor.RecognizeAsync(&mapa)?.join()?;
        let mut datos = Vec::new();
        for linea in resultado.Lines()? {
            let mut palabras = Vec::new();
            for palabra in linea.Words()? {
                let caja = palabra.BoundingRect()?;
                palabras.push(crate::sensibles::Palabra {
                    texto: palabra.Text()?.to_string_lossy(),
                    x: caja.X,
                    y: caja.Y,
                    ancho: caja.Width,
                    alto: caja.Height,
                });
            }
            datos.extend(datos_de_linea(&palabras, franja.y1 as f32, escala));
        }
        Ok(datos)
    })
}

/// Lo que se va haciendo, para el editor: cuántos fotogramas mirados de cuántos.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progreso {
    hechos: usize,
    total: usize,
}

/// El aviso de progreso de la búsqueda. Su espejo está en `src/lib/ipc.ts`.
pub const AVISO_PROGRESO: &str = "winshotx://tapar-video";

/// Busca los datos personales de una grabación entera y devuelve dónde y cuándo taparlos.
///
/// **No tapa nada por su cuenta**, igual que `tapar_datos` en una captura: devuelve las
/// tapas y las pone el editor en la línea de tiempo, donde se ven y se quitan.
#[tauri::command]
pub async fn tapar_datos_video(app: tauri::AppHandle, session_id: String) -> Result<DatosEnVideo> {
    use tauri::Manager;
    let session = app
        .state::<crate::state::AppState>()
        .sessions
        .read()
        .get(&session_id)
        .cloned()
        .ok_or_else(|| crate::error::AppError::UnknownSession(session_id.clone()))?;
    #[cfg(windows)]
    {
        use tauri::Emitter;
        let mut leer = lector_de_windows()?;
        let mut ultimo_aviso = std::time::Instant::now();
        let mut avisar = |hechos: usize, total: usize| {
            if hechos == total || ultimo_aviso.elapsed().as_millis() >= 120 {
                ultimo_aviso = std::time::Instant::now();
                let _ = app.emit(AVISO_PROGRESO, Progreso { hechos, total });
            }
        };
        buscar(&session, &mut leer, &mut avisar)
    }
    #[cfg(not(windows))]
    {
        let _ = session;
        Err(crate::error::AppError::Unsupported)
    }
}

#[cfg(test)]
#[path = "datos_en_video_pruebas.rs"]
pub(crate) mod pruebas;
