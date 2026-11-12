//! La webcam en el Estudio (F2): la cámara que se grabó aparte (`camara.mp4`, ver
//! `record::camara`) encima del vídeo exportado, con su forma, su recorte, su tamaño y su
//! sitio (`contrato::MarcoCamara`).
//!
//! **Va encima de todo lo vestido**: después del recorte, del zoom, del estudio, de las
//! marcas y del marco. Es una persona hablando delante del vídeo, no un trozo de la pantalla
//! grabada, así que ni el zoom la acerca ni el marco la encoge.
//!
//! **El tiempo es el de la grabación.** Cada paso del exportador cae en un instante de la
//! grabación (`Pasos::tiempos`), y la cámara de ese paso es la de ese instante, con la regla
//! de S2 (`record::camara::desfase`): el fotograma de `timestamp_ms` lleva la imagen de
//! `timestamp_ms - frames[0].timestamp_ms - desfase_ms`. Así avanza con el vídeo también con
//! los cortes y los trozos acelerados: un corte se salta lo mismo en la cara que en la
//! pantalla, y un trozo al doble mueve la cara al doble.
//!
//! **Nunca entera en memoria.** `camara.mp4` se lee de fotograma en fotograma con el lector
//! de Media Foundation de los fondos animados (`fondo_video::abrir`), en otro hilo y un
//! fotograma por delante, y solo se guarda la burbuja ya recortada, escalada y con su forma.
//!
//! Antes de que la cámara arranque (el desfase) se ve su primera imagen, y cuando se acaba,
//! la última: una burbuja que aparece de golpe a los 300 ms se nota más que una cara quieta.
//!
//! Las medidas (`caja`, `recorte_visible`, `de_fabrica`) son las mismas que usa la vista
//! previa del editor, en `src/lib/camara.ts`, con las mismas pruebas a los dos lados.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use image::RgbaImage;
use serde::Serialize;

use super::fondo_video;
use super::recorte::Recorte;
use crate::contrato::{Forma, MarcoCamara};
use crate::error::Result;
use crate::record::SessionData;

/// El lado mayor más pequeño que se acepta, de 0 a 1 sobre el ancho: menos es un punto.
const TAM_MINIMO: f32 = 0.05;
/// Un recorte más estrecho que esto no deja ver una cara: se toma la imagen entera.
const RECORTE_MINIMO: f32 = 0.02;
/// El radio de las esquinas del rectángulo redondeado, sobre su lado corto. El mismo que el
/// de la lupa (`lupa::pintar`), para que las dos formas redondeadas se vean de la misma
/// familia. Desde S13 es solo lo que lleva un `Redondeado` de antes, sin `redondeo`.
const RADIO_REDONDEADO: f32 = 0.18;
/// Lo más redondas que pueden ir las esquinas (S13): media vuelta del lado corto, que
/// deja el cuadrado en círculo y el rectángulo en pastilla. Más ya no redondea nada.
pub const REDONDEO_MAXIMO: f32 = 0.5;
/// Lo que se ve de fábrica: pequeña, en la esquina de abajo a la derecha, sin tapar el
/// centro. El lado mayor es el 18 % del ancho (345 px en un vídeo de 1920).
pub const TAM_DE_FABRICA: f32 = 0.18;
/// Y el aire que la separa del borde, sobre el ancho (58 px en 1920).
pub const AIRE_DE_FABRICA: f32 = 0.03;

/// Una caja en píxeles del vídeo exportado: la esquina de arriba a la izquierda y lo que mide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caja {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

/// El recorte con sus esquinas ordenadas y dentro de la imagen. Uno roto (con un lado
/// casi nulo o sin número) vale la imagen entera: mejor la cara entera que ninguna.
fn recorte_limpio(r: &Recorte) -> (f32, f32, f32, f32) {
    let numeros = [r.x1, r.y1, r.x2, r.y2];
    if numeros.iter().any(|v| !v.is_finite()) {
        return (0.0, 0.0, 1.0, 1.0);
    }
    let (x1, x2) = (
        r.x1.min(r.x2).clamp(0.0, 1.0),
        r.x1.max(r.x2).clamp(0.0, 1.0),
    );
    let (y1, y2) = (
        r.y1.min(r.y2).clamp(0.0, 1.0),
        r.y1.max(r.y2).clamp(0.0, 1.0),
    );
    if x2 - x1 < RECORTE_MINIMO || y2 - y1 < RECORTE_MINIMO {
        return (0.0, 0.0, 1.0, 1.0);
    }
    (x1, y1, x2, y2)
}

