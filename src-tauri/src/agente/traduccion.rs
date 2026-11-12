//! De lo que pide un agente a lo que la app ya entiende: `RecordOptions` para grabar y
//! `ExportRequest` para exportar. Son cuentas sin la app delante, para poder probarlas.
//!
//! **Exportar sin tocar nada sale igual que «sin editor».** La base es
//! `recorder::peticion_por_defecto`, la misma con la que se guarda una grabación cuando no se
//! abre el editor y con la que el editor abre su panel: entera, a su tamaño, con su sonido, el
//! puntero dibujado y la cámara en su esquina si se grabó. Encima se pone solo lo que se pide.
//!
//! **El estilo con nombre se lee aquí igual que en el editor.** Hasta ahora solo lo leía
//! `src/lib/presets.ts` (Rust lo guardaba sin mirarlo, ver `presets.rs`). Esto copia, campo a
//! campo, `leerEstilo`, `panelDe` y `edicionDe` de ese archivo: los mismos topes y los mismos
//! redondeos, para que un estilo puesto por un agente dé el mismo archivo que puesto en el panel.

use serde_json::Value;

use crate::agente::ordenes::{Formato, OpcionesDeGrabar, PedidoDeExportar};
use crate::contrato::{ClicElegido, FondoElegido, MarcoCamara, Suavizado};
use crate::exporter::ExportRequest;
use crate::presets::Preset;
use crate::record::SessionData;
use crate::recorder::RecordOptions;
use crate::settings::Settings;

/// Lo que se le pasa a `recorder::start`. Lo que el agente no dice sale de los ajustes, igual
/// que cuando graba la persona desde el overlay; salvo la marimba, que va apagada si no se
/// pide. Un GIF no lleva sonido, como en el overlay.
pub fn opciones_de_grabar(pedido: &OpcionesDeGrabar, ajustes: &Settings, ventana: Option<isize>) -> RecordOptions {
    let video = pedido.formato == Formato::Mp4;
    RecordOptions {
        format: if video { "video" } else { "gif" }.into(),
        fps: ajustes.fps,
        audio: video && pedido.sonido_pc.unwrap_or(ajustes.record_audio),
        microphone: video && pedido.microfono.unwrap_or(ajustes.record_microphone),
        highlight_clicks: ajustes.highlight_clicks,
        highlight_keys: ajustes.highlight_keys,
        window: ventana,
        camara: Some(pedido.camara.unwrap_or(ajustes.record_camera)),
        marimba: Some(pedido.marimba),
    }
}

/// El formato de lo exportado: el de la extensión de `output`, si no el de `format`, y si no,
/// el de la grabación (un GIF grabado como GIF sale GIF).
pub fn formato_de(pedido: &PedidoDeExportar, session: &SessionData) -> Formato {
    let por_extension = pedido
        .salida
        .as_ref()
        .and_then(|r| r.extension())
        .and_then(|e| e.to_str())
        .map(|e| if e.eq_ignore_ascii_case("gif") { Formato::Gif } else { Formato::Mp4 });
    por_extension.or(pedido.formato).unwrap_or(if session.format == "gif" { Formato::Gif } else { Formato::Mp4 })
}

fn ms(segundos: f64) -> u64 {
    (segundos.max(0.0) * 1000.0).round() as u64
}

/// La petición de exportar entera. Falla si el estilo no existe o el trozo cae fuera.
pub fn peticion_de_exportar(
    session: &SessionData,
    pedido: &PedidoDeExportar,
    presets: &[Preset],
) -> Result<ExportRequest, String> {
    let formato = formato_de(pedido, session);
    let mut r = crate::recorder::peticion_por_defecto(session);
    r.format = formato.extension().into();
    r.file = pedido.salida.as_ref().map(|p| p.to_string_lossy().into_owned());
    if let Some(nombre) = pedido.estilo.as_deref() {
        let preset = buscar_estilo(presets, nombre)?;
        aplicar_estilo(&mut r, &preset.estilo, session);
    }
    if pedido.desde.is_some() || pedido.hasta.is_some() {
        let total = session.duration_ms();
        let desde = ms(pedido.desde.unwrap_or(0.0));
        if desde >= total {
            return Err(format!(
                "\"trim\" starts at {} s but the recording lasts {:.2} s.",
                pedido.desde.unwrap_or(0.0),
                total as f64 / 1000.0
            ));
        }
        r.desde_ms = Some(desde);
        r.hasta_ms = pedido.hasta.map(|h| ms(h).min(total));
    }
    // 0 es no acercarse (el 1 del panel) y 1 es lo más que deja el panel (tres veces).
    if let Some(z) = pedido.zoom_en_clics {
        r.zoom = (1.0 + 2.0 * z.clamp(0.0, 1.0)) as f32;
    }
    if let Some(v) = pedido.velocidad {
        r.speed = v as f32;
    }
    // Como el panel: a otra velocidad no hay sonido (acelerarlo sin cambiarle el tono es otro
    // problema), y un GIF no tiene.
    r.audio = session.has_audio && formato == Formato::Mp4 && (r.velocidad() - 1.0).abs() < 1e-6;
    Ok(r)
}

