//! Escribir letras fuera de Windows, con las fuentes que ya tiene el sistema.
//!
//! En Windows las escribe GDI (`pastilla.rs`). En Linux no hay GDI, así que las fuentes se
//! buscan con `fontdb`, que lee la configuración de fontconfig sin cargar su biblioteca de
//! C, y se dibujan con `ab_glyph`. Son dos crates pequeños de Rust puro. Un motor de texto
//! entero (pango, cosmic-text) solo haría falta para árabe o ligaduras, y aquí se escriben
//! atajos de teclado, el número de un paso y notas cortas.
//!
//! **Las fuentes se cargan una sola vez, la primera vez que hacen falta.** Leer las del
//! sistema es abrir cientos de archivos, y la pastilla se pide en mitad de una exportación:
//! se paga una vez y luego se reusa.
//!
//! **Lo que no tiene la fuente se busca en las demás, letra a letra.** Sin eso, una nota en
//! chino sobre una fuente latina sale con huecos. Lo que no tiene ninguna fuente del sistema
//! se salta: mejor una letra de menos que una exportación que revienta.
//!
//! Solo Linux: en macOS no hay dependencias de fuentes (ver `Cargo.toml`) y la pastilla se
//! queda sin dibujar, como antes.
#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use ab_glyph::{
    point, Font, FontArc, FontRef, FontVec, Glyph, GlyphId, PxScale, ScaleFont, VariableFont,
};

/// Seminegrita, como el `FW_SEMIBOLD` con que escribe GDI.
const PESO: u16 = 600;

/// Las familias que se prueban para escribir, por orden. Segoe UI primero por si alguien la
/// tiene instalada (así sale igual que en Windows), y detrás las de los escritorios de Linux
/// más comunes. Si no hay ninguna, la sans-serif que diga fontconfig.
const FAMILIAS: [&str; 7] = [
    "Segoe UI",
    "Noto Sans",
    "Adwaita Sans",
    "Cantarell",
    "Ubuntu",
    "DejaVu Sans",
    "Liberation Sans",
];

/// Dónde se busca primero lo que no tiene la principal: las que cubren chino, japonés y
/// coreano, y las de símbolos. Empezar por aquí es lo barato, porque recorrer todas las
/// fuentes del sistema por una letra es abrir cientos de archivos.
const RESERVAS: [&str; 11] = [
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Source Han Sans SC",
    "Droid Sans Fallback",
    "WenQuanYi Zen Hei",
    "WenQuanYi Micro Hei",
    "Noto Sans Symbols",
    "Noto Sans Symbols 2",
    "Noto Emoji",
    "Symbola",
    "DejaVu Sans",
];

/// Un texto ya repartido en glifos, con lo que mide.
///
/// Medir y pintar salen del mismo reparto para que la caja y lo pintado no puedan dejar de
/// cuadrar: con GDI eran dos llamadas que miraban la misma fuente, aquí es una sola cuenta.
pub struct Renglon {
    piezas: Vec<(Cara, Glyph)>,
    /// La suma de los avances, como el `cx` de `GetTextExtentPoint32W`.
    pub ancho: i32,
    /// El alto de celda (ascendente más descendente de la principal), como su `cy`.
    pub alto: i32,
}

impl Renglon {
    /// Llama a `punto(x, y, cobertura)` por cada píxel que tocan los glifos, con (0, 0) en
    /// la esquina de arriba a la izquierda de la celda. La cobertura va de 0 a 1, y puede
    /// salirse de la caja: una letra que cuelga (la cola de una «f») cae en el aire de
    /// alrededor, igual que con GDI.
    pub fn pintar(&self, mut punto: impl FnMut(i32, i32, f32)) {
        for (cara, glifo) in &self.piezas {
            let Some(contorno) = cara.fuente.outline_glyph(glifo.clone()) else {
                continue;
            };
            let caja = contorno.px_bounds();
            let (x0, y0) = (caja.min.x as i32, caja.min.y as i32);
            contorno.draw(|x, y, cobertura| punto(x0 + x as i32, y0 + y as i32, cobertura));
        }
    }
}

