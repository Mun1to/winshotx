//! El menu de la bandeja, dibujado por winshotx en vez de por Windows.
//!
//! Un menu del sistema no sabe ensennar un interruptor, ni el atajo de cada cosa, ni decir
//! que version tiene puesta: son entradas de texto y poco mas. Esto es una ventana sin
//! marco, con las mismas piezas y los mismos colores que el resto de la aplicacion, y es
//! ademas lo que deja poner ahi el anillo de los ultimos segundos como lo que es, un
//! interruptor, y no como una entrada que enciende y otra que apaga.
//!
//! **Se crea la primera vez que se abre, no al arrancar**, y ese primer clic se nota: hay
//! que crear la ventana (~270 ms), cargar la interfaz, pedir el estado y medirse. Se probo
//! a prepararlo al arrancar y quedo mas rapido de verdad, pero el arreglo se llevo por
//! delante otras cosas (el arranque, y despues un cuelgue entero) y se retiro el 31 de
//! agosto de 2026. Lo aprendido, para quien lo vuelva a intentar, esta en las trampas 33 y
//! 34 de `docs/TRAMPAS.md`: una ventana escondida no arranca su interfaz, y nada que toque
//! una ventana puede hacerse desde otro hilo.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

use crate::error::Result;
use crate::windows_mgr::{monitor_de, MonitorSitio};

pub const TRAY_MENU_LABEL: &str = "tray-menu";

/// Ancho fijo, en pixeles logicos. El alto lo dice la propia interfaz al medirse.
const MENU_ANCHO: f64 = 284.0;
/// Con lo que nace, hasta que la interfaz mide lo suyo.
const MENU_ALTO_INICIAL: f64 = 300.0;
/// El aire contra el icono de la bandeja y contra los bordes de la pantalla.
const MARGEN: f64 = 8.0;

/// Donde estaba el icono la ultima vez que se abrio, para volver a colocarlo cuando la
/// interfaz diga cuanto mide: el menu crece hacia ARRIBA, no hacia abajo.
static ULTIMO_ANCLAJE: Mutex<Option<(i32, i32)>> = Mutex::new(None);

/// Cuando se escondio por perder el foco.
///
/// Volver a pulsar el icono de la bandeja tiene que CERRAR el menu, y ahi pasan dos cosas
/// seguidas: el clic le da el foco a la barra de tareas (y el menu se esconde solo) y
/// despues llega el evento del clic, que ya lo ve escondido y lo volveria a abrir. Con la
/// hora del ultimo escondite se distingue «estaba cerrado» de «lo acabo de cerrar yo».
static ESCONDIDO_EN: Mutex<Option<Instant>> = Mutex::new(None);

/// Lo que tarda el clic en llegar despues de que el menu pierda el foco. Medido a ojo y
/// generoso: mas de un cuarto de segundo entre las dos cosas ya es otro clic distinto.
const RECIEN_ESCONDIDO: Duration = Duration::from_millis(300);

/// Donde se pone el menu, dado el punto del icono de la bandeja.
///
/// **Encima del icono**, que es donde cabe: la barra de tareas suele estar abajo, asi que
/// un menu que creciera hacia abajo saldria de la pantalla. Si no hay sitio arriba (barra
/// de tareas arriba del todo), se pone debajo.
///
/// Va aparte y sin tocar ninguna ventana para poder probarla con la barra de tareas en
/// cualquier sitio y con monitores de coordenadas negativas, que es donde esto se rompe.
fn sitio_del_menu(
    anclaje: (i32, i32),
    ancho: i32,
    alto: i32,
    monitor: Option<MonitorSitio>,
) -> (i32, i32) {
    let (ax, ay) = anclaje;
    let escala = monitor.map(|m| m.escala).unwrap_or(1.0);
    let margen = (MARGEN * escala) as i32;

    let mut x = ax - ancho / 2;
    let mut y = ay - alto - margen;

    if let Some(m) = monitor {
        // Nunca contra cero: un monitor a la izquierda del principal tiene la x negativa,
        // y recortar a cero manda la ventana a la pantalla equivocada. Ya mordio tres
        // veces en esta misma aplicacion.
        let izquierda = m.x + margen;
        let derecha = (m.x + m.ancho - ancho - margen).max(izquierda);
        x = x.clamp(izquierda, derecha);

        let arriba = m.y + margen;
        if y < arriba {
            y = ay + margen;
        }
        let abajo = (m.y + m.alto - alto - margen).max(arriba);
        y = y.clamp(arriba, abajo);
    }
    (x, y)
}

