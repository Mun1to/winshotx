/**
 * El logo. Lo que puede cambiar es el color de la X del centro: blanca sobre lo oscuro de
 * siempre, y del color del texto donde el fondo cambia con el tema (el menú de la bandeja).
 * Las esquinas son siempre del azul de la marca, que es lo que se reconoce.
 */
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Marca } from "./Marca";

function trazos() {
  const logo = screen.getByRole("img");
  const [esquinas, aspa] = logo.querySelectorAll("g");
  return { esquinas: esquinas.getAttribute("stroke"), aspa: aspa.getAttribute("stroke") };
}

describe("el logo", () => {
  it("de serie, la X va en blanco y las esquinas en azul", () => {
    render(<Marca />);
    expect(trazos()).toEqual({ esquinas: "#0a9bff", aspa: "#fff" });
  });

  it("con x, la X toma ese color y las esquinas no cambian", () => {
    render(<Marca x="currentColor" />);
    expect(trazos()).toEqual({ esquinas: "#0a9bff", aspa: "currentColor" });
  });

  it("se llama como la app para quien no lo ve", () => {
    render(<Marca />);
    expect(screen.getByRole("img", { name: "winshotx" })).toBeInTheDocument();
  });
});
