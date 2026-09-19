// Sin consola detras de la ventana en las compilaciones de release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    winshotx_lib::run()
}