/// La ventana del menu, creandola si es la primera vez.
fn ventana(app: &AppHandle) -> Result<tauri::WebviewWindow> {
    if let Some(existente) = app.get_webview_window(TRAY_MENU_LABEL) {
        return Ok(existente);
    }

    let window = WebviewWindowBuilder::new(
        app,
        TRAY_MENU_LABEL,
        WebviewUrl::App("tray-menu.html".into()),
    )
    .additional_browser_args(crate::windows_mgr::NAVEGADOR_ARGS)
    .title("winshotx")
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .shadow(true)
    .visible(false)
    .inner_size(MENU_ANCHO, MENU_ALTO_INICIAL)
    .build()?;

    // Las esquinas las redondea Windows, no un `border-radius` sobre una ventana
    // transparente: eso deja un halo negro alrededor. Trampa conocida de esta app.
    crate::platform::window_style::rounded_corners(&window);

    // Issue #15: un dock de terceros (MyDockFinder, con la bandeja en su barra de arriba)
    // enseñaba este menú escondido, en blanco, encima del editor. Tauri lo marca como
    // ventana de la app; fuera de esa lista, y con cerrojo: solo lo enseña `mostrar`.
    crate::platform::guardia::fuera_de_la_lista(&window, 0, true);

    // Aqui se esta en el hilo principal (lo llama el clic de la bandeja), asi que preguntar
    // por la ventana a pelo no espera a nadie.
    let menu = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    MENU.store(menu, Ordering::Relaxed);

    // Un menu se cierra al pulsar en otro sitio. El del sistema lo hacia solo; aqui se
    // hace al perder el foco, que es lo mismo visto desde dentro... casi siempre.
    let suyo = app.clone();
    window.on_window_event(move |evento| {
        if let tauri::WindowEvent::Focused(enfocado) = evento {
            ENFOCADO.store(*enfocado, Ordering::Relaxed);
            apuntar(&suyo, if *enfocado { "coge el foco" } else { "pierde el foco" });
            if !*enfocado {
                al_perder_el_foco(&suyo, menu);
            }
        }
    });
    Ok(window)
}

/// Lo que se espera antes de mirar quien ha quedado delante.
const MIRAR_DELANTE_EN: Duration = Duration::from_millis(80);

/// Si Tauri cree que el menu tiene el foco: lo ultimo que aviso.
static ENFOCADO: AtomicBool = AtomicBool::new(false);
/// Cuantas veces se le ha devuelto el foco desde que se abrio. Devolverlo hace que Tauri
/// avise otra vez de que lo pierde, y sin tope eso es un bucle: paso en la primera version
/// de este arreglo, que se lo devolvia cada 100 ms mientras el menu estaba abierto.
static DEVUELTOS: AtomicU32 = AtomicU32::new(0);
const DEVOLVER_COMO_MUCHO: u32 = 2;

