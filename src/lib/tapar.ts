/**
 * Lo tapado de un vídeo, en el editor (D4 del plan de CleanShot X y Screen Studio).
 *
 * Cada `Tapa` es un mosaico que tapa un rectángulo (de 0 a 1 sobre la imagen grabada)
 * durante un rato (en milisegundos de la grabación). Las pone «Tapar datos» con lo que
 * encuentra el lector de Windows; en la línea de tiempo se ven como bloques, se quitan y se
 * alargan. El mosaico de verdad lo pone Rust al exportar (`encode/tapar.rs`); aquí está lo
 * que hace falta para enseñarlo y tocarlo.
 */

import type { Tapa } from "./contrato";

/** Lo que mide como mínimo un bloque al acortarlo: menos ya no se ve en la línea de tiempo. */
export const TAPA_MINIMA_MS = 100;

/** Si está puesta en ese instante: desde su principio, sin incluir su final. Como en Rust. */
export const puesta = (t: Tapa, ms: number) => ms >= t.desdeMs && ms < t.hastaMs && t.ancho > 0 && t.alto > 0;

/** Las que están puestas en ese instante, con su número en la lista. */
export function puestasEn(tapas: Tapa[], ms: number): { tapa: Tapa; indice: number }[] {
  return tapas.flatMap((tapa, indice) => (puesta(tapa, ms) ? [{ tapa, indice }] : []));
}

/** Lo que se pisan dos rectángulos, sobre el más pequeño: 1 es que uno está dentro del otro. */
function solape(a: Tapa, b: Tapa): number {
  const ancho = Math.min(a.x + a.ancho, b.x + b.ancho) - Math.max(a.x, b.x);
  const alto = Math.min(a.y + a.alto, b.y + b.alto) - Math.max(a.y, b.y);
  const menor = Math.min(a.ancho * a.alto, b.ancho * b.alto);
  return ancho > 0 && alto > 0 && menor > 0 ? (ancho * alto) / menor : 0;
}

/**
 * Junta lo que ha encontrado una búsqueda nueva con lo que ya había: lo que ya estaba (en
 * el mismo sitio y a la vez) no se repite, así que pulsar «Tapar datos» dos veces no llena
 * la pista de bloques dobles, y lo que el usuario ya había alargado se queda como lo dejó.
 */
export function anadirTapas(antes: Tapa[], nuevas: Tapa[]): Tapa[] {
  const salen = [...antes];
  for (const n of nuevas) {
    const repetida = salen.some(
      (a) => a.desdeMs < n.hastaMs && n.desdeMs < a.hastaMs && solape(a, n) >= 0.5,
    );
    if (!repetida) salen.push(n);
  }
  return salen.sort((a, b) => a.desdeMs - b.desdeMs || a.y - b.y);
}

export const quitarTapa = (tapas: Tapa[], i: number): Tapa[] => tapas.filter((_, k) => k !== i);

/**
 * Alarga o acorta un bloque tirando de un borde. Se queda dentro de la grabación y nunca
 * más corto que `TAPA_MINIMA_MS`. A diferencia de los zooms, dos tapas sí se pueden pisar:
 * son dos datos distintos que se ven a la vez.
 */
export function estirarTapa(tapas: Tapa[], i: number, lado: "desde" | "hasta", ms: number, total: number): Tapa[] {
  const t = tapas[i];
  if (!t) return tapas;
  const cambio =
    lado === "desde"
      ? { desdeMs: Math.round(Math.min(Math.max(0, ms), t.hastaMs - TAPA_MINIMA_MS)) }
      : { hastaMs: Math.round(Math.max(Math.min(total, ms), t.desdeMs + TAPA_MINIMA_MS)) };
  return tapas.map((x, k) => (k === i ? { ...x, ...cambio } : x));
}

/**
 * En qué fila de la pista va cada bloque, para que dos que se ven a la vez no se tapen uno
 * al otro: cada uno en la primera fila libre en su rato. Como mucho `maximo` filas; los que
 * no caben van en la última, encima.
 */
export function carriles(tapas: Tapa[], maximo = 3): { carril: number[]; cuantos: number } {
  const orden = tapas.map((t, i) => ({ t, i })).sort((a, b) => a.t.desdeMs - b.t.desdeMs);
  const libreDesde: number[] = [];
  const carril = new Array<number>(tapas.length).fill(0);
  for (const { t, i } of orden) {
    let c = libreDesde.findIndex((fin) => fin <= t.desdeMs);
    if (c < 0) c = libreDesde.length < maximo ? libreDesde.length : maximo - 1;
    libreDesde[c] = Math.max(libreDesde[c] ?? 0, t.hastaMs);
    carril[i] = c;
  }
  return { carril, cuantos: Math.max(1, libreDesde.length) };
}

/**
 * El lado de cada cuadrado del mosaico, en píxeles de la imagen grabada. Es el de Rust
 * (`anotacion::lado_del_mosaico`): la octava parte del lado corto de la zona, entre 6 y 40.
 */
export function ladoDelMosaico(anchoPx: number, altoPx: number): number {
  return Math.min(40, Math.max(6, Math.floor(Math.min(anchoPx, altoPx) / 8)));
}
