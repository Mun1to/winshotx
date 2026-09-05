use std::sync::atomic::{AtomicU32, Ordering};

use serde::Serialize;

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder,
};

use parking_lot::Mutex;

use crate::capture::{self, MonitorInfo, Rect};
use crate::error::Result;
use crate::state::{AppState, CandadoCaptura};

/// Las ventanas del overlay que esperan a ensennarse, hasta que cada una avise de que tiene
/// su imagen pintada (`overlay_listo`).
///
/// Antes se ensennaban nada mas congelar, y durante los 300-400 ms que tardaba en llegarles
/// la imagen el usuario veia una pantalla oscura con «Preparando la captura». Munir, el 5 de
/// septiembre de 2026: *«lo del principio de carga queda muy mal»*. Ahora la ventana sigue
/// aparcada fuera de las pantallas mientras carga, y lo que aparece, aparece ya pintado.
///
/// La generacion distingue una captura de la siguiente: un aviso que llegue tarde, de una
/// captura que ya se cancelo, no puede ensennar una ventana que nadie ha pedido.
struct Pendientes {
    generacion: u64,
    ventanas: Vec<(u32, String, MonitorInfo)>,
    cursor: Option<(i32, i32)>,
    con_foco: bool,
}

static PENDIENTES: Mutex<Pendientes> = Mutex::new(Pendientes {
    generacion: 0,
    ventanas: Vec::new(),
    cursor: None,
    con_foco: false,
});

/// Cuanto se espera a que un overlay avise de que esta pintado antes de ensennarlo igual.
/// Si el frontend no llega a avisar (porque ha fallado y esta ensennando su error), el
/// usuario tiene que poder ver ese error y salir con Escape, no quedarse sin nada.
const ESPERA_MAXIMA: std::time::Duration = std::time::Duration::from_millis(900);

/// El numero de la captura en curso: viaja en el payload y vuelve en `overlay_listo`.
pub fn generacion_actual() -> u64 {
    PENDIENTES.lock().generacion
}

pub const OVERLAY_PREFIX: &str = "overlay-";

/// Los argumentos con los que arranca el navegador de TODAS las ventanas.
///
/// Los tres primeros son los que pone wry si no se le dice nada (quitan el menu flotante de
/// Edge y su filtro de descargas). Los otros tres le prohiben a Chromium tratar como
/// «de fondo» a una ventana que no se ve: los overlays viven aparcados fuera de las
/// pantallas entre captura y captura, y con la ventana de fondo el navegador tardaba unos
/// 300 ms en enterarse del aviso de que hay captura nueva (trampa 33).
///
/// **Tienen que ser los mismos en todas las ventanas**, incluida la `main` de
/// `tauri.conf.json`: WebView2 comparte un solo proceso de navegador y lo configura la
/// primera ventana; una segunda que pida otros argumentos se queda sin webview y sale en
/// blanco. Eso es lo que paso la primera vez que se intento, cuando solo se pusieron en el
/// overlay.
pub const NAVEGADOR_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-renderer-backgrounding --disable-backgrounding-occluded-windows --disable-background-timer-throttling";

static OVERLAY_SEQUENCE: AtomicU32 = AtomicU32::new(0);
pub const RECORDER_PREFIX: &str = "recorder-";
/// Las capturas ancladas. Puede haber varias a la vez, cada una con su numero.
pub const PIN_PREFIX: &str = "pin-";
pub const EDITOR_LABEL: &str = "editor";
/// La ventanita de la cuenta atras del temporizador. Solo hay una, en la pantalla donde
/// este el raton, y como los overlays se esconde en vez de cerrarse.
pub const COUNTDOWN_LABEL: &str = "countdown";

/// Lado de la ventanita de la cuenta atras, en pixeles logicos: en una pantalla al 150 %
/// tiene que verse igual de grande, no mas pequenna.
const CUENTA_LADO: f64 = 132.0;

/// Lo que se le da al escritorio para repintar el hueco de la cuenta atras antes de
/// congelar. Sin esto la captura se lleva la ventanita dentro, y en el centro de la
/// pantalla eso no se le escapa a nadie.
const MARGEN_REPINTADO: std::time::Duration = std::time::Duration::from_millis(120);

/// Con que intencion se ha abierto el overlay. Lo decide el atajo que se pulso, y
/// es lo que deja que el modo instantaneo copie al soltar sin cargarse la grabacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayIntent {
    Capture,
    Record,
}

/// Punto de entrada del atajo de captura: prepara la pantalla y abre la seleccion.
///
/// Entre pulsar la tecla y congelar puede haber un paso previo, y ese paso es lo unico
/// que distingue esta funcion de `congelar_y_abrir`. Hay dos cosas que **tienen que pasar
/// antes** de la foto y no despues, asi que ninguna de las dos puede ser una tecla del
/// overlay como el resto: el temporizador (que existe para que la pantalla siga como
/// estaba cuando se pulso el atajo) y esconder los iconos del escritorio.
pub fn open_overlays(app: &AppHandle, intent: OverlayIntent) -> Result<()> {
    let state = app.state::<AppState>();
    if state.is_recording() {
        return Ok(());
    }
    // Si el atajo se pulsa dos veces muy seguidas, la segunda pulsacion se ignora en vez
    // de arrancar otra captura por encima de la que ya esta en marcha: la primera va a
    // dejar el overlay abierto igual, y dejar que dos disparos capturen pantalla a la vez
    // solo consigue que las dos vayan mas lentas. Con el temporizador puesto, el candado
    // cubre tambien la espera: pulsar otra vez durante la cuenta atras no la reinicia.
    let Some(candado) = state.intentar_capturar() else {
        return Ok(());
    };
    crate::crono::marca("atajo");
    *state.intent.write() = intent;

    // La espera es solo del atajo de capturar, aunque el overlay sea el mismo. Al grabar,
    // la pantalla congelada es un fondo para elegir la region y no la foto que se lleva
    // nadie: hacer esperar cinco segundos ahi solo seria una espera.
    let segundos = state.settings.read().capture_delay_seconds;
    if segundos == 0 || intent == OverlayIntent::Record {
        return congelar_y_abrir(app, candado);
    }

    // **El temporizador no puede dormir aqui.** Esta funcion la llama el manejador del
    // atajo global, y ese manejador corre dentro del bucle de eventos del hilo principal:
    // el plugin crea una ventana oculta que recibe el `WM_HOTKEY` y una ventana atiende
    // sus mensajes en el hilo que la creo. Un `sleep` aqui congela la aplicacion entera
    // durante los tres o cinco segundos, incluida la cuenta atras que se acaba de
    // ensennar, que se quedaria clavada en el primer numero.
    mostrar_cuenta_atras(app, segundos);
    let espera = app.clone();
    std::thread::spawn(move || {
        // La cuenta atras se esconde un pelin ANTES del final, no en el mismo instante en
        // que se congela. Esconder una ventana no borra su hueco de la pantalla: hay que
        // darle al escritorio el tiempo de repintar lo que habia debajo, o la captura se
        // lleva la ventanita dentro. Ese margen sale de la cuenta atras, no se suma
        // detras, asi que la espera total sigue siendo la que pidio el usuario y el ultimo
        // numero se ve 880 ms en vez de 1000: no se nota.
        let total = std::time::Duration::from_secs(u64::from(segundos));
        std::thread::sleep(total.saturating_sub(MARGEN_REPINTADO));

        // Volver al hilo principal no es opcional: manejar ventanas desde otro hilo se
        // queda esperando al bucle de eventos.
        let para_esconder = espera.clone();
        let _ = espera.run_on_main_thread(move || esconder_cuenta_atras(&para_esconder));

        std::thread::sleep(MARGEN_REPINTADO);
        let seguir = espera.clone();
        let _ = espera.run_on_main_thread(move || {
            if let Err(error) = congelar_y_abrir(&seguir, candado) {
                eprintln!("no se ha podido abrir el overlay: {error}");
            }
        });
    });
    Ok(())
}

