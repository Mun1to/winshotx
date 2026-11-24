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

// S39, segunda vuelta: cada botón de la barra de arriba dice su nombre y su tecla, y la tecla
// hace lo que dice el botón.
describe("la barra de arriba: cada botón con su tecla", () => {
  const CON_TECLA: [string, string][] = [
    ["Flecha", "Flecha (1)"],
    ["Flecha curva", "Flecha curva (7)"],
    ["Línea", "Línea (8)"],
    ["Rectángulo", "Rectángulo (2)"],
    ["Elipse", "Elipse (9)"],
    ["Lápiz", "Lápiz (0)"],
    ["Texto", "Texto (3)"],
    ["Resaltar", "Resaltar (4)"],
    ["Paso numerado", "Paso numerado (5)"],
    ["Foco", "Foco (F)"],
    ["Tapar con mosaico", "Tapar con mosaico (6)"],
    ["Encuadrar", "Quedarse con un trozo de la imagen (C)"],
    ["Tapar datos", "Tapar datos personales (D)"],
    ["Copiar texto", "Copiar el texto de la imagen (T)"],
    ["Deshacer (Ctrl+Z)", "Deshacer (Ctrl+Z)"],
    ["Quitar todas", "Quitar todas (Mayús+Supr)"],
  ];

  it("al pasar el ratón, cada botón dice su nombre y su tecla", async () => {
    await abrir();
    for (const [nombre, titulo] of CON_TECLA) {
      expect(screen.getByRole("button", { name: nombre }), nombre).toHaveAttribute("title", titulo);
    }
  });

  it("la tecla de cada herramienta la elige, y otra vez la suelta", async () => {
    await abrir();
    for (const [nombre, titulo] of CON_TECLA.slice(0, 11)) {
      const tecla = titulo.match(/\((.)\)$/)![1];
      fireEvent.keyDown(window, { key: tecla.toLowerCase() });
      expect(screen.getByRole("button", { name: nombre }), tecla).toHaveAttribute("aria-pressed", "true");
      fireEvent.keyDown(window, { key: tecla.toLowerCase() });
      expect(screen.getByRole("button", { name: nombre }), tecla).toHaveAttribute("aria-pressed", "false");
    }
  });

  it("el mosaico va pegado a «Tapar datos», que es lo mismo hecho solo", async () => {
    await abrir();
    const mosaico = screen.getByRole("button", { name: "Tapar con mosaico" });
    expect(mosaico.nextElementSibling).toBe(screen.getByRole("button", { name: "Tapar datos" }));
  });

  it("T copia el texto de la imagen y D tapa los datos, como sus botones", async () => {
    responde("copiar_texto", "hola que tal");
    responde("tapar_datos", { marcas: [], resumen: [] });
    await abrir();
    fireEvent.keyDown(window, { key: "t" });
    await waitFor(() => expect(llamadas.some((l) => l.comando === "copiar_texto")).toBe(true));
    await screen.findByText(/Texto copiado/);
    fireEvent.keyDown(window, { key: "d" });
    await waitFor(() => expect(llamadas.some((l) => l.comando === "tapar_datos")).toBe(true));
  });

  it("los colores plegados en uno se abren, eligen y se cierran; Escape los cierra sin cerrar el editor", async () => {
    await abrir();
    const plegados = screen.getByRole("button", { name: "Color" });
    fireEvent.click(plegados);
    expect(plegados).toHaveAttribute("aria-expanded", "true");
    const cinco = screen.getByRole("group", { name: "Color" });
    fireEvent.click(cinco.querySelector('[aria-label="#22c55e"]')!);
    expect(screen.queryByRole("group", { name: "Color" })).toBeNull();
    // El verde, puesto también en la fila de los cinco (la de la barra ancha).
    expect(screen.getAllByRole("button", { name: "#22c55e" })[0]).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(plegados);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("group", { name: "Color" })).toBeNull();
    expect(destruir).not.toHaveBeenCalled();
  });

  it("Mayús+Supr quita todas, y Ctrl+Z las devuelve", async () => {
    const { container } = await abrir();
    dibujarCaja(container);
    fireEvent.keyDown(window, { key: "Delete", shiftKey: true });
    expect(container.querySelector('[data-agarre="0"]')).toBeNull();
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(container.querySelector('[data-agarre="0"]')).not.toBeNull();
  });
});

