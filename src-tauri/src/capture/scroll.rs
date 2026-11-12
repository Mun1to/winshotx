//! La captura con scroll: se elige una zona, la página baja (sola o con la rueda de quien
//! captura) y las fotos se cosen en una sola imagen larga.
//!
//! **Lo difícil es coser**, y vive aparte, en `Cosedor`, sin nada de Windows: recibe fotos
//! de la misma zona y decide cuánto se ha movido cada una respecto a la anterior. Así se
//! prueba con fotos de verdad de un navegador sin tocar la pantalla de nadie
//! (`tests/scroll/`, que salen de `scripts/fotos-scroll.mjs`).
//!
//! **Cómo se cose.** Entre dos fotos seguidas:
//!
//! 1. Se miran las filas que siguen iguales en el mismo sitio, por arriba y por abajo: son
//!    lo que no se mueve (una cabecera fija, una barra de cookies abajo). No se comparan.
//! 2. En lo que queda se busca el desplazamiento con una firma de cada fila (la luz media
//!    de 32 trozos de la fila): se prueban todos los desplazamientos posibles, arriba y
//!    abajo, y los mejores se repasan píxel a píxel.
//! 3. A la imagen se le quita lo de abajo que no se mueve, se le pegan las filas nuevas y
//!    se le vuelve a poner el pie de la foto nueva. La cabecera sale una vez, la de la
//!    primera foto, porque de las demás solo se cogen filas de abajo.
//!
//! **Lo que se queda quieto abajo se pide siempre de más, nunca de menos.** Coger de la foto
//! nueva unas filas que ya estaban es inofensivo: son las mismas filas de la página en su
//! mismo sitio. Coger de menos es repetir la barra de abajo en medio de la imagen, que es
//! justo la costura que se ve. Por eso el pie que se quita es el mayor que se ha visto.

use std::time::Duration;

use image::RgbaImage;

/// Lo que puede cambiar un canal entre dos fotos de lo mismo sin contar como distinto.
/// La pantalla no tiene ruido, pero un texto con suavizado puede variar un poco de una
/// foto a otra al pasar el ratón por encima.
const TOLERANCIA: i16 = 24;

/// Trozos por fila de la firma. 32 basta para que dos filas de texto distintas no se
/// confundan, y la barra de desplazamiento (15 o 17 px) solo toca uno.
const TROZOS: usize = 32;

/// Las filas sueltas que se aguantan en el borde antes de lo que no se mueve, y lo que
/// tiene que medir eso para contar como fijo (ver `bandas_quietas`).
const RESQUICIO: u32 = 6;
const FIJO_MINIMO: u32 = 12;

/// Lo mínimo que tienen que compartir dos fotos para coserlas, en filas.
const SOLAPE_MINIMO: u32 = 24;

/// Por debajo de esto, lo que cambia entre dos fotos no es un desplazamiento: es algo que
/// se mueve dentro de la página (un cursor de texto que parpadea, un GIF).
const ZONA_MINIMA: u32 = 40;

/// La parte de píxeles distintos que se aguanta en un cosido bueno: el ratón encima de un
/// enlace, la barra de desplazamiento que ha bajado. Por encima, se ha perdido el hilo.
const ERROR_MAXIMO: f32 = 0.06;

/// Lo más alta que se deja crecer la imagen, y lo más grande en píxeles: una página
/// infinita no puede llenar la memoria. 30.000 filas son unas veinte pantallas.
pub const ALTO_MAXIMO: u32 = 30_000;
const PIXELES_MAXIMOS: u64 = 48_000_000;

/// Lo que ha pasado con una foto nueva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    /// Es la misma que la anterior: no se ha movido nada.
    Quieta,
    /// Ha bajado tantas filas, y se han cosido.
    Cosida(u32),
    /// Ha subido: quien desplaza ha ido hacia atrás. Se ignora hasta que vuelva.
    Atras,
    /// No se parece a la anterior: ha bajado demasiado de golpe, o ha cambiado la página.
    Perdida,
    /// La imagen ya no puede crecer más.
    Llena,
}

/// Una foto con su firma ya calculada.
struct Foto {
    pixeles: Vec<u8>,
    /// `alto` filas de `trozos` valores: la luz media de cada trozo de la fila.
    firma: Vec<u8>,
}

/// Junta las fotos de una misma zona en una imagen larga.
pub struct Cosedor {
    ancho: u32,
    alto: u32,
    trozos: usize,
    /// La imagen cosida hasta ahora, en RGBA. Mide siempre `inicio + alto` filas, donde
    /// `inicio` es dónde cae la fila 0 de la última foto aceptada.
    lienzo: Vec<u8>,
    referencia: Foto,
    /// Lo de abajo que no se mueve, el mayor que se ha visto (ver la cabecera del módulo).
    pie: u32,
    /// Lo que bajó el último cosido, para desempatar cuando la página es lisa.
    ultimo: Option<u32>,
    /// El alto de lo que se mueve en el último par comparado.
    zona: u32,
    alto_maximo: u32,
    cosidas: u32,
}

impl Cosedor {
    /// Empieza con la primera foto, que entra entera.
    pub fn nuevo(primera: RgbaImage) -> Self {
        let (ancho, alto) = primera.dimensions();
        let trozos = TROZOS.min(ancho.max(1) as usize);
        let pixeles = primera.into_raw();
        let firma = firmar(&pixeles, ancho, alto, trozos);
        let por_pixeles = (PIXELES_MAXIMOS / u64::from(ancho.max(1))) as u32;
        Self {
            ancho,
            alto,
            trozos,
            lienzo: pixeles.clone(),
            referencia: Foto { pixeles, firma },
            pie: 0,
            ultimo: None,
            zona: alto,
            alto_maximo: ALTO_MAXIMO.min(por_pixeles).max(alto),
            cosidas: 0,
        }
    }

    /// El alto de la imagen cosida hasta ahora.
    pub fn alto(&self) -> u32 {
        (self.lienzo.len() / (self.ancho as usize * 4)) as u32
    }

    pub fn ancho(&self) -> u32 {
        self.ancho
    }

    /// Cuántas fotos se han cosido, sin contar la primera.
    pub fn cosidas(&self) -> u32 {
        self.cosidas
    }

    /// El alto de lo que se desplaza, sin cabecera ni pie: lo que puede bajar de una vez.
    pub fn zona(&self) -> u32 {
        self.zona
    }

