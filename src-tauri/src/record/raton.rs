//! Saber cuando alguien pulsa un boton del raton, sin engancharse al escritorio.
//!
//! Se pregunta por el estado de los botones en cada fotograma en vez de poner un enganche
//! global (`SetWindowsHookEx`). Un enganche de bajo nivel mal hecho **le cuelga el
//! escritorio a quien lo tenga puesto**: si el callback tarda mas de la cuenta, Windows lo
//! desconecta y hasta entonces el raton va a tirones para todo el mundo. Preguntar cuesta
//! una lectura de memoria del sistema, y a treinta fotogramas por segundo se pregunta cada
//! 33 ms, que es menos de lo que dura el clic mas rapido de una persona.
//!
//! Lo unico que se pierde con este metodo es un clic tan corto que empiece y acabe entre
//! dos fotogramas, y eso no lo hace ni el raton de nadie.

#![cfg(windows)]

use super::realce::Clic;

/// Lo que se sabia de los botones la ultima vez que se miro.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vigilante {
    izquierdo: bool,
    derecho: bool,
}

impl Vigilante {
    /// Mira los botones y devuelve el clic si alguno acaba de bajar.
    ///
    /// Solo el momento de BAJAR cuenta: mientras se mantiene pulsado no se dibujan aros
    /// nuevos, que si no un arrastre de dos segundos dejaria sesenta aros encima.
    pub fn mirar(&mut self, ms: u64) -> Option<Clic> {
        let izquierdo = pulsado(VK_LBUTTON);
        let derecho = pulsado(VK_RBUTTON);
        let nuevo = (izquierdo && !self.izquierdo) || (derecho && !self.derecho);
        let era_derecho = derecho && !self.derecho;
        self.izquierdo = izquierdo;
        self.derecho = derecho;
        if !nuevo {
            return None;
        }
        let (x, y) = donde()?;
        Some(Clic {
            x,
            y,
            ms,
            derecho: era_derecho,
        })
    }
}

const VK_LBUTTON: i32 = 0x01;
const VK_RBUTTON: i32 = 0x02;

fn pulsado(tecla: i32) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    // El bit de arriba dice si esta pulsado AHORA. El de abajo dice si se pulso desde la
    // ultima consulta, y ese no vale: lo consume quien lo lea primero.
    (unsafe { GetAsyncKeyState(tecla) } as u16 & 0x8000) != 0
}

/// Donde esta el puntero, en coordenadas del escritorio virtual (pueden ser negativas).
/// Donde esta el cursor ahora mismo, en coordenadas del escritorio virtual.
///
/// Se usa para dos cosas ademas de los clics: para saber a donde acercar la camara cuando
/// se pulsa un atajo (un atajo no tiene sitio propio, pero el raton si), y para dibujar el
/// cursor al exportar.
pub fn cursor() -> Option<(i32, i32)> {
    donde()
}

/// Que forma tiene el puntero ahora mismo: la flecha, la barra de texto o la manita.
///
/// Windows no dice «esto es una barra de texto»: da el identificador del cursor que hay
/// puesto, y los cursores del sistema tienen identificadores fijos que se consiguen con
/// `LoadCursorW`. Se cargan una vez y se comparan. Un cursor personalizado no coincide con
/// ninguno y se dibuja como flecha, que es lo que hacia siempre.
pub fn forma() -> crate::encode::cursor::Forma {
    puntero_actual()
        .map(|(_, forma)| forma)
        .unwrap_or(crate::encode::cursor::Forma::Flecha)
}

/// El cursor que hay puesto ahora mismo: su identificador (con el que se puede leer su
/// imagen, ver `punteros.rs`) y su forma. `None` si esta escondido o no se puede saber.
pub fn puntero_actual() -> Option<(isize, crate::encode::cursor::Forma)> {
    use crate::encode::cursor::Forma;
    use std::sync::OnceLock;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorInfo, LoadCursorW, CURSORINFO, IDC_HAND, IDC_IBEAM,
    };

    static CONOCIDOS: OnceLock<(isize, isize)> = OnceLock::new();
    let (texto, mano) = *CONOCIDOS.get_or_init(|| {
        let carga = |id| unsafe { LoadCursorW(None, id) }.map(|h| h.0 as isize).unwrap_or(0);
        (carga(IDC_IBEAM), carga(IDC_HAND))
    });

    let mut info = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetCursorInfo(&mut info) }.ok()?;
    let actual = info.hCursor.0 as isize;
    if actual == 0 {
        return None;
    }
    let forma = if actual == texto {
        Forma::Texto
    } else if actual == mano {
        Forma::Mano
    } else {
        Forma::Flecha
    };
    Some((actual, forma))
}

fn donde() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut punto = POINT::default();
    unsafe { GetCursorPos(&mut punto) }.ok()?;
    Some((punto.x, punto.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No se puede pulsar el raton desde una prueba, pero si comprobar que preguntar no
    /// revienta y que el vigilante no se inventa clics cuando no hay ninguno.
    #[test]
    fn sin_tocar_nada_no_aparecen_clics() {
        let mut vigilante = Vigilante::default();
        // Dos vueltas: la primera aprende el estado y la segunda ya compara.
        vigilante.mirar(0);
        assert!(
            vigilante.mirar(33).is_none(),
            "se ha inventado un clic sin que nadie toque el ratón"
        );
    }

    #[test]
    fn el_puntero_esta_en_algun_sitio() {
        assert!(donde().is_some(), "Windows siempre sabe dónde está el ratón");
    }

    /// Preguntar la forma no revienta y devuelve una de las tres, sea cual sea el cursor
    /// que tenga puesto quien corre las pruebas.
    #[test]
    fn la_forma_del_puntero_es_una_de_las_tres() {
        let f = forma();
        assert!(matches!(
            f,
            crate::encode::cursor::Forma::Flecha
                | crate::encode::cursor::Forma::Texto
                | crate::encode::cursor::Forma::Mano
        ));
    }
}
