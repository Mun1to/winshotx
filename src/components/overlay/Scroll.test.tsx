/**
 * La captura con scroll, por el lado de la interfaz.
 *
 * Lo que se comprueba es lo que un fallo dejaría pasar en silencio: que el botón solo
 * sale en foto, que al soltar el recorte se le pide a Rust la zona exacta y NO una foto,
 * que la barra de acciones no sale encima, y que la barrita de la captura dice cómo va y
 * no tira nada con un solo clic.
 */
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BarraScroll, DESCARTAR_ARMADO_MS } from "./BarraScroll";
import { ModeBar } from "./ModeBar";
import { SelectionCanvas } from "./SelectionCanvas";
import { aplicarIdioma } from "../../lib/i18n";
import { EVENTO_SCROLL_PROGRESO, type ProgresoScroll } from "../../lib/scroll";
import type { CaptureMode, OverlayPayload, Settings } from "../../lib/types";
import { emite, falla, llamadas, responde } from "../../test/preparar";

const ANCHO = window.innerWidth;
const ALTO = window.innerHeight;

const PAYLOAD: OverlayPayload = {
  monitor: { id: 1, label: "PANTALLA 1", x: 0, y: 0, width: ANCHO, height: ALTO, scale: 1, isPrimary: true },
  generation: 1,
  windows: [],
  settings: {
    captureFlow: "toolbar",
    showMagnifier: false,
    fps: 30,
    recordAudio: false,
    recordMicrophone: false,
    highlightClicks: false,
    highlightKeys: false,
  } as unknown as Settings,
  intent: "capture",
  screenNumber: 1,
  screenCount: 1,
  lastRegion: null,
};

const comandos = () => llamadas.map((l) => l.comando);

beforeEach(() => {
  aplicarIdioma("es");
  responde("overlay_bootstrap", PAYLOAD);
  responde("freeze_png", new Uint8Array([137, 80, 78, 71]).buffer);
  vi.stubGlobal("createImageBitmap", vi.fn(async () => ({ width: ANCHO, height: ALTO })));
  HTMLCanvasElement.prototype.getContext = (() => ({
    drawImage: () => {},
    getImageData: () => ({ data: new Uint8ClampedArray([0, 0, 0, 255]) }),
  })) as unknown as HTMLCanvasElement["getContext"];
});

function barra(modo: CaptureMode, conScroll = false, onConScroll = vi.fn()) {
  render(
    <ModeBar
      value={modo}
      onChange={() => {}}
      pantallaEntera={false}
      onPantallaEntera={() => {}}
      conBarra
      onConBarra={() => {}}
      conScroll={conScroll}
      onConScroll={onConScroll}
      onAjustes={() => {}}
      onCancel={() => {}}
      dimmed={false}
    />,
  );
  return onConScroll;
}

describe("el botón de la barra de arriba", () => {
  it("sale en foto y lo enciende", () => {
    const encender = barra("still");
    fireEvent.click(screen.getByLabelText("Captura con scroll"));
    expect(encender).toHaveBeenCalledWith(true);
  });

  it("no sale grabando: un vídeo de una página que baja ya es grabar", () => {
    barra("video");
    expect(screen.queryByLabelText("Captura con scroll")).toBeNull();
  });

  it("encendido esconde «Elegir qué hacer», que con scroll no pinta nada", () => {
    barra("still", true);
    expect(screen.getByLabelText("Captura con scroll")).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByLabelText("Elegir qué hacer")).toBeNull();
  });
});

async function abrirOverlay() {
  const { container } = render(<SelectionCanvas monitorId={1} />);
  await screen.findByLabelText("Foto");
  return container.firstElementChild as HTMLElement;
}

function arrastrar(desde: Element, a: { x: number; y: number }, b: { x: number; y: number }) {
  fireEvent.pointerDown(desde, { clientX: a.x, clientY: a.y, buttons: 1 });
  fireEvent.pointerMove(desde, { clientX: b.x, clientY: b.y, buttons: 1 });
  fireEvent.pointerMove(window, { clientX: b.x, clientY: b.y, buttons: 1 });
  fireEvent.pointerUp(window, { clientX: b.x, clientY: b.y });
}

