//! El teclado mientras se graba: los atajos que se enseñan en una pastilla, la tecla de
//! zoom y los ratos en los que se escribe.
//!
//! **Solo atajos, nunca texto suelto.** Se enseña lo que se pulsa junto a Ctrl, Alt o Win,
//! y con Mayúsculas solo lo que no escribe nada (Mayús + Tab sí, Mayús + H no). AltGr, que
//! Windows ve como Ctrl + Alt, es escribir: es la @ y el # de los teclados españoles. Hay
//! dos razones y las dos mandan por igual:
//!
//! 1. **Es lo único que hace falta ver.** En un tutorial importa que se sepa que has hecho
//!    `Ctrl+Shift+P`; que hayas escrito «hola» se ve en la pantalla igual.
//! 2. **Enseñar cada tecla convierte esto en un registrador de pulsaciones**, y encima uno
//!    que las escribe dentro de un vídeo. Si alguien teclea una contraseña mientras graba,
//!    esa contraseña acaba en el archivo que después comparte. Con esta regla no puede
//!    pasar: una contraseña no lleva Ctrl delante.
//!
//! Los ratos de escritura (`Escritura`) siguen la misma regla por el otro lado: saben que se
//! está escribiendo, y lo único que sale de aquí es **cuándo**. Ni qué tecla ni cuántas.
//!
//! Igual que los clics, se pregunta por el estado de las teclas en cada vuelta en vez de
//! poner un enganche global de teclado. Un enganche mal hecho le deja el teclado a tirones a
//! quien lo tenga puesto, y uno de bajo nivel ya le colgó el ordenador a Munir: ver la
//! memoria `hooks-de-teclado-en-windows.md`.

#![cfg(windows)]

use crate::contrato::TramoEscritura;

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

// --- La foto del teclado ---

const VK_BACK: u16 = 0x08;
const VK_RETURN: u16 = 0x0D;
const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_SPACE: u16 = 0x20;
const VK_DELETE: u16 = 0x2E;
const VK_LWIN: u16 = 0x5B;
const VK_RWIN: u16 = 0x5C;
const VK_LCONTROL: u16 = 0xA2;
const VK_RMENU: u16 = 0xA5;

/// Las teclas que se miran, de una vez y en cada vuelta: los modificadores, las de los
/// atajos y las que se usan para escribir. Ni los botones del ratón (van por `raton.rs`)
/// ni las teclas de volumen o de reproducción.
const MIRADAS: [(u16, u16); 11] = [
    (0x08, 0x0D), // Retroceso, Tab, Intro
    (0x10, 0x12), // Mayús, Ctrl, Alt
    (0x1B, 0x2E), // Escape, espacio, flechas, Inicio, Fin, Insertar, Suprimir
    (0x30, 0x39), // los números de arriba
    (0x41, 0x5A), // las letras
    (0x5B, 0x5C), // las dos Win
    (0x60, 0x6F), // el teclado numérico
    (0x70, 0x87), // F1 a F24
    (0xA2, 0xA5), // Ctrl y Alt de cada lado, para saber si es AltGr
    (0xBA, 0xC0), // ; = , - . / ` (los signos, que cambian de sitio con el idioma)
    (0xDB, 0xE2), // [ \ ] ' y la tecla de < > de los teclados europeos
];

/// Qué teclas estaban abajo en un instante. Leerlas todas de una vez, y no a trozos, hace
/// que el atajo, la tecla de zoom y la escritura vean el mismo teclado.
///
/// Es una foto y no un registro: no sabe qué había antes ni guarda nada. De aquí no sale al
/// disco ni una tecla; solo lo que deciden con ella `Vigilante`, `TeclaZoom` y `Escritura`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Foto {
    bits: [u64; 4],
}

impl Foto {
    /// El teclado de verdad, ahora mismo. Son unas ciento treinta preguntas al sistema;
    /// lo que cuestan está medido en `leer_el_teclado_entero_cuesta_poco`.
    pub fn leer() -> Self {
        use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        let mut foto = Self::default();
        for (desde, hasta) in MIRADAS {
            for vk in desde..=hasta {
                // El bit de arriba dice si está pulsada AHORA. El de abajo dice si se pulsó
                // desde la última consulta, y ese no vale: lo consume quien lo lea primero.
                if (unsafe { GetAsyncKeyState(i32::from(vk)) } as u16 & 0x8000) != 0 {
                    foto.poner(vk);
                }
            }
        }
        foto
    }

    /// Una foto con esas teclas abajo, para las pruebas.
    #[cfg(test)]
    pub fn con(teclas: &[u16]) -> Self {
        let mut foto = Self::default();
        for &vk in teclas {
            foto.poner(vk);
        }
        foto
    }

    fn poner(&mut self, vk: u16) {
        let vk = usize::from(vk & 0xFF);
        self.bits[vk / 64] |= 1 << (vk % 64);
    }