/// Reparte el texto en glifos, con `alto_letra` como alto de celda.
///
/// Es lo que hace `CreateFontW` con un alto positivo: ese alto es el ascendente más el
/// descendente, no el em. En `ab_glyph` eso es justo `PxScale`, que mide el alto de celda.
///
/// `None` solo si el sistema no tiene ninguna fuente que se pueda usar.
pub fn componer(texto: &str, alto_letra: i32) -> Option<Renglon> {
    let letras = letras()?;
    let alto_letra = alto_letra.max(1) as f32;
    let principal = letras.principal.fuente.as_scaled(PxScale::from(alto_letra));
    // Redondeados por separado, como los `tmAscent` y `tmDescent` de GDI: la línea base cae
    // en un píxel entero y la celda mide lo que suman.
    let ascendente = principal.ascent().round();
    let descendente = (-principal.descent()).round();

    let mut piezas = Vec::new();
    let mut pluma = 0.0f32;
    let mut anterior: Option<(usize, GlyphId)> = None;
    for letra in texto.chars() {
        // Un salto de línea o un tabulador no se dibujan: esto escribe un renglón.
        if letra.is_control() {
            continue;
        }
        // Si no la tiene nadie se salta, y la de antes y la de después se juntan como si no
        // estuviera.
        let Some((cara, id)) = letras.cara_para(letra) else {
            continue;
        };
        // Las de otra fuente, al mismo em que la principal: con el mismo alto de celda, un
        // carácter chino saldría bastante más grande que las letras de al lado.
        let escala = PxScale::from(alto_letra * cara.celda_por_em / letras.principal.celda_por_em);
        let escalada = cara.fuente.as_scaled(escala);
        // Kerning solo entre dos letras de la misma fuente: entre fuentes no hay tabla.
        if let Some((orden, previo)) = anterior {
            if orden == cara.orden {
                pluma += escalada.kern(previo, id);
            }
        }
        // Cada letra empieza en un píxel entero, como con GDI, que redondea los avances: a
        // medio píxel la misma letra sale emborronada distinta cada vez que aparece.
        let glifo = id.with_scale_and_position(escala, point(pluma.round(), ascendente));
        pluma += escalada.h_advance(id);
        anterior = Some((cara.orden, id));
        piezas.push((cara, glifo));
    }
    Some(Renglon {
        piezas,
        ancho: pluma.max(0.0).ceil() as i32,
        alto: (ascendente + descendente) as i32,
    })
}

/// Una fuente ya cargada, lista para escribir.
#[derive(Clone)]
struct Cara {
    fuente: FontArc,
    /// Su número: 0 la principal, y las de reserva detrás, por orden de llegada. Sirve para
    /// saber si dos letras seguidas son de la misma fuente.
    orden: usize,
    /// Alto de celda por cada em. Con esto, una letra de otra fuente sale del mismo tamaño
    /// de em que la principal, que es lo que hace que se vean del mismo cuerpo.
    celda_por_em: f32,
}

/// Todas las fuentes del sistema, y las que ya se han cargado de verdad.
struct Letras {
    base: fontdb::Database,
    id_principal: fontdb::ID,
    principal: Cara,
    reservas: Mutex<Reservas>,
}

/// Las de reserva ya cargadas, y qué fuente le tocó a cada letra que no tenía la principal.
///
/// Va tras un candado porque se pide desde varios hilos a la vez (la exportación pinta en
/// paralelo), y se apunta también lo que no se encontró: si no, cada aparición de una letra
/// que no tiene nadie volvería a recorrer todas las fuentes del sistema.
#[derive(Default)]
struct Reservas {
    cargadas: Vec<(fontdb::ID, Cara)>,
    por_letra: HashMap<char, Option<usize>>,
}

