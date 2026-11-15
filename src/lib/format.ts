/** Formateo compartido por el overlay y el editor. */
import { idiomaActivo } from "./i18n";

export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

export function formatTimecode(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const centis = Math.floor((ms % 1000) / 10);
  return `${formatDuration(seconds * 1000)}.${String(centis).padStart(2, "0")}`;
}

/**
 * Lo que ocupa un archivo, como lo escribe una persona en su idioma: «3,9 GB» en español y
 * «3.9 GB» en inglés y en chino.
 *
 * Entre la cifra y la unidad va un espacio que no se parte (`\u00a0`): con uno normal, en
 * una frase larga la cifra se quedaba al final de un renglón y «MB» empezaba el siguiente.
 * El idioma es el de la ventana; se puede pasar otro para las pruebas.
 */
export function formatBytes(bytes: number, idioma: "es" | "en" | "zh" = idiomaActivo()): string {
  const decimal = (n: number) => {
    const texto = n.toFixed(1);
    return idioma === "es" ? texto.replace(".", ",") : texto;
  };
  if (bytes < 1024) return `${bytes}\u00a0B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}\u00a0KB`;
  // Los gigas hacen falta desde que el anillo de los últimos segundos dice lo que puede
  // llegar a ocupar: «4096,0 MB» es un número que nadie lee como cuatro gigas.
  if (bytes < 1024 * 1024 * 1024) return `${decimal(bytes / (1024 * 1024))}\u00a0MB`;
  return `${decimal(bytes / (1024 * 1024 * 1024))}\u00a0GB`;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
