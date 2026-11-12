/**
 * Los estilos guardados con nombre (D2 del plan de CleanShot X y Screen Studio).
 *
 * Munito lo pidió así: «que puedas hacer presets con nombre». Un estilo es cómo se VE una
 * exportación, sin nada de la grabación: el fondo, el aire y las esquinas, el puntero, los
 * clics y el zoom. Nunca los zooms puestos a mano, los cortes ni lo tapado, que son de ese
 * vídeo y no de otro.
 *
 * **Se guarda en proporción y no en píxeles.** El panel mide el aire y las esquinas en
 * píxeles del archivo y el puntero en píxeles del fotograma, y un estilo hecho con un vídeo
 * de 1920 se aplica a uno de 800: 96 píxeles de aire son un 5 % en el primero y un 12 % en
 * el segundo. Guardado como un 5 %, sale igual de bonito en los dos.
 *
 * Rust no mira lo de dentro (`presets.rs`): lo lee esto, y lo que le falte a un estilo
 * guardado con una versión vieja, o lo que venga roto, se lee como de fábrica.
 */

import type { Background } from "./types";
import type { ClicElegido, Edicion, FondoElegido, Forma, MarcoCamara, Suavizado } from "./contrato";
import type { PresetGuardado } from "./ipc";
import { FONDOS } from "./fondos";
import { punteroNormal } from "./estudio";

export interface Estilo {
  /** Cuánto se acerca a los clics: 1 es no acercarse. */
  zoom: number;
  /** Lo deprisa que se acerca y se aleja: 1 es como siempre. */
  velocidadZoom: number;
  /** El tamaño del puntero, en veces el normal para ese vídeo. 0 es sin puntero. */
  puntero: number;
  /** Marcar los clics con un aro. */
  aros: boolean;
  /** La pastilla con los atajos. */
  teclas: boolean;
  /** Quitarle al puntero los temblores de la mano. */
  suave: boolean;
  /** Esconder el puntero cuando no se mueve. */
  esconder: boolean;
  /** El aire alrededor, de 0 a 1 sobre el ancho del vídeo. 0 es sin fondo. */
  aire: number;
  /** El radio de las esquinas, de 0 a 1 sobre el ancho del vídeo. */
  esquinas: number;
  /** El fondo de nombre, que es también el de respaldo de uno propio. */
  fondo: Background;
  sombra: boolean;
  /** Lo del plan que no es de una grabación concreta (ver `Edicion`). */
  fondoElegido: FondoElegido | null;
  suavizado: Suavizado | null;
  esconderAlEscribir: boolean;
  sonidoClic: number;
  ondaClic: boolean;
  estela: number;
  zoomAlEscribir: boolean;
  /** Cómo va la webcam: forma, recorte, tamaño y sitio. Sin la cámara en sí. */
  camara: MarcoCamara | null;
  // S10 (sonidos)
  /** Qué suena en cada clic. null es el de fábrica, el «Ratón suave». */
  clic: ClicElegido | null;
}

export interface Preset {
  nombre: string;
  estilo: Estilo;
}

/** Lo de fábrica del panel: lo que sale si no se toca nada. Lo mismo que `ExportPanel`. */
export const ESTILO_DE_FABRICA: Estilo = {
  zoom: 1,
  velocidadZoom: 1,
  puntero: 1,
  aros: false,
  teclas: false,
  suave: true,
  esconder: false,
  aire: 0,
  esquinas: 0,
  fondo: "blanco",
  sombra: true,
  fondoElegido: null,
  suavizado: null,
  esconderAlEscribir: false,
  sonidoClic: 0,
  ondaClic: false,
  estela: 0,
  zoomAlEscribir: false,
  camara: null,
  // S10 (sonidos)
  clic: null,
};

// Leer lo guardado, campo a campo: lo que no es de su tipo, de fábrica.

const esObjeto = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const numero = (v: unknown, min: number, max: number, si_no: number) =>
  typeof v === "number" && Number.isFinite(v) ? Math.min(max, Math.max(min, v)) : si_no;
const booleano = (v: unknown, si_no: boolean) => (typeof v === "boolean" ? v : si_no);
const rgb = (v: unknown): [number, number, number] | null =>
  Array.isArray(v) && v.length === 3 && v.every((c) => typeof c === "number" && c >= 0 && c <= 255)
    ? [Math.round(v[0]), Math.round(v[1]), Math.round(v[2])]
    : null;