    /// La imagen entera, tal como va.
    pub fn imagen(&self) -> RgbaImage {
        RgbaImage::from_raw(self.ancho, self.alto(), self.lienzo.clone())
            .expect("el lienzo mide siempre filas enteras")
    }

    /// Mira una foto nueva de la zona y, si ha bajado, la cose.
    pub fn anadir(&mut self, nueva: RgbaImage) -> Paso {
        if nueva.dimensions() != (self.ancho, self.alto) {
            return Paso::Perdida;
        }
        let pixeles = nueva.into_raw();
        let foto = Foto {
            firma: firmar(&pixeles, self.ancho, self.alto, self.trozos),
            pixeles,
        };

        let (arriba, abajo) = self.bandas_quietas(&foto);
        if arriba + abajo >= self.alto {
            return Paso::Quieta;
        }
        let zona = self.alto - arriba - abajo;
        if zona < ZONA_MINIMA {
            return Paso::Quieta;
        }

        let Some((d, error)) = self.medir(&foto, arriba, abajo) else {
            return Paso::Perdida;
        };
        if error > ERROR_MAXIMO {
            return Paso::Perdida;
        }
        if d == 0 {
            return Paso::Quieta;
        }
        if d < 0 {
            return Paso::Atras;
        }
        let d = d as u32;
        if self.alto() + d > self.alto_maximo {
            return Paso::Llena;
        }

        // Lo de abajo que se quita y se vuelve a poner con la foto nueva. Nunca puede
        // empezar por encima de la zona que se mueve: ahí está la cabecera.
        let pie = self.pie.max(abajo).min(self.alto - arriba - d);
        let fila = self.ancho as usize * 4;
        let quedan = self.lienzo.len() - pie as usize * fila;
        self.lienzo.truncate(quedan);
        let desde = (self.alto - pie - d) as usize * fila;
        self.lienzo.extend_from_slice(&foto.pixeles[desde..]);

        self.pie = pie;
        self.zona = zona;
        self.ultimo = Some(d);
        self.referencia = foto;
        self.cosidas += 1;
        Paso::Cosida(d)
    }

    /// Las filas que no han cambiado de sitio, contadas desde arriba y desde abajo.
    fn bandas_quietas(&self, nueva: &Foto) -> (u32, u32) {
        let fila = self.ancho as usize * 4;
        let margen = margen_de(self.ancho);
        let igual = |y: u32| {
            let a = y as usize * fila;
            filas_iguales(
                &self.referencia.pixeles[a..a + fila],
                &nueva.pixeles[a..a + fila],
                margen,
            )
        };
        let alto = self.alto;
        // Lo fijo puede no empezar justo en el borde: al 125 % una cabecera pegajosa cae en
        // medio píxel y deja asomar por encima una tira de la página que sí se mueve. Se
        // busca también un tramo fijo que empiece en las primeras filas, si es largo.
        let tramo = |desde: u32| (desde..alto).take_while(|&y| igual(y)).count() as u32;
        let mut arriba = tramo(0);
        if arriba == alto {
            return (alto, 0);
        }
        for y in 1..=RESQUICIO.min(alto - 1) {
            let largo = tramo(y);
            if largo >= FIJO_MINIMO {
                arriba = arriba.max(y + largo);
            }
        }
        // Y lo mismo abajo, contando hacia arriba sin pisar lo de arriba.
        let tramo_abajo = |hasta: u32| (arriba..hasta).rev().take_while(|&y| igual(y)).count() as u32;
        let mut abajo = tramo_abajo(alto);
        for y in 1..=RESQUICIO.min(alto - arriba) {
            let largo = tramo_abajo(alto - y);
            if largo >= FIJO_MINIMO {
                abajo = abajo.max(y + largo);
            }
        }
        (arriba, abajo.min(alto - arriba))
    }

    /// Cuánto ha bajado la foto nueva respecto a la de referencia (negativo si ha subido),
    /// y la parte de píxeles que no cuadra con ese desplazamiento. `None` si la zona que se
    /// mueve no da para comparar.
    fn medir(&self, nueva: &Foto, arriba: u32, abajo: u32) -> Option<(i32, f32)> {
        let zona = self.alto - arriba - abajo;
        let solape = SOLAPE_MINIMO.max(zona / 5).min(zona);
        let tope = (zona - solape) as i32;

        // Primero, con la firma, todos los desplazamientos posibles. Con una zona alta se
        // mira una fila de cada tres: sobra para encontrar los buenos candidatos.
        let salto = if zona > 600 { 3 } else { 2 };
        let mut gruesos: Vec<(i32, f32)> = (-tope..=tope)
            .filter_map(|d| self.coste_grueso(nueva, arriba, abajo, d, salto).map(|c| (d, c)))
            .collect();
        if gruesos.is_empty() {
            return None;
        }
        gruesos.sort_by(|a, b| a.1.total_cmp(&b.1));

        // Y los mejores, píxel a píxel. Entre ellos va siempre el que repite lo que bajó la
        // vez anterior: en una página lisa muchos empatan, y ese es el que más se parece a
        // lo que está haciendo la rueda.
        let mut candidatos: Vec<i32> = gruesos.iter().take(6).map(|c| c.0).collect();
        if let Some(previo) = self.ultimo {
            let previo = previo as i32;
            if previo <= tope && !candidatos.contains(&previo) {
                candidatos.push(previo);
            }
        }
        let esperado = self.ultimo.map(|d| d as i32).unwrap_or(0);
        let mut mejor: Option<(i32, f32)> = None;
        for d in candidatos {
            let error = self.coste_fino(nueva, arriba, abajo, d);
            let gana = match mejor {
                None => true,
                Some((md, me)) => {
                    error < me - 0.002
                        || (error <= me + 0.002 && (d - esperado).abs() < (md - esperado).abs())
                }
            };
            if gana {
                mejor = Some((d, error));
            }
        }
        mejor
    }

    /// Las filas de la foto nueva que se comparan con `d`: las de la zona que se mueve
    /// cuya fila de la referencia (`y + d`) también cae dentro de la zona.
    fn filas_comunes(&self, arriba: u32, abajo: u32, d: i32) -> Option<(u32, u32)> {
        let alto = self.alto as i32;
        let (arriba, abajo) = (arriba as i32, abajo as i32);
        let desde = arriba.max(arriba - d);
        let hasta = (alto - abajo).min(alto - abajo - d);
        (hasta - desde >= SOLAPE_MINIMO as i32).then_some((desde as u32, hasta as u32))
    }

