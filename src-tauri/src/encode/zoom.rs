//! El zoom que se acerca solo a donde se hizo clic.
//!
//! Es lo que hace que una grabación de pantalla se entienda: en un monitor de 1920 el botón
//! que se pulsa mide veinte píxeles, y quien mira el vídeo no sabe dónde mirar. Acercarse a
//! ese punto durante un segundo y volver es la diferencia entre un vídeo y un tutorial.
//!
//! **Todo se calcula al exportar, sobre los fotogramas que ya están en disco.** Durante la
//! grabación solo se anota dónde y cuándo se pulsó. De ahí salen tres cosas:
//!
//! 1. **Cero megabytes de instalador y cero milisegundos de arranque.** Es aritmética.
//! 2. **Se puede cambiar de idea después de grabar**: subir el zoom, bajarlo o quitarlo no
//!    obliga a repetir la grabación.
//! 3. No hace falta ningún enganche de teclado ni de ratón.
//!
//! La cámara nunca se sale de la imagen: al pulsar en una esquina se acerca a esa esquina,
//! no a un trozo de negro que no existe.

use serde::{Deserialize, Serialize};

/// Un clic, en píxeles de la región grabada y milisegundos desde el principio.
///
/// Es lo único que se anota mientras se graba, y de aquí sale todo el estudio: el zoom se
/// acerca a estos puntos y los aros se dibujan encima de ellos. Doce bytes y un booleano.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Clic {
    pub ms: u64,
    pub x: i32,
    pub y: i32,
    /// El botón derecho va de otro color: abrir un menú no es pulsar un botón. Al zoom le
    /// da igual cuál fue, pero el aro que se dibuja después sí lo necesita.
    #[serde(default)]
    pub derecho: bool,
}

/// Cómo se comporta el zoom. Lo elige quien exporta, no quien graba.
#[derive(Debug, Clone, Copy)]
pub struct Ajustes {
    /// Cuánto se acerca. 2,0 es el doble de grande.
    pub escala: f32,
    /// Lo que tarda en acercarse y en alejarse.
    pub transicion_ms: u64,
    /// Lo que se queda quieto después del último clic del grupo.
    pub quieto_ms: u64,
    /// Cuánto pasado del ratón se promedia para seguirlo.
    ///
    /// Más alto es más suave y más retrasado; más bajo, más pegado y más nervioso. Medio
    /// segundo deja la cámara acompañando sin dar tirones.
    pub seguir_ms: u64,
    /// La inercia del centro: lo que tarda la cámara en recorrer dos tercios de lo que le
    /// falta hasta donde tiene que mirar. Es lo que convierte un cambio de sitio en un
    /// desplazamiento y no en un salto.
    pub inercia_ms: u64,
    /// Y el tope: cuánto puede moverse el centro por segundo, en anchos de imagen. Con
    /// 1,3, cruzar la pantalla entera cuesta como poco tres cuartos de segundo, que es lo
    /// que se sigue con la vista sin marearse.
    pub velocidad_max: f32,
}

impl Ajustes {
    /// Los de siempre a otra velocidad: con 2,0 se acerca y se aleja en la mitad de tiempo
    /// y sigue al ratón el doble de ligero; con 0,5, al revés. Lo que se queda quieto
    /// después del último clic no cambia: eso es cuánto se mira, no lo deprisa que se va.
    pub fn a_velocidad(escala: f32, velocidad: f32) -> Self {
        let v = if velocidad.is_finite() { velocidad.clamp(0.4, 2.5) } else { 1.0 };
        let base = Self::default();
        Self {
            escala,
            transicion_ms: (base.transicion_ms as f32 / v).round() as u64,
            inercia_ms: (base.inercia_ms as f32 / v).round() as u64,
            velocidad_max: base.velocidad_max * v,
            ..base
        }
    }
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            escala: 1.8,
            transicion_ms: 450,
            quieto_ms: 1200,
            seguir_ms: 500,
            inercia_ms: 220,
            velocidad_max: 1.3,
        }
    }
}

/// El centro de la cámara yendo hacia donde tiene que mirar, con inercia.
///
/// `siguiendo` dice a dónde debería mirar la cámara en cada instante; esto es lo que hace
/// que llegue **sin saltos**. Un clic a la izquierda y otro a la derecha ponían el objetivo
/// en la otra punta de la pantalla y la cámara cruzaba en un par de fotogramas: con el zoom
/// puesto, eso es un latigazo. Munir, 12 de septiembre de 2026: «es muy brusco cuando al
/// hacer zoom en clics te vas de un lado a otro».
///
/// Va con estado y se le pregunta **en orden**, fotograma a fotograma: en cada paso cierra
/// una parte de la distancia que le falta (un muelle amortiguado) y nunca más deprisa que
/// la velocidad máxima. Los dos que la usan, el exportador y la cámara muestreada para la
/// vista previa, recorren los fotogramas en orden, así que los dos ven el mismo camino.
#[derive(Debug, Default, Clone)]
pub struct Seguimiento {
    /// Dónde estaba el centro la última vez, y cuándo.
    ultimo: Option<(f32, f32, u64)>,
}

impl Seguimiento {
    /// La cámara que se enseña en este instante, partiendo de donde estaba la vez anterior.
    ///
    /// Con la imagen entera no hay centro que arrastrar: se olvida lo anterior, y así el
    /// siguiente acercamiento arranca centrado en su clic y no viniendo de otro sitio.
    pub fn avanzar(&mut self, objetivo: Camara, ms: u64, ancho: u32, ajustes: &Ajustes) -> Camara {
        if objetivo.escala <= 1.001 {
            self.ultimo = None;
            return objetivo;
        }
        let (tx, ty) = (objetivo.x as f32, objetivo.y as f32);
        let (px, py) = match self.ultimo {
            Some((x, y, t)) if ms >= t => {
                let dt = (ms - t) as f32;
                let parte = 1.0 - (-dt / ajustes.inercia_ms.max(1) as f32).exp();
                let mut dx = (tx - x) * parte;
                let mut dy = (ty - y) * parte;
                let tope = ajustes.velocidad_max * ancho.max(1) as f32 * dt / 1000.0;
                let paso = (dx * dx + dy * dy).sqrt();
                if paso > tope && paso > 0.0 {
                    dx *= tope / paso;
                    dy *= tope / paso;
                }
                (x + dx, y + dy)
            }
            // La primera vez, o si se ha vuelto hacia atrás: donde toque, sin recorrido.
            _ => (tx, ty),
        };
        self.ultimo = Some((px, py, ms));
        Camara {
            x: px.round() as i32,
            y: py.round() as i32,
            escala: objetivo.escala,
        }
    }

    /// La cámara justo donde se le pide, sin recorrido: la de un zoom fijo, que no se mueve
    /// de su punto. Lo apunta como sitio de partida, para que lo que venga detrás salga de
    /// ahí y no de donde estaba antes del tramo fijo.
    pub fn colocar(&mut self, objetivo: Camara, ms: u64) -> Camara {
        self.ultimo = (objetivo.escala > 1.001).then_some((objetivo.x as f32, objetivo.y as f32, ms));
        objetivo
    }
}

