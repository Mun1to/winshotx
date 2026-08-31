/**
 * El overlay de selección entero.
 *
 * Aquí se comprueba lo que ninguna pieza suelta puede ver, porque empieza en un
 * componente y acaba en otro. De momento, salir con Escape.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SelectionCanvas } from "./SelectionCanvas";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas, responde } from "../../test/preparar";
import type { OverlayPayload, Settings } from "../../lib/types";

const ANCHO = window.innerWidth;
const ALTO = window.innerHeight;

const AJUSTES = {
  captureFlow: "toolbar",
  showMagnifier: false,
  fps: 30,
  captureCursor: true,
  recordAudio: false,
  recordMicrophone: false,
  highlightClicks: false,
  highlightKeys: false,
} as unknown as Settings;

const PAYLOAD: OverlayPayload = {
  monitor: {
    id: 1,
    label: "PANTALLA 1",
    x: 0,
    y: 0,
    width: ANCHO,
    height: ALTO,
    scale: 1,
    isPrimary: true,
  },
  freezePath: "C:/temp/freeze-0.bmp",
  windows: [],
  settings: AJUSTES,
  intent: "capture",
  screenNumber: 1,
  screenCount: 1,
  lastRegion: null,
};

beforeEach(() => {
  aplicarIdioma("es");
  responde("overlay_bootstrap", PAYLOAD);
  // El congelado se pide por el protocolo asset y se pasa a bitmap: aquí no hay ni
  // servidor ni decodificador de imágenes, así que los dos se doblan. El bitmap mide lo
  // que la ventana para que la escala sea 1 y el tamaño que se lee sea el que se arrastra.
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({ ok: true, status: 200, blob: async () => new Blob(["freeze"]) })),
  );
  vi.stubGlobal("createImageBitmap", vi.fn(async () => ({ width: ANCHO, height: ALTO })));
  URL.createObjectURL = () => "blob:congelado";
  URL.revokeObjectURL = () => {};
  HTMLCanvasElement.prototype.getContext = (() => ({
    drawImage: () => {},
    getImageData: () => ({ data: new Uint8ClampedArray([0, 0, 0, 255]) }),
  })) as unknown as HTMLCanvasElement["getContext"];
});

/** Monta el overlay y espera a que el congelado esté puesto y la barra pintada. */
async function abrir() {
  const { container } = render(<SelectionCanvas monitorId={1} />);
  await screen.findByLabelText("Foto");
  return container.firstElementChild as HTMLElement;
}

/** Arrastra de un punto a otro empezando en `desde`, que puede ser cualquier elemento. */
function arrastrar(desde: Element, a: { x: number; y: number }, b: { x: number; y: number }) {
  fireEvent.pointerDown(desde, { clientX: a.x, clientY: a.y, buttons: 1 });
  fireEvent.pointerMove(desde, { clientX: b.x, clientY: b.y, buttons: 1 });
  fireEvent.pointerMove(window, { clientX: b.x, clientY: b.y, buttons: 1 });
  fireEvent.pointerUp(window, { clientX: b.x, clientY: b.y });
}

/** El tamaño que enseña la etiqueta del recorte, o null si no hay recorte. */
function recorte(): string | null {
  return screen.queryByText(/^\d+ × \d+$/)?.textContent ?? null;
}

describe("salir de la captura", () => {
  it("con UN solo Escape, aunque haya un recorte hecho", async () => {
    // Hasta el 31 de agosto de 2026 el primer Escape solo borraba el recorte y hacía
    // falta un segundo para salir. Quien pulsa Escape encima de una captura quiere irse.
    const lienzo = await abrir();
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    expect(recorte()).toBe("300 × 200");

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "cancel_capture")).toBe(true),
    );
  });

  it("y también sin nada seleccionado", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "cancel_capture")).toBe(true),
    );
  });
});
