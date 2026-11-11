//! Lo que se apunta mientras se graba, aparte de la imagen: donde se hace clic, que
//! atajos se pulsan, por donde va el raton, donde se pulso la tecla de zoom y cuando se
//! estuvo escribiendo.
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

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use image::RgbaImage;
use parking_lot::Mutex;

use super::{pastilla, punteros, raton, realce, teclas};
use crate::capture::Rect;
use crate::contrato::{MarcaZoom, TramoEscritura};
use crate::encode::zoom;

/// Lo que queda anotado al terminar, ya en coordenadas de la region grabada.
#[derive(Debug, Default)]
pub struct Anotaciones {
    pub clics: Vec<zoom::Clic>,
    pub teclas: Vec<teclas::Atajo>,
    pub cursor: Vec<(u64, i32, i32)>,
    /// La forma del puntero cada vez que cambia: flecha, barra de texto o manita.
    pub formas: Vec<(u64, u8)>,
    /// Las imagenes de verdad de los punteros que hubo, en el orden en que aparecieron.
    pub punteros: Vec<punteros::Puntero>,
    /// Cual de ellas estaba puesta y desde cuando. `u32::MAX` es una que no se pudo leer.
    pub cambios_puntero: Vec<(u64, u32)>,
    /// Donde estaba el raton cada vez que se pulso la tecla de zoom (A3).
    pub marcas_zoom: Vec<MarcaZoom>,
    /// Los ratos en los que se escribio, sin ninguna tecla dentro.
    pub escritura: Vec<TramoEscritura>,
}

pub struct Anotador {
    /// La zona que se graba. Con una ventana se va actualizando sola: ver `origen`.
    region: Rect,
    /// La ventana que se graba, si se graba una ventana y no un trozo de pantalla.
    ventana: Option<isize>,
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
    /// La ultima forma vista, para apuntar solo los cambios. Empieza en un valor que no
    /// es ninguna forma, asi el primer fotograma siempre deja la suya escrita.
    ultima_forma: u8,
    /// El identificador del ultimo cursor visto, y que imagen le corresponde a cada uno
    /// de los que ya se han leido. Leer la imagen cuesta; se hace una vez por cursor.
    ultimo_asa: isize,
    ids: HashMap<isize, u32>,
    /// La tecla que deja una marca de zoom, si la hay, y si estaba abajo la vez anterior:
    /// mantenerla pulsada es UNA marca, no sesenta por segundo.
    tecla_zoom: Option<teclas::TeclaZoom>,
    zoom_abajo: bool,
    escritura: teclas::Escritura,
    lector: teclas::Lector,
    /// Donde se vio el raton la ultima vez, ya en coordenadas de la region. Es a donde
    /// apuntan un atajo y una marca de zoom, que no tienen sitio propio.
    ultimo_raton: (i32, i32),
}

impl Anotador {
    pub fn new(region: Rect, marcar_clics: bool, marcar_teclas: bool) -> Self {
        Self::con_ventana(region, None, marcar_clics, marcar_teclas)
    }

    /// Igual, pero midiendo desde una VENTANA que se puede mover mientras se graba.
    ///
    /// Grabando una ventana, lo que se graba la sigue a donde vaya, asi que el raton y los
    /// clics no se pueden medir desde donde estaba al empezar: a la primera vez que alguien
    /// arrastra la ventana, los aros aparecen en otro sitio. El origen se pregunta en cada
    /// muestra, que cuesta una llamada al escritorio y vale lo que cuesta.
    pub fn con_ventana(
        region: Rect,
        ventana: Option<isize>,
        marcar_clics: bool,
        marcar_teclas: bool,
    ) -> Self {
        Self {
            region,
            ventana,
            marcar_clics,
            marcar_teclas,
            raton: raton::Vigilante::default(),
            teclado: teclas::Vigilante::default(),
            aros: Vec::new(),
            atajo: None,
            pastillas: pastilla::Cache::default(),
            anotaciones: Anotaciones::default(),
            ultima_forma: u8::MAX,
            ultimo_asa: 0,
            ids: HashMap::new(),
            tecla_zoom: None,
            zoom_abajo: false,
            escritura: teclas::Escritura::default(),
            lector: teclas::Lector::default(),
            ultimo_raton: (0, 0),
        }
    }

    /// Con la tecla de zoom de los ajustes. Sin ella (o con una que no se entiende) no se
    /// apunta ninguna marca, y todo lo demas sigue igual.
    pub fn con_tecla_zoom(mut self, tecla: Option<teclas::TeclaZoom>) -> Self {
        self.tecla_zoom = tecla;
        self
    }

