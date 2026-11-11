/**
 * El fondo de las capturas con un estilo guardado (S9): elegir uno manda a Rust su fondo
 * (también el propio) y su aire, y elegir un color de nombre quita el propio.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FondoEnLasCapturas } from "./FondoEnLasCapturas";
import { aplicarIdioma } from "../../lib/i18n";
import type { Settings } from "../../lib/types";

const PRESETS = [
  { nombre: "Tutorial", estilo: { aire: 0.05, fondo: "menta" } },
  { nombre: "Marca", estilo: { aire: 0.05, fondo: "noche", fondoElegido: { tipo: "color", rgb: [10, 155, 255] } } },
];

function pintar(ajustes: Partial<Settings>) {
  const patch = vi.fn();
  render(<FondoEnLasCapturas settings={{ ...({} as Settings), ...ajustes }} patch={patch} />);
  return patch;
}

beforeEach(() => aplicarIdioma("es"));

describe("un estilo guardado para todas las capturas", () => {
  it("sin estilos guardados no sale nada nuevo", () => {
    pintar({ marcoCapturas: { aire: 8, fondo: "atardecer" } });
    expect(screen.queryByRole("group", { name: "Estilo guardado para todas las capturas" })).toBeNull();
  });

  it("elegir uno manda su fondo propio y su aire", () => {
    const patch = pintar({ marcoCapturas: { aire: 0, fondo: "atardecer" }, presets: PRESETS });
    fireEvent.click(screen.getByRole("button", { name: "Marca" }));
    expect(patch).toHaveBeenCalledWith({
      marcoCapturas: { aire: 9, fondo: "noche", elegido: { tipo: "color", rgb: [10, 155, 255] } },
    });
  });

  it("el que coincide con lo puesto va marcado, y entonces ningún color de nombre", () => {
    pintar({
      marcoCapturas: { aire: 9, fondo: "noche", elegido: { tipo: "color", rgb: [10, 155, 255] } } as Settings["marcoCapturas"],
      presets: PRESETS,
    });
    expect(screen.getByRole("button", { name: "Marca" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Tutorial" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("radio", { name: "Noche" })).toHaveAttribute("aria-checked", "false");
  });

  it("elegir un color de nombre quita el propio del estilo", () => {
    const patch = pintar({
      marcoCapturas: { aire: 9, fondo: "noche", elegido: { tipo: "color", rgb: [10, 155, 255] } } as Settings["marcoCapturas"],
      presets: PRESETS,
    });
    fireEvent.click(screen.getByRole("radio", { name: "Coral" }));
    expect(patch).toHaveBeenCalledWith({ marcoCapturas: { aire: 9, fondo: "coral" } });
  });

  it("en inglés", () => {
    aplicarIdioma("en");
    pintar({ marcoCapturas: { aire: 8, fondo: "atardecer" }, presets: PRESETS });
    expect(screen.getByText("Or a saved style:")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("estilo guardado");
  });
});