    /// La diferencia media entre firmas con ese desplazamiento.
    fn coste_grueso(&self, nueva: &Foto, arriba: u32, abajo: u32, d: i32, salto: usize) -> Option<f32> {
        let (desde, hasta) = self.filas_comunes(arriba, abajo, d)?;
        let t = self.trozos;
        let mut total: u32 = 0;
        let mut filas: u32 = 0;
        for y in (desde as usize..hasta as usize).step_by(salto) {
            let n = &nueva.firma[y * t..(y + 1) * t];
            let r0 = (y as i32 + d) as usize * t;
            let r = &self.referencia.firma[r0..r0 + t];
            total += n
                .iter()
                .zip(r)
                .map(|(&a, &b)| u32::from(a.abs_diff(b)))
                .sum::<u32>();
            filas += 1;
        }
        Some(total as f32 / (filas.max(1) as usize * t) as f32)
    }

    /// La parte de píxeles distintos con ese desplazamiento, mirando una fila de cada dos.
    fn coste_fino(&self, nueva: &Foto, arriba: u32, abajo: u32, d: i32) -> f32 {
        let Some((desde, hasta)) = self.filas_comunes(arriba, abajo, d) else {
            return 1.0;
        };
        let fila = self.ancho as usize * 4;
        let mut distintos: u64 = 0;
        let mut mirados: u64 = 0;
        for y in (desde as usize..hasta as usize).step_by(2) {
            let n = &nueva.pixeles[y * fila..(y + 1) * fila];
            let r0 = (y as i32 + d) as usize * fila;
            let r = &self.referencia.pixeles[r0..r0 + fila];
            distintos += pixeles_distintos(n, r) as u64;
            mirados += u64::from(self.ancho);
        }
        distintos as f32 / mirados.max(1) as f32
    }
}

/// Cuántos píxeles distintos se aguantan en una fila que «no ha cambiado»: la barra de
/// desplazamiento cambia en cada foto y no por eso la fila se ha movido.
fn margen_de(ancho: u32) -> usize {
    (ancho as usize / 50).max(2)
}

fn pixeles_distintos(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| {
            (0..3).any(|c| (i16::from(p[c]) - i16::from(q[c])).abs() > TOLERANCIA)
        })
        .count()
}

fn filas_iguales(a: &[u8], b: &[u8], margen: usize) -> bool {
    if a == b {
        return true;
    }
    pixeles_distintos(a, b) <= margen
}

/// La luz media de cada trozo de cada fila.
fn firmar(pixeles: &[u8], ancho: u32, alto: u32, trozos: usize) -> Vec<u8> {
    let ancho = ancho as usize;
    let mut firma = Vec::with_capacity(alto as usize * trozos);
    for fila in pixeles.chunks_exact(ancho * 4) {
        for t in 0..trozos {
            let x0 = t * ancho / trozos;
            let x1 = ((t + 1) * ancho / trozos).max(x0 + 1);
            let suma: u32 = fila[x0 * 4..x1 * 4]
                .chunks_exact(4)
                .map(|p| (77 * u32::from(p[0]) + 150 * u32::from(p[1]) + 29 * u32::from(p[2])) >> 8)
                .sum();
            firma.push((suma / (x1 - x0) as u32) as u8);
        }
    }
    firma
}

/// Lo que se espera entre foto y foto mientras la página baja.
const ENTRE_FOTOS: Duration = Duration::from_millis(40);
/// Lo que se espera entre foto y foto cuando desplaza quien captura.
const MIRAR_A_MANO: Duration = Duration::from_millis(60);
/// Lo más que se espera a que la página se quede quieta tras girar la rueda: una página
/// con algo animado no se queda quieta nunca.
const ESPERA_MAXIMA: Duration = Duration::from_millis(1500);
/// Lo que se espera antes de dar por quieta una página que no se ha movido nada: el
/// desplazamiento suave tarda un poco en arrancar.
const ARRANQUE: Duration = Duration::from_millis(300);
/// Lo que se espera a una página que carga más al llegar abajo.
const CRECER: Duration = Duration::from_millis(400);
/// Lo más que se gira de una vez.
const MUESCAS_MAXIMAS: i32 = 8;
/// Las veces que se sube tras perderse antes de dejarle la rueda a quien captura.
const RESCATES: u32 = 3;

/// Lo que la captura necesita de la pantalla mientras baja. En Windows son las fotos, la
/// rueda y el ratón de verdad (`en_vivo`); en las pruebas, una página de mentira con su
/// propio reloj, para probar el bucle entero sin tocar la pantalla de nadie.
pub(crate) trait Pantalla {
    /// La foto de la zona.
    fn foto(&mut self) -> crate::error::Result<RgbaImage>;
    /// Gira la rueda: las muescas positivas bajan la página y las negativas la suben.
    fn rueda(&mut self, muescas: i32);
    fn esperar(&mut self, tiempo: Duration);
    /// El tiempo que lleva la captura.
    fn reloj(&self) -> Duration;
    /// Si quien captura ha cogido el ratón: entonces la rueda la pone él.
    fn raton_movido(&mut self) -> bool;
    /// Si se ha pedido parar: «Parar», Esc o la X.
    fn parar(&mut self) -> bool;
    /// Cómo va, para la barra.
    fn avisar(&mut self, alto: u32, estado: &'static str);
}

/// Cómo acaba una vuelta de la rueda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fin {
    /// La página se ha quedado quieta y lo que se ve encaja con lo cosido.
    Encaja,
    /// Se ha quedado quieta lejos de lo cosido: ha bajado más de lo que se puede coser.
    Perdida,
    /// Se ha pedido parar, la imagen está llena o la pantalla ya no da fotos.
    Parar,
}

struct Vuelta {
    bajado: u32,
    /// Si lo cosido ha visto moverse la página, hacia abajo o hacia arriba.
    movida: bool,
    fin: Fin,
}

/// Mira fotos seguidas hasta que la página se queda quieta: dos iguales a la anterior, o
/// lo más que se espera si algo se mueve dentro de ella.
struct Asiento {
    inicio: Duration,
    anterior: Option<RgbaImage>,
    iguales: u32,
    ha_cambiado: bool,
}