/// Un rato en el que la cámara está acercada a un sitio.
///
/// Sale de agrupar los clics cercanos en el tiempo: acercarse y alejarse por cada clic de un
/// doble clic, o mientras alguien rellena un formulario, marea más que ayuda.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tramo {
    /// Cuándo empieza a acercarse.
    pub desde_ms: u64,
    /// Cuándo termina de alejarse.
    pub hasta_ms: u64,
    /// El primer clic del grupo, que es cuando ya tiene que estar cerca.
    pub cerca_ms: u64,
    /// El último clic del grupo.
    pub fin_cerca_ms: u64,
    /// Cuándo empieza a alejarse: hasta aquí se queda quieto. En los de los clics es el
    /// último clic más el rato quieto; en los puestos a mano, el final del bloque menos lo
    /// que tarda en alejarse.
    pub lejos_ms: u64,
    pub x: i32,
    pub y: i32,
    /// Cuánto se acerca este tramo: 2 es el doble. Los de los clics llevan todos el del
    /// panel; cada bloque de la pista lleva el suyo.
    pub escala: f32,
    /// Anclado en `(x, y)`: la cámara no sigue al ratón y el centro no se mueve (A2).
    pub fijo: bool,
    /// Entra y sale de golpe, sin acercarse poco a poco.
    pub instantaneo: bool,
}

/// Un zoom de la pista del editor, ya en píxeles de la imagen sobre la que se mide la
/// cámara (la grabada, o el trozo que recortó el usuario).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomAMano {
    pub desde_ms: u64,
    pub hasta_ms: u64,
    pub escala: f32,
    pub x: i32,
    pub y: i32,
    pub fijo: bool,
    pub instantaneo: bool,
}

/// El tope de lo que se acerca un zoom: más allá, un píxel grabado ocupa una baldosa.
pub const ESCALA_MAXIMA: f32 = 4.0;

/// Los tramos de los zooms puestos a mano.
///
/// **El bloque es el rato entero en que la cámara no enseña la imagen entera**: se acerca
/// al principio del bloque, se queda, y se aleja antes de que acabe. Así lo que se ve en la
/// pista es lo que pasa en el vídeo, y un bloque hecho con un tramo de los clics (que
/// empieza a acercarse antes del clic y acaba de alejarse después) vuelve a dar ese mismo
/// tramo.
///
/// Van en orden y sin pisarse: si dos se pisan, el segundo empieza donde acaba el primero,
/// y uno que se queda sin largo, o que no acerca nada, no cuenta.
pub fn tramos_a_mano(zooms: &[ZoomAMano], ajustes: &Ajustes) -> Vec<Tramo> {
    let mut orden: Vec<ZoomAMano> = zooms.to_vec();
    orden.sort_by_key(|z| (z.desde_ms, z.hasta_ms));
    let mut salida: Vec<Tramo> = Vec::with_capacity(orden.len());
    for z in orden {
        let desde = salida.last().map_or(z.desde_ms, |t| z.desde_ms.max(t.hasta_ms));
        let escala = if z.escala.is_finite() { z.escala.min(ESCALA_MAXIMA) } else { 1.0 };
        if z.hasta_ms <= desde || escala < 1.01 {
            continue;
        }
        // En un bloque corto, acercarse y alejarse se comen como mucho un tercio cada uno:
        // siempre queda un rato quieto encima, que es para lo que se pone.
        let transicion = ajustes.transicion_ms.min((z.hasta_ms - desde) / 3);
        salida.push(Tramo {
            desde_ms: desde,
            hasta_ms: z.hasta_ms,
            cerca_ms: desde + transicion,
            fin_cerca_ms: desde + transicion,
            lejos_ms: z.hasta_ms - transicion,
            x: z.x,
            y: z.y,
            escala,
            fijo: z.fijo,
            instantaneo: z.instantaneo,
        });
    }
    salida
}

/// El tramo que hay en ese milisegundo, si hay alguno.
pub fn tramo_en(tramos: &[Tramo], ms: u64) -> Option<&Tramo> {
    tramos.iter().find(|t| ms >= t.desde_ms && ms <= t.hasta_ms)
}

/// Dónde mira la cámara en un instante.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camara {
    pub x: i32,
    pub y: i32,
    /// 1,0 es la imagen entera.
    pub escala: f32,
}

impl Camara {
    /// La imagen entera, sin acercarse a nada.
    pub fn entera(ancho: u32, alto: u32) -> Self {
        Self {
            x: ancho as i32 / 2,
            y: alto as i32 / 2,
            escala: 1.0,
        }
    }

    /// El trozo que hay que recortar: `(x, y, ancho, alto)`, siempre dentro de la imagen.
    ///
    /// **El centro se corrige antes de recortar, no después.** Un clic pegado al borde
    /// derecho pide una ventana que se sale, y lo correcto es enseñar la esquina de verdad,
    /// no un trozo con la mitad en negro.
    pub fn recorte(&self, ancho: u32, alto: u32) -> (u32, u32, u32, u32) {
        let escala = self.escala.max(1.0);
        let w = ((ancho as f32 / escala).round() as u32).clamp(2, ancho);
        let h = ((alto as f32 / escala).round() as u32).clamp(2, alto);
        let x = (self.x - w as i32 / 2).clamp(0, (ancho - w) as i32) as u32;
        let y = (self.y - h as i32 / 2).clamp(0, (alto - h) as i32) as u32;
        (x, y, w, h)
    }

    /// Lo mismo, pero como recorte de 0 a 1.
    ///
    /// Asi el zoom entra por el camino que ya existe: recortar la imagen, volver a medir
    /// las anotaciones sobre el trozo y escalar. Una flecha dibujada encima entra y sale
    /// de cuadro con la camara, que es lo que tiene que pasar.
    pub fn como_recorte(&self, ancho: u32, alto: u32) -> super::recorte::Recorte {
        let (x, y, w, h) = self.recorte(ancho, alto);
        super::recorte::Recorte {
            x1: x as f32 / ancho as f32,
            y1: y as f32 / alto as f32,
            x2: (x + w) as f32 / ancho as f32,
            y2: (y + h) as f32 / alto as f32,
        }
    }
}

/// Agrupa los clics en tramos de zoom.
///
/// Dos clics entran en el mismo tramo si el segundo llega antes de que el primero haya
/// **terminado del todo**, alejamiento incluido. Antes el corte estaba al acabar el rato
/// quieto, y un clic que caía durante el alejamiento abría un tramo nuevo que se pisaba con
/// el que se estaba cerrando: `camara` cogía el primero hasta que acababa y de golpe pasaba
/// al segundo, ya acercado en otro sitio. Un corte seco en mitad del vídeo.
///
/// El centro es el del PRIMER clic del grupo y no la media: la media de dos esquinas
/// opuestas cae en el medio de la pantalla, que es donde no ha pasado nada. A partir de ahí
/// la cámara sigue al ratón, con inercia (`Seguimiento`).
pub fn tramos(clics: &[Clic], ajustes: &Ajustes) -> Vec<Tramo> {
    let mut salida: Vec<Tramo> = Vec::new();
    for clic in clics {
        match salida.last_mut() {
            // Sigue dentro del anterior, o empezaria a acercarse antes de que el anterior
            // acabe de alejarse: se alarga en vez de abrir otro que se pisaria con el.
            Some(ultimo) if clic.ms <= ultimo.hasta_ms + ajustes.transicion_ms => {
                ultimo.fin_cerca_ms = clic.ms;
                ultimo.lejos_ms = clic.ms + ajustes.quieto_ms;
                ultimo.hasta_ms = clic.ms + ajustes.quieto_ms + ajustes.transicion_ms;
            }
            _ => salida.push(Tramo {
                desde_ms: clic.ms.saturating_sub(ajustes.transicion_ms),
                cerca_ms: clic.ms,
                fin_cerca_ms: clic.ms,
                lejos_ms: clic.ms + ajustes.quieto_ms,
                hasta_ms: clic.ms + ajustes.quieto_ms + ajustes.transicion_ms,
                x: clic.x,
                y: clic.y,
                escala: ajustes.escala,
                fijo: false,
                instantaneo: false,
            }),
        }
    }
    salida
}

