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
}