/// Congela la pantalla y abre un overlay por monitor.
/// Congelar primero es lo que hace que la seleccion sea estable y precisa al pixel.
///
/// Las ventanas overlay se reutilizan entre capturas en vez de crearse de cero cada vez:
/// construir un WebView2 nuevo costaba entre 200ms y 800ms por pantalla (variable, y el
/// peor coste con tres monitores), y ese tiempo desaparece del todo si la ventana ya
/// existe, oculta, de la vez anterior. `close_overlays` las esconde en vez de cerrarlas
/// para que quede algo que reutilizar. Cuando se reutiliza una ventana no llega un
/// remontaje de React que dispare su arranque solo, asi que se le avisa con
/// `EVENT_OVERLAY_SHOW` para que recargue la captura nueva.
///
/// El candado entra por parametro y se suelta al terminar: con el temporizador puesto se
/// cogio hace tres segundos, en otro hilo, y no se puede volver a pedir aqui.
fn congelar_y_abrir(app: &AppHandle, candado: CandadoCaptura) -> Result<()> {
    let state = app.state::<AppState>();

    // El anillo de los ultimos segundos se aparta mientras dura esto, y vuelve solo cuando
    // el guardian se suelta al final del hilo de abajo. Se come el 86% de un nucleo a 60
    // fps y alto nativo, y congelar tres pantallas compitiendo con eso es lo que hacia que
    // el atajo se notara. Ver `replay::apartar`, que explica por que esto no deja un
    // agujero en lo grabado.
    let anillo = crate::replay::apartar(app);

    // Los iconos se esconden solo lo que dura el disparo, no toda la seleccion: el
    // overlay tapa el escritorio de todas formas, asi que tenerlos escondidos mas rato no
    // se ve en la imagen y si aumenta la posibilidad de dejarselos escondidos a alguien.
    // El guardian los devuelve tambien si la captura falla a medias.
    let esconder_iconos = state.settings.read().hide_desktop_icons;
    let iconos = if esconder_iconos {
        crate::platform::desktop_icons::esconder()
    } else {
        None
    };

    // Las pantallas que hay, sin fotografiarlas todavia: hace falta saber cuantas son y
    // cual tiene el raton antes de decidir por cual se empieza.
    let monitors = capture::monitors()?;
    if monitors.is_empty() {
        return Err(crate::error::AppError::Msg("no se ha detectado ningún monitor".into()));
    }
    let cursor = cursor_position();

    // Una captura nueva: lo que quedara pendiente de la anterior ya no cuenta.
    let generacion = {
        let mut pendientes = PENDIENTES.lock();
        pendientes.generacion += 1;
        pendientes.ventanas.clear();
        pendientes.cursor = cursor;
        pendientes.con_foco = false;
        pendientes.generacion
    };

    // Si algun monitor de la ultima vez ya no existe (se desconecto una pantalla), su
    // ventana se queda escondida en el pool: no molesta a nadie ahi.
    let etiquetas_actuales: std::collections::HashSet<String> = monitors
        .iter()
        .map(|m| format!("{OVERLAY_PREFIX}{}", m.id))
        .collect();
    for (label, window) in app.webview_windows() {
        if label.starts_with(OVERLAY_PREFIX) && !etiquetas_actuales.contains(&label) {
            let _ = window.hide();
        }
    }

    // **Primero la pantalla donde esta el raton, sola.** Es la que el usuario mira. Se
    // congela, se le manda su imagen y se sigue con las demas: asi aparece bastante antes
    // que si se esperara a tener las tres, y las otras dos llegan justo detras.
    let primera = cursor
        .and_then(|(x, y)| monitors.iter().position(|m| m.contains(x, y)))
        .or_else(|| monitors.iter().position(|m| m.is_primary))
        .unwrap_or(0);
    let resto: Vec<usize> = (0..monitors.len()).filter(|&i| i != primera).collect();
    state.freezes.write().clear();

    // **Y todo eso en un hilo aparte, no en el principal.** El aviso a cada ventana viaja
    // por el bucle de mensajes del hilo principal: si ese hilo se queda esperando a que se
    // congelen las otras dos pantallas, el aviso de la primera se queda en la cola con el,
    // y la prioridad no sirve de nada. Medido el 5 de septiembre de 2026: congelada a los
    // 53 ms, y su navegador enterandose a los 131. Aqui el principal solo hace lo que solo
    // el puede hacer (hablar con las ventanas), unas decimas de milisegundo cada vez.
    let app = app.clone();
    std::thread::spawn(move || {
        // Los tres guardianes viven hasta que las dos tandas esten congeladas: el candado
        // de «hay una captura en marcha», el anillo apartado y los iconos escondidos.
        let candado = candado;
        let anillo = anillo;
        let iconos = iconos;
        let primera_id = monitors[primera].id;
        for (numero, tanda) in [vec![primera], resto].into_iter().enumerate() {
            if tanda.is_empty() {
                continue;
            }
            let freezes = match capture::freeze_monitors(&tanda) {
                Ok(freezes) => freezes,
                Err(error) => {
                    eprintln!("no se ha podido congelar la pantalla: {error}");
                    continue;
                }
            };
            crate::crono::marca(&format!("congelado-{}", tanda.len()));
            let monitores: Vec<MonitorInfo> = freezes.iter().map(|f| f.monitor.clone()).collect();
            app.state::<AppState>().freezes.write().extend(freezes);
            if numero == 1 {
                // Las demas no se mandan hasta que la primera este A LA VISTA. Servirles
                // sus PNG ocupa el hilo principal, que es el mismo que tiene que ensennar
                // la primera: medido, la retrasaba de 105 a 280 ms una de cada tres veces.
                // Capturarlas si se ha hecho ya, en paralelo con la primera; lo que espera
                // es solo el aviso.
                esperar_a_que_se_vea(primera_id, generacion);
            }
            let dentro = app.clone();
            let _ = app.run_on_main_thread(move || {
                let state = dentro.state::<AppState>();
                for monitor in &monitores {
                    if let Err(error) = preparar_overlay(&dentro, &state, monitor) {
                        eprintln!("no se ha podido preparar el overlay: {error}");
                    }
                }
            });
        }
        drop(iconos);
        drop(anillo);
        drop(candado);

        // Si alguna no avisa a tiempo, se ensenna igual: mejor ver la pantalla de error del
        // overlay, con su Escape, que quedarse sin nada tras pulsar el atajo.
        std::thread::sleep(ESPERA_MAXIMA);
        let dentro = app.clone();
        let _ = app.run_on_main_thread(move || mostrar_las_que_falten(&dentro, generacion));
    });

    Ok(())
}

