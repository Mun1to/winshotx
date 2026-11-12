/**
 * Los tipos que comparten las piezas del plan de CleanShot X y Screen Studio.
 *
 * Es el espejo de `src-tauri/src/contrato.rs`: si cambia uno, cambia el otro en el mismo
 * commit. Nada de esto cambia lo que sale hoy: todo viene vacío o apagado, y una petición
 * sin estos campos exporta igual que antes.
 *
 * Las posiciones van de 0 a 1 sobre la imagen grabada entera, antes de recortar.
 */

import type { Recorte } from "./recorte";

// Lo que se guarda al grabar

/** Una marca de la tecla de zoom, en píxeles de la región grabada. */
export interface MarcaZoom {
  ms: number;
  x: number;
  y: number;
}

/**
 * Un rato en el que se escribió. Solo cuándo, nunca qué: ni una tecla, ni cuántas. De aquí
 * salen acercarse al escribir, esconder el puntero al escribir y acelerar lo escrito.
 */
export interface TramoEscritura {
  desdeMs: number;
  hastaMs: number;
  /** Dónde estaba el cursor de texto al empezar, si Windows lo dio. */
  caret: [number, number] | null;
}

/** La webcam, grabada aparte como una pista más de la sesión. */
export interface CamaraGrabada {
  /** El nombre del archivo dentro de la carpeta de la sesión. */
  archivo: string;
  ancho: number;
  alto: number;
  /** Cuándo empieza la cámara respecto al primer fotograma. Positivo: arrancó más tarde. */
  desfaseMs: number;
}

// Lo que pide el editor al exportar

export type Forma = "circulo" | "cuadrado" | "rectangulo" | "redondeado";

/** «sigue» acompaña al ratón, como hoy; «fijo» se queda quieto en su punto. */
export type ModoZoom = "sigue" | "fijo";

/** Una lupa: amplía un trozo encima del vídeo sin mover la vista. */
export interface Lupa {
  forma: Forma;
  marco: boolean;
  /** El lado mayor, de 0 a 1 sobre el ancho del vídeo exportado. */
  tam: number;
  /** Cuánto amplía: 2 es el doble. */
  aumento: number;
}

/** Un zoom de la pista de zooms. */
export interface TramoZoom {
  desdeMs: number;
  hastaMs: number;
  /** Cuánto se acerca: 2 es el doble de grande. */
  nivel: number;
  modo: ModoZoom;
  /** El punto, de 0 a 1. En «sigue» es donde arranca; en «fijo», donde se queda. */
  x: number;
  y: number;
  /** Entra de golpe en vez de acercarse poco a poco. */
  instantaneo: boolean;
  /** Con esto puesto el bloque es una lupa y no mueve la cámara. */
  lupa?: Lupa | null;
}

/** Un trozo que se queda, y a qué velocidad. Lo que cae entre dos tramos se corta. */
export interface TramoVelocidad {
  desdeMs: number;
  hastaMs: number;
  /** 1 es la de verdad. Se multiplica por la de `speed`. */
  velocidad: number;
}

/** Un fondo que no es uno de los de nombre. */
export type FondoElegido =
  | { tipo: "imagen"; ruta: string }
  | { tipo: "escritorio" }
  | { tipo: "color"; rgb: [number, number, number] }
  | { tipo: "degradado"; rgb: [number, number, number] }
  | { tipo: "video"; ruta: string };

export type Suavizado = "nada" | "poco" | "mucho";

/** Cómo va la webcam encima del vídeo. */
export interface MarcoCamara {
  forma: Forma;
  /** El trozo de la imagen de la cámara que se enseña. */
  recorte: Recorte;
  /** El lado mayor, de 0 a 1 sobre el ancho del vídeo exportado. */
  tam: number;
  /** El centro, de 0 a 1 sobre el vídeo exportado. */
  x: number;
  y: number;
  borde: boolean;
}

/** Un rectángulo tapado con mosaico durante un rato. */
export interface Tapa {
  desdeMs: number;
  hastaMs: number;
  x: number;
  y: number;
  ancho: number;
  alto: number;
}

/**
 * Lo nuevo que puede pedir el editor al exportar. Son campos sueltos de `ExportRequest`,
 * todos opcionales. Un preset con nombre guarda esto mismo.
 */
export interface Edicion {
  /** Los zooms de la pista. null o ausente: los automáticos de hoy. */
  zooms?: TramoZoom[] | null;
  /** Cuando viene, manda sobre `desdeMs` y `hastaMs`. */
  tramos?: TramoVelocidad[] | null;
  /** Cuando viene, manda sobre `background`. */
  fondo?: FondoElegido | null;
  /** Cuando viene, manda sobre `smoothCursor`. */
  suavizado?: Suavizado | null;
  esconderAlEscribir?: boolean;
  /** El volumen del sonido de clic, de 0 a 1. 0 es sin sonido. */
  sonidoClic?: number;
  ondaClic?: boolean;
  /** La estela al moverse, de 0 a 1. 0 es apagada. */
  estela?: number;
  zoomAlEscribir?: boolean;
  camara?: MarcoCamara | null;
  tapar?: Tapa[];
  // S10 (sonidos)
  /** Qué suena en cada clic cuando `sonidoClic` no es 0. null o ausente: el «Ratón suave». */
  clic?: ClicElegido | null;
  // S11 (pistas)
  /** El sonido del PC, si la grabación lo trae aparte de la voz. Ausente: manda `volume`. */
  sonidoPc?: VolumenDePista | null;
  /** Y el del micrófono. */
  microfono?: VolumenDePista | null;
}

// S11 (pistas): el sonido del PC y el micrófono, cada uno en su archivo

/** Qué pistas de sonido trae una grabación, con el nombre de su archivo en la sesión. */
export interface PistasGrabadas {
  sistema: string | null;
  microfono: string | null;
}

/** Una pista en el editor: encendida o no, y a qué volumen (1 como se grabó, 2 el doble). */
export interface VolumenDePista {
  encendida: boolean;
  volumen: number;
}

// S10 (sonidos)
/** El sonido de cada clic: uno de los tres de serie, o uno propio ya copiado a la carpeta de la app. */
export type ClicElegido = { tipo: "suave" } | { tipo: "grave" } | { tipo: "toque" } | { tipo: "propio"; ruta: string };