/// El menu solo se cierra si de verdad hay otra ventana delante.
///
/// **La primera vez que se abria, se cerraba solo.** Munir, 11-nov-2026: *«la primera vez
/// que le das al icono con click derecho no funciona, solo la segunda; la primera como que
/// parpadea pero no se abre bien»*. La ventana se crea en ese primer clic, y cuando su
/// navegador termina de arrancar se lleva el foco del teclado a su pagina. Para Tauri eso
/// es «la ventana ha perdido el foco» aunque el menu siga delante, y lo cerraba. Las
/// siguientes veces la pagina ya esta cargada y el foco pasa dentro sin que Tauri lo cuente.
///
/// Por eso se mira un momento despues quien esta delante. Si sigue siendo el menu, no se
/// cierra. Lo normal (medido en `bandeja.log` el 11-nov) es que Tauri avise de que lo ha
/// recuperado 4 ms despues, y entonces no hay nada mas que hacer. Si no lo ha recuperado,
/// se le devuelve a la ventana, para que Tauri vuelva a avisar cuando lo pierda de verdad
/// (si no, un clic fuera ya no lo cerraria), y como mucho dos veces. Todo en el hilo
/// principal: el de fondo solo duerme (trampa 34).
fn al_perder_el_foco(app: &AppHandle, menu: isize) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(MIRAR_DELANTE_EN);
        let en_el_principal = app.clone();
        let _ = app.run_on_main_thread(move || {
            let app = en_el_principal;
            // La que pierde el foco es una que [`rehacer`] acaba de tirar: no es la de ahora,
            // y esconder la de ahora la dejaria sin salir.
            if menu != MENU.load(Ordering::Relaxed) {
                return;
            }
            let delante = crate::platform::window_style::primer_plano();
            if menu != 0 && delante == menu {
                if ENFOCADO.load(Ordering::Relaxed) {
                    apuntar(&app, "sigue delante y ya lo ha recuperado: se queda");
                } else if DEVUELTOS.fetch_add(1, Ordering::Relaxed) < DEVOLVER_COMO_MUCHO {
                    apuntar(&app, "sigue delante sin el foco: se le devuelve");
                    crate::platform::window_style::devolver_el_foco(menu);
                } else {
                    apuntar(&app, "sigue delante sin el foco, y ya se le devolvio: se queda");
                }
                return;
            }
            apuntar(
                &app,
                &format!(
                    "delante esta otra ({}): se esconde",
                    crate::platform::window_style::clase_de(delante)
                ),
            );
            *ESCONDIDO_EN.lock() = Some(Instant::now());
            esconder(&app);
        });
    });
}

/// Un registro corto de lo que le pasa al foco del menu, en el temporal de la app
/// (`bandeja.log`). Lo que falla aqui solo pasa con el clic de verdad de Munir en su
/// bandeja, y sin esto cada intento es adivinar. Nunca pasa de 32 KB.
fn apuntar(app: &AppHandle, que: &str) {
    use std::io::Write;
    use std::sync::OnceLock;

    static ARRANQUE: OnceLock<Instant> = OnceLock::new();
    let desde = ARRANQUE.get_or_init(Instant::now).elapsed().as_secs_f64();
    let ruta = app.state::<crate::state::AppState>().temp_root.join("bandeja.log");
    let lleno = std::fs::metadata(&ruta).is_ok_and(|m| m.len() > 32 * 1024);
    let archivo = std::fs::OpenOptions::new()
        .create(true)
        .append(!lleno)
        .write(true)
        .truncate(lleno)
        .open(&ruta);
    if let Ok(mut archivo) = archivo {
        let _ = writeln!(archivo, "{desde:9.3} s  {que}");
    }
}

/// La ventana del menu a pelo, guardada al crearla: se esconde desde los hilos de las
/// ordenes, y preguntarle a Tauri por ella desde ahi espera sin plazo (trampa 34).
static MENU: AtomicIsize = AtomicIsize::new(0);

pub fn esconder(app: &AppHandle) {
    ESPERANDO.store(0, Ordering::Relaxed);
    if let Some(window) = app.get_webview_window(TRAY_MENU_LABEL) {
        // Por Windows y por Tauri: si algo de fuera la hubiera ensennado, Tauri la seguiria
        // creyendo escondida y su `hide()` no haria nada (la miniatura de la 0.4.0).
        crate::platform::window_style::esconder_hwnd(MENU.load(Ordering::Relaxed));
        let _ = window.hide();
    }
}

