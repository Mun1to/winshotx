import type { Herramienta } from "../../lib/anotaciones";

/**
 * La tecla de cada herramienta de la barra de arriba, en un solo sitio: la usa el editor para
 * elegirla (`EditorApp.tsx`) y la barra para decirla al pasar el ratón (`BarraAnotar.tsx`).
 * Así lo que dice el botón y lo que hace la tecla no se pueden separar.
 *
 * Del 1 al 6 son las de siempre. Las marcas que entraron el 11 de noviembre siguen con el 7,
 * el 8, el 9 y el 0, y el foco va con la F (de «foco» y de «focus»).
 */
export const TECLA_DE: Partial<Record<Herramienta, string>> = {
  arrow: "1",
  box: "2",
  text: "3",
  highlight: "4",
  step: "5",
  blur: "6",
  curve: "7",
  line: "8",
  ellipse: "9",
  pen: "0",
  focus: "F",
};

/** Copiar el texto de la imagen: la T, la misma que en la captura. */
export const TECLA_COPIAR_TEXTO = "T";

/** Tapar los datos personales de una vez: la D, de «datos». */
export const TECLA_TAPAR_DATOS = "D";

/**
 * «Cortar aquí», donde está el cabezal: la B, la de cortar en casi todos los editores de vídeo
 * (y Ctrl+B también vale, que es la de CapCut).
 */
export const TECLA_CORTAR = "B";

/** La herramienta de una tecla («7», «f»...), o null si esa tecla no es de ninguna. */
export function herramientaDeTecla(tecla: string): Herramienta | null {
  const buscada = tecla.toUpperCase();
  for (const [herramienta, suya] of Object.entries(TECLA_DE)) {
    if (suya === buscada) return herramienta as Herramienta;
  }
  return null;
}

/** «Flecha (1)»: el nombre con su tecla, como lo enseña la barra al pasar el ratón. */
export function conTecla(nombre: string, tecla: string | undefined): string {
  return tecla ? `${nombre} (${tecla})` : nombre;
}
