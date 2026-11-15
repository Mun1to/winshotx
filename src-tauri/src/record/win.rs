use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Instant;

use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor as WcMonitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use crate::capture::Rect;
use crate::error::{AppError, Result};
use crate::record::comparar_gpu::{Cambio, Comparador};
use crate::record::delta::Parche;

/// windows-capture identifica monitores por HMONITOR, asi que se lo pedimos a Win32.
pub(crate) fn monitor_at(x: i32, y: i32) -> Result<WcMonitor> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};

    let handle = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    if handle.is_invalid() {
        return Err(AppError::Msg("no hay monitor en esa posición".into()));
    }
    Ok(WcMonitor::from_raw_hmonitor(handle.0))
}

/// Un fotograma recien capturado, ya recortado a la region y en BGRA.
pub struct CapturedFrame {
    /// El fotograma entero, o, si viene `zonas`, solo esas zonas una detras de otra.
    pub bgra: Vec<u8>,
    pub ts_ms: u64,
    /// S29: con `CaptureFlags::solo_cambios`, lo que cambio desde el fotograma anterior
    /// que se ENTREGO (no desde el anterior que dibujo Windows): sus rectangulos, en
    /// coordenadas de la region, y en `bgra` los pixeles de cada uno, fila a fila y uno
    /// detras de otro. `None` es un fotograma entero, como siempre.
    ///
    /// Quien lo pide tiene que aplicar TODOS los que le lleguen, en orden, sobre su copia:
    /// tirar uno es quedarse con un trozo de pantalla viejo para siempre (hasta el siguiente
    /// entero).
    pub zonas: Option<Vec<Parche>>,
}

#[derive(Clone)]
pub struct CaptureFlags {
    pub sender: Sender<CapturedFrame>,
    /// Recorte dentro del monitor: x1, y1, x2, y2.
    pub crop: (u32, u32, u32, u32),
    pub stop: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
    /// Milisegundos acumulados en pausa, para que el reloj no cuente ese tiempo.
    pub paused_ms: Arc<AtomicU64>,
    pub min_interval_ms: u64,
    /// El cero del reloj de los fotogramas. Lo crea quien arranca la grabacion y lo
    /// comparte con el hilo que muestrea el raton: asi el rastro y la imagen van en el
    /// mismo tiempo. Antes cada uno tenia su `Instant::now()` y se separaban unos
    /// milisegundos que no se podian corregir.
    pub start: Instant,
    /// Los bufers ya usados que vuelven para el siguiente fotograma. Ver `Reciclados`.
    pub reciclados: Reciclados,
    /// S29: entregar solo lo que cambio (ver `CapturedFrame::zonas`) cuando Windows sabe
    /// decirlo. Lo pide el anillo, que corre toda la tarde y casi siempre ve una pantalla
    /// en la que solo se mueve el raton. La grabacion normal no: su codificador quiere el
    /// fotograma entero en cada vuelta.
    pub solo_cambios: bool,
}

/// Bufers de fotograma que van y vuelven, para no reservar ocho megabytes nuevos sesenta
/// veces por segundo.
///
/// **Medido el 17 de septiembre de 2026** (`bench_anillo`, en release): copiar un fotograma
/// de 1920x1080 a un `Vec` recien reservado cuesta 3,4 ms, y copiarlo a uno que ya se uso
/// cuesta 0,5 ms. La diferencia no es la copia, es que Windows tiene que buscar y poner a
/// cero ocho megabytes de paginas nuevas cada vez, y el anillo hace eso toda la tarde.
///
/// Quien consume el fotograma lo devuelve con `devolver` cuando termina; la captura coge
/// uno con `coger` antes de copiar. Si no hay ninguno devuelto se reserva uno nuevo, como
/// antes: esto es un ahorro, nunca una espera.
#[derive(Clone, Default)]
pub struct Reciclados(Arc<parking_lot::Mutex<Vec<Vec<u8>>>>);

impl Reciclados {
    /// Cuantos se guardan como mucho. Los que sobren se sueltan: si el consumidor va por
    /// detras y devuelve de golpe, no hace falta quedarse con veinte fotogramas en memoria.
    const TOPE: usize = 4;

