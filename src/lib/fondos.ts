/**
 * Los fondos del marco del estudio, con los mismos colores que pinta Rust al exportar
 * (`src-tauri/src/encode/marco.rs`, `Fondo::color`): la vista previa los enseña en vivo y
 * tienen que ser los que salen en el archivo.
 *
 * Los de nombre son unos pocos elegidos y no una rueda de color: con dieciséis millones a
 * elegir, la pregunta deja de ser «cuál de estos» y pasa a ser «cuál de todos», que es justo
 * lo que hace que alguien cierre el panel sin exportar nada. Por eso los propios (una imagen,
 * el escritorio, un vídeo, cualquier color) van en `FondoPicker`, y la paleta detrás de
 * «Más colores».
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import type { FondoElegido } from "./contrato";
import type { Background } from "./types";

export const FONDOS: { id: Background; nombre: string; css: string }[] = [
  { id: "blanco", nombre: "Blanco", css: "rgb(255 255 255)" },
  { id: "gris", nombre: "Gris", css: "rgb(232 232 234)" },
  { id: "negro", nombre: "Negro", css: "rgb(18 18 20)" },
  { id: "noche", nombre: "Noche", css: "linear-gradient(135deg, rgb(30 41 59), rgb(15 23 42))" },
  { id: "atardecer", nombre: "Atardecer", css: "linear-gradient(135deg, rgb(88 116 245), rgb(168 85 217))" },
  { id: "menta", nombre: "Menta", css: "linear-gradient(135deg, rgb(34 197 152), rgb(56 132 232))" },
  { id: "oceano", nombre: "Océano", css: "linear-gradient(135deg, rgb(14 165 233), rgb(30 64 175))" },
  { id: "coral", nombre: "Coral", css: "linear-gradient(135deg, rgb(251 113 133), rgb(245 158 11))" },
];

export type Rgb = [number, number, number];

export const rgbCss = ([r, g, b]: Rgb) => `rgb(${r} ${g} ${b})`;

/**
 * El otro extremo del degradado de un color: el tono girado 40 grados y la luz al 82 %.
 *
 * Son las mismas cuentas, en el mismo orden, que `segundo_color` de `marco.rs`, y las
 * pruebas de los dos lados comprueban los mismos números: si no, la vista previa enseñaría
 * un degradado y el archivo saldría con otro.
 */
export function segundoColor(rgb: Rgb): Rgb {
  const [r, g, b] = rgb.map((c) => c / 255);
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  let l = (max + min) / 2;
  const d = max - min;
  let h = 0;
  let s = 0;
  if (d !== 0) {
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h *= 60;
  }
  h = (h + 40) % 360;
  l *= 0.82;
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = l - c / 2;
  const tramos: Rgb[] = [
    [c, x, 0],
    [x, c, 0],
    [0, c, x],
    [0, x, c],
    [x, 0, c],
    [c, 0, x],
  ];
  const [rr, gg, bb] = tramos[Math.min(5, Math.trunc(h / 60))];
  return [rr, gg, bb].map((v) => Math.min(255, Math.max(0, Math.round((v + m) * 255)))) as Rgb;
}

/** El color en «#rrggbb», como se escribe en cualquier programa de diseño. */
export const hexDe = (rgb: Rgb) => `#${rgb.map((c) => c.toString(16).padStart(2, "0")).join("")}`;

/** Lee «#3e5ba9», «3E5BA9» o «#35a». `null` si no es un color. */
export function rgbDeHex(texto: string): Rgb | null {
  let hex = texto.trim().replace(/^#/, "");
  if (/^[0-9a-f]{3}$/i.test(hex)) hex = [...hex].map((c) => c + c).join("");
  if (!/^[0-9a-f]{6}$/i.test(hex)) return null;
  return [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16)) as Rgb;
}

/** De RGB a tono (0 a 360), saturación y brillo (0 a 1): los ejes de la paleta. */
export function hsvDe([r, g, b]: Rgb): [number, number, number] {
  const [rr, gg, bb] = [r / 255, g / 255, b / 255];
  const max = Math.max(rr, gg, bb);
  const d = max - Math.min(rr, gg, bb);
  let h = 0;
  if (d !== 0) {
    if (max === rr) h = ((gg - bb) / d) % 6;
    else if (max === gg) h = (bb - rr) / d + 2;
    else h = (rr - gg) / d + 4;
    h = (h * 60 + 360) % 360;
  }
  return [h, max === 0 ? 0 : d / max, max];
}

export function rgbDeHsv(h: number, s: number, v: number): Rgb {
  const c = v * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - c;
  const tramos: Rgb[] = [
    [c, x, 0],
    [x, c, 0],
    [0, c, x],
    [0, x, c],
    [x, 0, c],
    [c, 0, x],
  ];
  const [r, g, b] = tramos[Math.min(5, Math.max(0, Math.trunc((h % 360) / 60)))];
  return [r, g, b].map((n) => Math.round((n + m) * 255)) as Rgb;
}

/** La vista de un fondo animado importado: su primer fotograma, al lado, en JPEG. */
export const vistaDelVideo = (ruta: string) => ruta.replace(/\.mp4$/i, ".jpg");

/**
 * Lo que tiene puesto el escritorio, tal cual lo trajo `FondoPicker` al elegirlo. Al
 * exportar Rust vuelve a preguntar a Windows, así que esto solo vale para enseñarlo.
 */
let escritorio: string | null = null;

export function recordarEscritorio(vista: { imagen: string } | { rgb: Rgb } | null) {
  if (vista === null) escritorio = null;
  else if ("imagen" in vista) escritorio = conImagen(vista.imagen);
  else escritorio = rgbCss(vista.rgb);
}

/** Una imagen del disco de fondo, recortada para llenar: el «cubrir» que hace Rust. */
const conImagen = (ruta: string) => `center / cover no-repeat url("${convertFileSrc(ruta)}"), rgb(18 18 20)`;

/** Cómo se pinta el fondo de un vistazo: el propio si lo hay, y si no el de nombre. */
export function cssDelFondo(fondo: Background, elegido?: FondoElegido | null): string {
  if (elegido) {
    switch (elegido.tipo) {
      case "color":
        return rgbCss(elegido.rgb);
      case "degradado":
        return `linear-gradient(135deg, ${rgbCss(elegido.rgb)}, ${rgbCss(segundoColor(elegido.rgb))})`;
      case "imagen":
        return conImagen(elegido.ruta);
      case "video":
        return conImagen(vistaDelVideo(elegido.ruta));
      case "escritorio":
        return escritorio ?? "rgb(18 18 20)";
    }
  }
  return FONDOS.find((f) => f.id === fondo)?.css ?? FONDOS[0].css;
}
