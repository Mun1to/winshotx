//! El reloj de la grabacion, el mismo para todo lo que se apunta mientras se graba.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// El reloj de la grabacion: lo que ha pasado desde el cero de los fotogramas, sin las
/// pausas. Es el mismo con el que se marcan los fotogramas (`win.rs`), a proposito.
#[derive(Clone)]
pub struct Reloj {
    pub start: Instant,
    pub paused_ms: Arc<AtomicU64>,
}

impl Reloj {
    pub fn ms(&self) -> u64 {
        (self.start.elapsed().as_millis() as u64)
            .saturating_sub(self.paused_ms.load(Ordering::Relaxed))
    }
}
