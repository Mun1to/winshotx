/**
 * La webcam en el Estudio: dónde cae la burbuja de la cámara y qué trozo de la cámara se ve
 * dentro, en la vista previa del editor.
 *
 * Es el espejo de `src-tauri/src/encode/camara_estudio.rs`, que la pinta al exportar: las
 * mismas cuentas con los mismos números, y las mismas pruebas a los dos lados. Si cambia
 * una, cambia la otra en el mismo commit. Así lo que se coloca arrastrando es lo que sale.
 *
 * Las posiciones del marco (`x`, `y`, `tam`) van de 0 a 1 sobre el vídeo EXPORTADO, con su
 * aire alrededor: la cámara va encima de todo, también del fondo.
 */

import type { Forma, MarcoCamara } from "./contrato";
import type { Recorte } from "./recorte";

/** La cámara de una sesión, como la da Rust (`camara_de_la_sesion`). */
export interface CamaraDeLaSesion {
  /** La ruta entera de `camara.mp4`, para `convertFileSrc`. */
  ruta: string;
  ancho: number;
  alto: number;
  /** Cuándo arrancó la cámara respecto al primer fotograma. Positivo: más tarde. */
  desfaseMs: number;
  /** El instante del primer fotograma de la pantalla. */
  primerMs: number;
}

/** Una caja en píxeles del lienzo: la esquina de arriba a la izquierda y lo que mide. */
export interface Caja {
  x: number;
  y: number;
  ancho: number;
  alto: number;
}

export interface Medida {
  ancho: number;
  alto: number;
}

/** El lado mayor más pequeño que se acepta: menos es un punto. */
const TAM_MINIMO = 0.05;
const RECORTE_MINIMO = 0.02;
const RADIO_REDONDEADO = 0.18;
/** De fábrica: el lado mayor es el 18 % del ancho, a un 3 % del borde. */
export const TAM_DE_FABRICA = 0.18;
export const AIRE_DE_FABRICA = 0.03;
/** Lo que deja el mando del tamaño y la esquina de arrastrar. */
export const TAM_MANDO_MINIMO = 0.08;
export const TAM_MANDO_MAXIMO = 0.5;

export const SIN_RECORTE: Recorte = { x1: 0, y1: 0, x2: 1, y2: 1 };

const dentro = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v));

/** El recorte ordenado y dentro de la imagen; uno roto vale la imagen entera. */
export function recorteLimpio(r: Recorte): Recorte {
  if (![r.x1, r.y1, r.x2, r.y2].every(Number.isFinite)) return SIN_RECORTE;
  const x1 = dentro(Math.min(r.x1, r.x2), 0, 1);
  const x2 = dentro(Math.max(r.x1, r.x2), 0, 1);
  const y1 = dentro(Math.min(r.y1, r.y2), 0, 1);
  const y2 = dentro(Math.max(r.y1, r.y2), 0, 1);
  if (x2 - x1 < RECORTE_MINIMO || y2 - y1 < RECORTE_MINIMO) return SIN_RECORTE;
  return { x1, y1, x2, y2 };
}

/** Si el recorte deja fuera algo de la cámara. */
export function recorta(r: Recorte): boolean {
  const l = recorteLimpio(r);
  return l.x1 > 0.001 || l.y1 > 0.001 || l.x2 < 0.999 || l.y2 < 0.999;
}

/**
 * Ancho entre alto de la burbuja: el círculo y el cuadrado son cuadrados, y los rectángulos
 * tienen la proporción del trozo de cámara que se enseña.
 */
export function proporcion(marco: MarcoCamara, camara: Medida): number {
  if (marco.forma === "circulo" || marco.forma === "cuadrado") return 1;
  const r = recorteLimpio(marco.recorte);
  const ancho = (r.x2 - r.x1) * Math.max(1, camara.ancho);
  const alto = (r.y2 - r.y1) * Math.max(1, camara.alto);
  return dentro(ancho / Math.max(alto, Number.EPSILON), 0.2, 5);
}