function fondoElegidoDe(v: unknown): FondoElegido | null {
  if (!esObjeto(v)) return null;
  switch (v.tipo) {
    case "escritorio":
      return { tipo: "escritorio" };
    case "color":
    case "degradado": {
      const color = rgb(v.rgb);
      return color ? { tipo: v.tipo, rgb: color } : null;
    }
    case "imagen":
    case "video":
      return typeof v.ruta === "string" && v.ruta ? { tipo: v.tipo, ruta: v.ruta } : null;
    default:
      return null;
  }
}

const FORMAS: Forma[] = ["circulo", "cuadrado", "rectangulo", "redondeado"];

// S10 (sonidos)
function clicDe(v: unknown): ClicElegido | null {
  if (!esObjeto(v)) return null;
  if (v.tipo === "suave" || v.tipo === "grave" || v.tipo === "toque") return { tipo: v.tipo };
  return v.tipo === "propio" && typeof v.ruta === "string" && v.ruta ? { tipo: "propio", ruta: v.ruta } : null;
}

function camaraDe(v: unknown): MarcoCamara | null {
  if (!esObjeto(v) || !FORMAS.includes(v.forma as Forma) || !esObjeto(v.recorte)) return null;
  const r = v.recorte;
  const x1 = numero(r.x1, 0, 1, 0);
  const y1 = numero(r.y1, 0, 1, 0);
  const x2 = numero(r.x2, 0, 1, 1);
  const y2 = numero(r.y2, 0, 1, 1);
  if (x2 <= x1 || y2 <= y1) return null;
  return {
    forma: v.forma as Forma,
    recorte: { x1, y1, x2, y2 },
    tam: numero(v.tam, 0.02, 1, 0.2),
    x: numero(v.x, 0, 1, 0.85),
    y: numero(v.y, 0, 1, 0.8),
    borde: booleano(v.borde, false),
  };
}

/** Lee un estilo guardado. Lo que falta o no se entiende sale de fábrica, nunca un error. */
export function leerEstilo(valor: unknown): Estilo {
  const f = ESTILO_DE_FABRICA;
  if (!esObjeto(valor)) return { ...f };
  const v = valor;
  return {
    zoom: numero(v.zoom, 1, 3, f.zoom),
    velocidadZoom: numero(v.velocidadZoom, 0.25, 4, f.velocidadZoom),
    puntero: numero(v.puntero, 0, 4, f.puntero),
    aros: booleano(v.aros, f.aros),
    teclas: booleano(v.teclas, f.teclas),
    suave: booleano(v.suave, f.suave),
    esconder: booleano(v.esconder, f.esconder),
    aire: numero(v.aire, 0, 0.5, f.aire),
    esquinas: numero(v.esquinas, 0, 0.5, f.esquinas),
    fondo: FONDOS.some((x) => x.id === v.fondo) ? (v.fondo as Background) : f.fondo,
    sombra: booleano(v.sombra, f.sombra),
    fondoElegido: fondoElegidoDe(v.fondoElegido),
    suavizado: v.suavizado === "nada" || v.suavizado === "poco" || v.suavizado === "mucho" ? v.suavizado : null,
    esconderAlEscribir: booleano(v.esconderAlEscribir, f.esconderAlEscribir),
    sonidoClic: numero(v.sonidoClic, 0, 1, f.sonidoClic),
    ondaClic: booleano(v.ondaClic, f.ondaClic),
    estela: numero(v.estela, 0, 1, f.estela),
    zoomAlEscribir: booleano(v.zoomAlEscribir, f.zoomAlEscribir),
    camara: camaraDe(v.camara),
    // S10 (sonidos)
    clic: clicDe(v.clic),
  };
}

/** La lista de los ajustes, leída. Los que no tienen nombre se quedan fuera. */
export function leerPresets(lista: unknown): Preset[] {
  if (!Array.isArray(lista)) return [];
  return lista
    .filter((p): p is { nombre: string; estilo?: unknown } => esObjeto(p) && typeof p.nombre === "string" && p.nombre.trim() !== "")
    .map((p) => ({ nombre: p.nombre.trim(), estilo: leerEstilo(p.estilo) }));
}

/** Lo que se manda a Rust. */
export const paraGuardar = (lista: Preset[]): PresetGuardado[] => lista.map((p) => ({ nombre: p.nombre, estilo: p.estilo }));

const mismoNombre = (a: string, b: string) => a.trim().toLocaleLowerCase() === b.trim().toLocaleLowerCase();