/// Una rampa que empieza y acaba parada, de 0 a 1.
///
/// Con una recta se nota el tirón al empezar y al terminar, y el zoom parece un salto en vez
/// de un movimiento. Es la misma curva que usa cualquier animación que se ve bien.
fn suave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Dónde mira la cámara en ese milisegundo, **siguiendo al ratón**.
///
/// Con la cámara acercada, quedarse clavada en el punto del clic deja al ratón saliéndose
/// del cuadro en cuanto se mueve un poco. Siguiéndolo, la vista acompaña a lo que se está
/// haciendo, que es lo que hace que se entienda.
///
/// **La cámara va por detrás.** El centro no salta a donde está el ratón, se acerca a él un
/// poco en cada fotograma: así un movimiento nervioso no zarandea la imagen, y un
/// movimiento largo se sigue igual. Sin ese retraso, el vídeo marea.
pub fn siguiendo(
    tramos: &[Tramo],
    rastro: &[(u64, i32, i32)],
    ms: u64,
    ancho: u32,
    alto: u32,
    ajustes: &Ajustes,
) -> Camara {
    let base = camara(tramos, ms, ancho, alto, ajustes);
    // Un zoom fijo no mira el ratón: se queda en su punto el tramo entero.
    let fijo = tramo_en(tramos, ms).is_some_and(|t| t.fijo);
    if base.escala <= 1.001 || rastro.is_empty() || fijo {
        return base;
    }
    // El ratón, suavizado: la media de donde ha estado en el último tramo de tiempo, con
    // más peso en lo reciente. Es lo que convierte un temblor en un movimiento.
    let desde = ms.saturating_sub(ajustes.seguir_ms);
    let (mut sx, mut sy, mut peso_total) = (0.0f32, 0.0f32, 0.0f32);
    // El rastro va en orden de tiempo, asi que la ventana se encuentra con dos busquedas
    // binarias en vez de recorrerlo entero. Recorrerlo se notaba: esto se pregunta una
    // vez por fotograma y el rastro tiene un punto por fotograma, o sea que una grabacion
    // de diez minutos eran trescientos millones de comparaciones para exportar, y otras
    // tantas cada vez que la vista previa mueve el deslizador del zoom.
    let inicio = rastro.partition_point(|(t, _, _)| *t < desde);
    let fin = rastro.partition_point(|(t, _, _)| *t <= ms);
    for (t, x, y) in &rastro[inicio..fin.max(inicio)] {
        // De 0 en el punto más antiguo a 1 en el más reciente.
        let cercania = (*t - desde) as f32 / ajustes.seguir_ms.max(1) as f32;
        let peso = 0.15 + cercania * cercania;
        sx += *x as f32 * peso;
        sy += *y as f32 * peso;
        peso_total += peso;
    }
    if peso_total <= 0.0 {
        return base;
    }
    Camara {
        x: (sx / peso_total).round() as i32,
        y: (sy / peso_total).round() as i32,
        escala: base.escala,
    }
}

/// Dónde mira la cámara en ese milisegundo, sin mirar el ratón.
///
/// Los ajustes ya no hacen falta aquí: cada tramo lleva su nivel y sus tiempos, porque los
/// de la pista los pone cada uno. Se siguen pidiendo para no cambiar a quien la llama.
pub fn camara(tramos: &[Tramo], ms: u64, ancho: u32, alto: u32, _ajustes: &Ajustes) -> Camara {
    let entera = Camara::entera(ancho, alto);
    let Some(tramo) = tramo_en(tramos, ms) else {
        return entera;
    };

    let cerca = |cuanto: f32| Camara {
        x: tramo.x,
        y: tramo.y,
        escala: 1.0 + (tramo.escala - 1.0) * cuanto,
    };

    if tramo.instantaneo {
        // De golpe: cerca del todo desde el primer milisegundo del tramo hasta el último.
        cerca(1.0)
    } else if ms < tramo.cerca_ms {
        // Acercándose.
        let dentro = (ms - tramo.desde_ms) as f32;
        let total = (tramo.cerca_ms - tramo.desde_ms).max(1) as f32;
        cerca(suave(dentro / total))
    } else if ms <= tramo.lejos_ms {
        // Quieto encima.
        cerca(1.0)
    } else {
        // Alejándose.
        let empieza = tramo.lejos_ms;
        let dentro = (ms - empieza) as f32;
        let total = (tramo.hasta_ms - empieza).max(1) as f32;
        cerca(1.0 - suave(dentro / total))
    }
}

// --- S7 (cursor): acercarse al escribir (A5) y la estela de la cámara (A4) ---

/// Los zooms de acercarse mientras se escribe (A5), como bloques de la pista.
///
/// Uno por rato de escritura (`SessionData::escritura`): empieza a acercarse un poco antes de
/// la primera tecla, se queda mientras se escribe y lo que se queda quieto tras un clic
/// (`quieto_ms`), y se aleja. **El sitio es el cursor de texto**, si Windows lo dio y cae
/// dentro de lo grabado, y ahí se queda fijo: mientras se escribe el ratón suele estar en
/// cualquier parte. Si no lo dio (en navegadores y apps de Electron muchas veces no), se
/// acerca a donde estaba el ratón y lo sigue, como los de los clics.
///
/// `ancho` y `alto` son los de la región grabada, que es en cuyos píxeles vienen el cursor de
/// texto y el ratón; los bloques salen de 0 a 1 sobre esa imagen entera, como los de la pista.
pub fn al_escribir(
    escritura: &[crate::contrato::TramoEscritura],
    rastro: &[(u64, i32, i32)],
    ancho: u32,
    alto: u32,
    nivel: f32,
    ajustes: &Ajustes,
) -> Vec<crate::contrato::TramoZoom> {
    use crate::contrato::{ModoZoom, TramoZoom};
    let (w, h) = (ancho.max(1) as f32, alto.max(1) as f32);
    escritura
        .iter()
        .filter(|t| t.hasta_ms >= t.desde_ms)
        .map(|t| {
            let caret = t
                .caret
                .filter(|&(x, y)| x >= 0 && y >= 0 && (x as u32) < ancho && (y as u32) < alto);
            let (x, y, modo) = match caret {
                Some((x, y)) => (x as f32 / w, y as f32 / h, ModoZoom::Fijo),
                None => {
                    let i = rastro.partition_point(|(ms, _, _)| *ms <= t.desde_ms);
                    let raton = i.checked_sub(1).or((!rastro.is_empty()).then_some(0)).map(|i| rastro[i]);
                    let (x, y) = raton.map_or((0.5, 0.5), |(_, x, y)| {
                        ((x as f32 / w).clamp(0.0, 1.0), (y as f32 / h).clamp(0.0, 1.0))
                    });
                    (x, y, ModoZoom::Sigue)
                }
            };
            TramoZoom {
                desde_ms: t.desde_ms.saturating_sub(ajustes.transicion_ms),
                hasta_ms: t.hasta_ms + ajustes.quieto_ms + ajustes.transicion_ms,
                nivel,
                modo,
                x,
                y,
                instantaneo: false,
                lupa: None,
            }
        })
        .collect()
}