/// Que el menu no salga dentro de lo que se esta grabando.
///
/// Se abre a menudo EN MEDIO de una grabacion, porque es desde donde se para, y hasta
/// ahora se colaba en el video: un panel cuadrado encima del tutorial, que ademas se queda
/// ahi hasta que pierde el foco. Munir, el 16 de septiembre de 2026: *«que cuando estes
/// grabando no se muestre el menu de settings cuadrado feo que se queda»*.
///
/// No se esconde, que desde el se para la grabacion: se le dice a Windows que no lo grabe.
/// Quien esta delante lo ve y lo pulsa igual; en el video sale lo que hay detras.
///
/// **Se vuelve a poner como estaba al terminar**, y no da igual: el temporizador de la
/// captura existe justo para fotografiar menus abiertos, y un menu que se quedara excluido
/// saldria en blanco en la foto sin que ningun error lo dijera.
pub fn fuera_de_la_grabacion(app: &AppHandle, grabando: bool) {
    if let Some(window) = app.get_webview_window(TRAY_MENU_LABEL) {
        crate::platform::window_style::fuera_de_la_captura(&window, grabando);
    }
}

fn esta_a_la_vista(app: &AppHandle) -> bool {
    app.get_webview_window(TRAY_MENU_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// Donde se aparca el menu mientras su pagina contesta: fuera de todas las pantallas, pero
/// visible, para que el navegador de dentro pinte (escondido, Windows lo duerme, trampa
/// 33). Es lo mismo que hacen los overlays.
const APARCADO: (i32, i32) = (-20_000, -20_000);

/// Lo que mide la pagina, como poco, cuando ya tiene el menu dentro. Antes de cargar mide
/// cero (la caja vacia) y la ventana se queda en 80.
const ALTO_CON_MENU: f64 = 120.0;

/// Cada apertura lleva su numero, y la pagina lo devuelve al medirse.
static APERTURA: AtomicU32 = AtomicU32::new(0);

/// La apertura que espera aparcada a que su pagina conteste; 0 si ninguna.
static ESPERANDO: AtomicU32 = AtomicU32::new(0);

/// Si la que espera es una ventana recien creada: su pagina todavia no sabe ningun numero
/// y contesta con 0.
static ESPERANDO_NUEVA: AtomicBool = AtomicBool::new(false);

/// Lo que se espera a una pagina recien creada antes de sacar el menu aunque no conteste:
/// tarda medio segundo en cargar, y sin menu no se puede ni salir de la app.
const RESCATE_NUEVA: Duration = Duration::from_millis(1500);

/// Lo que se espera a una pagina que ya estaba cargada. Contesta en unas decenas de ms; si
/// en un segundo no lo ha hecho, esta muerta o colgada y se rehace la ventana.
const RESCATE_REUTILIZADA: Duration = Duration::from_millis(1000);

/// La pantalla del icono de la bandeja.
fn pantalla_del_anclaje(anclaje: (i32, i32)) -> Option<MonitorSitio> {
    monitor_de(anclaje.0, anclaje.1)
}

/// Lo coloca sobre el icono de la bandeja y lo ensenna.
///
/// **No sale a la vista hasta que su pagina contesta a ESA apertura.** Se ensenna aparcado
/// fuera de las pantallas, se le avisa con el numero de la apertura, y es [`redimensionar`]
/// quien lo trae cuando la pagina contesta con ese numero.
///
/// - La primera vez, porque la ventana se crea en ese clic y su pagina tarda medio segundo:
///   ensennada ya, salia vacia con 300 px, encogia a 80, crecia con el menu y volvia a crecer
///   con las ultimas capturas, cuatro saltos delante de Munir («la primera como que
///   parpadea pero no se abre bien»).
/// - Las demas, desde la issue #15: un menu sin pintar salia a la vista en blanco. Una
///   pagina que ya estaba cargada contesta en unas decenas de ms; si no contesta, la
///   ventana se rehace con una pagina nueva ([`rehacer`]) en vez de sacar una en blanco.
fn mostrar(app: &AppHandle, anclaje: (i32, i32)) -> Result<()> {
    *ULTIMO_ANCLAJE.lock() = Some(anclaje);
    let nueva = app.get_webview_window(TRAY_MENU_LABEL).is_none();
    let menu = MENU.load(Ordering::Relaxed);
    if !nueva && crate::platform::window_style::minimizada(menu) {
        // `show()` la ensennaria minimizada, en ningun sitio que se vea.
        apuntar(app, "clic: algo de fuera lo ha minimizado, se rehace");
        rehacer(app);
        return Ok(());
    }
    apuntar(app, if nueva { "clic: se abre por primera vez" } else { "clic: se abre" });
    // Lo que la #15 no podia contar: si algo de fuera ha intentado ensennarlo escondido.
    let vetos = crate::platform::guardia::vetos(menu);
    if vetos > 0 {
        apuntar(app, &format!("algo de fuera ha intentado ensennarlo {vetos} veces: no ha podido"));
    }
    DEVUELTOS.store(0, Ordering::Relaxed);
    let window = ventana(app)?;
    let apertura = APERTURA.fetch_add(1, Ordering::Relaxed).wrapping_add(1).max(1);
    ESPERANDO_NUEVA.store(nueva, Ordering::Relaxed);
    ESPERANDO.store(apertura, Ordering::Relaxed);
    let _ = window.set_position(PhysicalPosition::new(APARCADO.0, APARCADO.1));
    // La ventana se reutiliza de una vez para otra, asi que esto se decide cada vez que se
    // abre y no al crearla: grabando, fuera del video; el resto del tiempo, normal.
    crate::platform::window_style::fuera_de_la_captura(
        &window,
        app.state::<crate::state::AppState>().is_recording(),
    );
    crate::platform::guardia::permitir(MENU.load(Ordering::Relaxed));
    window.show()?;
    window.set_focus()?;
    if !nueva {
        // Que vuelva a leer el estado (el anillo puede haberse encendido desde los ajustes)
        // y que conteste con este numero. Una recien creada lee el estado al cargar.
        let _ = window.emit_to(TRAY_MENU_LABEL, crate::EVENT_TRAY_MENU_OPENED, apertura);
    }
    apuntar(app, &format!("apertura {apertura}: aparcado hasta que la pagina conteste"));

    let suyo = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(if nueva { RESCATE_NUEVA } else { RESCATE_REUTILIZADA });
        let en_el_principal = suyo.clone();
        let _ = suyo.run_on_main_thread(move || {
            let app = en_el_principal;
            if ESPERANDO
                .compare_exchange(apertura, 0, Ordering::Relaxed, Ordering::Relaxed)
                .is_err()
            {
                return;
            }
            if nueva {
                apuntar(&app, "la pagina no se ha medido a tiempo: se saca igual");
                colocar(&app);
            } else {
                apuntar(&app, "la pagina no ha contestado: se rehace la ventana");
                rehacer(&app);
            }
        });
    });
    Ok(())
}