/// Lo que dura lo exportado: el trozo, a su velocidad.
pub fn segundos_exportados(session: &SessionData, r: &ExportRequest) -> f64 {
    let total = session.duration_ms();
    let inicio = session.frames.first().map_or(0, |f| f.timestamp_ms);
    let desde = r.desde_ms.unwrap_or(inicio).min(total);
    let hasta = r.hasta_ms.unwrap_or(total).min(total).max(desde);
    (hasta - desde) as f64 / 1000.0 / f64::from(r.velocidad())
}

/// El estilo con ese nombre, sin mirar mayúsculas ni espacios de los lados (como `indiceDe`
/// en `presets.ts`).
fn buscar_estilo<'a>(presets: &'a [Preset], nombre: &str) -> Result<&'a Preset, String> {
    let clave = nombre.trim().to_lowercase();
    presets.iter().find(|p| p.nombre.trim().to_lowercase() == clave).ok_or_else(|| {
        if presets.is_empty() {
            "There are no saved styles yet. Save one in the winshotx editor (\"Save style as…\"), or leave \"style\" out."
                .to_string()
        } else {
            format!(
                "No style named \"{nombre}\". Saved styles: {}.",
                presets.iter().map(|p| format!("\"{}\"", p.nombre)).collect::<Vec<_>>().join(", ")
            )
        }
    })
}

/// Los fondos con nombre que conoce el editor (`FONDOS` en `src/lib/fondos.ts`).
const FONDOS: [&str; 8] = ["blanco", "gris", "negro", "noche", "atardecer", "menta", "oceano", "coral"];

/// Un número del estilo dentro de su rango, o el de fábrica (`numero` en `presets.ts`).
fn numero(estilo: &Value, campo: &str, min: f64, max: f64, si_no: f64) -> f64 {
    estilo.get(campo).and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(si_no, |v| v.clamp(min, max))
}

fn booleano(estilo: &Value, campo: &str, si_no: bool) -> bool {
    estilo.get(campo).and_then(Value::as_bool).unwrap_or(si_no)
}

/// `fondoElegidoDe`: uno roto o con la ruta vacía no cuenta.
fn fondo_elegido(v: Option<&Value>) -> Option<FondoElegido> {
    let fondo: FondoElegido = serde_json::from_value(v?.clone()).ok()?;
    match &fondo {
        FondoElegido::Imagen { ruta } | FondoElegido::Video { ruta } if ruta.is_empty() => None,
        _ => Some(fondo),
    }
}

/// `camaraDe`: la forma tiene que ser una de las cuatro y el recorte tener tamaño; lo demás se
/// mete en su rango.
fn camara(v: Option<&Value>) -> Option<MarcoCamara> {
    let mut c: MarcoCamara = serde_json::from_value(v?.clone()).ok()?;
    let r = &mut c.recorte;
    (r.x1, r.y1, r.x2, r.y2) = (r.x1.clamp(0.0, 1.0), r.y1.clamp(0.0, 1.0), r.x2.clamp(0.0, 1.0), r.y2.clamp(0.0, 1.0));
    if r.x2 <= r.x1 || r.y2 <= r.y1 {
        return None;
    }
    let en = |v: f32, min: f32, max: f32, si_no: f32| if v.is_finite() { v.clamp(min, max) } else { si_no };
    c.tam = en(c.tam, 0.02, 1.0, 0.2);
    c.x = en(c.x, 0.0, 1.0, 0.85);
    c.y = en(c.y, 0.0, 1.0, 0.8);
    c.redondeo = c.redondeo.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 0.5));
    Some(c)
}

