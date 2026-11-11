/**
 * Lo que comparten las piezas de la captura con scroll: el overlay, que la enciende y
 * elige la zona, y la barrita que la acompaña mientras baja.
 *
 * Va aparte de `types.ts` a propósito: así la pieza entera se lee en sus archivos.
 */

/** Cómo va la captura. Lo manda Rust a la barra (`capture::scroll::en_vivo`). */
export const EVENTO_SCROLL_PROGRESO = "winshotx://scroll-progreso";

/**
 * El interruptor de la captura con scroll, en TODAS las pantallas.
 *
 * Hay un overlay por monitor, cada uno con su React, igual que con la barra de modos
 * (`EVENTS.overlayMode`): sin esto, encenderlo en una pantalla y recortar en otra no
 * cosía nada.
 */
export const EVENTO_SCROLL_MODO = "winshotx://overlay-scroll";

/** La ventanita de la barra se abre con el overlay y este parámetro en la dirección. */
export const PARAMETRO_BARRA = "barra";

/**
 * Qué está pasando, para decirlo en la barra:
 * - `bajando`: winshotx gira la rueda sola;
 * - `tu`: quien captura ha movido el ratón y desplaza él;
 * - `perdida`: ha bajado demasiado de golpe y no se sabe dónde va (hay que subir un poco);
 * - `no-baja`: la rueda no mueve nada ahí debajo, hay que desplazar a mano;
 * - `cosiendo`: terminado, juntando la imagen para el editor.
 */
export type EstadoScroll = "bajando" | "tu" | "perdida" | "no-baja" | "cosiendo";

export interface ProgresoScroll {
  /** El alto de la imagen cosida hasta ahora, en píxeles. */
  alto: number;
  estado: EstadoScroll;
}
