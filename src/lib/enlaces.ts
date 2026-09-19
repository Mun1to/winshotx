/**
 * Los cuatro sitios de fuera a los que winshotx abre el navegador.
 *
 * Estan aqui escritos enteros para poder ensennarlos debajo del boton: un boton que dice
 * «Invitar» sin decir a donde lleva es un boton en el que no se pincha. La lista de verdad,
 * la que decide si se abre o no, esta en `src-tauri/src/enlaces.rs`, y hay una prueba en
 * Rust que comprueba que las dos dicen lo mismo.
 */

/** Invitar a un café. Es la única forma de apoyar winshotx: no hay versión de pago. */
export const CAFE = "https://buymeacoffee.com/munito";

/** El código, que es de donde sale todo lo demás. */
export const REPO = "https://github.com/Mun1to/winshotx";

/** Contar un fallo o pedir algo. */
export const FALLOS = "https://github.com/Mun1to/winshotx/issues/new";

/** La página, que es donde se prueba sin instalar nada. */
export const WEB = "https://winshotx.com";

/** Lo mismo sin `https://`, que es como se enseña debajo de un botón. */
export const comoSeLee = (url: string) => url.replace(/^https:\/\//, "");
