//! Lo que se apunta mientras se graba, aparte de la imagen: donde se hace clic, que
//! atajos se pulsan y por donde va el raton.
//!
//! Vivia dentro del bucle que escribe los fotogramas, en `recorder.rs`, mezclado con el
//! cache, el codificador de vista previa y el sonido: doscientas lineas en las que no se
//! distinguia que era grabar y que era anotar. Aqui esta solo lo segundo, con una regla
//! que no cambia: **se anota SIEMPRE, se pinte o no.** Los aros y la pastilla se dibujan
//! al vuelo solo si se pidieron; el zoom, el cursor y las teclas del exportador salen de
//! lo anotado, y se deciden despues de grabar.
//!
//! Se pregunta por el raton y por el teclado en cada fotograma, nunca con un enganche
//! global: ver `raton.rs` y `teclas.rs`, que explican por que un enganche mal hecho le
//! deja el escritorio a tirones a quien lo tenga puesto.

#![cfg(windows)]

use image::RgbaImage;

use super::{pastilla, raton, realce, teclas};
use crate::capture::Rect;
use crate::encode::zoom;

/// Lo que queda anotado al terminar, ya en coordenadas de la region grabada.
#[derive(Debug, Default)]
pub struct Anotaciones {
    pub clics: Vec<zoom::Clic>,
    pub teclas: Vec<teclas::Atajo>,
    pub cursor: Vec<(u64, i32, i32)>,
}

pub struct Anotador {
    region: Rect,
    /// Si los aros de los clics se pintan en los fotogramas mientras se graba.
    marcar_clics: bool,
    /// Y la pastilla con el atajo.
    marcar_teclas: bool,
    raton: raton::Vigilante,
    teclado: teclas::Vigilante,
    /// Los aros que todavia se ven. Se olvidan enseguida; los de `anotaciones` no.
    aros: Vec<realce::Clic>,
    /// El atajo que todavia se ve, si se estan pintando.
    atajo: Option<teclas::Atajo>,
    pastillas: pastilla::Cache,
    anotaciones: Anotaciones,
}

impl Anotador {
    pub fn new(region: Rect, marcar_clics: bool, marcar_teclas: bool) -> Self {
        Self {
            region,
            marcar_clics,
            marcar_teclas,
            raton: raton::Vigilante::default(),
            teclado: teclas::Vigilante::default(),
            aros: Vec::new(),
            atajo: None,
            pastillas: pastilla::Cache::default(),
            anotaciones: Anotaciones::default(),
        }
    }

    /// Mira el raton y el teclado en este instante y apunta lo que haya pasado.
    ///
    /// Cuesta dos llamadas al sistema por fotograma. El raton viene en coordenadas del
    /// escritorio y se guarda en las de la region, como todo lo demas, para que el
    /// exportador no tenga que saber de monitores.
    pub fn mirar(&mut self, ts_ms: u64) {
        let (rx, ry) = (self.region.x, self.region.y);
        if let Some((cx, cy)) = raton::cursor() {
            self.anotaciones.cursor.push((ts_ms, cx - rx, cy - ry));
        }
        if let Some(clic) = self.raton.mirar(ts_ms) {
            self.anotaciones.clics.push(zoom::Clic {
                ms: clic.ms,
                x: clic.x - rx,
                y: clic.y - ry,
                derecho: clic.derecho,
            });
            if self.marcar_clics {
                self.aros.push(clic);
            }
        }
        if self.marcar_clics {
            realce::olvidar_viejos(&mut self.aros, ts_ms);
        }
        if let Some(nuevo) = self.teclado.mirar(ts_ms) {
            self.anotaciones.teclas.push(teclas::Atajo {
                x: nuevo.x - rx,
                y: nuevo.y - ry,
                ..nuevo.clone()
            });
            if self.marcar_teclas {
                self.atajo = Some(nuevo);
            }
        }
    }