    pub fn pulsada(&self, vk: u16) -> bool {
        let vk = usize::from(vk & 0xFF);
        self.bits[vk / 64] & (1 << (vk % 64)) != 0
    }

    /// AltGr, la tecla de la @ y el # en los teclados españoles. Windows la cuenta como
    /// Ctrl y Alt a la vez, así que sin mirarla aparte escribir una @ era un atajo, y
    /// salía en el vídeo y en la sesión como «Ctrl + Alt + 2».
    fn altgr(&self) -> bool {
        self.pulsada(VK_RMENU) && self.pulsada(VK_LCONTROL)
    }

    fn modificadores(&self) -> Modificadores {
        let altgr = self.altgr();
        Modificadores {
            ctrl: self.pulsada(VK_CONTROL) && !altgr,
            alt: self.pulsada(VK_MENU) && !altgr,
            shift: self.pulsada(VK_SHIFT),
            win: self.pulsada(VK_LWIN) || self.pulsada(VK_RWIN),
        }
    }
}

/// Lee el teclado sin preguntar tecla por tecla cuando nadie ha tocado nada.
///
/// Leer las ciento treinta teclas cuesta (medido en `leer_el_teclado_entero_cuesta_poco`),
/// y el muestreador lo hace sesenta veces por segundo. Pero Windows apunta cuándo fue la
/// última vez que alguien tocó el teclado o el ratón (`GetLastInputInfo`), y eso es una
/// sola pregunta: si no ha cambiado desde la vuelta anterior y entonces no había nada
/// pulsado, ahora tampoco lo hay.
#[derive(Debug, Default)]
pub struct Lector {
    ultimo_toque: Option<u32>,
    ultima: Foto,
}

impl Lector {
    pub fn leer(&mut self) -> Foto {
        let toque = ultimo_toque();
        if toque.is_some() && toque == self.ultimo_toque && self.ultima == Foto::default() {
            return Foto::default();
        }
        self.ultimo_toque = toque;
        self.ultima = Foto::leer();
        self.ultima
    }
}

/// Cuándo se tocó por última vez el teclado o el ratón, en el reloj de Windows.
fn ultimo_toque() -> Option<u32> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe { GetLastInputInfo(&mut info) }.as_bool().then_some(info.dwTime)
}

/// Los modificadores que están abajo, con AltGr ya descontado: AltGr es escribir.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Modificadores {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
}

impl Modificadores {
    /// Los que convierten una tecla en un atajo. Mayúsculas sola no: con ella se escribe.
    fn de_atajo(&self) -> bool {
        self.ctrl || self.alt || self.win
    }

    /// En el orden en que se dicen.
    fn nombres(&self) -> Vec<String> {
        let mut partes = Vec::new();
        if self.ctrl {
            partes.push("Ctrl".to_string());
        }
        if self.alt {
            partes.push("Alt".to_string());
        }
        if self.shift {
            partes.push("Mayús".to_string());
        }
        if self.win {
            partes.push("Win".to_string());
        }
        partes
    }
}

// --- Los atajos de la pastilla ---

/// Lo que se sabía del teclado la última vez.
#[derive(Debug, Default)]
pub struct Vigilante {
    /// La última tecla principal que se vio pulsada, para no repetir el atajo en cada
    /// fotograma mientras se mantiene el dedo encima.
    ultima: Option<u16>,
}

impl Vigilante {
    /// Mira el teclado de verdad y devuelve el atajo si acaba de empezar uno, con el ratón
    /// en coordenadas del escritorio. Es lo que usa el anillo de los últimos segundos.
    pub fn mirar(&mut self, ms: u64) -> Option<Atajo> {
        let atajo = self.mirar_foto(&Foto::leer(), ms)?;
        let (x, y) = super::raton::cursor().unwrap_or_default();
        Some(Atajo { x, y, ..atajo })
    }

    /// Lo mismo con una foto del teclado ya hecha. El atajo sale con `x` e `y` a cero:
    /// dónde estaba el ratón lo pone quien lo anota, que es el que sabe desde dónde se mide.
    pub fn mirar_foto(&mut self, foto: &Foto, ms: u64) -> Option<Atajo> {
        self.decidir(foto.modificadores().nombres(), tecla_principal(foto), ms)
    }

