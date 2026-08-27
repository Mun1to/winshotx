use serde::{Serialize, Serializer};

/// Un solo tipo de error para todo el backend: se serializa como texto plano,
/// que es lo que el frontend muestra en el panel de exportacion.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("no hay ninguna grabación en curso")]
    NoRecording,
    #[error("sesión desconocida: {0}")]
    UnknownSession(String),
    #[error("esta función solo está implementada en Windows")]
    #[allow(dead_code)]
    Unsupported,
    #[error("error de entrada/salida: {0}")]
    Io(#[from] std::io::Error),
    #[error("error de Tauri: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("error de imagen: {0}")]
    Image(#[from] image::ImageError),
    #[error("error de JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("error de GIF: {0}")]
    Gif(#[from] gif::EncodingError),
    #[error("error de QOI: {0}")]
    Qoi(#[from] qoi::Error),
    /// Lo que devuelven las APIs modernas de Windows (WinRT), como el lector de texto.
    #[cfg(windows)]
    #[error("error de Windows: {0}")]
    Windows(#[from] windows::core::Error),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        AppError::Msg(value)
    }
}

impl From<&str> for AppError {
    fn from(value: &str) -> Self {
        AppError::Msg(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