impl Asiento {
    fn nuevo(inicio: Duration) -> Self {
        Self { inicio, anterior: None, iguales: 0, ha_cambiado: false }
    }

    /// Cuenta una foto y dice si la página ya está quieta.
    fn quieta(&mut self, foto: &RgbaImage, ahora: Duration) -> bool {
        match &self.anterior {
            Some(anterior) if casi_iguales(anterior, foto) => self.iguales += 1,
            Some(_) => {
                self.iguales = 0;
                self.ha_cambiado = true;
            }
            None => {}
        }
        self.anterior = Some(foto.clone());
        let tiempo = ahora.saturating_sub(self.inicio);
        (self.iguales >= 2 && (self.ha_cambiado || tiempo > ARRANQUE)) || tiempo > ESPERA_MAXIMA
    }
}

/// Si dos fotos seguidas son la misma, aguantando un cursor de texto que parpadea.
fn casi_iguales(a: &RgbaImage, b: &RgbaImage) -> bool {
    if a.dimensions() != b.dimensions() {
        return false;
    }
    if a.as_raw() == b.as_raw() {
        return true;
    }
    let aguante = (a.width() as usize * a.height() as usize) / 1000 + 16;
    pixeles_distintos(a.as_raw(), b.as_raw()) <= aguante
}

/// Baja la página y va cosiendo hasta que se pide parar o se llega al final.
///
/// **Cuánto se gira cada vez.** Lo que baja una muesca lo decide el programa de debajo, y no
/// es siempre lo mismo: en Edge al 125 %, tres muescas bajaron 250 px y cuatro, 500. Se gira
/// para bajar media zona contando con lo MÁS que ha bajado una muesca, y como mucho el doble
/// que la vez anterior. Así se perdía el 11 de noviembre de 2026: con lo que había bajado la
/// última vuelta, la siguiente bajaba 500 px de golpe en una zona de 630, más de lo que se
/// puede coser, y la imagen se quedaba en el principio de la página.
///
/// **Si se pierde igual**, sube lo mismo que bajó hasta que lo que se ve vuelve a encajar
/// con lo cosido, y sigue de muesca en muesca. El final de la página es girar y que no se
/// mueva nada; una vuelta perdida ya no lo parece.
pub(crate) fn bajar(p: &mut impl Pantalla, cosedor: &mut Cosedor) {
    let mut sola = true;
    let mut estado: &'static str = "bajando";
    let mut muescas: i32 = 1;
    let mut por_muesca: u32 = 0;
    let mut giros_en_vano = 0;
    p.avisar(cosedor.alto(), estado);

    loop {
        if p.parar() {
            return;
        }
        if sola && p.raton_movido() {
            sola = false;
            estado = "tu";
        }
        p.avisar(cosedor.alto(), estado);

        if !sola {
            // La rueda la pone quien captura: se mira lo que haya y se cose si ha bajado.
            p.esperar(MIRAR_A_MANO);
            let Ok(foto) = p.foto() else { return };
            match cosedor.anadir(foto) {
                Paso::Cosida(_) => estado = "tu",
                Paso::Perdida => estado = "perdida",
                Paso::Llena => return,
                Paso::Quieta | Paso::Atras => {}
            }
            continue;
        }

        let vuelta = girar(p, cosedor, muescas, estado);
        match vuelta.fin {
            Fin::Parar => return,
            Fin::Perdida => {
                if volver(p, cosedor, muescas) {
                    muescas = 1;
                } else {
                    sola = false;
                    estado = "perdida";
                }
                continue;
            }
            Fin::Encaja => {}
        }

        if !vuelta.movida {
            giros_en_vano += 1;
            if giros_en_vano >= 2 {
                if cosedor.cosidas() > 0 {
                    // Ha bajado y ya no baja: es el final de la página.
                    return;
                }
                // No ha bajado nunca: la rueda ha caído en algo que no se desplaza.
                sola = false;
                estado = "no-baja";
            } else {
                p.esperar(CRECER);
            }
            continue;
        }
        giros_en_vano = 0;
        if vuelta.bajado > 0 {
            por_muesca = por_muesca.max(vuelta.bajado.div_ceil(muescas as u32));
            let caben = (cosedor.zona() / 2 / por_muesca) as i32;
            muescas = caben.clamp(1, (muescas * 2).min(MUESCAS_MAXIMAS));
        }
    }
}

/// Gira la rueda y fotografía hasta que la página se queda quieta, cosiendo lo que baje por
/// el camino.
fn girar(p: &mut impl Pantalla, cosedor: &mut Cosedor, muescas: i32, estado: &'static str) -> Vuelta {
    p.rueda(muescas);
    let mut asiento = Asiento::nuevo(p.reloj());
    let mut vuelta = Vuelta { bajado: 0, movida: false, fin: Fin::Encaja };
    loop {
        p.esperar(ENTRE_FOTOS);
        if p.parar() {
            vuelta.fin = Fin::Parar;
            return vuelta;
        }
        let Ok(foto) = p.foto() else {
            vuelta.fin = Fin::Parar;
            return vuelta;
        };
        let quieta = asiento.quieta(&foto, p.reloj());
        let paso = cosedor.anadir(foto);
        match paso {
            Paso::Cosida(d) => {
                vuelta.bajado += d;
                vuelta.movida = true;
                p.avisar(cosedor.alto(), estado);
            }
            Paso::Atras => vuelta.movida = true,
            Paso::Llena => {
                vuelta.fin = Fin::Parar;
                return vuelta;
            }
            Paso::Quieta | Paso::Perdida => {}
        }
        if quieta {
            if paso == Paso::Perdida {
                vuelta.fin = Fin::Perdida;
            }
            return vuelta;
        }
    }
}

/// Tras perderse: sube lo mismo que bajó hasta que lo que se ve vuelve a encajar con lo
/// cosido, o queda por encima (y entonces se alcanza bajando de poco en poco).
fn volver(p: &mut impl Pantalla, cosedor: &mut Cosedor, muescas: i32) -> bool {
    for _ in 0..RESCATES {
        p.rueda(-muescas);
        let mut asiento = Asiento::nuevo(p.reloj());
        let foto = loop {
            p.esperar(ENTRE_FOTOS);
            if p.parar() {
                return false;
            }
            let Ok(foto) = p.foto() else { return false };
            if asiento.quieta(&foto, p.reloj()) {
                break foto;
            }
        };
        match cosedor.anadir(foto) {
            Paso::Perdida => continue,
            Paso::Llena => return false,
            Paso::Cosida(_) | Paso::Quieta | Paso::Atras => return true,
        }
    }
    false
}

