/**
 * El editor entero.
 *
 * Aquí se comprueban dos cosas que ninguna pieza suelta puede ver: que las capas de
 * dibujar caen EXACTAMENTE encima de la imagen, y que las teclas hacen lo que dicen.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EditorApp } from "./EditorApp";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas, responde } from "../../test/preparar";

const destruir = vi.fn();
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    destroy: destruir,
    setTitle: () => Promise.resolve(),
    onCloseRequested: () => Promise.resolve(() => {}),
  }),
}));

/** Una captura vertical, que es donde el desajuste de las capas se veía a simple vista. */
const VERTICAL = { x: 0, y: 0, width: 600, height: 1000 };

function preparar(region = VERTICAL) {
  responde("session_info", {
    id: "s1",
    region,
    fps: 30,
    frameCount: 3,
    durationMs: 100,
    hasAudio: false,
    format: "still",
    mp4Path: null,
  });
  responde("session_frames", [
    { index: 0, timestampMs: 0, durationMs: 33, thumbPath: "C:\\t\\0.png" },
    { index: 1, timestampMs: 33, durationMs: 33, thumbPath: "C:\\t\\1.png" },
    { index: 2, timestampMs: 66, durationMs: 33, thumbPath: "C:\\t\\2.png" },
  ]);
  responde("get_settings", { saveDirectory: "C:\\capturas", theme: "oscuro", language: "es" });
  responde("ffmpeg_available", false);
}

/** El editor carga por `invoke`, así que hay que esperar a que deje de decir «preparando». */
async function abrir(region = VERTICAL) {
  preparar(region);
  const vista = render(<EditorApp sessionId="s1" />);
  await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
  return vista;
}

beforeEach(() => {
  aplicarIdioma("es");
  destruir.mockClear();
});

describe("dónde caen las capas de dibujar", () => {
  it("la imagen y las capas viven en una caja con la proporción de la captura", async () => {
    // La vista previa se ajusta con `object-contain`: en un hueco de otra proporción deja
    // franjas. Con las capas estiradas al hueco entero, sus coordenadas de 0 a 1 contaban
    // esas franjas como parte de la captura y lo dibujado salía desplazado en el archivo.
    const { container } = await abrir();
    const caja = container.querySelector<HTMLElement>('[style*="aspect-ratio"]');
    expect(caja).not.toBeNull();
    expect(caja!.style.aspectRatio.replace(/\s/g, "")).toBe("600/1000");
    expect(caja!.querySelector("img")).not.toBeNull();
    expect(caja!.querySelectorAll("svg").length).toBeGreaterThanOrEqual(1);
  });

  it("y la proporción sale de la captura, no de un número fijo", async () => {
    const { container } = await abrir({ x: 0, y: 0, width: 1920, height: 1080 });
    const caja = container.querySelector<HTMLElement>('[style*="aspect-ratio"]');
    expect(caja!.style.aspectRatio.replace(/\s/g, "")).toBe("1920/1080");
  });
});

describe("recortar la imagen", () => {
  it("la tecla C enciende el marco de recorte", async () => {
    const { container } = await abrir();
    expect(container.querySelector('svg[role="figure"]')).toBeNull();
    fireEvent.keyDown(window, { key: "c" });
    expect(container.querySelector('svg[role="figure"]')).not.toBeNull();
  });

  it("y volver a pulsarla lo apaga", async () => {
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "c" });
    fireEvent.keyDown(window, { key: "c" });
    expect(container.querySelector('svg[role="figure"]')).toBeNull();
  });

  it("Ctrl+C no lo enciende, que esa es la de copiar", async () => {
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "c", ctrlKey: true });
    expect(container.querySelector('svg[role="figure"]')).toBeNull();
  });

  it("encender el recorte apaga la herramienta de dibujar", async () => {
    // Las dos se arrastran encima de la imagen: con las dos puestas, un arrastre haría
    // dos cosas a la vez.
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "2" });
    fireEvent.keyDown(window, { key: "c" });
    expect(container.querySelector('[aria-label^="Rectángulo"]')?.getAttribute("aria-pressed")).toBe(
      "false",
    );
  });
});

describe("Escape sale de lo que se esté haciendo antes de cerrar", () => {
  // Cerrar el editor TIRA los fotogramas del disco. Un Escape sin querer mientras se
  // coloca un marco no puede llevarse por delante la grabación entera.
  it("con el recorte puesto, solo lo apaga", async () => {
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "c" });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(container.querySelector('svg[role="figure"]')).toBeNull();
    expect(destruir).not.toHaveBeenCalled();
  });

  it("con una herramienta elegida, solo la suelta", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "1" });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(destruir).not.toHaveBeenCalled();
  });

  it("y sin nada puesto, cierra y tira la sesión", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(destruir).toHaveBeenCalled());
    expect(llamadas.some((l) => l.comando === "discard_session")).toBe(true);
  });
});