    /// De donde se miden las coordenadas en este instante.
    ///
    /// Con una region, de su esquina, que no se mueve. Con una ventana, de la esquina que
    /// tenga AHORA: por eso se pregunta cada vez en vez de guardarla. Si la ventana ya no
    /// esta, se usa la ultima posicion conocida, que es mejor que mandar todo al cero.
    fn origen(&mut self) -> (i32, i32) {
        let Some(hwnd) = self.ventana else {
            return (self.region.x, self.region.y);
        };
        if let Some((x, y)) = esquina_de_la_ventana(hwnd) {
            self.region.x = x;
            self.region.y = y;
        }
        (self.region.x, self.region.y)
    }

    /// Mira el raton y el teclado en este instante y apunta lo que haya pasado.
    ///
    /// Cuesta dos llamadas al sistema por fotograma. El raton viene en coordenadas del
    /// escritorio y se guarda en las de la region, como todo lo demas, para que el
    /// exportador no tenga que saber de monitores.
    pub fn mirar(&mut self, ts_ms: u64) {
        let (rx, ry) = self.origen();
        if let Some((cx, cy)) = raton::cursor() {
            self.anotaciones.cursor.push((ts_ms, cx - rx, cy - ry));
            self.ultimo_raton = (cx - rx, cy - ry);
        }
        if let Some((asa, forma)) = raton::puntero_actual() {
            let forma = forma as u8;
            if forma != self.ultima_forma {
                self.anotaciones.formas.push((ts_ms, forma));
                self.ultima_forma = forma;
            }
            if asa != self.ultimo_asa {
                self.ultimo_asa = asa;
                let id = match self.ids.get(&asa) {
                    Some(id) => *id,
                    None => {
                        let id = match punteros::capturar(asa) {
                            Some(p) => {
                                self.anotaciones.punteros.push(p);
                                (self.anotaciones.punteros.len() - 1) as u32
                            }
                            None => u32::MAX,
                        };
                        self.ids.insert(asa, id);
                        id
                    }
                };
                self.anotaciones.cambios_puntero.push((ts_ms, id));
            }
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
        let foto = self.lector.leer();
        self.mirar_teclado(ts_ms, &foto, (rx, ry));
    }

    /// Lo que toca hacer con esa foto del teclado: la marca de zoom, el atajo y la
    /// escritura. Va aparte de `mirar` para poder probarlo con un teclado de mentira:
    /// desde una prueba no se puede pulsar Ctrl.
    fn mirar_teclado(&mut self, ts_ms: u64, foto: &teclas::Foto, (rx, ry): (i32, i32)) {
        let zoom = self.tecla_zoom.as_ref().is_some_and(|t| t.pulsada(foto));
        if zoom && !self.zoom_abajo {
            let (x, y) = self.ultimo_raton;
            self.anotaciones.marcas_zoom.push(MarcaZoom { ms: ts_ms, x, y });
        }
        self.zoom_abajo = zoom;

        // La tecla de zoom es de winshotx, no del tutorial: no sale en la pastilla.
        if let Some(nuevo) = self.teclado.mirar_foto(foto, ts_ms).filter(|_| !zoom) {
            let (x, y) = self.ultimo_raton;
            self.anotaciones.teclas.push(teclas::Atajo {
                x,
                y,
                ..nuevo.clone()
            });
            if self.marcar_teclas {
                self.atajo = Some(nuevo);
            }
        }

        self.escritura.mirar(ts_ms, foto, self.tecla_zoom.as_ref(), || {
            teclas::caret().map(|(x, y)| (x - rx, y - ry))
        });
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
        let mut anotaciones = self.anotaciones;
        anotaciones.escritura = self.escritura.terminar();
        anotaciones
    }
}

/// Cuanto se ve todavia la pastilla de ese atajo en este instante.
fn opacidad_de(atajo: &teclas::Atajo, ts_ms: u64) -> f32 {
    pastilla::opacidad(ts_ms.saturating_sub(atajo.ms), teclas::DURACION_MS)
}

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

/// Donde esta ahora la esquina de arriba a la izquierda de esa ventana.
///
/// Del marco REAL que compone el escritorio, igual que al arrancar la captura: con
/// `GetWindowRect` saldria desplazado los pocos pixeles del borde invisible de las sombras,
/// y los aros de clic se verian corridos.
#[cfg(windows)]
pub(crate) fn esquina_de_la_ventana(hwnd: isize) -> Option<(i32, i32)> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};

    let mut marco = RECT::default();
    let ok = unsafe {
        DwmGetWindowAttribute(
            HWND(hwnd as *mut core::ffi::c_void),
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut marco).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
    };
    ok.ok().map(|()| (marco.left, marco.top))
}

#[cfg(not(windows))]
pub(crate) fn esquina_de_la_ventana(_hwnd: isize) -> Option<(i32, i32)> {
    None
}

