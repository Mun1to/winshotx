/**
 * Lo que `capturas.mjs` midió al sacar los fotogramas de la app: dónde estaba el ratón en
 * cada uno del overlay, dónde quedó el recorte y cuánto mide cada ventana.
 */
import medidas from "../public/app/medidas.json";
import { staticFile } from "remotion";

export const MEDIDAS = medidas as unknown as {
  boton: { x: number; y: number; w: number; h: number };
  overlay: {
    fotogramas: number;
    fps: number;
    ratones: [number, number, number][];
    recorte: { x: number; y: number; w: number; h: number };
  };
  editor: { ancho: number; alto: number; escala: number };
  ajustes: { ancho: number; alto: number; escala: number };
};

export const fotoDelOverlay = (n: number) =>
  staticFile(`app/overlay/${String(Math.max(0, Math.min(MEDIDAS.overlay.fotogramas - 1, n))).padStart(4, "0")}.jpg`);

export const foto = (nombre: string) => staticFile(`app/${nombre}`);

/** Los momentos del recorrido del overlay, en fotogramas desde que aparece (los tramos de
    `capturas.mjs`: quieto 0,5 s, va al botón 1,4 s, se para 0,7 s, arrastra 1,3 s, suelta). */
export const OVERLAY = {
  quieto: 0,
  vaAlBoton: 15,
  mira: 57,
  arrastra: 78,
  suelta: 117,
  /** La barra de debajo del recorte ya está entera a la vista. */
  barra: 140,
  fin: 170,
} as const;