    /// La regla, separada de la lectura del teclado.
    ///
    /// **Se parte en dos a propósito.** Esta es la regla que impide que una contraseña
    /// tecleada durante la grabación acabe dentro del vídeo, y su prueba leía el teclado de
    /// verdad: fallaba si al correrla había una tecla pulsada, y **no podía comprobar lo que
    /// de verdad importa**, porque desde una prueba no se puede pulsar Ctrl. Lo único que
    /// llegaba a comprobar era el caso en el que no hay nada pulsado.
    ///
    /// Así la regla se prueba entera y sin depender de lo que tenga nadie en las manos.
    fn decidir(
        &mut self,
        modificadores: Vec<String>,
        principal: Option<u16>,
        ms: u64,
    ) -> Option<Atajo> {
        if modificadores.is_empty() {
            self.ultima = None;
            return None;
        }
        // Mayúsculas sola con una letra, un número o un signo no es un atajo: es como se
        // escribe una mayúscula o una @, y una contraseña con mayúsculas no puede salir en
        // el vídeo letra a letra. Con Tab, las flechas o las de función sí lo es.
        let solo_mayus = modificadores.iter().all(|m| m == "Mayús");
        if solo_mayus && principal.is_some_and(escribe_un_caracter) {
            self.ultima = None;
            return None;
        }
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
                Some(Atajo {
                    texto: partes.join(" + "),
                    ms,
                    x: 0,
                    y: 0,
                })
            }
        }
    }
}

/// La primera tecla que no sea un modificador y esté pulsada.
fn tecla_principal(foto: &Foto) -> Option<u16> {
    // Las que valen: letras, números, funciones, y las de navegación y edición. Se dejan
    // fuera los propios modificadores y los botones del ratón, que van por su lado.
    const RANGOS: [(u16, u16); 4] = [
        (0x08, 0x0D), // Retroceso, Tab, Intro
        (0x1B, 0x2E), // Escape, espacio, flechas, Inicio, Fin, Insertar, Suprimir
        (0x30, 0x5A), // números y letras
        (0x70, 0x7B), // F1 a F12
    ];
    RANGOS
        .iter()
        .flat_map(|&(desde, hasta)| desde..=hasta)
        .filter(|&codigo| codigo != VK_SHIFT && codigo != VK_CONTROL && codigo != VK_MENU)
        .find(|&codigo| foto.pulsada(codigo))
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

// --- La tecla de zoom (A3) ---

/// La tecla que, mientras se graba, deja una marca de zoom donde está el ratón.
///
/// Se lee con la misma foto del teclado que los atajos, sin registrarla en Windows. Eso
/// tiene una consecuencia que hay que saber al elegirla: **la pulsación llega también a la
/// aplicación que se graba**, igual que llega cualquier atajo que se enseña en la pastilla.
/// Por eso la de fábrica es una que casi ningún programa usa (ver `atajo_de_zoom` en
/// `settings.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeclaZoom {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    vk: u16,
}

impl TeclaZoom {
    /// Lee un atajo escrito como los guardan los ajustes («CmdOrCtrl+Shift+9»,
    /// «Super+Shift+KeyL», «Alt+F7»). `None` si no se entiende o no lleva modificador: una
    /// tecla suelta haría zoom cada vez que alguien la escribe.
    pub fn de_atajo(texto: &str) -> Option<Self> {
        let mut tecla = Self {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            vk: 0,
        };
        let mut principal = None;
        for parte in texto.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            match parte.to_ascii_lowercase().as_str() {
                "cmdorctrl" | "commandorcontrol" | "ctrl" | "control" => tecla.ctrl = true,
                "alt" | "option" => tecla.alt = true,
                "shift" => tecla.shift = true,
                "super" | "win" | "meta" | "cmd" | "command" => tecla.win = true,
                _ => {
                    if principal.is_some() {
                        return None;
                    }
                    principal = Some(vk_de(parte)?);
                }
            }
        }
        tecla.vk = principal?;
        (tecla.ctrl || tecla.alt || tecla.shift || tecla.win).then_some(tecla)
    }

    /// Si está pulsada en esa foto, con exactamente sus modificadores: con uno de más ya es
    /// otro atajo, que puede ser de la aplicación que se graba.
    pub fn pulsada(&self, foto: &Foto) -> bool {
        let m = foto.modificadores();
        foto.pulsada(self.vk)
            && m.ctrl == self.ctrl
            && m.alt == self.alt
            && m.shift == self.shift
            && m.win == self.win
    }
}

