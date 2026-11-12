/**
 * S11 (pistas): el sonido del PC y el micrófono por separado, en el editor.
 *
 * Una grabación nueva trae cada fuente en su WAV (`record::pistas` en Rust). El panel tiene un
 * mando para cada una (encendida y volumen), lo que se guarda las mezcla con esos volúmenes, y
 * la vista previa suena con las dos a la vez: el vídeo de vista previa, que las trae sumadas, se
 * calla, y cada pista va en su `<audio>` al compás de él.
 */
import type { VolumenDePista } from "./contrato";
import type { SessionInfo } from "./types";

/** Sin tocar nada, cada pista suena como se grabó. */
export const PISTA_DE_FABRICA: VolumenDePista = { encendida: true, volumen: 1 };

export function pistaDe(pista: VolumenDePista | null | undefined): VolumenDePista {
  return pista ?? PISTA_DE_FABRICA;
}

/** Si la grabación trae el PC y la voz por separado (con una sola de las dos, también). */
export function conPistas(session: Pick<SessionInfo, "sonidoPc" | "microfono"> | null | undefined): boolean {
  return !!(session?.sonidoPc || session?.microfono);
}

/** Lo que suena una pista en la vista previa, de 0 a 1: el navegador no sube de ahí, y por
 * encima del 100 % solo lo nota el archivo (igual que con el volumen de siempre). */
export function volumenEnVista(pista: VolumenDePista | null | undefined): number {
  const p = pistaDe(pista);
  if (!p.encendida || !Number.isFinite(p.volumen)) return 0;
  return Math.min(1, Math.max(0, p.volumen));
}

/** Lo que hace falta de un `<audio>` o un `<video>` para llevar una pista al compás. */
export interface Reproductor {
  currentTime: number;
  readonly paused: boolean;
  readonly seeking: boolean;
  readonly duration: number;
  volume: number;
  playbackRate: number;
  play(): Promise<void>;
  pause(): void;
}

export interface VideoDeLaVista {
  readonly currentTime: number;
  readonly paused: boolean;
  readonly seeking: boolean;
  readonly muted: boolean;
  readonly playbackRate: number;
}

/**
 * Lo lejos que puede ir una pista del vídeo, sonando, antes de recolocarla. Con la voz se nota
 * antes que con la cara (la cámara aguanta 0,15 s): una décima es lo que tarda en verse que los
 * labios no van con lo que se oye.
 */
export const DESVIO_MAXIMO_S = 0.1;
/** Y entre dos recolocaciones, al menos esto: arrancar una pista tarda unos milisegundos, y sin
 * respiro se podía quedar recolocándose sin parar, a trompicones. */
export const RESPIRO_MS = 750;

/** Cuándo se recolocó cada pista por última vez, para el respiro. */
export interface EstadoDePista {
  recolocadaMs: number;
}

/**
 * Lleva una pista al compás del vídeo de la vista previa. Se llama en cada pintada.
 *
 * Suena mientras el vídeo suena, al volumen de su mando, y va a su aire mientras no se aleje;
 * buscar un instante en cada pintada haría que se oyera a saltos. Lo acelerado va callado, como
 * en el archivo: es cuando la línea de tiempo cambia la velocidad del vídeo y lo calla.
 */
export function seguirAlVideo(
  pista: Reproductor,
  video: VideoDeLaVista | null,
  volumen: number,
  msParado: number,
  ahoraMs: number,
  estado: EstadoDePista,
): void {
  const meta = Math.max(0, (video ? video.currentTime * 1000 : msParado) / 1000);
  const dura = Number.isFinite(pista.duration) ? pista.duration : Infinity;
  if (pista.volume !== volumen) pista.volume = volumen;
  if (pista.playbackRate !== 1) pista.playbackRate = 1;
  const callada = !video || video.muted || video.playbackRate !== 1 || volumen <= 0;
  const suena = !!video && !video.paused && !video.seeking && !callada && meta < dura;
  if (!suena) {
    if (!pista.paused) pista.pause();
    // Parada se deja ya en su sitio: al darle al play arranca donde va el vídeo.
    if (!pista.seeking && meta < dura && Math.abs(pista.currentTime - meta) > 0.02) pista.currentTime = meta;
    return;
  }
  const lejos = Math.abs(pista.currentTime - meta) > DESVIO_MAXIMO_S;
  if (!pista.seeking && lejos && (pista.paused || ahoraMs - estado.recolocadaMs >= RESPIRO_MS)) {
    pista.currentTime = meta;
    estado.recolocadaMs = ahoraMs;
  }
  if (pista.paused) void pista.play().catch(() => undefined);
}
