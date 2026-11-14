/**
 * La proporción fija del recorte: libre, 16:9, 1:1, 9:16 o 4:3.
 *
 * Se elige con la X mientras se arrastra (o con el botón de la barra de arriba), y manda
 * en el recorte que se dibuja, en el que se ajusta con las asas y en el que ya estaba
 * hecho cuando se cambia.
 *
 * **Todas las cuentas van en píxeles FÍSICOS**, y en enteros. El recorte vive en píxeles
 * CSS, pero lo que se lleva el usuario es el PNG o el MP4, que van en físicos: a 125 % un
 * recuadro de 101 px CSS de ancho son 126,25 físicos, y redondeando ancho y alto cada uno
 * por su lado, un 16:9 salía 126 × 71 (1,775 en vez de 1,778). Aquí el lado es siempre un
 * múltiplo exacto de la proporción (16k × 9k) y la esquina cae en un píxel entero, así que
 * `aVirtual` lo devuelve tal cual y Rust recorta justo eso.
 *
 * Vive aparte del componente por lo mismo que `pantallas.ts`: son funciones sin estado que
 * se prueban con números.
 */
import type { Rect } from "./types";

export type Proporcion = "libre" | "16:9" | "1:1" | "9:16" | "4:3";

/** El orden en que se pasan con la X. */
export const PROPORCIONES: readonly Proporcion[] = ["libre", "16:9", "1:1", "9:16", "4:3"];

/**
 * El evento con el que se ponen de acuerdo los overlays de todas las pantallas: la X
 * llega solo a la que tiene el foco, y el botón de la barra se ve en la del ratón.
 */
export const EVENTO_PROPORCION = "overlay-proporcion";

/** La siguiente de la lista (o la anterior, con Mayús), dando la vuelta. */
export function siguienteProporcion(actual: Proporcion, atras = false): Proporcion {
  const i = PROPORCIONES.indexOf(actual);
  const n = PROPORCIONES.length;
  return PROPORCIONES[(i + (atras ? n - 1 : 1)) % n];
}

/** Ancho y alto del escalón mas pequeño de esa proporción, en píxeles físicos. */
interface Unidad {
  ancho: number;
  alto: number;
}

/**
 * El escalón de la proporción: el recorte mide siempre un número entero de ellos.
 *
 * Grabando, el doble: el vídeo necesita los dos lados pares (`Rect::to_even` en Rust
 * quita el píxel que sobre), y un 16:9 de 16 × 9 por un número impar perdería una fila y
 * dejaría de ser 16:9. Con 32 × 18 los dos lados son pares siempre.
 */
export function unidadDe(proporcion: Proporcion, grabando: boolean): Unidad | null {
  if (proporcion === "libre") return null;
  const [a, b] = proporcion.split(":").map(Number);
  const veces = grabando ? 2 : 1;
  return { ancho: a * veces, alto: b * veces };
}

/** El hueco donde cabe el recorte: la pantalla, en píxeles físicos. */
export interface Limite {
  ancho: number;
  alto: number;
}

/** De un rectángulo en físicos enteros a CSS de esta pantalla. */
function aCss(x: number, y: number, ancho: number, alto: number, escala: number): Rect {
  return { x: x / escala, y: y / escala, width: ancho / escala, height: alto / escala };
}

/**
 * El recorte que se dibuja desde `origen` hasta `cursor` con la proporción fija.
 *
 * Crece hacia donde va el ratón, y cubre el punto del cursor: el lado que manda es el que
 * más ha avanzado, y el otro se estira hasta su proporción. Así el recuadro nunca se queda
 * por detrás del ratón, que es lo que hacen las herramientas de dibujo. Si no cabe en la
 * pantalla, se queda en el más grande que cabe desde el origen, sin moverse de ahí.
 *
 * Entra y sale en CSS; por dentro, físicos enteros.
 */
