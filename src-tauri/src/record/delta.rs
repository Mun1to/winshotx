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

/// Como mucho cuantas zonas se guardan por fotograma. Cuatro cubren lo normal (un cursor,
/// una linea que se escribe, un reloj y un icono que gira); mas serian mas cabeceras de
/// QOI que ahorro.
pub const ZONAS_MAX: usize = 4;

/// Dos tramos de filas cambiadas separados por menos de esto se juntan: cada zona es una
/// cabecera de QOI y un recorte aparte, y por ocho filas no compensa.
const HUECO_MIN: usize = 8;

/// Las zonas que han cambiado entre dos fotogramas del mismo tamanno: varias, disjuntas, y
/// como mucho `ZONAS_MAX`. Vacio si son identicos.
///
/// `zona_cambiada` devuelve UNA caja que lo abarca todo, y con dos cambios pequennos y
/// lejanos (el cursor abajo y un icono que gira arriba) esa caja es media pantalla: media
/// pantalla de QOI por fotograma, sesenta veces por segundo, para dos cosas de cien
/// pixeles. Medido el 17 de septiembre de 2026 en el anillo: un marco parpadeando en una
/// esquina y el terminal en la otra daban 346.000 pixeles por fotograma.
///
/// Se hace por filas: las filas que cambiaron se agrupan en tramos (con `HUECO_MIN` de
/// tolerancia), cada tramo se recorta a las columnas que de verdad cambiaron, y si salen
/// mas de `ZONAS_MAX` se juntan los dos mas cercanos hasta que quepan. Cuesta lo mismo que
/// `zona_cambiada`: una pasada por las filas y una por los bordes de las que cambiaron.
pub fn zonas_cambiadas(anterior: &[u8], actual: &[u8], width: u32, height: u32) -> Vec<Parche> {
    let ancho = width as usize;
    let alto = height as usize;
    let fila = ancho * CANALES;
    if anterior.len() != actual.len() || anterior.len() < fila * alto || ancho == 0 {
        return vec![Parche {
            x: 0,
            y: 0,
            width,
            height,
        }];
    }

    // Tramos de filas cambiadas, ya con el hueco tolerado.
    let mut tramos: Vec<(usize, usize)> = Vec::new();
    for y in 0..alto {
        if anterior[y * fila..(y + 1) * fila] == actual[y * fila..(y + 1) * fila] {
            continue;
        }
        match tramos.last_mut() {
            Some((_, fin)) if y < *fin + HUECO_MIN => *fin = y + 1,
            _ => tramos.push((y, y + 1)),
        }
    }
    if tramos.is_empty() {
        return Vec::new();
    }
    while tramos.len() > ZONAS_MAX {
        // Los dos tramos con menos filas iguales entre medias se funden en uno.
        let (i, _) = tramos
            .windows(2)
            .enumerate()
            .map(|(i, par)| (i, par[1].0 - par[0].1))
            .min_by_key(|(_, hueco)| *hueco)
            .expect("hay al menos dos tramos");
        tramos[i].1 = tramos[i + 1].1;
        tramos.remove(i + 1);
    }

    // Cada tramo, recortado a las columnas que cambiaron en alguna de sus filas.
    tramos
        .into_iter()
        .map(|(desde, hasta)| {
            let mut izquierda = ancho;
            let mut derecha = 0usize;
            for y in desde..hasta {
                let base = y * fila;
                let a = &anterior[base..base + fila];
                let b = &actual[base..base + fila];
                if a == b {
                    continue;
                }
                if let Some(x) = (0..ancho).find(|&x| a[x * CANALES..(x + 1) * CANALES] != b[x * CANALES..(x + 1) * CANALES]) {
                    izquierda = izquierda.min(x);
                }
                if let Some(x) = (0..ancho).rev().find(|&x| a[x * CANALES..(x + 1) * CANALES] != b[x * CANALES..(x + 1) * CANALES]) {
                    derecha = derecha.max(x);
                }
            }
            Parche {
                x: izquierda as u32,
                y: desde as u32,
                width: (derecha + 1 - izquierda) as u32,
                height: (hasta - desde) as u32,
            }
        })
        .collect()
}

