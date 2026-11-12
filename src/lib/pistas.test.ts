/**
 * S11 (pistas): las cuentas de la vista previa con el PC y la voz por separado. Lo que de
 * verdad suena (el `<audio>` de Edge con un WAV de la sesión) se prueba en la app; aquí, que
 * cada pista va al compás del vídeo y a su volumen, con reproductores de mentira.
 */
import { describe, expect, it } from "vitest";
import {
  DESVIO_MAXIMO_S,
  RESPIRO_MS,
  conPistas,
  seguirAlVideo,
  volumenEnVista,
  type EstadoDePista,
  type Reproductor,
  type VideoDeLaVista,
} from "./pistas";

/** Un `<audio>` de mentira que apunta lo que se le pide. */
type DeMentira = Omit<Reproductor, "paused"> & { paused: boolean; plays: number; busquedas: number[] };

function pista(currentTime = 0): DeMentira {
  const p = {
    _t: currentTime,
    paused: true,
    seeking: false,
    duration: 30,
    volume: 1,
    playbackRate: 1,
    plays: 0,
    busquedas: [] as number[],
    get currentTime() {
      return p._t;
    },
    set currentTime(t: number) {
      p._t = t;
      p.busquedas.push(t);
    },
    play() {
      p.paused = false;
      p.plays++;
      return Promise.resolve();
    },
    pause() {
      p.paused = true;
    },
  };
  return p;
}

const sonando = (segundo: number, cambios: Partial<VideoDeLaVista> = {}): VideoDeLaVista => ({
  currentTime: segundo,
  paused: false,
  seeking: false,
  muted: false,
  playbackRate: 1,
  ...cambios,
});

const nuevo = (): EstadoDePista => ({ recolocadaMs: -Infinity });

describe("qué grabaciones traen las pistas", () => {
  it("con el PC, con la voz o con las dos; las de antes no", () => {
    expect(conPistas({ sonidoPc: "a.wav", microfono: null })).toBe(true);
    expect(conPistas({ sonidoPc: null, microfono: "b.wav" })).toBe(true);
    expect(conPistas({})).toBe(false);
    expect(conPistas(null)).toBe(false);
  });
});

describe("el volumen en la vista previa", () => {
  it("es el del mando hasta el 100 %, y apagada no suena", () => {
    expect(volumenEnVista(undefined)).toBe(1);
    expect(volumenEnVista({ encendida: true, volumen: 0.4 })).toBe(0.4);
    expect(volumenEnVista({ encendida: true, volumen: 1.8 })).toBe(1);
    expect(volumenEnVista({ encendida: false, volumen: 1 })).toBe(0);
  });
});

describe("una pista al compás del vídeo", () => {
  it("arranca donde va el vídeo, al volumen de su mando", () => {
    const p = pista(0);
    seguirAlVideo(p, sonando(4.2), 0.5, 0, 0, nuevo());
    expect(p.currentTime).toBeCloseTo(4.2, 6);
    expect(p.paused).toBe(false);
    expect(p.volume).toBe(0.5);
  });

  it("sonando y cerca del vídeo, va a su aire: buscar en cada pintada se oiría a saltos", () => {
    const p = pista(10);
    p.paused = false;
    seguirAlVideo(p, sonando(10 + DESVIO_MAXIMO_S * 0.8), 1, 0, 1_000, nuevo());
    expect(p.busquedas).toEqual([]);
  });

  it("si el vídeo salta (un corte), la pista salta con él", () => {
    const p = pista(2);
    p.paused = false;
    seguirAlVideo(p, sonando(7), 1, 0, 1_000, nuevo());
    expect(p.busquedas).toEqual([7]);
  });

  it("pero no se recoloca sin respiro, o se quedaría a trompicones", () => {
    const p = pista(2);
    p.paused = false;
    const estado = nuevo();
    seguirAlVideo(p, sonando(7), 1, 0, 1_000, estado);
    p.currentTime = 6.5;
    p.busquedas.length = 0;
    seguirAlVideo(p, sonando(7), 1, 0, 1_000 + RESPIRO_MS / 2, estado);
    expect(p.busquedas).toEqual([]);
    seguirAlVideo(p, sonando(7), 1, 0, 1_000 + RESPIRO_MS, estado);
    expect(p.busquedas).toEqual([7]);
  });

  it("con el vídeo parado se para, y se queda en su sitio para el próximo play", () => {
    const p = pista(3);
    p.paused = false;
    seguirAlVideo(p, sonando(5, { paused: true }), 1, 0, 0, nuevo());
    expect(p.paused).toBe(true);
    expect(p.currentTime).toBe(5);
  });

  it("en lo acelerado va callada, como en el archivo", () => {
    const p = pista(1);
    p.paused = false;
    seguirAlVideo(p, sonando(1, { playbackRate: 2, muted: true }), 1, 0, 0, nuevo());
    expect(p.paused).toBe(true);
    expect(p.playbackRate).toBe(1);
  });

  it("apagada no suena aunque el vídeo suene", () => {
    const p = pista(1);
    seguirAlVideo(p, sonando(1), 0, 0, 0, nuevo());
    expect(p.paused).toBe(true);
    expect(p.plays).toBe(0);
  });

  it("sin vídeo todavía, se queda parada en el instante que se enseña", () => {
    const p = pista(0);
    seguirAlVideo(p, null, 1, 2_500, 0, nuevo());
    expect(p.paused).toBe(true);
    expect(p.currentTime).toBe(2.5);
  });
});
