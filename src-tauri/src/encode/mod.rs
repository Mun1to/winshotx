#[cfg(test)]
mod bench_gif;
#[cfg(test)]
mod bench_previa;
pub mod anotacion;
pub mod cursor;
pub mod escalar;
pub mod estudio;
pub mod ffmpeg;
pub mod gif;
pub mod jpg;
pub mod marco;
pub mod mp4;
pub mod png;
pub mod recorte;
pub mod zoom;
// Lo nuevo del plan de CleanShot X y Screen Studio, cada sesión debajo de su marca.
// S1 (línea de tiempo)
pub mod lupa;
// S3 (fondos)
pub mod fondo_video;
// S4 (capturas)
pub mod marco_captura;
// S7 (cursor)
pub mod clic;
// S8 (cámara en el Estudio)
pub mod camara_estudio;
// S9 (presets y tapar)