/// La caja que abarca todas esas zonas. `None` si no hay ninguna.
pub fn envolvente(zonas: &[Parche]) -> Option<Parche> {
    let primera = zonas.first()?;
    let (mut x1, mut y1) = (primera.x, primera.y);
    let (mut x2, mut y2) = (primera.x + primera.width, primera.y + primera.height);
    for z in &zonas[1..] {
        x1 = x1.min(z.x);
        y1 = y1.min(z.y);
        x2 = x2.max(z.x + z.width);
        y2 = y2.max(z.y + z.height);
    }
    Some(Parche {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
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

/// Copia esa zona de `origen` a `destino`, los dos fotogramas del mismo ancho. Es como
/// `recortar` seguido de `aplicar` pero sin pasar por un `Vec` en medio.
pub fn copiar_zona(destino: &mut [u8], origen: &[u8], width: u32, parche: Parche) {
    let fila_completa = width as usize * CANALES;
    let fila_parche = parche.width as usize * CANALES;
    for y in 0..parche.height as usize {
        let inicio = (parche.y as usize + y) * fila_completa + parche.x as usize * CANALES;
        if inicio + fila_parche > destino.len() || inicio + fila_parche > origen.len() {
            return;
        }
        destino[inicio..inicio + fila_parche].copy_from_slice(&origen[inicio..inicio + fila_parche]);
    }
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

/// Como `zonas_cambiadas`, pero mirando SOLO dentro de `pistas`: quien llama sabe que fuera
/// de ellas no ha cambiado nada (se lo ha dicho Windows, ver `win::CapturedFrame::zonas`).
///
/// **Por que existe (S29).** Comparar dos fotogramas de 1920x1080 enteros son 16 MB leidos
/// por fotograma, y el anillo lo hacia sesenta veces por segundo para encontrar el cursor.
/// Con las pistas se lee lo que mide el cursor. Cada pista se ajusta a lo que de verdad
/// cambio (Windows da rectangulos generosos: una ventana repintada con lo mismo, por
/// ejemplo) y las que no cambiaron nada se tiran; el resultado se junta hasta `ZONAS_MAX`.
///
/// Lo que se promete es lo mismo que `zonas_cambiadas`: todo pixel distinto queda dentro de
/// alguna zona, y si no hay ninguno distinto, la lista sale vacia.
pub fn zonas_cambiadas_en(anterior: &[u8], actual: &[u8], width: u32, height: u32, pistas: &[Parche]) -> Vec<Parche> {
    let ancho = width as usize;
    let alto = height as usize;
    let fila = ancho * CANALES;
    if anterior.len() != actual.len() || anterior.len() < fila * alto || ancho == 0 {
        return vec![Parche { x: 0, y: 0, width, height }];
    }
    let mut zonas = Vec::with_capacity(pistas.len());
    for pista in pistas {
        let x1 = (pista.x as usize).min(ancho);
        let x2 = (pista.x as usize + pista.width as usize).min(ancho);
        let y1 = (pista.y as usize).min(alto);
        let y2 = (pista.y as usize + pista.height as usize).min(alto);
        if x1 >= x2 || y1 >= y2 {
            continue;
        }
        let tramo = |y: usize| (y * fila + x1 * CANALES)..(y * fila + x2 * CANALES);
        let distinta = |y: usize| anterior[tramo(y)] != actual[tramo(y)];
        let Some(primera) = (y1..y2).find(|&y| distinta(y)) else {
            continue;
        };
        let ultima = (primera..y2).rev().find(|&y| distinta(y)).unwrap_or(primera);
        let (mut izquierda, mut derecha) = (x2, x1);
        for y in primera..=ultima {
            let a = &anterior[tramo(y)];
            let b = &actual[tramo(y)];
            if a == b {
                continue;
            }
            // Comparando pixeles enteros como u32: es una comparacion por pixel y no cuatro.
            let pixel = |s: &[u8], i: usize| u32::from_ne_bytes([s[i * 4], s[i * 4 + 1], s[i * 4 + 2], s[i * 4 + 3]]);
            let n = x2 - x1;
            if let Some(i) = (0..n).find(|&i| pixel(a, i) != pixel(b, i)) {
                izquierda = izquierda.min(x1 + i);
            }
            if let Some(i) = (0..n).rev().find(|&i| pixel(a, i) != pixel(b, i)) {
                derecha = derecha.max(x1 + i);
            }
        }
        zonas.push(Parche {
            x: izquierda as u32,
            y: primera as u32,
            width: (derecha + 1 - izquierda) as u32,
            height: (ultima + 1 - primera) as u32,
        });
    }
    juntar_zonas(&mut zonas, ZONAS_MAX);
    zonas
}

/// Donde empiezan los pixeles de cada zona cuando van varias una detras de otra, fila a
/// fila (es lo que manda la captura con `win::CaptureFlags::solo_cambios`).
pub fn trozos_de_zonas(zonas: &[Parche]) -> impl Iterator<Item = (Parche, std::ops::Range<usize>)> + '_ {
    let mut desde = 0usize;
    zonas.iter().map(move |z| {
        let largo = z.pixeles() as usize * CANALES;
        let tramo = desde..desde + largo;
        desde += largo;
        (*z, tramo)
    })
}

/// Junta rectangulos hasta que no se solapen ni se toquen y no pasen de `max`, uniendo
/// cada vez el par que menos area de mas anade. El resultado cubre todo lo que cubrian.
pub fn juntar_zonas(zonas: &mut Vec<Parche>, max: usize) {
    fn union(a: &Parche, b: &Parche) -> Parche {
        let (x1, y1) = (a.x.min(b.x), a.y.min(b.y));
        let (x2, y2) = ((a.x + a.width).max(b.x + b.width), (a.y + a.height).max(b.y + b.height));
        Parche { x: x1, y: y1, width: x2 - x1, height: y2 - y1 }
    }
    fn se_tocan(a: &Parche, b: &Parche) -> bool {
        a.x <= b.x + b.width && b.x <= a.x + a.width && a.y <= b.y + b.height && b.y <= a.y + a.height
    }
    zonas.retain(|z| z.width > 0 && z.height > 0);
    loop {
        // Primero las que se solapan o se tocan: copiar dos veces el mismo pixel no sirve.
        let mut juntadas = true;
        while juntadas {
            juntadas = false;
            'fuera: for i in 0..zonas.len() {
                for j in i + 1..zonas.len() {
                    if se_tocan(&zonas[i], &zonas[j]) {
                        zonas[i] = union(&zonas[i], &zonas[j]);
                        zonas.swap_remove(j);
                        juntadas = true;
                        break 'fuera;
                    }
                }
            }
        }
        if zonas.len() <= max.max(1) {
            break;
        }
        let mut mejor = (0, 1, u64::MAX);
        for i in 0..zonas.len() {
            for j in i + 1..zonas.len() {
                let de_mas = union(&zonas[i], &zonas[j])
                    .pixeles()
                    .saturating_sub(zonas[i].pixeles() + zonas[j].pixeles());
                if de_mas < mejor.2 {
                    mejor = (i, j, de_mas);
                }
            }
        }
        let (i, j, _) = mejor;
        zonas[i] = union(&zonas[i], &zonas[j]);
        zonas.swap_remove(j);
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

    /// Dos cambios lejanos: `zonas_cambiadas` los da como DOS zonas de un pixel, no como
    /// la caja que los abarca. Es la razon de que exista.
    #[test]
    fn dos_cambios_lejanos_son_dos_zonas() {
        let a = liso(64, 64, [10, 20, 30, 255]);
        let mut b = a.clone();
        pintar(&mut b, 64, 2, 3, [255, 0, 0, 255]);
        pintar(&mut b, 64, 60, 58, [0, 255, 0, 255]);
        let zonas = zonas_cambiadas(&a, &b, 64, 64);
        assert_eq!(
            zonas,
            vec![
                Parche { x: 2, y: 3, width: 1, height: 1 },
                Parche { x: 60, y: 58, width: 1, height: 1 },
            ]
        );
        // Y la envolvente es lo que daba la caja unica.
        assert_eq!(envolvente(&zonas), zona_cambiada(&a, &b, 64, 64));
    }

    /// Dos cambios casi pegados en vertical se juntan en una zona: una cabecera de QOI por
    /// ocho filas de nada no compensa.
    #[test]
    fn cambios_casi_pegados_se_juntan() {
        let a = liso(32, 32, [0, 0, 0, 255]);
        let mut b = a.clone();
        pintar(&mut b, 32, 5, 10, [255, 255, 255, 255]);
        pintar(&mut b, 32, 20, 14, [255, 255, 255, 255]);
        let zonas = zonas_cambiadas(&a, &b, 32, 32);
        assert_eq!(zonas, vec![Parche { x: 5, y: 10, width: 16, height: 5 }]);
    }

    /// Mas de cuatro cambios repartidos: se juntan los mas cercanos hasta quedar en cuatro,
    /// y entre todas siguen cubriendo cada pixel que cambio.
    #[test]
    fn mas_de_cuatro_zonas_se_juntan_por_cercania() {
        let a = liso(16, 200, [0, 0, 0, 255]);
        let mut b = a.clone();
        // Seis cambios: dos casi juntos (filas 10 y 30), el resto lejos.
        for y in [10u32, 30, 70, 110, 150, 190] {
            pintar(&mut b, 16, 8, y, [255, 0, 0, 255]);
        }
        let zonas = zonas_cambiadas(&a, &b, 16, 200);
        assert_eq!(zonas.len(), ZONAS_MAX);
        for y in [10u32, 30, 70, 110, 150, 190] {
            assert!(
                zonas.iter().any(|z| y >= z.y && y < z.y + z.height && z.x <= 8 && 8 < z.x + z.width),
                "la fila {y} cambio y ninguna zona la cubre: {zonas:?}"
            );
        }
        // Las zonas no se pisan entre si.
        for (i, z) in zonas.iter().enumerate() {
            for otra in &zonas[i + 1..] {
                assert!(z.y + z.height <= otra.y, "dos zonas se solapan: {z:?} y {otra:?}");
            }
        }
    }

    /// Sin cambios, ninguna zona; y con tamannos que no cuadran, el fotograma entero.
    #[test]
    fn sin_cambios_no_hay_zonas() {
        let a = liso(16, 8, [10, 20, 30, 255]);
        assert!(zonas_cambiadas(&a, &a.clone(), 16, 8).is_empty());
        let corto = vec![0u8; 16];
        assert_eq!(zonas_cambiadas(&a, &corto, 16, 8), vec![Parche { x: 0, y: 0, width: 16, height: 8 }]);
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

    /// Un generador de numeros de mentira, el mismo en cada vuelta.
    fn azar(s: &mut u32) -> u32 {
        *s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *s >> 8
    }

    /// S29, lo que no se puede negociar de las pistas: TODO pixel distinto queda dentro de
    /// alguna zona, con pistas holgadas (como las da Windows) y con pistas que no cambiaron
    /// nada; y nunca mas de `ZONAS_MAX`.
    #[test]
    fn con_pistas_no_se_escapa_ningun_pixel() {
        let (w, h) = (97u32, 61u32);
        let mut s = 7u32;
        for vuelta in 0..300 {
            let a: Vec<u8> = (0..w * h * 4).map(|_| azar(&mut s) as u8).collect();
            let mut b = a.clone();
            let mut pistas = Vec::new();
            for _ in 0..(1 + azar(&mut s) % 7) {
                let (x, y) = (azar(&mut s) % w, azar(&mut s) % h);
                let (pw, ph) = (1 + azar(&mut s) % 20, 1 + azar(&mut s) % 20);
                let (pw, ph) = (pw.min(w - x), ph.min(h - y));
                // Algunos pixeles de dentro cambian (a veces ninguno: una ventana repintada
                // con lo mismo).
                if vuelta % 5 != 0 {
                    for _ in 0..(1 + azar(&mut s) % 4) {
                        let (cx, cy) = (x + azar(&mut s) % pw, y + azar(&mut s) % ph);
                        pintar(&mut b, w, cx, cy, [azar(&mut s) as u8, 1, 2, 255]);
                    }
                }
                // La pista es mas grande de la cuenta, como las de Windows.
                let (px, py) = (x.saturating_sub(3), y.saturating_sub(2));
                pistas.push(Parche { x: px, y: py, width: (pw + 6).min(w - px), height: (ph + 4).min(h - py) });
            }
            let zonas = zonas_cambiadas_en(&a, &b, w, h, &pistas);
            assert!(zonas.len() <= ZONAS_MAX, "{} zonas", zonas.len());
            let mut reconstruido = a.clone();
            for z in &zonas {
                aplicar(&mut reconstruido, w, *z, &recortar(&b, w, *z));
            }
            assert_eq!(reconstruido, b, "vuelta {vuelta}: un pixel cambiado se ha quedado fuera: {zonas:?}");
            if a == b {
                assert!(zonas.is_empty(), "sin cambios no puede haber zonas: {zonas:?}");
            }
        }
    }

    /// Y juntar zonas cubre todo lo que cubrian, sin solapes y sin pasar del maximo.
    #[test]
    fn juntar_zonas_cubre_todo_sin_solaparse() {
        let mut s = 3u32;
        for _ in 0..200 {
            let originales: Vec<Parche> = (0..(1 + azar(&mut s) % 30))
                .map(|_| Parche { x: azar(&mut s) % 500, y: azar(&mut s) % 300, width: 1 + azar(&mut s) % 60, height: 1 + azar(&mut s) % 40 })
                .collect();
            let mut zonas = originales.clone();
            let max = 1 + (azar(&mut s) % 8) as usize;
            juntar_zonas(&mut zonas, max);
            assert!(zonas.len() <= max);
            for o in &originales {
                assert!(
                    zonas.iter().any(|z| z.x <= o.x && z.y <= o.y && o.x + o.width <= z.x + z.width && o.y + o.height <= z.y + z.height),
                    "{o:?} no queda dentro de ninguna de {zonas:?}"
                );
            }
            for (i, a) in zonas.iter().enumerate() {
                for b in &zonas[i + 1..] {
                    let solapan = a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
                    assert!(!solapan, "{a:?} y {b:?} se solapan");
                }
            }
        }
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