/** Si ya hay uno con ese nombre (sin mirar mayúsculas), su número; si no, -1. */
export const indiceDe = (lista: Preset[], nombre: string) => lista.findIndex((p) => mismoNombre(p.nombre, nombre));

/** «Guardar estilo como…»: si ya hay uno con ese nombre se sustituye en su sitio; si no, va al final. */
export function guardarComo(lista: Preset[], nombre: string, estilo: Estilo): Preset[] {
  const limpio = nombre.trim();
  if (!limpio) return lista;
  const i = indiceDe(lista, limpio);
  if (i < 0) return [...lista, { nombre: limpio, estilo }];
  return lista.map((p, k) => (k === i ? { nombre: limpio, estilo } : p));
}

/** Cambia el nombre. No deja un nombre vacío ni uno que ya tiene otro: entonces no cambia nada. */
export function renombrar(lista: Preset[], i: number, nombre: string): Preset[] {
  const limpio = nombre.trim();
  const otro = indiceDe(lista, limpio);
  if (!limpio || !lista[i] || (otro >= 0 && otro !== i)) return lista;
  return lista.map((p, k) => (k === i ? { ...p, nombre: limpio } : p));
}

export const borrar = (lista: Preset[], i: number): Preset[] => lista.filter((_, k) => k !== i);

// Del panel al estilo y del estilo al panel.

/**
 * Lo que vive dentro de `ExportPanel` y entra en un estilo, tal cual lo tiene el panel:
 * el aire y las esquinas en píxeles del archivo, y el puntero en píxeles del fotograma.
 */
export interface EstadoDelPanel {
  zoom: number;
  velocidadZoom: number;
  cursor: number;
  aros: boolean;
  teclas: boolean;
  suave: boolean;
  esconder: boolean;
  margen: number;
  esquinas: number;
  fondo: Background;
  sombra: boolean;
}

/** Lo que miden el archivo (ancho) y la grabación (alto), para pasar de píxeles a proporción. */
export interface Medidas {
  /** El ancho del vídeo que se exporta, el de «Dimensiones». */
  ancho: number;
  /** El alto de lo grabado, que es con lo que se mide el puntero normal. */
  altoGrabado: number;
}

const redondear = (v: number, decimales = 4) => Math.round(v * 10 ** decimales) / 10 ** decimales;

/** El estilo de lo que hay puesto ahora: el panel y lo del plan que no es de la grabación. */
export function estiloDe(panel: EstadoDelPanel, edicion: Edicion | undefined, medidas: Medidas): Estilo {
  const ancho = Math.max(1, medidas.ancho);
  const e = edicion ?? {};
  return {
    zoom: panel.zoom,
    velocidadZoom: panel.velocidadZoom,
    puntero: redondear(panel.cursor / punteroNormal(medidas.altoGrabado), 2),
    aros: panel.aros,
    teclas: panel.teclas,
    suave: panel.suave,
    esconder: panel.esconder,
    aire: redondear(panel.margen / ancho),
    esquinas: redondear(panel.esquinas / ancho),
    fondo: panel.fondo,
    sombra: panel.sombra,
    fondoElegido: e.fondo ?? null,
    suavizado: e.suavizado ?? null,
    esconderAlEscribir: e.esconderAlEscribir ?? false,
    sonidoClic: e.sonidoClic ?? 0,
    ondaClic: e.ondaClic ?? false,
    estela: e.estela ?? 0,
    zoomAlEscribir: e.zoomAlEscribir ?? false,
    camara: e.camara ?? null,
    // S10 (sonidos)
    clic: e.clic ?? null,
  };
}

/**
 * Lo que un estilo le pone al panel, ya en sus píxeles y en sus pasos: el aire entre 4 y
 * 200, las esquinas hasta 60, y el puntero en pasos de 4 hasta 80, que es lo que dejan sus
 * deslizadores.
 */
export function panelDe(estilo: Estilo, medidas: Medidas): EstadoDelPanel {
  const ancho = Math.max(1, medidas.ancho);
  const cursor = Math.round((estilo.puntero * punteroNormal(medidas.altoGrabado)) / 4) * 4;
  return {
    zoom: estilo.zoom,
    velocidadZoom: estilo.velocidadZoom,
    cursor: estilo.puntero > 0 ? Math.min(80, Math.max(4, cursor)) : 0,
    aros: estilo.aros,
    teclas: estilo.teclas,
    suave: estilo.suave,
    esconder: estilo.esconder,
    margen: estilo.aire > 0 ? Math.min(200, Math.max(4, Math.round(estilo.aire * ancho))) : 0,
    esquinas: Math.min(60, Math.round(estilo.esquinas * ancho)),
    fondo: estilo.fondo,
    sombra: estilo.sombra,
  };
}