/// Junta los zooms que salen solos (los de los clics y la tecla) con los de escribir.
///
/// Dos que se pisan, o que dejarían menos de una transición entre uno y otro, son **uno**:
/// alejarse y volver a acercarse en un segundo marea. El junto dura los dos, se acerca lo
/// que el que más, mira donde miraba el primero, y solo se queda fijo si los dos lo eran y
/// en el mismo sitio: si no, sigue al ratón, que es lo que va de un sitio a otro (escribir
/// en un campo y pulsar «Enviar» en otro).
pub fn juntar(
    mut bloques: Vec<crate::contrato::TramoZoom>,
    escritos: Vec<crate::contrato::TramoZoom>,
    aire_ms: u64,
) -> Vec<crate::contrato::TramoZoom> {
    use crate::contrato::ModoZoom;
    bloques.extend(escritos);
    bloques.sort_by_key(|z| (z.desde_ms, z.hasta_ms));
    let mut salen: Vec<crate::contrato::TramoZoom> = Vec::with_capacity(bloques.len());
    for z in bloques {
        match salen.last_mut() {
            Some(antes) if z.desde_ms <= antes.hasta_ms + aire_ms => {
                let mismo_sitio = (antes.x - z.x).abs() < 0.02 && (antes.y - z.y).abs() < 0.02;
                if !(antes.modo == ModoZoom::Fijo && z.modo == ModoZoom::Fijo && mismo_sitio) {
                    antes.modo = ModoZoom::Sigue;
                }
                antes.hasta_ms = antes.hasta_ms.max(z.hasta_ms);
                antes.nivel = antes.nivel.max(z.nivel);
            }
            _ => salen.push(z),
        }
    }
    salen
}

/// Lo más que puede separar un paso del anterior para que la cámara deje estela entre los
/// dos: más lejos, lo de en medio es un corte de la línea de tiempo y no un movimiento.
pub const ESTELA_HUECO_MS: u64 = 100;

/// El trozo de la imagen entera que enseña una cadena de recortes (el del usuario y el de la
/// cámara), cada uno medido sobre lo que dejó el anterior.
pub fn encuadre_de(recortes: &[super::recorte::Recorte]) -> super::recorte::Recorte {
    let mut r = super::recorte::Recorte { x1: 0.0, y1: 0.0, x2: 1.0, y2: 1.0 };
    for c in recortes {
        let (cx1, cx2) = (c.x1.min(c.x2).clamp(0.0, 1.0), c.x1.max(c.x2).clamp(0.0, 1.0));
        let (cy1, cy2) = (c.y1.min(c.y2).clamp(0.0, 1.0), c.y1.max(c.y2).clamp(0.0, 1.0));
        let (w, h) = (r.x2 - r.x1, r.y2 - r.y1);
        r = super::recorte::Recorte {
            x1: r.x1 + cx1 * w,
            y1: r.y1 + cy1 * h,
            x2: r.x1 + cx2 * w,
            y2: r.y1 + cy2 * h,
        };
    }
    r
}

/// Lo que se mueve lo que se ve de un encuadre al otro, en píxeles del vídeo exportado:
/// lo más que se desplaza uno de sus bordes medido sobre lo que se enseña ahora.
pub fn desplazamiento(
    antes: super::recorte::Recorte,
    ahora: super::recorte::Recorte,
    ancho: u32,
    alto: u32,
) -> f32 {
    let w = (ahora.x2 - ahora.x1).abs().max(1e-6);
    let h = (ahora.y2 - ahora.y1).abs().max(1e-6);
    let dx = (antes.x1 - ahora.x1).abs().max((antes.x2 - ahora.x2).abs()) / w * ancho as f32;
    let dy = (antes.y1 - ahora.y1).abs().max((antes.y2 - ahora.y2).abs()) / h * alto as f32;
    dx.max(dy)
}

/// La estela de la cámara (A4): el fotograma hecho con lo que la cámara enseñó por el camino
/// entre el paso anterior y este, como el obturador abierto de una cámara de verdad.
///
/// `imagen` es la grabada entera, y `antes` y `ahora` lo que se enseñaba de ella (ver
/// `encuadre_de`). Con la fuerza a 1 la estela cubre todo el camino desde el paso anterior;
/// a 0,5, la mitad más cercana. Si se mueve menos de dos píxeles no hay estela: `None`, y el
/// fotograma sale como siempre. Las copias van de 2 a 8, una cada tres píxeles de camino.
pub fn estela_de_camara(
    imagen: &image::RgbaImage,
    antes: super::recorte::Recorte,
    ahora: super::recorte::Recorte,
    fuerza: f32,
    ancho: u32,
    alto: u32,
) -> Option<image::RgbaImage> {
    let fuerza = if fuerza.is_finite() { fuerza.clamp(0.0, 1.0) } else { 0.0 };
    let largo = desplazamiento(antes, ahora, ancho, alto) * fuerza;
    if largo < 2.0 || ancho == 0 || alto == 0 {
        return None;
    }
    let copias = ((largo / 3.0).ceil() as usize).clamp(2, 8);
    let (iw, ih) = imagen.dimensions();
    let mut suma = vec![0u16; (ancho * alto * 4) as usize];
    for j in 0..copias {
        let f = 1.0 - fuerza * j as f32 / (copias - 1) as f32;
        let mezcla = |a: f32, b: f32| a + (b - a) * f;
        let r = super::recorte::Recorte {
            x1: mezcla(antes.x1, ahora.x1),
            y1: mezcla(antes.y1, ahora.y1),
            x2: mezcla(antes.x2, ahora.x2),
            y2: mezcla(antes.y2, ahora.y2),
        };
        let trozo = r.recorta_algo(iw, ih).then(|| r.aplicar(imagen));
        let trozo = trozo.as_ref().unwrap_or(imagen);
        let escalado;
        let lista = if trozo.dimensions() == (ancho, alto) {
            trozo
        } else {
            escalado = super::escalar::a_medida(trozo, ancho, alto);
            &escalado
        };
        for (s, p) in suma.iter_mut().zip(lista.as_raw()) {
            *s += u16::from(*p);
        }
    }
    let n = copias as u16;
    let datos: Vec<u8> = suma.iter().map(|s| ((s + n / 2) / n) as u8).collect();
    image::RgbaImage::from_raw(ancho, alto, datos)
}

