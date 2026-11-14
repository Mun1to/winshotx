/**
 * Mover lo dibujado, con el editor entero (S23): las teclas de lo elegido (Supr, Escape,
 * Ctrl+Z), y que «Guardar» mande lo movido donde se ve.
 *
 * Escape importa más de lo que parece: sin nada puesto, cierra el editor y TIRA los
 * fotogramas del disco. Con algo elegido, solo lo suelta.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EditorApp } from "./EditorApp";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas, responde } from "../../test/preparar";
import type { Anotacion } from "../../lib/anotaciones";

const destruir = vi.fn();
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    destroy: destruir,
    setTitle: () => Promise.resolve(),
    onCloseRequested: () => Promise.resolve(() => {}),
  }),
}));

async function abrir() {
  responde("session_info", {
    id: "s1",
    region: { x: 0, y: 0, width: 800, height: 400 },
    fps: 30,
    frameCount: 1,
    durationMs: 0,
    hasAudio: false,
    format: "still",
    mp4Path: null,
  });
  responde("session_frames", [{ index: 0, timestampMs: 0, durationMs: 33, thumbPath: "C:\\t\\0.png" }]);
  responde("get_settings", { saveDirectory: "C:\\capturas", theme: "oscuro", language: "es" });
  responde("ffmpeg_available", false);
  const vista = render(<EditorApp sessionId="s1" />);
  await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
  return vista;
}

/** Todo mide 400 x 200 desde (100, 50): 40 píxeles son 0,1 de ancho y 20, 0,1 de alto. */
beforeEach(() => {
  aplicarIdioma("es");
  destruir.mockClear();
  llamadas.length = 0;
  Element.prototype.getBoundingClientRect = vi.fn(() => ({
    x: 100,
    y: 50,
    left: 100,
    top: 50,
    right: 500,
    bottom: 250,
    width: 400,
    height: 200,
    toJSON: () => ({}),
  })) as unknown as typeof Element.prototype.getBoundingClientRect;
});

/** Un rectángulo de (0,1, 0,1) a (0,4, 0,4), y la herramienta suelta. */
function dibujarCaja(container: HTMLElement) {
  fireEvent.keyDown(window, { key: "2" });
  const capa = container.querySelector("svg.cursor-crosshair")!;
  fireEvent.pointerDown(capa, { clientX: 140, clientY: 70 });
  fireEvent.pointerMove(capa, { clientX: 260, clientY: 130 });
  fireEvent.pointerUp(capa);
  fireEvent.keyDown(window, { key: "Escape" });
}

const silueta = (container: HTMLElement) => container.querySelector('[data-agarre="0"] [data-silueta]')!.firstElementChild!;
const xDeLaCaja = (container: HTMLElement) => container.querySelector('[data-agarre="0"] > rect')?.getAttribute("x");

/** Coge la caja por su borde y la lleva 0,1 a la derecha y 0,1 abajo. */
function moverCaja(container: HTMLElement) {
  fireEvent.pointerDown(silueta(container), { clientX: 140, clientY: 70 });
  fireEvent.pointerMove(window, { clientX: 160, clientY: 80 });
  fireEvent.pointerMove(window, { clientX: 180, clientY: 90 });
  fireEvent.pointerUp(window, { clientX: 180, clientY: 90 });
}

async function loQueSeGuarda(): Promise<Anotacion[]> {
  const antes = llamadas.filter((l) => l.comando === "export_media").length;
  fireEvent.click(screen.getByRole("button", { name: "Guardar PNG" }));
  await waitFor(() => expect(llamadas.filter((l) => l.comando === "export_media").length).toBe(antes + 1));
  const ultima = llamadas.filter((l) => l.comando === "export_media").at(-1)!;
  return (ultima.args as { request: { annotations: Anotacion[] } }).request.annotations;
}

