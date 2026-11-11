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

/**
 * Un trozo de la grabación entre dos cortes de tijera: se queda o se quita, y va a su
 * velocidad. Las piezas cubren la grabación entera, una detrás de otra; lo que se manda a
 * Rust (`tramosDeEnvio`) son las que se quedan, dentro de las asas.
 */
export interface Pieza {
  desdeMs: number;
  hastaMs: number;
  /** 1 es la de verdad, 2 el doble de rápido. */
  velocidad: number;
  /** Quitada con las tijeras: no sale ni su imagen ni su sonido. */
  cortada: boolean;
}

/** Las velocidades que se ofrecen para un trozo. */
export const VELOCIDADES_DE_TROZO = [1, 1.5, 2, 4, 8];

/** La grabación entera, sin cortar. */
export function piezasDe(totalMs: number): Pieza[] {
  return [{ desdeMs: 0, hastaMs: totalMs, velocidad: 1, cortada: false }];
}

/** La pieza que hay en ese instante, o -1 si no hay ninguna. */
export function piezaEn(piezas: Pieza[], ms: number): number {
  const i = piezas.findIndex((p) => ms >= p.desdeMs && ms < p.hastaMs);
  if (i >= 0) return i;
  // El final exacto de la grabación es de la última pieza.
  const ultima = piezas.length - 1;
  return ultima >= 0 && ms === piezas[ultima].hastaMs ? ultima : -1;
}

/**
 * Corta con las tijeras en `ms`: la pieza que lo contiene pasa a ser dos, con su misma
 * velocidad. Tan cerca de un corte que una de las dos se quedaría sin largo, no corta.
 */
export function partir(piezas: Pieza[], ms: number): Pieza[] {
  const i = piezaEn(piezas, ms);
  if (i < 0) return piezas;
  const p = piezas[i];
  const corte = Math.round(ms);
  if (corte - p.desdeMs < TRAMO_MINIMO_MS || p.hastaMs - corte < TRAMO_MINIMO_MS) return piezas;
  return [...piezas.slice(0, i), { ...p, hastaMs: corte }, { ...p, desdeMs: corte }, ...piezas.slice(i + 1)];
}

/** Si se puede cortar ahí: lo mismo que mira `partir`, para apagar las tijeras si no. */
export function sePuedePartir(piezas: Pieza[], ms: number): boolean {
  return partir(piezas, ms) !== piezas;
}

export function cambiarPieza(piezas: Pieza[], i: number, cambio: Partial<Pieza>): Pieza[] {
  return piezas.map((p, k) => (k === i ? { ...p, ...cambio } : p));
}

/**
 * Acelera los ratos en que se escribió: corta en el principio y el final de cada uno y
 * pone esos trozos a `velocidad`. Lo que ya estaba quitado sigue quitado, y los ratos más
 * cortos que el mínimo no se tocan.
 */
export function acelerarEscritura(
  piezas: Pieza[],
  escritura: { desdeMs: number; hastaMs: number }[],
  velocidad: number,
): Pieza[] {
  let salen = piezas;
  for (const rato of escritura) {
    if (rato.hastaMs - rato.desdeMs < TRAMO_MINIMO_MS) continue;
    salen = partir(partir(salen, rato.desdeMs), rato.hastaMs);
  }
  return salen.map((p) => {
    const medio = (p.desdeMs + p.hastaMs) / 2;
    const escrito = escritura.some(
      (r) => r.hastaMs - r.desdeMs >= TRAMO_MINIMO_MS && medio >= r.desdeMs && medio < r.hastaMs,
    );
    return escrito && !p.cortada ? { ...p, velocidad } : p;
  });
}

/**
 * Lo que se manda a Rust: los trozos que se quedan dentro de las asas, con su velocidad,
 * y los seguidos que van igual juntados en uno.
 *
 * `null` si no hay nada cortado ni acelerado: entonces mandan `desdeMs` y `hastaMs` como
 * siempre, y el vídeo sale exactamente igual que antes de que existieran las tijeras.
 */
export function tramosDeEnvio(
  piezas: Pieza[],
  desdeMs: number,
  hastaMs: number,
): { desdeMs: number; hastaMs: number; velocidad: number }[] | null {
  const salen: { desdeMs: number; hastaMs: number; velocidad: number }[] = [];
  for (const p of piezas) {
    if (p.cortada) continue;
    const desde = Math.max(p.desdeMs, desdeMs);
    const hasta = Math.min(p.hastaMs, hastaMs);
    if (hasta <= desde) continue;
    const ultimo = salen[salen.length - 1];
    if (ultimo && ultimo.hastaMs === desde && ultimo.velocidad === p.velocidad) ultimo.hastaMs = hasta;
    else salen.push({ desdeMs: desde, hastaMs: hasta, velocidad: p.velocidad });
  }
  // Sin nada que se quede (la pista sin cargar todavía) se exporta lo de siempre.
  if (salen.length === 0) return null;
  const igual =
    salen.length === 1 && salen[0].velocidad === 1 && salen[0].desdeMs === desdeMs && salen[0].hastaMs === hastaMs;
  return igual ? null : salen;
}

/** Lo que dura el vídeo que sale: los trozos que se quedan, cada uno a su velocidad. */
export function duracionQueSale(piezas: Pieza[], desdeMs: number, hastaMs: number): number {
  const tramos = tramosDeEnvio(piezas, desdeMs, hastaMs);
  if (!tramos) return Math.max(0, hastaMs - desdeMs);
  return tramos.reduce((total, t) => total + (t.hastaMs - t.desdeMs) / t.velocidad, 0);
}
