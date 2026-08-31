/**
 * El overlay de selección entero.
 *
 * Aquí se comprueban los dos gestos que ninguna pieza suelta puede ver, porque empiezan
 * en un componente y acaban en otro: salir con Escape, y empezar a recortar justo donde
 * está la barra de arriba.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SelectionCanvas } from "./SelectionCanvas";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas, responde } from "../../test/preparar";
import type { OverlayPayload, Settings } from "../../lib/types";

const ANCHO = window.innerWidth;
const ALTO = window.innerHeight;

/** La barra vive arriba y centrada: este punto cae dentro de ella. */
const EN_LA_BARRA = { x: Math.round(ANCHO / 2), y: 30 };

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

describe("la barra de arriba no se queda el sitio", () => {
  it("se puede empezar a recortar encima de ella", async () => {
    // La barra tapa una franja del centro de arriba, y ahí no había forma de arrastrar:
    // el botón se quedaba el gesto entero.
    const lienzo = await abrir();
    const boton = screen.getByLabelText("Foto");
    fireEvent.pointerDown(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y, buttons: 1 });
    fireEvent.pointerMove(lienzo, { clientX: EN_LA_BARRA.x + 260, clientY: 190, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: EN_LA_BARRA.x + 260, clientY: 190, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: EN_LA_BARRA.x + 260, clientY: 190 });

    expect(recorte()).toBe("260 × 160");
  });

  it("pero un clic seco sigue siendo del botón, y no recorta nada", async () => {
    await abrir();
    const boton = screen.getByLabelText("GIF");
    fireEvent.pointerDown(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y, buttons: 1 });
    fireEvent.pointerUp(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y });
    fireEvent.click(boton);

    expect(recorte()).toBeNull();
    expect(llamadas.some((l) => l.comando === "capture_still")).toBe(false);
  });

  it("y soltar el botón deja el ratón libre: moverlo después no dibuja solo", async () => {
    // El gesto queda apuntado hasta que se sepa qué es. Si no se borra al soltar, el
    // siguiente movimiento del ratón, ya sin botón, se ponía a recortar por su cuenta.
    const lienzo = await abrir();
    const boton = screen.getByLabelText("Foto");
    fireEvent.pointerDown(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y, buttons: 1 });
    fireEvent.pointerUp(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y });

    fireEvent.pointerMove(lienzo, { clientX: 700, clientY: 600, buttons: 0 });
    expect(recorte()).toBeNull();
  });
});