#[cfg(test)]
mod al_escribir_y_estela {
    use super::*;
    use crate::contrato::{ModoZoom, TramoEscritura, TramoZoom};
    use crate::encode::recorte::Recorte;

    fn escrito(desde_ms: u64, hasta_ms: u64, caret: Option<(i32, i32)>) -> TramoEscritura {
        TramoEscritura { desde_ms, hasta_ms, caret }
    }

    #[test]
    fn con_cursor_de_texto_se_queda_fijo_en_el_y_sin_el_sigue_al_raton() {
        let a = Ajustes::default();
        let rastro = [(0u64, 100, 50), (3_000, 300, 150)];
        let bloques = al_escribir(
            &[escrito(1_000, 2_000, Some((200, 100))), escrito(5_000, 6_000, None)],
            &rastro,
            400,
            200,
            2.0,
            &a,
        );
        assert_eq!(bloques.len(), 2);
        let (uno, dos) = (bloques[0], bloques[1]);
        assert_eq!((uno.desde_ms, uno.hasta_ms), (1_000 - a.transicion_ms, 2_000 + a.quieto_ms + a.transicion_ms));
        assert_eq!((uno.modo, uno.x, uno.y), (ModoZoom::Fijo, 0.5, 0.5), "en el cursor de texto");
        // Sin cursor de texto, donde estaba el ratón al empezar a escribir: a los 5 s, en 300, 150.
        assert_eq!((dos.modo, dos.x, dos.y), (ModoZoom::Sigue, 0.75, 0.75));
        assert_eq!(dos.nivel, 2.0);
    }

    #[test]
    fn un_cursor_de_texto_fuera_de_lo_grabado_no_cuenta() {
        let bloques = al_escribir(&[escrito(1_000, 2_000, Some((900, 10)))], &[(0, 40, 20)], 400, 200, 2.0, &Ajustes::default());
        assert_eq!((bloques[0].modo, bloques[0].x, bloques[0].y), (ModoZoom::Sigue, 0.1, 0.1));
    }

    fn bloque(desde_ms: u64, hasta_ms: u64, modo: ModoZoom, x: f32) -> TramoZoom {
        TramoZoom { desde_ms, hasta_ms, nivel: 2.0, modo, x, y: 0.5, instantaneo: false, lupa: None }
    }

    #[test]
    fn los_que_se_pisan_se_juntan_en_uno_y_los_lejanos_no() {
        // Un clic en el campo y escribir justo después: un solo zoom, que sigue al ratón.
        let clic = bloque(500, 2_600, ModoZoom::Sigue, 0.3);
        let escribir = bloque(1_000, 6_000, ModoZoom::Fijo, 0.32);
        let lejos = bloque(9_000, 10_000, ModoZoom::Fijo, 0.8);
        let salen = juntar(vec![clic], vec![escribir, lejos], 450);
        assert_eq!(salen.len(), 2);
        assert_eq!((salen[0].desde_ms, salen[0].hasta_ms, salen[0].modo, salen[0].x), (500, 6_000, ModoZoom::Sigue, 0.3));
        assert_eq!(salen[1], lejos);
        // Dos de escribir en el mismo sitio y casi seguidos: uno, y sigue fijo.
        let a = bloque(0, 3_000, ModoZoom::Fijo, 0.5);
        let b = bloque(3_200, 5_000, ModoZoom::Fijo, 0.505);
        let junto = juntar(Vec::new(), vec![a, b], 450);
        assert_eq!(junto.len(), 1);
        assert_eq!((junto[0].hasta_ms, junto[0].modo), (5_000, ModoZoom::Fijo));
    }

    #[test]
    fn el_encuadre_de_dos_recortes_es_el_trozo_del_trozo() {
        let mitad_derecha = Recorte { x1: 0.5, y1: 0.0, x2: 1.0, y2: 1.0 };
        let su_mitad_izquierda = Recorte { x1: 0.0, y1: 0.0, x2: 0.5, y2: 0.5 };
        let r = encuadre_de(&[mitad_derecha, su_mitad_izquierda]);
        assert_eq!((r.x1, r.y1, r.x2, r.y2), (0.5, 0.0, 0.75, 0.5));
        let entera = encuadre_de(&[]);
        assert_eq!((entera.x1, entera.x2), (0.0, 1.0));
    }

