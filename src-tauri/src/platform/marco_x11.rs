//! El marco de grabar en X11: las mismas cuatro tiras que en Windows (`marco.rs`), con
//! ventanas del servidor X a pelo.
//!
//! **Solo en X11.** En Wayland una ventana no elige su sitio ni se pone encima de las demas,
//! y el escritorio ya ensenna su propio aviso de que se esta grabando.
//!
//! **Siempre por fuera de la region, o no se pinta.** En Windows las tiras se sacan de toda
//! captura, y por eso un lado pegado al borde de su pantalla puede ir por dentro. En X11 eso
//! no existe: lo que esta en pantalla sale en el video. Asi que un lado que por fuera no cabe
//! en su pantalla no se pinta, y grabando la pantalla entera no hay marco, igual que no hay
//! barra (`recorder/linux.rs`).
//!
//! **Con compositor y sin el.** Con un compositor (casi todos los escritorios) cada tira es
//! una ventana de 32 bits con su transparencia: sale el dibujo entero, con las esquinas suaves
//! y la luz de fuera. Sin compositor (un gestor de ventanas a secas) no hay transparencia: la
//! ventana se recorta a la forma de la linea (extension Shape) y se pinta de su color, sin
//! luz. Si el compositor aparece o se va a mitad (KWin lo apaga con un juego a pantalla
//! completa), el marco se vuelve a montar de la otra manera.
//!
//! El dibujo va como fondo de la ventana, asi que el servidor lo repinta solo cuando algo le
//! pasa por encima: no hay eventos que atender. El raton las atraviesa (su region de entrada
//! esta vacia), el gestor de ventanas no las toca (`override_redirect`: ni foco, ni barra de
//! tareas, ni Alt+Tab) y, si algo las tapa, se vuelven a subir encima en medio segundo.
//!
//! Viven en su propio hilo con su propia conexion, como la imagen y el raton al grabar.

#![cfg(target_os = "linux")]

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ClipOrdering, Colormap, ColormapAlloc, ConfigureWindowAux,
    ConnectionExt as _, CreateGCAux, CreateWindowAux, ImageFormat, ImageOrder, PropMode, Rectangle, StackMode,
    VisualClass, Visualid, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::marco_comun::{lados_por_dentro, linea_de_tira, pintar_tira, Color, Estilo, Forma, Medidas, ESTILO};
use crate::capture::Rect;
use crate::error::{AppError, Result};

/// Cada cuanto se mira si algo ha tapado el marco o si el compositor ha cambiado.
const MIRAR_CADA: Duration = Duration::from_millis(500);

/// Lo mas que se manda en una peticion de imagen. Sin la extension de peticiones grandes el
/// tope del servidor son 262 KB, y una tira a lo ancho de una pantalla grande pasa de ahi.
const BANDA: usize = 200_000;

/// El nombre con que se presentan las tiras (`WM_CLASS`), para quien quiera darles una regla
/// propia en su compositor.
const CLASE: &[u8] = b"winshotx-marco\0winshotx-marco\0";

/// Lo que se le pide al hilo.
enum Orden {
    Pausar(bool),
    Cerrar,
}

/// El marco en pantalla. Se cierra al soltarlo.
pub struct Marco {
    hilo: Option<JoinHandle<()>>,
    ordenes: mpsc::Sender<Orden>,
}

impl Marco {
    /// Abre el marco alrededor de la region, con las medidas de una pantalla a esa `escala`.
    ///
    /// Vuelve cuando las ventanas ya existen y estan pintadas, o con el error si no se pudo.
    /// Un marco que no se abre no impide grabar: quien llama lo apunta y sigue.
    pub fn abrir(region: Rect, escala: f64) -> Result<Self> {
        Self::abrir_con(region, escala, ESTILO)
    }

