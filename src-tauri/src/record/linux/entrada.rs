//! El raton y el teclado en X11, preguntando: que botones y que teclas estan abajo AHORA.
//!
//! Es lo que en Windows hace `GetAsyncKeyState`, y por la misma razon: se pregunta en cada
//! vuelta en vez de engancharse a los eventos de nadie (ver `raton.rs` y `teclas.rs`). X11 lo
//! contesta con dos preguntas, `QueryPointer` (donde esta el raton y que botones tiene abajo)
//! y `QueryKeymap` (que teclas), sin permiso especial y este donde este el foco.
//!
//! **Las teclas se traducen a los codigos de Windows (VK)**, que es el idioma de `teclas.rs`:
//! asi los atajos, la tecla de zoom y los ratos de escritura son el mismo codigo en los dos
//! sistemas. X11 numera las teclas por su sitio en el teclado (keycode) y dice que pone en
//! cada una (keysym); la tabla de keycode a VK se hace con el teclado que hay puesto, y se
//! rehace si cambia.
//!
//! **En Wayland no hay nada de esto, a proposito:** una app normal no puede leer el teclado
//! ni el raton de las demas. Y no vale preguntarle al XWayland que suele haber al lado: solo
//! sabe lo que pasa encima de las ventanas de X11, y saldrian clics y atajos a medias.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, KeyButMask, Window};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

/// Los codigos de Windows de una tecla. Casi siempre es uno; AltGr son cuatro, que es como
/// lo ve Windows (Ctrl y Alt a la vez, y cada uno con su lado). El 0 es «ninguno».
pub(crate) type Vks = [u16; 4];

const NADA: Vks = [0; 4];

const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_LWIN: u16 = 0x5B;
const VK_RWIN: u16 = 0x5C;
const VK_LCONTROL: u16 = 0xA2;
const VK_RCONTROL: u16 = 0xA3;
const VK_LMENU: u16 = 0xA4;
const VK_RMENU: u16 = 0xA5;
/// Una tecla que escribe algo y no tiene codigo propio aqui (la ñ, la ç, un acento, una
/// letra de otro alfabeto). En Windows cada una cae en un codigo de signo distinto segun el
/// idioma; lo unico que `teclas.rs` hace con ellos es saber que con ellos se escribe.
const VK_OTRA: u16 = 0xDF;