/// Lo que se espera entre vistazos a que Tauri suelte la ventana destruida.
const RELEVO: Duration = Duration::from_millis(50);
/// Y cuantos vistazos, como mucho, antes de abrir los ajustes en su lugar.
const RELEVOS: u32 = 20;

/// Tira la ventana del menu y abre otra en el mismo sitio, con una pagina nueva.
///
/// Para cuando la de ahora no sirve: su pagina no contesta (el proceso del navegador de
/// dentro se ha caido o colgado, y ni wry ni Tauri escuchan `ProcessFailed`: sin esto, el
/// menu se quedaria en blanco para toda la sesion) o algo de fuera la ha minimizado.
///
/// `destroy()` va por el bucle de la app, asi que la etiqueta no queda libre hasta una
/// vuelta despues: se mira cada poco, y la nueva se abre en cuanto la vieja se ha ido.
fn rehacer(app: &AppHandle) {
    let Some(anclaje) = *ULTIMO_ANCLAJE.lock() else {
        return;
    };
    ESPERANDO.store(0, Ordering::Relaxed);
    let vieja = MENU.swap(0, Ordering::Relaxed);
    if let Some(window) = app.get_webview_window(TRAY_MENU_LABEL) {
        crate::platform::window_style::esconder_hwnd(vieja);
        let _ = window.destroy();
    }
    let suyo = app.clone();
    std::thread::spawn(move || {
        for _ in 0..RELEVOS {
            std::thread::sleep(RELEVO);
            let (listo, hecho) = std::sync::mpsc::channel();
            let en_el_principal = suyo.clone();
            let _ = suyo.run_on_main_thread(move || {
                let app = en_el_principal;
                let terminado = match app.get_webview_window(TRAY_MENU_LABEL) {
                    // Sigue la vieja: todavia no se ha ido.
                    Some(w) if w.hwnd().map(|h| h.0 as isize).unwrap_or(0) == vieja => false,
                    // Ya hay otra: la ha abierto un clic mientras tanto.
                    Some(_) => true,
                    None => {
                        if let Err(error) = mostrar(&app, anclaje) {
                            eprintln!("[bandeja] no se ha podido rehacer el menú: {error}");
                            let _ = crate::windows_mgr::show_settings(&app);
                        }
                        true
                    }
                };
                let _ = listo.send(terminado);
            });
            if hecho.recv_timeout(Duration::from_secs(5)).unwrap_or(true) {
                return;
            }
        }
        // Sin menu no habria ni forma de salir: los ajustes tienen su boton de Salir.
        let en_el_principal = suyo.clone();
        let _ = suyo.run_on_main_thread(move || {
            apuntar(&en_el_principal, "la ventana vieja no se va: se abren los ajustes");
            let _ = crate::windows_mgr::show_settings(&en_el_principal);
        });
    });
}