export function recorteConProporcion(
  origen: { x: number; y: number },
  cursor: { x: number; y: number },
  unidad: Unidad,
  limite: Limite,
  escala: number,
): Rect {
  const ox = clampEntero(Math.round(origen.x * escala), 0, limite.ancho);
  const oy = clampEntero(Math.round(origen.y * escala), 0, limite.alto);
  const dx = cursor.x * escala - ox;
  const dy = cursor.y * escala - oy;
  const haciaDerecha = dx >= 0;
  const haciaAbajo = dy >= 0;
  const cabenAncho = Math.floor((haciaDerecha ? limite.ancho - ox : ox) / unidad.ancho);
  const cabenAlto = Math.floor((haciaAbajo ? limite.alto - oy : oy) / unidad.alto);
  const pedidos = Math.round(Math.max(Math.abs(dx) / unidad.ancho, Math.abs(dy) / unidad.alto));
  const k = Math.max(0, Math.min(pedidos, cabenAncho, cabenAlto));
  const ancho = k * unidad.ancho;
  const alto = k * unidad.alto;
  return aCss(haciaDerecha ? ox : ox - ancho, haciaAbajo ? oy : oy - alto, ancho, alto, escala);
}

/** Las asas, igual que en `SelectionHandles`. */
export type Asa = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";

/**
 * El recorte que sale al tirar de un asa con la proporción fija.
 *
 * Las de las esquinas son como dibujar desde la esquina de enfrente, que se queda quieta.
 * Las de los lados mueven ese lado, y el otro eje crece o encoge a partes iguales por los
 * dos lados, para que el recuadro no se vaya de lado mientras lo agrandas; si así se sale
 * de la pantalla, se arrima al borde.
 */
export function asaConProporcion(
  base: Rect,
  asa: Asa,
  cursor: { x: number; y: number },
  unidad: Unidad,
  limite: Limite,
  escala: number,
): Rect {
  const izquierda = Math.round(base.x * escala);
  const arriba = Math.round(base.y * escala);
  const derecha = Math.round((base.x + base.width) * escala);
  const abajo = Math.round((base.y + base.height) * escala);

  if (asa.length === 2) {
    // Esquina: el ancla es la de enfrente.
    const ancla = {
      x: (asa.includes("w") ? derecha : izquierda) / escala,
      y: (asa.startsWith("n") ? abajo : arriba) / escala,
    };
    return recorteConProporcion(ancla, cursor, unidad, limite, escala);
  }

  const horizontal = asa === "e" || asa === "w";
  if (horizontal) {
    const fijo = asa === "e" ? izquierda : derecha;
    const mueve = cursor.x * escala - fijo;
    const haciaDerecha = mueve >= 0;
    const cabenAncho = Math.floor((haciaDerecha ? limite.ancho - fijo : fijo) / unidad.ancho);
    const cabenAlto = Math.floor(limite.alto / unidad.alto);
    const k = Math.max(0, Math.min(Math.round(Math.abs(mueve) / unidad.ancho), cabenAncho, cabenAlto));
    const ancho = k * unidad.ancho;
    const alto = k * unidad.alto;
    const y = clampEntero(Math.round((arriba + abajo) / 2 - alto / 2), 0, limite.alto - alto);
    return aCss(haciaDerecha ? fijo : fijo - ancho, y, ancho, alto, escala);
  }

  const fijo = asa === "s" ? arriba : abajo;
  const mueve = cursor.y * escala - fijo;
  const haciaAbajo = mueve >= 0;
  const cabenAlto = Math.floor((haciaAbajo ? limite.alto - fijo : fijo) / unidad.alto);
  const cabenAncho = Math.floor(limite.ancho / unidad.ancho);
  const k = Math.max(0, Math.min(Math.round(Math.abs(mueve) / unidad.alto), cabenAncho, cabenAlto));
  const ancho = k * unidad.ancho;
  const alto = k * unidad.alto;
  const x = clampEntero(Math.round((izquierda + derecha) / 2 - ancho / 2), 0, limite.ancho - ancho);
  return aCss(x, haciaAbajo ? fijo : fijo - alto, ancho, alto, escala);
}

