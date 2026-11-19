//! Quitarle a Windows Impr Pant y Win+Mayús+S, y devolvérselas como estaban.
//!
//! Lo usan los dos interruptores de los ajustes (`commands::use_print_screen` y
//! `commands::use_win_shift_s`), y el registro de verdad lo escribe `platform::snipping`. Va
//! aparte, sobre un registro que se puede cambiar por uno de mentira, para que las cuentas de
//! qué se guarda y qué se devuelve se prueben sin tocar el registro de nadie.

use crate::error::Result;
use crate::settings::Settings;

/// Los dos valores del registro del usuario que winshotx toca para quedarse con las teclas.
pub(crate) trait RegistroDeTeclas {
    /// `PrintScreenKeyForSnippingEnabled`, o None si Windows nunca lo ha escrito.
    fn impr_pant(&self) -> Option<u32>;
    /// Lo pone, o lo borra con None.
    fn poner_impr_pant(&mut self, valor: Option<u32>) -> Result<()>;
    /// `DisabledHotkeys`, las letras que el escritorio deja de atender con la tecla Windows.
    fn letras(&self) -> Option<String>;
    /// Las pone, o borra el valor con None.
    fn poner_letras(&mut self, letras: Option<&str>) -> Result<()>;
}

/// El registro de verdad del usuario que corre winshotx.
pub(crate) struct RegistroDeVerdad;

impl RegistroDeTeclas for RegistroDeVerdad {
    fn impr_pant(&self) -> Option<u32> {
        crate::platform::snipping::read()
    }

    fn poner_impr_pant(&mut self, valor: Option<u32>) -> Result<()> {
        match valor {
            Some(valor) => crate::platform::snipping::write(valor),
            None => crate::platform::snipping::remove(),
        }
    }

    fn letras(&self) -> Option<String> {
        crate::platform::snipping::read_disabled_hotkeys()
    }

    fn poner_letras(&mut self, letras: Option<&str>) -> Result<()> {
        crate::platform::snipping::write_disabled_hotkeys(letras)
    }
}

/// Le quita Impr Pant a la Herramienta de Recortes.
pub(crate) fn tomar_impr_pant(registro: &mut impl RegistroDeTeclas, settings: &mut Settings) -> Result<()> {
    // Solo la primera vez: si ya estaba activo, lo guardado es el 0 que pusimos
    // nosotros y machacarlo seria perder el valor original del usuario.
    //
    // Y solo lo de Impr Pant: hasta el 19 de noviembre apuntaba aqui tambien las letras de
    // `DisabledHotkeys`, un resto de cuando las dos teclas iban en un solo interruptor
    // (`9fc8861`). Con Win+Mayus+S ya cogida, eso guardaba la S de winshotx como si fuera del
    // usuario, y Win+S no volvia nunca.
    if !settings.print_screen_capture {
        settings.snipping_key_restore = registro.impr_pant();
    }
    registro.poner_impr_pant(Some(0))?;
    settings.print_screen_capture = true;
    Ok(())
}

/// Le devuelve Impr Pant a la Herramienta de Recortes, con el valor que tenía antes.
pub(crate) fn devolver_impr_pant(registro: &mut impl RegistroDeTeclas, settings: &mut Settings) -> Result<()> {
    // Si winshotx no la cogió, el valor que haya es del usuario (lo pudo poner a 0 él mismo
    // en Configuración) y no se toca.
    if !settings.print_screen_capture {
        return Ok(());
    }
    // Se deja el registro como estaba, incluido el caso de que no hubiera valor.
    registro.poner_impr_pant(settings.snipping_key_restore.take())?;
    settings.print_screen_capture = false;
    Ok(())
}

/// Le quita Win+Mayús+S al escritorio, apuntando la S en `DisabledHotkeys`.
pub(crate) fn tomar_win_shift_s(registro: &mut impl RegistroDeTeclas, settings: &mut Settings) -> Result<()> {
    if !settings.take_win_shift_s {
        settings.disabled_hotkeys_restore = registro.letras();
    }
    // Se anade a lo que hay ahora en vez de sustituirlo: quien tuviera otras letras
    // apagadas las conserva.
    let actuales = registro.letras().unwrap_or_default();
    if !tiene_s(&actuales) {
        registro.poner_letras(Some(&format!("{actuales}S")))?;
    }
    settings.take_win_shift_s = true;
    Ok(())
}

