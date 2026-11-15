/**
 * Los colores del editor que no son un token de `styles.css`, en un solo sitio.
 *
 * El editor sigue el tema de la app desde la 0.5 (antes era oscuro siempre, con sus colores
 * escritos a mano). Casi todo va con los tokens por oficio (`text-titulo`, `bg-realce`,
 * `border-linea`...); esto es lo poco que no tiene token.
 */

/**
 * Lo RELLENO de azul con letra o icono blanco encima: «Guardar», la herramienta puesta.
 *
 * Es el azul de la marca del tema claro (`#0a76c9`) en los dos temas: el del tema oscuro
 * (`#0a9bff`) es más claro, y con blanco encima se queda en 2,9 a 1 de contraste (no se lee
 * bien); este da 4,7 a 1. Para las rayas, los iconos y el texto azul vale `marca`.
 */
export const RELLENO = "bg-[#0a76c9] text-white hover:bg-[#0b84e0]";

/** Lo que falló, en rojo: más oscuro en claro para leerse (los dos pasan el 4,5 a 1). */
export const ROJO = "text-red-700 [[data-tema=oscuro]_&]:text-red-300";