describe("en el overlay", () => {
  it("con la S y un recorte, pide a Rust que baje ESA zona y no hace ninguna foto", async () => {
    const lienzo = await abrirOverlay();
    fireEvent.keyDown(window, { key: "s" });
    expect(await screen.findByText("Recorta lo que se desplaza y winshotx lo bajará solo")).toBeInTheDocument();

    arrastrar(lienzo, { x: 120, y: 150 }, { x: 620, y: 550 });
    await waitFor(() => expect(comandos()).toContain("scroll_iniciar"));
    expect(llamadas.find((l) => l.comando === "scroll_iniciar")?.args).toEqual({
      region: { x: 120, y: 150, width: 500, height: 400 },
    });
    expect(comandos()).not.toContain("capture_still");
    // Ni la barra de acciones: lo cosido va al editor, no hay nada que elegir.
    expect(screen.queryByLabelText("Copiar")).toBeNull();
  });

  it("Ctrl+S sigue siendo guardar, no el scroll", async () => {
    await abrirOverlay();
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    expect(screen.queryByText("Recorta lo que se desplaza y winshotx lo bajará solo")).toBeNull();
  });

  it("si Rust no puede, lo dice en vez de quedarse callado", async () => {
    falla("scroll_iniciar", "Esa zona es muy pequeña para desplazarla: recorta algo más grande.");
    const lienzo = await abrirOverlay();
    fireEvent.click(screen.getByLabelText("Captura con scroll"));
    arrastrar(lienzo, { x: 100, y: 100 }, { x: 160, y: 140 });
    expect(
      await screen.findByText("Esa zona es muy pequeña para desplazarla: recorta algo más grande."),
    ).toBeInTheDocument();
  });

  it("sin encenderlo, soltar el recorte sigue sacando la barra de siempre", async () => {
    const lienzo = await abrirOverlay();
    arrastrar(lienzo, { x: 120, y: 150 }, { x: 620, y: 550 });
    expect(comandos()).not.toContain("scroll_iniciar");
  });
});

describe("la barrita mientras baja", () => {
  const progreso = (p: ProgresoScroll) =>
    act(async () => {
      emite(EVENTO_SCROLL_PROGRESO, p);
    });

  it("dice qué pasa y cuánto mide ya la imagen", async () => {
    render(<BarraScroll />);
    expect(screen.getByRole("status")).toHaveTextContent("Bajando sola");
    await progreso({ alto: 2340, estado: "tu" });
    expect(screen.getByRole("status")).toHaveTextContent("Desplázala tú");
    expect(screen.getByText(/2.?340 px/)).toBeInTheDocument();
    await progreso({ alto: 2340, estado: "perdida" });
    expect(screen.getByRole("status")).toHaveTextContent("Sube un poco, me he perdido");
  });

  it("Parar se pide una vez y deja la barra en Cosiendo", () => {
    render(<BarraScroll />);
    expect(screen.getByLabelText("Parar")).toHaveTextContent("Esc");
    fireEvent.click(screen.getByLabelText("Parar"));
    fireEvent.click(screen.getByLabelText("Parar"));
    expect(comandos().filter((c) => c === "scroll_terminar")).toHaveLength(1);
    expect(screen.getByRole("status")).toHaveTextContent("Cosiendo…");
    expect(screen.getByLabelText("Descartar")).toBeDisabled();
  });

  it("la X pide dos toques, y si nadie confirma se desarma", () => {
    vi.useFakeTimers();
    try {
      render(<BarraScroll />);
      fireEvent.click(screen.getByLabelText("Descartar"));
      expect(comandos()).not.toContain("scroll_descartar");
      act(() => {
        vi.advanceTimersByTime(DESCARTAR_ARMADO_MS + 10);
      });
      expect(screen.getByLabelText("Descartar")).toBeInTheDocument();
      fireEvent.click(screen.getByLabelText("Descartar"));
      fireEvent.click(screen.getByLabelText("¿Descartar?"));
      expect(comandos()).toContain("scroll_descartar");
    } finally {
      vi.useRealTimers();
    }
  });

  it("en inglés no queda nada en español", async () => {
    aplicarIdioma("en");
    render(<BarraScroll />);
    await progreso({ alto: 900, estado: "no-baja" });
    expect(screen.getByRole("status")).toHaveTextContent("It won't scroll by itself: scroll it yourself");
    expect(screen.getByLabelText("Stop")).toBeInTheDocument();
    expect(screen.getByLabelText("Discard")).toBeInTheDocument();
  });
});