/// Cada cuanto se mira el raton. Sesenta veces por segundo, que es lo que se ve suave en
/// la vista previa y de sobra para exportar a treinta.
pub const CADA: Duration = Duration::from_millis(16);

/// El hilo que mira el raton y el teclado a su ritmo, no al de los fotogramas.
///
/// Antes se miraba una vez por fotograma RECIBIDO, y sin el cursor cocido en la imagen un
/// raton que se mueve sobre una pantalla quieta no genera fotogramas: el rastro se quedaba
/// sin puntos, el puntero dibujado se clavaba y luego pegaba un salto. Es lo que Munir vio
/// el 10 de septiembre de 2026: «el cursor se hara donde no deberia de estar».
pub struct Muestreador {
    anotador: Arc<Mutex<Anotador>>,
    parar: Arc<AtomicBool>,
    hilo: Option<JoinHandle<()>>,
}

impl Muestreador {
    pub fn empezar(anotador: Anotador, reloj: Reloj, pause: Arc<AtomicBool>) -> Self {
        let anotador = Arc::new(Mutex::new(anotador));
        let parar = Arc::new(AtomicBool::new(false));
        let hilo = {
            let anotador = Arc::clone(&anotador);
            let parar = Arc::clone(&parar);
            std::thread::spawn(move || {
                while !parar.load(Ordering::Relaxed) {
                    std::thread::sleep(CADA);
                    // En pausa no se mira: los fotogramas tampoco llegan, y el reloj
                    // descuenta la pausa solo al reanudar, asi que un punto tomado en
                    // medio llevaria un tiempo que luego va hacia atras.
                    if pause.load(Ordering::Relaxed) || parar.load(Ordering::Relaxed) {
                        continue;
                    }
                    let ms = reloj.ms();
                    anotador.lock().mirar(ms);
                }
            })
        };
        Self {
            anotador,
            parar,
            hilo: Some(hilo),
        }
    }

    /// Para pintar los aros y la pastilla en los fotogramas, si se pidieron al grabar.
    pub fn anotador(&self) -> &Arc<Mutex<Anotador>> {
        &self.anotador
    }

    /// Para el hilo y devuelve todo lo anotado.
    pub fn terminar(mut self) -> Anotaciones {
        self.parar.store(true, Ordering::Relaxed);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
        let anotador = match Arc::try_unwrap(self.anotador) {
            Ok(m) => m.into_inner(),
            // No deberia pasar (el hilo ya ha soltado su copia), pero antes que reventar
            // se devuelve lo que haya.
            Err(compartido) => {
                let mut guardado = compartido.lock();
                std::mem::replace(
                    &mut *guardado,
                    Anotador::new(
                        Rect {
                            x: 0,
                            y: 0,
                            width: 2,
                            height: 2,
                        },
                        false,
                        false,
                    ),
                )
            }
        };
        anotador.terminar()
    }
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

    /// El muestreador mira el raton a su ritmo aunque no llegue ni un fotograma, y sus
    /// tiempos van siempre hacia delante.
    #[test]
    fn el_muestreador_apunta_el_raton_sin_fotogramas() {
        let reloj = Reloj {
            start: Instant::now(),
            paused_ms: Arc::new(AtomicU64::new(0)),
        };
        let pause = Arc::new(AtomicBool::new(false));
        let m = Muestreador::empezar(Anotador::new(region(), false, false), reloj, pause);
        std::thread::sleep(Duration::from_millis(120));
        let anotaciones = m.terminar();
        // Mas de una vez, no cuatro. Lo que esta prueba demuestra es que el muestreador
        // tiene reloj propio y no espera a que llegue un fotograma; cuantas vueltas le den
        // en 120 ms lo decide la maquina, y con las otras trescientas pruebas corriendo a
        // la vez se ha visto quedarse en tres. Pedir cuatro convertia eso en un fallo rojo
        // que no reproduce nadie y que no dice nada del producto.
        assert!(
            anotaciones.cursor.len() >= 2,
            "en 120 ms tendria que haber mirado mas de una vez: {}",
            anotaciones.cursor.len()
        );
        let tiempos: Vec<u64> = anotaciones.cursor.iter().map(|p| p.0).collect();
        assert!(tiempos.windows(2).all(|w| w[0] <= w[1]), "el tiempo va hacia atras: {tiempos:?}");
        // El puntero puesto se apunta al empezar... si hay uno a la vista. Windows lo
        // esconde mientras se teclea, y quien corre las pruebas suele estar tecleando.
        if raton::puntero_actual().is_some() {
            assert!(!anotaciones.cambios_puntero.is_empty(), "el puntero puesto se apunta al empezar");
        }
    }