/**
 * Un recorte que ya estaba hecho, pasado a otra proporción.
 *
 * Se queda en el mismo sitio (el centro no se mueve) y con más o menos la misma
 * superficie, no metido dentro del de antes: si no, pasar por las cinco con la X lo iría
 * encogiendo cada vuelta. Si no cabe, el más grande que cabe, arrimado dentro.
 */
export function cambiarProporcion(rect: Rect, unidad: Unidad, limite: Limite, escala: number): Rect {
  const ancho0 = rect.width * escala;
  const alto0 = rect.height * escala;
  const cx = (rect.x + rect.width / 2) * escala;
  const cy = (rect.y + rect.height / 2) * escala;
  const pedidos = Math.round(Math.sqrt((ancho0 * alto0) / (unidad.ancho * unidad.alto)));
  const caben = Math.min(
    Math.floor(limite.ancho / unidad.ancho),
    Math.floor(limite.alto / unidad.alto),
  );
  const k = Math.max(1, Math.min(pedidos, caben));
  const ancho = k * unidad.ancho;
  const alto = k * unidad.alto;
  const x = clampEntero(Math.round(cx - ancho / 2), 0, limite.ancho - ancho);
  const y = clampEntero(Math.round(cy - alto / 2), 0, limite.alto - alto);
  return aCss(x, y, ancho, alto, escala);
}

/**
 * Agrandar o encoger con `Alt` y las flechas, sin perder la proporción: cada pulsación es
 * un escalón (diez con `Mayús`), y la esquina de arriba a la izquierda se queda quieta.
 */
export function crecerConProporcion(
  rect: Rect,
  escalones: number,
  unidad: Unidad,
  limite: Limite,
  escala: number,
): Rect {
  const x = Math.round(rect.x * escala);
  const y = Math.round(rect.y * escala);
  const ahora = Math.round((rect.width * escala) / unidad.ancho);
  const caben = Math.min(
    Math.floor((limite.ancho - x) / unidad.ancho),
    Math.floor((limite.alto - y) / unidad.alto),
  );
  const k = Math.max(1, Math.min(ahora + escalones, caben));
  return aCss(x, y, k * unidad.ancho, k * unidad.alto, escala);
}

/**
 * El recorte que se manda a grabar, con los dos lados pares sin perder la proporción.
 *
 * Un recorte hecho en foto puede llegar aquí con un número impar de escalones (se cambió
 * a vídeo después de dibujarlo): se queda en el par de debajo, en el mismo sitio. Lo que
 * se ve y lo que se graba no cambian si ya era par, que es lo normal grabando.
 */
export function paraGrabar(rect: Rect, proporcion: Proporcion, escala: number): Rect {
  const unidad = unidadDe(proporcion, true);
  if (!unidad) return rect;
  const k = Math.max(1, Math.floor((rect.width * escala) / unidad.ancho));
  return {
    x: rect.x,
    y: rect.y,
    width: (k * unidad.ancho) / escala,
    height: (k * unidad.alto) / escala,
  };
}

/**
 * Si un recorte, ya en píxeles físicos, tiene esa proporción exacta.
 *
 * La etiqueta de la medida solo dice «16:9» cuando es verdad: con la proporción puesta
 * también hay recortes libres (la pantalla entera con `Ctrl+A`, la de antes con la R, una
 * ventana de un clic), y decirle 16:9 a una ventana de 1100 × 700 sería mentirle.
 */
export function tieneLaProporcion(ancho: number, alto: number, proporcion: Proporcion): boolean {
  if (proporcion === "libre") return false;
  const [a, b] = proporcion.split(":").map(Number);
  const w = Math.round(ancho);
  const h = Math.round(alto);
  return w > 0 && w * b === h * a;
}

function clampEntero(valor: number, minimo: number, maximo: number): number {
  return Math.min(Math.max(valor, minimo), Math.max(minimo, maximo));
}