#[cfg(windows)]
mod en_vivo;

/// Empieza a capturar esa zona (en píxeles del escritorio virtual): cierra el overlay,
/// pone el marco y la barra, y baja la página. Vuelve enseguida; lo demás va en su hilo.
#[tauri::command]
pub async fn scroll_iniciar(app: tauri::AppHandle, region: super::Rect) -> crate::error::Result<()> {
    #[cfg(windows)]
    {
        en_vivo::iniciar(&app, region)
    }
    // Fuera de Windows no hay rueda que girar ni pantalla que fotografiar así.
    #[cfg(not(windows))]
    {
        let _ = (app, region);
        Err(crate::error::AppError::Msg(
            "la captura con scroll solo funciona en Windows".into(),
        ))
    }
}

/// Para y se queda con lo que lleve: lo abre en el editor.
#[tauri::command]
pub async fn scroll_terminar() {
    #[cfg(windows)]
    en_vivo::pedir(en_vivo::TERMINAR);
}

/// Para y lo tira.
#[tauri::command]
pub async fn scroll_descartar() {
    #[cfg(windows)]
    en_vivo::pedir(en_vivo::DESCARTAR);
}

/// El evento con el que la barra se entera de cómo va la captura.
pub const EVENTO_PROGRESO: &str = "winshotx://scroll-progreso";

/// La etiqueta de la ventanita de la barra. Tiene su propio permiso en
/// `capabilities/scroll.json`.
pub const BARRA_LABEL: &str = "scroll-barra";