/// Le manda a la ventana de ese monitor su captura y la deja apuntada como pendiente de
/// ensennar. La ventana se crea si no existia.
fn preparar_overlay(app: &AppHandle, state: &AppState, monitor: &MonitorInfo) -> Result<()> {
    let label = format!("{OVERLAY_PREFIX}{}", monitor.id);

    let window = if let Some(existente) = app.get_webview_window(&label) {
        // El payload va ya construido en el propio evento: sin esto, el frontend
        // tendria que pedirlo aparte con un invoke (overlay_bootstrap) despues de
        // enterarse, y esa vuelta de IPC completa se ahorra entera.
        // `emit` manda el evento a TODAS las ventanas, no solo a `existente`: con eso
        // las tres pantallas recibian el payload de la ultima que se procesaba en
        // este bucle y se pisaban entre si. `emit_to` con la etiqueta de esta ventana
        // es lo que lo manda solo a ella.
        if let Ok(payload) = crate::commands::build_overlay_payload(state, monitor.id) {
            let _ = existente.emit_to(label.as_str(), crate::EVENT_OVERLAY_SHOW, payload);
        }
        crate::crono::marca(&format!("emitido-{}", monitor.id));
        existente
    } else {
        // Recien creada arranca sola y pide su payload con `overlay_bootstrap`.
        build_overlay_window(app, &label, monitor.id)?
    };

    // El tamanno se pone ya, aparcada, para que el navegador se acomode a el mientras
    // carga la imagen. La posicion NO: eso es ensennarla, y se ensenna cuando avise de
    // que esta pintada (`overlay_listo`), o cuando se agote la espera del rescate.
    window.set_size(PhysicalSize::new(monitor.width, monitor.height))?;
    PENDIENTES
        .lock()
        .ventanas
        .push((monitor.id, label, monitor.clone()));
    Ok(())
}