/// Ancho entre alto de la burbuja. El círculo y el cuadrado son cuadrados; los dos
/// rectángulos tienen la proporción del trozo de cámara que se enseña, así que recortar
/// la cámara es también darle forma al rectángulo.
pub fn proporcion(marco: &MarcoCamara, camara: (u32, u32)) -> f32 {
    match marco.forma {
        Forma::Circulo | Forma::Cuadrado => 1.0,
        Forma::Rectangulo | Forma::Redondeado => {
            let (x1, y1, x2, y2) = recorte_limpio(&marco.recorte);
            let ancho = (x2 - x1) * camara.0.max(1) as f32;
            let alto = (y2 - y1) * camara.1.max(1) as f32;
            (ancho / alto.max(f32::EPSILON)).clamp(0.2, 5.0)
        }
    }
}

/// Dónde cae la burbuja en un vídeo exportado de `lienzo` píxeles.
///
/// Su lado mayor es `tam` por el ancho del vídeo, y nunca más grande que el vídeo. Su centro
/// es `x` e `y`, pero metido para que la burbuja entera quede dentro: arrastrada contra un
/// borde, se queda pegada a él en vez de salirse.
pub fn caja(marco: &MarcoCamara, camara: (u32, u32), lienzo: (u32, u32)) -> Caja {
    let (lw, lh) = (lienzo.0.max(1) as f32, lienzo.1.max(1) as f32);
    let tam = if marco.tam.is_finite() {
        marco.tam.clamp(TAM_MINIMO, 1.0)
    } else {
        TAM_DE_FABRICA
    };
    let lado = tam * lw;
    let r = proporcion(marco, camara);
    let (mut ancho, mut alto) = if r >= 1.0 {
        (lado, lado / r)
    } else {
        (lado * r, lado)
    };
    let encoge = (lw / ancho).min(lh / alto).min(1.0);
    ancho *= encoge;
    alto *= encoge;
    let centro = |v: f32, defecto: f32| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            defecto
        }
    };
    let cx = (centro(marco.x, 0.85) * lw).clamp(ancho / 2.0, lw - ancho / 2.0);
    let cy = (centro(marco.y, 0.8) * lh).clamp(alto / 2.0, lh - alto / 2.0);
    Caja {
        x: cx - ancho / 2.0,
        y: cy - alto / 2.0,
        ancho,
        alto,
    }
}

/// El trozo de la cámara que se ve dentro de la burbuja, de 0 a 1: el recorte, centrado y
/// rebajado por el lado que sobre para llenar la burbuja sin deformar la cara. En los
/// rectángulos es el recorte tal cual; en el círculo, lo más grande y cuadrado que cabe en él.
pub fn recorte_visible(marco: &MarcoCamara, camara: (u32, u32)) -> Recorte {
    recorte_para(&marco.recorte, proporcion(marco, camara), camara)
}

fn recorte_para(recorte: &Recorte, proporcion: f32, camara: (u32, u32)) -> Recorte {
    let (x1, y1, x2, y2) = recorte_limpio(recorte);
    let (cw, ch) = (camara.0.max(1) as f32, camara.1.max(1) as f32);
    let (ancho, alto) = ((x2 - x1) * cw, (y2 - y1) * ch);
    if ancho / alto > proporcion {
        // Sobra a lo ancho: se queda el centro.
        let nuevo = alto * proporcion / cw;
        let sobra = (x2 - x1 - nuevo) / 2.0;
        Recorte {
            x1: x1 + sobra,
            y1,
            x2: x2 - sobra,
            y2,
        }
    } else {
        let nuevo = ancho / proporcion / ch;
        let sobra = (y2 - y1 - nuevo) / 2.0;
        Recorte {
            x1,
            y1: y1 + sobra,
            x2,
            y2: y2 - sobra,
        }
    }
}

/// El grosor del borde blanco, en píxeles: un 2 % del lado corto de la burbuja, entre 2 y 8.
pub fn grosor_del_borde(caja: &Caja) -> f32 {
    (caja.ancho.min(caja.alto) * 0.02).clamp(2.0, 8.0)
}

/// Lo redondas que van las esquinas, sobre el lado corto (S13). El círculo, siempre media
/// vuelta. El cuadrado y el rectángulo, lo que se eligió con «Esquinas redondeadas»; y sin
/// elegir (lo de antes de S13), rectas, salvo el `Redondeado`, que va a su 0,18 de siempre.
pub fn redondeo(marco: &MarcoCamara) -> f32 {
    if marco.forma == Forma::Circulo {
        return REDONDEO_MAXIMO;
    }
    match marco.redondeo {
        Some(r) if r.is_finite() => r.clamp(0.0, REDONDEO_MAXIMO),
        _ if marco.forma == Forma::Redondeado => RADIO_REDONDEADO,
        _ => 0.0,
    }
}