/** Lo que mide la burbuja en un lienzo, sin colocarla: el lado mayor es `tam` por el ancho. */
function medidaDeLaBurbuja(marco: MarcoCamara, camara: Medida, lienzo: Medida): Medida {
  const lw = Math.max(1, lienzo.ancho);
  const lh = Math.max(1, lienzo.alto);
  const tam = Number.isFinite(marco.tam) ? dentro(marco.tam, TAM_MINIMO, 1) : TAM_DE_FABRICA;
  const lado = tam * lw;
  const r = proporcion(marco, camara);
  let [ancho, alto] = r >= 1 ? [lado, lado / r] : [lado * r, lado];
  const encoge = Math.min(lw / ancho, lh / alto, 1);
  ancho *= encoge;
  alto *= encoge;
  return { ancho, alto };
}

/**
 * Dónde cae la burbuja en un lienzo de `lienzo` píxeles. Su centro es `x` e `y`, metido
 * para que la burbuja entera quede dentro.
 */
export function caja(marco: MarcoCamara, camara: Medida, lienzo: Medida): Caja {
  const lw = Math.max(1, lienzo.ancho);
  const lh = Math.max(1, lienzo.alto);
  const { ancho, alto } = medidaDeLaBurbuja(marco, camara, lienzo);
  const centro = (v: number, defecto: number) => (Number.isFinite(v) ? dentro(v, 0, 1) : defecto);
  const cx = dentro(centro(marco.x, 0.85) * lw, ancho / 2, lw - ancho / 2);
  const cy = dentro(centro(marco.y, 0.8) * lh, alto / 2, lh - alto / 2);
  return { x: cx - ancho / 2, y: cy - alto / 2, ancho, alto };
}

/**
 * El trozo de la cámara que se ve dentro de la burbuja, de 0 a 1: el recorte, centrado y
 * rebajado por el lado que sobre para llenar la burbuja sin deformar la cara.
 */
export function recorteVisible(marco: MarcoCamara, camara: Medida): Recorte {
  const r = recorteLimpio(marco.recorte);
  const p = proporcion(marco, camara);
  const cw = Math.max(1, camara.ancho);
  const ch = Math.max(1, camara.alto);
  const ancho = (r.x2 - r.x1) * cw;
  const alto = (r.y2 - r.y1) * ch;
  if (ancho / alto > p) {
    const sobra = (r.x2 - r.x1 - (alto * p) / cw) / 2;
    return { x1: r.x1 + sobra, y1: r.y1, x2: r.x2 - sobra, y2: r.y2 };
  }
  const sobra = (r.y2 - r.y1 - ancho / p / ch) / 2;
  return { x1: r.x1, y1: r.y1 + sobra, x2: r.x2, y2: r.y2 - sobra };
}

/** El grosor del borde blanco: un 2 % del lado corto, entre 2 y 8 píxeles. */
export function grosorDelBorde(c: Caja): number {
  return dentro(Math.min(c.ancho, c.alto) * 0.02, 2, 8);
}

/** El radio de las esquinas de la burbuja, en píxeles. */
export function radio(forma: Forma, c: Caja): number {
  const corto = Math.min(c.ancho, c.alto);
  if (forma === "circulo") return corto / 2;
  if (forma === "redondeado") return corto * RADIO_REDONDEADO;
  return 0;
}

/** De fábrica: en círculo, sin recortar ni borde, pequeña y abajo a la derecha. */
export function marcoDeFabrica(ancho: number, alto: number): MarcoCamara {
  const medio = TAM_DE_FABRICA / 2;
  return {
    forma: "circulo",
    recorte: SIN_RECORTE,
    tam: TAM_DE_FABRICA,
    x: 1 - AIRE_DE_FABRICA - medio,
    y: dentro(1 - ((AIRE_DE_FABRICA + medio) * Math.max(1, ancho)) / Math.max(1, alto), 0, 1),
    borde: false,
  };
}

