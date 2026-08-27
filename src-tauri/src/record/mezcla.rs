//! Juntar dos sonidos que no van al mismo ritmo ni tienen los mismos canales.
//!
//! El altavoz y el micrófono son dos aparatos distintos con dos relojes distintos: el
//! mezclador suele entregar 48.000 muestras por segundo en estéreo, y un micrófono de
//! diadema 44.100 en mono. Para que la voz entre en el mismo vídeo que el sonido del
//! sistema hay que llevarla al formato del otro y sumarlas.
//!
//! Todo lo de aquí son funciones sin estado que reciben números y devuelven números, para
//! poder comprobarlas sin abrir ningún aparato. El trabajo con los dispositivos, que es lo
//! que no se puede probar en una máquina cualquiera, se queda en `audio.rs`.

use super::audio::Formato;

/// Lleva un trozo de sonido de un formato a otro: primero los canales, luego el ritmo.
///
/// En ese orden porque cambiar de canales es exacto y cambiar de ritmo es una
/// aproximación: conviene aproximar una sola vez y sobre el número de canales definitivo.
pub fn adaptar(muestras: &[f32], de: Formato, a: Formato) -> Vec<f32> {
    let con_canales = cambiar_canales(muestras, de.canales, a.canales);
    remuestrear(
        &con_canales,
        a.canales.max(1) as usize,
        de.muestras_por_segundo,
        a.muestras_por_segundo,
    )
}

/// De mono a estéreo se copia el mismo sonido a los dos lados; de estéreo a mono se
/// promedian. Es lo que hace cualquier mezclador y no hay decisión que tomar.
fn cambiar_canales(muestras: &[f32], de: u16, a: u16) -> Vec<f32> {
    let de = de.max(1) as usize;
    let a = a.max(1) as usize;
    if de == a {
        return muestras.to_vec();
    }
    let mut salida = Vec::with_capacity(muestras.len() / de * a);
    for instante in muestras.chunks_exact(de) {
        // La media de lo que venía, repetida en cada canal de salida. Con 1 a 2 eso es
        // copiar, y con 2 a 1 es promediar, que son los dos casos que se dan de verdad.
        let media = instante.iter().sum::<f32>() / de as f32;
        for _ in 0..a {
            salida.push(media);
        }
    }
    salida
}

/// Cambia cuántas muestras por segundo tiene un trozo, interpolando entre las vecinas.
///
/// Es una interpolación lineal, no un remuestreador de los buenos: para voz y para sonido
/// de escritorio no se distingue, y uno de los buenos es una biblioteca entera para una
/// diferencia que nadie va a oír en la narración de un tutorial.
fn remuestrear(muestras: &[f32], canales: usize, de: u32, a: u32) -> Vec<f32> {
    if de == a || de == 0 || a == 0 || muestras.is_empty() {
        return muestras.to_vec();
    }
    let instantes = muestras.len() / canales;
    if instantes == 0 {
        return Vec::new();
    }
    let salida_instantes = ((instantes as u64 * a as u64) / de as u64).max(1) as usize;
    let paso = instantes as f32 / salida_instantes as f32;

    let mut salida = Vec::with_capacity(salida_instantes * canales);
    for i in 0..salida_instantes {
        let sitio = i as f32 * paso;
        let izquierda = sitio.floor() as usize;
        let derecha = (izquierda + 1).min(instantes - 1);
        let peso = sitio - izquierda as f32;
        for c in 0..canales {
            let a0 = muestras[izquierda * canales + c];
            let a1 = muestras[derecha * canales + c];
            salida.push(a0 + (a1 - a0) * peso);
        }
    }
    salida
}

/// Suma el segundo sonido sobre el primero, sin pasarse de los topes.
///
/// **Se recorta a [-1, 1] al sumar.** Dos sonidos fuertes a la vez se salen del rango, y
/// una muestra por encima de uno da la vuelta al convertirla a entero: en vez de sonar más
/// alto, suena a chasquido. Es la misma razón por la que `audio::a_pcm16` recorta.
///
/// Si el segundo es más corto, el resto del primero se queda como estaba: el micrófono se
/// calla y sigue oyéndose el sistema, que es justo lo que tiene que pasar.
pub fn sumar(base: &mut [f32], encima: &[f32]) {
    for (destino, extra) in base.iter_mut().zip(encima) {
        *destino = (*destino + *extra).clamp(-1.0, 1.0);
    }
}

/// Lee un trozo de bytes de coma flotante de 32 bits como muestras sueltas.
pub fn como_flotantes(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|m| f32::from_le_bytes([m[0], m[1], m[2], m[3]]))
        .collect()
}

