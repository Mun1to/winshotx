/**
 * La barra que acompanna a la grabacion.
 *
 * Lo que se comprueba aqui es lo que un fallo dejaria pasar sin que nadie lo viera hasta
 * perder una grabacion: que descartar pide dos toques, que «Guardando…» no vuelve atras,
 * y que la barra dice que se esta grabando sin tener que preguntar nada.
 */
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CACHE_AVISO, DESCARTAR_ARMADO_MS, RecorderBar } from "./RecorderBar";
import { aplicarIdioma } from "../../lib/i18n";
import { EVENTS, type RecordingTick } from "../../lib/types";
import { emite, llamadas } from "../../test/preparar";

function tick(cambios: Partial<RecordingTick> = {}): RecordingTick {
  return {
    elapsedMs: 12_000,
    frames: 360,
    bytes: 20_000_000,
    paused: false,
    saving: false,
    format: "video",
    audio: false,
    microphone: false,
    ...cambios,
  };
}

const comandos = () => llamadas.map((l) => l.comando);

beforeEach(() => {
  aplicarIdioma("es");
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("lo que dice la barra", () => {
  it("ensenna el tiempo que manda Rust y el formato", async () => {
    render(<RecorderBar />);
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ elapsedMs: 83_000, format: "gif" }));
    });
    expect(screen.getByRole("timer")).toHaveTextContent("1:23");
    expect(screen.getByText("GIF")).toBeInTheDocument();
    expect(screen.queryByText("MP4")).toBeNull();
  });

  /**
   * La 0.5 (S37): el formato iba en una cajita a 10 px, «en pausa» y el tamaño a 11, y el
   * ámbar era un `amber-400` suelto. Ahora todo se lee a 12 px como poco, sin caja, y lo que
   * hay que mirar va en el color de aviso de toda la app.
   */
  it("lo que se lee va a 12 px como poco, el formato sin caja y el aviso en su color", async () => {
    render(<RecorderBar />);
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ bytes: CACHE_AVISO + 1, audio: true, microphone: true }));
    });
    const formato = screen.getByText("MP4");
    expect(formato.className).not.toMatch(/\bborder\b|\brounded/);
    const tamanno = screen.getByTitle(/La caché sin pérdida/);
    expect(tamanno.className).toMatch(/\btext-aviso\b/);
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ paused: true }));
    });
    expect(screen.getByText("en pausa").className).toMatch(/\btext-aviso\b/);
    const clases = [...document.querySelectorAll("[class]")].map((el) => el.getAttribute("class") ?? "");
    const tamannos = clases.flatMap((c) => [...c.matchAll(/text-\[(\d+(?:\.\d+)?)px\]/g)].map((m) => Number(m[1])));
    expect(Math.min(...tamannos)).toBeGreaterThanOrEqual(12);
    expect(clases.some((c) => /amber-/.test(c))).toBe(false);
  });

  it("solo ensenna el sonido que entra de verdad", async () => {
    render(<RecorderBar />);
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ audio: true, microphone: false }));
    });
    expect(screen.getByLabelText("Sonido del sistema grabándose")).toBeInTheDocument();
    expect(screen.queryByLabelText("Micrófono grabándose")).toBeNull();
  });

  it("el tamanno solo aparece cuando el cache ya pesa de verdad", async () => {
    render(<RecorderBar />);
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ bytes: 20_000_000 }));
    });
    expect(screen.queryByText(/MB/)).toBeNull();
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ bytes: CACHE_AVISO + 1 }));
    });
    expect(screen.getByText(/GB|MB/)).toBeInTheDocument();
  });

  it("en pausa lo dice, y el boton pasa a reanudar", async () => {
    render(<RecorderBar />);
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ paused: true }));
    });
    expect(screen.getByText("en pausa")).toBeInTheDocument();
    expect(screen.getByLabelText("Reanudar")).toBeInTheDocument();
  });

  it("en ingles no queda ni un texto en espannol", async () => {
    aplicarIdioma("en");
    render(<RecorderBar />);
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ audio: true, microphone: true, paused: false }));
    });
    expect(screen.getByLabelText("Pause")).toBeInTheDocument();
    expect(screen.getByLabelText("Stop")).toBeInTheDocument();
    expect(screen.getByLabelText("Discard")).toBeInTheDocument();
    expect(screen.getByLabelText("System sound being recorded")).toBeInTheDocument();
    expect(screen.getByLabelText("Microphone being recorded")).toBeInTheDocument();
    expect(screen.queryByLabelText("Pausar")).toBeNull();
    expect(screen.queryByLabelText("Descartar")).toBeNull();
  });
});