    /// Una imagen con una raya blanca vertical en medio: movida, la estela la ensancha.
    #[test]
    fn la_camara_que_se_mueve_deja_estela_y_la_quieta_no() {
        let imagen = image::RgbaImage::from_fn(200, 100, |x, _| {
            if (98..102).contains(&x) { image::Rgba([255, 255, 255, 255]) } else { image::Rgba([0, 0, 0, 255]) }
        });
        let ahora = Recorte { x1: 0.25, y1: 0.0, x2: 0.75, y2: 1.0 };
        assert!(estela_de_camara(&imagen, ahora, ahora, 1.0, 200, 100).is_none(), "quieta, sin estela");
        let antes = Recorte { x1: 0.2, y1: 0.0, x2: 0.7, y2: 1.0 };
        assert!(estela_de_camara(&imagen, antes, ahora, 0.0, 200, 100).is_none(), "a fuerza cero, sin estela");
        let movida = estela_de_camara(&imagen, antes, ahora, 1.0, 200, 100).expect("se movió 20 px");
        let tocadas = |img: &image::RgbaImage| (0..200).filter(|&x| img.get_pixel(x, 50).0[0] > 10).count();
        let sin = super::super::escalar::a_medida(&ahora.aplicar(&imagen), 200, 100);
        assert!(tocadas(&movida) > tocadas(&sin) + 10, "la raya no se ha estirado: {} vs {}", tocadas(&movida), tocadas(&sin));
        // Y no se inventa luz: lo más blanco de la estela no pasa del blanco de la raya.
        assert!(movida.pixels().all(|p| p.0[3] == 255));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clic(ms: u64, x: i32, y: i32) -> Clic {
        Clic {
            ms,
            x,
            y,
            derecho: false,
        }
    }

    fn ajustes() -> Ajustes {
        Ajustes {
            escala: 2.0,
            transicion_ms: 400,
            quieto_ms: 1000,
            seguir_ms: 500,
            inercia_ms: 220,
            velocidad_max: 1.3,
        }
    }

    #[test]
    fn sin_clics_la_camara_no_se_mueve() {
        let c = camara(&[], 5000, 1920, 1080, &ajustes());
        assert_eq!(c.escala, 1.0);
        assert_eq!(c.recorte(1920, 1080), (0, 0, 1920, 1080));
    }

    #[test]
    fn un_clic_abre_un_tramo_que_empieza_antes_de_pulsar() {
        // Si el zoom empezara al pulsar, llegaria cerca cuando ya ha pasado lo importante.
        let t = tramos(&[clic(2000, 100, 100)], &ajustes());
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].desde_ms, 1600);
        assert_eq!(t[0].cerca_ms, 2000);
        assert_eq!(t[0].hasta_ms, 3400);
    }

    #[test]
    fn en_el_momento_del_clic_ya_esta_cerca_del_todo() {
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        assert_eq!(camara(&t, 2000, 1920, 1080, &a).escala, 2.0);
    }

    #[test]
    fn antes_del_tramo_y_despues_esta_entera() {
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        assert_eq!(camara(&t, 1000, 1920, 1080, &a).escala, 1.0);
        assert_eq!(camara(&t, 5000, 1920, 1080, &a).escala, 1.0);
    }

    #[test]
    fn a_media_transicion_esta_a_medio_camino() {
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        let mitad = camara(&t, 1800, 1920, 1080, &a).escala;
        assert!((mitad - 1.5).abs() < 0.01, "ha salido {mitad}");
    }

    #[test]
    fn el_movimiento_empieza_y_acaba_parado() {
        // Con una recta se nota el tiron. La curva tiene que moverse MENOS en los extremos
        // que en el centro, y eso es lo que se comprueba: no que valga 0,5 en la mitad,
        // que tambien lo cumple una recta.
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        let en = |ms| camara(&t, ms, 1920, 1080, &a).escala;
        let arranque = en(1640) - en(1600);
        let centro = en(1820) - en(1780);
        assert!(
            centro > arranque * 2.0,
            "el arranque ({arranque}) no es mas suave que el centro ({centro})"
        );
    }

    #[test]
    fn dos_clics_seguidos_son_un_solo_tramo() {
        // Acercarse y alejarse por cada clic de un doble clic marea mas que ayuda.
        let t = tramos(&[clic(2000, 100, 100), clic(2300, 120, 110)], &ajustes());
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].fin_cerca_ms, 2300);
    }

    #[test]
    fn y_se_queda_en_el_sitio_del_primero() {
        // La media de dos esquinas opuestas cae en el centro, que es donde no ha pasado nada.
        let t = tramos(&[clic(2000, 100, 100), clic(2300, 1800, 900)], &ajustes());
        assert_eq!((t[0].x, t[0].y), (100, 100));
    }

    #[test]
    fn dos_clics_lejos_en_el_tiempo_son_dos_tramos() {
        let t = tramos(&[clic(2000, 100, 100), clic(9000, 800, 400)], &ajustes());
        assert_eq!(t.len(), 2);
        assert_eq!((t[1].x, t[1].y), (800, 400));
    }

    #[test]
    fn un_clic_durante_el_alejamiento_alarga_el_tramo_en_vez_de_pisarlo() {
        // El primero se queda quieto hasta 3000 y se aleja hasta 3400. Un clic a 3200 caia
        // en el alejamiento y abria un tramo que se pisaba con este: corte seco.
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100), clic(3200, 1800, 900)], &a);
        assert_eq!(t.len(), 1, "tenia que alargarse, no abrir otro: {t:?}");
        assert_eq!(t[0].fin_cerca_ms, 3200);
    }

    #[test]
    fn dos_tramos_seguidos_nunca_se_pisan() {
        let a = ajustes();
        let clics: Vec<Clic> = (0..6).map(|i| clic(1000 + i * 1450, (i * 300) as i32, 100)).collect();
        let t = tramos(&clics, &a);
        for par in t.windows(2) {
            assert!(
                par[1].desde_ms >= par[0].hasta_ms,
                "el tramo {:?} empieza antes de que acabe {:?}",
                par[1],
                par[0]
            );
        }
    }

    /// El objetivo salta de una punta a la otra y la camara NO: va, y nunca mas deprisa
    /// que el tope. Es lo que evita el latigazo al hacer clic a la izquierda y luego a la
    /// derecha con el zoom puesto.
    #[test]
    fn la_camara_no_salta_de_un_lado_a_otro() {
        let a = ajustes();
        let ancho = 1920u32;
        let mut s = Seguimiento::default();
        let cerca = |x: i32| Camara { x, y: 500, escala: 2.0 };
        let primera = s.avanzar(cerca(200), 1000, ancho, &a);
        assert_eq!(primera.x, 200, "la primera vez se pone donde toca, sin recorrido");

        // A partir de aqui el objetivo esta en la otra punta.
        let mut anterior = primera;
        let mut llego = None;
        for i in 1..=90u64 {
            let ms = 1000 + i * 33;
            let ahora = s.avanzar(cerca(1700), ms, ancho, &a);
            let paso = (ahora.x - anterior.x).abs() as f32;
            let tope = a.velocidad_max * ancho as f32 * 33.0 / 1000.0 + 1.0;
            assert!(paso <= tope, "a los {ms} ms ha saltado {paso} px, mas que el tope {tope}");
            assert!(ahora.x >= anterior.x, "no puede volver atras");
            if llego.is_none() && (ahora.x - 1700).abs() <= 20 {
                llego = Some(ms);
            }
            anterior = ahora;
        }
        let llego = llego.expect("tiene que llegar");
        assert!(llego - 1000 >= 500, "ha llegado demasiado rapido: {} ms", llego - 1000);
        assert!(llego - 1000 <= 2500, "ha tardado demasiado: {} ms", llego - 1000);
    }

    #[test]
    fn con_la_imagen_entera_la_camara_se_olvida_de_donde_estaba() {
        let a = ajustes();
        let mut s = Seguimiento::default();
        s.avanzar(Camara { x: 200, y: 200, escala: 2.0 }, 1000, 1920, &a);
        let entera = s.avanzar(Camara::entera(1920, 1080), 1500, 1920, &a);
        assert_eq!(entera.escala, 1.0);
        // El siguiente acercamiento arranca centrado en su clic, no viniendo del anterior.
        let nuevo = s.avanzar(Camara { x: 1700, y: 900, escala: 2.0 }, 2000, 1920, &a);
        assert_eq!((nuevo.x, nuevo.y), (1700, 900));
    }

    #[test]
    fn entre_dos_tramos_se_vuelve_a_ver_la_pantalla_entera() {
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100), clic(9000, 800, 400)], &a);
        assert_eq!(camara(&t, 5500, 1920, 1080, &a).escala, 1.0);
    }

    #[test]
    fn el_recorte_al_doble_es_la_cuarta_parte_de_la_imagen() {
        let c = Camara {
            x: 960,
            y: 540,
            escala: 2.0,
        };
        assert_eq!(c.recorte(1920, 1080), (480, 270, 960, 540));
    }

    #[test]
    fn un_clic_en_la_esquina_no_saca_la_camara_de_la_imagen() {
        // Lo correcto es ensennar la esquina de verdad, no un trozo con la mitad en negro.
        let c = Camara {
            x: 5,
            y: 5,
            escala: 2.0,
        };
        assert_eq!(c.recorte(1920, 1080), (0, 0, 960, 540));
    }

    #[test]
    fn y_en_la_esquina_de_abajo_a_la_derecha_tampoco() {
        let c = Camara {
            x: 1915,
            y: 1075,
            escala: 2.0,
        };
        let (x, y, w, h) = c.recorte(1920, 1080);
        assert_eq!((x + w, y + h), (1920, 1080));
    }

    #[test]
    fn una_escala_por_debajo_de_uno_no_agranda_la_imagen() {
        // Pedir menos de 1 seria pedir un recorte mas grande que la imagen.
        let c = Camara {
            x: 960,
            y: 540,
            escala: 0.5,
        };
        assert_eq!(c.recorte(1920, 1080), (0, 0, 1920, 1080));
    }

    #[test]
    fn el_recorte_de_0_a_1_dice_lo_mismo_que_el_de_pixeles() {
        // El zoom entra por el mismo camino que el recorte del usuario, asi que los dos
        // tienen que decir exactamente lo mismo o la imagen saltaria al pasar de uno a otro.
        let c = Camara {
            x: 960,
            y: 540,
            escala: 2.0,
        };
        let r = c.como_recorte(1920, 1080);
        assert_eq!(r.en_pixeles(1920, 1080), c.recorte(1920, 1080));
    }

    #[test]
    fn sin_zoom_el_recorte_es_la_imagen_entera() {
        let r = Camara::entera(1920, 1080).como_recorte(1920, 1080);
        assert!(!r.recorta_algo(1920, 1080));
    }

    #[test]
    fn el_zoom_nunca_recorta_a_menos_de_dos_pixeles() {
        // H.264 no acepta un lado impar, y un lado de cero revienta al codificador.
        let c = Camara {
            x: 100,
            y: 100,
            escala: 5000.0,
        };
        let (_, _, w, h) = c.recorte(1920, 1080);
        assert!(w >= 2 && h >= 2, "{w}x{h}");
    }

    #[test]
    fn los_clics_de_un_arrastre_largo_no_dejan_la_camara_pegada_para_siempre() {
        // Un tramo se alarga con cada clic, pero termina: sin esto, alguien que hace clic
        // cada segundo durante un minuto tendria un minuto de zoom fijo sin poder salir.
        let a = ajustes();
        let clics: Vec<Clic> = (0..10).map(|i| clic(1000 + i * 500, 200, 200)).collect();
        let t = tramos(&clics, &a);
        assert_eq!(t.len(), 1);
        let ultimo = 1000 + 9 * 500;
        assert_eq!(t[0].hasta_ms, ultimo + a.quieto_ms + a.transicion_ms);
        assert_eq!(camara(&t, t[0].hasta_ms + 1, 1920, 1080, &a).escala, 1.0);
    }

    /// Un rastro de raton: una anotacion cada 33 ms, como al grabar a 30 fps.
    fn rastro(puntos: &[(u64, i32, i32)]) -> Vec<(u64, i32, i32)> {
        puntos.to_vec()
    }

    #[test]
    fn sin_zoom_el_raton_no_mueve_la_camara() {
        // Con la imagen entera no hay a donde seguir a nadie: se ve todo.
        let a = ajustes();
        let t = tramos(&[clic(2000, 100, 100)], &a);
        let r = rastro(&[(5000, 1800, 1000)]);
        assert_eq!(siguiendo(&t, &r, 5000, 1920, 1080, &a).escala, 1.0);
    }

    #[test]
    fn con_zoom_la_camara_se_va_hacia_donde_esta_el_raton() {
        // Quedarse clavada en el clic deja al raton fuera de cuadro en cuanto se mueve.
        let a = ajustes();
        let t = tramos(&[clic(1000, 200, 200)], &a);
        let r = rastro(&[
            (1000, 200, 200),
            (1200, 400, 300),
            (1400, 600, 400),
            (1500, 700, 450),
        ]);
        let c = siguiendo(&t, &r, 1500, 1920, 1080, &a);
        assert!(c.escala > 1.5, "tendria que seguir acercada");
        assert!(c.x > 300, "no ha seguido al raton: x = {}", c.x);
        assert!(c.y > 250, "no ha seguido al raton: y = {}", c.y);
    }

    #[test]
    fn pero_va_por_detras_y_no_pegada() {
        // Si saltara al punto exacto, un movimiento nervioso zarandearia la imagen.
        let a = ajustes();
        let t = tramos(&[clic(1000, 200, 200)], &a);
        let r = rastro(&[
            (1000, 200, 200),
            (1100, 300, 200),
            (1200, 400, 200),
            (1300, 500, 200),
            (1400, 900, 200),
        ]);
        let c = siguiendo(&t, &r, 1400, 1920, 1080, &a);
        assert!(
            c.x < 900,
            "la camara ha saltado encima del raton en vez de ir detras: {}",
            c.x
        );
        assert!(c.x > 300, "se ha quedado demasiado atras: {}", c.x);
    }

    #[test]
    fn un_temblor_no_zarandea_la_imagen() {
        // El raton va y viene alrededor de un punto: la camara tiene que quedarse quieta.
        let a = ajustes();
        let t = tramos(&[clic(1000, 500, 400)], &a);
        let mut puntos = Vec::new();
        for i in 0..20u64 {
            let vaiven = if i % 2 == 0 { 12 } else { -12 };
            puntos.push((1000 + i * 25, 500 + vaiven, 400 - vaiven));
        }
        let r = rastro(&puntos);
        let uno = siguiendo(&t, &r, 1400, 1920, 1080, &a);
        let otro = siguiendo(&t, &r, 1425, 1920, 1080, &a);
        assert!(
            (uno.x - otro.x).abs() <= 4,
            "la camara tiembla con el raton: {} y {}",
            uno.x,
            otro.x
        );
    }

    #[test]
    fn sin_rastro_se_queda_en_el_punto_del_clic() {
        // Las grabaciones de antes de esta version no tienen rastro anotado, y tienen que
        // seguir exportandose: se acercan al clic y ya, como hacian.
        let a = ajustes();
        let t = tramos(&[clic(1000, 200, 200)], &a);
        let c = siguiendo(&t, &[], 1000, 1920, 1080, &a);
        assert_eq!((c.x, c.y), (200, 200));
    }

    #[test]
    fn el_rastro_de_otro_momento_no_cuenta() {
        // Solo se promedia el pasado reciente. Con todo el rastro, la camara se iria a la
        // media de la grabacion entera, que es el centro de la pantalla.
        let a = ajustes();
        let t = tramos(&[clic(9000, 1500, 800)], &a);
        let mut puntos = vec![(100, 50, 50), (200, 60, 60), (300, 70, 70)];
        puntos.push((9000, 1500, 800));
        let c = siguiendo(&t, &rastro(&puntos), 9000, 1920, 1080, &a);
        assert!(c.x > 1200, "se ha ido al pasado antiguo: {}", c.x);
    }
}

