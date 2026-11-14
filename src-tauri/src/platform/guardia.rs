//! Un vigilante en el procedimiento de una ventana: lo que Windows le va a hacer, lo ve
//! antes que nadie y puede cambiarlo.
//!
//! **Para qué hace falta (issue #15).** Tauri crea TODAS sus ventanas con
//! `WS_EX_APPWINDOW`, la marca de «ventana de la aplicación», y `skip_taskbar` solo le quita
//! el botón a la barra de tareas de Windows. Un dock de terceros que haga de barra de tareas
//! no mira ese botón: ve la marca y trata el menú de la bandeja como una ventana más de
//! winshotx, que puede volver a enseñar cuando la app se activa. Y una página de WebView2
//! enseñada por fuera puede salir sin pintar: el recuadro blanco del que abrió la #15.
//!
//! Ponerle `WS_EX_TOOLWINDOW` una vez no basta: tao reescribe el estilo extendido entero
//! cada vez que enseña o esconde la ventana (`WindowFlags::apply_diff`), y con él vuelve
//! `WS_EX_APPWINDOW`. Por eso el vigilante se queda en la ventana y corrige el estilo en
//! cada cambio (`WM_STYLECHANGING`).
//!
//! Y a una ventana con **cerrojo** solo la puede enseñar winshotx: un intento de enseñarla
//! sin [`permitir`] antes se queda en nada (`WM_WINDOWPOSCHANGING` sin `SWP_SHOWWINDOW`), y
//! el permiso se gasta en cuanto la ventana se esconde.
//!
//! Va con `SetWindowLongPtrW` y no con `SetWindowSubclass` de comctl32 a propósito: esa
//! función solo existe en comctl32 v6, que el ejecutable de `cargo test --lib` no carga, y
//! con ella dentro no arrancaría ninguna prueba (memoria del banco de S17).