/// El radio de las esquinas de la burbuja, en píxeles.
pub fn radio(marco: &MarcoCamara, caja: &Caja) -> f32 {
    caja.ancho.min(caja.alto) * redondeo(marco)
}

/// La cámara como sale de fábrica en un vídeo de `ancho` por `alto`: en círculo, sin
/// recortar, sin borde, pequeña y en la esquina de abajo a la derecha, con el mismo aire
/// por abajo que por la derecha.
pub fn de_fabrica(ancho: u32, alto: u32) -> MarcoCamara {
    let (w, h) = (ancho.max(1) as f32, alto.max(1) as f32);
    let medio = TAM_DE_FABRICA / 2.0;
    MarcoCamara {
        forma: Forma::Circulo,
        recorte: Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        },
        tam: TAM_DE_FABRICA,
        x: 1.0 - AIRE_DE_FABRICA - medio,
        y: (1.0 - (AIRE_DE_FABRICA + medio) * w / h).clamp(0.0, 1.0),
        borde: false,
        redondeo: None,
    }
}

/// En qué instante de `camara.mp4` está la cámara cuando la grabación está en `ms` (en el
/// reloj de los fotogramas). Negativo: la cámara todavía no había arrancado.
pub fn instante(ms: u64, primer_fotograma_ms: u64, desfase_ms: i64) -> i64 {
    ms as i64 - primer_fotograma_ms as i64 - desfase_ms
}

/// Si una petición pinta la cámara: la pide y la sesión la grabó. Con ella encima no hay
/// dos instantes iguales (la cara se mueve aunque la pantalla no), así que el exportador no
/// junta pasos.
pub fn hay(session: &SessionData, marco: Option<&MarcoCamara>) -> bool {
    marco.is_some() && archivo(session).is_some()
}

/// El archivo de la cámara de una sesión, si se grabó y sigue en el disco.
fn archivo(session: &SessionData) -> Option<PathBuf> {
    let grabada = session.camara.as_ref()?;
    // Solo el nombre, nunca una ruta: lo que hay en `session.json` no sale de su carpeta.
    let nombre = Path::new(&grabada.archivo).file_name()?;
    let ruta = session.dir.join(nombre);
    ruta.is_file().then_some(ruta)
}

// --- Para el editor ---

/// Lo que necesita la vista previa del editor para enseñar la cámara.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CamaraDeLaSesion {
    /// La ruta entera de `camara.mp4`, para el protocolo `asset`.
    pub ruta: String,
    pub ancho: u32,
    pub alto: u32,
    pub desfase_ms: i64,
    /// El instante del primer fotograma de la pantalla: la cámara cuenta desde él.
    pub primer_ms: u64,
}

/// La cámara de una sesión tal como la pide el editor, o `None` si no se grabó.
pub fn de_la_sesion(session: &SessionData) -> Option<CamaraDeLaSesion> {
    let grabada = session.camara.as_ref()?;
    let ruta = archivo(session)?;
    Some(CamaraDeLaSesion {
        ruta: ruta.to_string_lossy().to_string(),
        ancho: grabada.ancho,
        alto: grabada.alto,
        desfase_ms: grabada.desfase_ms,
        primer_ms: session.frames.first().map_or(0, |f| f.timestamp_ms),
    })
}

/// La cámara de esa sesión, para la vista previa y los mandos del editor. `null` si la
/// grabación no la trae (o su archivo ya no está).
#[tauri::command]
pub async fn camara_de_la_sesion(
    app: tauri::AppHandle,
    session_id: String,
) -> Result<Option<CamaraDeLaSesion>> {
    use tauri::Manager;
    let state = app.state::<crate::state::AppState>();
    let en_memoria = state.sessions.read().get(&session_id).cloned();
    let session = match en_memoria {
        Some(s) => s,
        // La sesión puede seguir en el disco aunque la app se haya reiniciado.
        None => {
            let ruta = state.session_dir(&session_id).join("session.json");
            match std::fs::read_to_string(ruta) {
                Ok(texto) => serde_json::from_str(&texto)?,
                Err(_) => return Ok(None),
            }
        }
    };
    Ok(de_la_sesion(&session))
}

// --- Pintarla ---

/// La distancia con signo al borde de una caja de esquinas redondeadas centrada en el
/// origen: negativa dentro, positiva fuera. La misma cuenta que la lupa.
fn distancia(x: f32, y: f32, medio_ancho: f32, medio_alto: f32, radio: f32) -> f32 {
    let qx = x.abs() - (medio_ancho - radio);
    let qy = y.abs() - (medio_alto - radio);
    let fuera = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    fuera + qx.max(qy).min(0.0) - radio
}

/// Píxeles de más alrededor de la burbuja, para el borde suavizado y la raya de fuera.
const MARGEN: u32 = 2;

