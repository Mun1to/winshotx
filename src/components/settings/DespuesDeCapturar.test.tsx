/**
 * Las filas de «Después de capturar» (S4): la miniatura de la esquina y el fondo para
 * todas las capturas. Lo que importa es lo que se manda a guardar, que es lo que lee Rust,
 * y que unos ajustes viejos (sin estos campos) se lean con lo de fábrica.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DespuesDeCapturar, ajustesDeDuracion, duracionDe } from "./DespuesDeCapturar";
import { aplicarIdioma } from "../../lib/i18n";
import type { Settings } from "../../lib/types";

/** Solo lo que miran estas filas: el resto de los ajustes no les importa. */
const VIEJOS = {} as Settings;

function pintar(ajustes: Partial<Settings> = {}) {
  const patch = vi.fn();
  render(<DespuesDeCapturar settings={{ ...VIEJOS, ...ajustes }} patch={patch} />);
  return patch;
}

beforeEach(() => aplicarIdioma("es"));

describe("la miniatura", () => {
  it("unos ajustes viejos la traen como Rust: a los 10 segundos", () => {
    expect(duracionDe(VIEJOS)).toBe("10");
  });

  it("cada opción guarda lo que Rust entiende", () => {
    expect(ajustesDeDuracion("no")).toEqual({ miniatura: false });
    expect(ajustesDeDuracion("5")).toEqual({ miniatura: true, miniaturaSegundos: 5 });
    expect(ajustesDeDuracion("siempre")).toEqual({ miniatura: true, miniaturaSegundos: 0 });
    expect(duracionDe({ ...VIEJOS, miniatura: true, miniaturaSegundos: 0 })).toBe("siempre");
    expect(duracionDe({ ...VIEJOS, miniatura: false, miniaturaSegundos: 30 })).toBe("no");
  });

  it("pulsar «No» la apaga", () => {
    const patch = pintar();
    fireEvent.click(screen.getByRole("button", { name: "No" }));
    expect(patch).toHaveBeenCalledWith({ miniatura: false });
  });
});

describe("el fondo", () => {
  it("de fábrica no hay, y entonces no sale la fila del color", () => {
    pintar();
    expect(screen.queryByRole("radiogroup", { name: "Color del fondo" })).toBeNull();
  });

  it("elegir el aire lo guarda con el fondo que hubiera", () => {
    const patch = pintar({ marcoCapturas: { aire: 0, fondo: "menta" } });
    fireEvent.click(screen.getByRole("button", { name: "Mucho" }));
    expect(patch).toHaveBeenCalledWith({ marcoCapturas: { aire: 14, fondo: "menta" } });
  });

  it("con fondo puesto sale el color, y elegir uno lo guarda", () => {
    const patch = pintar({ marcoCapturas: { aire: 8, fondo: "atardecer" } });
    expect(screen.getByRole("radio", { name: "Atardecer" })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(screen.getByRole("radio", { name: "Noche" }));
    expect(patch).toHaveBeenCalledWith({ marcoCapturas: { aire: 8, fondo: "noche" } });
  });
});

describe("en inglés", () => {
  it("no queda nada en español", () => {
    aplicarIdioma("en");
    pintar({ marcoCapturas: { aire: 8, fondo: "atardecer" } });
    const textos = [
      document.body.textContent ?? "",
      ...[...document.querySelectorAll("[aria-label],[title]")].flatMap((el) => [
        el.getAttribute("aria-label") ?? "",
        el.getAttribute("title") ?? "",
      ]),
    ].join(" ");
    for (const espanol of ["Después de capturar", "Miniatura", "Fondo en", "Hasta cerrarla", "Mucho", "Color del fondo", "Atardecer"]) {
      expect(textos).not.toContain(espanol);
    }
  });
});