    fn abrir_con(region: Rect, escala: f64, estilo: Estilo) -> Result<Self> {
        let (ordenes, cola) = mpsc::channel::<Orden>();
        let (aviso_tx, aviso_rx) = mpsc::channel::<std::result::Result<(), String>>();
        let hilo = std::thread::Builder::new()
            .name("winshotx-marco".into())
            .spawn(move || bucle(region, escala, estilo, cola, aviso_tx))
            .map_err(|e| AppError::Msg(format!("no se ha podido abrir el marco: {e}")))?;
        aviso_rx
            .recv()
            .map_err(|_| AppError::Msg("el hilo del marco no ha llegado a arrancar".into()))?
            .map_err(AppError::Msg)?;
        Ok(Self { hilo: Some(hilo), ordenes })
    }

    /// Cambia el color: el de pausa en pausa, el de grabar grabando.
    pub fn pausado(&self, pausado: bool) {
        let _ = self.ordenes.send(Orden::Pausar(pausado));
    }
}

impl Drop for Marco {
    fn drop(&mut self) {
        let _ = self.ordenes.send(Orden::Cerrar);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

/// Lo que corre en el hilo: monta las ventanas, avisa, y atiende hasta que le digan que cierre.
fn bucle(
    region: Rect,
    escala: f64,
    estilo: Estilo,
    cola: mpsc::Receiver<Orden>,
    aviso: mpsc::Sender<std::result::Result<(), String>>,
) {
    let mut dibujo = match Dibujo::crear(region, escala, estilo) {
        Ok(dibujo) => dibujo,
        Err(error) => {
            let _ = aviso.send(Err(error));
            return;
        }
    };
    let _ = aviso.send(Ok(()));
    loop {
        match cola.recv_timeout(MIRAR_CADA) {
            Ok(Orden::Pausar(pausado)) => dibujo.pausar(pausado),
            Ok(Orden::Cerrar) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => dibujo.mirar(),
        }
    }
    dibujo.desmontar();
}

/// Como se pinta el marco.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Modo {
    /// Hay compositor: ventanas de 32 bits, con ese visual y su mapa de colores.
    Compuesto { visual: Visualid, colores: Colormap },
    /// No lo hay: ventanas como la raiz, recortadas a la linea.
    Recortado,
}

fn texto(error: impl std::fmt::Display) -> String {
    format!("el servidor X no ha querido: {error}")
}

/// Las ventanas del marco y lo que ensennan, dentro de su hilo.
struct Dibujo {
    conexion: RustConnection,
    raiz: Window,
    /// El tamanno de la pantalla de X, por si no se pueden saber los monitores.
    pantalla: (u16, u16),
    /// El mapa de colores de la raiz, para pedir el color de la linea sin compositor.
    colores_de_la_raiz: Colormap,
    /// La seleccion que tiene cogida el compositor de esta pantalla mientras esta en marcha.
    del_compositor: Atom,
    /// Con que se presentan las tiras: (`_NET_WM_WINDOW_TYPE`, `_NET_WM_WINDOW_TYPE_DOCK`).
    tipo: Option<(Atom, Atom)>,
    region: Rect,
    escala: f64,
    estilo: Estilo,
    pausado: bool,
    modo: Modo,
    forma: Forma,
    /// Los lados que no caben por fuera (arriba, abajo, izquierda, derecha): no se pintan.
    dentro: [bool; 4],
    /// Las tiras que hay en pantalla, cada una con su ventana: las que van por fuera.
    tiras: Vec<(Window, Rect)>,
}

impl Dibujo {
    fn crear(region: Rect, escala: f64, estilo: Estilo) -> std::result::Result<Self, String> {
        let (conexion, numero) =
            RustConnection::connect(None).map_err(|e| format!("no se puede hablar con el servidor X: {e}"))?;
        let (raiz, pantalla, colores_de_la_raiz) = {
            let p = conexion.setup().roots.get(numero).ok_or("el servidor X no tiene esa pantalla")?;
            (p.root, (p.width_in_pixels, p.height_in_pixels), p.default_colormap)
        };
        // Sin Shape no hay como dejar pasar el raton, y un marco que se come los clics es peor
        // que no tener marco.
        conexion
            .shape_query_version()
            .ok()
            .and_then(|c| c.reply().ok())
            .ok_or("el servidor X no tiene la extensión Shape")?;
        let atomo = |nombre: &[u8]| conexion.intern_atom(false, nombre).ok()?.reply().ok().map(|r| r.atom);
        let del_compositor = atomo(format!("_NET_WM_CM_S{numero}").as_bytes()).ok_or("el servidor X no da nombres")?;
        let tipo = atomo(b"_NET_WM_WINDOW_TYPE").zip(atomo(b"_NET_WM_WINDOW_TYPE_DOCK"));
        let medidas = Medidas::a_escala(escala, estilo);
        let mut dibujo = Self {
            conexion,
            raiz,
            pantalla,
            colores_de_la_raiz,
            del_compositor,
            tipo,
            region,
            escala,
            estilo,
            pausado: false,
            modo: Modo::Recortado,
            forma: Forma::nueva(region, medidas, [false; 4]),
            dentro: [false; 4],
            tiras: Vec::new(),
        };
        dibujo.montar()?;
        Ok(dibujo)
    }