/// La forma de la burbuja, calculada una vez por exportación: qué parte de cada píxel es
/// cámara, cuánto es borde blanco y cuánto tapa de lo de debajo.
struct Plantilla {
    /// Dónde cae la esquina de la plantilla en el vídeo exportado (con su margen).
    x0: i32,
    y0: i32,
    /// Lo que mide la plantilla, margen incluido.
    ancho: u32,
    alto: u32,
    /// Lo que mide la burbuja, que es a lo que se escala la cámara.
    burbuja: (u32, u32),
    /// Por píxel: cuánto de la cámara, cuánto de blanco (ya multiplicado por 255) y cuánto
    /// tapa en total. El color final es `cámara * k + blanco + debajo * (1 - tapa)`.
    factores: Vec<[f32; 3]>,
}

impl Plantilla {
    fn nueva(caja: &Caja, marco: &MarcoCamara) -> Option<Self> {
        let borde = marco.borde;
        let (bx, by) = (caja.x.round() as i32, caja.y.round() as i32);
        let (bw, bh) = (caja.ancho.round() as u32, caja.alto.round() as u32);
        if bw < 4 || bh < 4 {
            return None;
        }
        let redonda = Caja {
            x: bx as f32,
            y: by as f32,
            ancho: bw as f32,
            alto: bh as f32,
        };
        let radio = radio(marco, &redonda);
        let grosor = if borde {
            grosor_del_borde(&redonda)
        } else {
            0.0
        };
        let (ancho, alto) = (bw + MARGEN * 2, bh + MARGEN * 2);
        let (mx, my) = (bw as f32 / 2.0, bh as f32 / 2.0);
        let mut factores = Vec::with_capacity((ancho * alto) as usize);
        for py in 0..alto {
            for px in 0..ancho {
                let dx = px as f32 + 0.5 - MARGEN as f32 - mx;
                let dy = py as f32 + 0.5 - MARGEN as f32 - my;
                let d = distancia(dx, dy, mx, my, radio);
                let dentro = (0.5 - d).clamp(0.0, 1.0);
                // Con borde: un disco blanco y la cámara dentro de él, un poco más pequeña.
                // Así el filo de fuera es blanco sobre lo de debajo, sin cara asomando.
                let (blanco, camara) = if borde {
                    (dentro, (0.5 - (d + grosor)).clamp(0.0, 1.0))
                } else {
                    (0.0, dentro)
                };
                // La raya oscura de un píxel por fuera del borde, como la de la lupa: separa
                // el blanco de un fondo blanco.
                let raya = if borde {
                    ((1.5 - d).clamp(0.0, 1.0) - dentro).max(0.0) * 0.32
                } else {
                    0.0
                };
                let k = camara * (1.0 - raya);
                let b = 255.0 * blanco * (1.0 - camara) * (1.0 - raya);
                let tapa = raya + (camara + blanco * (1.0 - camara)) * (1.0 - raya);
                factores.push([k, b, tapa]);
            }
        }
        Some(Self {
            x0: bx - MARGEN as i32,
            y0: by - MARGEN as i32,
            ancho,
            alto,
            burbuja: (bw, bh),
            factores,
        })
    }

    /// La burbuja de un fotograma de la cámara: recortado, escalado y con la forma puesta.
    /// Sale con el color ya multiplicado por lo que tapa, que es lo que se suma al pintar.
    fn burbuja(&self, fotograma: &RgbaImage, recorte: &Recorte, proporcion: f32) -> RgbaImage {
        let visible = recorte_para(recorte, proporcion, fotograma.dimensions());
        let (x, y, w, h) = visible.en_pixeles(fotograma.width(), fotograma.height());
        let trozo = image::imageops::crop_imm(fotograma, x, y, w, h).to_image();
        let cara = super::escalar::a_medida(&trozo, self.burbuja.0, self.burbuja.1);
        let mut salida = RgbaImage::new(self.ancho, self.alto);
        for (i, p) in salida.pixels_mut().enumerate() {
            let [k, b, tapa] = self.factores[i];
            if tapa <= 0.0 {
                continue;
            }
            let (px, py) = (i as u32 % self.ancho, i as u32 / self.ancho);
            let (cx, cy) = (px.wrapping_sub(MARGEN), py.wrapping_sub(MARGEN));
            let fuente = if k > 0.0 && cx < cara.width() && cy < cara.height() {
                cara.get_pixel(cx, cy).0
            } else {
                [0, 0, 0, 255]
            };
            let canal = |c: u8| (c as f32 * k + b).round().clamp(0.0, 255.0) as u8;
            *p = image::Rgba([
                canal(fuente[0]),
                canal(fuente[1]),
                canal(fuente[2]),
                (tapa * 255.0).round().clamp(0.0, 255.0) as u8,
            ]);
        }
        salida
    }
}