/** En qué instante de `camara.mp4` está la cámara cuando la grabación está en `ms`. */
export function instanteDeLaCamara(ms: number, primerMs: number, desfaseMs: number): number {
  return ms - primerMs - desfaseMs;
}

/**
 * Qué hace el vídeo de la cara en la vista previa cuando la grabación está en `ms`: a qué
 * segundo de `camara.mp4` va (`meta`) y si se reproduce o se queda quieto en él.
 *
 * Una cámara tarda en encenderse (la Insta360 de Munito, unos 2 s), así que al principio de
 * la grabación todavía no hay cara: hasta que arranca se queda QUIETA en su primera imagen,
 * como en el MP4. Antes se reproducía desde 0 y, al alejarse de 0, se la devolvía a 0: el
 * mismo trocito una y otra vez durante esos 2 s («se traba y se repite», 12-nov-2026).
 */
export function pasoDeLaCara(
  ms: number,
  primerMs: number,
  desfaseMs: number,
  duraS: number,
  pantallaSonando: boolean,
): { meta: number; suena: boolean } {
  const quiere = instanteDeLaCamara(ms, primerMs, desfaseMs) / 1000;
  // Acabada la cámara se queda en su última imagen, como en el archivo.
  const meta = Math.min(Math.max(0, quiere), Math.max(0, duraS - 0.04));
  return { meta, suena: pantallaSonando && quiere >= 0 && quiere < duraS };
}

/** El marco con la burbuja llevada a ese centro (en píxeles del lienzo), sin salirse. */
export function mover(marco: MarcoCamara, camara: Medida, lienzo: Medida, cx: number, cy: number): MarcoCamara {
  const lw = Math.max(1, lienzo.ancho);
  const lh = Math.max(1, lienzo.alto);
  const { ancho, alto } = medidaDeLaBurbuja(marco, camara, lienzo);
  return {
    ...marco,
    x: dentro(cx, ancho / 2, lw - ancho / 2) / lw,
    y: dentro(cy, alto / 2, lh - alto / 2) / lh,
  };
}

/** La esquina por la que se cambia de tamaño: la que mira al centro del vídeo. */
export function esquinaDeAgarre(c: Caja, lienzo: Medida): { haciaX: -1 | 1; haciaY: -1 | 1 } {
  return {
    haciaX: c.x + c.ancho / 2 > lienzo.ancho / 2 ? -1 : 1,
    haciaY: c.y + c.alto / 2 > lienzo.alto / 2 ? -1 : 1,
  };
}

/**
 * El marco con la burbuja estirada desde una esquina hasta el puntero, con la esquina de
 * enfrente quieta. `hacia` es hacia dónde mira la esquina que se arrastra (-1 izquierda o
 * arriba, 1 derecha o abajo).
 */
export function redimensionar(
  marco: MarcoCamara,
  camara: Medida,
  lienzo: Medida,
  antes: Caja,
  hacia: { haciaX: -1 | 1; haciaY: -1 | 1 },
  puntero: { x: number; y: number },
): MarcoCamara {
  const lw = Math.max(1, lienzo.ancho);
  const r = proporcion(marco, camara);
  // La esquina de enfrente, que no se mueve.
  const fijaX = hacia.haciaX > 0 ? antes.x : antes.x + antes.ancho;
  const fijaY = hacia.haciaY > 0 ? antes.y : antes.y + antes.alto;
  const ancho = Math.max(Math.abs(puntero.x - fijaX), Math.abs(puntero.y - fijaY) * r);
  const ladoMayor = r >= 1 ? ancho : ancho / r;
  const tam = dentro(ladoMayor / lw, TAM_MANDO_MINIMO, TAM_MANDO_MAXIMO);
  const nuevo = { ...marco, tam };
  const m = medidaDeLaBurbuja(nuevo, camara, lienzo);
  return mover(nuevo, camara, lienzo, fijaX + (hacia.haciaX * m.ancho) / 2, fijaY + (hacia.haciaY * m.alto) / 2);
}
