//! Esquinas redondeadas de verdad. Dibujarlas en CSS sobre una ventana transparente
//! deja un halo negro alrededor; Windows 11 sabe recortarlas él mismo.

#[cfg(windows)]
pub fn rounded_corners(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };

    let Ok(handle) = window.hwnd() else { return };
    unsafe {
        let hwnd = HWND(handle.0);
        let preference = DWMWCP_ROUND;
        // En Windows 10 este atributo no existe: el error se ignora sin más.
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const preference).cast(),
            std::mem::size_of_val(&preference) as u32,
        );
    }
}

#[cfg(not(windows))]
pub fn rounded_corners(_window: &tauri::WebviewWindow) {}

/// La ventana a pelo, como numero: el `HWND` en Windows y 0 en los demas sistemas, donde
/// no hay nada que guardar (las funciones que lo reciben no hacen nada alli).
#[cfg(windows)]
pub fn a_pelo(window: &tauri::WebviewWindow) -> isize {
    window.hwnd().map(|h| h.0 as isize).unwrap_or(0)
}

#[cfg(not(windows))]
pub fn a_pelo(_window: &tauri::WebviewWindow) -> isize {
    0
}

/// Una ventana que no coge el foco JAMAS, ni al mostrarse ni al hacerle clic encima.
///
/// El `.focused(false)` del constructor solo vale para el primer momento: en cuanto se
/// esconde y se vuelve a mostrar, `ShowWindow` la activa como a cualquier otra. Para la
/// cuenta atras eso no es un detalle estetico, es lo unico que decide si la funcion sirve:
/// el temporizador existe para fotografiar un menu abierto, y robarle el foco al menu lo
/// cierra, que es exactamente lo que veniamos a evitar. `WS_EX_NOACTIVATE` se lo prohibe
/// a Windows, y `WS_EX_TOOLWINDOW` la deja fuera de Alt+Tab.
///
/// **Y se quedan puestos.** Hasta el 14-nov-2026 se escribían una vez, y tao los borraba en
/// el primer `show()` o `hide()` de Tauri (reescribe el estilo extendido entero cada vez):
/// de la segunda cuenta atrás en adelante, la ventana volvía a poder quitar el foco y a
/// llevar la marca de ventana de la app. Ahora los mantiene el vigilante de `guardia`.
#[cfg(windows)]
pub fn never_focus(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::WS_EX_NOACTIVATE;

    crate::platform::guardia::fuera_de_la_lista(window, WS_EX_NOACTIVATE.0, false);
}

#[cfg(not(windows))]
pub fn never_focus(_window: &tauri::WebviewWindow) {}

/// Mueve la ventana a ese punto y la ensenna, de una sola vez y sin activarla.
///
/// Es para los overlays, que esperan aparcados fuera de las pantallas a tener su imagen:
/// en el momento de aparecer cada llamada a Windows cuenta, y `set_position` mas `show`
/// eran dos, cada una con su vuelta por el compositor. `SWP_NOACTIVATE` porque el foco se
/// le da aparte y solo a la ventana de la pantalla del raton. Devuelve `false` si no ha
/// podido, para que quien llama use el camino normal.
///
/// **Tauri no se entera de que la ventana se ve.** Sigue creyendo que esta escondida, y su
/// `hide()` compara con lo que cree y no hace nada: la ventana se queda en pantalla. Quien
/// la ensenne asi, o llama a `show()` despues para ponerle al dia (los overlays), o la
/// esconde con [`esconder_hwnd`] (la miniatura, que no puede llamar a `show()` porque
/// cogeria el foco). Paso con la miniatura de la 0.4.0: las cartas se iban y el recuadro se
/// quedaba.
#[cfg(windows)]
pub fn colocar_y_ensennar(window: &tauri::WebviewWindow, x: i32, y: i32) -> bool {
    let Ok(handle) = window.hwnd() else { return false };
    colocar_y_ensennar_hwnd(handle.0 as isize, x, y)
}

/// Lo mismo, sobre una ventana de Windows a pelo, para poder probarlo.
#[cfg(windows)]
pub fn colocar_y_ensennar_hwnd(ventana: isize, x: i32, y: i32) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    unsafe {
        SetWindowPos(
            HWND(ventana as *mut core::ffi::c_void),
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE,
        )
        .is_ok()
    }
}