    /// Si hay un compositor en marcha en esta pantalla.
    fn hay_compositor(&self) -> bool {
        self.conexion
            .get_selection_owner(self.del_compositor)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|r| r.owner != x11rb::NONE)
    }

    /// Como hay que pintar ahora mismo: con transparencia si hay compositor y la pantalla
    /// tiene un visual de 32 bits de los de siempre, y si no, recortando.
    fn elegir_modo(&self) -> Modo {
        if !self.hay_compositor() {
            return Modo::Recortado;
        }
        let instalacion = self.conexion.setup();
        let de_32 = instalacion.pixmap_formats.iter().any(|f| f.depth == 32 && f.bits_per_pixel == 32);
        if !de_32 || instalacion.image_byte_order != ImageOrder::LSB_FIRST {
            return Modo::Recortado;
        }
        let visual = instalacion
            .roots
            .iter()
            .find(|p| p.root == self.raiz)
            .into_iter()
            .flat_map(|p| p.allowed_depths.iter())
            .filter(|d| d.depth == 32)
            .flat_map(|d| d.visuals.iter())
            .find(|v| {
                v.class == VisualClass::TRUE_COLOR
                    && v.red_mask == 0x00FF_0000
                    && v.green_mask == 0x0000_FF00
                    && v.blue_mask == 0x0000_00FF
            })
            .map(|v| v.visual_id);
        let Some(visual) = visual else { return Modo::Recortado };
        // Una ventana con un visual que no es el de su madre necesita su mapa de colores.
        let colores = self.conexion.generate_id().ok().filter(|&id| {
            self.conexion
                .create_colormap(ColormapAlloc::NONE, id, self.raiz, visual)
                .ok()
                .is_some_and(|c| c.check().is_ok())
        });
        match colores {
            Some(colores) => Modo::Compuesto { visual, colores },
            None => Modo::Recortado,
        }
    }