/// Pone una burbuja (con el color ya multiplicado) encima de la imagen, en su sitio.
fn componer(imagen: &mut RgbaImage, burbuja: &RgbaImage, x0: i32, y0: i32) {
    let (iw, ih) = (imagen.width() as i32, imagen.height() as i32);
    for (bx, by, p) in burbuja.enumerate_pixels() {
        let tapa = p.0[3];
        if tapa == 0 {
            continue;
        }
        let (x, y) = (x0 + bx as i32, y0 + by as i32);
        if x < 0 || y < 0 || x >= iw || y >= ih {
            continue;
        }
        let debajo = imagen.get_pixel_mut(x as u32, y as u32);
        let queda = 255 - u32::from(tapa);
        for c in 0..3 {
            let v = u32::from(p.0[c]) + (u32::from(debajo.0[c]) * queda + 127) / 255;
            debajo.0[c] = v.min(255) as u8;
        }
        debajo.0[3] = 255;
    }
}

/// Lo que contesta el hilo que lee la cámara: la burbuja de ese instante, o por qué no.
type Respuesta = std::result::Result<Arc<RgbaImage>, String>;

/// La webcam de una exportación: un hilo que lee `camara.mp4` por delante y contesta con la
/// burbuja de cada instante, ya lista para ponerla encima.
///
/// Se le pide en orden. Hacia atrás (la paleta de un GIF, otra pasada) vuelve a empezar el
/// archivo, que solo se lee hacia delante. El hilo se acaba cuando se suelta esto.
pub struct Webcam {
    pide: mpsc::Sender<u64>,
    contesta: mpsc::Receiver<Respuesta>,
    x0: i32,
    y0: i32,
    primer_ms: u64,
    desfase_ms: i64,
    /// Si ya falló una vez: entonces el resto del vídeo sale sin cámara, sin volver a probar
    /// en cada fotograma.
    rota: bool,
}

impl Webcam {
    /// La webcam de una exportación a un vídeo de `lienzo` píxeles, o `None` si no hay que
    /// pintarla (no se pidió, la sesión no la trae, o no se ve de tan pequeña).
    pub fn nueva(
        session: &SessionData,
        marco: Option<&MarcoCamara>,
        lienzo: (u32, u32),
    ) -> Option<Self> {
        let marco = *marco?;
        let grabada = session.camara.as_ref()?;
        let Some(ruta) = archivo(session) else {
            eprintln!("[winshotx] la sesión dice que grabó la cámara, pero su archivo no está: sale sin ella");
            return None;
        };
        let camara = (grabada.ancho.max(1), grabada.alto.max(1));
        let caja = caja(&marco, camara, lienzo);
        let plantilla = Plantilla::nueva(&caja, &marco)?;
        let (x0, y0) = (plantilla.x0, plantilla.y0);
        let proporcion = proporcion(&marco, camara);
        let (pide, peticiones) = mpsc::channel::<u64>();
        let (respuestas, contesta) = mpsc::channel::<Respuesta>();
        std::thread::Builder::new()
            .name("webcam-del-estudio".into())
            .spawn(move || {
                let mut lector = Lectura::new(ruta);
                while let Ok(ms) = peticiones.recv() {
                    let respuesta = lector
                        .en(ms)
                        .map(|f| f.burbuja(&plantilla, &marco.recorte, proporcion))
                        .map_err(|e| e.to_string());
                    let fallo = respuesta.is_err();
                    if respuestas.send(respuesta).is_err() || fallo {
                        break;
                    }
                    // El siguiente, mientras quien pidió viste y codifica el suyo.
                    lector.adelantar();
                }
            })
            .ok()?;
        Some(Self {
            pide,
            contesta,
            x0,
            y0,
            primer_ms: session.frames.first().map_or(0, |f| f.timestamp_ms),
            desfase_ms: grabada.desfase_ms,
            rota: false,
        })
    }

    /// Pinta la cámara del instante `ms` de la grabación encima de `imagen`, que es el
    /// fotograma ya vestido del vídeo exportado. Si la cámara no se puede leer, lo dice una
    /// vez y deja el vídeo sin ella: perder la cara es mejor que perder la exportación.
    pub fn pintar(&mut self, imagen: &mut RgbaImage, ms: u64) {
        if self.rota {
            return;
        }
        let cuando = instante(ms, self.primer_ms, self.desfase_ms).max(0) as u64;
        let respuesta = self
            .pide
            .send(cuando)
            .ok()
            .and_then(|_| self.contesta.recv_timeout(Duration::from_secs(60)).ok());
        match respuesta {
            Some(Ok(burbuja)) => componer(imagen, &burbuja, self.x0, self.y0),
            Some(Err(motivo)) => {
                eprintln!(
                    "[winshotx] la cámara no se puede leer ({motivo}): el vídeo sale sin ella"
                );
                self.rota = true;
            }
            None => {
                eprintln!("[winshotx] el hilo de la cámara no contesta: el vídeo sale sin ella");
                self.rota = true;
            }
        }
    }
}