/// Abre el menu, o lo cierra si ya estaba abierto.
pub fn alternar(app: &AppHandle, anclaje: (i32, i32)) {
    if esta_a_la_vista(app) {
        esconder(app);
        return;
    }
    // Y si acaba de esconderse por el foco, este clic es el que lo estaba cerrando.
    if let Some(cuando) = *ESCONDIDO_EN.lock() {
        if cuando.elapsed() < RECIEN_ESCONDIDO {
            return;
        }
    }
    if let Err(error) = mostrar(app, anclaje) {
        eprintln!("[bandeja] no se ha podido abrir el menú: {error}");
        // Sin menu no habria ni forma de salir de la aplicacion: los ajustes tienen su
        // boton de Salir, asi que ese es el plan B.
        let _ = crate::windows_mgr::show_settings(app);
    }
}

/// Le da a la ventana el alto que ha medido su contenido, y la trae a su sitio si estaba
/// aparcada esperando a que su pagina contestara a esta apertura (`apertura`, 0 si la
/// pagina acaba de cargar y aun no sabe ninguna).
///
/// Se vuelve a colocar despues de crecer: el menu esta anclado al icono de la bandeja por
/// su borde de ABAJO, asi que si solo cambiara el alto, crecería hacia abajo y se metería
/// debajo de la barra de tareas.
pub fn redimensionar(app: &AppHandle, alto: f64, apertura: u32) {
    let Some(window) = app.get_webview_window(TRAY_MENU_LABEL) else {
        return;
    };
    let escala = window.scale_factor().unwrap_or(1.0);
    let ancho = (MENU_ANCHO * escala) as u32;
    let alto_px = (alto * escala).round().max(80.0) as u32;
    let _ = window.set_size(PhysicalSize::new(ancho, alto_px));

    let esperando = ESPERANDO.load(Ordering::Relaxed);
    if esperando != 0 {
        if !es_la_que_espera(apertura, esperando, ESPERANDO_NUEVA.load(Ordering::Relaxed)) {
            apuntar(
                app,
                &format!("contesta la apertura {apertura} y se espera la {esperando}: sigue aparcado"),
            );
            return;
        }
        if alto < ALTO_CON_MENU {
            apuntar(app, &format!("la pagina mide {alto:.0}: aun vacia, sigue aparcado"));
            return;
        }
        if ESPERANDO
            .compare_exchange(esperando, 0, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        apuntar(
            app,
            &format!("la pagina contesta a la apertura {esperando} y mide {alto:.0}: se trae a su sitio"),
        );
        colocar(app);
        return;
    }
    // Ya en su sitio, crece o encoge (las ultimas capturas llegan despues) y se recoloca.
    // Escondido no se mueve: la proxima apertura lo aparca igual.
    if window.is_visible().unwrap_or(false) {
        colocar(app);
    }
}

/// Si una respuesta de la pagina es la de la apertura que espera aparcada.
///
/// Solo la de su numero: una medida que llega tarde de la vez anterior (el historial tarda
/// un cuarto de segundo) no dice nada de si la pagina esta viva ahora. El 0 es el de una
/// pagina recien cargada, que aun no ha recibido ningun numero, y solo vale si la ventana
/// que espera es nueva: en una reutilizada, un 0 es de antes de la primera reapertura.
fn es_la_que_espera(apertura: u32, esperando: u32, nueva: bool) -> bool {
    esperando != 0 && (apertura == esperando || (apertura == 0 && nueva))
}

/// Pone el menu encima del icono de la bandeja, con el alto que tenga ahora.
fn colocar(app: &AppHandle) {
    let Some(window) = app.get_webview_window(TRAY_MENU_LABEL) else {
        return;
    };
    // El mismo alto que se acaba de poner con `set_size`, que es el de dentro.
    let Ok(medida) = window.inner_size() else { return };
    let (ancho, alto_px) = (medida.width, medida.height);
    if let Some(anclaje) = *ULTIMO_ANCLAJE.lock() {
        let (x, y) = sitio_del_menu(
            anclaje,
            ancho as i32,
            alto_px as i32,
            pantalla_del_anclaje(anclaje),
        );
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, ancho: i32, alto: i32) -> MonitorSitio {
        MonitorSitio {
            x,
            y,
            ancho,
            alto,
            escala: 1.0,
        }
    }

    /// La barra de tareas suele estar abajo: el menu tiene que crecer hacia arriba.
    #[test]
    fn el_menu_sale_encima_del_icono() {
        let (x, y) = sitio_del_menu((1700, 1050), 268, 300, Some(monitor(0, 0, 1920, 1080)));
        assert_eq!(x, 1700 - 134, "no ha quedado centrado sobre el icono");
        assert_eq!(y, 1050 - 300 - 8);
    }

    /// Y con la barra de tareas arriba del todo, debajo, que es donde cabe.
    #[test]
    fn con_la_barra_arriba_sale_debajo() {
        let (_, y) = sitio_del_menu((960, 20), 268, 300, Some(monitor(0, 0, 1920, 1080)));
        assert_eq!(y, 28, "se ha salido por arriba de la pantalla");
    }

    /// El icono esta en la esquina derecha: el menu no puede salirse por ahi.
    #[test]
    fn no_se_sale_por_la_derecha() {
        let (x, _) = sitio_del_menu((1915, 1050), 268, 300, Some(monitor(0, 0, 1920, 1080)));
        assert_eq!(x, 1920 - 268 - 8);
    }

    /// Y en un monitor a la izquierda del principal, con coordenadas NEGATIVAS, se queda
    /// en el suyo. Recortar contra cero lo mandaria a la pantalla principal.
    #[test]
    fn se_queda_en_el_monitor_de_coordenadas_negativas() {
        let (x, y) = sitio_del_menu(
            (-1800, 1050),
            268,
            300,
            Some(monitor(-1920, 0, 1920, 1080)),
        );
        assert!((-1912..0).contains(&x), "se ha ido a la pantalla principal: {x}");
        assert_eq!(y, 742);
    }

    /// Issue #15: el menu solo sale a la vista cuando su pagina contesta a ESA apertura.
    #[test]
    fn solo_vale_la_respuesta_de_esta_apertura() {
        assert!(es_la_que_espera(7, 7, false));
        assert!(!es_la_que_espera(6, 7, false), "una respuesta vieja lo ha sacado");
        assert!(!es_la_que_espera(0, 7, false), "un 0 en una ventana reutilizada lo ha sacado");
        assert!(es_la_que_espera(0, 1, true), "la pagina recien cargada contesta con 0");
        assert!(!es_la_que_espera(7, 0, false), "sin nadie esperando no hay nada que sacar");
    }
}