/// Le devuelve Win+Mayús+S (y con ella Win+S) al escritorio. Dice si ha quitado la S.
///
/// Quita la S y nada más, y solo si no estaba antes de winshotx: las letras que el usuario
/// apagara por su cuenta, antes o después, se quedan. Hasta el 19 de noviembre se escribía la
/// lista de antes tal cual, y eso borraba lo que se hubiera apuntado mientras tanto.
pub(crate) fn devolver_win_shift_s(registro: &mut impl RegistroDeTeclas, settings: &mut Settings) -> Result<bool> {
    if !settings.take_win_shift_s {
        return Ok(false);
    }
    let antes = settings.disabled_hotkeys_restore.take();
    let ahora = registro.letras();
    let quedan = letras_sin_la_s_de_winshotx(ahora.as_deref(), antes.as_deref());
    let quitada = quedan != ahora;
    if quitada {
        registro.poner_letras(quedan.as_deref())?;
    }
    settings.take_win_shift_s = false;
    Ok(quitada)
}

/// Las letras que quedan al devolver la S: todas menos la S, salvo que ya estuviera antes.
fn letras_sin_la_s_de_winshotx(ahora: Option<&str>, antes: Option<&str>) -> Option<String> {
    let ahora = ahora.unwrap_or_default();
    let quedan: String = if antes.is_some_and(tiene_s) {
        ahora.to_owned()
    } else {
        ahora.chars().filter(|letra| !letra.eq_ignore_ascii_case(&'S')).collect()
    };
    (!quedan.is_empty()).then_some(quedan)
}

fn tiene_s(letras: &str) -> bool {
    letras.chars().any(|letra| letra.eq_ignore_ascii_case(&'S'))
}

/// Lo que hace el desinstalador: devolverle a Windows lo que winshotx le quitó, por el mismo
/// camino que apagar los dos interruptores.
///
/// Los ajustes NO se cambian: si quien desinstala se deja los datos y vuelve a instalar, o si
/// esto lo corre otro instalador que pone otra versión encima, winshotx arranca con sus
/// interruptores como estaban y vuelve a coger las teclas al arrancar (`lib.rs`), igual que
/// cuando una actualización de Windows se las devuelve a la Herramienta de Recortes.
///
/// Dice si ha quitado la S, que es lo único que no vuelve hasta que el escritorio relee la lista.
pub(crate) fn al_desinstalar(registro: &mut impl RegistroDeTeclas, settings: &Settings) -> Result<bool> {
    let mut copia = settings.clone();
    // Las dos, aunque falle la primera: lo que se pueda devolver, se devuelve.
    let impr_pant = devolver_impr_pant(registro, &mut copia);
    let win_s = devolver_win_shift_s(registro, &mut copia);
    impr_pant?;
    win_s
}

/// La orden con la que el desinstalador le pide a winshotx que devuelva las teclas
/// (`src-tauri/windows/hooks.nsh`).
pub const DEVOLVER_TECLAS: &str = "--restore-windows-keys";

/// Y la que añade cuando hay alguien delante para leer el aviso: sin ella, en una
/// desinstalación silenciosa (winget, `/S`), un cuadro de diálogo la dejaría parada.
pub const AVISAR: &str = "--tell";

/// `winshotx.exe --restore-windows-keys [--tell]`: devuelve las teclas y sale, sin abrir la app.
///
/// Va en `lib.rs::run` antes que nada, como las órdenes del agente: no necesita WebView2, y el
/// plugin de una sola instancia se tragaría el argumento si hubiera un winshotx abierto.
pub fn desde_la_linea_de_ordenes() -> Option<i32> {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    pide_devolver(&args).map(devolver_al_desinstalar)
}