    /// Las pantallas, para saber que lados caben por fuera. Si no se pueden saber, la de X
    /// entera.
    fn pantallas(&self) -> Vec<Rect> {
        let monitores: Vec<Rect> = xcap::Monitor::all()
            .map(|todos| {
                todos
                    .iter()
                    .filter_map(|m| {
                        Some(Rect { x: m.x().ok()?, y: m.y().ok()?, width: m.width().ok()?, height: m.height().ok()? })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if monitores.is_empty() {
            vec![Rect { x: 0, y: 0, width: self.pantalla.0.into(), height: self.pantalla.1.into() }]
        } else {
            monitores
        }
    }

    /// Crea las ventanas de los lados que caben por fuera, las pinta y las ensenna.
    fn montar(&mut self) -> std::result::Result<(), String> {
        self.modo = self.elegir_modo();
        let medidas = Medidas::a_escala(self.escala, self.estilo);
        // Sin compositor no hay luz: a un lado le basta con que quepan el hueco y la linea.
        let hace_falta = match self.modo {
            Modo::Compuesto { .. } => medidas.grosor(),
            Modo::Recortado => medidas.hueco + medidas.linea,
        };
        let dentro = lados_por_dentro(self.region, hace_falta, &self.pantallas());
        self.forma = Forma::nueva(self.region, medidas, dentro);
        self.dentro = dentro;
        for (tira, por_dentro) in self.forma.tiras.into_iter().zip(dentro) {
            if por_dentro {
                continue;
            }
            let ventana = self.ventana(tira)?;
            self.tiras.push((ventana, tira));
        }
        self.pintar()?;
        for (ventana, _) in &self.tiras {
            self.conexion.map_window(*ventana).map_err(texto)?;
        }
        // Hasta que el servidor no lo ha hecho todo no se vuelve: quien abre el marco se lo
        // encuentra ya en pantalla.
        self.conexion.sync().map_err(texto)
    }

    /// Una tira: una ventana que el gestor de ventanas no toca y que el raton atraviesa.
    fn ventana(&self, tira: Rect) -> std::result::Result<Window, String> {
        let c = &self.conexion;
        let id = c.generate_id().map_err(texto)?;
        let (x, y) = (tira.x.clamp(-32_768, 32_767) as i16, tira.y.clamp(-32_768, 32_767) as i16);
        let (ancho, alto) = (tira.width.clamp(1, 65_535) as u16, tira.height.clamp(1, 65_535) as u16);
        let base = CreateWindowAux::new().override_redirect(1u32).border_pixel(0u32).background_pixel(0u32);
        let creada = match self.modo {
            Modo::Compuesto { visual, colores } => c.create_window(
                32,
                id,
                self.raiz,
                x,
                y,
                ancho,
                alto,
                0,
                WindowClass::INPUT_OUTPUT,
                visual,
                &base.colormap(colores),
            ),
            Modo::Recortado => c.create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                id,
                self.raiz,
                x,
                y,
                ancho,
                alto,
                0,
                WindowClass::INPUT_OUTPUT,
                x11rb::COPY_FROM_PARENT,
                &base,
            ),
        };
        creada.map_err(texto)?.check().map_err(texto)?;
        // El raton no es nuestro: sin region de entrada, el clic llega a lo que haya debajo.
        c.shape_rectangles(shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED, id, 0, 0, &[])
            .map_err(texto)?
            .check()
            .map_err(texto)?;
        if self.modo == Modo::Recortado {
            // Lo que se ve de la ventana es solo la linea.
            let mut linea = linea_de_tira(&self.forma, tira);
            vaciar(&mut linea, 1, tira.width as usize, &self.arranques(tira), false);
            let linea = rectangulos(&linea, tira.width as usize);
            c.shape_rectangles(shape::SO::SET, shape::SK::BOUNDING, ClipOrdering::UNSORTED, id, 0, 0, &linea)
                .map_err(texto)?
                .check()
                .map_err(texto)?;
        }
        // Como se presenta. De tipo «dock» porque es el que los compositores dejan sin sombra
        // ni desenfoque: una sombra suya caeria dentro de lo que se graba.
        let _ = c.change_property8(PropMode::REPLACE, id, AtomEnum::WM_CLASS, AtomEnum::STRING, CLASE);
        if let Some((propiedad, dock)) = self.tipo {
            let _ = c.change_property32(PropMode::REPLACE, id, propiedad, AtomEnum::ATOM, &[dock]);
        }
        Ok(id)
    }

    /// Lo que sobra en esa tira por los lados que no se pintan (`arranques`).
    fn arranques(&self, tira: Rect) -> Vec<Trozo> {
        arranques(self.region, Medidas::a_escala(self.escala, self.estilo), self.dentro, tira)
    }

    /// Le da a cada ventana su dibujo, con el color del estado.
    fn pintar(&self) -> std::result::Result<(), String> {
        for &(ventana, tira) in &self.tiras {
            match self.modo {
                Modo::Compuesto { .. } => self.pintar_con_transparencia(ventana, tira)?,
                Modo::Recortado => self.pintar_de_un_color(ventana)?,
            }
        }
        self.conexion.flush().map_err(texto)
    }

    /// El dibujo entero, como fondo de la ventana: asi el servidor lo repinta solo.
    fn pintar_con_transparencia(&self, ventana: Window, tira: Rect) -> std::result::Result<(), String> {
        let c = &self.conexion;
        let mut bgra = pintar_tira(&self.forma, tira, self.estilo, self.pausado);
        vaciar(&mut bgra, 4, tira.width as usize, &self.arranques(tira), 0);
        let (ancho, alto) = (tira.width.clamp(1, 65_535) as u16, tira.height.clamp(1, 65_535) as u16);
        let lienzo = c.generate_id().map_err(texto)?;
        c.create_pixmap(32, lienzo, ventana, ancho, alto).map_err(texto)?;
        let pincel = c.generate_id().map_err(texto)?;
        c.create_gc(pincel, lienzo, &CreateGCAux::new()).map_err(texto)?;
        let por_fila = usize::from(ancho) * 4;
        let filas_por_banda = (BANDA / por_fila).max(1);
        for (n, banda) in bgra.chunks(filas_por_banda * por_fila).enumerate() {
            let filas = (banda.len() / por_fila) as u16;
            let desde = (n * filas_por_banda) as i16;
            c.put_image(ImageFormat::Z_PIXMAP, lienzo, pincel, ancho, filas, 0, desde, 0, 32, banda).map_err(texto)?;
        }
        c.change_window_attributes(ventana, &ChangeWindowAttributesAux::new().background_pixmap(lienzo))
            .map_err(texto)?
            .check()
            .map_err(texto)?;
        c.clear_area(false, ventana, 0, 0, 0, 0).map_err(texto)?;
        // La ventana se queda con su copia: el lienzo ya no hace falta.
        let _ = c.free_gc(pincel);
        let _ = c.free_pixmap(lienzo);
        Ok(())
    }

    /// Sin transparencia, la ventana (recortada a la linea) va entera del color de la linea.
    fn pintar_de_un_color(&self, ventana: Window) -> std::result::Result<(), String> {
        let c = &self.conexion;
        let Color(r, g, b) = self.estilo.linea(self.pausado);
        let a_16 = |v: u8| u16::from(v) * 257;
        let color = c
            .alloc_color(self.colores_de_la_raiz, a_16(r), a_16(g), a_16(b))
            .map_err(texto)?
            .reply()
            .map_err(texto)?
            .pixel;
        c.change_window_attributes(ventana, &ChangeWindowAttributesAux::new().background_pixel(color))
            .map_err(texto)?;
        c.clear_area(false, ventana, 0, 0, 0, 0).map_err(texto)?;
        Ok(())
    }

    fn pausar(&mut self, pausado: bool) {
        if pausado != self.pausado {
            self.pausado = pausado;
            let _ = self.pintar();
        }
    }

    /// Lo de cada medio segundo: si el compositor ha aparecido o se ha ido, el marco se monta
    /// de la otra manera; y si algo lo tapa, se sube.
    fn mirar(&mut self) {
        // Nadie pide eventos, pero alguno llega igual (un cambio de teclado): que no se queden.
        while let Ok(Some(_)) = self.conexion.poll_for_event() {}
        if self.hay_compositor() != matches!(self.modo, Modo::Compuesto { .. }) {
            self.desmontar();
            if let Err(error) = self.montar() {
                eprintln!("[grabar] el marco no se ha podido volver a montar: {error}");
            }
            return;
        }
        self.subir();
    }

    /// Las deja encima de todo, si no lo estan ya.
    fn subir(&self) {
        if self.tiras.is_empty() {
            return;
        }
        let Some(arbol) = self.conexion.query_tree(self.raiz).ok().and_then(|c| c.reply().ok()) else {
            return;
        };
        // Las ventanas vienen de abajo arriba: las nuestras tienen que ser las ultimas.
        let nuestra = |ventana: &Window| self.tiras.iter().any(|(v, _)| v == ventana);
        let n = self.tiras.len();
        if arbol.children.len() >= n && arbol.children[arbol.children.len() - n..].iter().all(nuestra) {
            return;
        }
        let encima = ConfigureWindowAux::new().stack_mode(StackMode::ABOVE);
        for (ventana, _) in &self.tiras {
            let _ = self.conexion.configure_window(*ventana, &encima);
        }
        let _ = self.conexion.flush();
    }

    /// Quita las ventanas de la pantalla.
    fn desmontar(&mut self) {
        for (ventana, _) in self.tiras.drain(..) {
            let _ = self.conexion.destroy_window(ventana);
        }
        if let Modo::Compuesto { colores, .. } = self.modo {
            let _ = self.conexion.free_colormap(colores);
        }
        self.modo = Modo::Recortado;
        let _ = self.conexion.sync();
    }
}

/// Un trozo de una tira, en sus pixeles: columna, fila, ancho y alto.
type Trozo = (usize, usize, usize, usize);

/// Lo que sobra en una tira cuando el lado izquierdo o el derecho no se pinta.
///
/// La figura comun cuenta por dentro el lado que no cabe por fuera y, para unirlo con sus
/// vecinos, deja en las tiras de arriba y de abajo su arranque: el trozo de linea que cruza el
/// hueco hasta la region. En Windows ese lado se pinta por dentro y el arranque lo cierra. Aqui
/// no se pinta, y el arranque se quedaria en un piquito suelto al final de la linea.
fn arranques(region: Rect, medidas: Medidas, dentro: [bool; 4], tira: Rect) -> Vec<Trozo> {
    let (x0, x1) = (region.x, region.x + region.width as i32);
    let (y0, y1) = (region.y, region.y + region.height as i32);
    let mut sobra = Vec::new();
    for (por_dentro, columnas) in [(dentro[2], (x0, x0 + medidas.linea)), (dentro[3], (x1 - medidas.linea, x1))] {
        if !por_dentro {
            continue;
        }
        for filas in [(y0 - medidas.hueco, y0), (y1, y1 + medidas.hueco)] {
            // Lo que de ese trozo cae en la tira.
            let (a, b) = (columnas.0.max(tira.x), columnas.1.min(tira.x + tira.width as i32));
            let (c, d) = (filas.0.max(tira.y), filas.1.min(tira.y + tira.height as i32));
            if a < b && c < d {
                sobra.push(((a - tira.x) as usize, (c - tira.y) as usize, (b - a) as usize, (d - c) as usize));
            }
        }
    }
    sobra
}

/// Deja vacios esos trozos de una tira de `ancho` pixeles por fila, con `cada` valores por pixel.
fn vaciar<T: Copy>(pixeles: &mut [T], cada: usize, ancho: usize, trozos: &[Trozo], vacio: T) {
    for &(columna, fila, cuantas, filas) in trozos {
        for f in fila..fila + filas {
            let desde = (f * ancho + columna) * cada;
            pixeles[desde..desde + cuantas * cada].fill(vacio);
        }
    }
}

/// Los rectangulos que cubren justo los pixeles marcados (fila a fila, `ancho` por fila), para
/// recortar una ventana: un rectangulo por tramo, estirado hacia abajo mientras las filas
/// siguientes repitan los mismos tramos. Un lado recto sale en un solo rectangulo.
fn rectangulos(marcados: &[bool], ancho: usize) -> Vec<Rectangle> {
    let mut hechos: Vec<Rectangle> = Vec::new();
    // Los tramos de la fila anterior: (donde empieza, lo que mide, cual es en `hechos`).
    let mut abiertos: Vec<(usize, usize, usize)> = Vec::new();
    for (y, fila) in marcados.chunks(ancho.max(1)).enumerate() {
        let mut tramos: Vec<(usize, usize)> = Vec::new();
        let mut x = 0;
        while x < fila.len() {
            if !fila[x] {
                x += 1;
                continue;
            }
            let desde = x;
            while x < fila.len() && fila[x] {
                x += 1;
            }
            tramos.push((desde, x - desde));
        }
        let repetida =
            tramos.len() == abiertos.len() && tramos.iter().zip(&abiertos).all(|(t, a)| *t == (a.0, a.1));
        if repetida {
            for (_, _, cual) in &abiertos {
                hechos[*cual].height += 1;
            }
        } else {
            abiertos = tramos
                .into_iter()
                .map(|(desde, largo)| {
                    hechos.push(Rectangle { x: desde as i16, y: y as i16, width: largo as u16, height: 1 });
                    (desde, largo, hechos.len() - 1)
                })
                .collect();
        }
    }
    hechos
}

#[cfg(test)]
#[path = "marco_x11_pruebas.rs"]
mod tests;