/** Lo que un estilo le pone a `edicion`. Nunca los zooms, los cortes ni lo tapado. */
export function edicionDe(estilo: Estilo): Partial<Edicion> {
  return {
    fondo: estilo.fondoElegido,
    suavizado: estilo.suavizado,
    esconderAlEscribir: estilo.esconderAlEscribir,
    sonidoClic: estilo.sonidoClic,
    ondaClic: estilo.ondaClic,
    estela: estilo.estela,
    zoomAlEscribir: estilo.zoomAlEscribir,
    camara: estilo.camara,
    // S10 (sonidos)
    clic: estilo.clic,
  };
}

/**
 * Si lo que hay puesto es ese estilo: para enseñar su nombre en vez de «Sin guardar».
 *
 * Con un poco de holgura en los números, porque aplicar un estilo redondea (el puntero va de
 * 4 en 4 píxeles, el aire a píxeles enteros) y, recién puesto, tiene que seguir diciendo su
 * nombre. Y lo que no se ve no cuenta: sin aire no hay fondo, sin puntero no hay suavizado,
 * y sin zoom da igual a qué velocidad iría.
 *
 * La cámara solo cuenta si los dos la traen. Un estilo guardado sin cámara no dice nada de
 * ella: aplicado a una grabación con cámara, la deja como estaba (`aplicarEstilo` en el
 * panel), y tiene que seguir diciendo su nombre, no «Sin guardar».
 */
export function esElMismo(a: Estilo, b: Estilo): boolean {
  const visible = (e: Estilo): Estilo => {
    const x = leerEstilo(e);
    if (x.aire === 0) Object.assign(x, { fondo: "blanco", esquinas: 0, sombra: true, fondoElegido: null });
    if (x.puntero === 0) Object.assign(x, { suave: true, esconder: false, suavizado: null, esconderAlEscribir: false });
    if (x.zoom <= 1.05) Object.assign(x, { zoom: 1, velocidadZoom: 1 });
    // S10 (sonidos): sin sonido de clic da igual cuál; y sin elegir es el suave.
    x.clic = x.sonidoClic === 0 ? null : (x.clic ?? { tipo: "suave" });
    return x;
  };
  const [x, y] = [visible(a), visible(b)];
  return (Object.keys(x) as (keyof Estilo)[]).every((k) => {
    const [p, q] = [x[k], y[k]];
    if (k === "camara" && (p === null || q === null)) return true;
    if (typeof p === "number" && typeof q === "number") return Math.abs(p - q) <= Math.max(0.002, Math.abs(p) * 0.05);
    return JSON.stringify(p) === JSON.stringify(q);
  });
}

// Las capturas sueltas (E1): «Usar un estilo para todas las capturas».

/** El fondo de las capturas tal cual lo guarda Rust (`marco_captura::MarcoCaptura`). */
export interface MarcoCapturas {
  /** Tanto por ciento del lado corto. 0 es sin fondo. */
  aire: number;
  fondo: Background;
  elegido?: FondoElegido | null;
}

/**
 * El fondo de las capturas que sale de un estilo. El aire del estilo es de 0 a 1 sobre el
 * ancho de un vídeo, y el de las capturas, un tanto por ciento del lado corto: se pasa como
 * si fuera un vídeo de 16:9, que es lo que suele ser, y se queda entre 2 y 40, que es lo
 * que admite Rust. La sombra y las esquinas de las capturas van siempre, como las decidió
 * S4: de un estilo se cogen el fondo y el aire.
 */
export function marcoCapturasDe(estilo: Estilo): MarcoCapturas {
  const aire = estilo.aire > 0 ? Math.min(40, Math.max(2, Math.round(((estilo.aire * 16) / 9) * 100))) : 0;
  return { aire, fondo: estilo.fondo, elegido: estilo.fondoElegido };
}

/** El estilo que tienen puesto las capturas, si alguno coincide; si no, -1. */
export function estiloDeLasCapturas(lista: Preset[], marco: MarcoCapturas | undefined): number {
  if (!marco || marco.aire === 0) return -1;
  return lista.findIndex((p) => {
    const de = marcoCapturasDe(p.estilo);
    return (
      de.aire === marco.aire &&
      de.fondo === marco.fondo &&
      JSON.stringify(de.elegido ?? null) === JSON.stringify(marco.elegido ?? null)
    );
  });
}
