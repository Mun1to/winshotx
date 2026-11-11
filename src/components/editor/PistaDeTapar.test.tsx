/**
 * La pista de lo tapado (S9, D4) dentro de la línea de tiempo: los mosaicos se ven, se
 * alargan por los bordes y se quitan, y elegir uno lleva la vista previa a donde se ve.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LineaDeTiempo } from "./LineaDeTiempo";
import { aplicarIdioma } from "../../lib/i18n";
import type { Tapa } from "../../lib/contrato";

/** Diez segundos: la tira empieza en x = 100 y mide 1000 px, así que cada píxel son 10 ms. */
const FRAMES = Array.from({ length: 100 }, (_, i) => ({ index: i, timestampMs: i * 100, durationMs: 100, thumbPath: "" }));
const x = (ms: number) => 100 + ms / 10;
const TAPAS: Tapa[] = [
  { desdeMs: 1000, hastaMs: 3000, x: 0.1, y: 0.1, ancho: 0.2, alto: 0.05 },
  { desdeMs: 2000, hastaMs: 4000, x: 0.5, y: 0.5, ancho: 0.2, alto: 0.05 },
];

const original = HTMLElement.prototype.getBoundingClientRect;
beforeEach(() => {
  aplicarIdioma("es");
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    if (this.hasAttribute("data-pista")) {
      return { left: 100, top: 0, width: 1000, height: 56, right: 1100, bottom: 56, x: 100, y: 0, toJSON() {} } as DOMRect;
    }
    if (this.hasAttribute("data-tapa")) {
      const izquierda = parseFloat(this.style.left) * 10 + 100;
      const ancho = parseFloat(this.style.width) * 10;
      return { left: izquierda, top: 0, width: ancho, height: 20, right: izquierda + ancho, bottom: 20, x: izquierda, y: 0, toJSON() {} } as DOMRect;
    }
    return original.call(this);
  });
});
afterEach(() => vi.restoreAllMocks());

function pintar(tapas = TAPAS, elegida: number | null = null) {
  const onTapas = vi.fn();
  const onElegirTapa = vi.fn();
  const onIr = vi.fn();
  const onElegir = vi.fn();
  render(
    <LineaDeTiempo
      frames={FRAMES}
      desdeMs={0}
      hastaMs={10_000}
      posicionMs={0}
      videoRef={createRef<HTMLVideoElement>()}
      reproduciendo={false}
      proporcion={16 / 9}
      delante={null}
      onTramo={() => {}}
      onIr={onIr}
      onFinal={() => {}}
      onElegir={onElegir}
      tapas={tapas}
      onTapas={onTapas}
      tapaElegida={elegida}
      onElegirTapa={onElegirTapa}
    />,
  );
  return { onTapas, onElegirTapa, onIr, onElegir };
}

describe("la pista de lo tapado", () => {
  it("cada mosaico es un bloque en su rato, y dos a la vez van en filas distintas", () => {
    pintar();
    const bloques = screen.getAllByRole("button", { name: /^Mosaico de/ });
    expect(bloques).toHaveLength(2);
    expect(bloques[0]).toHaveAccessibleName("Mosaico de 0:01.0 a 0:03.0");
    expect(bloques[0].style.left).toBe("10%");
    expect(bloques[0].style.width).toBe("20%");
    expect(bloques[0].style.top).not.toBe(bloques[1].style.top);
  });

  it("sin nada tapado no hay pista", () => {
    pintar([]);
    expect(document.querySelector("[data-pista-tapar]")).toBeNull();
  });

  it("un clic en el medio lo elige, suelta el zoom y lleva la vista previa a su principio", () => {
    const { onElegirTapa, onIr, onElegir } = pintar();
    fireEvent.pointerDown(screen.getAllByRole("button", { name: /^Mosaico de/ })[0], { clientX: x(2000), button: 0 });
    expect(onElegirTapa).toHaveBeenCalledWith(0);
    expect(onElegir).toHaveBeenCalledWith(null);
    expect(onIr).toHaveBeenCalledWith(1000);
  });

  it("tirar del borde de la derecha lo alarga hasta donde se suelta", () => {
    const { onTapas } = pintar();
    fireEvent.pointerDown(screen.getAllByRole("button", { name: /^Mosaico de/ })[0], { clientX: x(3000) - 2, button: 0 });
    fireEvent.pointerMove(window, { clientX: x(5500) });
    fireEvent.pointerUp(window);
    const ultima = onTapas.mock.calls.at(-1)![0] as Tapa[];
    expect(ultima[0].hastaMs).toBe(5500);
    expect(ultima[0].desdeMs).toBe(1000);
    expect(ultima[1]).toEqual(TAPAS[1]);
  });

  it("tirar del de la izquierda lo hace empezar antes", () => {
    const { onTapas } = pintar();
    fireEvent.pointerDown(screen.getAllByRole("button", { name: /^Mosaico de/ })[1], { clientX: x(2000) + 2, button: 0 });
    fireEvent.pointerMove(window, { clientX: x(500) });
    fireEvent.pointerUp(window);
    expect((onTapas.mock.calls.at(-1)![0] as Tapa[])[1].desdeMs).toBe(500);
  });

  it("el clic derecho lo quita", () => {
    const { onTapas } = pintar();
    fireEvent.contextMenu(screen.getAllByRole("button", { name: /^Mosaico de/ })[0]);
    expect(onTapas).toHaveBeenCalledWith([TAPAS[1]]);
  });

  it("con uno elegido, Supr lo quita y los mandos de debajo lo dicen", () => {
    const { onTapas, onElegirTapa } = pintar(TAPAS, 1);
    expect(screen.getByText("Mosaico de 0:02.0 a 0:04.0", { selector: "span" })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Delete" });
    expect(onTapas).toHaveBeenCalledWith([TAPAS[0]]);
    expect(onElegirTapa).toHaveBeenCalledWith(null);
  });

  it("«Quitar todos» los quita de una vez", () => {
    const { onTapas } = pintar(TAPAS, 0);
    fireEvent.click(screen.getByRole("button", { name: "Quitar todos (2)" }));
    expect(onTapas).toHaveBeenCalledWith([]);
  });

  it("en inglés", () => {
    aplicarIdioma("en");
    pintar(TAPAS, 0);
    expect(screen.getAllByRole("button", { name: /^Mosaic from/ })).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Remove the mosaic" })).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("Mosaico");
  });
});