#[cfg(test)]
mod velocidad {
    use super::*;

    #[test]
    fn el_doble_de_rapido_tarda_la_mitad_y_la_de_siempre_no_cambia_nada() {
        let rapido = Ajustes::a_velocidad(2.0, 2.0);
        assert_eq!(rapido.transicion_ms, 225);
        assert_eq!(rapido.inercia_ms, 110);
        assert!((rapido.velocidad_max - 2.6).abs() < 1e-6);
        assert_eq!(rapido.quieto_ms, Ajustes::default().quieto_ms, "lo que se mira no cambia");
        let normal = Ajustes::a_velocidad(2.0, 1.0);
        assert_eq!(normal.transicion_ms, Ajustes::default().transicion_ms);
        assert_eq!(normal.escala, 2.0);
    }

    #[test]
    fn una_velocidad_disparatada_se_queda_en_sus_topes() {
        assert_eq!(Ajustes::a_velocidad(2.0, 100.0).transicion_ms, 180);
        assert_eq!(Ajustes::a_velocidad(2.0, f32::NAN).transicion_ms, 450);
    }
}

#[cfg(test)]
mod a_mano {
    use super::*;

    fn bloque(desde_ms: u64, hasta_ms: u64, escala: f32) -> ZoomAMano {
        ZoomAMano { desde_ms, hasta_ms, escala, x: 400, y: 300, fijo: false, instantaneo: false }
    }

