/**
 * Dónde cae el globo de ayuda.
 *
 * Un flotante se rompe siempre por los mismos dos sitios: la última fila, que lo manda
 * fuera de la pantalla por abajo, y la columna de la derecha, que lo manda fuera por un
 * lado. Aquí eso son cuentas con números, así que se comprueban sin abrir nada.
 */
import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { sitioDelGlobo, Ayuda } from "./Ayuda";
import { aplicarIdioma } from "../../lib/i18n";

const VENTANA = { ancho: 840, alto: 640 };

/** Un icono de 16 px en (x, y), como los de las filas de ajustes. */
const icono = (x: number, y: number) => ({
  left: x,
  top: y,
  bottom: y + 16,
  width: 16,
  height: 16,
});

/** Y el globo, que es ancho y de dos o tres líneas. */
const GLOBO = { left: 0, top: 0, bottom: 0, width: 304, height: 92 };

describe("dónde se pone el globo", () => {
  it("debajo del icono cuando hay sitio", () => {
    const { x, y } = sitioDelGlobo(icono(40, 176), GLOBO, VENTANA);
    expect(x).toBe(40);
    expect(y).toBe(200);
  });

  it("y encima cuando la fila está abajo del todo", () => {
    // 600 + 16 + 8 + 92 se sale de 640: si no se diera la vuelta, la explicación de la
    // última fila sería justo la que no se puede leer.
    const { y } = sitioDelGlobo(icono(40, 600), GLOBO, VENTANA);
    expect(y).toBe(500);
    expect(y + GLOBO.height).toBeLessThanOrEqual(VENTANA.alto);
  });

  it("se arrima al borde en vez de salirse por la derecha", () => {
    const { x } = sitioDelGlobo(icono(700, 176), GLOBO, VENTANA);
    expect(x).toBe(VENTANA.ancho - GLOBO.width - 8);
    expect(x + GLOBO.width).toBeLessThanOrEqual(VENTANA.ancho);
  });

  it("y nunca se sale por la izquierda, ni en una ventana estrecha", () => {
    const { x } = sitioDelGlobo(icono(40, 176), GLOBO, { ancho: 320, alto: 640 });
    expect(x).toBe(8);
  });
});

describe("el icono que explica", () => {
  it("enseña el texto al pasar por encima y lo quita al salir", () => {
    aplicarIdioma("es");
    render(
      <Ayuda texto="La pantalla se congela al pulsar">
        <svg />
      </Ayuda>,
    );
    const icono = screen.getByRole("button", { name: "Qué hace este ajuste" });

    expect(screen.queryByRole("tooltip")).toBeNull();
    fireEvent.pointerEnter(icono);
    expect(screen.getByRole("tooltip")).toHaveTextContent("La pantalla se congela al pulsar");
    fireEvent.pointerLeave(icono);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("también con el teclado, que es de donde se sale con Escape", () => {
    aplicarIdioma("es");
    render(
      <Ayuda texto="Lo que hace este ajuste">
        <svg />
      </Ayuda>,
    );
    const icono = screen.getByRole("button", { name: "Qué hace este ajuste" });

    fireEvent.focus(icono);
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    // El globo se anuncia al lector de pantalla, que si no es un texto que no existe.
    expect(icono).toHaveAttribute("aria-describedby", screen.getByRole("tooltip").id);
    fireEvent.keyDown(icono, { key: "Escape" });
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});
