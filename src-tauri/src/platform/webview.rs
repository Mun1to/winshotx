//! Que winshotx no arranque nunca ensennando una pagina de error.
//!
//! **De donde sale esto.** El 1 de septiembre de 2026 la Microsoft Store rechazo el envio
//! de winshotx con este motivo, politica `10.1.2.10 Functionality`:
//!
//! > Unusable Feature: **Display error page at launch**.
//! > Observed On: ASUS EXPERTBOOK P5405CSA, OS build 26200.9168
//!
//! winshotx pinta toda su interfaz con WebView2, que es el motor de Edge. Windows 11 y un
//! Windows 10 al dia lo traen puesto, pero **no esta garantizado**: una instalacion recien
//! hecha, una imagen corporativa recortada o una maquina de pruebas pueden no tenerlo. Y sin
//! el, la ventana se abre igual y lo que aparece dentro es la pagina de error del navegador,
//! que no le dice a nadie ni que ha pasado ni que hacer.
//!
//! Aqui se comprueba ANTES de crear ninguna ventana. Si no esta, se dice con un cuadro de
//! dialogo del sistema (que no necesita WebView2 para pintarse) y la aplicacion se cierra
//! sin abrir nada.

/// Que version de WebView2 hay en esta maquina, o nada si no hay ninguna.
#[cfg(windows)]
pub fn version() -> Option<String> {
    tauri::webview_version().ok()
}

#[cfg(not(windows))]
pub fn version() -> Option<String> {
    Some(String::new())
}

/// El texto del aviso, en los dos idiomas de la aplicacion.
///
/// Aparte y puro para poder probarlo: lo que aqui importa es que diga QUE falta y COMO se
/// arregla, porque quien lo lea no va a tener ni consola ni interfaz donde mirar nada mas.
/// El idioma no puede salir de `i18n`, que vive en el frontend y es justo lo que no hay.
pub fn aviso(en_espannol: bool) -> (String, String) {
    if en_espannol {
        (
            "winshotx necesita WebView2".to_string(),
            "winshotx dibuja su ventana con WebView2, el motor de Edge, y en este equipo no \
             está instalado.\n\n\
             Se descarga gratis de Microsoft, buscando «WebView2 Runtime», y winshotx \
             funcionará en cuanto lo instales.\n\n\
             Windows 11 y Windows 10 actualizado lo traen de serie, así que esto no suele \
             pasar."
                .to_string(),
        )
    } else {
        (
            "winshotx needs WebView2".to_string(),
            "winshotx draws its window with WebView2, the Edge engine, and it is not \
             installed on this machine.\n\n\
             It is a free download from Microsoft, search for \"WebView2 Runtime\", and \
             winshotx will work as soon as it is installed.\n\n\
             Windows 11 and an up to date Windows 10 ship with it, so this is unusual."
                .to_string(),
        )
    }
}

/// Si el idioma del sistema es espannol. Sin `i18n`, que vive en el frontend.
#[cfg(windows)]
fn sistema_en_espannol() -> bool {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;
    let mut buffer = [0u16; 85];
    let largo = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    if largo <= 0 {
        return false;
    }
    let nombre = String::from_utf16_lossy(&buffer[..(largo as usize - 1).min(buffer.len())]);
    nombre.to_lowercase().starts_with("es")
}

/// Comprueba WebView2 y, si no esta, lo dice y devuelve `false` para que nadie siga.
///
/// El cuadro de dialogo es de Windows, no de la aplicacion: tiene que poder pintarse
/// precisamente cuando lo que no funciona es lo que pinta la aplicacion.
#[cfg(windows)]
pub fn hay_con_que_pintar() -> bool {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    if version().is_some() {
        return true;
    }

    let (titulo, cuerpo) = aviso(sistema_en_espannol());
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(cuerpo),
            &HSTRING::from(titulo),
            MB_OK | MB_ICONERROR,
        );
    }
    false
}

#[cfg(not(windows))]
pub fn hay_con_que_pintar() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::aviso;

    /// El aviso tiene que decir las dos cosas que necesita quien lo lee: que falta y como se
    /// consigue. Sin eso es una pantalla de error mas educada, que es lo que se venia a
    /// evitar.
    #[test]
    fn el_aviso_dice_que_falta_y_donde_se_consigue() {
        for espannol in [true, false] {
            let (titulo, cuerpo) = aviso(espannol);
            assert!(titulo.contains("WebView2"), "el título no nombra lo que falta");
            assert!(cuerpo.contains("WebView2"), "el cuerpo no nombra lo que falta");
            assert!(
                cuerpo.to_lowercase().contains("microsoft"),
                "no dice de dónde se saca"
            );
        }
    }

    /// Los dos idiomas de la aplicación, y distintos de verdad: una traducción que se olvida
    /// deja a media aplicación hablando el idioma que no es, y este texto sale justo cuando
    /// no hay nada más que leer.
    #[test]
    fn hay_texto_en_los_dos_idiomas() {
        let (_, es) = aviso(true);
        let (_, en) = aviso(false);
        assert_ne!(es, en);
        assert!(es.contains("no está instalado"), "el castellano no dice lo que pasa");
        assert!(en.contains("not installed"), "el inglés no dice lo que pasa");
    }
}
