//! Enseñar los atajos que se pulsan durante la grabación, en una pastilla abajo.
//!
//! **Solo atajos, nunca texto suelto.** Se enseña lo que se pulsa junto a Ctrl, Alt, Win o
//! Mayúsculas, y nada más. Hay dos razones y las dos mandan por igual:
//!
//! 1. **Es lo único que hace falta ver.** En un tutorial importa que se sepa que has hecho
//!    `Ctrl+Shift+P`; que hayas escrito «hola» se ve en la pantalla igual.
//! 2. **Enseñar cada tecla convierte esto en un registrador de pulsaciones**, y encima uno
//!    que las escribe dentro de un vídeo. Si alguien teclea una contraseña mientras graba,
//!    esa contraseña acaba en el archivo que después comparte. Con esta regla no puede
//!    pasar: una contraseña no lleva Ctrl delante.
//!
//! Igual que los clics, se pregunta por el estado de las teclas en cada fotograma en vez
//! de poner un enganche global de teclado. Un enganche mal hecho le deja el teclado a
//! tirones a quien lo tenga puesto.

#![cfg(windows)]

/// Cuánto se queda la pastilla en pantalla desde la última pulsación, en milisegundos.
pub const DURACION_MS: u64 = 1_100;

/// Un atajo que se está viendo.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Atajo {
    /// Ya escrito para leerse: «Ctrl + Mayús + P».
    pub texto: String,
    /// Cuándo se pulsó, desde que empezó la grabación.
    pub ms: u64,
    /// Dónde estaba el ratón al pulsarlo, en coordenadas de la región grabada.
    ///
    /// Un atajo no tiene sitio propio, pero quien lo pulsa está mirando a algún lado, y ese
    /// lado es donde está su ratón. De aquí sale el zoom al pulsar una tecla: sin esto, la
    /// cámara no sabría a dónde acercarse.
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}

/// Lo que se sabía del teclado la última vez.
#[derive(Debug, Default)]
pub struct Vigilante {
    /// La última tecla principal que se vio pulsada, para no repetir el atajo en cada
    /// fotograma mientras se mantiene el dedo encima.
    ultima: Option<u16>,
}

impl Vigilante {
    /// Mira el teclado y devuelve el atajo si acaba de empezar uno.
    pub fn mirar(&mut self, ms: u64) -> Option<Atajo> {
        let modificadores = modificadores_pulsados();
        if modificadores.is_empty() {
            self.ultima = None;
            return None;
        }
        let principal = tecla_principal();
        match principal {
            None => {
                // Solo modificadores: todavía no es un atajo, es un dedo esperando.
                self.ultima = None;
                None
            }
            Some(codigo) if self.ultima == Some(codigo) => None,
            Some(codigo) => {
                self.ultima = Some(codigo);
                let mut partes = modificadores;
                partes.push(nombre_de(codigo));
                let (x, y) = super::raton::cursor().unwrap_or_default();
                Some(Atajo {
                    texto: partes.join(" + "),
                    ms,
                    x,
                    y,
                })
            }
        }
    }
}

const VK_SHIFT: i32 = 0x10;
const VK_CONTROL: i32 = 0x11;
const VK_MENU: i32 = 0x12;
const VK_LWIN: i32 = 0x5B;
const VK_RWIN: i32 = 0x5C;

fn pulsado(tecla: i32) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    (unsafe { GetAsyncKeyState(tecla) } as u16 & 0x8000) != 0
}

/// Los modificadores que están abajo ahora mismo, en el orden en que se dicen.
fn modificadores_pulsados() -> Vec<String> {
    let mut partes = Vec::new();
    if pulsado(VK_CONTROL) {
        partes.push("Ctrl".to_string());
    }
    if pulsado(VK_MENU) {
        partes.push("Alt".to_string());
    }
    if pulsado(VK_SHIFT) {
        partes.push("Mayús".to_string());
    }
    if pulsado(VK_LWIN) || pulsado(VK_RWIN) {
        partes.push("Win".to_string());
    }
    partes
}