#[cfg(windows)]
mod win {
    use parking_lot::Mutex;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, GetWindowLongPtrW, SetWindowLongPtrW, GWLP_WNDPROC,
        GWL_EXSTYLE, GWL_STYLE, STYLESTRUCT, SWP_HIDEWINDOW, SWP_SHOWWINDOW, WINDOWPOS,
        WM_NCDESTROY, WM_STYLECHANGING, WM_WINDOWPOSCHANGED, WM_WINDOWPOSCHANGING, WNDPROC,
        WS_VISIBLE,
    };

    struct Guardia {
        ventana: isize,
        /// El procedimiento que habia antes, al que se le pasa todo.
        anterior: isize,
        /// Bits del estilo extendido que la ventana tiene que tener siempre, y los que nunca.
        poner: u32,
        quitar: u32,
        /// Si solo la puede enseñar winshotx.
        cerrojo: bool,
        /// Si winshotx la acaba de dejar enseñarse.
        permiso: bool,
        /// Las veces que alguien ha intentado enseñarla sin permiso.
        vetos: u32,
    }

    /// Las ventanas vigiladas. Son tres o cuatro, y se miran en cada mensaje, pero nunca se
    /// guarda el candado mientras se llama al procedimiento de debajo: ese puede volver a
    /// entrar aquí (un `SetWindowLongPtrW` dentro dispara otro `WM_STYLECHANGING`).
    static GUARDIAS: Mutex<Vec<Guardia>> = Mutex::new(Vec::new());

    pub fn vigilar(ventana: isize, poner: u32, quitar: u32, cerrojo: bool) {
        if ventana == 0 {
            return;
        }
        let hwnd = HWND(ventana as *mut core::ffi::c_void);
        {
            let mut guardias = GUARDIAS.lock();
            if let Some(g) = guardias.iter_mut().find(|g| g.ventana == ventana) {
                g.poner |= poner;
                g.quitar |= quitar;
                g.cerrojo |= cerrojo;
            } else {
                // Se apunta ANTES de cambiar el procedimiento: si la ventana es de otro hilo,
                // le pueden llegar mensajes en cuanto se cambia, y el vigilante tiene que
                // encontrarse ya en la lista para saber a quién pasárselos.
                let anterior = unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) };
                guardias.push(Guardia {
                    ventana,
                    anterior,
                    poner,
                    quitar,
                    cerrojo,
                    permiso: false,
                    vetos: 0,
                });
                drop(guardias);
                let nuestro = procedimiento as *const () as isize;
                let devuelto = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, nuestro) };
                if devuelto != 0 && devuelto != anterior {
                    if let Some(g) = GUARDIAS.lock().iter_mut().find(|g| g.ventana == ventana) {
                        g.anterior = devuelto;
                    }
                }
            }
        }
        // Y el estilo de ahora, que ya pasa por el vigilante.
        unsafe {
            let actual = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (actual | poner as isize) & !(quitar as isize));
        }
    }

    pub fn permitir(ventana: isize) {
        if let Some(g) = GUARDIAS.lock().iter_mut().find(|g| g.ventana == ventana) {
            g.permiso = true;
        }
    }

    #[cfg(test)]
    pub fn vigilada(ventana: isize) -> bool {
        GUARDIAS.lock().iter().any(|g| g.ventana == ventana)
    }

    pub fn vetos(ventana: isize) -> u32 {
        GUARDIAS
            .lock()
            .iter_mut()
            .find(|g| g.ventana == ventana)
            .map(|g| std::mem::take(&mut g.vetos))
            .unwrap_or(0)
    }

    unsafe extern "system" fn procedimiento(
        hwnd: HWND,
        mensaje: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let ventana = hwnd.0 as isize;
        let suya = GUARDIAS
            .lock()
            .iter()
            .find(|g| g.ventana == ventana)
            .map(|g| (g.anterior, g.poner, g.quitar, g.cerrojo && !g.permiso));
        let Some((anterior, poner, quitar, cerrada)) = suya else {
            return DefWindowProcW(hwnd, mensaje, wparam, lparam);
        };
        let mut vetada = false;

        match mensaje {
            WM_STYLECHANGING => {
                let estilo = &mut *(lparam.0 as *mut STYLESTRUCT);
                let cual = wparam.0 as i32;
                if cual == GWL_EXSTYLE.0 {
                    estilo.styleNew = (estilo.styleNew | poner) & !quitar;
                } else if cual == GWL_STYLE.0
                    && cerrada
                    && estilo.styleNew & WS_VISIBLE.0 != 0
                    && estilo.styleOld & WS_VISIBLE.0 == 0
                {
                    estilo.styleNew &= !WS_VISIBLE.0;
                    vetada = true;
                }
            }
            WM_WINDOWPOSCHANGING if cerrada => {
                let sitio = &mut *(lparam.0 as *mut WINDOWPOS);
                if sitio.flags.0 & SWP_SHOWWINDOW.0 != 0 {
                    sitio.flags.0 &= !SWP_SHOWWINDOW.0;
                    vetada = true;
                }
            }
            WM_WINDOWPOSCHANGED => {
                let sitio = &*(lparam.0 as *const WINDOWPOS);
                if sitio.flags.0 & SWP_HIDEWINDOW.0 != 0 {
                    if let Some(g) = GUARDIAS.lock().iter_mut().find(|g| g.ventana == ventana) {
                        g.permiso = false;
                    }
                }
            }
            WM_NCDESTROY => {
                // Lo último que recibe la ventana: se le devuelve su procedimiento y se olvida.
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, anterior);
                GUARDIAS.lock().retain(|g| g.ventana != ventana);
            }
            _ => {}
        }

        if vetada {
            if let Some(g) = GUARDIAS.lock().iter_mut().find(|g| g.ventana == ventana) {
                g.vetos += 1;
            }
        }
        let anterior: WNDPROC = std::mem::transmute(anterior);
        CallWindowProcW(anterior, hwnd, mensaje, wparam, lparam)
    }
}

/// Que la ventana no cuente como ventana de la aplicación para nadie: ni la barra de tareas,
/// ni Alt+Tab, ni un dock de terceros. `WS_EX_TOOLWINDOW` puesto y `WS_EX_APPWINDOW` quitado,
/// y así se quedan aunque tao reescriba el estilo. Con `cerrojo`, además, solo la enseña
/// winshotx (ver [`permitir`]).
#[cfg(windows)]
pub fn fuera_de_la_lista_hwnd(ventana: isize, otros: u32, cerrojo: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{WS_EX_APPWINDOW, WS_EX_TOOLWINDOW};
    win::vigilar(ventana, WS_EX_TOOLWINDOW.0 | otros, WS_EX_APPWINDOW.0, cerrojo);
}

#[cfg(not(windows))]
pub fn fuera_de_la_lista_hwnd(_ventana: isize, _otros: u32, _cerrojo: bool) {}