/// El código de tecla de Windows de una tecla escrita como la escribe el navegador
/// (`KeyboardEvent.code`: «KeyZ», «Digit9», «F7», «Numpad3») o como venía de fábrica («9»).
///
/// Las letras y los números van por lo que pone en la tecla, igual que los registra el
/// plugin de atajos globales para los demás atajos de winshotx: así la tecla de zoom se
/// comporta como sus hermanas también en un teclado francés.
fn vk_de(nombre: &str) -> Option<u16> {
    let n = nombre.trim();
    let sin = |prefijo: &str| n.strip_prefix(prefijo).filter(|r| !r.is_empty());
    if let Some(letra) = sin("Key").or(Some(n)).filter(|r| r.len() == 1) {
        let c = letra.chars().next()?.to_ascii_uppercase();
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return Some(c as u16);
        }
    }
    if let Some(cifra) = sin("Digit") {
        let c = cifra.chars().next().filter(char::is_ascii_digit)?;
        return (cifra.len() == 1).then_some(c as u16);
    }
    if let Some(cifra) = sin("Numpad") {
        if let Ok(d) = cifra.parse::<u16>() {
            return (d <= 9).then_some(0x60 + d);
        }
    }
    if let Some(f) = sin("F") {
        if let Ok(numero) = f.parse::<u16>() {
            return (1..=24).contains(&numero).then_some(0x6F + numero);
        }
    }
    Some(match n {
        "Space" => VK_SPACE,
        "Enter" => VK_RETURN,
        "Tab" => 0x09,
        "Backspace" => VK_BACK,
        "Delete" => VK_DELETE,
        "Insert" => 0x2D,
        "Home" => 0x24,
        "End" => 0x23,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        "ArrowLeft" => 0x25,
        "ArrowUp" => 0x26,
        "ArrowRight" => 0x27,
        "ArrowDown" => 0x28,
        "Semicolon" => 0xBA,
        "Equal" => 0xBB,
        "Comma" => 0xBC,
        "Minus" => 0xBD,
        "Period" => 0xBE,
        "Slash" => 0xBF,
        "Backquote" => 0xC0,
        "BracketLeft" => 0xDB,
        "Backslash" => 0xDC,
        "BracketRight" => 0xDD,
        "Quote" => 0xDE,
        "IntlBackslash" => 0xE2,
        _ => return None,
    })
}

// --- Los ratos de escritura ---

/// Dos pulsaciones a menos de esto son el mismo rato de escritura. Un segundo y medio es lo
/// que se para alguien entre dos palabras pensando la siguiente; más es que ha dejado de
/// escribir (ha ido al ratón, está leyendo).
pub const HUECO_MS: u64 = 1_500;

/// Una tecla sola no es escribir: es un Intro, un espacio para bajar la página o un
/// Retroceso. Hacen falta al menos estas para que el rato cuente.
const MINIMO: u32 = 2;

/// ¿Es una tecla con la que se escribe? Letras, números, signos, espacio, Intro, borrar y
/// el teclado numérico. Las flechas, Tab, Esc y las de función no: con ellas se navega.
fn de_escribir(vk: u16) -> bool {
    matches!(vk, VK_BACK | VK_RETURN | VK_DELETE) || escribe_un_caracter(vk)
}

/// ¿Pone una letra en la pantalla? Las letras, los números, los signos y el espacio.
fn escribe_un_caracter(vk: u16) -> bool {
    matches!(vk,
        VK_SPACE | 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2)
}

/// Un rato de escritura que todavía no ha terminado.
#[derive(Debug, Clone, Copy)]
struct Abierto {
    desde_ms: u64,
    ultimo_ms: u64,
    /// **Solo en memoria y solo para decidir si el rato cuenta** (`MINIMO`). No sale de
    /// aquí: el tramo que se guarda no dice cuántas teclas hubo, porque con eso ya se
    /// sabría cuánto mide una contraseña.
    pulsaciones: u32,
    caret: Option<(i32, i32)>,
}

/// Saca los ratos en los que se escribió, sin quedarse con ninguna tecla.
///
/// Ve qué teclas bajan (eso no se puede evitar: es lo que dice que se escribe) y de ahí
/// solo apunta cuándo. Dos pulsaciones cercanas son el mismo rato (`HUECO_MS`); con Ctrl,
/// Alt o Win abajo no se escribe, se pulsa un atajo, y eso no cuenta.
#[derive(Debug, Default)]
pub struct Escritura {
    /// Las teclas de escribir que estaban abajo la vez anterior: una pulsación es una
    /// tecla que ahora está abajo y antes no. Mantener el dedo cuenta una vez.
    antes: Foto,
    abierto: Option<Abierto>,
    tramos: Vec<TramoEscritura>,
}

impl Escritura {
    /// Mira esa foto del teclado. `caret` dice dónde está el cursor de texto, y solo se
    /// pregunta al empezar un rato: preguntarlo en cada vuelta costaría una llamada al
    /// sistema sesenta veces por segundo para nada.
    pub fn mirar(
        &mut self,
        ms: u64,
        foto: &Foto,
        ignorar: Option<&TeclaZoom>,
        caret: impl FnOnce() -> Option<(i32, i32)>,
    ) {
        let atajo = foto.modificadores().de_atajo() || ignorar.is_some_and(|t| t.pulsada(foto));
        let mut nuevas = 0u32;
        let mut ahora = Foto::default();
        for (desde, hasta) in MIRADAS {
            for vk in (desde..=hasta).filter(|&vk| de_escribir(vk) && foto.pulsada(vk)) {
                ahora.poner(vk);
                if !self.antes.pulsada(vk) {
                    nuevas += 1;
                }
            }
        }
        self.antes = ahora;

        // Si el último rato ya se enfrió, se cierra antes de mirar lo nuevo.
        if self.abierto.is_some_and(|a| ms.saturating_sub(a.ultimo_ms) > HUECO_MS) {
            self.cerrar();
        }
        if atajo || nuevas == 0 {
            return;
        }
        match self.abierto.as_mut() {
            Some(abierto) => {
                abierto.ultimo_ms = ms;
                abierto.pulsaciones = abierto.pulsaciones.saturating_add(nuevas);
            }
            None => {
                self.abierto = Some(Abierto {
                    desde_ms: ms,
                    ultimo_ms: ms,
                    pulsaciones: nuevas,
                    caret: caret(),
                });
            }
        }
    }