    /// El bloque es el rato entero acercado: entra al principio, sale antes del final, y
    /// fuera de él se ve la imagen entera.
    #[test]
    fn el_bloque_es_el_rato_entero_acercado() {
        let a = Ajustes::default();
        let t = tramos_a_mano(&[bloque(1_000, 4_000, 2.5)], &a);
        let en = |ms| camara(&t, ms, 1920, 1080, &a).escala;
        assert_eq!(en(999), 1.0);
        assert_eq!(en(1_000), 1.0, "empieza acercándose desde la imagen entera");
        assert_eq!(en(1_000 + a.transicion_ms), 2.5, "acaba de acercarse");
        assert_eq!(en(2_500), 2.5);
        assert_eq!(en(4_000 - a.transicion_ms), 2.5, "todavía no se aleja");
        assert_eq!(en(4_000), 1.0, "y acaba de alejarse en el final del bloque");
        assert_eq!(en(4_001), 1.0);
    }

    /// Cada bloque lleva su nivel: dos bloques seguidos se acercan cada uno lo suyo.
    #[test]
    fn cada_bloque_se_acerca_lo_suyo() {
        let a = Ajustes::default();
        let t = tramos_a_mano(&[bloque(0, 2_000, 1.5), bloque(3_000, 5_000, 3.0)], &a);
        assert_eq!(camara(&t, 1_000, 1920, 1080, &a).escala, 1.5);
        assert_eq!(camara(&t, 4_000, 1920, 1080, &a).escala, 3.0);
    }

    /// De golpe: cerca del todo desde el primer milisegundo y entera justo después.
    #[test]
    fn el_instantaneo_entra_y_sale_de_golpe() {
        let a = Ajustes::default();
        let mut b = bloque(1_000, 3_000, 2.0);
        b.instantaneo = true;
        let t = tramos_a_mano(&[b], &a);
        assert_eq!(camara(&t, 999, 1920, 1080, &a).escala, 1.0);
        assert_eq!(camara(&t, 1_000, 1920, 1080, &a).escala, 2.0);
        assert_eq!(camara(&t, 3_000, 1920, 1080, &a).escala, 2.0);
        assert_eq!(camara(&t, 3_001, 1920, 1080, &a).escala, 1.0);
    }

    /// Dos bloques que se pisan no dan dos cámaras a la vez: el segundo empieza donde acaba
    /// el primero. Y uno al revés, vacío o que no acerca nada, no cuenta.
    #[test]
    fn los_que_se_pisan_se_ordenan_y_los_vacios_no_cuentan() {
        let a = Ajustes::default();
        let t = tramos_a_mano(
            &[bloque(2_000, 5_000, 2.0), bloque(0, 3_000, 2.0), bloque(6_000, 6_000, 2.0), bloque(7_000, 9_000, 1.0)],
            &a,
        );
        assert_eq!(t.len(), 2, "{t:?}");
        assert_eq!((t[0].desde_ms, t[0].hasta_ms), (0, 3_000));
        assert_eq!((t[1].desde_ms, t[1].hasta_ms), (3_000, 5_000));
    }

    /// Un bloque corto sigue teniendo su rato quieto encima.
    #[test]
    fn un_bloque_corto_se_queda_quieto_un_rato() {
        let a = Ajustes::default();
        let t = tramos_a_mano(&[bloque(0, 600, 2.0)], &a);
        assert_eq!(t[0].cerca_ms, 200);
        assert_eq!(t[0].lejos_ms, 400);
        assert_eq!(camara(&t, 300, 1920, 1080, &a).escala, 2.0);
    }

    /// Un tramo de los clics hecho bloque vuelve a dar el mismo tramo: los automáticos
    /// salen en la pista como bloques, y tocarlos no puede cambiar lo que ya hacían.
    #[test]
    fn un_tramo_de_los_clics_hecho_bloque_es_el_mismo_tramo() {
        let a = Ajustes::a_velocidad(2.0, 1.0);
        let clics = [
            Clic { ms: 2_000, x: 100, y: 100, derecho: false },
            Clic { ms: 2_600, x: 300, y: 200, derecho: false },
        ];
        let auto = tramos(&clics, &a);
        let bloques: Vec<ZoomAMano> = auto
            .iter()
            .map(|t| ZoomAMano {
                desde_ms: t.desde_ms,
                hasta_ms: t.hasta_ms,
                escala: t.escala,
                x: t.x,
                y: t.y,
                fijo: false,
                instantaneo: false,
            })
            .collect();
        let otra_vez = tramos_a_mano(&bloques, &a);
        for ms in (0..6_000).step_by(10) {
            assert_eq!(
                camara(&auto, ms, 1920, 1080, &a),
                camara(&otra_vez, ms, 1920, 1080, &a),
                "en el ms {ms}"
            );
        }
    }

    /// Fijo: con el ratón cruzando la pantalla, el centro no se mueve en todo el tramo.
    #[test]
    fn el_fijo_no_sigue_al_raton() {
        let a = Ajustes::default();
        let mut b = bloque(0, 3_000, 2.0);
        b.fijo = true;
        let t = tramos_a_mano(&[b], &a);
        let rastro: Vec<(u64, i32, i32)> = (0..200u64).map(|k| (k * 16, 50 + k as i32 * 9, 900)).collect();
        let mut sigue = Seguimiento::default();
        for ms in (0..=3_000).step_by(33) {
            let objetivo = siguiendo(&t, &rastro, ms, 1920, 1080, &a);
            let c = sigue.colocar(objetivo, ms);
            if c.escala > 1.001 {
                assert_eq!((c.x, c.y), (400, 300), "en el ms {ms} el centro se ha ido a {c:?}");
            }
        }
    }

    /// Después de un tramo fijo, la inercia sale de donde estaba la cámara, no de antes.
    #[test]
    fn colocar_deja_apuntado_el_sitio_de_partida() {
        let a = Ajustes::default();
        let mut s = Seguimiento::default();
        s.colocar(Camara { x: 400, y: 300, escala: 2.0 }, 1_000);
        let despues = s.avanzar(Camara { x: 1_400, y: 300, escala: 2.0 }, 1_033, 1920, &a);
        assert!(despues.x > 400 && despues.x < 1_400, "ha salido de {despues:?}");
        assert_eq!(s.colocar(Camara::entera(1920, 1080), 2_000).escala, 1.0);
        let de_nuevo = s.avanzar(Camara { x: 1_400, y: 300, escala: 2.0 }, 2_033, 1920, &a);
        assert_eq!(de_nuevo.x, 1_400, "con la imagen entera se olvida de dónde estaba");
    }
}