    /// En pausa no se apunta nada: ni un punto con un tiempo que luego se descuenta.
    #[test]
    fn en_pausa_el_muestreador_no_apunta() {
        let reloj = Reloj {
            start: Instant::now(),
            paused_ms: Arc::new(AtomicU64::new(0)),
        };
        let pause = Arc::new(AtomicBool::new(true));
        let m = Muestreador::empezar(Anotador::new(region(), false, false), reloj, pause);
        std::thread::sleep(Duration::from_millis(80));
        let anotaciones = m.terminar();
        assert!(anotaciones.cursor.is_empty(), "{:?}", anotaciones.cursor);
    }

    const VK_CONTROL: u16 = 0x11;
    const VK_SHIFT: u16 = 0x10;
    const NUEVE: u16 = 0x39;

    /// **A3, lo que se pidio:** una grabacion con dos pulsaciones de la tecla de zoom da
    /// dos marcas, cada una donde estaba el raton al pulsarla. Mantenerla no repite, y la
    /// tecla no sale en la pastilla como si fuera un atajo del tutorial.
    #[test]
    fn dos_pulsaciones_de_la_tecla_de_zoom_dan_dos_marcas() {
        let tecla = teclas::TeclaZoom::de_atajo("CmdOrCtrl+Shift+9");
        let mut anotador = Anotador::new(region(), false, true).con_tecla_zoom(tecla);
        let zoom = teclas::Foto::con(&[VK_CONTROL, VK_SHIFT, NUEVE]);
        let nada = teclas::Foto::default();

        anotador.ultimo_raton = (10, 20);
        anotador.mirar_teclado(100, &zoom, (100, 50));
        anotador.mirar_teclado(116, &zoom, (100, 50)); // con el dedo encima
        anotador.mirar_teclado(132, &zoom, (100, 50));
        anotador.mirar_teclado(148, &nada, (100, 50));
        anotador.ultimo_raton = (30, 40);
        anotador.mirar_teclado(500, &zoom, (100, 50));
        anotador.mirar_teclado(516, &nada, (100, 50));

        assert!(anotador.atajo.is_none(), "la tecla de zoom ha salido en la pastilla");
        let anotaciones = anotador.terminar();
        assert_eq!(
            anotaciones.marcas_zoom,
            vec![
                MarcaZoom { ms: 100, x: 10, y: 20 },
                MarcaZoom { ms: 500, x: 30, y: 40 },
            ]
        );
        assert!(anotaciones.teclas.is_empty(), "{:?}", anotaciones.teclas);
        assert!(anotaciones.escritura.is_empty(), "pulsar la tecla de zoom no es escribir");
    }

    /// Sin tecla de zoom en los ajustes, la misma combinacion es un atajo mas y no deja
    /// ninguna marca.
    #[test]
    fn sin_tecla_de_zoom_no_hay_marcas() {
        let mut anotador = Anotador::new(region(), false, false);
        anotador.mirar_teclado(0, &teclas::Foto::con(&[VK_CONTROL, VK_SHIFT, NUEVE]), (0, 0));
        let anotaciones = anotador.terminar();
        assert!(anotaciones.marcas_zoom.is_empty());
        assert_eq!(anotaciones.teclas.len(), 1);
        assert_eq!(anotaciones.teclas[0].texto, "Ctrl + Mayús + 9");
    }

    /// Un atajo apunta a donde estaba el raton, en coordenadas de la region.
    #[test]
    fn el_atajo_va_donde_estaba_el_raton() {
        let mut anotador = Anotador::new(region(), false, false);
        anotador.ultimo_raton = (7, 9);
        anotador.mirar_teclado(0, &teclas::Foto::con(&[VK_CONTROL, 0x43]), (100, 50));
        let atajo = &anotador.terminar().teclas[0];
        assert_eq!((atajo.texto.as_str(), atajo.x, atajo.y), ("Ctrl + C", 7, 9));
    }

    /// Lo que se escribe llega a las anotaciones como un rato, y el que seguia abierto al
    /// parar tambien se guarda.
    #[test]
    fn el_rato_de_escritura_llega_a_las_anotaciones() {
        let mut anotador = Anotador::new(region(), false, false);
        let mut ms = 1_000;
        for vk in [0x48u16, 0x4F, 0x4C, 0x41] {
            anotador.mirar_teclado(ms, &teclas::Foto::con(&[vk]), (100, 50));
            anotador.mirar_teclado(ms + 40, &teclas::Foto::default(), (100, 50));
            ms += 110;
        }
        let escritura = anotador.terminar().escritura;
        assert_eq!(escritura.len(), 1, "{escritura:?}");
        assert_eq!((escritura[0].desde_ms, escritura[0].hasta_ms), (1_000, 1_330));
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