    fn cerrar(&mut self) {
        let Some(abierto) = self.abierto.take() else { return };
        if abierto.pulsaciones >= MINIMO {
            self.tramos.push(TramoEscritura {
                desde_ms: abierto.desde_ms,
                hasta_ms: abierto.ultimo_ms,
                caret: abierto.caret,
            });
        }
    }

    /// Los ratos que hubo, cerrando el que siguiera abierto al parar.
    pub fn terminar(mut self) -> Vec<TramoEscritura> {
        self.cerrar();
        self.tramos
    }
}

/// Dónde está el cursor de texto de la ventana que tiene el teclado, en coordenadas del
/// escritorio. `None` si Windows no lo sabe.
///
/// Sale del cursor de texto del sistema (`CreateCaret`), que usan el Bloc de notas, Word,
/// los cuadros de texto de Windows y casi todo lo hecho con Win32. **Los navegadores y las
/// aplicaciones de Electron dibujan el suyo** y muchas veces no le dicen nada al sistema:
/// ahí sale `None`, y quien lo use se acerca al ratón.
pub fn caret() -> Option<(i32, i32)> {
    caret_de_hilo(0)
}

/// Lo mismo para un hilo concreto. El 0 es el hilo de la ventana de delante.
fn caret_de_hilo(hilo: u32) -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{GetGUIThreadInfo, GUITHREADINFO};

    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetGUIThreadInfo(hilo, &mut info) }.ok()?;
    if info.hwndCaret.is_invalid() {
        return None;
    }
    // El rectángulo viene en coordenadas de la ventana del cursor; se pasa al escritorio.
    // Se toma la mitad de su alto: es donde se ve la letra, no el borde de arriba.
    let r = info.rcCaret;
    let mut punto = POINT {
        x: r.left,
        y: r.top + (r.bottom - r.top) / 2,
    };
    unsafe { ClientToScreen(info.hwndCaret, &mut punto) }
        .as_bool()
        .then_some((punto.x, punto.y))
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
        let nada = Foto::default();
        vigilante.mirar_foto(&nada, 0);
        assert!(vigilante.mirar_foto(&nada, 33).is_none());
    }

    /// Leer el teclado de verdad no revienta, y lo que cuesta cabe de sobra en las sesenta
    /// vueltas por segundo del muestreador.
    #[test]
    fn leer_el_teclado_entero_cuesta_poco() {
        let vueltas = 200;
        // La mejor de veinte tandas de diez, y no la media de las doscientas: con las demás
        // pruebas corriendo a la vez y otras compilaciones en la máquina, la media pasó del
        // milisegundo una vez (11-nov-2026) leyendo 250 µs a solas. Lo más rápido que sale
        // es lo que cuesta leer; lo que sobra es la máquina ocupada en otra cosa.
        let por_vuelta = (0..20)
            .map(|_| {
                let inicio = std::time::Instant::now();
                for _ in 0..10 {
                    std::hint::black_box(Foto::leer());
                }
                inicio.elapsed() / 10
            })
            .min()
            .unwrap();
        // Y con el lector, que no pregunta tecla por tecla si nadie ha tocado nada.
        let mut lector = Lector::default();
        let inicio = std::time::Instant::now();
        for _ in 0..vueltas {
            std::hint::black_box(lector.leer());
        }
        let quieto = inicio.elapsed() / vueltas;
        println!("leer el teclado: {por_vuelta:?} por vuelta; con el lector: {quieto:?}");
        // A sesenta vueltas por segundo, un milisegundo por vuelta sería el 6 % de un
        // núcleo solo en esto. El techo es holgado para no fallar en una máquina cargada.
        assert!(
            por_vuelta < std::time::Duration::from_millis(1),
            "leer el teclado tarda {por_vuelta:?}"
        );
    }

    /// La letra A, que es lo que alguien teclearia escribiendo una contrasenna.
    const A: u16 = 0x41;
    const B: u16 = 0x42;

    fn ctrl() -> Vec<String> {
        vec!["Ctrl".to_string()]
    }

    #[test]
    fn una_tecla_sola_nunca_es_un_atajo() {
        // **La regla de seguridad del proyecto.** Sin modificador no hay atajo, y por eso
        // una contrasenna escrita durante la grabacion no puede acabar dentro del video:
        // una contrasenna no lleva Ctrl delante.
        let mut vigilante = Vigilante::default();
        assert!(vigilante.decidir(Vec::new(), Some(A), 0).is_none());
    }

    #[test]
    fn ni_aunque_se_teclee_una_palabra_entera() {
        // Letra a letra, que es como se escribe una contrasenna de verdad.
        let mut vigilante = Vigilante::default();
        for tecla in [0x68, 0x6F, 0x6C, 0x61, 0x31, 0x32, 0x33] {
            assert!(
                vigilante.decidir(Vec::new(), Some(tecla), 0).is_none(),
                "la tecla {tecla:#x} ha salido en el video"
            );
        }
    }

    #[test]
    fn ni_con_mayusculas() {
        // «Hola» lleva Mayús en la H. Antes eso salía como «Mayús + H» en la pastilla y
        // en la sesión: la primera letra de cada palabra con mayúscula, al disco.
        let mut vigilante = Vigilante::default();
        let foto = Foto::con(&[VK_SHIFT, 0x48]);
        assert!(vigilante.mirar_foto(&foto, 0).is_none());
        // Pero Mayús con Tab o con una flecha sí es un atajo: no escribe nada.
        let tab = Foto::con(&[VK_SHIFT, 0x09]);
        assert_eq!(vigilante.mirar_foto(&tab, 10).unwrap().texto, "Mayús + Tab");
    }

    #[test]
    fn ni_con_altgr() {
        // AltGr + 2 es la @ en un teclado español, y Windows la ve como Ctrl + Alt + 2.
        // Una contraseña con @ o # no puede dejar sus signos en el vídeo.
        let mut vigilante = Vigilante::default();
        let arroba = Foto::con(&[VK_CONTROL, VK_LCONTROL, VK_MENU, VK_RMENU, 0x32]);
        assert!(vigilante.mirar_foto(&arroba, 0).is_none());
    }

    #[test]
    fn ctrl_y_alt_de_verdad_si_son_un_atajo() {
        // Con el Alt de la izquierda no es AltGr: Ctrl + Alt + Supr sigue siendo un atajo.
        let mut vigilante = Vigilante::default();
        let foto = Foto::con(&[VK_CONTROL, VK_LCONTROL, VK_MENU, 0xA4, VK_DELETE]);
        assert_eq!(vigilante.mirar_foto(&foto, 0).unwrap().texto, "Ctrl + Alt + Supr");
    }

    #[test]
    fn con_modificador_si_es_un_atajo() {
        let mut vigilante = Vigilante::default();
        let atajo = vigilante.decidir(ctrl(), Some(A), 500).expect("tendria que salir");
        assert_eq!(atajo.texto, "Ctrl + A");
        assert_eq!(atajo.ms, 500);
    }

    #[test]
    fn la_foto_del_teclado_llega_al_atajo() {
        let mut vigilante = Vigilante::default();
        let foto = Foto::con(&[VK_CONTROL, VK_SHIFT, 0x50]);
        assert_eq!(vigilante.mirar_foto(&foto, 0).unwrap().texto, "Ctrl + Mayús + P");
    }

    #[test]
    fn los_modificadores_solos_no_bastan() {
        // Un dedo esperando encima de Ctrl no es un atajo todavia.
        let mut vigilante = Vigilante::default();
        assert!(vigilante.decidir(ctrl(), None, 0).is_none());
    }

    #[test]
    fn mantener_la_tecla_no_repite_el_atajo_en_cada_fotograma() {
        // A treinta fotogramas por segundo, medio segundo con el dedo encima serian quince
        // pastillas encima de la anterior.
        let mut vigilante = Vigilante::default();
        assert!(vigilante.decidir(ctrl(), Some(A), 0).is_some());
        assert!(vigilante.decidir(ctrl(), Some(A), 33).is_none());
        assert!(vigilante.decidir(ctrl(), Some(A), 66).is_none());
        // Otra tecla si abre uno nuevo.
        assert!(vigilante.decidir(ctrl(), Some(B), 99).is_some());
    }

    #[test]
    fn soltar_los_modificadores_permite_repetir_el_mismo_atajo() {
        // Pulsar Ctrl+C dos veces seguidas tiene que ensennarse dos veces.
        let mut vigilante = Vigilante::default();
        assert!(vigilante.decidir(ctrl(), Some(A), 0).is_some());
        assert!(vigilante.decidir(Vec::new(), None, 100).is_none());
        assert!(vigilante.decidir(ctrl(), Some(A), 200).is_some());
    }

    // --- La tecla de zoom ---

    #[test]
    fn la_tecla_de_zoom_se_lee_como_la_guardan_los_ajustes() {
        let de_fabrica = TeclaZoom::de_atajo("CmdOrCtrl+Shift+9").unwrap();
        assert!(de_fabrica.pulsada(&Foto::con(&[VK_CONTROL, VK_SHIFT, 0x39])));
        // Lo que escribe el campo de atajos: el código de la tecla del navegador.
        let del_campo = TeclaZoom::de_atajo("CmdOrCtrl+Shift+Digit9").unwrap();
        assert_eq!(del_campo, de_fabrica);
        let letra = TeclaZoom::de_atajo("Super+Shift+KeyZ").unwrap();
        assert!(letra.pulsada(&Foto::con(&[VK_LWIN, VK_SHIFT, 0x5A])));
        let funcion = TeclaZoom::de_atajo("Alt+F7").unwrap();
        assert!(funcion.pulsada(&Foto::con(&[VK_MENU, 0x76])));
        let numerico = TeclaZoom::de_atajo("CmdOrCtrl+Numpad5").unwrap();
        assert!(numerico.pulsada(&Foto::con(&[VK_CONTROL, 0x65])));
    }

    #[test]
    fn sin_modificador_o_sin_tecla_no_hay_tecla_de_zoom() {
        // Una tecla suelta haría zoom cada vez que alguien escribe un 9.
        assert!(TeclaZoom::de_atajo("9").is_none());
        assert!(TeclaZoom::de_atajo("CmdOrCtrl+Shift").is_none());
        assert!(TeclaZoom::de_atajo("").is_none());
        assert!(TeclaZoom::de_atajo("CmdOrCtrl+Patata").is_none());
        assert!(TeclaZoom::de_atajo("CmdOrCtrl+A+B").is_none());
    }

    #[test]
    fn con_un_modificador_de_mas_ya_no_es_la_tecla_de_zoom() {
        // Ctrl + Alt + Mayús + 9 puede ser de la aplicación que se graba.
        let zoom = TeclaZoom::de_atajo("CmdOrCtrl+Shift+9").unwrap();
        assert!(!zoom.pulsada(&Foto::con(&[VK_CONTROL, VK_SHIFT, 0xA4, VK_MENU, 0x39])));
        assert!(!zoom.pulsada(&Foto::con(&[VK_CONTROL, 0x39])), "y con uno de menos tampoco");
    }

    // --- Los ratos de escritura ---

    /// Teclea esa palabra letra a letra, cada una pulsada y soltada, desde ese instante y
    /// con ese ritmo. Devuelve el instante en que acaba.
    fn teclear(escritura: &mut Escritura, palabra: &str, desde: u64, ritmo: u64) -> u64 {
        let mut ms = desde;
        for c in palabra.chars() {
            let vk = c.to_ascii_uppercase() as u16;
            escritura.mirar(ms, &Foto::con(&[vk]), None, || Some((10, 20)));
            escritura.mirar(ms + ritmo / 2, &Foto::default(), None, || None);
            ms += ritmo;
        }
        ms
    }

    #[test]
    fn escribir_una_frase_es_un_solo_rato() {
        let mut escritura = Escritura::default();
        let fin = teclear(&mut escritura, "hola que tal", 1_000, 120);
        let tramos = escritura.terminar();
        assert_eq!(tramos.len(), 1, "{tramos:?}");
        assert_eq!(tramos[0].desde_ms, 1_000);
        assert_eq!(tramos[0].hasta_ms, fin - 120, "acaba en la última pulsación");
        assert_eq!(tramos[0].caret, Some((10, 20)), "el cursor de texto al empezar");
    }

    #[test]
    fn dos_frases_separadas_por_una_pausa_son_dos_ratos() {
        let mut escritura = Escritura::default();
        let fin = teclear(&mut escritura, "primera", 0, 100);
        // Se va al ratón tres segundos: el rato se cierra aunque no vuelva a escribir.
        escritura.mirar(fin + 3_000, &Foto::default(), None, || None);
        teclear(&mut escritura, "segunda", fin + 3_000, 100);
        let tramos = escritura.terminar();
        assert_eq!(tramos.len(), 2, "{tramos:?}");
        assert!(tramos[0].hasta_ms < tramos[1].desde_ms);
    }

    #[test]
    fn una_tecla_sola_no_es_un_rato_de_escritura() {
        // Un Intro, o un espacio para bajar la página.
        let mut escritura = Escritura::default();
        escritura.mirar(0, &Foto::con(&[VK_RETURN]), None, || None);
        escritura.mirar(5_000, &Foto::default(), None, || None);
        assert!(escritura.terminar().is_empty());
    }

    #[test]
    fn mantener_una_tecla_no_es_escribir() {
        // Con el dedo encima del Retroceso, la tecla se repite en la pantalla pero baja
        // una sola vez: cuenta como una, y una sola no hace un rato.
        let mut escritura = Escritura::default();
        for ms in (0..2_000).step_by(16) {
            escritura.mirar(ms, &Foto::con(&[VK_BACK]), None, || None);
        }
        assert!(escritura.terminar().is_empty());
    }

    #[test]
    fn los_atajos_y_la_navegacion_no_son_escribir() {
        let mut escritura = Escritura::default();
        let mut ms = 0;
        for vk in [0x43, 0x56, 0x5A, 0x53] {
            // Ctrl+C, Ctrl+V, Ctrl+Z, Ctrl+S
            escritura.mirar(ms, &Foto::con(&[VK_CONTROL, vk]), None, || None);
            escritura.mirar(ms + 50, &Foto::default(), None, || None);
            ms += 100;
        }
        for vk in [0x25, 0x26, 0x27, 0x28, 0x09, 0x1B] {
            // las flechas, Tab y Esc
            escritura.mirar(ms, &Foto::con(&[vk]), None, || None);
            escritura.mirar(ms + 50, &Foto::default(), None, || None);
            ms += 100;
        }
        assert!(escritura.terminar().is_empty());
    }

    #[test]
    fn la_tecla_de_zoom_no_es_escribir_aunque_solo_lleve_mayus() {
        let zoom = TeclaZoom::de_atajo("Shift+KeyZ").unwrap();
        let mut escritura = Escritura::default();
        for ms in [0, 200, 400] {
            escritura.mirar(ms, &Foto::con(&[VK_SHIFT, 0x5A]), Some(&zoom), || None);
            escritura.mirar(ms + 100, &Foto::default(), Some(&zoom), || None);
        }
        assert!(escritura.terminar().is_empty());
    }

    #[test]
    fn con_altgr_si_se_escribe() {
        // juan@correo: la @ es AltGr + 2, y sigue siendo escribir.
        let mut escritura = Escritura::default();
        let fin = teclear(&mut escritura, "juan", 0, 100);
        escritura.mirar(fin, &Foto::con(&[VK_CONTROL, VK_LCONTROL, VK_MENU, VK_RMENU, 0x32]), None, || None);
        escritura.mirar(fin + 50, &Foto::default(), None, || None);
        teclear(&mut escritura, "correo", fin + 100, 100);
        assert_eq!(escritura.terminar().len(), 1);
    }

    /// **La regla de seguridad de los ratos.** Lo que se guarda en la sesión no dice qué
    /// se escribió ni cuántas teclas: escribir «hola» y «contraseña» en el mismo rato da
    /// el mismo tramo, campo por campo.
    #[test]
    fn el_tramo_no_dice_que_se_escribio_ni_cuantas_teclas() {
        let corto = {
            let mut e = Escritura::default();
            e.mirar(0, &Foto::con(&[0x48]), None, || None);
            e.mirar(10, &Foto::default(), None, || None);
            e.mirar(890, &Foto::con(&[0x4F]), None, || None);
            e.mirar(900, &Foto::con(&[0x4C]), None, || None);
            e.terminar()
        };
        let largo = {
            // «contraseña», con la ñ, de 0 a 900 ms.
            let mut e = Escritura::default();
            e.mirar(0, &Foto::con(&[0x43]), None, || None);
            for (i, vk) in [0x4F, 0x4E, 0x54, 0x52, 0x41, 0x53, 0x45, 0xC0, 0x41].iter().enumerate() {
                e.mirar(100 * (i as u64 + 1) - 50, &Foto::default(), None, || None);
                e.mirar(100 * (i as u64 + 1), &Foto::con(&[*vk]), None, || None);
            }
            e.terminar()
        };
        assert_eq!(corto, largo, "los dos ratos tienen que ser indistinguibles");
        let json = serde_json::to_string(&largo).unwrap();
        assert_eq!(json, r#"[{"desdeMs":0,"hastaMs":900,"caret":null}]"#);
    }

    /// El cursor de texto de una ventana de verdad, hecha aquí, en este hilo y fuera de la
    /// pantalla: no aparece nada en el escritorio de nadie.
    #[test]
    fn el_cursor_de_texto_sale_en_coordenadas_del_escritorio() {
        use windows::core::w;
        use windows::Win32::System::Threading::GetCurrentThreadId;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateCaret, CreateWindowExW, DestroyCaret, DestroyWindow, SetCaretPos, WINDOW_EX_STYLE,
            WS_POPUP,
        };

        let ventana = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!(""),
                WS_POPUP,
                -20_000,
                -20_000,
                300,
                100,
                None,
                None,
                None,
                None,
            )
        }
        .expect("no se ha podido crear la ventana de prueba");
        unsafe { CreateCaret(ventana, None, 2, 16) }.expect("CreateCaret");
        unsafe { SetCaretPos(40, 10) }.expect("SetCaretPos");

        let hilo = unsafe { GetCurrentThreadId() };
        let caret = caret_de_hilo(hilo);

        let _ = unsafe { DestroyCaret() };
        let _ = unsafe { DestroyWindow(ventana) };

        // La ventana está en -20000, -20000; el cursor, a 40 y 10 dentro, con 16 de alto.
        assert_eq!(caret, Some((-20_000 + 40, -20_000 + 10 + 8)));
    }
}