/// Los codigos de Windows de lo que pone en una tecla (su keysym). `NADA` si no es de las
/// que se miran: las de volumen, Bloq Mayus, Impr Pant.
fn vks_de_keysym(keysym: u32) -> Vks {
    let una = |vk: u32| [vk as u16, 0, 0, 0];
    match keysym {
        // Las letras y los numeros de arriba, por lo que pone en la tecla, como en Windows.
        0x61..=0x7A => una(keysym - 0x20),
        0x41..=0x5A | 0x30..=0x39 | 0x20 => una(keysym),
        // Retroceso, Tab (y su Mayus + Tab), Intro (y el del teclado numerico), Escape.
        0xFF08 => una(0x08),
        0xFF09 | 0xFE20 => una(0x09),
        0xFF0D | 0xFF8D => una(0x0D),
        0xFF1B => una(0x1B),
        // Inicio, las flechas, Re Pag, Av Pag, Fin, Insertar, Suprimir.
        0xFF50 => una(0x24),
        0xFF51..=0xFF54 => una(0x25 + (keysym - 0xFF51)),
        0xFF55 => una(0x21),
        0xFF56 => una(0x22),
        0xFF57 => una(0x23),
        0xFF63 => una(0x2D),
        0xFFFF => una(0x2E),
        // El teclado numerico: * + separador - . / y los numeros.
        0xFFAA..=0xFFAF => una(0x6A + (keysym - 0xFFAA)),
        0xFFB0..=0xFFB9 => una(0x60 + (keysym - 0xFFB0)),
        // F1 a F24.
        0xFFBE..=0xFFD5 => una(0x70 + (keysym - 0xFFBE)),
        // Los modificadores. Meta es la tecla de Windows cuando el teclado la llama asi.
        0xFFE1 | 0xFFE2 => una(u32::from(VK_SHIFT)),
        0xFFE3 => [VK_CONTROL, VK_LCONTROL, 0, 0],
        0xFFE4 => [VK_CONTROL, VK_RCONTROL, 0, 0],
        0xFFE9 => [VK_MENU, VK_LMENU, 0, 0],
        0xFFEA => [VK_MENU, VK_RMENU, 0, 0],
        0xFFEB | 0xFFE7 => una(u32::from(VK_LWIN)),
        0xFFEC | 0xFFE8 => una(u32::from(VK_RWIN)),
        // AltGr (ISO_Level3_Shift, y el Mode_switch de los teclados viejos): la tecla de la @
        // y el # en un teclado espannol. Windows la da como Ctrl y Alt a la vez, y con eso
        // `teclas.rs` sabe que es escribir y no un atajo.
        0xFE03 | 0xFF7E => [VK_MENU, VK_CONTROL, VK_RMENU, VK_LCONTROL],
        // Los signos, con el codigo que tienen en un teclado de Estados Unidos, que es como
        // los nombra el ajuste de la tecla de zoom: ; = , - . / ` [ \ ] ' y la de < >.
        0x3B => una(0xBA),
        0x3D => una(0xBB),
        0x2C => una(0xBC),
        0x2D => una(0xBD),
        0x2E => una(0xBE),
        0x2F => una(0xBF),
        0x60 => una(0xC0),
        0x5B => una(0xDB),
        0x5C => una(0xDC),
        0x5D => una(0xDD),
        0x27 => una(0xDE),
        0x3C => una(0xE2),
        _ => NADA,
    }
}

/// Si con ese keysym se escribe algo: una letra de cualquier alfabeto, un signo o un acento
/// de los que esperan a la letra siguiente. Las teclas de funcion, las de volumen y los
/// modificadores no.
fn escribe(keysym: u32) -> bool {
    matches!(keysym, 0x20..=0x7E | 0xA0..=0x20FF | 0xFE50..=0xFE93 | 0x0100_0100..=0x0110_FFFF)
}

/// Los codigos de Windows de una tecla, con todo lo que pone en ella: sola, con Mayusculas,
/// en el segundo idioma y con AltGr, en ese orden.
///
/// Manda lo primero que tenga codigo. Casi siempre es lo que pone la tecla sola, pero no en
/// tres casos que importan: en un teclado frances los numeros se escriben con Mayusculas (la
/// tecla sola pone & é "), en el teclado numerico la tecla sola es Inicio o Fin, y con un
/// alfabeto que no es el latino las letras de los atajos estan en el segundo idioma.
pub(crate) fn vks_de_tecla(keysyms: &[u32]) -> Vks {
    if let Some(vks) = keysyms.iter().map(|&k| vks_de_keysym(k)).find(|vks| *vks != NADA) {
        return vks;
    }
    if keysyms.first().is_some_and(|&k| escribe(k)) {
        return [VK_OTRA, 0, 0, 0];
    }
    NADA
}

/// Donde esta el raton, en pixeles del escritorio, y que botones tiene abajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raton {
    pub x: i32,
    pub y: i32,
    pub izquierdo: bool,
    pub derecho: bool,
}

/// Lo que vale una respuesta del raton. En una misma vuelta se pregunta tres veces (donde
/// esta, y cada boton): con esto es una sola pregunta al servidor, y las tres respuestas son
/// del mismo instante. Las vueltas van cada 16 ms, asi que nunca comparten respuesta.
const VALE: Duration = Duration::from_millis(4);

struct Entrada {
    conexion: RustConnection,
    raiz: Window,
    /// Los codigos de Windows de cada tecla de X, por su keycode.
    tabla: Mutex<Vec<Vks>>,
    raton: Mutex<Option<(Instant, Raton)>>,
}

static ENTRADA: OnceLock<Option<Entrada>> = OnceLock::new();

