/**
 * El estudio, en la vista previa: las mismas cuentas que hace Rust al exportar, para que
 * el zoom, los aros, el puntero y la pastilla se vean ANTES de exportar.
 *
 * Hasta septiembre de 2026 todo esto solo existía en el archivo exportado. Quien ponía el
 * zoom tenía que exportar, abrir el vídeo, volver, tocar el deslizador y exportar otra
 * vez. Ahora la cámara la calcula Rust (una muestra por fotograma, con `sessionCamera`) y
 * aquí solo se interpola y se dibuja: una sola fuente de verdad para el encuadre, y las
 * formas (aro, flecha, pastilla) repetidas a mano porque un canvas no sabe de `image`.
 */
import type { AtajoGrabado, ClicGrabado, MuestraCamara } from "./types";
import type { Recorte } from "./recorte";

/** Lo que se decide en el panel y se dibuja en la vista previa. */
export interface Estudio {
  /** Cuánto se acerca la cámara a cada clic. 1 es no acercarse. */
  zoom: number;
  /** Alto del puntero dibujado, en píxeles del vídeo exportado. 0 es no dibujarlo. */
  cursor: number;
  aros: boolean;
  teclas: boolean;
}

export const ESTUDIO_APAGADO: Estudio = { zoom: 1, cursor: 0, aros: false, teclas: false };

/** Cuánto dura el aro desde que se pulsa. La misma cifra que `realce::DURACION_MS`. */
export const ARO_MS = 420;
/** Cuánto se queda la pastilla del atajo. La misma que `teclas::DURACION_MS`. */
export const TECLA_MS = 1100;
/** Cuánto dura el apretón del puntero tras un clic. La misma que `estudio::PULSANDO_MS`. */
export const PULSANDO_MS = 140;

/**
 * El alto del puntero que se dibuja si nadie dice otra cosa: la misma cuenta que
 * `estudio::puntero_normal`, un 4 % del alto entre 24 y 64.
 */
export function punteroNormal(altoVideo: number): number {
  return Math.round(Math.min(64, Math.max(24, altoVideo * 0.04)));
}

/** Si el encuadre es la imagen entera, o sea sin zoom que enseñar. */
export function esEntero(r: Recorte | null): boolean {
  return !r || (r.x1 <= 0.001 && r.y1 <= 0.001 && r.x2 >= 0.999 && r.y2 >= 0.999);
}

/** La muestra que va justo antes o en ese instante, o -1 si todas son posteriores. */
function ultimaAntes<T>(lista: T[], ms: number, tiempo: (t: T) => number): number {
  let lo = 0;
  let hi = lista.length - 1;
  let mejor = -1;
  while (lo <= hi) {
    const medio = (lo + hi) >> 1;
    if (tiempo(lista[medio]) <= ms) {
      mejor = medio;
      lo = medio + 1;
    } else {
      hi = medio - 1;
    }
  }
  return mejor;
}

/**
 * Dónde mira la cámara en ese instante, interpolando entre las dos muestras que lo
 * rodean. Las muestras van una por fotograma y el vídeo se pinta a sesenta por segundo:
 * sin interpolar, el zoom avanzaría a saltos de 33 ms.
 */
export function encuadreEn(muestras: MuestraCamara[], ms: number): Recorte | null {
  if (muestras.length === 0) return null;
  const i = ultimaAntes(muestras, ms, (m) => m.ms);
  if (i < 0) return muestras[0];
  if (i >= muestras.length - 1) return muestras[muestras.length - 1];
  const a = muestras[i];
  const b = muestras[i + 1];
  const tramo = b.ms - a.ms;
  const t = tramo <= 0 ? 0 : Math.min(1, Math.max(0, (ms - a.ms) / tramo));
  return {
    x1: a.x1 + (b.x1 - a.x1) * t,
    y1: a.y1 + (b.y1 - a.y1) * t,
    x2: a.x2 + (b.x2 - a.x2) * t,
    y2: a.y2 + (b.y2 - a.y2) * t,
  };
}

/**
 * La transformación CSS que enseña ese encuadre en una caja de `ancho` por `alto`: el
 * trozo `[x1, x2] × [y1, y2]` de la imagen pasa a llenar la caja entera. Con el origen en
 * la esquina de arriba a la izquierda, escalar y luego desplazar.
 */