    /// Un bufer vacio, con la memoria de la vez anterior si la hay.
    pub fn coger(&self) -> Vec<u8> {
        self.0.lock().pop().unwrap_or_default()
    }

    /// Devuelve un bufer para que lo use el siguiente fotograma.
    pub fn devolver(&self, mut bufer: Vec<u8>) {
        bufer.clear();
        let mut guardados = self.0.lock();
        if guardados.len() < Self::TOPE {
            guardados.push(bufer);
        }
    }
}

/// La textura de la que se leen los fotogramas, guardada entre uno y otro.
///
/// **Medido en S29:** el crate creaba una textura de ocho megabytes NUEVA en cada fotograma
/// (`Frame::buffer_crop`) y copiaba a ella la pantalla entera. Guardandola, ademas, se queda
/// con lo que tenia: copiar a ella solo las zonas que cambiaron basta para que siga teniendo
/// la pantalla entera al dia (es lo que comprueba `verificacion`).
struct Lector {
    textura: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    ancho: u32,
    alto: u32,
}

impl Lector {
    fn nuevo(frame: &Frame, ancho: u32, alto: u32) -> std::result::Result<Self, windows::core::Error> {
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_CPU_ACCESS_READ, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
        };
        use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: ancho,
            Height: alto,
            MipLevels: 1,
            ArraySize: 1,
            Format: frame.desc().Format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut textura = None;
        unsafe { frame.device().CreateTexture2D(&desc, None, Some(&mut textura))? };
        Ok(Self {
            textura: textura.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?,
            ancho,
            alto,
        })
    }
}

pub struct RegionCapture {
    flags: CaptureFlags,
    last_emit: Option<Instant>,
    lector: Option<Lector>,
    /// Con `solo_cambios`: la comparacion en la GPU con el ultimo fotograma entregado. Ver
    /// `comparar_gpu`.
    comparador: Option<Comparador>,
    /// Si crear el comparador fallo una vez, no se reintenta en cada fotograma: se sigue
    /// leyendo entero, como siempre.
    sin_comparador: bool,
}