/// None si no es esta orden; si lo es, si hay que avisar.
fn pide_devolver(args: &[String]) -> Option<bool> {
    (args.first().map(String::as_str) == Some(DEVOLVER_TECLAS))
        .then(|| args.iter().any(|arg| arg == AVISAR))
}

fn devolver_al_desinstalar(avisar: bool) -> i32 {
    // La copia de la Store nunca escribió en el registro de verdad, y tampoco se desinstala
    // por aquí; pero si alguien llama a la orden desde ella, no hay nada que devolver.
    if !crate::platform::snipping::windows_ve_el_registro() {
        return 0;
    }
    // Sin ajustes, winshotx no llegó a coger nada.
    let Some(settings) = crate::settings::leer_sin_la_app() else {
        return 0;
    };
    match al_desinstalar(&mut RegistroDeVerdad, &settings) {
        Ok(quitada) => {
            if quitada && avisar {
                avisar_de_win_s(settings.language);
            }
            0
        }
        Err(error) => {
            eprintln!("[teclas] no se han podido devolver a Windows: {error}");
            1
        }
    }
}

/// El aviso de que Win+S no vuelve en el acto, en el idioma de la app.
///
/// No vuelve en el acto porque el escritorio lee `DisabledHotkeys` al arrancar, y desde el
/// desinstalador no se reinicia el Explorador (ver `src-tauri/windows/hooks.nsh`).
// Solo lo usa el aviso de Windows; en Linux (rama `plan/s21-linux`) no hay desinstalador NSIS.
#[cfg_attr(not(windows), allow(dead_code))]
fn aviso_de_win_s(idioma: &str) -> &'static str {
    match idioma {
        "es" => "Win+S vuelve a abrir la búsqueda de Windows en cuanto cierres sesión y vuelvas a entrar, o al reiniciar el equipo.",
        "zh" => "注销并重新登录（或重启电脑）后，Win+S 将重新打开 Windows 搜索。",
        _ => "Win+S opens Windows search again once you sign out and back in, or restart your PC.",
    }
}

/// El idioma de la app, con «el del sistema» resuelto como en el frontend (`src/lib/i18n.ts`).
// Solo lo usa el aviso de Windows; en Linux (rama `plan/s21-linux`) no hay desinstalador NSIS.
#[cfg_attr(not(windows), allow(dead_code))]
fn idioma(elegido: crate::settings::Language, sistema: &str) -> &'static str {
    use crate::settings::Language;
    match elegido {
        Language::Es => "es",
        Language::En => "en",
        Language::Zh => "zh",
        Language::Sistema => {
            let sistema = sistema.to_lowercase();
            if sistema.starts_with("es") {
                "es"
            } else if sistema.starts_with("zh") {
                "zh"
            } else {
                "en"
            }
        }
    }
}

#[cfg(windows)]
fn avisar_de_win_s(elegido: crate::settings::Language) {
    use windows::core::HSTRING;
    use windows::Win32::Globalization::GetUserDefaultLocaleName;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
    };

    let mut buffer = [0u16; 85];
    let largo = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    let sistema = if largo > 0 {
        String::from_utf16_lossy(&buffer[..(largo as usize - 1).min(buffer.len())])
    } else {
        String::new()
    };
    let texto = aviso_de_win_s(idioma(elegido, &sistema));
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(texto),
            &HSTRING::from("winshotx"),
            MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND | MB_TOPMOST,
        );
    }
}

