//! Guardar de un fotograma solo lo que ha cambiado respecto al anterior.
//!
//! Grabando una pantalla, entre dos fotogramas seguidos casi nunca cambia mas que el
//! raton y un par de lineas de texto. Guardar la imagen entera cada vez costaba 19 MB por
//! segundo a 1920x1200, o sea 1,1 GB el minuto, y eso es lo que llenaba el disco.
//!
//! Aqui solo esta el calculo, que es codigo puro y se puede probar sin grabar nada: que
//! zona ha cambiado, como se recorta y como se vuelve a pegar encima. Quien lo usa es
//! `FrameCache`, que ademas decide cada cuanto guarda un fotograma entero.

/// La zona rectangular que ha cambiado, en pixeles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parche {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Parche {
    pub fn pixeles(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

const CANALES: usize = 4;

/// La zona que ha cambiado entre dos fotogramas del mismo tamanno, o `None` si son
/// identicos.
///
/// Se busca por filas y luego por columnas en vez de recorrer pixel a pixel: comparar dos
/// filas enteras es una sola comparacion de memoria, que el procesador hace de una tacada.
/// Recorrer 2,3 millones de pixeles uno a uno treinta veces por segundo costaria mas que
/// lo que se ahorra.
pub fn zona_cambiada(anterior: &[u8], actual: &[u8], width: u32, height: u32) -> Option<Parche> {
    let ancho = width as usize;
    let alto = height as usize;
    let fila = ancho * CANALES;
    if anterior.len() != actual.len() || anterior.len() < fila * alto {
        // Tamannos que no cuadran: se trata como un fotograma entero, que siempre es valido.
        return Some(Parche {
            x: 0,
            y: 0,
            width,
            height,
        });
    }

    let primera = (0..alto).find(|&y| anterior[y * fila..(y + 1) * fila] != actual[y * fila..(y + 1) * fila])?;
    let ultima = (primera..alto)
        .rev()
        .find(|&y| anterior[y * fila..(y + 1) * fila] != actual[y * fila..(y + 1) * fila])
        .unwrap_or(primera);

    // Y ahora las columnas, mirando solo las filas que ya se sabe que cambiaron.
    let mut izquierda = ancho;
    let mut derecha = 0usize;
    for y in primera..=ultima {
        let base = y * fila;
        for x in 0..ancho {
            let p = base + x * CANALES;
            if anterior[p..p + CANALES] != actual[p..p + CANALES] {
                if x < izquierda {
                    izquierda = x;
                }
                break;
            }
        }
        for x in (0..ancho).rev() {
            let p = base + x * CANALES;
            if anterior[p..p + CANALES] != actual[p..p + CANALES] {
                if x > derecha {
                    derecha = x;
                }
                break;
            }
        }
    }

    Some(Parche {
        x: izquierda as u32,
        y: primera as u32,
        width: (derecha + 1 - izquierda) as u32,
        height: (ultima + 1 - primera) as u32,
    })
}

/// Saca del fotograma los pixeles de esa zona, fila a fila.
pub fn recortar(frame: &[u8], width: u32, parche: Parche) -> Vec<u8> {
    let fila_completa = width as usize * CANALES;
    let fila_parche = parche.width as usize * CANALES;
    let mut salida = Vec::with_capacity(fila_parche * parche.height as usize);
    for y in 0..parche.height as usize {
        let inicio = (parche.y as usize + y) * fila_completa + parche.x as usize * CANALES;
        salida.extend_from_slice(&frame[inicio..inicio + fila_parche]);
    }
    salida
}

/// Pega esos pixeles encima del fotograma anterior, que asi se convierte en el siguiente.
pub fn aplicar(destino: &mut [u8], width: u32, parche: Parche, pixeles: &[u8]) {
    let fila_completa = width as usize * CANALES;
    let fila_parche = parche.width as usize * CANALES;
    for y in 0..parche.height as usize {
        let inicio = (parche.y as usize + y) * fila_completa + parche.x as usize * CANALES;
        let origen = y * fila_parche;
        if inicio + fila_parche > destino.len() || origen + fila_parche > pixeles.len() {
            return;
        }
        destino[inicio..inicio + fila_parche].copy_from_slice(&pixeles[origen..origen + fila_parche]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un lienzo de un color, para montar fotogramas de mentira.
    fn liso(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        color
            .iter()
            .cycle()
            .take((width * height) as usize * CANALES)
            .copied()
            .collect()
    }

    fn pintar(frame: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
        let p = (y as usize * width as usize + x as usize) * CANALES;
        frame[p..p + CANALES].copy_from_slice(&color);
    }

    #[test]
    fn dos_fotogramas_iguales_no_tienen_zona_que_guardar() {
        let a = liso(16, 8, [10, 20, 30, 255]);
        assert_eq!(zona_cambiada(&a, &a.clone(), 16, 8), None);
    }

    /// El caso que justifica todo esto: se mueve el raton y cambia un pixel de dos
    /// millones. Si el rectangulo saliera mas grande de la cuenta, el ahorro se esfuma.
    #[test]
    fn un_pixel_distinto_da_un_rectangulo_de_un_pixel() {
        let a = liso(16, 8, [10, 20, 30, 255]);
        let mut b = a.clone();
        pintar(&mut b, 16, 5, 3, [255, 0, 0, 255]);
        assert_eq!(
            zona_cambiada(&a, &b, 16, 8),
            Some(Parche {
                x: 5,
                y: 3,
                width: 1,
                height: 1
            })
        );
    }

    /// Dos cambios lejanos: el rectangulo tiene que abarcarlos a los dos, porque se guarda
    /// una sola zona y no una lista de zonas.
    #[test]
    fn dos_cambios_lejanos_caben_en_el_mismo_rectangulo() {
        let a = liso(16, 8, [10, 20, 30, 255]);
        let mut b = a.clone();
        pintar(&mut b, 16, 2, 1, [1, 2, 3, 255]);
        pintar(&mut b, 16, 12, 6, [1, 2, 3, 255]);
        assert_eq!(
            zona_cambiada(&a, &b, 16, 8),
            Some(Parche {
                x: 2,
                y: 1,
                width: 11,
                height: 6
            })
        );
    }

    /// Lo unico que de verdad importa: recortar y volver a pegar tiene que devolver el
    /// fotograma exacto, pixel a pixel. Si esto falla, el editor ensenna basura.
    #[test]
    fn recortar_y_pegar_reconstruye_el_fotograma_entero() {
        let a = liso(32, 20, [7, 7, 7, 255]);
        let mut b = a.clone();
        for x in 4..19 {
            for y in 2..9 {
                pintar(&mut b, 32, x, y, [(x * 8) as u8, (y * 12) as u8, 200, 255]);
            }
        }
        let parche = zona_cambiada(&a, &b, 32, 20).expect("algo ha cambiado");
        let pixeles = recortar(&b, 32, parche);
        let mut reconstruido = a.clone();
        aplicar(&mut reconstruido, 32, parche, &pixeles);
        assert_eq!(reconstruido, b);
    }

    /// Y con el cambio pegado a las cuatro esquinas, que es donde se sale un indice mal
    /// puesto.
    #[test]
    fn un_cambio_en_las_esquinas_se_reconstruye_igual() {
        let a = liso(9, 5, [0, 0, 0, 255]);
        let mut b = a.clone();
        pintar(&mut b, 9, 0, 0, [255, 255, 255, 255]);
        pintar(&mut b, 9, 8, 4, [255, 255, 255, 255]);
        let parche = zona_cambiada(&a, &b, 9, 5).expect("algo ha cambiado");
        assert_eq!(
            parche,
            Parche {
                x: 0,
                y: 0,
                width: 9,
                height: 5
            }
        );
        let mut reconstruido = a.clone();
        aplicar(&mut reconstruido, 9, parche, &recortar(&b, 9, parche));
        assert_eq!(reconstruido, b);
    }

    /// Una fila entera cambiada, que es lo que pasa al desplazar una lista.
    #[test]
    fn una_fila_entera_se_guarda_como_una_fila() {
        let a = liso(12, 6, [3, 3, 3, 255]);
        let mut b = a.clone();
        for x in 0..12 {
            pintar(&mut b, 12, x, 4, [9, 9, 9, 255]);
        }
        assert_eq!(
            zona_cambiada(&a, &b, 12, 6),
            Some(Parche {
                x: 0,
                y: 4,
                width: 12,
                height: 1
            })
        );
    }
}