/// La conexion con el servidor X, que se abre la primera vez que hace falta y se queda. Es
/// aparte de las de la imagen y del puntero (`x11.rs`), que estan ocupadas esperando lo suyo.
fn entrada() -> Option<&'static Entrada> {
    ENTRADA
        .get_or_init(|| {
            if crate::platform::xdg::es_wayland() {
                return None;
            }
            let abierta = RustConnection::connect(None)
                .map_err(|e| format!("no se puede hablar con el servidor X: {e}"))
                .and_then(|(conexion, numero)| {
                    let raiz = conexion.setup().roots.get(numero).map(|p| p.root).ok_or("el servidor X no tiene esa pantalla")?;
                    let tabla = leer_tabla(&conexion).ok_or("el servidor X no dice qué teclado hay")?;
                    Ok(Entrada { conexion, raiz, tabla: Mutex::new(tabla), raton: Mutex::new(None) })
                });
            abierta.map_err(|error: String| eprintln!("[grabar] sin clics ni teclas: {error}")).ok()
        })
        .as_ref()
}

/// La tabla de keycode a codigos de Windows, con el teclado que hay puesto ahora.
fn leer_tabla(conexion: &RustConnection) -> Option<Vec<Vks>> {
    let (primera, ultima) = (conexion.setup().min_keycode, conexion.setup().max_keycode);
    let cuantas = ultima.checked_sub(primera)?.checked_add(1)?;
    let teclado = conexion.get_keyboard_mapping(primera, cuantas).ok()?.reply().ok()?;
    let mut tabla = vec![NADA; 256];
    let por_tecla = usize::from(teclado.keysyms_per_keycode);
    if por_tecla > 0 {
        for (n, keysyms) in teclado.keysyms.chunks(por_tecla).enumerate() {
            if let Some(hueco) = tabla.get_mut(usize::from(primera) + n) {
                *hueco = vks_de_tecla(keysyms);
            }
        }
    }
    Some(tabla)
}

/// El raton ahora mismo. `None` en Wayland o si el servidor X no contesta.
pub fn raton() -> Option<Raton> {
    let entrada = entrada()?;
    let mut guardado = entrada.raton.lock();
    if let Some((cuando, raton)) = *guardado {
        if cuando.elapsed() < VALE {
            return Some(raton);
        }
    }
    let puntero = entrada.conexion.query_pointer(entrada.raiz).ok()?.reply().ok()?;
    if !puntero.same_screen {
        return None;
    }
    let raton = Raton {
        x: i32::from(puntero.root_x),
        y: i32::from(puntero.root_y),
        izquierdo: puntero.mask.contains(KeyButMask::BUTTON1),
        derecho: puntero.mask.contains(KeyButMask::BUTTON3),
    };
    *guardado = Some((Instant::now(), raton));
    Some(raton)
}

