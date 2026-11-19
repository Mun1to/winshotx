import type { ComoParar } from "./types";

/**
 * La frase que dice cómo se para una grabación, en español (que es la clave del diccionario).
 *
 * En Linux la barra de grabar no se puede dejar fuera del vídeo, así que hay grabaciones sin
 * barra. Quien graba tiene que saber cómo parar ANTES de empezar: grabando ya no se le puede
 * enseñar nada, porque saldría en el vídeo. Por eso lo dice la pantalla de elegir zona.
 *
 * Solo nombra lo que esa persona tiene: una bandeja que su escritorio no enseña o una tecla
 * que nadie ha puesto no ayudan a parar. Y cada caso es una frase entera, no trozos cosidos:
 * el orden de las palabras cambia de un idioma a otro (ver `i18n.ts`).
 *
 * `{atajo}` y `{orden}` se quedan como marcas: quien la pinta los pone en negrita.
 */
export function fraseDeComoParar(como: ComoParar): string {
  const { bandeja, escritorio } = como;
  if (como.atajo) {
    if (bandeja && escritorio) {
      return "Para parar la grabación, pulsa {atajo}, usa el icono de winshotx en la bandeja o deja de compartir la pantalla desde el escritorio.";
    }
    if (bandeja) return "Para parar la grabación, pulsa {atajo} o usa el icono de winshotx en la bandeja.";
    if (escritorio) {
      return "Para parar la grabación, pulsa {atajo} o deja de compartir la pantalla desde el escritorio.";
    }
    return "Para parar la grabación, pulsa {atajo}.";
  }
  if (bandeja) {
    return escritorio
      ? "Para parar la grabación, usa el icono de winshotx en la bandeja o deja de compartir la pantalla desde el escritorio."
      : "Para parar la grabación, usa el icono de winshotx en la bandeja.";
  }
  // Sin tecla y sin bandeja siempre queda la orden: a nadie se le deja sin forma de parar.
  return escritorio
    ? "Para parar la grabación, deja de compartir la pantalla desde el escritorio o ejecuta {orden}."
    : "Para parar la grabación, ejecuta {orden}.";
}