/// Espera a que el overlay de ese monitor haya salido de la lista de pendientes (o sea, a
/// que se haya ensennado), con un tope por si no llega a avisar: la espera es para ordenar,
/// no para bloquear.
fn esperar_a_que_se_vea(monitor_id: u32, generacion: u64) {
    let tope = std::time::Instant::now() + std::time::Duration::from_millis(250);
    while std::time::Instant::now() < tope {
        {
            let pendientes = PENDIENTES.lock();
            if pendientes.generacion != generacion
                || !pendientes.ventanas.iter().any(|(id, _, _)| *id == monitor_id)
            {
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
}

/// El overlay de ese monitor ya esta pintado: se ensenna, y si es el de la pantalla del
/// raton se le da el foco.
///
/// Llega desde un comando (otro hilo), y las ventanas se manejan en el principal.
pub fn overlay_listo(app: &AppHandle, monitor_id: u32, generacion: u64) {
    let dentro = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some((label, monitor, con_foco)) = sacar_pendiente(monitor_id, generacion) else {
            // De una captura anterior, o ya ensennada por el rescate: no hay nada que hacer.
            return;
        };
        ensennar_overlay(&dentro, &label, &monitor, con_foco);
        crate::crono::marca(&format!("mostrada-{monitor_id}"));
    });
}

/// Saca de la lista de pendientes la ventana de ese monitor, si es de esta captura, y
/// decide si le toca el foco: a la pantalla del raton, o a la ultima que quede si el raton
/// no esta en ninguna.
fn sacar_pendiente(monitor_id: u32, generacion: u64) -> Option<(String, MonitorInfo, bool)> {
    let mut pendientes = PENDIENTES.lock();
    if pendientes.generacion != generacion {
        return None;
    }
    let posicion = pendientes.ventanas.iter().position(|(id, _, _)| *id == monitor_id)?;
    let (_, label, monitor) = pendientes.ventanas.remove(posicion);
    let cursor = pendientes.cursor;
    let es_la_del_raton = cursor.is_some_and(|(x, y)| monitor.contains(x, y));
    let otra_lo_tendra = pendientes
        .ventanas
        .iter()
        .any(|(_, _, m)| cursor.is_some_and(|(x, y)| m.contains(x, y)));
    let con_foco = !pendientes.con_foco && (es_la_del_raton || !otra_lo_tendra);
    if con_foco {
        pendientes.con_foco = true;
    }
    Some((label, monitor, con_foco))
}

/// La coloca en su pantalla y la ensenna. Siempre desde el hilo principal.
fn ensennar_overlay(app: &AppHandle, label: &str, monitor: &MonitorInfo, con_foco: bool) {
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    // Posicion en pixeles fisicos: el escalado por DPI no debe tocarla. El tamanno ya se
    // puso al preparar la ventana, aparcada. Colocar y ensennar van en UNA llamada a
    // Windows (`SetWindowPos` con `SWP_SHOWWINDOW`) en vez de dos; el `show()` de despues
    // solo pone al dia lo que Tauri cree de la ventana, y no cuesta nada si ya esta visible.
    #[cfg(windows)]
    let colocada = crate::platform::window_style::colocar_y_ensennar(&window, monitor.x, monitor.y);
    #[cfg(not(windows))]
    let colocada = false;
    if !colocada {
        let _ = window.set_position(PhysicalPosition::new(monitor.x, monitor.y));
    }
    let _ = window.show();
    // Si el aparcadero no pudo darle el DPI de su monitor y Windows la ha reescalado al
    // traerla, el tamanno se corrige aqui. Normalmente no pasa y esta comprobacion es una
    // lectura.
    if window
        .inner_size()
        .is_ok_and(|s| s.width != monitor.width || s.height != monitor.height)
    {
        let _ = window.set_size(PhysicalSize::new(monitor.width, monitor.height));
    }
    if con_foco {
        // Sin esto, el primer clic del usuario se lo come Windows para activar
        // la ventana y la seleccion nunca empieza.
        let _ = window.set_focus();
        #[cfg(windows)]
        force_foreground(&window);
    }
}

/// El rescate: pasado el plazo, se ensenna lo que no haya avisado, sea lo que sea.
fn mostrar_las_que_falten(app: &AppHandle, generacion: u64) {
    let que_faltan: Vec<(u32, String, MonitorInfo, bool)> = {
        let mut pendientes = PENDIENTES.lock();
        if pendientes.generacion != generacion {
            return;
        }
        let cursor = pendientes.cursor;
        let mut lista = Vec::new();
        for (id, label, monitor) in std::mem::take(&mut pendientes.ventanas) {
            let con_foco = !pendientes.con_foco
                && cursor.is_some_and(|(x, y)| monitor.contains(x, y));
            if con_foco {
                pendientes.con_foco = true;
            }
            lista.push((id, label, monitor, con_foco));
        }
        // Si el raton no estaba en ninguna, el foco a la primera que quede.
        if !pendientes.con_foco {
            if let Some(primera) = lista.first_mut() {
                primera.3 = true;
                pendientes.con_foco = true;
            }
        }
        lista
    };
    for (id, label, monitor, con_foco) in que_faltan {
        ensennar_overlay(app, &label, &monitor, con_foco);
        crate::crono::marca(&format!("mostrada-{id}-tarde"));
    }
}

/// Ensenna los segundos que faltan, abajo a la derecha de la pantalla donde esta el raton.
///
/// La ventana se crea la primera vez que se usa y se queda escondida para las siguientes,
/// igual que los overlays. No se precrea al arrancar como ellos porque casi nadie usa el
/// temporizador, y las cifras de memoria en reposo estan publicadas: la primera cuenta
/// atras del dia paga el WebView2 y sale unas decimas tarde, las demas salen al instante.
fn mostrar_cuenta_atras(app: &AppHandle, segundos: u32) {
    let window = match app.get_webview_window(COUNTDOWN_LABEL) {
        Some(existente) => {
            // Trampa 8: el destino de un evento lo deciden los dos lados. Aqui solo hay
            // una ventana escuchando, pero se escribe dirigido igual que en el overlay
            // para que nadie copie de aqui la version que da problemas con varias.
            let _ = existente.emit_to(COUNTDOWN_LABEL, crate::EVENT_COUNTDOWN, segundos);
            existente
        }
        None => {
            // La primera vez el numero viaja en la URL: la pagina todavia no esta cargada
            // cuando se emitiria el evento, asi que no habria nadie escuchandolo.
            let url = WebviewUrl::App(format!("cuenta.html?segundos={segundos}").into());
            let construida = WebviewWindowBuilder::new(app, COUNTDOWN_LABEL, url)
                .additional_browser_args(NAVEGADOR_ARGS)
                .title("winshotx")
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .shadow(true)
                .focused(false)
                .visible(false)
                .inner_size(CUENTA_LADO, CUENTA_LADO)
                .build();
            let Ok(nueva) = construida else {
                eprintln!("no se ha podido crear la cuenta atras");
                return;
            };
            crate::platform::window_style::rounded_corners(&nueva);
            // Sin esto la cuenta atras cierra el menu que se venia a fotografiar.
            crate::platform::window_style::never_focus(&nueva);
            nueva
        }
    };

    colocar_cuenta_atras(&window);
    let _ = window.show();
}

/// La pone en el centro del monitor donde esta el cursor, que es el que el usuario esta
/// mirando. Se recoloca en cada disparo porque el raton puede estar en otra pantalla que
/// la vez anterior, y porque cada pantalla puede tener su propio escalado.
///
/// En el centro y no en una esquina porque es donde se mira sin buscarla: con tres
/// pantallas, un numero en la esquina de una de ellas se pierde. Tapa lo que haya debajo
/// durante la cuenta, pero eso no llega a la captura: se esconde antes de congelar.
fn colocar_cuenta_atras(window: &tauri::WebviewWindow) {
    colocar_ventanita(window, None)
}

/// Y la misma colocacion, pero sobre la pantalla que se diga en vez de la del raton.
fn colocar_ventanita(window: &tauri::WebviewWindow, en: Option<u32>) {
    let Ok(monitores) = xcap::Monitor::all() else {
        return;
    };
    // Con una pantalla pedida manda esa; si no, la del raton.
    if let Some(indice) = en {
        if let Some(monitor) = monitores.get(indice as usize) {
            poner_en(window, monitor);
            return;
        }
    }
    let cursor = cursor_position();
    let dentro = |m: &xcap::Monitor, x: i32, y: i32| -> bool {
        let (Ok(mx), Ok(my), Ok(mw), Ok(mh)) = (m.x(), m.y(), m.width(), m.height()) else {
            return false;
        };
        x >= mx && x < mx + mw as i32 && y >= my && y < my + mh as i32
    };
    let elegido = cursor
        .and_then(|(x, y)| monitores.iter().find(|m| dentro(m, x, y)))
        .or_else(|| monitores.iter().find(|m| m.is_primary().unwrap_or(false)))
        .or_else(|| monitores.first());

    let Some(monitor) = elegido else { return };
    poner_en(window, monitor);
}

/// La deja centrada en ese monitor, con el tamanno que le toca por su escalado.
fn poner_en(window: &tauri::WebviewWindow, monitor: &xcap::Monitor) {
    let (Ok(x), Ok(y), Ok(width), Ok(height)) = (
        monitor.x(),
        monitor.y(),
        monitor.width(),
        monitor.height(),
    ) else {
        return;
    };

    // El escalado que cuenta es el de la pantalla de destino, no el de donde este la
    // ventana ahora mismo: en un equipo con una pantalla al 150 % y otra al 100 %,
    // preguntarselo a la ventana da el numero de la pantalla equivocada.
    let escala = f64::from(monitor.scale_factor().unwrap_or(1.0));
    let lado = (CUENTA_LADO * escala) as u32;

    let _ = window.set_size(PhysicalSize::new(lado, lado));
    let _ = window.set_position(PhysicalPosition::new(
        x + (width as i32 - lado as i32) / 2,
        y + (height as i32 - lado as i32) / 2,
    ));
}

/// Ensenna el numero de una pantalla en su centro, un par de segundos.
///
/// Elegir «la pantalla 2» en los ajustes no dice nada si no se sabe cual es la 2. Esto lo
/// dice de la unica forma que no admite duda: ensennando el numero **en esa pantalla**.
///
/// Reutiliza la ventanita de la cuenta atras, que ya sabe ponerse centrada en un monitor
/// concreto y no coge el foco. Va con un numero fijo, no con una cuenta.
pub fn mostrar_numero_de_pantalla(app: &AppHandle, indice: u32) {
    let Some(window) = ventanita_de_numero(app, indice) else {
        return;
    };
    let _ = window.emit_to(COUNTDOWN_LABEL, crate::EVENT_SCREEN_NUMBER, indice + 1);
    colocar_ventanita(&window, Some(indice));
    let _ = window.show();

    // Dos segundos: lo que se tarda en mirar y reconocer la pantalla. Se esconde desde otro
    // hilo para no dejar los ajustes esperando a que pase.
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2000));
        if let Some(window) = app.get_webview_window(COUNTDOWN_LABEL) {
            let _ = window.hide();
            // Se deja en cero, que la pagina dibuja como la camara: si la proxima cuenta
            // atras se ve un fotograma con lo viejo, que no sea un numero de pantalla.
            let _ = window.emit_to(COUNTDOWN_LABEL, crate::EVENT_SCREEN_NUMBER, 0u32);
        }
    });
}

/// La ventanita, creada si todavia no existe.
fn ventanita_de_numero(app: &AppHandle, indice: u32) -> Option<tauri::WebviewWindow> {
    if let Some(existente) = app.get_webview_window(COUNTDOWN_LABEL) {
        return Some(existente);
    }
    let url = WebviewUrl::App(format!("cuenta.html?pantalla={}", indice + 1).into());
    let construida = WebviewWindowBuilder::new(app, COUNTDOWN_LABEL, url)
        .additional_browser_args(NAVEGADOR_ARGS)
        .title("winshotx")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(true)
        .focused(false)
        .visible(false)
        .inner_size(CUENTA_LADO, CUENTA_LADO)
        .build()
        .ok()?;
    crate::platform::window_style::rounded_corners(&construida);
    crate::platform::window_style::never_focus(&construida);
    Some(construida)
}

