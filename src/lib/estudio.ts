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
import type { AtajoGrabado, Background, ClicGrabado, MuestraCamara } from "./types";
import type { Recorte } from "./recorte";
import type { Suavizado, TramoEscritura } from "./contrato";

/** Lo que se decide en el panel y se dibuja en la vista previa. */
export interface Estudio {
  /** Cuánto se acerca la cámara a cada clic. 1 es no acercarse. */
  zoom: number;
  /** Alto del puntero dibujado, en píxeles del vídeo exportado. 0 es no dibujarlo. */
  cursor: number;
  aros: boolean;
  teclas: boolean;
  /** Quitarle al puntero los temblores de la mano (ver `sitioDelPuntero`). */
  suave: boolean;
  /** Apagar el puntero cuando lleva un rato quieto (ver `opacidadDelPuntero`). */
  esconder: boolean;
  /** A qué velocidad se acerca el zoom: 1 la de siempre. */
  velocidadZoom: number;
  /** El volumen del sonido: 1 como se grabó. */
  volumen: number;
  /** El marco de alrededor, para enseñarlo en la vista previa. null es sin marco. */
  marco: MarcoDeVista | null;
}

/**
 * El marco tal como lo dibuja la vista previa: el aire y las esquinas en tanto por uno del
 * ancho de lo que se exporta, para que crezcan y encojan con la ventana igual que la imagen.
 */
export interface MarcoDeVista {
  aire: number;
  esquinas: number;
  fondo: Background;
  sombra: boolean;
}

export const ESTUDIO_APAGADO: Estudio = {
  zoom: 1,
  cursor: 0,
  aros: false,
  teclas: false,
  suave: false,
  esconder: false,
  velocidadZoom: 1,
  volumen: 1,
  marco: null,
};

/**
 * Lo que se mira hacia atrás y hacia delante para suavizar el puntero, y lo que tiene que
 * estar quieto para esconderse y lo que tarda en irse. Las mismas cifras que
 * `estudio::SUAVE_MS`, `QUIETO_MS` y `DESVANECE_MS` en Rust.
 */
export const SUAVE_MS = 120;
export const QUIETO_MS = 1500;
export const DESVANECE_MS = 300;

/**
 * Dónde se dibuja el puntero en ese instante, en píxeles de la región grabada.
 *
 * Sin `suave`, donde estaba. Con `suave`, la media de donde estuvo un poco antes y un poco
 * después, pesando más lo más cercano: quita los temblores sin que vaya por detrás. Es la
 * misma cuenta que `estudio::sitio_del_puntero`, y sus pruebas comprueban las mismas cifras.
 */
export function sitioDelPuntero(
  rastro: [number, number, number][],
  ms: number,
  suave: boolean,
): [number, number] | null {
  return mediaDelPuntero(rastro, ms, suave ? SUAVE_MS : 0);
}

/** Lo que mira «mucho» hacia cada lado. La misma cifra que `estudio::MUCHO_MS`. */
export const MUCHO_MS = 260;

/**
 * Dónde se dibuja el puntero con el suavizado elegido: «nada» es el ratón tal cual, «poco»
 * lo de siempre y «mucho» mira más lejos pero se ciñe a los clics, para que en el instante
 * del clic esté donde se pulsó. Es `estudio::sitio_con`, con las mismas cifras.
 */
export function sitioSuavizado(
  rastro: [number, number, number][],
  clics: ClicGrabado[],
  ms: number,
  suavizado: Suavizado,
): [number, number] | null {
  if (suavizado === "nada") return mediaDelPuntero(rastro, ms, 0);
  if (suavizado === "poco") return mediaDelPuntero(rastro, ms, SUAVE_MS);
  let cerca = Infinity;
  for (const c of clics) cerca = Math.min(cerca, Math.abs(c.ms - ms));
  return mediaDelPuntero(rastro, ms, Math.min(MUCHO_MS, cerca));
}