#[cfg(not(windows))]
fn avisar_de_win_s(_elegido: crate::settings::Language) {}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un registro en memoria que empieza como se le diga.
    #[derive(Default)]
    struct DeMentira {
        impr_pant: Option<u32>,
        letras: Option<String>,
    }

    impl RegistroDeTeclas for DeMentira {
        fn impr_pant(&self) -> Option<u32> {
            self.impr_pant
        }
        fn poner_impr_pant(&mut self, valor: Option<u32>) -> Result<()> {
            self.impr_pant = valor;
            Ok(())
        }
        fn letras(&self) -> Option<String> {
            self.letras.clone()
        }
        fn poner_letras(&mut self, letras: Option<&str>) -> Result<()> {
            self.letras = letras.filter(|l| !l.is_empty()).map(str::to_owned);
            Ok(())
        }
    }

    fn registro(impr_pant: Option<u32>, letras: Option<&str>) -> DeMentira {
        DeMentira { impr_pant, letras: letras.map(str::to_owned) }
    }

    /// El fallo que dejó `9fc8861` (25 de agosto): al separar los dos interruptores, encender
    /// Impr Pant siguió apuntando las letras de ese momento como «las de antes». Con Win+Mayús+S
    /// ya puesta, eso incluye la S de winshotx, y apagar después Win+Mayús+S la volvía a escribir:
    /// Win+S no volvía nunca.
    #[test]
    fn coger_impr_pant_despues_no_hace_suya_la_s_de_winshotx() {
        let mut reg = registro(None, None);
        let mut settings = Settings::default();
        tomar_win_shift_s(&mut reg, &mut settings).unwrap();
        assert_eq!(reg.letras.as_deref(), Some("S"));
        tomar_impr_pant(&mut reg, &mut settings).unwrap();

        devolver_win_shift_s(&mut reg, &mut settings).unwrap();
        assert_eq!(reg.letras, None, "Win+S se quedó apagada");
    }

    /// Apagar el interruptor quita la S y nada más: las letras que el usuario apagó por su
    /// cuenta mientras tanto se quedan.
    #[test]
    fn apagar_win_shift_s_deja_las_letras_que_se_pusieron_despues() {
        let mut reg = registro(None, Some("Q"));
        let mut settings = Settings::default();
        tomar_win_shift_s(&mut reg, &mut settings).unwrap();
        assert_eq!(reg.letras.as_deref(), Some("QS"));
        reg.letras = Some("QSV".into());

        assert!(devolver_win_shift_s(&mut reg, &mut settings).unwrap());
        assert_eq!(reg.letras.as_deref(), Some("QV"));
        assert!(!settings.take_win_shift_s);
    }

    /// Si winshotx no cogió Impr Pant, el 0 que haya lo puso el usuario en Configuración, y
    /// apagar el interruptor no lo borra.
    #[test]
    fn apagar_impr_pant_sin_haberla_cogido_no_toca_el_valor() {
        let mut reg = registro(Some(0), None);
        let mut settings = Settings::default();
        devolver_impr_pant(&mut reg, &mut settings).unwrap();
        assert_eq!(reg.impr_pant, Some(0));
    }

    /// Si la S ya estaba antes de winshotx, no es suya y no la quita.
    #[test]
    fn apagar_win_shift_s_deja_la_s_que_ya_estaba() {
        let mut reg = registro(None, Some("S"));
        let mut settings = Settings::default();
        tomar_win_shift_s(&mut reg, &mut settings).unwrap();
        assert_eq!(reg.letras.as_deref(), Some("S"));

        assert!(!devolver_win_shift_s(&mut reg, &mut settings).unwrap(), "no había S nuestra que quitar");
        assert_eq!(reg.letras.as_deref(), Some("S"));
    }

    /// La orden solo cuenta en primer lugar, como `agent` y `mcp`, y `--tell` es lo único
    /// que cambia si se avisa.
    #[test]
    fn la_orden_de_devolver_se_reconoce() {
        let args = |lista: &[&str]| lista.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        assert_eq!(pide_devolver(&args(&["--restore-windows-keys"])), Some(false));
        assert_eq!(pide_devolver(&args(&["--restore-windows-keys", "--tell"])), Some(true));
        assert_eq!(pide_devolver(&args(&[])), None);
        assert_eq!(pide_devolver(&args(&["--autostart"])), None);
        assert_eq!(pide_devolver(&args(&["--capture", "--restore-windows-keys"])), None);
    }

    /// El aviso sale en el idioma de la app, y con «el del sistema» igual que la interfaz.
    #[test]
    fn el_aviso_de_win_s_sale_en_el_idioma_de_la_app() {
        use crate::settings::Language;
        assert_eq!(idioma(Language::Sistema, "es-ES"), "es");
        assert_eq!(idioma(Language::Sistema, "zh-CN"), "zh");
        assert_eq!(idioma(Language::Sistema, "de-DE"), "en");
        assert_eq!(idioma(Language::Sistema, ""), "en");
        assert_eq!(idioma(Language::En, "es-ES"), "en");
        for (codigo, palabra) in [("es", "cierres sesión"), ("en", "sign out"), ("zh", "注销")] {
            let texto = aviso_de_win_s(codigo);
            assert!(texto.contains("Win+S") && texto.contains(palabra), "{codigo}: {texto}");
        }
    }

    /// El desinstalador y esta orden tienen que hablar de lo mismo: si cambia el nombre de la
    /// orden o se pierde el gancho de `tauri.conf.json`, desinstalar vuelve a dejar Win+S
    /// apagada y no se entera nadie.
    #[test]
    fn el_desinstalador_llama_a_la_orden_de_verdad() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("leer tauri.conf.json");
        assert_eq!(
            conf["bundle"]["windows"]["nsis"]["installerHooks"].as_str(),
            Some("./windows/hooks.nsh")
        );
        let gancho = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/windows/hooks.nsh"))
            .expect("falta src-tauri/windows/hooks.nsh");
        assert!(gancho.contains("!macro NSIS_HOOK_PREUNINSTALL"));
        assert!(gancho.contains(&format!("{DEVOLVER_TECLAS}'")), "la orden sin aviso");
        assert!(gancho.contains(&format!("{DEVOLVER_TECLAS} {AVISAR}'")), "la orden con aviso");
        // Actualizar no devuelve nada (ver el propio gancho).
        assert!(gancho.contains("${If} $UpdateMode <> 1"));
        assert!(gancho.contains("${AndIf} $EXEDIR != $INSTDIR"));
    }

    /// El encargo del 19 de noviembre: quien desinstala con las dos teclas cogidas se queda con
    /// Windows como estaba antes de winshotx.
    #[test]
    fn desinstalar_devuelve_las_dos_teclas() {
        let mut reg = registro(Some(1), Some("Q"));
        let mut settings = Settings::default();
        tomar_impr_pant(&mut reg, &mut settings).unwrap();
        tomar_win_shift_s(&mut reg, &mut settings).unwrap();
        assert_eq!((reg.impr_pant, reg.letras.as_deref()), (Some(0), Some("QS")));

        assert!(al_desinstalar(&mut reg, &settings).unwrap(), "tiene que decir que ha quitado la S");
        assert_eq!(reg.impr_pant, Some(1), "Impr Pant vuelve a su valor de antes");
        assert_eq!(reg.letras.as_deref(), Some("Q"), "solo se va la S de winshotx");
    }

    /// Windows 11 de fábrica no trae `PrintScreenKeyForSnippingEnabled`: si no estaba, se borra.
    #[test]
    fn desinstalar_borra_impr_pant_si_no_estaba() {
        let mut reg = registro(None, None);
        let mut settings = Settings::default();
        tomar_impr_pant(&mut reg, &mut settings).unwrap();
        assert_eq!(reg.impr_pant, Some(0));

        assert!(!al_desinstalar(&mut reg, &settings).unwrap(), "la S no se tocó");
        assert_eq!(reg.impr_pant, None);
    }

    /// La S que ya estaba antes de winshotx se queda al desinstalar.
    #[test]
    fn desinstalar_deja_la_s_que_ya_estaba() {
        let mut reg = registro(None, Some("S"));
        let mut settings = Settings::default();
        tomar_win_shift_s(&mut reg, &mut settings).unwrap();

        assert!(!al_desinstalar(&mut reg, &settings).unwrap());
        assert_eq!(reg.letras.as_deref(), Some("S"));
    }

    /// Lo que winshotx no cogió no es suyo: con los interruptores apagados no toca nada, ni el
    /// 0 ni la S que el usuario puso a mano.
    #[test]
    fn desinstalar_no_toca_lo_que_no_cogio_winshotx() {
        let mut reg = registro(Some(0), Some("S"));
        let settings = Settings::default();

        assert!(!al_desinstalar(&mut reg, &settings).unwrap());
        assert_eq!((reg.impr_pant, reg.letras.as_deref()), (Some(0), Some("S")));
    }
}