/// La esconde y la deja en cero, que la pagina dibuja como el icono de la camara y no
/// como un numero: asi, si la proxima vez se ve un fotograma con el valor viejo, lo que
/// se ve es "ya voy" y no un tres que ya no significa nada.
fn esconder_cuenta_atras(app: &AppHandle) {
    let Some(window) = app.get_webview_window(COUNTDOWN_LABEL) else {
        return;
    };
    let _ = window.hide();
    let _ = window.emit_to(COUNTDOWN_LABEL, crate::EVENT_COUNTDOWN, 0u32);
}

fn build_overlay_window(app: &AppHandle, label: &str, monitor_id: u32) -> Result<tauri::WebviewWindow> {
    let url = WebviewUrl::App(format!("overlay.html?monitor={monitor_id}").into());
    Ok(WebviewWindowBuilder::new(app, label, url)
        .additional_browser_args(NAVEGADOR_ARGS)
        .title("winshotx")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .visible(false)
        .build()?)
}

/// Crea de antemano las ventanas overlay de los monitores actuales, aparcadas, sin esperar
/// a la primera captura del dia. Se llama una vez al arrancar la app: en ese momento no
/// hay ninguna ventana visible que el usuario este esperando (winshotx arranca en la
/// bandeja), asi que el coste de montar cada WebView2 no lo nota nadie. Sin esto, la
/// primera vez que se pulsa el atajo tras abrir la app tiene que pagar ese coste igual
/// que antes del pool.
pub fn precrear_overlays(app: &AppHandle) {
    let Ok(monitors) = capture::monitors() else {
        return;
    };
    for monitor in &monitors {
        let label = format!("{OVERLAY_PREFIX}{}", monitor.id);
        if app.get_webview_window(&label).is_some() {
            continue;
        }
        let Ok(window) = build_overlay_window(app, &label, monitor.id) else {
            continue;
        };
        // Aparcada fuera de las pantallas y ENSENNADA, no escondida.
        //
        // Una ventana que no se ha ensennado nunca no tiene interfaz montada, asi que la
        // primera captura del dia pagaba entero el arranque de su navegador. Aparcada
        // fuera se monta ahora, mientras nadie espera nada (la app vive en la bandeja), y
        // el primer atajo la encuentra despierta. Es lo mismo que hace `close_overlays`
        // entre captura y captura, y por la misma razon. Primero el sitio y despues el
        // tamanno, para que el tamanno se aplique ya con el DPI de su monitor.
        let _ = window.set_position(aparcadero_de(monitor, &monitors));
        let _ = window.set_size(PhysicalSize::new(monitor.width, monitor.height));
        let _ = window.show();
    }
}

#[cfg(windows)]
fn cursor_position() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    Some((point.x, point.y))
}

#[cfg(not(windows))]
fn cursor_position() -> Option<(i32, i32)> {
    None
}

/// Esconde el overlay en vez de cerrarlo: la ventana se reutiliza en la siguiente
/// captura, ver `open_overlays`.
pub fn close_overlays(app: &AppHandle) {
    // Lo que estuviera esperando a ensennarse ya no se ensenna: la captura se ha cerrado.
    {
        let mut pendientes = PENDIENTES.lock();
        pendientes.generacion += 1;
        pendientes.ventanas.clear();
    }
    let monitores = capture::monitors().unwrap_or_default();
    for (label, window) in app.webview_windows() {
        let Some(id) = label
            .strip_prefix(OVERLAY_PREFIX)
            .and_then(|resto| resto.parse::<u32>().ok())
        else {
            continue;
        };
        // **Aparcada fuera de las pantallas, no escondida.**
        //
        // Una ventana escondida se queda sin atender: al ensennarla otra vez tardaba
        // entre 300 y 490 ms en enterarse siquiera del aviso de Rust, y ahi se iba la
        // mitad de lo que tarda el atajo (medido: de 905 a 623 ms el camino entero,
        // hasta ver la imagen). Aparcada sigue existiendo para Windows, asi que
        // reacciona antes, y como esta fuera de todas las pantallas no la ve nadie.
        // (Y desde que el navegador arranca con `NAVEGADOR_ARGS`, aparcada tampoco se
        // duerme: el aviso le llega en 10 ms.)
        //
        // Y cada una a SU aparcadero, pegado a su monitor: ver `aparcadero_de`.
        let sitio = monitores
            .iter()
            .find(|m| m.id == id)
            .map(|m| aparcadero_de(m, &monitores))
            .unwrap_or_else(|| aparcadero(app));
        let _ = window.set_position(sitio);
    }
}

/// Donde aparcar el overlay de ESE monitor: fuera de todas las pantallas, pero de forma
/// que **el monitor mas cercano a la ventana aparcada sea el suyo**, o uno con su mismo
/// escalado.
///
/// Windows le asigna a una ventana el DPI del monitor mas cercano. Aparcadas todas en la
/// misma esquina, el overlay del monitor vertical (al 100 %) quedaba pegado a uno al 125 %:
/// al traerlo a su pantalla Windows le cambiaba el DPI y **lo reescalaba a 864x1536**, en
/// vez de los 1080x1920 que le tocan, y el navegador tenia que volver a dibujarlo todo a
/// otra escala. Visto en una foto de la ventana el 5 de septiembre de 2026.
///
/// Se prueban los cuatro lados (encima, izquierda, debajo, derecha de todo el escritorio,
/// alineado con el monitor) y se elige el primero cuyo vecino mas cercano es el propio
/// monitor; si ninguno lo consigue (un monitor rodeado por otros), vale uno cuyo vecino
/// tenga el mismo escalado, que es lo que de verdad importa.
fn aparcadero_de(monitor: &MonitorInfo, todos: &[MonitorInfo]) -> PhysicalPosition<i32> {
    const MARGEN: i32 = 100;
    let izquierda = todos.iter().map(|m| m.x).min().unwrap_or(monitor.x);
    let arriba = todos.iter().map(|m| m.y).min().unwrap_or(monitor.y);
    let derecha = todos.iter().map(|m| m.x + m.width as i32).max().unwrap_or(monitor.x);
    let abajo = todos.iter().map(|m| m.y + m.height as i32).max().unwrap_or(monitor.y);
    let (ancho, alto) = (monitor.width as i32, monitor.height as i32);
    let candidatos = [
        (monitor.x, arriba - alto - MARGEN),
        (izquierda - ancho - MARGEN, monitor.y),
        (monitor.x, abajo + MARGEN),
        (derecha + MARGEN, monitor.y),
    ];
    let mas_cercano = |x: i32, y: i32| {
        todos
            .iter()
            .min_by_key(|m| distancia_entre((x, y, x + ancho, y + alto), (m.x, m.y, m.x + m.width as i32, m.y + m.height as i32)))
    };
    let elegido = candidatos
        .iter()
        .find(|(x, y)| mas_cercano(*x, *y).is_some_and(|m| m.id == monitor.id))
        .or_else(|| {
            candidatos.iter().find(|(x, y)| {
                mas_cercano(*x, *y).is_some_and(|m| (m.scale - monitor.scale).abs() < 0.01)
            })
        })
        .copied()
        .unwrap_or(candidatos[0]);
    PhysicalPosition::new(elegido.0, elegido.1)
}

