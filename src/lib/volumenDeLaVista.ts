/**
 * S31: el volumen de la vista previa del editor, el deslizador de al lado del altavoz.
 *
 * Solo cambia lo que se OYE al mirar: el archivo que se guarda sale con el volumen de sus
 * mandos (el del estudio y los de las pistas), que no pasan por aquí. Lo que suena es el
 * volumen de cada cosa por este nivel, como la rueda de un reproductor.
 */

export interface VolumenDeLaVista {
  /** Lo que marca el deslizador cuando suena, de 0 a 1. Callada, se guarda para volver. */
  nivel: number;
  /** Callada con el altavoz o con la M. */
  callada: boolean;
}

/**
 * Sin nada guardado, a la mitad: lo que suena nace bajo y nunca a tope (regla de la casa, de
 * Munito: «tú añades un vídeo y de repente sale todo volumen»).
 */
export const NIVEL_DE_FABRICA = 0.5;

/** Lo que sube o baja cada flecha. */
const PASO = 0.1;

const CLAVE_NIVEL = "winshotx.volumenVista";
/** La de la 0.4.8, que solo guardaba el silencio: se sigue usando para lo mismo. */
const CLAVE_CALLADA = "winshotx.vistaCallada";

const entre01 = (x: number) => Math.min(1, Math.max(0, x));
/** En centésimas, que es lo que enseña el deslizador: 0,1 + 0,2 no deja 0,30000000000000004. */
const redondo = (x: number) => Math.round(x * 100) / 100;

/** Lo que se dejó en este ordenador. Sin almacenamiento, como nace. */
export function leer(): VolumenDeLaVista {
  try {
    const guardado = localStorage.getItem(CLAVE_NIVEL);
    const numero = guardado === null ? NaN : Number(guardado);
    return {
      nivel: Number.isFinite(numero) ? entre01(numero) : NIVEL_DE_FABRICA,
      callada: localStorage.getItem(CLAVE_CALLADA) === "1",
    };
  } catch {
    return { nivel: NIVEL_DE_FABRICA, callada: false };
  }
}

/** Se recuerda para el vídeo siguiente. Sin almacenamiento suena igual; solo no se recuerda. */
export function guardar(v: VolumenDeLaVista): void {
  try {
    localStorage.setItem(CLAVE_NIVEL, String(v.nivel));
    localStorage.setItem(CLAVE_CALLADA, v.callada ? "1" : "0");
  } catch {
    // Nada que hacer: la vista previa sigue sonando a lo que marca.
  }
}

/** Lo que se oye ahora, de 0 a 1: lo que enseña el deslizador. */
export function loQueSuena(v: VolumenDeLaVista): number {
  return v.callada ? 0 : v.nivel;
}

/**
 * El altavoz y la M. Si suena, se calla y se guarda el nivel; si no suena, vuelve a ese nivel.
 * Con el deslizador en 0 no hay nivel al que volver: vuelve a la mitad, no se queda mudo.
 */
export function alternar(v: VolumenDeLaVista): VolumenDeLaVista {
  if (loQueSuena(v) > 0) return { nivel: v.nivel, callada: true };
  return { nivel: v.nivel > 0 ? v.nivel : NIVEL_DE_FABRICA, callada: false };
}

/** El deslizador. Moverlo con la vista callada la vuelve a oír, a lo que marca. */
export function conNivel(_v: VolumenDeLaVista, nivel: number): VolumenDeLaVista {
  return { nivel: redondo(entre01(nivel)), callada: false };
}

/**
 * Las flechas de arriba (+1) y abajo (-1), de diez en diez. Desde lo que se oye, como el
 * deslizador: con la vista callada, subir la saca al 10 %, y bajar no toca nada (así la M
 * sigue sabiendo a qué volver).
 */
export function mover(v: VolumenDeLaVista, sentido: 1 | -1): VolumenDeLaVista {
  if (sentido < 0 && loQueSuena(v) === 0) return v;
  return conNivel(v, loQueSuena(v) + sentido * PASO);
}

/**
 * Si una flecha pulsada ahí es de otra cosa: un campo, un asa de la línea de tiempo, o un
 * panel que baja con ella. Entonces no toca el volumen.
 */
export function flechaDeOtro(destino: EventTarget | null): boolean {
  if (!(destino instanceof Element)) return false;
  if (destino.closest('input, textarea, select, [contenteditable="true"], [role="slider"]')) return true;
  for (let el: Element | null = destino; el; el = el.parentElement) {
    const { overflowY } = getComputedStyle(el);
    if ((overflowY === "auto" || overflowY === "scroll") && el.scrollHeight > el.clientHeight) return true;
  }
  return false;
}

/** El dibujo del altavoz: sin sonido, bajo (hasta la mitad) o alto. */
export function escalon(v: VolumenDeLaVista): "sin" | "bajo" | "alto" {
  const suena = loQueSuena(v);
  if (suena <= 0) return "sin";
  return suena <= 0.5 ? "bajo" : "alto";
}