/// Llama a `poner` con el codigo de Windows de cada tecla que esta abajo ahora mismo. En
/// Wayland, o si el servidor X no contesta, con ninguna.
pub fn teclas_abajo(mut poner: impl FnMut(u16)) {
    let Some(entrada) = entrada() else { return };
    let Some(abajo) = entrada.conexion.query_keymap().ok().and_then(|c| c.reply().ok()) else {
        return;
    };
    // El servidor avisa a todo el mundo cuando cambia el teclado (otro idioma): la tabla se
    // rehace antes de traducir. Y de paso no se acumulan avisos sin leer.
    let mut cambiado = false;
    while let Ok(Some(aviso)) = entrada.conexion.poll_for_event() {
        cambiado |= matches!(aviso, Event::MappingNotify(_));
    }
    let mut tabla = entrada.tabla.lock();
    if cambiado {
        if let Some(nueva) = leer_tabla(&entrada.conexion) {
            *tabla = nueva;
        }
    }
    for (n, &octeto) in abajo.keys.iter().enumerate() {
        for bit in (0..8).filter(|bit| octeto & (1u8 << bit) != 0) {
            for &vk in tabla[n * 8 + bit].iter().filter(|&&vk| vk != 0) {
                poner(vk);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::teclas::{Foto, TeclaZoom, Vigilante};

    // Los keysym de las pruebas, con su nombre de X11.
    const CONTROL_L: u32 = 0xFFE3;
    const SHIFT_L: u32 = 0xFFE1;
    const ALT_L: u32 = 0xFFE9;
    const META_L: u32 = 0xFFE7;
    const SUPER_L: u32 = 0xFFEB;
    const ISO_LEVEL3_SHIFT: u32 = 0xFE03;

    fn una(vk: u16) -> Vks {
        [vk, 0, 0, 0]
    }

    /// Una foto del teclado con esas teclas de X abajo, cada una con todo lo que pone en ella.
    fn foto(teclas: &[&[u32]]) -> Foto {
        let vks: Vec<u16> = teclas.iter().flat_map(|t| vks_de_tecla(t)).filter(|&vk| vk != 0).collect();
        Foto::con(&vks)
    }

    #[test]
    fn un_teclado_de_estados_unidos_da_los_codigos_de_windows() {
        // Las letras: minuscula sola y mayuscula con Mayusculas.
        assert_eq!(vks_de_tecla(&[0x61, 0x41]), una(0x41));
        assert_eq!(vks_de_tecla(&[0x7A, 0x5A]), una(0x5A));
        // Los numeros de arriba, con su signo encima.
        assert_eq!(vks_de_tecla(&[0x31, 0x21]), una(0x31));
        assert_eq!(vks_de_tecla(&[0x39, 0x28]), una(0x39));
        // Espacio, Retroceso, Tab, Intro, Escape y Suprimir.
        for (keysym, vk) in [(0x20, 0x20), (0xFF08, 0x08), (0xFF09, 0x09), (0xFF0D, 0x0D), (0xFF1B, 0x1B), (0xFFFF, 0x2E)] {
            assert_eq!(vks_de_tecla(&[keysym]), una(vk), "{keysym:#X}");
        }
        // Inicio, izquierda, arriba, derecha, abajo, Re Pag, Av Pag, Fin, Insertar.
        let navegar = [0x24, 0x25, 0x26, 0x27, 0x28, 0x21, 0x22, 0x23];
        for (n, vk) in navegar.into_iter().enumerate() {
            assert_eq!(vks_de_tecla(&[0xFF50 + n as u32]), una(vk), "la {n} de navegar");
        }
        assert_eq!(vks_de_tecla(&[0xFF63]), una(0x2D));
        // F1, F12 y F24.
        assert_eq!(vks_de_tecla(&[0xFFBE]), una(0x70));
        assert_eq!(vks_de_tecla(&[0xFFC9]), una(0x7B));
        assert_eq!(vks_de_tecla(&[0xFFD5]), una(0x87));
        // Los signos: ; = , - . / ` [ \ ] ' y la tecla de < >.
        let signos = [(0x3B, 0xBA), (0x3D, 0xBB), (0x2C, 0xBC), (0x2D, 0xBD), (0x2E, 0xBE), (0x2F, 0xBF), (0x60, 0xC0)];
        let mas_signos = [(0x5B, 0xDB), (0x5C, 0xDC), (0x5D, 0xDD), (0x27, 0xDE), (0x3C, 0xE2)];
        for (keysym, vk) in signos.into_iter().chain(mas_signos) {
            assert_eq!(vks_de_tecla(&[keysym]), una(vk), "{keysym:#X}");
        }
    }

    /// El teclado numerico da sus numeros con o sin Bloq Num: la tecla sola es Inicio, Fin o
    /// Insertar, y el numero va con Mayusculas.
    #[test]
    fn el_teclado_numerico_da_sus_numeros() {
        // KP_Home con KP_7, KP_Insert con KP_0, KP_Delete con KP_Decimal.
        assert_eq!(vks_de_tecla(&[0xFF95, 0xFFB7]), una(0x67));
        assert_eq!(vks_de_tecla(&[0xFF9E, 0xFFB0]), una(0x60));
        assert_eq!(vks_de_tecla(&[0xFF9F, 0xFFAE]), una(0x6E));
        // KP_Multiply, KP_Add, KP_Subtract, KP_Divide y KP_Enter.
        for (keysym, vk) in [(0xFFAA, 0x6A), (0xFFAB, 0x6B), (0xFFAD, 0x6D), (0xFFAF, 0x6F), (0xFF8D, 0x0D)] {
            assert_eq!(vks_de_tecla(&[keysym]), una(vk), "{keysym:#X}");
        }
    }

    /// Los modificadores salen como los ve Windows: el generico y el de su lado. Alt lleva
    /// Meta encima en casi todos los teclados y sigue siendo Alt; Meta sola es la de Windows.
    #[test]
    fn los_modificadores_son_los_de_windows() {
        assert_eq!(vks_de_tecla(&[SHIFT_L]), una(VK_SHIFT));
        assert_eq!(vks_de_tecla(&[0xFFE2]), una(VK_SHIFT));
        assert_eq!(vks_de_tecla(&[CONTROL_L]), [VK_CONTROL, VK_LCONTROL, 0, 0]);
        assert_eq!(vks_de_tecla(&[0xFFE4]), [VK_CONTROL, VK_RCONTROL, 0, 0]);
        assert_eq!(vks_de_tecla(&[ALT_L, META_L]), [VK_MENU, VK_LMENU, 0, 0]);
        assert_eq!(vks_de_tecla(&[0xFFEA, 0xFFE8]), [VK_MENU, VK_RMENU, 0, 0]);
        assert_eq!(vks_de_tecla(&[SUPER_L]), una(VK_LWIN));
        assert_eq!(vks_de_tecla(&[0xFFEC]), una(VK_RWIN));
        assert_eq!(vks_de_tecla(&[META_L]), una(VK_LWIN));
    }

    /// Lo que se pidio: con Ctrl, Mayusculas y el 9 abajo, `teclas.rs` ve la tecla de zoom de
    /// fabrica y, sin ella en los ajustes, el atajo con su nombre.
    #[test]
    fn un_atajo_de_x11_es_el_mismo_atajo_que_en_windows() {
        let abajo = foto(&[&[CONTROL_L], &[SHIFT_L], &[0x39, 0x28]]);
        let zoom = TeclaZoom::de_atajo("CmdOrCtrl+Shift+9").expect("la tecla de fábrica");
        assert!(zoom.pulsada(&abajo), "la tecla de zoom no se reconoce");
        let atajo = Vigilante::default().mirar_foto(&abajo, 7).expect("es un atajo");
        assert_eq!(atajo.texto, "Ctrl + Mayús + 9");

        let con_win = foto(&[&[SUPER_L], &[0x65, 0x45]]);
        assert_eq!(Vigilante::default().mirar_foto(&con_win, 0).expect("es un atajo").texto, "Win + E");
        let con_alt = foto(&[&[ALT_L, META_L], &[0xFFC1]]);
        assert_eq!(Vigilante::default().mirar_foto(&con_alt, 0).expect("es un atajo").texto, "Alt + F4");
    }

    /// **La regla de seguridad, tambien aqui:** escribir no es un atajo. Ni una letra sola,
    /// ni una mayuscula, ni la @ de un teclado espannol, que se escribe con AltGr + 2.
    #[test]
    fn escribir_no_es_un_atajo_ni_con_altgr() {
        assert!(Vigilante::default().mirar_foto(&foto(&[&[0x61, 0x41]]), 0).is_none());
        assert!(Vigilante::default().mirar_foto(&foto(&[&[SHIFT_L], &[0x61, 0x41]]), 0).is_none());
        // La tecla del 2 en espannol: 2, comillas y, con AltGr, la arroba.
        let arroba = foto(&[&[ISO_LEVEL3_SHIFT], &[0x32, 0x22, 0x32, 0x22, 0x40]]);
        assert_eq!(vks_de_tecla(&[ISO_LEVEL3_SHIFT]), [VK_MENU, VK_CONTROL, VK_RMENU, VK_LCONTROL]);
        assert!(Vigilante::default().mirar_foto(&arroba, 0).is_none(), "la @ ha salido como atajo");
        let zoom = TeclaZoom::de_atajo("CmdOrCtrl+Alt+2").expect("un atajo");
        assert!(!zoom.pulsada(&arroba), "la @ ha hecho de tecla de zoom");
    }

    /// En un teclado frances los numeros de arriba se escriben con Mayusculas: la tecla sola
    /// pone otra cosa. Windows los llama por el numero, y aqui tambien.
    #[test]
    fn en_un_teclado_frances_los_numeros_son_los_de_la_tecla() {
        // ampersand con 1, eacute con 2, agrave con 0.
        assert_eq!(vks_de_tecla(&[0x26, 0x31]), una(0x31));
        assert_eq!(vks_de_tecla(&[0xE9, 0x32]), una(0x32));
        assert_eq!(vks_de_tecla(&[0xE0, 0x30]), una(0x30));
        // Y la A esta donde en Estados Unidos esta la Q, pero es la A.
        assert_eq!(vks_de_tecla(&[0x61, 0x41]), una(0x41));
    }

    /// Con el ruso puesto y el ingles de segundo idioma, las letras de los atajos siguen
    /// siendo las latinas, que es lo que hace Windows. Sin segundo idioma no hay letra que
    /// ensennar, pero sigue contando como escribir.
    #[test]
    fn con_otro_alfabeto_las_letras_salen_del_segundo_idioma() {
        // Cyrillic_ef y Cyrillic_EF, y en el segundo idioma la a.
        assert_eq!(vks_de_tecla(&[0x6C6, 0x6E6, 0x61, 0x41]), una(0x41));
        assert_eq!(vks_de_tecla(&[0x6C6, 0x6E6]), una(VK_OTRA));
    }

    /// Las teclas que escriben algo sin codigo propio (la ñ, la ç, un acento) cuentan como
    /// escribir; las que no escriben nada, no cuentan.
    #[test]
    fn lo_que_escribe_cuenta_y_lo_que_no_escribe_no() {
        // ntilde, ccedilla, exclamdown, dead_acute y una letra por su numero de Unicode.
        for keysym in [0xF1, 0xE7, 0xA1, 0xFE51, 0x0100_0451] {
            assert_eq!(vks_de_tecla(&[keysym]), una(VK_OTRA), "{keysym:#X}");
        }
        // Bloq Mayus, Bloq Num, Impr Pant, Pausa, Menu, el volumen y una tecla sin nada.
        for keysym in [0xFFE5, 0xFF7F, 0xFF61, 0xFF13, 0xFF67, 0x1008_FF12, 0] {
            assert_eq!(vks_de_tecla(&[keysym]), NADA, "{keysym:#X}");
        }
        assert_eq!(vks_de_tecla(&[]), NADA);
        // La ñ con su mayuscula y, con AltGr, la virgulilla: sigue siendo escribir.
        assert_eq!(vks_de_tecla(&[0xF1, 0xD1, 0xF1, 0xD1, 0x7E]), una(VK_OTRA));
    }

    /// Todo lo que sale de la tabla lo entiende `teclas.rs`: una foto con ese codigo lo da por
    /// pulsado. (Los codigos de Windows caben en un octeto; uno mas alto se confundiria con otro.)
    #[test]
    fn los_codigos_caben_en_la_foto() {
        for keysym in (0x20..=0xFF).chain(0xFE00..=0xFFFF) {
            for vk in vks_de_tecla(&[keysym]).into_iter().filter(|&vk| vk != 0) {
                assert!((0x08..=0xE2).contains(&vk), "{keysym:#X} da {vk:#X}");
            }
        }
    }

    /// Con un servidor X de verdad: un boton y unas teclas que estan abajo se ven, y el raton
    /// esta donde esta. Se pulsan de mentira con XTEST, asi que solo en un servidor de pruebas:
    /// `DISPLAY=:77 cargo test --lib entrada -- --ignored --test-threads=1`
    #[test]
    #[ignore = "necesita un servidor X (DISPLAY) y le pulsa teclas y botones de mentira"]
    fn en_un_servidor_x_se_ve_lo_que_esta_pulsado() {
        use x11rb::protocol::xproto::{BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, KEY_PRESS_EVENT, KEY_RELEASE_EVENT};
        use x11rb::protocol::xtest::ConnectionExt as _;
        use x11rb::wrapper::ConnectionExt as _;

        let (conexion, numero) = RustConnection::connect(None).expect("hace falta un servidor X");
        let raiz = conexion.setup().roots[numero].root;
        let primera = conexion.setup().min_keycode;
        let teclado = conexion
            .get_keyboard_mapping(primera, conexion.setup().max_keycode - primera + 1)
            .unwrap()
            .reply()
            .unwrap();
        // La tecla que tiene ese keysym en su primer sitio.
        let tecla = |keysym: u32| -> u8 {
            let n = teclado
                .keysyms
                .chunks(usize::from(teclado.keysyms_per_keycode))
                .position(|k| k[0] == keysym)
                .unwrap_or_else(|| panic!("el teclado del servidor no tiene {keysym:#X}"));
            primera + n as u8
        };
        let teclas = [tecla(CONTROL_L), tecla(SHIFT_L), tecla(0x39)];
        let mandar = |tipo: u8, detalle: u8| {
            conexion.xtest_fake_input(tipo, detalle, 0, raiz, 0, 0, 0).unwrap();
        };
        let esperar = || {
            conexion.sync().unwrap();
            std::thread::sleep(VALE * 3);
        };

        conexion.warp_pointer(x11rb::NONE, raiz, 0, 0, 0, 0, 321, 234).unwrap();
        esperar();
        let suelto = raton();
        let nada = Foto::leer();

        teclas.iter().for_each(|&t| mandar(KEY_PRESS_EVENT, t));
        mandar(BUTTON_PRESS_EVENT, 3);
        esperar();
        let pulsado = raton();
        let abajo = Foto::leer();
        // Y a traves del anotador, que es quien lo usa: el clic y el rastro del raton.
        let region = crate::capture::Rect { x: 300, y: 200, width: 400, height: 300 };
        let mut anotador = crate::record::anotador::Anotador::new(region, false, false);
        anotador.mirar(40);

        // Se suelta todo ANTES de comprobar nada: una prueba que falla no puede dejarle al
        // servidor una tecla pisada.
        mandar(BUTTON_RELEASE_EVENT, 3);
        teclas.iter().rev().for_each(|&t| mandar(KEY_RELEASE_EVENT, t));
        esperar();
        let despues = (raton(), Foto::leer());
        let anotado = anotador.terminar();

        assert_eq!(suelto, Some(Raton { x: 321, y: 234, izquierdo: false, derecho: false }));
        assert_eq!(nada, Foto::default(), "hay teclas abajo antes de pulsar ninguna");
        assert_eq!(pulsado, Some(Raton { x: 321, y: 234, izquierdo: false, derecho: true }));
        assert!(TeclaZoom::de_atajo("CmdOrCtrl+Shift+9").unwrap().pulsada(&abajo), "{abajo:?}");
        assert_eq!(Vigilante::default().mirar_foto(&abajo, 0).map(|a| a.texto).as_deref(), Some("Ctrl + Mayús + 9"));
        assert_eq!(despues, (suelto, Foto::default()), "queda algo pulsado después de soltarlo");
        // En coordenadas de la region, con el boton derecho, y con el atajo anotado.
        assert_eq!(anotado.cursor, vec![(40, 21, 34)]);
        assert_eq!(anotado.clics.iter().map(|c| (c.ms, c.x, c.y, c.derecho)).collect::<Vec<_>>(), vec![(40, 21, 34, true)]);
        assert_eq!(anotado.teclas.iter().map(|a| (a.texto.as_str(), a.x, a.y)).collect::<Vec<_>>(), vec![("Ctrl + Mayús + 9", 21, 34)]);
    }
}
