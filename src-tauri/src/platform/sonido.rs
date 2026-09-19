//! El clic de obturador, que suena al capturar si el ajuste esta encendido.
//!
//! **El WAV va dentro del binario** (`include_bytes!`), no al lado del ejecutable: son 12 KB
//! y asi no hay un archivo suelto que alguien pueda borrar, ni una ruta que resolver en
//! tiempo de ejecucion. El instalador pasa de 2,48 a 2,49 MB.
//!
//! Y suena con `PlaySoundW`, que es lo que trae Windows: reproducir un sonido corto no
//! justifica meter un motor de audio con sus hilos y sus dependencias. A cambio, **solo
//! sabe leer WAV PCM**, y por eso la prueba de aqui abajo comprueba la cabecera: un mp3
//! renombrado a `.wav` no sonaria, y no se enteraria nadie hasta que alguien capturase.

/// El sonido, ya recortado a los tres golpes de un obturador de reflex.
const CLIC: &[u8] = include_bytes!("../../assets/obturador.wav");

/// Suena el obturador, sin esperar a que termine.
///
/// No devuelve error a proposito: que no suene un clic no puede estropear una captura, y
/// una captura que falla porque el audio esta ocupado seria mucho peor que el silencio.
pub fn obturador() {
    #[cfg(windows)]
    {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
        use windows::core::PCWSTR;

        // SND_MEMORY hace que el primer parametro sea un puntero a los bytes del WAV y no
        // una ruta. El buffer es `'static`, asi que sigue vivo mientras suena en su hilo.
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(CLIC.as_ptr() as *const u16),
                None,
                // NODEFAULT: si el WAV no se puede tocar, silencio. Sin esto Windows suelta
                // su pitido de sistema, que es peor que no sonar nada.
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Que lo que se empaqueta sea de verdad lo unico que `PlaySoundW` sabe tocar.
    ///
    /// Munir, el 30 de agosto de 2026: *«y no suena ningun sonido xd»*. Entonces era que
    /// nadie leia el ajuste; ahora que si se lee, el riesgo se muda al archivo.
    #[test]
    fn el_clic_es_un_wav_pcm_que_windows_sabe_tocar() {
        assert_eq!(&CLIC[0..4], b"RIFF", "no empieza por RIFF");
        assert_eq!(&CLIC[8..12], b"WAVE", "no es un WAVE");
        assert_eq!(&CLIC[12..16], b"fmt ", "no trae el bloque de formato");

        let leer16 = |i: usize| u16::from_le_bytes([CLIC[i], CLIC[i + 1]]);
        let leer32 = |i: usize| u32::from_le_bytes([CLIC[i], CLIC[i + 1], CLIC[i + 2], CLIC[i + 3]]);

        assert_eq!(leer16(20), 1, "no es PCM sin comprimir");
        assert_eq!(leer16(22), 1, "tiene que ser mono");
        assert_eq!(leer32(24), 44_100, "tiene que ir a 44.100 Hz");
        assert_eq!(leer16(34), 16, "tiene que ser de 16 bits");

        // Y que siga siendo un clic y no una cancion: al capturar, el sonido tiene que
        // haber acabado antes de que el usuario mire el resultado.
        let ms = (CLIC.len() as u64 - 44) * 1000 / (44_100 * 2);
        assert!((80..=300).contains(&ms), "dura {ms} ms, y eso no es un clic");
    }

    /// Y que suene de verdad. Va con `--ignored` porque **hace ruido en el equipo**.
    #[test]
    #[ignore = "suena por los altavoces de quien lo corra"]
    fn suena() {
        obturador();
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
}