/// Y la vuelta, para devolvérselo al codificador en los bytes que espera.
pub fn como_bytes(muestras: &[f32]) -> Vec<u8> {
    let mut salida = Vec::with_capacity(muestras.len() * 4);
    for m in muestras {
        salida.extend_from_slice(&m.to_le_bytes());
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formato(canales: u16, hz: u32) -> Formato {
        Formato {
            canales,
            muestras_por_segundo: hz,
            bits_por_muestra: 32,
        }
    }

    #[test]
    fn de_mono_a_estereo_se_copia_a_los_dos_lados() {
        let salida = adaptar(&[0.5, -0.25], formato(1, 48_000), formato(2, 48_000));
        assert_eq!(salida, vec![0.5, 0.5, -0.25, -0.25]);
    }

    #[test]
    fn de_estereo_a_mono_se_promedian_los_dos_lados() {
        let salida = adaptar(&[1.0, 0.0, -1.0, 1.0], formato(2, 48_000), formato(1, 48_000));
        assert_eq!(salida, vec![0.5, 0.0]);
    }

    #[test]
    fn el_mismo_formato_no_toca_nada() {
        let dentro = vec![0.1, 0.2, 0.3, 0.4];
        assert_eq!(adaptar(&dentro, formato(2, 48_000), formato(2, 48_000)), dentro);
    }

    #[test]
    fn subir_el_ritmo_alarga_el_trozo_en_la_proporcion_justa() {
        // 4 instantes en mono a 24.000 pasan a 8 a 48.000.
        let salida = adaptar(&[0.0, 1.0, 0.0, -1.0], formato(1, 24_000), formato(1, 48_000));
        assert_eq!(salida.len(), 8);
    }

    #[test]
    fn bajar_el_ritmo_lo_acorta() {
        let dentro: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        // 44.100 a 22.050 es justo la mitad.
        let salida = adaptar(&dentro, formato(1, 44_100), formato(1, 22_050));
        assert_eq!(salida.len(), 50);
    }

    /// El caso real: un micrófono de 44.100 en mono metido en un vídeo de 48.000 estéreo.
    #[test]
    fn el_caso_de_verdad_es_un_microfono_mono_a_44100_en_un_video_estereo_a_48000() {
        // Un segundo de micrófono.
        let voz = vec![0.3f32; 44_100];
        let salida = adaptar(&voz, formato(1, 44_100), formato(2, 48_000));
        // Un segundo en estéreo a 48.000 son 96.000 muestras. Se admite un instante de
        // diferencia por el redondeo, no más.
        assert!(
            (salida.len() as i64 - 96_000).abs() <= 2,
            "han salido {} muestras y se esperaban unas 96.000",
            salida.len()
        );
        // Y el sonido sigue valiendo lo mismo: interpolar entre dos valores iguales da ese
        // valor, así que si esto cambia es que la interpolación está mal.
        assert!((salida[1000] - 0.3).abs() < 0.001);
    }

    /// Comparar sumas de coma flotante con `==` es pedir que falle: 0.2 + 0.1 no da 0.3
    /// exacto en binario, ni aqui ni en ningun sitio.
    fn casi(a: &[f32], b: &[f32]) {
        assert_eq!(a.len(), b.len(), "distinto numero de muestras");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!((x - y).abs() < 1e-5, "muestra {i}: {x} contra {y}");
        }
    }

    #[test]
    fn sumar_junta_los_dos_sonidos() {
        let mut base = vec![0.2, -0.3];
        sumar(&mut base, &[0.1, 0.1]);
        casi(&base, &[0.3, -0.2]);
    }

    #[test]
    fn sumar_recorta_en_vez_de_dar_la_vuelta() {
        // Sin recortar, 0.9 + 0.8 = 1.7, que al pasar a entero de 16 bits da la vuelta y
        // suena a chasquido en vez de sonar más alto.
        let mut base = vec![0.9, -0.9];
        sumar(&mut base, &[0.8, -0.8]);
        assert_eq!(base, vec![1.0, -1.0]);
    }

    #[test]
    fn si_el_microfono_se_queda_corto_el_resto_sigue_sonando() {
        let mut base = vec![0.5, 0.5, 0.5, 0.5];
        sumar(&mut base, &[0.1, 0.1]);
        casi(&base, &[0.6, 0.6, 0.5, 0.5]);
    }

    #[test]
    fn ir_y_volver_entre_bytes_y_muestras_no_pierde_nada() {
        let muestras = vec![0.0, 1.0, -1.0, 0.375];
        assert_eq!(como_flotantes(&como_bytes(&muestras)), muestras);
    }

    #[test]
    fn un_trozo_vacio_no_revienta() {
        assert!(adaptar(&[], formato(1, 44_100), formato(2, 48_000)).is_empty());
        assert!(como_flotantes(&[]).is_empty());
    }

    /// Windows puede entregar un trozo con bytes de sobra que no llegan a una muestra
    /// entera. Se descartan en vez de leer basura del final.
    #[test]
    fn los_bytes_sueltos_del_final_se_descartan() {
        assert_eq!(como_flotantes(&[0, 0, 0, 0, 1, 2]).len(), 1);
    }
}