impl GraphicsCaptureApiHandler for RegionCapture {
    type Flags = CaptureFlags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            flags: ctx.flags,
            last_emit: None,
            lector: None,
            comparador: None,
            sin_comparador: false,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> std::result::Result<(), Self::Error> {
        if self.flags.stop.load(Ordering::Relaxed) {
            control.stop();
            return Ok(());
        }
        // En pausa no se lee nada. Al volver, el comparador sigue teniendo el ultimo que se
        // entrego, asi que lo que cambio mientras tanto sale en la primera comparacion.
        if self.flags.pause.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Windows entrega fotogramas al ritmo del compositor; aqui se recorta al fps pedido.
        // Los que se saltan no hacen falta para nada: se compara siempre con el ultimo
        // ENTREGADO, no con el anterior de Windows.
        let now = Instant::now();
        if let Some(last) = self.last_emit {
            if now.duration_since(last).as_millis() < self.flags.min_interval_ms as u128 {
                return Ok(());
            }
        }

        let (x1, y1, x2, y2) = self.flags.crop;
        // Grabando una VENTANA, su tamanno puede cambiar mientras se graba, y el
        // codificador necesita el mismo tamanno en todos los fotogramas. Se coge lo que
        // haya y lo que falte se queda en negro; si la ventana se ha ido del todo, este
        // fotograma se salta.
        let (ancho_real, alto_real) = (frame.width(), frame.height());
        let (hasta_x, hasta_y) = (x2.min(ancho_real), y2.min(alto_real));
        if x1 >= hasta_x || y1 >= hasta_y {
            return Ok(());
        }
        let pedido = ((x2 - x1) as usize, (y2 - y1) as usize);
        let hay = (hasta_x - x1, hasta_y - y1);
        if self.lector.as_ref().is_none_or(|l| (l.ancho, l.alto) != hay) {
            self.lector = Some(Lector::nuevo(frame, hay.0, hay.1)?);
        }

        // Que se lee: solo lo que cambio, o todo. El comparador solo cuando se pidio y la
        // region esta entera (una ventana que encogio va entera, rellena de negro).
        let entera = hay == (pedido.0 as u32, pedido.1 as u32);
        if self.flags.solo_cambios && entera && !self.sin_comparador {
            if self.comparador.as_ref().is_none_or(|c| c.medida() != hay) {
                match Comparador::nuevo(frame.device(), frame.desc().Format, hay.0, hay.1) {
                    Ok(c) => self.comparador = Some(c),
                    Err(error) => {
                        eprintln!("[captura] sin comparar en la GPU, se lee entero: {error}");
                        self.sin_comparador = true;
                        self.comparador = None;
                    }
                }
            }
        } else {
            self.comparador = None;
        }
        let zonas = match self.comparador.as_mut() {
            Some(comparador) => match comparador.comparar(frame.device_context(), frame.as_raw_texture(), (x1, y1)) {
                // El mismo fotograma: ni se lee ni se manda, y el ritmo no lo cuenta.
                Ok(Cambio::Nada) => return Ok(()),
                Ok(Cambio::Entero) => None,
                Ok(Cambio::Zonas(zonas)) => Some(zonas),
                Err(error) => {
                    eprintln!("[captura] la comparacion en la GPU ha fallado, se lee entero: {error}");
                    self.sin_comparador = true;
                    self.comparador = None;
                    None
                }
            },
            None => None,
        };
        self.last_emit = Some(now);

        // De donde se lee: de la copia que ya hizo el comparador (la region recortada, en
        // 0,0), o del fotograma de Windows.
        let lector = self.lector.as_ref().expect("el lector se acaba de crear");
        let (origen, esquina) = match self.comparador.as_ref().and_then(Comparador::ultimo) {
            Some(textura) => (textura, (0, 0)),
            None => (frame.as_raw_texture(), (x1, y1)),
        };
        let data = leer(frame.device_context(), origen, esquina, lector, &zonas, pedido, &self.flags.reciclados)?;
        #[cfg(test)]
        if zonas.is_some() && verificacion::ACTIVA.load(Ordering::Relaxed) {
            verificacion::comparar(frame, lector, (x1, y1));
        }

        let elapsed = now.duration_since(self.flags.start).as_millis() as u64;
        let ts_ms = elapsed.saturating_sub(self.flags.paused_ms.load(Ordering::Relaxed));
        let _ = self.flags.sender.send(CapturedFrame {
            bgra: data,
            ts_ms,
            zonas,
        });
        Ok(())
    }

