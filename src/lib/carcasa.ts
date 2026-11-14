/**
 * S25 (marco): la captura dentro de una ventana de Windows 11 o de un portátil.
 *
 * Es el espejo de `src-tauri/src/encode/carcasa.rs`, con las MISMAS cuentas: Rust pinta la
 * carcasa en el archivo y la vista previa la dibuja en SVG con estas medidas. Las pruebas de
 * los dos lados comprueban los mismos números (`carcasa.test.ts` y
 * `las_medidas_son_las_mismas_que_dibuja_el_editor`): si cambia uno, cambia el otro.
 *
 * La captura no se encoge: la carcasa crece por fuera, como el aire del fondo.
 */

/** Lo que se elige en el panel y se guarda en un estilo. El tema de la ventana se pone al pedir. */
export type CarcasaElegida = "ventana" | "portatil";

/** Los tres marcos, en el orden en que se ofrecen: sin marco, la ventana y el portátil. Sin submenús. */
export const MARCOS: { id: CarcasaElegida | null; nombre: string }[] = [
  { id: null, nombre: "Ninguno" },
  { id: "ventana", nombre: "Ventana" },
  { id: "portatil", nombre: "Portátil" },
];

/** Lo que viaja a Rust en la petición: la ventana ya con su tema. */
export type Carcasa = { tipo: "ventana"; oscura: boolean } | { tipo: "portatil" };

/** Un rectángulo con cada esquina a su radio: arriba izquierda, arriba derecha, abajo derecha, abajo izquierda. */
export interface Forma {
  x: number;
  y: number;
  ancho: number;
  alto: number;
  radios: [number, number, number, number];
}

/** Lo que mide la carcasa alrededor de una captura, en píxeles del archivo. */
export interface Geometria {
  /** La pieza entera, sin el aire del fondo. */
  ancho: number;
  alto: number;
  /** Dónde va la captura dentro de la pieza. */
  x: number;
  y: number;
  /** Las esquinas de la captura, en el orden de `Forma`. */
  radios: [number, number, number, number];
  /** Lo que echa sombra: la ventana entera, o la tapa y la base del portátil. */
  sombra: Forma[];
}

/** Las medidas de Windows 11 a 100 %. Las mismas que `carcasa.rs`. */
const LADO_DE_REFERENCIA = 1920;
const BARRA = 32;
const BOTON = 46;
const GLIFO = 10;
export const TRAZO = 1;
const RADIO_VENTANA = 8;

const BISEL_LADO = 2.4;
const BISEL_ARRIBA = 3.0;
const BISEL_ABAJO = 3.0;
const RADIO_TAPA = 2.0;
const CANTO = 0.22;
const VUELO = 4.5;
const ALTO_BASE = 1.5;
const MUESCA = 14.0;
const CAMARA = 0.32;

/** 1 hasta 1920 de lado largo; más grande, crece con la captura como con el escalado de Windows. */
export const escala = (ancho: number, alto: number) => Math.max(1, Math.max(ancho, alto) / LADO_DE_REFERENCIA);

/** Un 1 % del lado largo de la pantalla del portátil, y nunca menos que el de una de 640. */
export const unidadDelPortatil = (ancho: number, alto: number) => Math.max(ancho, alto, 640) / 100;

const borde = (s: number) => Math.max(1, Math.round(s));
const px = (unidades: number, u: number) => Math.max(1, Math.round(unidades * u));

const recta = (x: number, y: number, ancho: number, alto: number, radio: number): Forma => ({
  x,
  y,
  ancho,
  alto,
  radios: [radio, radio, radio, radio],
});

export function geometria(carcasa: CarcasaElegida, ancho: number, alto: number): Geometria {
  if (carcasa === "ventana") {
    const s = escala(ancho, alto);
    const b = borde(s);
    const barra = Math.round(BARRA * s);
    const radio = Math.round(RADIO_VENTANA * s);
    const pieza = { ancho: ancho + 2 * b, alto: alto + barra + 2 * b };
    const dentro = Math.max(0, radio - b);
    return {
      ...pieza,
      x: b,
      y: b + barra,
      radios: [0, 0, dentro, dentro],
      sombra: [recta(0, 0, pieza.ancho, pieza.alto, radio)],
    };
  }
  const u = unidadDelPortatil(ancho, alto);
  const [lado, arriba, abajo] = [px(BISEL_LADO, u), px(BISEL_ARRIBA, u), px(BISEL_ABAJO, u)];
  const tapa = { ancho: ancho + 2 * lado, alto: alto + arriba + abajo };
  const vuelo = px(VUELO, u);
  const altoBase = px(ALTO_BASE, u);
  const pieza = { ancho: tapa.ancho + 2 * vuelo, alto: tapa.alto + altoBase };
  const r = Math.round(RADIO_TAPA * u);
  const rb = Math.round(altoBase * 0.7);
  return {
    ...pieza,
    x: vuelo + lado,
    y: arriba,
    radios: [0, 0, 0, 0],
    sombra: [
      { x: vuelo, y: 0, ancho: tapa.ancho, alto: tapa.alto, radios: [r, r, r * 0.25, r * 0.25] },
      { x: 0, y: tapa.alto, ancho: pieza.ancho, alto: altoBase, radios: [0, 0, rb, rb] },
    ],
  };
}