describe("descartar pide dos toques", () => {
  it("un clic solo arma el boton, y el segundo es el que tira la grabacion", () => {
    render(<RecorderBar />);
    fireEvent.click(screen.getByLabelText("Descartar"));
    expect(comandos()).not.toContain("cancel_recording");
    expect(screen.getByLabelText("¿Descartar?")).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("¿Descartar?"));
    expect(comandos()).toContain("cancel_recording");
  });

  it("si nadie confirma, el boton se desarma solo", () => {
    render(<RecorderBar />);
    fireEvent.click(screen.getByLabelText("Descartar"));
    act(() => {
      vi.advanceTimersByTime(DESCARTAR_ARMADO_MS + 10);
    });
    expect(screen.queryByLabelText("¿Descartar?")).toBeNull();
    expect(screen.getByLabelText("Descartar")).toBeInTheDocument();
    expect(comandos()).not.toContain("cancel_recording");
  });

  it("Escape hace lo mismo: uno arma, dos descartan", () => {
    render(<RecorderBar />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(comandos()).not.toContain("cancel_recording");
    fireEvent.keyDown(window, { key: "Escape" });
    expect(comandos()).toContain("cancel_recording");
  });
});

describe("parar", () => {
  it("pide parar una sola vez y se queda en Guardando con los botones apagados", () => {
    render(<RecorderBar />);
    fireEvent.click(screen.getByLabelText("Parar"));
    fireEvent.click(screen.getByLabelText("Parar"));
    expect(comandos().filter((c) => c === "stop_recording")).toHaveLength(1);
    expect(screen.getByText("Guardando…")).toBeInTheDocument();
    expect(screen.getByLabelText("Parar")).toBeDisabled();
    expect(screen.getByLabelText("Pausar")).toBeDisabled();
    expect(screen.getByLabelText("Descartar")).toBeDisabled();
  });

  it("Guardando no vuelve atras aunque llegue un tick viejo detras", async () => {
    render(<RecorderBar />);
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ saving: true }));
    });
    expect(screen.getByText("Guardando…")).toBeInTheDocument();
    await act(async () => {
      emite(EVENTS.recordingTick, tick({ saving: false }));
    });
    expect(screen.getByText("Guardando…")).toBeInTheDocument();
    expect(screen.getByLabelText("Parar")).toBeDisabled();
  });
});

describe("pausar", () => {
  it("pinta la pausa al pulsar, sin esperar al siguiente tick", () => {
    render(<RecorderBar />);
    fireEvent.click(screen.getByLabelText("Pausar"));
    expect(llamadas.find((l) => l.comando === "pause_recording")?.args).toEqual({ paused: true });
    expect(screen.getByLabelText("Reanudar")).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText("Reanudar"));
    expect(llamadas.filter((l) => l.comando === "pause_recording").at(-1)?.args).toEqual({
      paused: false,
    });
  });

  it("la barra espaciadora pausa y reanuda", () => {
    render(<RecorderBar />);
    fireEvent.keyDown(document.body, { key: " " });
    expect(llamadas.find((l) => l.comando === "pause_recording")?.args).toEqual({ paused: true });
  });
});
