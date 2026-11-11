/**
 * Las cuentas de la pista de zooms del editor: poner, mover, estirar y quitar bloques.
 *
 * Cada bloque es un `TramoZoom` del contrato, tal cual viaja a Rust. La lista va siempre en
 * orden y sin pisarse: Rust también lo arregla si le llega mal, pero lo que se ve en la
 * pista tiene que ser lo que va a salir, y dos bloques encima uno de otro no se pueden ver.
 */
import type { Lupa, TramoZoom } from "./contrato";

/** Lo menos que dura un bloque: menos que esto, el zoom no llega a verse quieto. */
export const ZOOM_MINIMO_MS = 400;
/** Lo que dura un bloque recién puesto, si cabe. */
export const ZOOM_NUEVO_MS = 2000;
/** El nivel de un bloque nuevo cuando el panel no tiene el zoom de los clics puesto. */
export const NIVEL_NUEVO = 2;
/** Lo que más se acerca un zoom. El mismo tope que `zoom::ESCALA_MAXIMA` en Rust. */
export const NIVEL_MAXIMO = 4;
/** Cómo nace una lupa: redonda, con marco, de un cuarto del ancho y al doble. */
export const LUPA_NUEVA: Lupa = { forma: "circulo", marco: true, tam: 0.25, aumento: 2 };

/** La lista en orden de tiempo. */
export function ordenar(zooms: TramoZoom[]): TramoZoom[] {
  return [...zooms].sort((a, b) => a.desdeMs - b.desdeMs || a.hastaMs - b.hastaMs);
}

/** El bloque que hay en ese instante, o -1. */
export function bloqueEn(zooms: TramoZoom[], ms: number): number {
  return zooms.findIndex((z) => ms >= z.desdeMs && ms < z.hastaMs);
}

/** El hueco libre en que puede vivir el bloque `i`: de donde acaba el de antes a donde empieza el de después. */
export function hueco(zooms: TramoZoom[], i: number, totalMs: number): { desdeMs: number; hastaMs: number } {
  return {
    desdeMs: i > 0 ? zooms[i - 1].hastaMs : 0,
    hastaMs: i < zooms.length - 1 ? zooms[i + 1].desdeMs : totalMs,
  };
}

/**
 * Un bloque nuevo centrado en `ms`, de dos segundos o de lo que quepa en el hueco libre
 * que hay ahí. `null` si ahí ya hay un bloque o el hueco es más corto que el mínimo.
 */
export function bloqueNuevo(
  zooms: TramoZoom[],
  ms: number,
  totalMs: number,
  punto: { x: number; y: number },
  nivel: number = NIVEL_NUEVO,
): TramoZoom | null {
  if (bloqueEn(zooms, ms) >= 0) return null;
  const antes = zooms.filter((z) => z.hastaMs <= ms).reduce((m, z) => Math.max(m, z.hastaMs), 0);
  const despues = zooms.filter((z) => z.desdeMs >= ms).reduce((m, z) => Math.min(m, z.desdeMs), totalMs);
  const cabe = despues - antes;
  if (cabe < ZOOM_MINIMO_MS) return null;
  const largo = Math.min(ZOOM_NUEVO_MS, cabe);
  const desde = Math.min(Math.max(antes, ms - largo / 2), despues - largo);
  return {
    desdeMs: Math.round(desde),
    hastaMs: Math.round(desde + largo),
    nivel: Math.min(NIVEL_MAXIMO, Math.max(1.2, nivel)),
    modo: "sigue",
    x: Math.min(1, Math.max(0, punto.x)),
    y: Math.min(1, Math.max(0, punto.y)),
    instantaneo: false,
    lupa: null,
  };
}

/** Añade un bloque y devuelve la lista nueva y dónde ha quedado. */
export function anadir(zooms: TramoZoom[], nuevo: TramoZoom): { zooms: TramoZoom[]; indice: number } {
  const lista = ordenar([...zooms, nuevo]);
  return { zooms: lista, indice: lista.indexOf(nuevo) };
}

/** Mueve el bloque `i` para que empiece en `desdeMs`, con su largo y sin salirse de su hueco. */
export function mover(zooms: TramoZoom[], i: number, desdeMs: number, totalMs: number): TramoZoom[] {
  const z = zooms[i];
  if (!z) return zooms;
  const largo = z.hastaMs - z.desdeMs;
  const libre = hueco(zooms, i, totalMs);
  const desde = Math.round(Math.min(Math.max(libre.desdeMs, desdeMs), libre.hastaMs - largo));
  return zooms.map((b, k) => (k === i ? { ...b, desdeMs: desde, hastaMs: desde + largo } : b));
}

/** Lleva un borde del bloque `i` a `ms`, sin pisar a los vecinos ni bajar del mínimo. */
export function estirar(
  zooms: TramoZoom[],
  i: number,
  lado: "desde" | "hasta",
  ms: number,
  totalMs: number,
): TramoZoom[] {
  const z = zooms[i];
  if (!z) return zooms;
  const libre = hueco(zooms, i, totalMs);
  const cambio =
    lado === "desde"
      ? { desdeMs: Math.round(Math.min(Math.max(libre.desdeMs, ms), z.hastaMs - ZOOM_MINIMO_MS)) }
      : { hastaMs: Math.round(Math.max(Math.min(libre.hastaMs, ms), z.desdeMs + ZOOM_MINIMO_MS)) };
  return zooms.map((b, k) => (k === i ? { ...b, ...cambio } : b));
}

export function quitar(zooms: TramoZoom[], i: number): TramoZoom[] {
  return zooms.filter((_, k) => k !== i);
}

export function cambiar(zooms: TramoZoom[], i: number, cambio: Partial<TramoZoom>): TramoZoom[] {
  return zooms.map((b, k) => (k === i ? { ...b, ...cambio } : b));
}

/**
 * El trozo de la imagen (de 0 a 1) que enseña un zoom quieto en su punto: el mismo que
 * recorta Rust (`zoom::Camara::recorte`), metido dentro de la imagen. Es el recuadro que
 * se dibuja en la vista previa mientras se coloca un zoom fijo.
 */
export function ventanaDelZoom(
  x: number,
  y: number,
  nivel: number,
): { x1: number; y1: number; x2: number; y2: number } {
  const lado = 1 / Math.max(1, nivel);
  const x1 = Math.min(Math.max(0, x - lado / 2), 1 - lado);
  const y1 = Math.min(Math.max(0, y - lado / 2), 1 - lado);
  return { x1, y1, x2: x1 + lado, y2: y1 + lado };
}

/** Lo que mide una lupa en una imagen de `ancho` por `alto`. La misma cuenta que `lupa::medidas`. */
export function medidasDeLupa(lupa: Lupa, ancho: number, alto: number): { ancho: number; alto: number } {
  const tam = Math.min(1, Math.max(0.05, lupa.tam));
  const lado = tam * ancho;
  if (lupa.forma === "circulo" || lupa.forma === "cuadrado") {
    const l = Math.min(lado, alto);
    return { ancho: l, alto: l };
  }
  return { ancho: Math.min(lado, ancho), alto: Math.min(lado * 0.6, alto) };
}
