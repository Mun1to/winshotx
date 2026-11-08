/**
 * Los fondos del marco del estudio, con los mismos colores que pinta Rust al exportar
 * (`src-tauri/src/encode/marco.rs`, `Fondo::color`): la vista previa los enseña en vivo y
 * tienen que ser los que salen en el archivo.
 *
 * Son unos pocos elegidos y no una rueda de color: con dieciséis millones a elegir, la
 * pregunta deja de ser «cuál de estos» y pasa a ser «cuál de todos», que es justo lo que
 * hace que alguien cierre el panel sin exportar nada.
 */
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

export function cssDelFondo(fondo: Background): string {
  return FONDOS.find((f) => f.id === fondo)?.css ?? FONDOS[0].css;
}
