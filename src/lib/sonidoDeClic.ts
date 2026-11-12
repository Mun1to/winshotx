/**
 * El sonido de clic en la vista previa del editor (S10): cuáles suenan, cuándo, y el sonido.
 *
 * Suena lo mismo que en el MP4 y en el mismo sitio: las muestras las da Rust (`muestrasDeClic`,
 * las mismas que `clic::mezclar` pone en el archivo), a `sonidoClic · PICO` del máximo, y solo
 * los clics de los trozos que se quedan y van a su velocidad, la regla de `clic::instantes`.
 */
import type { ClicElegido } from "./contrato";
import { FRECUENCIA_DEL_CLIC, muestrasDeClic } from "./ipc";
import type { Pieza } from "./tramo";

/** Lo mismo que `clic::PICO` en Rust: el clic a tope llega al 60 % del máximo. */
export const PICO_DEL_CLIC = 0.6;

/** Los trozos que se quedan dentro de las asas, cada uno con su velocidad. Sin piezas, todo. */
function quedan(piezas: Pieza[], desdeMs: number, hastaMs: number): Pieza[] {
  const todas = piezas.length > 0 ? piezas : [{ desdeMs, hastaMs, velocidad: 1, cortada: false }];
  return todas
    .filter((p) => !p.cortada)
    .map((p) => ({ ...p, desdeMs: Math.max(p.desdeMs, desdeMs), hastaMs: Math.min(p.hastaMs, hastaMs) }))
    .filter((p) => p.hastaMs > p.desdeMs);
}

/** Los clics que suenan, en el reloj de la grabación y en orden. */
export function clicsQueSuenan(clics: number[], piezas: Pieza[], desdeMs: number, hastaMs: number): number[] {
  const normales = quedan(piezas, desdeMs, hastaMs).filter((p) => p.velocidad === 1);
  return clics.filter((ms) => normales.some((p) => ms >= p.desdeMs && ms < p.hastaMs)).sort((a, b) => a - b);
}

/**
 * Del reloj de la grabación al del vídeo que sale: lo que lleva sonando la vista previa en ese
 * instante, con lo cortado saltado y lo acelerado a su velocidad. Es el tiempo de verdad de la
 * reproducción, así que lo que falta para un clic es la resta de los dos.
 */
export function relojDeSalida(piezas: Pieza[], desdeMs: number, hastaMs: number): (ms: number) => number {
  const trozos = quedan(piezas, desdeMs, hastaMs);
  return (ms) => {
    let hecho = 0;
    for (const p of trozos) {
      if (ms < p.desdeMs) return hecho;
      if (ms < p.hastaMs) return hecho + (ms - p.desdeMs) / p.velocidad;
      hecho += (p.hastaMs - p.desdeMs) / p.velocidad;
    }
    return hecho;
  };
}

let contexto: AudioContext | null = null;

/** Un solo `AudioContext` para todo el editor: cada uno se queda con un hilo de sonido. */
export function contextoDeSonido(): AudioContext | null {
  if (!contexto) {
    try {
      contexto = new AudioContext();
    } catch {
      return null;
    }
  }
  if (contexto.state === "suspended") void contexto.resume().catch(() => undefined);
  return contexto;
}

const guardados = new Map<string, Promise<AudioBuffer>>();

/** El clic elegido como `AudioBuffer`, pedido a Rust una vez por sonido. */
export function bufferDelClic(ctx: AudioContext, clic: ClicElegido | null | undefined): Promise<AudioBuffer> {
  const clave = JSON.stringify(clic ?? null);
  let hecho = guardados.get(clave);
  if (!hecho) {
    hecho = muestrasDeClic(clic ?? null).then((muestras) => {
      const buffer = ctx.createBuffer(1, Math.max(1, muestras.length), FRECUENCIA_DEL_CLIC);
      buffer.getChannelData(0).set(muestras);
      return buffer;
    });
    // Si falla, la próxima vez se vuelve a pedir: puede que el archivo vuelva a estar.
    hecho.catch(() => guardados.delete(clave));
    guardados.set(clave, hecho);
  }
  return hecho;
}

/** Lo que pesa el clic a ese volumen del editor, de 0 a 1. */
export const gananciaDelClic = (volumen: number) => Math.min(1, Math.max(0, volumen)) * PICO_DEL_CLIC;

/** Suena el clic una vez, ya: al elegirlo, para oír cómo es. */
export async function oirClic(clic: ClicElegido | null | undefined, volumen: number): Promise<void> {
  const ctx = contextoDeSonido();
  if (!ctx || volumen <= 0) return;
  const buffer = await bufferDelClic(ctx, clic);
  const ganancia = ctx.createGain();
  ganancia.gain.value = gananciaDelClic(volumen);
  ganancia.connect(ctx.destination);
  const fuente = ctx.createBufferSource();
  fuente.buffer = buffer;
  fuente.connect(ganancia);
  fuente.start();
}

/** El nombre que se enseña de un sonido propio: el de su archivo, sin lo que le pone Rust. */
export function nombreDelClic(ruta: string): string {
  const base = ruta.split(/[\\/]/).pop() ?? ruta;
  return base.replace(/^clic-[0-9a-f]{16}-/, "").replace(/\.wav$/i, "");
}
