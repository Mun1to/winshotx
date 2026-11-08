/**
 * Las cuentas de la línea de tiempo del editor: el tramo que se queda, por reloj.
 *
 * El recorte va en milisegundos y no en fotogramas porque una pantalla quieta es UN
 * fotograma que dura lo que dure la quietud (Windows no manda otro mientras nada cambia).
 * Por fotogramas, los diez segundos parados del final de una grabación se quitaban
 * enteros o se dejaban enteros; por reloj se cortan donde se suelte el asa.
 */
import type { FrameMeta } from "./types";

/** Lo menos que se puede dejar: menos que esto no se ve ni se oye. */
export const TRAMO_MINIMO_MS = 200;

/** Lo que dura la grabación entera: hasta donde acaba su último fotograma. */
export function duracionDe(frames: FrameMeta[]): number {
  const ultimo = frames[frames.length - 1];
  return ultimo ? ultimo.timestampMs + ultimo.durationMs : 0;
}

/**
 * El fotograma que SE ESTÁ VIENDO en ese instante: el último que empezó antes.
 *
 * No el más cercano: en una pantalla quieta, el siguiente fotograma todavía no ha
 * llegado, y lo que hay en pantalla es el anterior. Es la misma regla que usa Rust al
 * exportar (`resample_tramo`), y por eso lo que enseña el editor es lo que sale.
 */
export function indiceEn(frames: FrameMeta[], ms: number): number {
  let bajo = 0;
  let alto = frames.length - 1;
  if (alto < 0) return 0;
  while (bajo < alto) {
    const medio = (bajo + alto + 1) >> 1;
    if (frames[medio].timestampMs <= ms) bajo = medio;
    else alto = medio - 1;
  }
  return bajo;
}

/**
 * Mete el tramo dentro de la grabación, con lo mínimo de largo.
 *
 * `mueve` dice qué asa se está moviendo: la otra no se toca, y si la que se mueve se
 * acerca demasiado, se para ella, no empuja a la otra. Un asa que se mueve sola cuando
 * arrastras la de enfrente se lee como un fallo.
 */
export function limitarTramo(
  desdeMs: number,
  hastaMs: number,
  totalMs: number,
  mueve: "desde" | "hasta" | "las dos" = "las dos",
): { desdeMs: number; hastaMs: number } {
  const minimo = Math.min(TRAMO_MINIMO_MS, totalMs);
  let desde = Math.min(Math.max(0, desdeMs), totalMs);
  let hasta = Math.min(Math.max(0, hastaMs), totalMs);
  if (mueve === "desde") desde = Math.min(desde, hasta - minimo);
  else if (mueve === "hasta") hasta = Math.max(hasta, desde + minimo);
  else if (hasta - desde < minimo) hasta = Math.min(totalMs, desde + minimo);
  desde = Math.max(0, Math.min(desde, totalMs - minimo));
  hasta = Math.min(totalMs, Math.max(hasta, desde + minimo));
  return { desdeMs: Math.round(desde), hastaMs: Math.round(hasta) };
}

/**
 * Qué fotograma enseña cada hueco de la tira de miniaturas.
 *
 * Se reparten por TIEMPO, no por fotogramas: diez segundos quietos ocupan lo que ocupan
 * diez segundos, aunque sean un fotograma, y un segundo con mucho movimiento no se lleva
 * media tira. Cada hueco enseña lo que se veía en su centro.
 */
export function muestrasDeMiniaturas(frames: FrameMeta[], huecos: number): number[] {
  const total = duracionDe(frames);
  if (frames.length === 0 || huecos <= 0) return [];
  return Array.from({ length: huecos }, (_, k) => indiceEn(frames, ((k + 0.5) / huecos) * total));
}

/** El instante del fotograma de al lado, para moverse de uno en uno con las flechas. */
export function fotogramaVecino(frames: FrameMeta[], ms: number, paso: 1 | -1): number {
  if (frames.length === 0) return 0;
  const actual = indiceEn(frames, ms);
  // Hacia atrás, si estás a mitad de un fotograma largo, el primer paso te lleva a su
  // principio: es lo que se espera al pulsar «atrás» en medio de una pantalla quieta.
  if (paso === -1 && ms > frames[actual].timestampMs) return frames[actual].timestampMs;
  const vecino = Math.min(frames.length - 1, Math.max(0, actual + paso));
  return frames[vecino].timestampMs;
}

/** «0:03.2»: minutos, segundos y décimas. Las centésimas no las lee nadie. */
export function formatoTiempo(ms: number): string {
  const decimas = Math.max(0, Math.floor(ms / 100));
  const minutos = Math.floor(decimas / 600);
  const segundos = Math.floor((decimas % 600) / 10);
  return `${minutos}:${String(segundos).padStart(2, "0")}.${decimas % 10}`;
}
