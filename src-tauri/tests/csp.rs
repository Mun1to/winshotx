//! El overlay tapa la pantalla entera y es opaco: si el webview no puede
//! descargar el PNG de la pantalla congelada, el usuario se queda con un
//! rectangulo negro encima de todo y sin saber que hacer.
//!
//! Quien decide si puede descargarlo es la CSP. `default-src 'self'` a secas
//! parece inofensivo, pero `connect-src` hereda de ahi y el PNG no viaja por
//! 'self': viaja por el protocolo asset. En desarrollo no se nota, porque la
//! pagina la sirve Vite y la CSP ni se aplica; en el instalador, si.
//!
//! Esta prueba se pone roja si alguien vuelve a recortar esas fuentes.

/// Fuentes que la directiva connect-src tiene que permitir para que la app
/// funcione en produccion: el protocolo asset (fondo del overlay) y el ipc
/// (todas las llamadas a Rust, que en Windows van por fetch).
const FUENTES: [&str; 4] = [
    "asset:",
    "http://asset.localhost",
    "ipc:",
    "http://ipc.localhost",
];

fn directiva<'a>(csp: &'a str, nombre: &str) -> Option<&'a str> {
    csp.split(';')
        .map(str::trim)
        .find(|d| d.starts_with(nombre))
}

/// Devuelve lo que falta; vacio significa que la CSP deja trabajar a la app.
fn fuentes_que_faltan(csp: &str) -> Vec<&'static str> {
    let Some(connect) = directiva(csp, "connect-src") else {
        return FUENTES.to_vec();
    };
    FUENTES
        .iter()
        .copied()
        .filter(|fuente| !connect.contains(fuente))
        .collect()
}

fn csp_del_proyecto() -> String {
    let raw = include_str!("../tauri.conf.json");
    let config: serde_json::Value = serde_json::from_str(raw).expect("tauri.conf.json ilegible");
    config["app"]["security"]["csp"]
        .as_str()
        .expect("la configuracion no declara ninguna CSP")
        .to_string()
}

#[test]
fn la_csp_deja_al_overlay_pintar_la_pantalla_congelada() {
    let csp = csp_del_proyecto();
    let faltan = fuentes_que_faltan(&csp);
    assert!(
        faltan.is_empty(),
        "la CSP no permite {faltan:?} en connect-src, asi que el overlay saldria en negro: {csp}"
    );
}

#[test]
fn el_fondo_del_overlay_puede_cargarse_como_imagen() {
    let csp = csp_del_proyecto();
    let img = directiva(&csp, "img-src").expect("la CSP no declara img-src");
    assert!(img.contains("blob:"), "el freeze se pinta desde un blob: {img}");
    assert!(
        img.contains("asset:") && img.contains("http://asset.localhost"),
        "las miniaturas del editor se sirven por el protocolo asset: {img}"
    );
}

#[test]
fn la_csp_que_dejaba_la_pantalla_en_negro_no_pasaria_esta_prueba() {
    // Exactamente la que tenia el proyecto cuando el overlay salia negro.
    let rota = "default-src 'self'; img-src 'self' asset: http://asset.localhost data: blob:; \
                media-src 'self' asset: http://asset.localhost blob:; \
                style-src 'self' 'unsafe-inline'; script-src 'self'";
    assert_eq!(fuentes_que_faltan(rota), FUENTES.to_vec());
}