    fn on_closed(&mut self) -> std::result::Result<(), Self::Error> {
        self.flags.stop.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// S29: la prueba de que leer solo las zonas no pierde nada. Puesta, cada fotograma que se
/// lee por zonas se lee TAMBIEN entero (por el camino de siempre del crate) y se compara
/// pixel a pixel con lo que tiene la textura guardada: si se escapara un cambio, ahi
/// saldria. Asi se vio que las zonas que dice Windows NO valen (ver `comparar_gpu`). Solo
/// en pruebas.
#[cfg(test)]
pub(crate) mod verificacion {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use windows_capture::frame::Frame;

    pub static ACTIVA: AtomicBool = AtomicBool::new(false);
    pub static MIRADOS: AtomicU64 = AtomicU64::new(0);
    pub static MALOS: AtomicU64 = AtomicU64::new(0);
    pub static PIXELES_MALOS: AtomicU64 = AtomicU64::new(0);

    pub fn reiniciar() {
        MIRADOS.store(0, Ordering::Relaxed);
        MALOS.store(0, Ordering::Relaxed);
        PIXELES_MALOS.store(0, Ordering::Relaxed);
    }

    pub(super) fn comparar(frame: &mut Frame, lector: &super::Lector, (x1, y1): (u32, u32)) {
        use windows::Win32::Graphics::Direct3D11::{D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ};
        let fresco = {
            let Ok(buffer) = frame.buffer_crop(x1, y1, x1 + lector.ancho, y1 + lector.alto) else {
                return;
            };
            let mut apoyo = Vec::new();
            buffer.as_nopadding_buffer(&mut apoyo).to_vec()
        };
        let contexto = frame.device_context();
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        if unsafe { contexto.Map(&lector.textura, 0, D3D11_MAP_READ, 0, Some(&mut mapa)) }.is_err() {
            return;
        }
        let fila = lector.ancho as usize * 4;
        let mut malos = 0u64;
        for y in 0..lector.alto as usize {
            let guardada =
                unsafe { std::slice::from_raw_parts((mapa.pData as *const u8).add(y * mapa.RowPitch as usize), fila) };
            let nueva = &fresco[y * fila..(y + 1) * fila];
            if guardada != nueva {
                malos += guardada
                    .chunks_exact(4)
                    .zip(nueva.chunks_exact(4))
                    .filter(|(a, b)| a != b)
                    .count() as u64;
            }
        }
        unsafe { contexto.Unmap(&lector.textura, 0) };
        MIRADOS.fetch_add(1, Ordering::Relaxed);
        if malos > 0 {
            MALOS.fetch_add(1, Ordering::Relaxed);
            PIXELES_MALOS.fetch_add(malos, Ordering::Relaxed);
        }
    }
}

/// Copia de la GPU a la textura guardada lo que haya que leer (esas zonas, o la region
/// entera) y lo saca a un bufer: las zonas una detras de otra, o el fotograma entero con lo
/// que falte en negro si la ventana encogio.
fn leer(
    contexto: &windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    origen: &windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    (x1, y1): (u32, u32),
    lector: &Lector,
    zonas: &Option<Vec<Parche>>,
    pedido: (usize, usize),
    reciclados: &Reciclados,
) -> std::result::Result<Vec<u8>, windows::core::Error> {
    use windows::Win32::Graphics::Direct3D11::{D3D11_BOX, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ};

    let entera = [Parche { x: 0, y: 0, width: lector.ancho, height: lector.alto }];
    let que = zonas.as_deref().unwrap_or(&entera);
    for z in que {
        let caja = D3D11_BOX {
            left: x1 + z.x,
            top: y1 + z.y,
            front: 0,
            right: x1 + z.x + z.width,
            bottom: y1 + z.y + z.height,
            back: 1,
        };
        unsafe {
            contexto.CopySubresourceRegion(&lector.textura, 0, z.x, z.y, 0, origen, 0, Some(&caja));
        }
    }
    let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { contexto.Map(&lector.textura, 0, D3D11_MAP_READ, 0, Some(&mut mapa))? };
    let paso = mapa.RowPitch as usize;
    let base = mapa.pData as *const u8;
    let fila = |y: u32, x: u32, ancho: u32| -> &[u8] {
        // SAFETY: la textura mide `lector.ancho` x `lector.alto` y esta mapeada hasta el
        // Unmap de abajo; las zonas caen dentro (salen de teselas recortadas a la region).
        unsafe { std::slice::from_raw_parts(base.add(y as usize * paso + x as usize * 4), ancho as usize * 4) }
    };
    let mut data = reciclados.coger();
    match zonas {
        Some(zonas) => {
            data.reserve(zonas.iter().map(|z| z.pixeles() as usize * 4).sum());
            for z in zonas {
                for y in z.y..z.y + z.height {
                    data.extend_from_slice(fila(y, z.x, z.width));
                }
            }
        }
        None if (lector.ancho as usize, lector.alto as usize) == pedido => {
            data.reserve(pedido.0 * pedido.1 * 4);
            for y in 0..lector.alto {
                data.extend_from_slice(fila(y, 0, lector.ancho));
            }
        }
        None => {
            data.resize(pedido.0 * pedido.1 * 4, 0);
            for y in 0..lector.alto as usize {
                let destino = y * pedido.0 * 4;
                data[destino..destino + lector.ancho as usize * 4].copy_from_slice(fila(y as u32, 0, lector.ancho));
            }
        }
    }
    unsafe { contexto.Unmap(&lector.textura, 0) };
    Ok(data)
}

pub type Control =
    windows_capture::capture::CaptureControl<RegionCapture, Box<dyn std::error::Error + Send + Sync>>;

/// Arranca la captura de la region sobre el monitor que la contiene.
/// `monitor_origin` viene de la enumeracion de xcap, que si expone coordenadas.
pub fn start(
    region: Rect,
    monitor_origin: (i32, i32),
    capture_cursor: bool,
    fps: u32,
    flags_base: CaptureFlags,
) -> Result<Control> {
    let (cx, cy) = region.center();
    let monitor = monitor_at(cx, cy)?;

    let x1 = (region.x - monitor_origin.0).max(0) as u32;
    let y1 = (region.y - monitor_origin.1).max(0) as u32;
    let flags = CaptureFlags {
        crop: (x1, y1, x1 + region.width, y1 + region.height),
        min_interval_ms: (1000 / fps.max(1)).saturating_sub(1) as u64,
        ..flags_base
    };

    let settings = Settings::new(
        monitor,
        if capture_cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        },
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    );

    RegionCapture::start_free_threaded(settings).map_err(|e| AppError::Msg(e.to_string()))
}

/// Arranca la captura de UNA VENTANA, se vea o no se vea.
///
/// Es lo que no se puede hacer capturando la pantalla y recortando: ahi lo que se graba es
/// lo que hay delante, asi que cualquier cosa que pase por encima (un aviso, el explorador,
/// otra ventana) acaba dentro del video. Windows compone cada ventana por su cuenta, y
/// pidiendole ESA, lo de delante no sale.
///
/// **El tamanno se clava al que tiene al empezar.** Una ventana se puede redimensionar
/// mientras se graba y el codificador necesita que todos los fotogramas midan igual: si
/// crece, se graba el trozo de siempre; si encoge, lo que falta se queda en negro.
pub fn start_window(
    hwnd: isize,
    capture_cursor: bool,
    fps: u32,
    flags_base: CaptureFlags,
) -> Result<Control> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};

    let ventana = HWND(hwnd as *mut core::ffi::c_void);
    // El tamanno que se le pide a Windows es el del marco REAL (el que compone el
    // escritorio), no el de `GetWindowRect`, que incluye el borde invisible de las sombras
    // y dejaria franjas transparentes a los lados.
    let mut marco = RECT::default();
    let medida = unsafe {
        DwmGetWindowAttribute(
            ventana,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut marco).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
    };
    if medida.is_err() {
        return Err(AppError::Msg("esa ventana ya no está".into()));
    }
    let ancho = (marco.right - marco.left).max(1) as u32;
    let alto = (marco.bottom - marco.top).max(1) as u32;

    let flags = CaptureFlags {
        // Entera: el recorte dentro de la ventana lo hace despues el editor, que ahi si se
        // puede deshacer.
        crop: (0, 0, ancho, alto),
        min_interval_ms: (1000 / fps.max(1)).saturating_sub(1) as u64,
        ..flags_base
    };

    let settings = Settings::new(
        windows_capture::window::Window::from_raw_hwnd(ventana.0),
        if capture_cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        },
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    );

    RegionCapture::start_free_threaded(settings).map_err(|e| AppError::Msg(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc::channel;
    use std::time::Duration;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::CreateSolidBrush;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, ShowWindow,
        SW_SHOWNOACTIVATE, WNDCLASSW,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    const CLASE: &[u16] = &[
        b'w' as u16, b's' as u16, b'x' as u16, b'-' as u16, b'p' as u16, b'r' as u16,
        b'u' as u16, b'e' as u16, b'b' as u16, b'a' as u16, 0,
    ];

    /// Lo unico que hace esta ventana es dejarse pintar del color de su clase.
    unsafe extern "system" fn procedimiento(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }

    /// Un rectangulo magenta, encima de todo y que no se puede pulsar, en la esquina de la
    /// pantalla principal. El magenta no sale en ningun escritorio por accidente.
    fn ventana_magenta(region: Rect) -> isize {
        let instancia = unsafe { GetModuleHandleW(None) }.expect("sin instancia");
        let clase = WNDCLASSW {
            hInstance: instancia.into(),
            lpszClassName: PCWSTR(CLASE.as_ptr()),
            lpfnWndProc: Some(procedimiento),
            hbrBackground: unsafe { CreateSolidBrush(COLORREF(0x00FF_00FF)) },
            ..Default::default()
        };
        let _ = unsafe { RegisterClassW(&clase) };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                PCWSTR(CLASE.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                region.x,
                region.y,
                region.width as i32,
                region.height as i32,
                None,
                None,
                Some(instancia.into()),
                None,
            )
        }
        .expect("no se ha podido abrir la ventana de prueba");
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        hwnd.0 as isize
    }

    /// Graba la region medio segundo y dice si el ultimo fotograma lleva magenta.
    fn sale_el_magenta(region: Rect, ventana: isize) -> bool {
        // Windows solo entrega un fotograma cuando algo cambia en el monitor. Con la
        // pantalla quieta llegaba UNO y luego nada, y esta prueba dependia de que el
        // terminal estuviera escribiendo en esa pantalla para pasar. Asi que la propia
        // ventana se mueve un pixel de un lado a otro mientras dura la captura, y los
        // fotogramas llegan porque hay motivo, no por suerte.
        //
        // La mueve ESTE hilo, que es el que la creo: desde otro hilo `SetWindowPos` manda
        // el mensaje al dueño, que estaria aqui esperando fotogramas sin atenderlo, y se
        // queda colgado para siempre. Ya paso.
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        };
        let hwnd = HWND(ventana as *mut core::ffi::c_void);
        let mut paso = 0;
        let (sender, receiver) = channel::<CapturedFrame>();
        let stop = Arc::new(AtomicBool::new(false));
        let control = start(
            region,
            (0, 0),
            false,
            30,
            CaptureFlags {
                sender,
                crop: (0, 0, 0, 0),
                stop: stop.clone(),
                pause: Arc::new(AtomicBool::new(false)),
                paused_ms: Arc::new(AtomicU64::new(0)),
                min_interval_ms: 0,
                start: Instant::now(),
                reciclados: Reciclados::default(),
                solo_cambios: false,
            },
        )
        .expect("no se ha podido arrancar la captura");

        // El ULTIMO de medio segundo, no el primero: los primeros fotogramas pueden venir
        // de antes de que el compositor haya pintado el cambio.
        let hasta = Instant::now() + Duration::from_millis(600);
        let mut ultimo = None;
        while Instant::now() < hasta {
            paso = (paso + 1) % 2;
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    region.x + paso,
                    region.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            if let Ok(frame) = receiver.recv_timeout(Duration::from_millis(30)) {
                ultimo = Some(frame);
            }
        }
        stop.store(true, Ordering::Relaxed);
        let _ = control.stop();

        let frame = ultimo.expect("la captura no ha dado ni un fotograma");
        // BGRA: magenta es azul al maximo, verde a cero y rojo al maximo.
        frame
            .bgra
            .chunks_exact(4)
            .any(|p| p[0] > 200 && p[1] < 60 && p[2] > 200)
    }

    /// Graba LA VENTANA medio segundo y dice si lo que sale es suyo.
    ///
    /// Igual que `sale_el_magenta`, pero pidiendole a Windows esa ventana en vez de un
    /// trozo de pantalla. Se mueve desde este hilo por lo mismo: es el que la creo.
    fn graba_la_ventana(ventana: isize, region: Rect) -> Option<CapturedFrame> {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        };
        let hwnd = HWND(ventana as *mut core::ffi::c_void);
        let mut paso = 0;
        let (sender, receiver) = channel::<CapturedFrame>();
        let stop = Arc::new(AtomicBool::new(false));
        let control = start_window(
            ventana,
            false,
            30,
            CaptureFlags {
                sender,
                crop: (0, 0, 0, 0),
                stop: stop.clone(),
                pause: Arc::new(AtomicBool::new(false)),
                paused_ms: Arc::new(AtomicU64::new(0)),
                min_interval_ms: 0,
                start: Instant::now(),
                reciclados: Reciclados::default(),
                solo_cambios: false,
            },
        )
        .expect("no se ha podido arrancar la captura de la ventana");

        let hasta = Instant::now() + Duration::from_millis(600);
        let mut ultimo = None;
        while Instant::now() < hasta {
            paso = (paso + 1) % 2;
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    region.x + paso,
                    region.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            if let Ok(frame) = receiver.recv_timeout(Duration::from_millis(30)) {
                ultimo = Some(frame);
            }
        }
        stop.store(true, Ordering::Relaxed);
        let _ = control.stop();
        ultimo
    }

    /// Cuantos pixeles de ese fotograma son magenta, en tanto por ciento.
    fn cuanto_magenta(frame: &CapturedFrame) -> f32 {
        let total = frame.bgra.len() / 4;
        if total == 0 {
            return 0.0;
        }
        let magenta = frame
            .bgra
            .chunks_exact(4)
            .filter(|p| p[0] > 200 && p[1] < 60 && p[2] > 200)
            .count();
        magenta as f32 * 100.0 / total as f32
    }

    /// **Lo que promete grabar una ventana: que lo de delante no sale.**
    ///
    /// Se abren dos ventanas en el mismo sitio, una magenta debajo y una verde encima que
    /// la tapa entera. Grabando la PANTALLA ahi solo se ve verde, que es el problema de
    /// siempre: un aviso que salta encima se cuela en el video. Grabando la VENTANA
    /// magenta, Windows la compone aparte y lo que sale es magenta.
    ///
    ///     cargo test --lib grabar_una_ventana -- --ignored --nocapture
    #[test]
    #[ignore = "abre dos ventanas en la pantalla y graba un trozo: se corre a mano"]
    fn grabar_una_ventana_tapada_la_graba_a_ella_y_no_a_lo_de_delante() {
        let region = Rect { x: 0, y: 0, width: 160, height: 120 };
        let abajo = ventana_magenta(region);
        std::thread::sleep(Duration::from_millis(300));

        let suyo = graba_la_ventana(abajo, region).expect("sin fotogramas de la ventana");
        let antes = cuanto_magenta(&suyo);
        println!("sin nada delante: {antes:.1}% magenta");
        assert!(antes > 50.0, "la ventana no se graba ni estando a la vista: {antes:.1}%");

        // Y ahora una verde del mismo tamanno encima, que la tapa entera.
        let encima = ventana_tapadera(region);
        std::thread::sleep(Duration::from_millis(400));

        // La pantalla de esa esquina ya no tiene magenta: lo de delante manda.
        let de_la_pantalla = sale_el_magenta(region, abajo);

        let tapada = graba_la_ventana(abajo, region).expect("sin fotogramas con la tapadera");
        let despues = cuanto_magenta(&tapada);
        println!("con la verde delante: {despues:.1}% magenta");

        unsafe {
            let _ = DestroyWindow(HWND(encima as *mut core::ffi::c_void));
            let _ = DestroyWindow(HWND(abajo as *mut core::ffi::c_void));
        }

        assert!(
            !de_la_pantalla,
            "la tapadera no esta tapando: esta prueba no prueba nada asi"
        );
        assert!(
            despues > 50.0,
            "tapada, la grabacion de la ventana ha salido con {despues:.1}% de lo suyo: \
             se esta grabando lo de delante y no la ventana"
        );
    }

    /// **El estudio sigue a la ventana cuando se mueve.**
    ///
    /// Los aros de clic y el rastro del raton se guardan en coordenadas de lo que se
    /// graba, restando la esquina de la zona. Grabando una VENTANA esa esquina cambia en
    /// cuanto alguien la arrastra, asi que se pregunta en cada muestra: si se guardara la
    /// del principio, al mover la ventana los aros saldrian corridos justo lo que se movio.
    ///
    ///     cargo test --lib la_esquina_sigue -- --ignored --nocapture
    #[test]
    #[ignore = "abre una ventana en la pantalla y la mueve: se corre a mano"]
    fn la_esquina_sigue_a_la_ventana_cuando_se_mueve() {
        use crate::record::anotador::esquina_de_la_ventana;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        };

        let region = Rect { x: 40, y: 40, width: 160, height: 120 };
        let ventana = ventana_magenta(region);
        std::thread::sleep(Duration::from_millis(250));

        let antes = esquina_de_la_ventana(ventana).expect("sin esquina al empezar");
        println!("esquina al empezar: {antes:?}");

        // Se mueve 120 px a la derecha y 60 hacia abajo, como haria cualquiera.
        let hwnd = HWND(ventana as *mut core::ffi::c_void);
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                None,
                region.x + 120,
                region.y + 60,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        std::thread::sleep(Duration::from_millis(250));
        let despues = esquina_de_la_ventana(ventana).expect("sin esquina despues de mover");
        println!("esquina despues de mover: {despues:?}");

        unsafe {
            let _ = DestroyWindow(hwnd);
        }

        let (dx, dy) = (despues.0 - antes.0, despues.1 - antes.1);
        assert!(
            (dx - 120).abs() <= 2 && (dy - 60).abs() <= 2,
            "la esquina tenia que moverse 120 x 60 y se ha movido {dx} x {dy}"
        );
    }

    /// Y si la ventana ya no esta, no hay esquina: quien pregunta se queda con la ultima
    /// que supo, que es mejor que mandar todos los aros al cero de la pantalla.
    #[test]
    fn una_ventana_que_no_existe_no_da_esquina() {
        use crate::record::anotador::esquina_de_la_ventana;
        assert!(esquina_de_la_ventana(0).is_none());
        assert!(esquina_de_la_ventana(0x7FFF_FFFF).is_none());
    }

    /// Una ventana verde del mismo tamanno, para ponerla encima.
    fn ventana_tapadera(region: Rect) -> isize {
        const CLASE_VERDE: &[u16] = &[
            b'w' as u16, b's' as u16, b'x' as u16, b't' as u16, b'a' as u16, b'p' as u16, 0,
        ];
        let instancia = unsafe { GetModuleHandleW(None) }.expect("sin instancia");
        let clase = WNDCLASSW {
            hInstance: instancia.into(),
            lpszClassName: PCWSTR(CLASE_VERDE.as_ptr()),
            lpfnWndProc: Some(procedimiento),
            hbrBackground: unsafe { CreateSolidBrush(COLORREF(0x0000_FF00)) },
            ..Default::default()
        };
        let _ = unsafe { RegisterClassW(&clase) };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                PCWSTR(CLASE_VERDE.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                region.x,
                region.y,
                region.width as i32,
                region.height as i32,
                None,
                None,
                Some(instancia.into()),
                None,
            )
        }
        .expect("no se ha podido abrir la tapadera");
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        hwnd.0 as isize
    }

    /// La prueba de que el arreglo sirve de algo: una ventana marcada como fuera de la
    /// captura NO sale en lo que graba winshotx, y la misma ventana sin marcar SI sale.
    ///
    /// Esta ignorada porque abre una ventana en la pantalla de quien la corra y graba
    /// medio segundo de esa esquina: no es para una maquina de integracion, es para
    /// comprobar a mano que Windows sigue cumpliendo.
    ///
    ///     cargo test --lib una_ventana_excluida -- --ignored --nocapture
    #[test]
    #[ignore = "abre una ventana en la pantalla y graba un trozo: se corre a mano"]
    fn una_ventana_excluida_no_sale_en_lo_que_se_graba() {
        let region = Rect { x: 0, y: 0, width: 160, height: 120 };
        let ventana = ventana_magenta(region);
        std::thread::sleep(Duration::from_millis(300));

        assert!(
            sale_el_magenta(region, ventana),
            "sin excluirla tiene que salir, o esta prueba no prueba nada"
        );

        assert!(
            crate::platform::window_style::fuera_de_la_captura_hwnd(ventana, true),
            "Windows no ha aceptado sacarla de la captura"
        );
        std::thread::sleep(Duration::from_millis(300));
        let sigue = sale_el_magenta(region, ventana);

        unsafe {
            let _ = DestroyWindow(HWND(ventana as *mut core::ffi::c_void));
        }
        assert!(!sigue, "la ventana excluida se sigue colando en la grabacion");
    }
}