describe("lo dibujado, con el editor entero", () => {
  it("se mueve sin herramienta, y «Guardar» lo manda donde se ve", async () => {
    const { container } = await abrir();
    dibujarCaja(container);
    expect(xDeLaCaja(container)).toBe("100");
    moverCaja(container);
    expect(xDeLaCaja(container)).toBe("200");
    const [caja] = await loQueSeGuarda();
    expect(caja.kind).toBe("box");
    expect(caja.x1).toBeCloseTo(0.2, 5);
    expect(caja.y1).toBeCloseTo(0.2, 5);
    expect(caja.x2).toBeCloseTo(0.5, 5);
    expect(caja.y2).toBeCloseTo(0.5, 5);
  });

  it("Ctrl+Z la devuelve a donde estaba, de un golpe", async () => {
    const { container } = await abrir();
    dibujarCaja(container);
    moverCaja(container);
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(xDeLaCaja(container)).toBe("100");
    // Y el siguiente Ctrl+Z quita la caja, como antes.
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(container.querySelector('[data-agarre="0"]')).toBeNull();
  });

  it("Supr borra la elegida, y Ctrl+Z la trae de vuelta", async () => {
    const { container } = await abrir();
    dibujarCaja(container);
    fireEvent.pointerDown(silueta(container), { clientX: 140, clientY: 70 });
    fireEvent.pointerUp(window, { clientX: 140, clientY: 70 });
    expect(container.querySelector("[data-halo]")).not.toBeNull();
    fireEvent.keyDown(window, { key: "Delete" });
    expect(container.querySelector('[data-agarre="0"]')).toBeNull();
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(xDeLaCaja(container)).toBe("100");
  });

  it("Supr sin nada elegido no borra nada", async () => {
    const { container } = await abrir();
    dibujarCaja(container);
    fireEvent.keyDown(window, { key: "Delete" });
    expect(container.querySelector('[data-agarre="0"]')).not.toBeNull();
  });

  it("con algo elegido, Escape lo suelta y no cierra el editor", async () => {
    const { container } = await abrir();
    dibujarCaja(container);
    fireEvent.pointerDown(silueta(container), { clientX: 140, clientY: 70 });
    fireEvent.pointerUp(window, { clientX: 140, clientY: 70 });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(container.querySelector("[data-halo]")).toBeNull();
    expect(destruir).not.toHaveBeenCalled();
  });

  it("«Quitar todas» también se deshace", async () => {
    const { container } = await abrir();
    dibujarCaja(container);
    fireEvent.click(screen.getByRole("button", { name: "Quitar todas" }));
    expect(container.querySelector('[data-agarre="0"]')).toBeNull();
    const deshacer = screen.getByRole("button", { name: "Deshacer (Ctrl+Z)" });
    expect(deshacer).not.toBeDisabled();
    fireEvent.click(deshacer);
    expect(container.querySelector('[data-agarre="0"]')).not.toBeNull();
  });

  it("un texto se reescribe con doble clic y «Guardar» manda lo nuevo en su sitio", async () => {
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "3" });
    fireEvent.pointerDown(container.querySelector("svg.cursor-crosshair")!, { clientX: 300, clientY: 150 });
    const campo = screen.getByRole("textbox", { name: "Texto" });
    fireEvent.change(campo, { target: { value: "hola" } });
    fireEvent.keyDown(campo, { key: "Enter" });
    fireEvent.keyDown(window, { key: "Escape" });
    const span = screen.getByText("hola");
    for (let i = 0; i < 2; i++) {
      fireEvent.pointerDown(span, { clientX: 305, clientY: 155 });
      fireEvent.pointerUp(window, { clientX: 305, clientY: 155 });
    }
    const otraVez = screen.getByRole("textbox", { name: "Texto" });
    fireEvent.change(otraVez, { target: { value: "hola, Munito" } });
    fireEvent.keyDown(otraVez, { key: "Enter" });
    const marcas = await loQueSeGuarda();
    expect(marcas).toHaveLength(1);
    expect(marcas[0]).toMatchObject({ kind: "text", text: "hola, Munito", x1: 0.5, y1: 0.5 });
  });

  // S24: con algo elegido, pinchar un color de la barra se lo cambia; sin nada, es para lo próximo.
  const colorDeLaCaja = (container: HTMLElement) => container.querySelector('[data-agarre="0"] > rect')?.getAttribute("stroke");
  const elegirCaja = (container: HTMLElement) => {
    fireEvent.pointerDown(silueta(container), { clientX: 140, clientY: 70 });
    fireEvent.pointerUp(window, { clientX: 140, clientY: 70 });
  };

  it("con la caja elegida, pinchar un color se lo cambia, «Guardar» lo manda y Ctrl+Z se lo devuelve", async () => {
    const { container } = await abrir();
    dibujarCaja(container);
    expect(colorDeLaCaja(container)).toBe("#ef4444");
    elegirCaja(container);
    fireEvent.click(screen.getByRole("button", { name: "#0a9bff" }));
    expect(colorDeLaCaja(container)).toBe("#0a9bff");
    // La paleta enseña el color de lo elegido.
    expect(screen.getByRole("button", { name: "#0a9bff" })).toHaveAttribute("aria-pressed", "true");
    const [caja] = await loQueSeGuarda();
    expect(caja.color).toBe("#0a9bff");
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(colorDeLaCaja(container)).toBe("#ef4444");
    // Y deshacer solo le quita el color: la caja sigue ahí.
    expect(container.querySelector('[data-agarre="0"]')).not.toBeNull();
  });

  it("sin nada elegido, el color es solo para lo próximo que se dibuje", async () => {
    const { container } = await abrir();
    dibujarCaja(container);
    fireEvent.click(screen.getByRole("button", { name: "#22c55e" }));
    expect(colorDeLaCaja(container)).toBe("#ef4444");
    dibujarCaja(container);
    expect(container.querySelector('[data-agarre="1"] > rect')?.getAttribute("stroke")).toBe("#22c55e");
  });
});