/// Las fuentes, cargadas la primera vez que se piden. `None` si el sistema no tiene ninguna.
fn letras() -> Option<&'static Letras> {
    static LETRAS: OnceLock<Option<Letras>> = OnceLock::new();
    LETRAS.get_or_init(Letras::cargar).as_ref()
}

impl Letras {
    fn cargar() -> Option<Letras> {
        let mut base = fontdb::Database::new();
        base.load_system_fonts();
        let (id_principal, principal) = elegir_principal(&base)?;
        Some(Letras {
            base,
            id_principal,
            principal,
            reservas: Mutex::new(Reservas::default()),
        })
    }

    /// La fuente con que se escribe esa letra y su glifo en ella: la principal si la tiene,
    /// y si no, la primera de las demás que la tenga. `None` si no la tiene ninguna.
    fn cara_para(&self, letra: char) -> Option<(Cara, GlyphId)> {
        let id = self.principal.fuente.glyph_id(letra);
        if id.0 != 0 {
            return Some((self.principal.clone(), id));
        }
        let cara = self.reserva_para(letra)?;
        let id = cara.fuente.glyph_id(letra);
        Some((cara, id))
    }

    fn reserva_para(&self, letra: char) -> Option<Cara> {
        // Un hilo que reventó con el candado puesto no deja nada a medias que importe: como
        // mucho, una fuente cargada que nadie apuntó. Se sigue usando.
        let mut reservas = self.reservas.lock().unwrap_or_else(PoisonError::into_inner);
        let hallada = match reservas.por_letra.get(&letra) {
            Some(&hallada) => hallada,
            None => {
                let hallada = self.buscar(&mut reservas, letra);
                reservas.por_letra.insert(letra, hallada);
                hallada
            }
        };
        hallada.map(|i| reservas.cargadas[i].1.clone())
    }

    /// Busca una fuente con esa letra: primero entre las ya cargadas, luego en las de
    /// `RESERVAS` y al final en todas las del sistema, de las más parecidas a la principal
    /// a las menos. La que la tiene se carga y se queda para las siguientes.
    fn buscar(&self, reservas: &mut Reservas, letra: char) -> Option<usize> {
        let ya_cargada = reservas.cargadas.iter().position(|(_, cara)| tiene(&cara.fuente, letra));
        if ya_cargada.is_some() {
            return ya_cargada;
        }
        let nombradas = RESERVAS
            .iter()
            .filter_map(|nombre| mejor_de_la_familia(&self.base, nombre));
        // Todas las del sistema solo se ordenan si las de la lista no bastan.
        let todas = std::iter::once(()).flat_map(|_| por_parecido(&self.base));
        for id in nombradas.chain(todas) {
            if id == self.id_principal || reservas.cargadas.iter().any(|(ya, _)| *ya == id) {
                continue;
            }
            // Mirar si la tiene no copia nada: fontdb presta el archivo un momento.
            let la_tiene = self
                .base
                .with_face_data(id, |datos, indice| {
                    FontRef::try_from_slice_and_index(datos, indice).is_ok_and(|f| tiene(&f, letra))
                })
                .unwrap_or(false);
            if !la_tiene {
                continue;
            }
            let Some(cara) = cargar(&self.base, id, reservas.cargadas.len() + 1) else {
                continue;
            };
            reservas.cargadas.push((id, cara));
            return Some(reservas.cargadas.len() - 1);
        }
        None
    }
}

/// La fuente principal: la primera de `FAMILIAS` que haya, luego la sans-serif de
/// fontconfig, y si tampoco, cualquiera que sepa escribir una A. Un texto en otra letra es
/// mejor que ninguno.
fn elegir_principal(base: &fontdb::Database) -> Option<(fontdb::ID, Cara)> {
    let sans = base.family_name(&fontdb::Family::SansSerif).to_string();
    let por_nombre = FAMILIAS
        .iter()
        .copied()
        .chain(std::iter::once(sans.as_str()))
        .filter_map(|nombre| mejor_de_la_familia(base, nombre));
    let cualquiera = std::iter::once(())
        .flat_map(|_| por_parecido(base))
        .filter(|id| {
            base.with_face_data(*id, |datos, indice| {
                FontRef::try_from_slice_and_index(datos, indice).is_ok_and(|f| tiene(&f, 'A'))
            })
            .unwrap_or(false)
        });
    por_nombre
        .chain(cualquiera)
        .find_map(|id| cargar(base, id, 0).map(|cara| (id, cara)))
}