/// Un fotograma de la cámara ya leído, y su burbuja si ya se pidió.
struct Leido {
    desde_ms: u64,
    imagen: RgbaImage,
    burbuja: Option<Arc<RgbaImage>>,
}

impl Leido {
    fn burbuja(
        &mut self,
        plantilla: &Plantilla,
        recorte: &Recorte,
        proporcion: f32,
    ) -> Arc<RgbaImage> {
        self.burbuja
            .get_or_insert_with(|| Arc::new(plantilla.burbuja(&self.imagen, recorte, proporcion)))
            .clone()
    }
}

/// Lo que vive dentro del hilo: el lector y por dónde va.
struct Lectura {
    ruta: PathBuf,
    lector: Option<Box<dyn fondo_video::Lector>>,
    /// El que se está viendo.
    actual: Option<Leido>,
    /// Si el que se está viendo es el primero del archivo: antes de él no hay nada, así que
    /// pedir un instante anterior no obliga a volver a empezar.
    es_el_primero: bool,
    /// El siguiente, ya leído.
    siguiente: Option<Leido>,
    /// El archivo ya no da más.
    acabado: bool,
}

impl Lectura {
    fn new(ruta: PathBuf) -> Self {
        Self {
            ruta,
            lector: None,
            actual: None,
            es_el_primero: false,
            siguiente: None,
            acabado: false,
        }
    }

    fn leer(&mut self) -> Result<Option<Leido>> {
        if self.acabado {
            return Ok(None);
        }
        if self.lector.is_none() {
            self.lector = Some(fondo_video::abrir(&self.ruta)?);
        }
        let lector = self.lector.as_mut().expect("recién abierto");
        match lector.siguiente()? {
            Some(c) => Ok(Some(Leido {
                desde_ms: c.desde_ms,
                imagen: c.imagen,
                burbuja: None,
            })),
            None => {
                self.acabado = true;
                // Se suelta ya: el lector tiene Media Foundation encendido y el archivo abierto.
                self.lector = None;
                Ok(None)
            }
        }
    }

    /// El fotograma que se ve en el instante `ms` de la cámara: el último que empezó antes.
    fn en(&mut self, ms: u64) -> Result<&mut Leido> {
        if self
            .actual
            .as_ref()
            .is_some_and(|a| ms < a.desde_ms && !self.es_el_primero)
        {
            *self = Self::new(std::mem::take(&mut self.ruta));
        }
        loop {
            if self.siguiente.is_none() {
                self.siguiente = self.leer()?;
            }
            let entra = match (&self.actual, &self.siguiente) {
                (None, Some(_)) => true,
                (Some(_), Some(s)) => s.desde_ms <= ms,
                (_, None) => false,
            };
            if !entra {
                break;
            }
            self.es_el_primero = self.actual.is_none();
            self.actual = self.siguiente.take();
        }
        self.actual.as_mut().ok_or_else(|| {
            crate::error::AppError::Msg("el archivo de la cámara no tiene ni una imagen".into())
        })
    }

