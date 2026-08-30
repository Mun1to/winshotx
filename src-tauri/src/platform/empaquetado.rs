//! ¿Estamos corriendo dentro de un paquete MSIX, o suelta desde el instalador?
//!
//! Dentro de la Microsoft Store cambian dos cosas y hay que saberlo en las dos puntas:
//! el arranque con Windows no puede ir por el registro (queda virtualizado y el sistema
//! nunca lo lee) y actualizarse a mano es imposible, porque la carpeta de la app es de
//! solo lectura y de eso se encarga la propia Store.
//!
//! Se pregunta al sistema una vez y se guarda: no cambia mientras el proceso viva.

#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
static ES_MSIX: OnceLock<bool> = OnceLock::new();

/// `true` solo si el proceso tiene identidad de paquete, que es lo que da la Store.
#[cfg(windows)]
pub fn es_msix() -> bool {
    *ES_MSIX.get_or_init(|| {
        use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
        use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;

        // No se quiere el nombre, solo saber si lo hay: sin paquete devuelve
        // APPMODEL_ERROR_NO_PACKAGE, y con paquete se queja del hueco vacío.
        let mut largo = 0u32;
        let estado = unsafe { GetCurrentPackageFullName(&mut largo, None) };
        estado == ERROR_INSUFFICIENT_BUFFER
    })
}

#[cfg(not(windows))]
pub fn es_msix() -> bool {
    false
}
