//! Activación nativa del Explorador mediante la asociación de ms-screenclip.
use crate::windows_mgr::OverlayIntent;

const BANDERA: &str = "--screenclip";
pub(crate) const AJUSTES: &str = "ms-settings:defaultapps?registeredAppUser=winshotx";

pub(crate) enum Solicitud<'a> {
    Recortar(OverlayIntent),
    OtroEnlace(&'a str),
    Invalida,
}

/// Solo el formulario del shell activa nuestra selección. Las llamadas de apps al
/// protocolo de Microsoft pueden pedir resultados o capacidades y siguen su camino.
pub(crate) fn solicitud(argumentos: &[String]) -> Option<Solicitud<'_>> {
    if argumentos.first().map(String::as_str) != Some(BANDERA) {
        return None;
    }
    let [_, enlace] = argumentos else {
        return Some(Solicitud::Invalida);
    };
    let Ok(uri) = tauri::Url::parse(enlace) else {
        return Some(Solicitud::Invalida);
    };
    if uri.scheme() != "ms-screenclip" {
        return Some(Solicitud::Invalida);
    }
    if uri.host_str().is_some_and(|host| !host.is_empty())
        || !matches!(uri.path(), "" | "/")
        || uri.fragment().is_some()
    {
        return Some(Solicitud::OtroEnlace(enlace));
    }
    let mut grabar = false;
    for (clave, valor) in uri.query_pairs() {
        match (
            clave.to_ascii_lowercase().as_str(),
            valor.to_ascii_lowercase().as_str(),
        ) {
            ("type", "recording") | ("source", "screenrecorderhotkey") => grabar = true,
            ("type", "snipping")
            | ("source", "hotkey" | "printscreen" | "penbutton" | "quickactions") => (),
            _ => return Some(Solicitud::OtroEnlace(enlace)),
        }
    }
    Some(Solicitud::Recortar(if grabar {
        OverlayIntent::GrabarWindows
    } else {
        OverlayIntent::CapturarWindows
    }))
}

pub(crate) fn disparar(app: &tauri::AppHandle, intent: OverlayIntent) {
    use tauri::Manager;

    let resultado = if intent == OverlayIntent::GrabarWindows
        && app.state::<crate::state::AppState>().is_recording()
    {
        crate::recorder::stop(app)
    } else {
        crate::windows_mgr::open_overlays(app, intent)
    };
    if let Err(error) = resultado {
        eprintln!("[ms-screenclip] {error}");
        crate::activacion::salir_si_termino(app);
    }
}

pub(crate) fn es_predeterminado() -> bool {
    use windows::core::{w, PCWSTR, PWSTR};
    use windows::Win32::UI::Shell::{AssocQueryStringW, ASSOCF_IS_PROTOCOL, ASSOCSTR_PROGID};

    let mut progid = [0u16; 128];
    let mut largo = progid.len() as u32;
    let resultado = unsafe {
        AssocQueryStringW(
            ASSOCF_IS_PROTOCOL,
            ASSOCSTR_PROGID,
            w!("ms-screenclip"),
            PCWSTR::null(),
            Some(PWSTR(progid.as_mut_ptr())),
            &mut largo,
        )
    };
    resultado.is_ok()
        && String::from_utf16_lossy(
            &progid[..largo.saturating_sub(1).min(progid.len() as u32) as usize],
        )
        .eq_ignore_ascii_case("winshotx.ScreenClip")
}

/// Conserva, si sigue instalada, la aplicación a la que iban las solicitudes de terceros.
/// El destino explícito evita que Windows vuelva a invocar esta asociación recursivamente.
pub(crate) fn reenviar(enlace: &str) -> crate::error::Result<()> {
    use windows::core::HSTRING;
    use windows::Foundation::{Collections::ValueSet, Uri};
    use windows::System::{Launcher, LauncherOptions};
    use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

    unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };
    let resultado = (|| {
        let opciones = LauncherOptions::new()?;
        opciones.SetTargetApplicationPackageFamilyName(&HSTRING::from(
            "Microsoft.ScreenSketch_8wekyb3d8bbwe",
        ))?;
        let uri = Uri::CreateUri(&HSTRING::from(enlace))?;
        if Launcher::LaunchUriWithDataAsync(&uri, &opciones, &ValueSet::new()?)?.join()? {
            Ok(())
        } else {
            Err("Windows no pudo abrir la Herramienta de Recortes".into())
        }
    })();
    unsafe { RoUninitialize() };
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pedir(uri: &str) -> OverlayIntent {
        match solicitud(&[BANDERA.into(), uri.into()]) {
            Some(Solicitud::Recortar(intent)) => intent,
            _ => panic!("no es una activación de Windows"),
        }
    }

    #[test]
    fn distingue_las_dos_teclas_del_explorador() {
        assert_eq!(
            pedir("ms-screenclip://?source=HotKey"),
            OverlayIntent::CapturarWindows
        );
        assert_eq!(
            pedir("ms-screenclip://?source=ScreenRecorderHotKey&type=recording"),
            OverlayIntent::GrabarWindows,
        );
    }

    #[test]
    fn rechaza_otros_protocolos_y_deriva_solicitudes_de_apps() {
        assert!(matches!(
            solicitud(&[BANDERA.into(), "https://example.com".into()]),
            Some(Solicitud::Invalida)
        ));
        let enlace = "ms-screenclip://capture/video?redirect-uri=example%3Aresponse";
        assert!(matches!(
            solicitud(&[BANDERA.into(), enlace.into()]),
            Some(Solicitud::OtroEnlace(_))
        ));
    }
}
