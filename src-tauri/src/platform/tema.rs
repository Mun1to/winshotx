//! S25 (marco): si las ventanas de winshotx van en oscuro.
//!
//! Lo pregunta el marco de las capturas sueltas: la ventana de Windows 11 que envuelve una
//! captura sale clara u oscura como las de winshotx, y en automático (el de fábrica) como las
//! de Windows. El editor decide lo mismo en su lado (`src/lib/carcasa.ts::ventanaOscura`).

use crate::settings::Theme;

/// Si con ese tema de winshotx las ventanas van en oscuro.
pub fn oscuro(tema: Theme) -> bool {
    match tema {
        Theme::Oscuro => true,
        Theme::Claro => false,
        Theme::Sistema => sistema_oscuro(),
    }
}

/// Lo que tiene puesto Windows para las aplicaciones: `AppsUseLightTheme` a cero es oscuro.
/// Sin el valor (un Windows sin tocar), claro, que es lo que trae de fábrica.
fn sistema_oscuro() -> bool {
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

        let mut dato = 1u32;
        let mut largo = std::mem::size_of::<u32>() as u32;
        let hecho = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
                &HSTRING::from("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some((&raw mut dato).cast()),
                Some(&mut largo),
            )
        };
        return hecho.is_ok() && dato == 0;
    }
    #[cfg(not(windows))]
    {
        return false;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_tema_elegido_manda_sobre_el_de_windows() {
        assert!(oscuro(Theme::Oscuro));
        assert!(!oscuro(Theme::Claro));
        // El del sistema depende de la máquina: solo que conteste sin reventar.
        let _ = oscuro(Theme::Sistema);
    }
}
