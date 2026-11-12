/**
 * El sonido de clic al compás de la vista previa (S10), con un WebAudio de mentira: lo que se
 * prueba es CUÁNDO se programa cada clic y con qué volumen, no cómo suena.
 */
import { act, render } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SonidoDeClics } from "./SonidoDeClics";
import type { Pieza } from "../../lib/tramo";

vi.mock("../../lib/ipc", () => ({
  FRECUENCIA_DEL_CLIC: 48_000,
  muestrasDeClic: vi.fn(() => Promise.resolve(new Float32Array(5_760))),
}));

interface Fuente {
  start: ReturnType<typeof vi.fn>;
  stop: ReturnType<typeof vi.fn>;
}

const fuentes: Fuente[] = [];
const ganancias: { gain: { value: number } }[] = [];
let pintadas: FrameRequestCallback[] = [];

class ContextoDeMentira {
  currentTime = 10;
  state = "running";
  destination = {};
  createGain() {
    const g = { gain: { value: 1 }, connect: vi.fn(), disconnect: vi.fn() };
    ganancias.push(g);
    return g;
  }
  createBuffer(_canales: number, largo: number) {
    return { getChannelData: () => new Float32Array(largo) };
  }
  createBufferSource() {
    const f = { buffer: null, connect: vi.fn(), start: vi.fn(), stop: vi.fn() };
    fuentes.push(f);
    return f;
  }
  resume() {
    return Promise.resolve();
  }
}

beforeEach(() => {
  fuentes.length = 0;
  ganancias.length = 0;
  pintadas = [];
  vi.stubGlobal("AudioContext", ContextoDeMentira);
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => pintadas.push(cb));
  vi.stubGlobal("cancelAnimationFrame", () => undefined);
});

afterEach(() => vi.unstubAllGlobals());

/** Una pintada de la pantalla: corre lo que estaba esperando a la siguiente. */
function pintar() {
  const ahora = pintadas;
  pintadas = [];
  for (const cb of ahora) cb(0);
}

const PIEZAS: Pieza[] = [
  { desdeMs: 0, hastaMs: 2_000, velocidad: 1, cortada: false },
  { desdeMs: 2_000, hastaMs: 3_000, velocidad: 1, cortada: true },
  { desdeMs: 3_000, hastaMs: 5_000, velocidad: 2, cortada: false },
  { desdeMs: 5_000, hastaMs: 6_000, velocidad: 1, cortada: false },
];

describe("el sonido de clic en la vista previa", () => {
  it("programa cada clic que suena en su instante, a su volumen, y los cortados o acelerados no", async () => {
    const video = { currentTime: 0.9, paused: false, seeking: false } as HTMLVideoElement;
    const videoRef = createRef<HTMLVideoElement>() as { current: HTMLVideoElement | null };
    videoRef.current = video;
    render(
      <SonidoDeClics
        videoRef={videoRef}
        reproduciendo
        clics={[1_000, 2_500, 4_000, 5_200]}
        piezas={PIEZAS}
        desdeMs={0}
        hastaMs={6_000}
        volumen={0.5}
        clic={{ tipo: "grave" }}
      />,
    );
    await act(async () => {});
    pintar();
    expect(fuentes).toHaveLength(1);
    // A los 0,9 s, el de 1 s suena dentro de 100 ms exactos, no en la pintada que lo pase.
    expect(fuentes[0].start.mock.calls[0][0]).toBeCloseTo(10.1, 5);
    expect(ganancias.at(-1)?.gain.value).toBeCloseTo(0.3, 5);
    pintar();
    expect(fuentes).toHaveLength(1);

    // Ya en el trozo de después del acelerado: el de 4 s (al doble) no ha sonado, y el de 5,2 s
    // se programa cuando le toca.
    video.currentTime = 5.1;
    pintar();
    expect(fuentes).toHaveLength(2);
    expect(fuentes[1].start.mock.calls[0][0]).toBeCloseTo(10.1, 5);

    // Al parar, lo programado se quita.
    (video as { paused: boolean }).paused = true;
    pintar();
    expect(fuentes[1].stop).toHaveBeenCalled();
  });

  it("a volumen cero o sin reproducir no hace nada", async () => {
    const videoRef = { current: { currentTime: 0.95, paused: false, seeking: false } as HTMLVideoElement };
    const { rerender } = render(
      <SonidoDeClics videoRef={videoRef} reproduciendo clics={[1_000]} piezas={[]} desdeMs={0} hastaMs={2_000} volumen={0} clic={null} />,
    );
    await act(async () => {});
    pintar();
    rerender(
      <SonidoDeClics videoRef={videoRef} reproduciendo={false} clics={[1_000]} piezas={[]} desdeMs={0} hastaMs={2_000} volumen={0.3} clic={null} />,
    );
    await act(async () => {});
    pintar();
    expect(fuentes).toHaveLength(0);
  });
});
