/**
 * El selector de pocas opciones, que desde la 0.5 es texto con la elegida subrayada.
 *
 * Lo que se comprueba es lo que no se ve en una foto: que la elegida lleve la raya y las
 * demás no (antes era una pastilla, y quedaban tres formas de elegir en la app), que lo
 * diga en voz alta para un lector de pantalla, que el grupo se llame como lo que elige, y
 * que la forma compacta de la barra de abajo vaya también en texto con su raya (S40).
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Segmented } from "./Segmented";

const FPS = [
  { value: 15, label: "15 fps" },
  { value: 30, label: "30 fps" },
  { value: 60, label: "60 fps" },
];

function pintar(extra: Partial<Parameters<typeof Segmented<number>>[0]> = {}) {
  const onChange = vi.fn();
  render(<Segmented value={30} options={FPS} onChange={onChange} etiqueta="Fotogramas por segundo" {...extra} />);
  return onChange;
}

describe("las opciones en texto", () => {
  it("la elegida lleva la raya azul y las demás no", () => {
    pintar();
    const elegida = screen.getByRole("button", { name: "30 fps" });
    expect(elegida.className).toMatch(/border-b-\[3px\]/);
    expect(elegida.className).toMatch(/\bborder-marca\b/);
    expect(elegida.className).toMatch(/\btext-titulo\b/);
    for (const nombre of ["15 fps", "60 fps"]) {
      const otra = screen.getByRole("button", { name: nombre });
      expect(otra.className).toMatch(/\bborder-transparent\b/);
      expect(otra.className).toMatch(/\btext-apagado\b/);
      expect(otra.className).not.toMatch(/\bborder-marca\b/);
    }
  });

  it("sin pastilla: ni fondo en el grupo ni en la elegida", () => {
    pintar();
    const grupo = screen.getByRole("group", { name: "Fotogramas por segundo" });
    expect(grupo.className).not.toMatch(/\bbg-/);
    expect(grupo.className).not.toMatch(/\brounded/);
    expect(screen.getByRole("button", { name: "30 fps" }).className).not.toMatch(/\bbg-pastilla\b/);
  });

  it("la letra de las opciones no baja de 14 px y no se parte en dos renglones", () => {
    pintar();
    for (const boton of screen.getAllByRole("button")) {
      expect(boton.className).toMatch(/text-\[14px\]/);
      expect(boton.className).toMatch(/\bwhitespace-nowrap\b/);
    }
  });

  it("dice cuál está elegida a quien no ve el color", () => {
    pintar();
    expect(screen.getByRole("button", { name: "30 fps" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "15 fps" })).toHaveAttribute("aria-pressed", "false");
  });

  it("al pulsar otra, avisa con su valor", () => {
    const onChange = pintar();
    fireEvent.click(screen.getByRole("button", { name: "60 fps" }));
    expect(onChange).toHaveBeenCalledWith(60);
  });

  it("sin etiqueta no se inventa un grupo sin nombre", () => {
    pintar({ etiqueta: undefined });
    expect(screen.queryByRole("group")).toBeNull();
  });

  it("con icono, el nombre sigue estando para el lector y para el globo", () => {
    pintar({
      options: [
        { value: 1, label: "Claro", icono: <svg data-testid="sol" /> },
        { value: 2, label: "Oscuro", icono: <svg data-testid="luna" /> },
      ],
      value: 1,
    });
    const claro = screen.getByRole("button", { name: "Claro" });
    expect(claro).toHaveAttribute("title", "Claro");
    expect(claro).toContainElement(screen.getByTestId("sol"));
  });
});

describe("la forma compacta, la de la barra de abajo", () => {
  it("también en texto con su raya, sin pastilla y a 13 px (S40)", () => {
    pintar({ compacto: true });
    const grupo = screen.getByRole("group", { name: "Fotogramas por segundo" });
    expect(grupo.className).not.toMatch(/\bbg-hueco\b/);
    const elegida = screen.getByRole("button", { name: "30 fps" });
    expect(elegida.className).toMatch(/border-b-2/);
    expect(elegida.className).toMatch(/\bborder-marca\b/);
    expect(elegida.className).toMatch(/text-\[13px\]/);
    expect(elegida.className).not.toMatch(/\bbg-pastilla\b/);
    expect(elegida).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "15 fps" }).className).toMatch(/\bborder-transparent\b/);
  });
});