/// La cara de esa familia que mejor sirve: derecha antes que cursiva, de ancho normal, y la
/// más cercana al peso 600.
fn mejor_de_la_familia(base: &fontdb::Database, nombre: &str) -> Option<fontdb::ID> {
    base.faces()
        .filter(|cara| cara.families.iter().any(|(familia, _)| familia == nombre))
        .min_by_key(|cara| {
            (
                cara.style != fontdb::Style::Normal,
                cara.stretch != fontdb::Stretch::Normal,
                lejos_del_peso(base, cara),
            )
        })
        .map(|cara| cara.id)
}

/// Cuánto se aleja una cara del peso 600.
///
/// No se usa la consulta de fontdb porque sigue la regla de CSS, que con un 500 y un 800
/// se queda el 800, y porque no sabe de fuentes variables: de una así solo ve el peso con
/// que viene de fábrica (el 400 de la Noto Sans de Fedora, que es una sola con el eje de
/// peso). Una variable cuyo eje llega al 600 cuenta como exacta. Entre dos igual de lejos
/// gana la más gruesa.
fn lejos_del_peso(base: &fontdb::Database, cara: &fontdb::FaceInfo) -> u32 {
    let llega = base
        .with_face_data(cara.id, |datos, indice| {
            FontRef::try_from_slice_and_index(datos, indice).is_ok_and(|fuente| {
                eje_de_peso(&fuente)
                    .is_some_and(|(menos, mas)| (menos..=mas).contains(&f32::from(PESO)))
            })
        })
        .unwrap_or(false);
    if llega {
        return 0;
    }
    let peso = cara.weight.0;
    u32::from(peso.abs_diff(PESO)) * 2 + u32::from(peso < PESO)
}

/// Todas las caras del sistema, de las que más se parecen a la principal a las que menos:
/// derechas, de ancho normal, no de máquina de escribir, y cerca del peso 600.
fn por_parecido(base: &fontdb::Database) -> Vec<fontdb::ID> {
    let mut caras: Vec<&fontdb::FaceInfo> = base.faces().collect();
    caras.sort_by_key(|cara| {
        (
            cara.style != fontdb::Style::Normal,
            cara.stretch != fontdb::Stretch::Normal,
            cara.monospaced,
            cara.weight.0.abs_diff(PESO),
        )
    });
    caras.into_iter().map(|cara| cara.id).collect()
}

/// Lo que va de un extremo al otro del eje de peso, si la fuente es variable y lo tiene.
fn eje_de_peso(fuente: &impl VariableFont) -> Option<(f32, f32)> {
    fuente
        .variations()
        .into_iter()
        .find(|eje| &eje.tag == b"wght")
        .map(|eje| (eje.min_value, eje.max_value))
}

/// Si la fuente sabe dibujar esa letra.
///
/// Tener el número del glifo no basta: las fuentes de emoji en color lo traen, pero con un
/// dibujo hecho de píxeles y sin contorno, y `ab_glyph` solo pinta contornos. Un hueco sí
/// vale sin contorno, porque no se pinta, solo ocupa.
fn tiene(fuente: &impl Font, letra: char) -> bool {
    let id = fuente.glyph_id(letra);
    id.0 != 0 && (letra.is_whitespace() || fuente.outline(id).is_some())
}