    /// Pinta encima del fotograma lo que toque verse ahora: los aros y la pastilla.
    ///
    /// Si no hay nada que pintar no toca el fotograma, y si lo hay lo pinta EN EL SITIO:
    /// se monta la imagen una sola vez para las dos cosas, sin copiar los pixeles, porque
    /// a treinta fotogramas por segundo cada copia de dos millones de pixeles se nota.
    pub fn pintar(&mut self, rgba: &mut Vec<u8>, width: u32, height: u32, ts_ms: u64) {
        let hay_teclas = self
            .atajo
            .as_ref()
            .is_some_and(|a| opacidad_de(a, ts_ms) > 0.0);
        if self.aros.is_empty() && !hay_teclas {
            return;
        }
        if rgba.len() != (width as usize) * (height as usize) * 4 {
            return;
        }
        let Some(mut imagen) = RgbaImage::from_raw(width, height, std::mem::take(rgba)) else {
            return;
        };
        if !self.aros.is_empty() {
            realce::pintar(&mut imagen, &self.aros, self.region.x, self.region.y, ts_ms);
        }
        if let Some(a) = self.atajo.as_ref().filter(|_| hay_teclas) {
            let opaca = opacidad_de(a, ts_ms);
            if let Some(dibujo) = self.pastillas.pastilla(&a.texto) {
                pastilla::pegar(&mut imagen, dibujo, opaca);
            }
        }
        *rgba = imagen.into_raw();
    }

    pub fn terminar(self) -> Anotaciones {
        self.anotaciones
    }
}

/// Cuanto se ve todavia la pastilla de ese atajo en este instante.
fn opacidad_de(atajo: &teclas::Atajo, ts_ms: u64) -> f32 {
    pastilla::opacidad(ts_ms.saturating_sub(atajo.ms), teclas::DURACION_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region() -> Rect {
        Rect {
            x: 100,
            y: 50,
            width: 64,
            height: 48,
        }
    }

    /// Sin nada pulsado no se pinta nada, y el fotograma sale intacto y del mismo
    /// tamanno: si `pintar` se llevara los bytes por el camino, el cache guardaria un
    /// fotograma vacio y no se veria hasta reproducir.
    #[test]
    fn sin_nada_que_pintar_el_fotograma_no_se_toca() {
        let mut anotador = Anotador::new(region(), true, true);
        let mut rgba = vec![7u8; 64 * 48 * 4];
        let copia = rgba.clone();
        anotador.mirar(0);
        anotador.pintar(&mut rgba, 64, 48, 0);
        assert_eq!(rgba, copia);
    }

    /// Un fotograma con el tamanno equivocado no puede reventar la grabacion ni perderse.
    #[test]
    fn un_fotograma_con_el_tamanno_equivocado_se_deja_estar() {
        let mut anotador = Anotador::new(region(), true, false);
        anotador.aros.push(realce::Clic {
            x: 110,
            y: 60,
            ms: 0,
            derecho: false,
        });
        let mut rgba = vec![1u8; 10];
        anotador.pintar(&mut rgba, 64, 48, 5);
        assert_eq!(rgba.len(), 10, "se ha perdido el fotograma");
    }

    /// Con un aro que se ve, el fotograma cambia y sigue midiendo lo mismo.
    #[test]
    fn con_un_aro_el_fotograma_cambia_sin_cambiar_de_tamanno() {
        let mut anotador = Anotador::new(region(), true, false);
        anotador.aros.push(realce::Clic {
            x: 132,
            y: 74,
            ms: 0,
            derecho: false,
        });
        let mut rgba = vec![0u8; 64 * 48 * 4];
        anotador.pintar(&mut rgba, 64, 48, 5);
        assert_eq!(rgba.len(), 64 * 48 * 4);
        assert!(rgba.iter().any(|&b| b != 0), "el aro no ha dejado ni un pixel");
    }

    /// El raton se apunta en coordenadas de la region, no del escritorio.
    #[test]
    fn el_rastro_del_raton_va_en_coordenadas_de_la_region() {
        let mut anotador = Anotador::new(region(), false, false);
        anotador.mirar(33);
        let anotaciones = anotador.terminar();
        let Some(&(ms, x, y)) = anotaciones.cursor.first() else {
            panic!("Windows siempre sabe dónde está el ratón");
        };
        let (cx, cy) = raton::cursor().expect("el ratón está en algún sitio");
        assert_eq!(ms, 33);
        // Entre las dos lecturas el raton puede haberse movido un pelin; lo que no puede
        // es venir sin restar la esquina de la region.
        assert!((x - (cx - 100)).abs() < 50, "x sin trasladar: {x} contra {cx}");
        assert!((y - (cy - 50)).abs() < 50, "y sin trasladar: {y} contra {cy}");
    }
}