#[cfg(not(windows))]
pub fn colocar_y_ensennar(_window: &tauri::WebviewWindow, _x: i32, _y: i32) -> bool {
    false
}

/// Esconde la ventana por el mismo camino por el que la ensenna [`colocar_y_ensennar`]:
/// directamente con Windows, sin pasar por lo que Tauri cree. Quien llama hace despues el
/// `hide()` de Tauri, que no hace nada si Tauri ya la creia escondida y la pone al dia si no.
///
/// Recibe la ventana a pelo y no un `WebviewWindow` a proposito: sacar el `hwnd()` de una
/// ventana de Tauri desde otro hilo es una pregunta que espera sin plazo (trampa 34), y la
/// miniatura se esconde desde los hilos de sus ordenes.
#[cfg(windows)]
pub fn esconder_hwnd(ventana: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

    // Lo que devuelve es si se veia antes, no si ha ido bien: no hay nada que mirar.
    let _ = unsafe { ShowWindow(HWND(ventana as *mut core::ffi::c_void), SW_HIDE) };
}

#[cfg(not(windows))]
pub fn esconder_hwnd(_ventana: isize) {}

/// Si alguien la ha minimizado. Un `show()` no la devuelve: la enseña minimizada, en
/// ningún sitio que se vea.
#[cfg(windows)]
pub fn minimizada(ventana: isize) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsIconic;

    ventana != 0 && unsafe { IsIconic(HWND(ventana as *mut core::ffi::c_void)) }.as_bool()
}

#[cfg(not(windows))]
pub fn minimizada(_ventana: isize) -> bool {
    false
}

/// La ventana que esta delante ahora mismo (la que recibe el teclado), o 0 si no hay.
#[cfg(windows)]
pub fn primer_plano() -> isize {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe { GetForegroundWindow() }.0 as isize
}

#[cfg(not(windows))]
pub fn primer_plano() -> isize {
    0
}

/// La clase de una ventana de Windows («Shell_TrayWnd» es la barra de tareas), para el
/// registro: dice quien se ha quedado delante sin tener que adivinarlo.
#[cfg(windows)]
pub fn clase_de(ventana: isize) -> String {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

    if ventana == 0 {
        return "ninguna".into();
    }
    let mut texto = [0u16; 128];
    let largo = unsafe { GetClassNameW(HWND(ventana as *mut core::ffi::c_void), &mut texto) };
    String::from_utf16_lossy(&texto[..largo.max(0) as usize])
}

#[cfg(not(windows))]
pub fn clase_de(_ventana: isize) -> String {
    String::new()
}

/// Le devuelve el foco del teclado a la ventana de arriba, que se lo pasa a su pagina.
///
/// Solo vale desde el hilo de la ventana (el principal) y con la app delante. Sirve para
/// que Tauri vuelva a saber que la ventana tiene el foco despues de que el navegador de
/// dentro se lo llevara por su cuenta: sin eso, Tauri ya no avisa cuando lo pierde de verdad.
#[cfg(windows)]
pub fn devolver_el_foco(ventana: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;

    let _ = unsafe { SetFocus(Some(HWND(ventana as *mut core::ffi::c_void))) };
}

#[cfg(not(windows))]
pub fn devolver_el_foco(_ventana: isize) {}

/// Deja la ventana fuera de lo que se graba, sin esconderla.
///
/// `WDA_EXCLUDEFROMCAPTURE` le dice al compositor de Windows que, para quien fotografia o
/// graba la pantalla, esa ventana no esta: quien esta delante la sigue viendo y la sigue
/// pulsando, y en el video sale lo que hay detras. Es lo mismo que hacen las aplicaciones
/// de videollamada para que su propia ventana no se cuele al compartir pantalla.
///
/// Existe desde Windows 10 2004. En uno mas viejo la llamada falla, se devuelve `false` y
/// la ventana se queda como estaba, que es exactamente lo de antes.
#[cfg(windows)]
pub fn fuera_de_la_captura(window: &tauri::WebviewWindow, fuera: bool) -> bool {
    let Ok(handle) = window.hwnd() else { return false };
    return fuera_de_la_captura_hwnd(handle.0 as isize, fuera);
}