/** Las cajas de los tres dibujos de la barra (minimizar, maximizar, cerrar): x, y y lado. */
export function botones(g: Geometria, ancho: number, alto: number): [number, number, number][] {
  const s = escala(ancho, alto);
  const b = borde(s);
  const barra = Math.round(BARRA * s);
  const boton = BOTON * s;
  const lado = Math.round(GLIFO * s);
  const derecha = g.ancho - b;
  const cy = b + barra / 2;
  return [0, 1, 2].map((i) => {
    const cx = derecha - boton * (3 - i) + boton / 2;
    return [Math.round(cx - lado / 2), Math.round(cy - lado / 2), lado] as [number, number, number];
  });
}

/** La barra, sus dibujos y el borde de la ventana, como `colores_de_la_ventana`. */
export const coloresDeLaVentana = (oscura: boolean) =>
  oscura
    ? { barra: "rgb(32 32 32)", glifo: "rgb(255 255 255)", borde: "rgb(58 58 58)" }
    : { barra: "rgb(243 243 243)", glifo: "rgb(26 26 26)", borde: "rgb(204 204 204)" };

/** Los del portátil, como las constantes de `carcasa.rs`. */
export const COLORES_DEL_PORTATIL = {
  canto: "rgb(92 94 100)",
  bisel: "rgb(12 12 14)",
  camara: "rgb(38 40 46)",
  baseArriba: "rgb(226 228 232)",
  baseAbajo: "rgb(150 153 160)",
  muesca: "rgb(132 135 142)",
};

/** Las medidas de dentro del portátil que dibuja la vista previa: el canto, la muesca y la cámara. */
export function detallesDelPortatil(g: Geometria, ancho: number, alto: number) {
  const u = unidadDelPortatil(ancho, alto);
  const [tapa, base] = g.sombra;
  const canto = Math.max(1, Math.round(CANTO * u));
  const muescaAncho = Math.min(Math.round(MUESCA * u), base.ancho * 0.4);
  const muescaAlto = Math.max(1, Math.round(base.alto * 0.32));
  const camara = Math.max(1, CAMARA * u);
  return {
    bisel: {
      x: tapa.x + canto,
      y: tapa.y + canto,
      ancho: tapa.ancho - 2 * canto,
      alto: tapa.alto - 2 * canto,
      radios: tapa.radios.map((r) => Math.max(0, r - canto)) as Forma["radios"],
    } satisfies Forma,
    muesca: {
      x: Math.round(base.x + (base.ancho - muescaAncho) / 2),
      y: base.y,
      ancho: muescaAncho,
      alto: muescaAlto,
      radios: [0, 0, muescaAlto, muescaAlto],
    } satisfies Forma,
    camara: { cx: tapa.x + tapa.ancho / 2, cy: g.y / 2, r: camara },
  };
}

/** El contorno de una forma en SVG, cada esquina con su arco, en píxeles por `k`. */
export function caminoDe(f: Forma, k = 1): string {
  const [x, y, w, h] = [f.x * k, f.y * k, f.ancho * k, f.alto * k];
  const tope = Math.min(w, h) / 2;
  const [a, b, c, d] = f.radios.map((r) => Math.min(tope, Math.max(0, r * k)));
  return [
    `M${x + a},${y}`,
    `H${x + w - b}`,
    b ? `A${b},${b} 0 0 1 ${x + w},${y + b}` : "",
    `V${y + h - c}`,
    c ? `A${c},${c} 0 0 1 ${x + w - c},${y + h}` : "",
    `H${x + d}`,
    d ? `A${d},${d} 0 0 1 ${x},${y + h - d}` : "",
    `V${y + a}`,
    a ? `A${a},${a} 0 0 1 ${x + a},${y}` : "",
    "Z",
  ]
    .filter(Boolean)
    .join(" ");
}

/**
 * Si la ventana sale oscura: con el tema de winshotx, el que se eligió en los ajustes (que
 * `tema.ts` guarda en `winshotx.tema`), y en automático con el de Windows. Es lo mismo que
 * decide Rust para las capturas sueltas (`platform::tema`). No importa `tema.ts`, que al
 * cargarse pinta el tema en la página, y el editor va siempre en oscuro.
 */
export function ventanaOscura(): boolean {
  let tema: string | null = null;
  try {
    tema = localStorage.getItem("winshotx.tema");
  } catch {
    // Sin almacenamiento, el de Windows.
  }
  if (tema === "oscuro") return true;
  if (tema === "claro") return false;
  return typeof window.matchMedia === "function" ? window.matchMedia("(prefers-color-scheme: dark)").matches : true;
}

/** Lo que viaja a Rust para lo elegido, con el tema de ahora. */
export const carcasaParaPedir = (elegida: CarcasaElegida | null | undefined, oscura: boolean): Carcasa | null =>
  elegida === "ventana" ? { tipo: "ventana", oscura } : elegida === "portatil" ? { tipo: "portatil" } : null;