/** La media de donde estuvo el ratón `ventana` antes y después, cada 8 ms. */
function mediaDelPuntero(
  rastro: [number, number, number][],
  ms: number,
  ventana: number,
): [number, number] | null {
  const aqui = cursorEn(rastro, ms);
  const paso = 8;
  const lado = Math.floor(ventana / paso);
  if (!aqui || lado === 0) return aqui;
  let sx = 0;
  let sy = 0;
  let pesos = 0;
  for (let k = 0; k <= 2 * lado; k++) {
    const t = ms + k * paso - lado * paso;
    if (t < 0) continue;
    const p = cursorEn(rastro, t);
    if (!p) continue;
    const peso = lado + 1 - Math.abs(k - lado);
    sx += p[0] * peso;
    sy += p[1] * peso;
    pesos += peso;
  }
  return [sx / pesos, sy / pesos];
}

/** Los instantes en que el ratón cambió de sitio. La misma cuenta que `estudio::movimientos`. */
export function movimientos(rastro: [number, number, number][]): number[] {
  const salen: number[] = [];
  let antes: string | null = null;
  for (const [t, x, y] of rastro) {
    const aqui = `${x},${y}`;
    if (aqui !== antes) {
      salen.push(t);
      antes = aqui;
    }
  }
  return salen;
}

/**
 * Lo visible que está el puntero, de 0 a 1: del todo mientras se mueve, y si lleva más de
 * `QUIETO_MS` sin moverse ni pulsar, se apaga en `DESVANECE_MS`. Un clic cuenta como moverse.
 * La misma cuenta que `estudio::opacidad_del_puntero`.
 */
export function opacidadDelPuntero(movs: number[], clics: ClicGrabado[], ms: number): number {
  const i = ultimaAntes(movs, ms, (t) => t);
  if (i < 0) return 1;
  let pulsado = 0;
  for (const c of clics) if (c.ms <= ms && c.ms > pulsado) pulsado = c.ms;
  const quieto = Math.max(0, ms - Math.max(movs[i], pulsado));
  return quieto <= QUIETO_MS ? 1 : Math.max(0, 1 - (quieto - QUIETO_MS) / DESVANECE_MS);
}

/**
 * Lo que tarda el puntero en irse al empezar a escribir y en volver, y lo que se queda a la
 * vista tras usar el ratón en mitad de lo escrito. `estudio::FUNDIDO_ESCRIBIR_MS` y `VUELVE_MS`.
 */
export const FUNDIDO_ESCRIBIR_MS = 150;
export const VUELVE_MS = 800;

/**
 * Lo visible que está el puntero según se esté escribiendo, de 0 a 1: se va con un fundido
 * en cada rato de escritura y vuelve al acabar, y un movimiento o un clic en mitad lo trae un
 * rato. Es `estudio::opacidad_al_escribir`, con las mismas cifras.
 */
export function opacidadAlEscribir(
  escritura: TramoEscritura[],
  movs: number[],
  clics: ClicGrabado[],
  ms: number,
): number {
  const f = FUNDIDO_ESCRIBIR_MS;
  const tramo = escritura.find(
    (t) => ms >= t.desdeMs && ms < Math.max(t.hastaMs, t.desdeMs) + FUNDIDO_ESCRIBIR_MS,
  );
  if (!tramo) return 1;
  const desde = tramo.desdeMs;
  const hasta = Math.max(tramo.hastaMs, tramo.desdeMs);
  const entra = (ms - desde) / f;
  const sale = ms > hasta ? 1 - (ms - hasta) / f : 1;
  const escondido = Math.min(1, Math.max(0, Math.min(entra, sale)));
  const porGesto = (g: number | null): number => {
    if (g === null || g < desde || g > hasta) return 0;
    const v = ms < g ? 1 - (g - ms) / f : ms - g <= VUELVE_MS ? 1 : 1 - (ms - g - VUELVE_MS) / f;
    return Math.min(1, Math.max(0, v));
  };
  const i = ultimaAntes(movs, ms, (t) => t);
  const movAntes = i >= 0 ? movs[i] : null;
  const movDespues = i + 1 < movs.length ? movs[i + 1] : null;
  let clicAntes: number | null = null;
  let clicDespues: number | null = null;
  for (const c of clics) {
    if (c.ms <= ms) clicAntes = clicAntes === null ? c.ms : Math.max(clicAntes, c.ms);
    else clicDespues = clicDespues === null ? c.ms : Math.min(clicDespues, c.ms);
  }
  const antes = movAntes === null ? clicAntes : clicAntes === null ? movAntes : Math.max(movAntes, clicAntes);
  const despues =
    movDespues === null ? clicDespues : clicDespues === null ? movDespues : Math.min(movDespues, clicDespues);
  const visible = Math.max(porGesto(antes), porGesto(despues));
  return 1 - Math.min(escondido, 1 - visible);
}

