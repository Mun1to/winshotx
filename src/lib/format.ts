/** Formateo compartido por el overlay y el editor. */

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

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/** "1 fotograma" y "8 fotogramas": el plural del castellano, para no cantar. */
export function plural(cantidad: number, singular: string, terminacion = "s"): string {
  return `${cantidad} ${singular}${cantidad === 1 ? "" : terminacion}`;
}