    /// Lee el siguiente si todavía no está: es lo que se hace mientras se pinta el de ahora.
    fn adelantar(&mut self) {
        if self.siguiente.is_none() {
            if let Ok(s) = self.leer() {
                self.siguiente = s;
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn marco(forma: Forma, tam: f32, x: f32, y: f32) -> MarcoCamara {
        MarcoCamara {
            forma,
            recorte: Recorte {
                x1: 0.0,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
            tam,
            x,
            y,
            borde: false,
            redondeo: None,
        }
    }

    /// Los mismos números que `src/lib/camara.test.ts`: si cambia uno, cambia el otro.
    #[test]
    fn la_caja_de_cada_forma() {
        let lienzo = (1920, 1080);
        let camara = (1280, 720);
        // El círculo de fábrica: 345,6 px, a 58 px del borde de la derecha y del de abajo.
        let c = caja(&de_fabrica(1920, 1080), camara, lienzo);
        assert!(
            (c.ancho - 345.6).abs() < 0.01 && (c.alto - 345.6).abs() < 0.01,
            "{c:?}"
        );
        assert!((1920.0 - (c.x + c.ancho) - 57.6).abs() < 0.01, "{c:?}");
        assert!((1080.0 - (c.y + c.alto) - 57.6).abs() < 0.01, "{c:?}");
        // El rectángulo sin recortar tiene la proporción de la cámara.
        let r = caja(&marco(Forma::Rectangulo, 0.25, 0.5, 0.5), camara, lienzo);
        assert!(
            (r.ancho - 480.0).abs() < 0.01 && (r.alto - 270.0).abs() < 0.01,
            "{r:?}"
        );
        // Recortada a la mitad izquierda, la proporción es la de esa mitad (640 x 720).
        let mut m = marco(Forma::Redondeado, 0.25, 0.5, 0.5);
        m.recorte = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 0.5,
            y2: 1.0,
        };
        let r = caja(&m, camara, lienzo);
        assert!(
            (r.alto - 480.0).abs() < 0.01 && (r.ancho - 426.67).abs() < 0.01,
            "{r:?}"
        );
    }

    /// Arrastrada contra un borde se queda dentro, y nunca mide más que el vídeo.
    #[test]
    fn la_caja_no_se_sale() {
        let c = caja(
            &marco(Forma::Cuadrado, 0.2, 1.0, 0.0),
            (640, 480),
            (1000, 500),
        );
        assert_eq!((c.x, c.y, c.ancho, c.alto), (800.0, 0.0, 200.0, 200.0));
        let enorme = caja(
            &marco(Forma::Circulo, 1.0, 0.5, 0.5),
            (640, 480),
            (1000, 500),
        );
        assert_eq!((enorme.ancho, enorme.alto), (500.0, 500.0));
        let roto = caja(
            &marco(Forma::Circulo, f32::NAN, f32::NAN, f32::INFINITY),
            (640, 480),
            (1000, 500),
        );
        assert!(
            roto.ancho > 0.0 && roto.x >= 0.0 && roto.y + roto.alto <= 500.0,
            "{roto:?}"
        );
    }

    /// El círculo enseña el centro cuadrado del recorte; el rectángulo, el recorte entero.
    #[test]
    fn lo_que_se_ve_de_la_camara() {
        let v = recorte_visible(&marco(Forma::Circulo, 0.2, 0.5, 0.5), (1280, 720));
        assert!(
            (v.x1 - 0.21875).abs() < 1e-4 && (v.x2 - 0.78125).abs() < 1e-4,
            "{v:?}"
        );
        assert_eq!((v.y1, v.y2), (0.0, 1.0));
        let mut m = marco(Forma::Rectangulo, 0.2, 0.5, 0.5);
        m.recorte = Recorte {
            x1: 0.1,
            y1: 0.2,
            x2: 0.6,
            y2: 0.9,
        };
        let v = recorte_visible(&m, (1280, 720));
        assert!(
            (v.x1 - 0.1).abs() < 1e-5 && (v.y2 - 0.9).abs() < 1e-5,
            "{v:?}"
        );
        // Un recorte sin anchura no deja la burbuja vacía: vale la imagen entera.
        m.recorte = Recorte {
            x1: 0.4,
            y1: 0.0,
            x2: 0.4,
            y2: 1.0,
        };
        assert_eq!(proporcion(&m, (1280, 720)), 1280.0 / 720.0);
    }

    /// S13 (redondeo). Los mismos números que `src/lib/camara.test.ts`: en una burbuja de
    /// 400 x 300, el círculo es media vuelta siempre; el cuadrado y el rectángulo, lo que se
    /// eligió; y sin elegir, lo de antes (rectas, y el `Redondeado` a su 0,18).
    #[test]
    fn el_radio_de_cada_forma() {
        let c = Caja {
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 300.0,
        };
        let r = |forma: Forma, redondeo: Option<f32>| {
            radio(
                &MarcoCamara {
                    redondeo,
                    ..marco(forma, 0.2, 0.5, 0.5)
                },
                &c,
            )
        };
        assert_eq!(r(Forma::Circulo, None), 150.0);
        assert_eq!(r(Forma::Circulo, Some(0.0)), 150.0, "el círculo no se cuadra");
        assert_eq!(r(Forma::Cuadrado, None), 0.0);
        assert_eq!(r(Forma::Rectangulo, None), 0.0);
        assert!((r(Forma::Redondeado, None) - 54.0).abs() < 1e-3);
        assert_eq!(r(Forma::Cuadrado, Some(0.25)), 75.0);
        assert_eq!(r(Forma::Rectangulo, Some(0.5)), 150.0);
        assert_eq!(r(Forma::Rectangulo, Some(0.9)), 150.0, "más de media vuelta no hay");
        assert_eq!(r(Forma::Rectangulo, Some(-1.0)), 0.0);
        assert_eq!(r(Forma::Rectangulo, Some(f32::NAN)), 0.0);
        assert_eq!(r(Forma::Redondeado, Some(0.0)), 0.0, "lo elegido manda");
        assert!((r(Forma::Redondeado, Some(f32::NAN)) - 54.0).abs() < 1e-3);
    }

    /// El desfase: positivo es que la cámara arrancó tarde, y entonces va por detrás.
    #[test]
    fn el_instante_de_la_camara() {
        assert_eq!(instante(1_000, 0, 0), 1_000);
        assert_eq!(instante(1_000, 0, 300), 700);
        assert_eq!(instante(100, 0, 300), -200);
        assert_eq!(instante(1_050, 50, -100), 1_100);
    }

    /// La plantilla: el círculo deja las esquinas de su caja sin tocar, con borde el filo es
    /// blanco, y el centro es todo cámara.
    #[test]
    fn la_forma_y_el_borde() {
        let caja = Caja {
            x: 10.0,
            y: 10.0,
            ancho: 100.0,
            alto: 100.0,
        };
        let gris = image::Rgba([90, 90, 90, 255]);
        let rojo = RgbaImage::from_pixel(64, 64, image::Rgba([230, 20, 20, 255]));
        let completo = Recorte {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let pintar_con = |forma: Forma, borde: bool, redondeo: Option<f32>| {
            let m = MarcoCamara {
                borde,
                redondeo,
                ..marco(forma, 0.2, 0.5, 0.5)
            };
            let p = Plantilla::nueva(&caja, &m).unwrap();
            let burbuja = p.burbuja(&rojo, &completo, 1.0);
            let mut lienzo = RgbaImage::from_pixel(140, 140, gris);
            componer(&mut lienzo, &burbuja, p.x0, p.y0);
            lienzo
        };
        let pintar = |forma: Forma, borde: bool| pintar_con(forma, borde, None);
        let circulo = pintar(Forma::Circulo, false);
        assert_eq!(
            circulo.get_pixel(12, 12),
            &gris,
            "la esquina de la caja del círculo no se toca"
        );
        assert_eq!(
            circulo.get_pixel(60, 60).0,
            [230, 20, 20, 255],
            "el centro es la cámara"
        );
        assert_eq!(circulo.get_pixel(5, 60), &gris, "fuera no se toca");
        let cuadrado = pintar(Forma::Cuadrado, false);
        assert_eq!(
            cuadrado.get_pixel(12, 12).0,
            [230, 20, 20, 255],
            "el cuadrado llega a la esquina"
        );
        let con_borde = pintar(Forma::Cuadrado, true);
        assert_eq!(
            con_borde.get_pixel(11, 60).0,
            [255, 255, 255, 255],
            "con borde, el filo es blanco"
        );
        assert_eq!(con_borde.get_pixel(60, 60).0, [230, 20, 20, 255]);
        let redondeado = pintar(Forma::Redondeado, false);
        assert_eq!(
            redondeado.get_pixel(11, 11),
            &gris,
            "el redondeado no llega a la esquina"
        );
        assert_eq!(
            redondeado.get_pixel(20, 20).0,
            [230, 20, 20, 255],
            "pero casi"
        );
        // S13 (redondeo): el cuadrado con sus esquinas redondeadas tampoco llega, y del
        // todo redondas se queda como el círculo.
        let suave = pintar_con(Forma::Cuadrado, false, Some(0.1));
        assert_eq!(suave.get_pixel(11, 11), &gris, "redondeado al 10 %, la esquina no");
        assert_eq!(suave.get_pixel(20, 20).0, [230, 20, 20, 255]);
        let redondo = pintar_con(Forma::Cuadrado, true, Some(0.5));
        assert_eq!(
            redondo.get_pixel(12, 12),
            &gris,
            "del todo redondo, como el círculo"
        );
        assert_eq!(
            redondo.get_pixel(11, 60).0,
            [255, 255, 255, 255],
            "y el borde sigue la curva"
        );
    }

    /// Pintada en una esquina, la plantilla se sale del lienzo y no revienta.
    #[test]
    fn pegada_al_borde_no_revienta() {
        let caja = Caja {
            x: -1.0,
            y: 30.0,
            ancho: 40.0,
            alto: 40.0,
        };
        let m = MarcoCamara {
            borde: true,
            ..marco(Forma::Circulo, 0.2, 0.5, 0.5)
        };
        let p = Plantilla::nueva(&caja, &m).unwrap();
        let burbuja = p.burbuja(
            &RgbaImage::new(32, 32),
            &Recorte {
                x1: 0.0,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
            1.0,
        );
        let mut lienzo = RgbaImage::new(60, 60);
        componer(&mut lienzo, &burbuja, p.x0, p.y0);
    }
}