export function transformacion(encuadre: Recorte | null, ancho: number, alto: number): string {
  if (esEntero(encuadre) || !encuadre) return "none";
  const sx = 1 / Math.max(0.001, encuadre.x2 - encuadre.x1);
  const sy = 1 / Math.max(0.001, encuadre.y2 - encuadre.y1);
  const tx = -encuadre.x1 * ancho * sx;
  const ty = -encuadre.y1 * alto * sy;
  return `translate(${tx.toFixed(2)}px, ${ty.toFixed(2)}px) scale(${sx.toFixed(4)}, ${sy.toFixed(4)})`;
}

/**
 * Lleva un punto de la imagen (de 0 a 1) al cuadro que se ve, tras el encuadre. `null`
 * si el punto se quedó fuera: un clic que no se ve no tiene aro. Es `estudio::colocar`.
 */
export function colocar(
  u: number,
  v: number,
  encuadre: Recorte | null,
): { u: number; v: number } | null {
  if (!encuadre || esEntero(encuadre)) return { u, v };
  const nu = (u - encuadre.x1) / Math.max(1e-6, encuadre.x2 - encuadre.x1);
  const nv = (v - encuadre.y1) / Math.max(1e-6, encuadre.y2 - encuadre.y1);
  if (nu < 0 || nu > 1 || nv < 0 || nv > 1) return null;
  return { u: nu, v: nv };
}

/** Dónde estaba el ratón en ese instante: la última anotación que no es posterior. */
export function cursorEn(
  rastro: [number, number, number][],
  ms: number,
): [number, number] | null {
  const i = ultimaAntes(rastro, ms, (p) => p[0]);
  if (i < 0) return null;
  return [rastro[i][1], rastro[i][2]];
}

/** Un aro que se está viendo, con lo avanzado que va de 0 a 1. */
export interface Aro {
  x: number;
  y: number;
  derecho: boolean;
  avance: number;
}

/** Los aros que todavía se ven en ese instante. */
export function arosEn(clics: ClicGrabado[], ms: number): Aro[] {
  return clics
    .filter((c) => ms >= c.ms && ms - c.ms < ARO_MS)
    .map((c) => ({ x: c.x, y: c.y, derecho: c.derecho, avance: (ms - c.ms) / ARO_MS }));
}

/** El radio que alcanza el aro en un vídeo de ese alto. Es `realce::radio_maximo`. */
export function radioMaximo(altoVideo: number): number {
  return Math.min(44, Math.max(14, altoVideo * 0.024));
}

/** El radio del aro con ese avance: se abre desde el punto y se queda. */
export function radioDelAro(avance: number, altoVideo: number): number {
  const crecida = Math.min(1, avance * 3);
  return radioMaximo(altoVideo) * (0.28 + 0.72 * crecida);
}

/** Lo opaco que está el aro: entero al principio, apagándose al final. */
export function opacidadDelAro(avance: number): number {
  return avance < 0.35 ? 0.85 : 0.85 * (1 - (avance - 0.35) / 0.65);
}

/** La pastilla que se ve, si la hay: el último atajo que sigue vivo. */
export function atajoEn(
  atajos: AtajoGrabado[],
  ms: number,
): { texto: string; opacidad: number } | null {
  for (let i = atajos.length - 1; i >= 0; i--) {
    const a = atajos[i];
    if (ms >= a.ms && ms - a.ms < TECLA_MS) {
      const avance = (ms - a.ms) / TECLA_MS;
      return { texto: a.texto, opacidad: avance < 0.75 ? 1 : 1 - (avance - 0.75) / 0.25 };
    }
  }
  return null;
}

/** Lo que mide el puntero en ese instante: se encoge mientras dura el apretón del clic. */
export function altoDelPuntero(alto: number, clics: ClicGrabado[], ms: number): number {
  const pulsando = clics.some((c) => ms >= c.ms && ms - c.ms < PULSANDO_MS);
  return pulsando ? alto * 0.82 : alto;
}

/**
 * La flecha del puntero, de 0 a 1 donde 1 es su alto. Los mismos siete puntos que
 * `cursor::FLECHA`, medidos del puntero de Windows.
 */
export const FLECHA: [number, number][] = [
  [0.0, 0.0],
  [0.0, 0.82],
  [0.22, 0.63],
  [0.38, 1.0],
  [0.55, 0.93],
  [0.39, 0.57],
  [0.62, 0.57],
];