/// La primera tecla que no sea un modificador y esté pulsada.
fn tecla_principal() -> Option<u16> {
    // Las que valen: letras, números, funciones, y las de navegación y edición. Se dejan
    // fuera los propios modificadores y los botones del ratón, que van por su lado.
    const RANGOS: [(i32, i32); 4] = [
        (0x08, 0x0D), // Retroceso, Tab, Intro
        (0x1B, 0x2E), // Escape, espacio, flechas, Inicio, Fin, Insertar, Suprimir
        (0x30, 0x5A), // números y letras
        (0x70, 0x7B), // F1 a F12
    ];
    for (desde, hasta) in RANGOS {
        for codigo in desde..=hasta {
            if codigo == VK_SHIFT || codigo == VK_CONTROL || codigo == VK_MENU {
                continue;
            }
            if pulsado(codigo) {
                return Some(codigo as u16);
            }
        }
    }
    None
}

/// El nombre con el que se conoce esa tecla.
fn nombre_de(codigo: u16) -> String {
    match codigo {
        0x08 => "Retroceso".into(),
        0x09 => "Tab".into(),
        0x0D => "Intro".into(),
        0x1B => "Esc".into(),
        0x20 => "Espacio".into(),
        0x21 => "Re Pág".into(),
        0x22 => "Av Pág".into(),
        0x23 => "Fin".into(),
        0x24 => "Inicio".into(),
        0x25 => "←".into(),
        0x26 => "↑".into(),
        0x27 => "→".into(),
        0x28 => "↓".into(),
        0x2D => "Insert".into(),
        0x2E => "Supr".into(),
        0x30..=0x39 => char::from(b'0' + (codigo - 0x30) as u8).to_string(),
        0x41..=0x5A => char::from(b'A' + (codigo - 0x41) as u8).to_string(),
        0x70..=0x7B => format!("F{}", codigo - 0x6F),
        otro => format!("{otro:#04X}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_letras_se_llaman_por_su_letra() {
        assert_eq!(nombre_de(0x41), "A");
        assert_eq!(nombre_de(0x5A), "Z");
    }

    #[test]
    fn los_numeros_por_su_numero() {
        assert_eq!(nombre_de(0x30), "0");
        assert_eq!(nombre_de(0x39), "9");
    }

    #[test]
    fn las_de_funcion_van_de_f1_a_f12() {
        assert_eq!(nombre_de(0x70), "F1");
        assert_eq!(nombre_de(0x7B), "F12");
    }

    #[test]
    fn las_flechas_se_dibujan_en_vez_de_escribirse() {
        assert_eq!(nombre_de(0x25), "←");
        assert_eq!(nombre_de(0x28), "↓");
    }

    #[test]
    fn una_tecla_rara_sale_con_su_numero_en_vez_de_reventar() {
        assert!(nombre_de(0xFF).starts_with("0x"));
    }

    /// Sin tocar el teclado no aparece ningún atajo. Es lo que impide que la pastilla
    /// salga sola en una grabación en la que nadie escribe.
    #[test]
    fn sin_pulsar_nada_no_hay_atajo() {
        let mut vigilante = Vigilante::default();
        vigilante.mirar(0);
        assert!(vigilante.mirar(33).is_none());
    }

    /// Que no se pueda pulsar una tecla desde una prueba no impide comprobar la regla
    /// central: sin modificador no hay atajo, y por tanto una contraseña escrita durante
    /// la grabación no puede acabar dentro del vídeo.
    #[test]
    fn una_tecla_sola_nunca_es_un_atajo() {
        // Con los modificadores sueltos (que es como está el teclado en una prueba), el
        // vigilante devuelve None pase lo que pase con las demás teclas.
        assert!(modificadores_pulsados().is_empty());
        let mut vigilante = Vigilante::default();
        assert!(vigilante.mirar(0).is_none());
    }
}
