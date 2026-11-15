/**
 * La identidad de winshotx, copiada de donde vive: los colores del `:root` de
 * `frontlaxweb/estilos.css` (el tema oscuro, que es el de la portada) y la letra de sus
 * titulares, la Segoe UI Variable de Windows. Nada se inventa: si cambia la web, se cambia aquí.
 */
export const COLOR = {
  fondo: "#0b0b0d",
  panel: "#141418",
  texto: "#e9e9ee",
  suave: "#abacb8",
  apagado: "#8e8e9a",
  /** El azul de la marca, el del logo. */
  azul: "#0a9bff",
  azulClaro: "#4fb8ff",
  /** La luz azul de detrás del titular de la portada. */
  halo: "rgba(10, 155, 255, 0.16)",
  /** La barra de «la otra herramienta» en las cifras de la web. */
  otra: "#5a5a66",
} as const;

export const LETRA = {
  titulo: "'Segoe UI Variable Display', 'Segoe UI', system-ui, sans-serif",
  texto: "'Segoe UI Variable Text', 'Segoe UI', system-ui, sans-serif",
  /** La de las notas a mano de la portada. */
  mano: "'Ink Free', 'Segoe Print', cursive",
} as const;

export const FPS = 30;
export const ANCHO = 1920;
export const ALTO = 1080;

/** Segundos a fotogramas. */
export const s = (segundos: number) => Math.round(segundos * FPS);