/// La distancia entre dos rectangulos (izquierda, arriba, derecha, abajo), al cuadrado:
/// cero si se tocan o se solapan.
fn distancia_entre(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> i64 {
    let dx = i64::from((b.0 - a.2).max(a.0 - b.2).max(0));
    let dy = i64::from((b.1 - a.3).max(a.1 - b.3).max(0));
    dx * dx + dy * dy
}

/// Un punto fuera de todas las pantallas, donde aparcar un overlay sin que se vea. Es el
/// respaldo de `aparcadero_de`, para una ventana de la que no se sabe el monitor.
///
/// No vale un numero grande y negativo a ojo: con un monitor a la izquierda del principal,
/// las coordenadas negativas son pantalla de verdad. Se busca la esquina de mas arriba y
/// mas a la izquierda de todo el escritorio y se sale de ahi por el alto de la pantalla mas
/// alta, que es lo que puede llegar a medir un overlay.
fn aparcadero(app: &AppHandle) -> PhysicalPosition<i32> {
    let mut x = 0;
    let mut y = 0;
    let mut alto = 1080;
    if let Ok(monitores) = app.available_monitors() {
        for m in monitores {
            x = x.min(m.position().x);
            y = y.min(m.position().y);
            alto = alto.max(m.size().height as i32);
        }
    }
    PhysicalPosition::new(x - 200, y - alto - 200)
}

/// Barra flotante que acompanna a la grabacion, justo debajo de la region.
/// Las ventanas se crean en el hilo principal: hacerlo desde el hilo de un comando
/// bloquea a la espera del bucle de eventos y la grabacion se queda colgada.
pub fn open_recorder(app: &AppHandle, region: Rect) -> Result<()> {
    close_recorder(app);
    let label = format!(
        "{RECORDER_PREFIX}{}",
        OVERLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("recorder.html".into()))
        .additional_browser_args(NAVEGADOR_ARGS)
        .title("winshotx")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(true)
        .visible(false)
        .inner_size(360.0, 52.0)
        .build()?;
    crate::platform::window_style::rounded_corners(&window);

    colocar_barra(&window, region);
    window.show()?;
    Ok(())
}

/// Pone la barra debajo de la region, y dentro de la pantalla donde esta esa region.
///
/// Antes se usaba `x.max(0)`, y eso mandaba la barra a la pantalla principal siempre que
/// se grababa en un monitor colocado a la izquierda o encima del principal: ahi las
/// coordenadas del escritorio son NEGATIVAS, y recortarlas a cero es literalmente saltar
/// de pantalla. Y el escalado se le preguntaba a la ventana, que acaba de nacer y todavia
/// esta en la pantalla equivocada.
fn colocar_barra(window: &tauri::WebviewWindow, region: Rect) {
    let centro = (
        region.x + region.width as i32 / 2,
        region.y + region.height as i32 / 2,
    );
    let (x, y) = sitio_de_la_barra(region, monitor_de(centro.0, centro.1));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// La cuenta de donde va la barra, aparte del resto para poder probarla sin pantallas.
///
/// Si no se sabe en que monitor cae, se coloca debajo de la region y se deja estar: es
/// preferible una barra un poco fuera de sitio que una barra en otra pantalla.
fn sitio_de_la_barra(region: Rect, monitor: Option<MonitorSitio>) -> (i32, i32) {
    let escala = monitor.map(|m| m.escala).unwrap_or(1.0);
    let ancho_px = (BARRA_ANCHO * escala) as i32;
    let alto_px = (BARRA_ALTO * escala) as i32;
    let hueco = (12.0 * escala) as i32;

    let mut x = region.x + (region.width as i32 - ancho_px) / 2;
    let mut y = region.y + region.height as i32 + hueco;

    if let Some(m) = monitor {
        let margen = (8.0 * escala) as i32;
        let izquierda = m.x + margen;
        let derecha = (m.x + m.ancho - ancho_px - margen).max(izquierda);
        x = x.clamp(izquierda, derecha);
        // Si la region llega al borde de abajo, la barra se sube encima en vez de quedarse
        // colgando fuera de la pantalla o saltando a la de al lado.
        let tope = m.y + m.alto - alto_px - margen;
        if y > tope {
            y = (region.y - alto_px - hueco).max(m.y + margen);
        }
    }
    (x, y)
}

/// Ancho y alto de la barra de grabacion, en pixeles logicos. Los mismos que pide el
/// constructor de la ventana: si se separan, la barra deja de quedar centrada.
const BARRA_ANCHO: f64 = 360.0;
const BARRA_ALTO: f64 = 52.0;

/// Los datos de un monitor que hacen falta para colocar una ventana encima.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MonitorSitio {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) ancho: i32,
    pub(crate) alto: i32,
    pub(crate) escala: f64,
}

/// El monitor que contiene ese punto del escritorio, o el principal si el punto se ha
/// quedado en tierra de nadie (pasa entre monitores desalineados).
pub(crate) fn monitor_de(x: i32, y: i32) -> Option<MonitorSitio> {
    let monitores = xcap::Monitor::all().ok()?;
    let leer = |m: &xcap::Monitor| -> Option<MonitorSitio> {
        Some(MonitorSitio {
            x: m.x().ok()?,
            y: m.y().ok()?,
            ancho: m.width().ok()? as i32,
            alto: m.height().ok()? as i32,
            escala: f64::from(m.scale_factor().ok()?),
        })
    };
    let dentro = |s: &MonitorSitio| x >= s.x && x < s.x + s.ancho && y >= s.y && y < s.y + s.alto;

    monitores
        .iter()
        .filter_map(leer)
        .find(dentro)
        .or_else(|| monitores.iter().filter_map(leer).next())
}

pub fn close_recorder(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(RECORDER_PREFIX) {
            let _ = window.close();
        }
    }
}