/** Lo que dura la onda de un clic. La misma cifra que `estudio::ONDA_MS`. */
export const ONDA_MS = 650;

/**
 * La onda de un clic con ese avance, en un vídeo de ese alto: radio, opacidad y grosor, en
 * píxeles del vídeo. Es `estudio::onda_en`.
 */
export function ondaEn(avance: number, altoVideo: number): { radio: number; opacidad: number; grosor: number } {
  const a = Math.min(1, Math.max(0, avance));
  const grande = radioMaximo(altoVideo);
  const abre = 1 - (1 - a) ** 3;
  return {
    radio: grande * (0.6 + 2 * abre),
    opacidad: 0.6 * (1 - a) * (1 - a),
    grosor: Math.min(5.5, Math.max(2.5, grande / 7)) * 0.7 * (1 - 0.5 * a),
  };
}

/** Las ondas que se ven en ese instante, con lo avanzada que va cada una. */
export function ondasEn(clics: ClicGrabado[], ms: number): Aro[] {
  return clics
    .filter((c) => ms >= c.ms && ms - c.ms < ONDA_MS)
    .map((c) => ({ x: c.x, y: c.y, derecho: c.derecho, avance: (ms - c.ms) / ONDA_MS }));
}

/** Lo que dura la estela del puntero a tope, y cuántas copias lleva. `estudio::ESTELA_MS`. */
export const ESTELA_MS = 70;
export const COPIAS_ESTELA = 4;

/**
 * Las copias de la estela del puntero: el instante de cada una y lo opaca que va, de la más
 * vieja a la más nueva. Es `estudio::estela_del_puntero`.
 */
export function estelaDelPuntero(ms: number, estela: number): { t: number; alfa: number }[] {
  const ventana = Math.round(ESTELA_MS * Math.min(1, Math.max(0, estela)));
  if (ventana === 0) return [];
  const salen: { t: number; alfa: number }[] = [];
  for (let k = COPIAS_ESTELA; k >= 1; k--) {
    const t = ms - Math.floor((ventana * k) / COPIAS_ESTELA);
    if (t < 0) continue;
    salen.push({ t, alfa: 0.55 * (1 - k / (COPIAS_ESTELA + 1)) });
  }
  return salen;
}

/**
 * Lo que se emborrona la vista previa cuando la cámara se mueve, en píxeles de la pantalla.
 *
 * Al exportar, la estela de la cámara es la media de lo que se vio por el camino desde el
 * paso anterior (`zoom::estela_de_camara`): un desenfoque en la dirección del movimiento. El
 * navegador solo desenfoca en redondo, así que aquí se pone el que da el mismo largo: un
 * camino de L píxeles promediado es un desenfoque de unos 0,29·L. Se parece; no es igual.
 */
export function desenfoqueDeCamara(
  antes: Recorte | null,
  ahora: Recorte | null,
  ancho: number,
  alto: number,
  estela: number,
): number {
  const fuerza = Math.min(1, Math.max(0, estela));
  if (fuerza <= 0) return 0;
  const a = antes ?? { x1: 0, y1: 0, x2: 1, y2: 1 };
  const b = ahora ?? { x1: 0, y1: 0, x2: 1, y2: 1 };
  const w = Math.max(1e-6, Math.abs(b.x2 - b.x1));
  const h = Math.max(1e-6, Math.abs(b.y2 - b.y1));
  const dx = (Math.max(Math.abs(a.x1 - b.x1), Math.abs(a.x2 - b.x2)) / w) * ancho;
  const dy = (Math.max(Math.abs(a.y1 - b.y1), Math.abs(a.y2 - b.y2)) / h) * alto;
  const largo = Math.max(dx, dy) * fuerza;
  return largo < 2 ? 0 : 0.29 * largo;
}

