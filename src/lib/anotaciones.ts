/**
 * Las seis marcas que se pueden dibujar sobre una captura antes de exportarla.
 *
 * Seis y ni una más: el editor de imagen no es el producto. Lo que hace falta para señalar
 * algo, para tapar un dato o para explicar un orden está aquí; lo demás es otro programa.
 */
export type Herramienta = "arrow" | "box" | "text" | "highlight" | "blur" | "step";

/**
 * Una marca, con sus dos esquinas **de 0 a 1** sobre el ancho y el alto de la imagen.
 *
 * En tanto por uno y no en píxeles porque quien dibuja lo hace sobre una vista previa que
 * casi nunca mide lo que va a medir el archivo. Así el mismo dibujo vale exportando al
 * 50 % o al doble, sin recalcular nada.
 */
export interface Anotacion {
  kind: Herramienta;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  /** En `#rrggbb`. El difuminado lo ignora. */
  color: string;
  /** Lo que dice, si es un texto. Y en un paso, su número. */
  text: string;
}

/** Los colores de las marcas. Cinco, y el primero es el de señalar. */
export const COLORES = ["#ef4444", "#0a9bff", "#22c55e", "#fbbf24", "#111827"];

/** El color con el que sale el resaltado, que no se elige: un marcador es amarillo. */
export const COLOR_RESALTADO = "#fbbf24";