/// Editor de recorte y exportacion para una sesion ya grabada.
///
/// **Con el marco de Windows, no con una barra dibujada.** Es una ventana normal, de las
/// que se minimizan, se maximizan y se cierran con los botones que todo el mundo conoce,
/// y las ventanas normales de winshotx llevan el marco del sistema. Los overlays, la barra
/// de grabacion, la cuenta atras y las capturas ancladas van sin marco porque son
/// herramientas flotantes, no ventanas.
pub fn open_editor(app: &AppHandle, session_id: &str) -> Result<()> {
    for (label, window) in app.webview_windows() {
        if label.starts_with(EDITOR_LABEL) {
            let _ = window.close();
        }
    }
    let label = format!(
        "{EDITOR_LABEL}-{}",
        OVERLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let url = WebviewUrl::App(format!("editor.html?session={session_id}").into());
    let window = WebviewWindowBuilder::new(app, &label, url)
        .additional_browser_args(NAVEGADOR_ARGS)
        .title("winshotx · editor")
        .decorations(true)
        .resizable(true)
        .inner_size(1080.0, 700.0)
        .min_inner_size(760.0, 520.0)
        .center()
        .visible(true)
        .build()?;
    // Sin `rounded_corners`: eso es para las ventanas sin marco. El marco del sistema ya
    // trae las esquinas que Windows quiere, y forzarlas encima deja un borde raro.
    let _ = window.set_min_size(Some(LogicalSize::new(760.0, 520.0)));
    window.set_focus()?;
    Ok(())
}

/// Deja la captura flotando encima de todo, para tenerla a la vista mientras se trabaja.
///
/// La ventana sale **del tamanno exacto del recorte y justo encima de donde estaba**, asi
/// que al aparecer no se mueve nada: parece que el trozo de pantalla se ha quedado quieto
/// mientras lo de debajo sigue. Despues se arrastra a donde se quiera.
///
/// Se pueden anclar varias: cada una es su propia ventana y se cierra por su cuenta. Por
/// eso el numero en la etiqueta, igual que en los overlays.
pub fn open_pin(app: &AppHandle, region: Rect, imagen: &std::path::Path) -> Result<()> {
    let label = format!(
        "{PIN_PREFIX}{}",
        OVERLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    // La ruta viaja en la URL porque la ventana nace sabiendo que imagen lleva y no
    // cambia nunca: pedirsela despues por un comando seria un viaje de ida y vuelta para
    // ensennar algo que ya estaba decidido.
    let url = WebviewUrl::App(
        format!(
            "pin.html?imagen={}",
            para_url(&imagen.to_string_lossy())
        )
        .into(),
    );
    let window = WebviewWindowBuilder::new(app, &label, url)
        .additional_browser_args(NAVEGADOR_ARGS)
        .title("winshotx")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(true)
        .visible(false)
        .build()?;
    crate::platform::window_style::rounded_corners(&window);

    // El tamanno y el sitio van en pixeles FISICOS, que es lo que trae la region: los
    // logicos dependen del zoom de cada pantalla y sobre un monitor al 150 % el ancla
    // habria salido una vez y media mas grande que el recorte.
    let (x, y) = sitio_del_ancla(region, monitor_de(region.x, region.y));
    let _ = window.set_size(PhysicalSize::new(region.width, region.height));
    let _ = window.set_position(PhysicalPosition::new(x, y));
    window.show()?;
    Ok(())
}

/// Lo minimo para que una ruta de Windows quepa dentro de una URL sin romperla.
///
/// `C:\Users\Muni\...` lleva dos puntos y barras invertidas, y una carpeta con espacios o
/// con una almohadilla partiria la direccion por la mitad. Se escapa todo lo que no sea
/// una letra, un numero o los cuatro simbolos que las URL dejan pasar tal cual. Son seis
/// lineas y evitan traerse una caja entera solo para esto.
fn para_url(texto: &str) -> String {
    texto
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            otro => format!("%{otro:02X}"),
        })
        .collect()
}

/// Donde cae la captura anclada, aparte para poder probarla sin tener las pantallas.
///
/// Encima de su recorte, y si de ahi se sale del monitor (porque el recorte tocaba el
/// borde y la sombra pide un par de pixeles), se empuja hacia dentro. **Nunca se recorta
/// a cero**: en un escritorio con el monitor a la izquierda del principal, las
/// coordenadas son negativas y un `max(0)` manda el ancla a otra pantalla.
fn sitio_del_ancla(region: Rect, monitor: Option<MonitorSitio>) -> (i32, i32) {
    let Some(m) = monitor else {
        return (region.x, region.y);
    };
    let ancho = region.width as i32;
    let alto = region.height as i32;
    // El limite de arriba manda sobre el de abajo: con un recorte mas grande que la
    // pantalla, preferimos ver la esquina de arriba a la izquierda que la de abajo.
    let x = (region.x).min(m.x + m.ancho - ancho).max(m.x);
    let y = (region.y).min(m.y + m.alto - alto).max(m.y);
    (x, y)
}

/// La ventana de ajustes no se destruye al cerrarla, solo se esconde, asi que su
/// interfaz sigue montada con los datos del arranque. Al volver a mostrarla hay que
/// decirselo para que refresque el tamanno de la cache y mire si hay version nueva.
pub fn show_settings(app: &AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        let _ = window.unminimize();
        let _ = window.set_focus();
        // Lo mismo que el overlay, y por lo mismo: `show` y `set_focus` no bastan cuando
        // la orden no viene de un clic del usuario en NUESTRA ventana. Desde el menu de la
        // bandeja, o desde una segunda instancia que pasa el testigo, Windows nos niega el
        // primer plano y la ventana se quedaba escondida sin decir ni un error.
        #[cfg(windows)]
        force_foreground(&window);
        let _ = window.emit(crate::EVENT_SETTINGS_SHOWN, ());
    }
    Ok(())
}