/// Carga esa cara para escribir con ella, en seminegrita si es variable.
///
/// Se copia el archivo a memoria (`FontVec`): `ab_glyph` necesita los bytes para siempre, y
/// fontdb solo los presta mientras dura la llamada. Una fuente latina son unos cientos de KB;
/// las de chino, varios MB, y solo se cargan si alguien escribe en chino.
fn cargar(base: &fontdb::Database, id: fontdb::ID, orden: usize) -> Option<Cara> {
    let mut fuente = base
        .with_face_data(id, |datos, indice| {
            FontVec::try_from_vec_and_index(datos.to_vec(), indice).ok()
        })
        .flatten()?;
    // Una variable viene con el peso de fábrica, casi siempre el normal: se le pone el 600,
    // o lo más cerca que admita su eje.
    if let Some((menos, mas)) = eje_de_peso(&fuente) {
        fuente.set_variation(b"wght", f32::from(PESO).clamp(menos, mas));
    }
    let celda_por_em = fuente.height_unscaled() / fuente.units_per_em()?;
    (celda_por_em.is_finite() && celda_por_em > 0.0).then(|| Cara {
        fuente: FontArc::new(fuente),
        orden,
        celda_por_em,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que mancha un renglón: la suma de la cobertura de todos sus píxeles.
    fn tinta(renglon: &Renglon) -> f32 {
        let mut total = 0.0;
        renglon.pintar(|_, _, cobertura| total += cobertura.clamp(0.0, 1.0));
        total
    }

    /// Sin fuentes estas pruebas no comprueban nada, y pasar en verde sin mirar es peor que
    /// fallar: en la CI eso quiere decir que falta el paquete de fuentes.
    fn las_letras() -> &'static Letras {
        letras().expect("este sistema no tiene ninguna fuente (en Debian: fonts-dejavu-core)")
    }

    /// El alto que se pide es el de la celda, como con `CreateFontW`: la caja mide eso.
    #[test]
    fn la_celda_mide_el_alto_pedido() {
        las_letras();
        for alto in [14, 22, 37, 100] {
            let renglon = componer("Ag", alto).expect("componer");
            assert!(
                (renglon.alto - alto).abs() <= 1,
                "se pidió una celda de {alto} y mide {}",
                renglon.alto
            );
            assert!(renglon.ancho > 0, "«Ag» no puede medir cero de ancho");
        }
    }

    /// `PxScale` es el alto de celda, medido: la H que se pinta a 100 mide lo que dice su
    /// contorno escalado por la celda, y no lo que mediría escalado por el em.
    #[test]
    fn la_escala_es_la_de_la_celda_y_no_la_del_em() {
        let letras = las_letras();
        let fuente = &letras.principal.fuente;
        let contorno = fuente.outline(fuente.glyph_id('H')).expect("la principal tiene H");
        // En unidades de la fuente el `min` lleva el y de arriba y el `max` el de abajo.
        let unidades = contorno.bounds.min.y - contorno.bounds.max.y;
        let por_celda = unidades * 100.0 / fuente.height_unscaled();
        let por_em = unidades * 100.0 / fuente.units_per_em().expect("unidades por em");

        let mut filas = std::collections::BTreeSet::new();
        componer("H", 100).expect("componer").pintar(|_, y, cobertura| {
            if cobertura > 0.0 {
                filas.insert(y);
            }
        });
        let medido = filas.len() as f32;
        eprintln!("H a 100: {medido} filas; por celda serían {por_celda:.1} y por em {por_em:.1}");
        assert!(
            (medido - por_celda).abs() <= 1.5,
            "la H mide {medido} filas y a escala de celda tendría que medir {por_celda:.1}"
        );
    }

    /// Seminegrita de verdad: con el mismo texto y el mismo tamaño, la principal mancha más
    /// que la cara normal de su misma familia.
    #[test]
    fn la_principal_es_mas_gruesa_que_la_normal() {
        let letras = las_letras();
        eprintln!(
            "la principal es {:?}",
            letras.base.face(letras.id_principal).map(|c| (&c.post_script_name, c.weight.0))
        );
        let familia = letras
            .base
            .face(letras.id_principal)
            .and_then(|cara| cara.families.first())
            .map(|(nombre, _)| nombre.clone())
            .expect("la principal tiene familia");
        // La cara más cercana al 400 de esa familia, cargada tal cual, sin tocarle el peso.
        let id_normal = letras
            .base
            .faces()
            .filter(|cara| cara.style == fontdb::Style::Normal)
            .filter(|cara| cara.families.iter().any(|(nombre, _)| *nombre == familia))
            .min_by_key(|cara| cara.weight.0.abs_diff(400))
            .map(|cara| cara.id)
            .expect("la familia de la principal tiene alguna cara derecha");
        let normal = letras
            .base
            .with_face_data(id_normal, |datos, indice| {
                FontVec::try_from_vec_and_index(datos.to_vec(), indice).ok()
            })
            .flatten()
            .expect("cargar la normal");
        if id_normal == letras.id_principal && eje_de_peso(&normal).is_none() {
            eprintln!("«{familia}» no tiene nada más grueso que la normal: nada que comparar");
            return;
        }
        let texto = "Illl Ctrl";
        let escala = PxScale::from(60.0);
        let mut de_la_normal = 0.0f32;
        let mut x = 0.0;
        for letra in texto.chars() {
            let id = normal.glyph_id(letra);
            let glifo = id.with_scale_and_position(escala, point(x, 50.0));
            x += normal.as_scaled(escala).h_advance(id);
            if let Some(contorno) = normal.outline_glyph(glifo) {
                contorno.draw(|_, _, c| de_la_normal += c.clamp(0.0, 1.0));
            }
        }
        let de_la_principal = tinta(&componer(texto, 60).expect("componer"));
        assert!(
            de_la_principal > de_la_normal * 1.08,
            "«{familia}» seminegrita mancha {de_la_principal:.0} y la normal {de_la_normal:.0}"
        );
    }

    /// Una letra que no tiene ninguna fuente se salta: no ocupa ni revienta.
    #[test]
    fn una_letra_que_no_tiene_nadie_se_salta() {
        las_letras();
        let con = componer("A\u{10FFFD}B", 22).expect("componer");
        let sin = componer("AB", 22).expect("componer");
        assert_eq!(con.ancho, sin.ancho, "la letra que no existe ha ocupado sitio");
        assert!((tinta(&con) - tinta(&sin)).abs() < 0.01, "y ha pintado algo");
    }

    /// Un carácter chino sale de otra fuente si el sistema tiene alguna que lo cubra, y si
    /// no, se salta sin reventar.
    #[test]
    fn un_caracter_chino_sale_de_otra_fuente_si_la_hay() {
        let letras = las_letras();
        let alguna = letras.base.faces().any(|cara| {
            letras
                .base
                .with_face_data(cara.id, |datos, indice| {
                    FontRef::try_from_slice_and_index(datos, indice).is_ok_and(|f| tiene(&f, '中'))
                })
                .unwrap_or(false)
        });
        let renglon = componer("中文", 40).expect("componer");
        if alguna {
            assert!(renglon.ancho > 40, "dos caracteres chinos a 40 px miden {}", renglon.ancho);
            let manchado = tinta(&renglon);
            assert!(manchado > 100.0, "los caracteres chinos no se han dibujado: {manchado:.0}");
            // Y del mismo cuerpo que la principal, no del alto de celda de la otra: un
            // carácter chino ocupa alrededor de un em de ancho.
            let em = 40.0 / letras.principal.celda_por_em;
            let por_letra = renglon.ancho as f32 / 2.0;
            assert!(
                (por_letra - em).abs() < em * 0.25,
                "cada carácter mide {por_letra:.1} px y el em de la principal es {em:.1}"
            );
        } else {
            eprintln!("ninguna fuente de este sistema tiene «中»: tiene que saltarse");
            assert_eq!(renglon.ancho, 0);
        }
    }
}