/// Lo mismo, sobre una ventana de Tauri.
pub fn fuera_de_la_lista(window: &tauri::WebviewWindow, otros: u32, cerrojo: bool) {
    #[cfg(windows)]
    if let Ok(handle) = window.hwnd() {
        fuera_de_la_lista_hwnd(handle.0 as isize, otros, cerrojo);
    }
    #[cfg(not(windows))]
    let _ = (window, otros, cerrojo);
}

/// Deja que la ventana con cerrojo se enseñe UNA vez: el permiso se gasta en cuanto se
/// esconde. Se llama justo antes del `show()` de winshotx.
pub fn permitir(ventana: isize) {
    #[cfg(windows)]
    win::permitir(ventana);
    #[cfg(not(windows))]
    let _ = ventana;
}

/// Cuántas veces ha intentado alguien de fuera enseñarla sin permiso desde la última vez
/// que se preguntó. Es para `bandeja.log`: dice si algo de fuera anda tocando el menú.
pub fn vetos(ventana: isize) -> u32 {
    #[cfg(windows)]
    return win::vetos(ventana);
    #[cfg(not(windows))]
    {
        let _ = ventana;
        0
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GetWindowLongPtrW, IsWindowVisible, SetWindowLongPtrW,
        SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE, HWND_TOP, SWP_NOMOVE, SWP_NOSIZE,
        SWP_SHOWWINDOW, SW_HIDE, SW_RESTORE, SW_SHOW, SW_SHOWNA, SW_SHOWNOACTIVATE,
        WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };

    /// Una ventana de Windows a pelo, con `WS_EX_APPWINDOW` como las de Tauri, y fuera de
    /// todas las pantallas para que no la vea nadie.
    fn ventana_de_prueba() -> isize {
        let clase: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_APPWINDOW,
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

    fn h(ventana: isize) -> HWND {
        HWND(ventana as *mut core::ffi::c_void)
    }

    fn se_ve(ventana: isize) -> bool {
        unsafe { IsWindowVisible(h(ventana)) }.as_bool()
    }

    fn estilo_ex(ventana: isize) -> u32 {
        unsafe { GetWindowLongPtrW(h(ventana), GWL_EXSTYLE) as u32 }
    }

    /// Lo que hace tao cada vez que enseña o esconde: escribir el estilo extendido entero,
    /// con `WS_EX_APPWINDOW` y sin `WS_EX_TOOLWINDOW`.
    fn como_tao(ventana: isize) {
        unsafe {
            SetWindowLongPtrW(h(ventana), GWL_EXSTYLE, WS_EX_APPWINDOW.0 as isize);
        }
    }

    /// Un dock que trate el menú como ventana de la app lo ve por la marca de Tauri. Con el
    /// vigilante, la marca no vuelve aunque tao reescriba el estilo cada vez.
    #[test]
    fn la_ventana_sigue_fuera_de_la_lista_aunque_tao_reescriba_el_estilo() {
        let ventana = ventana_de_prueba();
        assert_ne!(estilo_ex(ventana) & WS_EX_APPWINDOW.0, 0, "nace como las de Tauri");

        fuera_de_la_lista_hwnd(ventana, WS_EX_NOACTIVATE.0, false);
        let estilo = estilo_ex(ventana);
        assert_ne!(estilo & WS_EX_TOOLWINDOW.0, 0, "sin WS_EX_TOOLWINDOW");
        assert_ne!(estilo & WS_EX_NOACTIVATE.0, 0, "sin el bit que pedía quien llama");
        assert_eq!(estilo & WS_EX_APPWINDOW.0, 0, "sigue marcada como ventana de la app");

        como_tao(ventana);
        let estilo = estilo_ex(ventana);
        assert_ne!(estilo & WS_EX_TOOLWINDOW.0, 0, "tao le ha quitado WS_EX_TOOLWINDOW");
        assert_ne!(estilo & WS_EX_NOACTIVATE.0, 0, "tao le ha quitado WS_EX_NOACTIVATE");
        assert_eq!(estilo & WS_EX_APPWINDOW.0, 0, "tao le ha vuelto a poner WS_EX_APPWINDOW");

        // Sin cerrojo se enseña y se esconde como siempre.
        unsafe {
            let _ = ShowWindow(h(ventana), SW_SHOWNOACTIVATE);
        }
        assert!(se_ve(ventana), "sin cerrojo tiene que poder enseñarse");
        unsafe {
            let _ = ShowWindow(h(ventana), SW_HIDE);
            let _ = DestroyWindow(h(ventana));
        }
    }

    /// La #15: el menú escondido salía a la vista, en blanco, sin que winshotx lo abriera.
    /// Con cerrojo, nada de fuera lo enseña, venga por donde venga.
    #[test]
    fn con_cerrojo_nadie_mas_la_ensenna() {
        let ventana = ventana_de_prueba();
        fuera_de_la_lista_hwnd(ventana, 0, true);

        for (como, ensennar) in [
            ("ShowWindow(SW_SHOW)", Box::new(|v| unsafe {
                let _ = ShowWindow(h(v), SW_SHOW);
            }) as Box<dyn Fn(isize)>),
            ("ShowWindow(SW_SHOWNOACTIVATE)", Box::new(|v| unsafe {
                let _ = ShowWindow(h(v), SW_SHOWNOACTIVATE);
            })),
            ("ShowWindow(SW_SHOWNA)", Box::new(|v| unsafe {
                let _ = ShowWindow(h(v), SW_SHOWNA);
            })),
            ("ShowWindow(SW_RESTORE)", Box::new(|v| unsafe {
                let _ = ShowWindow(h(v), SW_RESTORE);
            })),
            ("SetWindowPos(SWP_SHOWWINDOW)", Box::new(|v| unsafe {
                let _ = SetWindowPos(h(v), Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            })),
            ("WS_VISIBLE a pelo", Box::new(|v| unsafe {
                let estilo = GetWindowLongPtrW(h(v), GWL_STYLE);
                SetWindowLongPtrW(h(v), GWL_STYLE, estilo | WS_VISIBLE.0 as isize);
            })),
        ] {
            ensennar(ventana);
            assert!(!se_ve(ventana), "{como} la ha enseñado sin permiso de winshotx");
        }
        assert_eq!(vetos(ventana), 6, "cada intento cuenta para bandeja.log");
        assert_eq!(vetos(ventana), 0, "y se cuentan desde la última vez que se preguntó");

        // Con permiso, sí.
        permitir(ventana);
        unsafe {
            let _ = ShowWindow(h(ventana), SW_SHOWNOACTIVATE);
        }
        assert!(se_ve(ventana), "winshotx con permiso tiene que poder enseñarla");

        // Y el permiso se gasta al esconderla: lo siguiente de fuera ya no pasa.
        unsafe {
            let _ = ShowWindow(h(ventana), SW_HIDE);
        }
        assert!(!se_ve(ventana));
        unsafe {
            let _ = ShowWindow(h(ventana), SW_SHOWNOACTIVATE);
        }
        assert!(!se_ve(ventana), "el permiso tenía que haberse gastado al esconderla");

        unsafe {
            let _ = DestroyWindow(h(ventana));
        }
    }

    /// Lo de fuera de verdad viene de otro hilo (el dock es otro proceso): Windows le manda
    /// los mensajes al hilo de la ventana, que aquí tiene que estar atendiéndolos.
    #[test]
    fn tampoco_desde_otro_hilo() {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
        };

        let ventana = ventana_de_prueba();
        fuera_de_la_lista_hwnd(ventana, 0, true);

        let de_fuera = std::thread::spawn(move || unsafe {
            let _ = ShowWindow(h(ventana), SW_SHOWNOACTIVATE);
        });
        let hasta = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !de_fuera.is_finished() && std::time::Instant::now() < hasta {
            let mut mensaje = MSG::default();
            while unsafe { PeekMessageW(&mut mensaje, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&mensaje);
                    DispatchMessageW(&mensaje);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        de_fuera.join().expect("el hilo de fuera");
        assert!(!se_ve(ventana), "un ShowWindow desde otro hilo la ha enseñado");

        unsafe {
            let _ = DestroyWindow(h(ventana));
        }
    }

    /// Al destruirse, la ventana sale de la lista: si no, un `HWND` reutilizado por Windows
    /// para otra ventana heredaría el vigilante de la vieja.
    #[test]
    fn al_destruirse_se_olvida() {
        let ventana = ventana_de_prueba();
        fuera_de_la_lista_hwnd(ventana, 0, true);
        unsafe {
            let _ = DestroyWindow(h(ventana));
        }
        assert!(
            !win::vigilada(ventana),
            "la ventana destruida sigue en la lista del vigilante"
        );
    }
}