/// Windows solo deja robar el primer plano a quien tiene derecho a ello.
/// Al venir de un atajo global lo tenemos, pero hay que pedirlo explicitamente.
#[cfg(windows)]
fn force_foreground(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, SW_SHOW, ShowWindow};

    let Ok(handle) = window.hwnd() else { return };
    unsafe {
        let hwnd = HWND(handle.0);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
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

    fn region(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// El fallo que Munir vio el 27 de agosto de 2026: grabo en el monitor vertical de la
    /// izquierda y la barra aparecio en otro. Ese monitor tiene coordenadas NEGATIVAS en
    /// el escritorio, y el codigo hacia `x.max(0)`, que es literalmente "salta a la
    /// pantalla principal".
    #[test]
    fn la_barra_se_queda_en_el_monitor_de_la_izquierda() {
        let izquierdo = monitor(-1080, 0, 1080, 1920);
        let (x, y) = sitio_de_la_barra(region(-900, 200, 600, 400), Some(izquierdo));
        assert!(
            x < 0,
            "la barra se ha ido a la pantalla principal: x = {x}, y el monitor acaba en 0"
        );
        assert!(x >= izquierdo.x, "y tampoco puede salirse por la izquierda");
        assert_eq!(y, 612, "debajo de la región, doce píxeles más abajo");
    }

    /// Debajo de la region es lo normal, y centrada con ella.
    #[test]
    fn la_barra_va_centrada_debajo_de_la_region() {
        let principal = monitor(0, 0, 1920, 1080);
        let (x, y) = sitio_de_la_barra(region(500, 100, 400, 300), Some(principal));
        assert_eq!(x, 500 + (400 - 360) / 2, "centrada con la región");
        assert_eq!(y, 100 + 300 + 12);
    }

    /// Y si la region llega abajo del todo, la barra se sube encima en vez de salirse.
    #[test]
    fn si_no_cabe_debajo_la_barra_se_pone_encima() {
        let principal = monitor(0, 0, 1920, 1080);
        let (_, y) = sitio_de_la_barra(region(300, 700, 800, 370), Some(principal));
        assert!(y < 700, "tenía que subirse encima de la región y está en {y}");
        assert!(y >= 8, "y sin salirse por arriba");
    }

    /// Una region ancha en un monitor estrecho: la barra se pega al borde, pero dentro.
    #[test]
    fn la_barra_nunca_se_sale_por_los_lados() {
        let estrecho = monitor(1920, 0, 400, 800);
        let (x, _) = sitio_de_la_barra(region(1930, 10, 380, 200), Some(estrecho));
        assert!(x >= 1928, "se sale por la izquierda: {x}");
        assert!(x + 360 <= 1920 + 400 - 8, "se sale por la derecha: {x}");
    }

    /// Lo que hace que anclar se entienda sin explicarlo: la captura aparece EXACTAMENTE
    /// donde estaba el recorte, asi que al pulsar la tecla no se mueve nada en pantalla.
    #[test]
    fn el_ancla_nace_justo_encima_de_su_recorte() {
        let principal = monitor(0, 0, 1920, 1080);
        let (x, y) = sitio_del_ancla(region(400, 250, 600, 400), Some(principal));
        assert_eq!((x, y), (400, 250));
    }

    /// Y la trampa de siempre: el monitor de la izquierda empieza en negativo, asi que
    /// recortar a cero mandaria el ancla a la pantalla principal.
    #[test]
    fn el_ancla_se_queda_en_el_monitor_de_coordenadas_negativas() {
        let izquierdo = monitor(-1080, 0, 1080, 1920);
        let (x, y) = sitio_del_ancla(region(-900, 200, 600, 400), Some(izquierdo));
        assert_eq!((x, y), (-900, 200), "el ancla tiene que quedarse donde estaba");
    }

    /// Un recorte pegado al borde de abajo a la derecha: la ventana se empuja hacia dentro
    /// para que no quede media fuera de la pantalla.
    #[test]
    fn el_ancla_se_mete_dentro_si_el_recorte_tocaba_el_borde() {
        let principal = monitor(0, 0, 1920, 1080);
        let (x, y) = sitio_del_ancla(region(1900, 1060, 300, 200), Some(principal));
        assert_eq!(x, 1920 - 300, "empujada hacia dentro por la derecha");
        assert_eq!(y, 1080 - 200, "y por abajo");
    }

    /// Con un recorte mas grande que la pantalla manda el borde de ARRIBA: se ve la
    /// esquina de arriba a la izquierda, que es donde se mira primero.
    #[test]
    fn un_ancla_mas_grande_que_la_pantalla_ensenna_la_esquina_de_arriba() {
        let principal = monitor(0, 0, 1920, 1080);
        let (x, y) = sitio_del_ancla(region(0, 0, 3000, 2000), Some(principal));
        assert_eq!((x, y), (0, 0));
    }

    /// Sin saber en que monitor cae, se deja donde estaba el recorte: es lo unico honrado.
    #[test]
    fn sin_monitor_el_ancla_se_queda_donde_el_recorte() {
        let (x, y) = sitio_del_ancla(region(-500, 300, 200, 100), None);
        assert_eq!((x, y), (-500, 300));
    }

    /// La ruta de la imagen viaja dentro de una URL, y las de Windows llevan dos puntos,
    /// barras invertidas y a veces espacios: sin escapar, la direccion se parte por ahi.
    #[test]
    fn la_ruta_del_ancla_sobrevive_a_ir_en_una_url() {
        let escapada = para_url(r"C:\Users\Mi Carpeta\pin-1.png");
        assert!(!escapada.contains('\\'), "la barra invertida rompe la URL");
        assert!(!escapada.contains(' '), "el espacio corta la direccion");
        assert!(!escapada.contains(':'), "los dos puntos tambien");
        assert!(escapada.contains("pin-1.png"), "el nombre se sigue leyendo: {escapada}");
    }

    /// Las tres pantallas de Munir tal como estan puestas: la principal al 125 %, una
    /// vertical al 100 % a su izquierda (y algo mas baja) y una tercera al 125 % debajo.
    fn pantallas_de_munir() -> Vec<MonitorInfo> {
        let m = |id, x, y, w, h, scale, is_primary| MonitorInfo {
            id,
            label: format!("M{id}"),
            x,
            y,
            width: w,
            height: h,
            scale,
            is_primary,
        };
        vec![
            m(0, 0, 0, 1920, 1080, 1.25, true),
            m(1, -1080, 238, 1080, 1920, 1.0, false),
            m(2, 0, 1080, 1536, 960, 1.25, false),
        ]
    }

    /// Lo que vecino mas cercano tiene un rectangulo aparcado en ese punto.
    fn vecino(x: i32, y: i32, m: &MonitorInfo, todos: &[MonitorInfo]) -> u32 {
        todos
            .iter()
            .min_by_key(|otro| {
                distancia_entre(
                    (x, y, x + m.width as i32, y + m.height as i32),
                    (otro.x, otro.y, otro.x + otro.width as i32, otro.y + otro.height as i32),
                )
            })
            .map(|otro| otro.id)
            .unwrap()
    }

    /// El fallo visto en la foto: el overlay vertical, aparcado en la esquina de siempre,
    /// tenia como vecino un monitor al 125 % y volvia reescalado a 864x1536.
    #[test]
    fn cada_overlay_se_aparca_junto_a_un_monitor_de_su_mismo_escalado() {
        let todos = pantallas_de_munir();
        for m in &todos {
            let sitio = aparcadero_de(m, &todos);
            let cerca = vecino(sitio.x, sitio.y, m, &todos);
            let escala_del_vecino = todos.iter().find(|o| o.id == cerca).unwrap().scale;
            assert!(
                (escala_del_vecino - m.scale).abs() < 0.01,
                "el overlay {} se aparca en {:?} junto al monitor {cerca}, que va a otra escala",
                m.id,
                (sitio.x, sitio.y)
            );
        }
    }

    #[test]
    fn el_aparcadero_queda_fuera_de_todas_las_pantallas() {
        let todos = pantallas_de_munir();
        for m in &todos {
            let sitio = aparcadero_de(m, &todos);
            let rect = (sitio.x, sitio.y, sitio.x + m.width as i32, sitio.y + m.height as i32);
            for otro in &todos {
                let suyo = (otro.x, otro.y, otro.x + otro.width as i32, otro.y + otro.height as i32);
                assert!(
                    distancia_entre(rect, suyo) > 0,
                    "el overlay {} aparcado en {:?} pisa el monitor {}",
                    m.id,
                    (sitio.x, sitio.y),
                    otro.id
                );
            }
        }
    }

    /// El vertical, que es el que fallaba, tiene que quedar con EL como vecino: a su
    /// izquierda hay sitio libre.
    #[test]
    fn el_vertical_se_aparca_a_su_izquierda() {
        let todos = pantallas_de_munir();
        let vertical = &todos[1];
        let sitio = aparcadero_de(vertical, &todos);
        assert_eq!(vecino(sitio.x, sitio.y, vertical, &todos), vertical.id);
        assert!(sitio.x < -1080, "tenia que salir por la izquierda: {}", sitio.x);
    }

    #[test]
    fn la_distancia_entre_rectangulos_es_cero_si_se_tocan() {
        assert_eq!(distancia_entre((0, 0, 10, 10), (10, 0, 20, 10)), 0);
        assert_eq!(distancia_entre((0, 0, 10, 10), (20, 0, 30, 10)), 100);
        assert_eq!(distancia_entre((0, 0, 10, 10), (13, 14, 30, 30)), 9 + 16);
    }
}