// Las issues 18 y 19: el texto y los pasos llevan su tamaño, y la barra lo cambia como el color.
describe("el tamaño del texto y de los pasos, con el editor entero", () => {
  const pasoEn = (container: HTMLElement, i = 0) =>
    container.querySelector<HTMLElement>(`[data-marca="step"][data-agarre="${i}"]`)!;
  const tamanoDeLaBarra = (container: HTMLElement) => container.querySelector("[data-tamano]")?.textContent ?? null;

  /** Un paso en el centro, y la herramienta suelta. */
  function ponerPaso(container: HTMLElement) {
    fireEvent.keyDown(window, { key: "5" });
    fireEvent.pointerDown(container.querySelector("svg.cursor-crosshair")!, { clientX: 300, clientY: 150 });
    fireEvent.keyDown(window, { key: "Escape" });
  }

  it("con la herramienta del paso puesta, la barra da el tamaño de lo próximo", async () => {
    const { container } = await abrir();
    // Sin texto ni paso a la vista, no hay tamaño que dar.
    expect(tamanoDeLaBarra(container)).toBeNull();
    fireEvent.keyDown(window, { key: "5" });
    expect(tamanoDeLaBarra(container)).toBe("100 %");
    fireEvent.click(screen.getByRole("button", { name: "Más grande" }));
    expect(tamanoDeLaBarra(container)).toBe("125 %");
    fireEvent.pointerDown(container.querySelector("svg.cursor-crosshair")!, { clientX: 300, clientY: 150 });
    const [paso] = await loQueSeGuarda();
    expect(paso).toMatchObject({ kind: "step", escala: 1.25 });
  });

  it("con un paso elegido, se lo cambia, «Guardar» lo manda y Ctrl+Z se lo devuelve", async () => {
    const { container } = await abrir();
    ponerPaso(container);
    fireEvent.pointerDown(pasoEn(container), { button: 0, clientX: 300, clientY: 150 });
    fireEvent.pointerUp(window, { clientX: 300, clientY: 150 });
    expect(tamanoDeLaBarra(container)).toBe("100 %");
    fireEvent.click(screen.getByRole("button", { name: "Más grande" }));
    fireEvent.click(screen.getByRole("button", { name: "Más grande" }));
    expect(tamanoDeLaBarra(container)).toBe("150 %");
    expect((await loQueSeGuarda())[0]).toMatchObject({ kind: "step", escala: 1.5 });
    // Cada pulsación es un cambio: Ctrl+Z quita la última.
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect((await loQueSeGuarda())[0]).toMatchObject({ kind: "step", escala: 1.25 });
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect((await loQueSeGuarda())[0]).not.toHaveProperty("escala");
  });

  it("un texto a medias se queda abierto al cambiarle el tamaño, y sale con el nuevo", async () => {
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "3" });
    fireEvent.pointerDown(container.querySelector("svg.cursor-crosshair")!, { clientX: 300, clientY: 150 });
    const campo = screen.getByRole("textbox");
    fireEvent.change(campo, { target: { value: "grande" } });
    const mas = screen.getByRole("button", { name: "Más grande" });
    // El botón no se lleva el foco: el texto sigue abierto.
    expect(fireEvent.mouseDown(mas)).toBe(false);
    fireEvent.click(mas);
    expect(screen.getByRole("textbox")).toBe(campo);
    fireEvent.keyDown(campo, { key: "Enter" });
    expect((await loQueSeGuarda())[0]).toMatchObject({ kind: "text", text: "grande", escala: 1.25 });
  });
});

// Con la barra justa el tamaño no cabe a la vista: va dentro de los colores plegados.
describe("el tamaño en la barra justa", () => {
  it("los colores plegados lo llevan debajo con el texto, y cambiarlo no los cierra", async () => {
    await abrir();
    // Sin texto ni paso, plegados solo llevan el color.
    expect(screen.getByRole("button", { name: "Color" })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "3" });
    const plegados = screen.getByRole("button", { name: "Color y tamaño" });
    fireEvent.click(plegados);
    const dentro = plegados.parentElement!;
    const mas = [...dentro.querySelectorAll<HTMLButtonElement>("button")].find((b) => b.getAttribute("aria-label") === "Más grande")!;
    fireEvent.click(mas);
    fireEvent.click(mas);
    expect(dentro.querySelector("[data-tamano]")!.textContent).toBe("150 %");
    expect(plegados).toHaveAttribute("aria-expanded", "true");
    // Y abrirlos no se lleva el foco: un texto a medias seguiría abierto.
    expect(fireEvent.mouseDown(plegados)).toBe(false);
  });
});