/// Dónde va la barra: debajo de la zona si cabe, encima si no, y si la zona ocupa la
/// pantalla entera, dentro y pegada abajo (no sale en las fotos: se aparta de la captura).
pub(crate) fn sitio_de_la_barra(
    region: super::Rect,
    pantalla: Option<(i32, i32, i32, i32)>,
    ancho: i32,
    alto: i32,
    hueco: i32,
) -> (i32, i32) {
    let mut x = region.x + (region.width as i32 - ancho) / 2;
    let abajo = region.y + region.height as i32 + hueco;
    let Some((px, py, pw, ph)) = pantalla else {
        return (x, abajo);
    };
    x = x.clamp(px + hueco, (px + pw - ancho - hueco).max(px + hueco));
    let y = if abajo + alto <= py + ph {
        abajo
    } else if region.y - hueco - alto >= py {
        region.y - hueco - alto
    } else {
        (region.y + region.height as i32).min(py + ph) - hueco - alto
    };
    (x, y)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use image::Rgba;

    /// Una página larga de mentira: rayas de colores de alto variable con un dibujo en
    /// cada una, para que ninguna fila se confunda con otra.
    fn pagina(ancho: u32, alto: u32) -> RgbaImage {
        RgbaImage::from_fn(ancho, alto, |x, y| {
            let banda = y / 37;
            let base = [(banda * 53 % 200) as u8 + 30, (banda * 97 % 180) as u8 + 40, (banda * 31 % 220) as u8 + 20];
            // Unas «letras»: puntos que dependen de la fila y de la columna.
            if (x * 7 + y * 13 + banda * 5) % 23 < 4 {
                Rgba([base[0] / 3, base[1] / 3, base[2] / 3, 255])
            } else {
                Rgba([base[0], base[1], base[2], 255])
            }
        })
    }

    /// Lo que se vería en la zona con la página bajada `y` filas, con una cabecera fija
    /// arriba y un pie fijo abajo.
    fn vista(pagina: &RgbaImage, y: u32, alto: u32, cabecera: u32, pie: u32) -> RgbaImage {
        let ancho = pagina.width();
        RgbaImage::from_fn(ancho, alto, |x, f| {
            if f < cabecera {
                Rgba([20, 20, 30, 255])
            } else if f >= alto - pie {
                if (x / 9) % 2 == 0 { Rgba([240, 200, 40, 255]) } else { Rgba([30, 30, 30, 255]) }
            } else {
                *pagina.get_pixel(x, y + f)
            }
        })
    }

    fn coser(fotos: Vec<RgbaImage>) -> (RgbaImage, Vec<Paso>) {
        let mut fotos = fotos.into_iter();
        let mut cosedor = Cosedor::nuevo(fotos.next().unwrap());
        let pasos = fotos.map(|f| cosedor.anadir(f)).collect();
        (cosedor.imagen(), pasos)
    }

    #[test]
    fn sin_nada_fijo_sale_la_pagina_tal_cual() {
        let p = pagina(300, 2000);
        let posiciones = [0, 120, 350, 351, 700, 1000, 1200, 1400];
        let fotos = posiciones.iter().map(|&y| vista(&p, y, 600, 0, 0)).collect();
        let (imagen, pasos) = coser(fotos);
        assert_eq!(
            pasos,
            vec![
                Paso::Cosida(120),
                Paso::Cosida(230),
                Paso::Cosida(1),
                Paso::Cosida(349),
                Paso::Cosida(300),
                Paso::Cosida(200),
                Paso::Cosida(200),
            ]
        );
        assert_eq!(imagen.dimensions(), (300, 2000));
        assert_eq!(imagen.as_raw(), p.as_raw(), "la página cosida no es la página");
    }

    #[test]
    fn la_cabecera_y_el_pie_fijos_salen_una_vez() {
        let p = pagina(320, 1800);
        let (alto, cabecera, pie) = (500, 60, 40);
        let posiciones = [0, 150, 300, 450, 600, 750, 900, 1000, 1200, 1300 - 0];
        let ultima = 1800 - (alto - cabecera - pie);
        let mut fotos: Vec<RgbaImage> = posiciones.iter().map(|&y| vista(&p, y, alto, cabecera, pie)).collect();
        fotos.push(vista(&p, ultima - cabecera, alto, cabecera, pie));
        let (imagen, pasos) = coser(fotos);
        assert!(pasos.iter().all(|p| matches!(p, Paso::Cosida(_))), "{pasos:?}");

        // Lo que tendría que salir: la cabecera, la página entera desde donde la enseña la
        // primera foto, y el pie.
        let alto_esperado = cabecera + (ultima - cabecera + alto - cabecera - pie) + pie;
        assert_eq!(imagen.height(), alto_esperado);
        let esperada = RgbaImage::from_fn(320, alto_esperado, |x, f| {
            if f < cabecera {
                Rgba([20, 20, 30, 255])
            } else if f >= alto_esperado - pie {
                if (x / 9) % 2 == 0 { Rgba([240, 200, 40, 255]) } else { Rgba([30, 30, 30, 255]) }
            } else {
                *p.get_pixel(x, f)
            }
        });
        assert_eq!(imagen.as_raw(), esperada.as_raw(), "costura repetida o fila perdida");
    }

    #[test]
    fn la_misma_foto_otra_vez_no_cose_nada() {
        let p = pagina(200, 900);
        let mut cosedor = Cosedor::nuevo(vista(&p, 100, 400, 30, 0));
        assert_eq!(cosedor.anadir(vista(&p, 100, 400, 30, 0)), Paso::Quieta);
        assert_eq!(cosedor.alto(), 400);
    }

    #[test]
    fn hacia_atras_se_ignora_y_al_volver_sigue() {
        let p = pagina(240, 1500);
        let mut cosedor = Cosedor::nuevo(vista(&p, 300, 500, 0, 0));
        assert_eq!(cosedor.anadir(vista(&p, 500, 500, 0, 0)), Paso::Cosida(200));
        assert_eq!(cosedor.anadir(vista(&p, 380, 500, 0, 0)), Paso::Atras);
        assert_eq!(cosedor.anadir(vista(&p, 650, 500, 0, 0)), Paso::Cosida(150));
        let imagen = cosedor.imagen();
        assert_eq!(imagen.height(), 850);
        let esperada = image::imageops::crop_imm(&p, 0, 300, 240, 850).to_image();
        assert_eq!(imagen.as_raw(), esperada.as_raw());
    }

    #[test]
    fn un_salto_sin_nada_en_comun_se_pierde() {
        let p = pagina(240, 3000);
        let mut cosedor = Cosedor::nuevo(vista(&p, 0, 400, 0, 0));
        assert_eq!(cosedor.anadir(vista(&p, 1500, 400, 0, 0)), Paso::Perdida);
        assert_eq!(cosedor.alto(), 400, "lo perdido no se pega");
    }

    #[test]
    fn no_crece_por_encima_del_maximo() {
        let p = pagina(64, 4000);
        let mut cosedor = Cosedor::nuevo(vista(&p, 0, 300, 0, 0));
        cosedor.alto_maximo = 700;
        assert_eq!(cosedor.anadir(vista(&p, 200, 300, 0, 0)), Paso::Cosida(200));
        assert_eq!(cosedor.anadir(vista(&p, 400, 300, 0, 0)), Paso::Cosida(200));
        assert_eq!(cosedor.anadir(vista(&p, 600, 300, 0, 0)), Paso::Llena);
        assert_eq!(cosedor.alto(), 700);
    }

    #[test]
    fn el_maximo_baja_con_una_zona_muy_ancha() {
        let cosedor = Cosedor::nuevo(RgbaImage::new(4000, 100));
        assert_eq!(cosedor.alto_maximo, 12_000, "48 millones de píxeles entre 4000 de ancho");
        let estrecha = Cosedor::nuevo(RgbaImage::new(800, 100));
        assert_eq!(estrecha.alto_maximo, ALTO_MAXIMO);
    }

    /// Lo que se espera de cada foto según dónde estaba la página: quieta si no se movió,
    /// hacia atrás si subió, y cosida con lo que bajó desde la última que se coció.
    fn pasos_esperados(posiciones: &[f64], escala: f64) -> Vec<Paso> {
        let mut referencia = posiciones[0];
        posiciones[1..]
            .iter()
            .map(|&y| {
                if y == referencia {
                    Paso::Quieta
                } else if y < referencia {
                    Paso::Atras
                } else {
                    let d = ((y - referencia) * escala).round() as u32;
                    referencia = y;
                    Paso::Cosida(d)
                }
            })
            .collect()
    }

    /// LA prueba de que cose bien: fotos de una página de verdad en Edge a distintas
    /// alturas (`scripts/fotos-scroll.mjs`), cosidas, tienen que dar la misma página que
    /// el propio Edge fotografía de un tirón. Con y sin cabecera y pie fijos, y al 125 %.
    ///
    /// Si falla, deja la cosida en la carpeta temporal para poder mirarla.
    #[test]
    fn fotos_de_verdad_de_una_pagina_larga() {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("scroll");
        for escena in ["sin-fijos", "cabecera-y-pie", "escala-125"] {
            let carpeta = raiz.join(escena);
            let abrir = |nombre: &str| {
                image::open(carpeta.join(nombre))
                    .unwrap_or_else(|e| panic!("{escena}/{nombre}: {e}"))
                    .to_rgba8()
            };
            let datos: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(carpeta.join("posiciones.json")).expect("posiciones.json"),
            )
            .expect("posiciones.json no es JSON");
            let escala = datos["escala"].as_f64().expect("escala");
            let posiciones: Vec<f64> = datos["posiciones"]
                .as_array()
                .expect("posiciones")
                .iter()
                .map(|v| v.as_f64().expect("posición"))
                .collect();

            let mut cosedor = Cosedor::nuevo(abrir("00.png"));
            let pasos: Vec<Paso> = (1..posiciones.len())
                .map(|i| cosedor.anadir(abrir(&format!("{i:02}.png"))))
                .collect();
            let cosida = cosedor.imagen();
            let entera = abrir("entera.png");

            let guardar_si_falla = || {
                let ruta = std::env::temp_dir().join(format!("winshotx-scroll-{escena}.png"));
                let _ = cosida.save(&ruta);
                ruta
            };
            assert_eq!(
                pasos,
                pasos_esperados(&posiciones, escala),
                "{escena}: cada foto tenía que bajar lo que bajó la página ({})",
                guardar_si_falla().display()
            );
            assert_eq!(
                cosida.dimensions(),
                entera.dimensions(),
                "{escena}: la cosida no mide lo que la página entera ({})",
                guardar_si_falla().display()
            );
            let distintos = pixeles_distintos(cosida.as_raw(), entera.as_raw());
            let parte = distintos as f64 / (entera.width() * entera.height()) as f64;
            println!(
                "{escena}: {} fotos, {}x{} px, {distintos} píxeles distintos de la página entera",
                posiciones.len(),
                cosida.width(),
                cosida.height()
            );
            assert!(
                parte < 0.001,
                "{escena}: {distintos} píxeles distintos de la página entera ({:.3} %): costura \
                 repetida o fila perdida ({})",
                parte * 100.0,
                guardar_si_falla().display()
            );
        }
    }

    /// Lo cosido llega al editor como una foto (`recorder::session_from_image`), y de ahí
    /// sale por «Guardar». Aquí se hace ese mismo viaje con una imagen larga de verdad: la
    /// caché de fotogramas, la miniatura, y el PNG por `exporter::escribir`, que es lo que
    /// hace «Guardar» (también enmarcar y anotar). El PNG tiene que ser la imagen cosida,
    /// píxel a píxel.
    #[test]
    fn lo_cosido_sobrevive_a_la_cache_y_al_png() {
        let carpeta = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("scroll")
            .join("cabecera-y-pie");
        let abrir = |n: usize| image::open(carpeta.join(format!("{n:02}.png"))).unwrap().to_rgba8();
        let mut cosedor = Cosedor::nuevo(abrir(0));
        for n in 1..14 {
            cosedor.anadir(abrir(n));
        }
        let cosida = cosedor.imagen();
        assert_eq!(cosida.dimensions(), (480, 2172));

        let dir = std::env::temp_dir().join(format!("winshotx-scroll-sesion-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cache = crate::record::FrameCache::new(&dir).unwrap();
        cache.push_rgba(cosida.as_raw(), cosida.width(), cosida.height(), 0).unwrap();
        let mut sesion = crate::record::SessionData {
            id: "scroll".into(),
            dir: dir.clone(),
            region: crate::capture::Rect { x: 0, y: 0, width: cosida.width(), height: cosida.height() },
            fps: 1,
            format: "still".into(),
            has_audio: false,
            audio: None,
            clics: Vec::new(),
            cursor_capturado: false,
            formas: Vec::new(),
            punteros: Vec::new(),
            cambios_puntero: Vec::new(),
            marcas_zoom: Vec::new(),
            camara: None,
            escritura: Vec::new(),
            pistas: None,
            teclas: Vec::new(),
            cursor: Vec::new(),
            width: cosida.width(),
            height: cosida.height(),
            mp4_path: None,
            frames: cache.finish(0, 1).unwrap(),
        };
        crate::record::generate_thumbnails(&mut sesion).unwrap();

        let leida = crate::record::read_frame(&sesion, 0).unwrap();
        assert_eq!(leida.as_raw(), cosida.as_raw(), "la caché ha cambiado la imagen");
        let png = dir.join("cosida.png");
        let mut peticion = crate::recorder::peticion_por_defecto(&sesion);
        peticion.format = "png".into();
        crate::exporter::escribir(&sesion, &peticion, &png, &|_, _, _| {}).unwrap();
        let releida = image::open(&png).unwrap().to_rgba8();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(releida.dimensions(), (480, 2172));
        assert_eq!(releida.as_raw(), cosida.as_raw(), "el PNG no es la imagen cosida");
    }

    /// Lo que tarda coser una foto del tamaño de una ventana de navegador de verdad: la
    /// captura baja sola a unas diez fotos por segundo, y coser no puede ser lo que la frene.
    /// `cargo test --release --lib medir_el_cosido -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn medir_el_cosido() {
        let (ancho, alto) = (1600, 1000);
        let p = pagina(ancho, 9000);
        let mut cosedor = Cosedor::nuevo(vista(&p, 0, alto, 80, 50));
        let inicio = std::time::Instant::now();
        let mut fotos = 0;
        for y in (550..8000).step_by(550) {
            assert!(matches!(cosedor.anadir(vista(&p, y, alto, 80, 50)), Paso::Cosida(550)));
            fotos += 1;
        }
        let por_foto = inicio.elapsed().as_secs_f64() * 1000.0 / fotos as f64;
        let quieta = std::time::Instant::now();
        assert_eq!(cosedor.anadir(vista(&p, 7700, alto, 80, 50)), Paso::Quieta);
        println!(
            "{fotos} fotos de {ancho}x{alto}: {por_foto:.1} ms por foto cosida, {:.1} ms una quieta; {} px de alto",
            quieta.elapsed().as_secs_f64() * 1000.0,
            cosedor.alto()
        );
    }

    #[test]
    fn la_barra_va_debajo_encima_o_dentro() {
        let pantalla = Some((0, 0, 1920, 1080));
        let zona = |y, alto| super::super::Rect { x: 400, y, width: 800, height: alto };
        assert_eq!(sitio_de_la_barra(zona(100, 500), pantalla, 380, 52, 12), (610, 612));
        assert_eq!(sitio_de_la_barra(zona(400, 660), pantalla, 380, 52, 12), (610, 336));
        assert_eq!(sitio_de_la_barra(zona(0, 1080), pantalla, 380, 52, 12), (610, 1016));
    }

    /// Lo que tarda el desplazamiento suave en llegar, como el de Edge.
    const ANIMACION: f64 = 0.2;

    /// Una página de mentira que baja como un navegador: con desplazamiento suave, lo que
    /// baje cada giro a su manera, y fotos que tardan. El reloj es suyo, así que la prueba
    /// no espera nada de verdad.
    struct Simulada {
        pagina: RgbaImage,
        alto: u32,
        /// Lo que bajan esas muescas en el giro número tantos, en píxeles.
        bajada: Box<dyn FnMut(usize, i32) -> i32>,
        desde: f64,
        hasta: f64,
        empezo: Duration,
        reloj: Duration,
        tarda_foto: Duration,
        parar_en: Option<Duration>,
        raton_en: Option<Duration>,
        /// Cada giro: cuándo y cuántas muescas.
        giros: Vec<(Duration, i32)>,
    }

    impl Simulada {
        fn nueva(pagina: RgbaImage, alto: u32, bajada: impl FnMut(usize, i32) -> i32 + 'static) -> Self {
            Self {
                pagina,
                alto,
                bajada: Box::new(bajada),
                desde: 0.0,
                hasta: 0.0,
                empezo: Duration::ZERO,
                reloj: Duration::ZERO,
                tarda_foto: Duration::from_millis(30),
                parar_en: None,
                raton_en: None,
                giros: Vec::new(),
            }
        }

        fn donde(&self) -> f64 {
            let t = ((self.reloj.saturating_sub(self.empezo)).as_secs_f64() / ANIMACION).clamp(0.0, 1.0);
            let suave = 1.0 - (1.0 - t).powi(3);
            self.desde + (self.hasta - self.desde) * suave
        }

        fn empezar(&mut self) -> Cosedor {
            Cosedor::nuevo(self.foto().unwrap())
        }
    }

    impl Pantalla for Simulada {
        fn foto(&mut self) -> crate::error::Result<RgbaImage> {
            self.reloj += self.tarda_foto;
            let y = self.donde().round() as u32;
            Ok(image::imageops::crop_imm(&self.pagina, 0, y, self.pagina.width(), self.alto).to_image())
        }

        fn rueda(&mut self, muescas: i32) {
            let ahora = self.donde();
            let tope = f64::from(self.pagina.height() - self.alto);
            let px = (self.bajada)(self.giros.len(), muescas);
            self.giros.push((self.reloj, muescas));
            self.hasta = (self.hasta + f64::from(px)).clamp(0.0, tope);
            self.desde = ahora;
            self.empezo = self.reloj;
        }

        fn esperar(&mut self, tiempo: Duration) {
            self.reloj += tiempo;
        }

        fn reloj(&self) -> Duration {
            self.reloj
        }

        fn raton_movido(&mut self) -> bool {
            self.raton_en.is_some_and(|t| self.reloj >= t)
        }

        fn parar(&mut self) -> bool {
            // Y a los cinco minutos de reloj de mentira, siempre: una prueba que se queda
            // esperando a quien captura no puede colgarse.
            self.parar_en.is_some_and(|t| self.reloj >= t) || self.reloj > Duration::from_secs(300)
        }

        fn avisar(&mut self, _alto: u32, _estado: &'static str) {}
    }

    /// Como Edge al 125 %: 125 px por muesca, salvo que tres bajan 250 y cuatro, 500.
    fn como_edge(_giro: usize, muescas: i32) -> i32 {
        let px = match muescas.abs() {
            3 => 250,
            4 => 500,
            n => 125 * n,
        };
        px * muescas.signum()
    }

    #[test]
    fn baja_sola_hasta_el_final_y_sale_la_pagina_entera() {
        let p = pagina(240, 6000);
        let mut pantalla = Simulada::nueva(p.clone(), 630, como_edge);
        let mut cosedor = pantalla.empezar();
        bajar(&mut pantalla, &mut cosedor);
        let imagen = cosedor.imagen();
        assert_eq!(imagen.dimensions(), (240, 6000), "giros: {:?}", pantalla.giros);
        assert_eq!(imagen.as_raw(), p.as_raw(), "la página cosida no es la página");
        assert!(
            pantalla.giros.iter().all(|&(_, m)| m > 0 && m <= 2),
            "con 125 px por muesca y 630 de zona, más de dos muescas se pasa de media zona: {:?}",
            pantalla.giros
        );
    }

    /// Lo que pasó el 11 de noviembre de 2026: una vuelta baja mucho más de lo que se puede
    /// coser. Antes se perdía, y dos vueltas perdidas parecían el final de la página: la
    /// imagen se quedaba en las dos primeras pantallas.
    #[test]
    fn si_una_vuelta_se_pasa_sube_y_sigue_hasta_el_final() {
        let p = pagina(240, 5000);
        // El cuarto giro baja cuatro veces más de la cuenta: 1000 px en una zona de 630.
        let mut pantalla = Simulada::nueva(p.clone(), 630, |giro, muescas| {
            let px = como_edge(giro, muescas);
            if giro == 3 { px * 4 } else { px }
        });
        pantalla.tarda_foto = Duration::from_millis(120);
        let mut cosedor = pantalla.empezar();
        bajar(&mut pantalla, &mut cosedor);
        assert!(pantalla.giros.iter().any(|&(_, m)| m < 0), "no ha subido: {:?}", pantalla.giros);
        let imagen = cosedor.imagen();
        assert_eq!(imagen.dimensions(), (240, 5000), "giros: {:?}", pantalla.giros);
        assert_eq!(imagen.as_raw(), p.as_raw(), "la página cosida no es la página");
    }

    #[test]
    fn se_para_cuando_se_pide_y_se_queda_con_lo_que_lleva() {
        let p = pagina(240, 20_000);
        let mut pantalla = Simulada::nueva(p.clone(), 630, como_edge);
        pantalla.parar_en = Some(Duration::from_secs(3));
        let mut cosedor = pantalla.empezar();
        bajar(&mut pantalla, &mut cosedor);
        assert!(
            pantalla.reloj < Duration::from_millis(3200),
            "ha tardado {:?} en parar",
            pantalla.reloj
        );
        let imagen = cosedor.imagen();
        assert!(imagen.height() > 630 && imagen.height() < 20_000, "{}", imagen.height());
        let principio = image::imageops::crop_imm(&p, 0, 0, 240, imagen.height()).to_image();
        assert_eq!(imagen.as_raw(), principio.as_raw(), "lo que lleva no es el principio de la página");
    }

    #[test]
    fn si_quien_captura_coge_el_raton_la_rueda_deja_de_girar() {
        let p = pagina(240, 20_000);
        let mut pantalla = Simulada::nueva(p, 630, como_edge);
        pantalla.raton_en = Some(Duration::from_secs(1));
        pantalla.parar_en = Some(Duration::from_secs(4));
        let mut cosedor = pantalla.empezar();
        bajar(&mut pantalla, &mut cosedor);
        assert!(!pantalla.giros.is_empty());
        // El giro que ya estaba en marcha acaba; después de esa vuelta no gira más.
        assert!(
            pantalla.giros.iter().all(|&(t, _)| t < Duration::from_millis(1600)),
            "{:?}",
            pantalla.giros
        );
    }

    #[test]
    fn una_rueda_que_no_mueve_nada_no_cose_nada() {
        let p = pagina(240, 3000);
        let mut pantalla = Simulada::nueva(p, 630, |_, _| 0);
        pantalla.parar_en = Some(Duration::from_secs(10));
        let mut cosedor = pantalla.empezar();
        bajar(&mut pantalla, &mut cosedor);
        assert_eq!(cosedor.alto(), 630);
        assert_eq!(pantalla.giros.len(), 2, "tras dos giros en vano, la rueda la pone quien captura");
    }
}