/// Pone un estilo guardado en la petición, como `aplicarEstilo` en `ExportPanel.tsx`: el panel
/// (`panelDe`) y lo del plan que no es de la grabación (`edicionDe`). Nunca los zooms a mano,
/// los cortes ni lo tapado. Un estilo sin cámara no quita la de esta grabación.
pub fn aplicar_estilo(r: &mut ExportRequest, estilo: &Value, session: &SessionData) {
    let zoom = numero(estilo, "zoom", 1.0, 3.0, 1.0);
    let velocidad_zoom = numero(estilo, "velocidadZoom", 0.25, 4.0, 1.0);
    let puntero = numero(estilo, "puntero", 0.0, 4.0, 1.0);
    let aire = numero(estilo, "aire", 0.0, 0.5, 0.0);
    let esquinas = numero(estilo, "esquinas", 0.0, 0.5, 0.0);
    let fondo = estilo
        .get("fondo")
        .and_then(Value::as_str)
        .filter(|f| FONDOS.contains(f))
        .unwrap_or("blanco");

    // `panelDe`: el aire y las esquinas, de proporción a píxeles del ancho que se exporta, y el
    // puntero de veces el normal a píxeles, en pasos de 4 entre 4 y 80.
    let ancho = f64::from(r.width.max(1));
    let normal = f64::from(crate::encode::estudio::puntero_normal(session.height));
    let cursor = if puntero > 0.0 { (((puntero * normal) / 4.0).round() * 4.0).clamp(4.0, 80.0) } else { 0.0 };
    let margen = if aire > 0.0 { (aire * ancho).round().clamp(4.0, 200.0) as u32 } else { 0 };
    r.zoom = zoom as f32;
    r.zoom_speed = velocidad_zoom as f32;
    // Con el cursor de Windows cocido en los fotogramas saldrían dos punteros.
    if !session.cursor_capturado {
        r.cursor = cursor as f32;
    }
    r.clicks = booleano(estilo, "aros", false);
    r.keys = booleano(estilo, "teclas", false);
    r.smooth_cursor = booleano(estilo, "suave", true);
    r.hide_idle_cursor = booleano(estilo, "esconder", false);
    r.margin = margen;
    r.corners = if margen > 0 { (esquinas * ancho).round().min(60.0) as u32 } else { 0 };
    r.background = fondo.into();
    r.shadow = booleano(estilo, "sombra", true);

    // `edicionDe`.
    let e = &mut r.edicion;
    e.fondo = fondo_elegido(estilo.get("fondoElegido"));
    e.suavizado = estilo.get("suavizado").and_then(|v| serde_json::from_value::<Suavizado>(v.clone()).ok());
    e.esconder_al_escribir = booleano(estilo, "esconderAlEscribir", false);
    e.sonido_clic = numero(estilo, "sonidoClic", 0.0, 1.0, 0.0) as f32;
    e.onda_clic = booleano(estilo, "ondaClic", false);
    e.estela = numero(estilo, "estela", 0.0, 1.0, 0.0) as f32;
    e.zoom_al_escribir = booleano(estilo, "zoomAlEscribir", false);
    if let Some(c) = camara(estilo.get("camara")) {
        e.camara = Some(c);
    }
    e.clic = estilo.get("clic").and_then(|v| serde_json::from_value::<ClicElegido>(v.clone()).ok()).filter(|c| match c {
        ClicElegido::Propio { ruta } => !ruta.is_empty(),
        _ => true,
    });
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    /// Una sesión de mentira de 1920x1080 que dura 10 s, con sonido, sin cámara.
    fn sesion() -> SessionData {
        serde_json::from_value(json!({
            "id": "abc123", "dir": "C:\\no-existe", "region": {"x":0,"y":0,"width":1920,"height":1080},
            "fps": 30, "format": "video", "hasAudio": true, "width": 1920, "height": 1080,
            "frames": [
                {"index": 0, "timestampMs": 0, "durationMs": 5000},
                {"index": 1, "timestampMs": 5000, "durationMs": 5000}
            ],
        }))
        .expect("una sesión mínima se lee")
    }

    fn ajustes() -> Settings {
        let mut a: Settings = serde_json::from_str("{}").unwrap();
        a.fps = 30;
        a.record_audio = true;
        a.record_microphone = false;
        a.record_camera = true;
        a.highlight_clicks = true;
        a
    }

    #[test]
    fn grabar_sin_decir_nada_es_lo_de_los_ajustes_sin_marimba() {
        let o = opciones_de_grabar(&OpcionesDeGrabar::default(), &ajustes(), None);
        assert_eq!((o.format.as_str(), o.fps, o.audio, o.microphone), ("video", 30, true, false));
        assert_eq!((o.camara, o.marimba, o.window, o.highlight_clicks), (Some(true), Some(false), None, true));
    }

    #[test]
    fn grabar_lo_que_pide_el_agente_manda_sobre_los_ajustes() {
        let pedido = OpcionesDeGrabar {
            camara: Some(false),
            sonido_pc: Some(false),
            microfono: Some(true),
            marimba: true,
            ..Default::default()
        };
        let o = opciones_de_grabar(&pedido, &ajustes(), Some(222));
        assert_eq!((o.audio, o.microphone, o.camara, o.marimba, o.window), (false, true, Some(false), Some(true), Some(222)));
        // Un GIF no lleva sonido aunque se pida.
        let gif = OpcionesDeGrabar { formato: Formato::Gif, sonido_pc: Some(true), microfono: Some(true), ..Default::default() };
        let o = opciones_de_grabar(&gif, &ajustes(), None);
        assert_eq!((o.format.as_str(), o.audio, o.microphone), ("gif", false, false));
    }

    #[test]
    fn exportar_sin_nada_es_lo_mismo_que_sin_editor() {
        let s = sesion();
        let pedido = PedidoDeExportar { sesion: "abc123".into(), ..Default::default() };
        let r = peticion_de_exportar(&s, &pedido, &[]).unwrap();
        let base = crate::recorder::peticion_por_defecto(&s);
        assert_eq!(format!("{r:?}"), format!("{base:?}"));
        assert_eq!((r.format.as_str(), r.audio, r.file.as_deref()), ("mp4", true, None));
        assert!((segundos_exportados(&s, &r) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn exportar_con_trozo_zoom_velocidad_y_ruta() {
        let s = sesion();
        let pedido = PedidoDeExportar {
            sesion: "abc123".into(),
            salida: Some(PathBuf::from(r"C:\v\demo.gif")),
            desde: Some(1.5),
            hasta: Some(9.0),
            zoom_en_clics: Some(0.5),
            velocidad: Some(2.0),
            ..Default::default()
        };
        let r = peticion_de_exportar(&s, &pedido, &[]).unwrap();
        assert_eq!((r.format.as_str(), r.file.as_deref()), ("gif", Some(r"C:\v\demo.gif")));
        assert_eq!((r.desde_ms, r.hasta_ms), (Some(1500), Some(9000)));
        assert_eq!((r.zoom, r.speed, r.audio), (2.0, 2.0, false));
        assert!((segundos_exportados(&s, &r) - 3.75).abs() < 1e-9);
        // Un final más allá de la grabación se queda en el final; un principio fuera, error.
        let largo = PedidoDeExportar { sesion: "abc123".into(), hasta: Some(99.0), ..Default::default() };
        assert_eq!(peticion_de_exportar(&s, &largo, &[]).unwrap().hasta_ms, Some(10_000));
        let fuera = PedidoDeExportar { sesion: "abc123".into(), desde: Some(10.0), ..Default::default() };
        assert!(peticion_de_exportar(&s, &fuera, &[]).unwrap_err().contains("lasts 10.00 s"));
        // A velocidad normal y en MP4, con su sonido.
        let normal = PedidoDeExportar { sesion: "abc123".into(), velocidad: Some(1.0), ..Default::default() };
        assert!(peticion_de_exportar(&s, &normal, &[]).unwrap().audio);
    }

    #[test]
    fn el_formato_sale_de_la_ruta_del_campo_o_de_la_grabacion() {
        let mut s = sesion();
        let p = |salida: Option<&str>, formato| PedidoDeExportar {
            salida: salida.map(PathBuf::from),
            formato,
            ..Default::default()
        };
        assert_eq!(formato_de(&p(Some(r"C:\a.GIF"), None), &s), Formato::Gif);
        assert_eq!(formato_de(&p(None, Some(Formato::Gif)), &s), Formato::Gif);
        assert_eq!(formato_de(&p(None, None), &s), Formato::Mp4);
        s.format = "gif".into();
        assert_eq!(formato_de(&p(None, None), &s), Formato::Gif);
    }

    fn estilos() -> Vec<Preset> {
        vec![
            Preset { nombre: "Redes".into(), estilo: json!({"zoom": 1.5}) },
            Preset {
                nombre: "Tutorial".into(),
                estilo: json!({
                    "zoom": 2, "velocidadZoom": 1.5, "puntero": 1.5, "aros": true, "teclas": true,
                    "suave": false, "esconder": true, "aire": 0.05, "esquinas": 0.01, "fondo": "menta",
                    "sombra": false, "fondoElegido": {"tipo": "color", "rgb": [10, 20, 30]},
                    "suavizado": "mucho", "esconderAlEscribir": true, "sonidoClic": 0.4, "ondaClic": true,
                    "estela": 0.3, "zoomAlEscribir": true,
                    "camara": {"forma": "cuadrado", "recorte": {"x1": 0, "y1": 0, "x2": 1, "y2": 1}, "tam": 0.25, "x": 0.1, "y": 0.9, "borde": true, "redondeo": 0.3},
                    "clic": {"tipo": "grave"}
                }),
            },
        ]
    }

    /// Los mismos números que `panelDe` y `edicionDe` en `presets.ts` para un vídeo de 1920 de
    /// ancho y 1080 de alto: aire 0,05 → 96 px, esquinas 0,01 → 19 px, y el puntero 1,5 veces el
    /// normal (43 px a 1080) → 64, que es el múltiplo de 4 más cercano a 64,5.
    #[test]
    fn un_estilo_guardado_sale_con_los_numeros_del_panel() {
        let s = sesion();
        let pedido = PedidoDeExportar { sesion: "abc123".into(), estilo: Some("  tutorial ".into()), ..Default::default() };
        let r = peticion_de_exportar(&s, &pedido, &estilos()).unwrap();
        assert_eq!((r.zoom, r.zoom_speed, r.cursor), (2.0, 1.5, 64.0));
        assert_eq!((r.clicks, r.keys, r.smooth_cursor, r.hide_idle_cursor), (true, true, false, true));
        assert_eq!((r.margin, r.corners, r.background.as_str(), r.shadow), (96, 19, "menta", false));
        let e = &r.edicion;
        assert_eq!(e.fondo, Some(FondoElegido::Color { rgb: [10, 20, 30] }));
        assert_eq!(e.suavizado, Some(Suavizado::Mucho));
        assert_eq!((e.esconder_al_escribir, e.sonido_clic, e.onda_clic, e.estela, e.zoom_al_escribir), (true, 0.4, true, 0.3, true));
        let c = e.camara.expect("la cámara del estilo");
        assert_eq!((c.tam, c.x, c.y, c.borde, c.redondeo), (0.25, 0.1, 0.9, true, Some(0.3)));
        assert_eq!(e.clic, Some(ClicElegido::Grave));
        // Nunca los zooms, los cortes ni lo tapado.
        assert!(e.zooms.is_none() && e.tramos.is_none() && e.tapar.is_empty());
    }

    /// Lo que falta, de fábrica (`ESTILO_DE_FABRICA`); y lo roto, también.
    #[test]
    fn un_estilo_viejo_o_roto_se_lee_de_fabrica() {
        let s = sesion();
        let mut r = crate::recorder::peticion_por_defecto(&s);
        aplicar_estilo(&mut r, &json!({"zoom": 9, "fondo": "fucsia", "aire": "mucho", "camara": {"forma": "estrella"}}), &s);
        assert_eq!((r.zoom, r.zoom_speed, r.cursor), (3.0, 1.0, 44.0));
        assert_eq!((r.margin, r.corners, r.background.as_str(), r.shadow, r.smooth_cursor), (0, 0, "blanco", true, true));
        assert!(r.edicion.fondo.is_none() && r.edicion.camara.is_none() && r.edicion.clic.is_none());
    }

    #[test]
    fn un_estilo_sin_camara_no_quita_la_de_la_grabacion() {
        let mut s = sesion();
        s.camara = Some(crate::contrato::CamaraGrabada { archivo: "camara.mp4".into(), ancho: 640, alto: 480, desfase_ms: 0 });
        let pedido = PedidoDeExportar { sesion: "abc123".into(), estilo: Some("Redes".into()), ..Default::default() };
        let r = peticion_de_exportar(&s, &pedido, &estilos()).unwrap();
        assert!(r.edicion.camara.is_some(), "la de fábrica de `peticion_por_defecto` se queda");
        assert_eq!(r.zoom, 1.5);
    }

    #[test]
    fn un_estilo_que_no_existe_dice_cuales_hay() {
        let s = sesion();
        let pedido = PedidoDeExportar { sesion: "abc123".into(), estilo: Some("Cine".into()), ..Default::default() };
        let e = peticion_de_exportar(&s, &pedido, &estilos()).unwrap_err();
        assert!(e.contains("\"Cine\"") && e.contains("\"Redes\", \"Tutorial\""), "{e}");
        assert!(peticion_de_exportar(&s, &pedido, &[]).unwrap_err().contains("no saved styles"));
    }
}