/// Lo mismo, sobre una ventana de Windows a pelo. Va aparte para poder probarlo: crear un
/// `WebviewWindow` en una prueba significa levantar media aplicacion, y una ventana del
/// sistema son tres lineas.
#[cfg(windows)]
pub fn fuera_de_la_captura_hwnd(ventana: isize, fuera: bool) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    };

    let hwnd = HWND(ventana as *mut core::ffi::c_void);
    let afinidad = if fuera { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
    return unsafe { SetWindowDisplayAffinity(hwnd, afinidad) }.is_ok();
}

#[cfg(not(windows))]
pub fn fuera_de_la_captura(_window: &tauri::WebviewWindow, _fuera: bool) -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GetWindowDisplayAffinity, IsWindowVisible,
        WDA_EXCLUDEFROMCAPTURE, WDA_NONE, WINDOW_EX_STYLE, WS_POPUP,
    };

    /// Una ventana de verdad, de la clase que ya trae Windows, y fuera de todas las
    /// pantallas para que no la vea nadie mientras corren las pruebas.
    fn ventana_de_prueba() -> isize {
        let clase: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(clase.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                -20_000,
                -20_000,
                40,
                20,
                None,
                None,
                None,
                None,
            )
        }
        .expect("no se ha podido crear la ventana de prueba");
        hwnd.0 as isize
    }

    fn afinidad(ventana: isize) -> u32 {
        let mut valor = 0u32;
        unsafe {
            GetWindowDisplayAffinity(HWND(ventana as *mut core::ffi::c_void), &mut valor)
                .expect("no se ha podido leer la afinidad");
        }
        valor
    }

    /// Que Windows acepte la orden es lo unico que no se puede saber leyendo el codigo: en
    /// una version vieja, o en una ventana que no la admita, `SetWindowDisplayAffinity`
    /// devuelve error y el menu seguiria saliendo en el video sin que nadie se entere.
    #[test]
    fn la_ventana_se_puede_sacar_de_la_captura_y_devolver() {
        let ventana = ventana_de_prueba();
        assert!(
            fuera_de_la_captura_hwnd(ventana, true),
            "Windows no ha aceptado sacar la ventana de la captura"
        );
        assert_eq!(afinidad(ventana), WDA_EXCLUDEFROMCAPTURE.0);

        assert!(fuera_de_la_captura_hwnd(ventana, false));
        assert_eq!(
            afinidad(ventana),
            WDA_NONE.0,
            "tiene que volver a poder fotografiarse cuando ya no se graba"
        );

        unsafe {
            let _ = DestroyWindow(HWND(ventana as *mut core::ffi::c_void));
        }
    }

    fn se_ve(ventana: isize) -> bool {
        unsafe { IsWindowVisible(HWND(ventana as *mut core::ffi::c_void)) }.as_bool()
    }

    /// La miniatura de la 0.4.0 se quedaba en pantalla, vacía, al irse su última carta: se
    /// enseñaba con `colocar_y_ensennar` y se escondía con el `hide()` de Tauri, que no la
    /// veía visible y no hacía nada. Lo que se enseña por ese camino se esconde por este.
    /// Fuera de todas las pantallas, para que no aparezca nada mientras corre.
    #[test]
    fn lo_que_se_ensenna_por_fuera_de_tauri_se_esconde() {
        let ventana = ventana_de_prueba();
        assert!(!se_ve(ventana), "la ventana de prueba nace escondida");

        assert!(colocar_y_ensennar_hwnd(ventana, -20_000, -20_000));
        assert!(se_ve(ventana), "colocar_y_ensennar tiene que enseñarla");

        esconder_hwnd(ventana);
        assert!(!se_ve(ventana), "esconder tiene que quitarla de verdad");

        // Y otra vuelta, que la miniatura sale y se va muchas veces con la misma ventana.
        assert!(colocar_y_ensennar_hwnd(ventana, -20_000, -20_000));
        assert!(se_ve(ventana));
        esconder_hwnd(ventana);
        assert!(!se_ve(ventana));

        unsafe {
            let _ = DestroyWindow(HWND(ventana as *mut core::ffi::c_void));
        }
    }
}