/**
 * Lo que mide la vista previa con el marco puesto: la imagen, el aire y el radio de las
 * esquinas, en píxeles de pantalla, cabiendo entero en el hueco.
 *
 * El aire va en tanto por uno del ancho de la imagen, así que lo que cabe es una imagen de
 * ancho `w` con `aire · w` por cada lado: de ahí salen los dos topes.
 */
export function medidasConMarco(
  huecoAncho: number,
  huecoAlto: number,
  ancho: number,
  alto: number,
  marco: MarcoDeVista | null,
): { width: number; height: number; aire: number; radio: number } {
  const proporcion = ancho / Math.max(1, alto);
  const m = marco && marco.aire > 0 ? marco.aire : 0;
  if (huecoAncho <= 0 || huecoAlto <= 0 || ancho <= 0 || alto <= 0) {
    return { width: 0, height: 0, aire: 0, radio: 0 };
  }
  const w = Math.max(1, Math.min(huecoAncho / (1 + 2 * m), huecoAlto / (1 / proporcion + 2 * m)));
  return {
    width: Math.round(w),
    height: Math.round(w / proporcion),
    aire: Math.round(m * w),
    radio: m > 0 && marco ? Math.round(marco.esquinas * w) : 0,
  };
}

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

/** Las tres formas del puntero, con los mismos números que `cursor::Forma` en Rust. */
export type Forma = "flecha" | "texto" | "mano";

/** Qué forma tenía el puntero en ese instante. Sin nada anotado, la flecha. */
export function formaEn(formas: [number, number][] | undefined, ms: number): Forma {
  if (!formas || formas.length === 0) return "flecha";
  const i = ultimaAntes(formas, ms, (f) => f[0]);
  if (i < 0) return "flecha";
  return formas[i][1] === 1 ? "texto" : formas[i][1] === 2 ? "mano" : "flecha";
}

/** Uno que no se pudo leer al grabar: en su tramo se dibuja la forma a mano. */
const ILEGIBLE = 4294967295;

/** Qué puntero leído estaba puesto en ese instante, o `null` si ninguno. */
export function punteroEn(cambios: [number, number][] | undefined, ms: number): number | null {
  if (!cambios || cambios.length === 0) return null;
  const i = ultimaAntes(cambios, ms, (c) => c[0]);
  if (i < 0) return null;
  const id = cambios[i][1];
  return id === ILEGIBLE ? null : id;
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
 * `cursor::FLECHA`, medidos del puntero de Windows. El punto caliente es la punta, (0, 0).
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

/** La barra de texto: una I con sus dos remates. El punto caliente es su centro. */
export const BARRA_TEXTO: [number, number][] = [
  [0.2, 0.0],
  [0.8, 0.0],
  [0.8, 0.1],
  [0.55, 0.1],
  [0.55, 0.9],
  [0.8, 0.9],
  [0.8, 1.0],
  [0.2, 1.0],
  [0.2, 0.9],
  [0.45, 0.9],
  [0.45, 0.1],
  [0.2, 0.1],
];

/**
 * La manita de los enlaces: el índice arriba y los otros tres dedos recogidos. El punto
 * caliente es la yema del índice.
 */
export const MANO: [number, number][] = [
  [0.3, 0.0],
  [0.44, 0.0],
  [0.44, 0.42],
  [0.58, 0.4],
  [0.58, 0.48],
  [0.72, 0.46],
  [0.72, 0.54],
  [0.84, 0.52],
  [0.84, 0.6],
  [0.82, 0.8],
  [0.7, 1.0],
  [0.3, 1.0],
  [0.1, 0.78],
  [0.0, 0.55],
  [0.06, 0.48],
  [0.18, 0.52],
  [0.3, 0.62],
];

/** El dibujo de cada forma: sus puntos, dónde está su punto caliente y de qué color va. */
export function dibujoDe(forma: Forma): {
  puntos: [number, number][];
  caliente: [number, number];
  claro: boolean;
} {
  switch (forma) {
    case "texto":
      return { puntos: BARRA_TEXTO, caliente: [0.5, 0.5], claro: false };
    case "mano":
      return { puntos: MANO, caliente: [0.37, 0.0], claro: true };
    default:
      return { puntos: FLECHA, caliente: [0, 0], claro: false };
  }
}
