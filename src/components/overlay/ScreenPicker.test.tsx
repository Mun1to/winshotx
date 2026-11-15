/**
 * El cartel de «esta es la pantalla 2» de la captura de pantalla entera.
 *
 * Lo que se comprueba es que habla el idioma de la app: «capturarla entera» y «para
 * llevarte las 3 de una vez» estaban escritas en el código sin pasar por el diccionario,
 * y salían en español con la app en inglés (S37, 15-nov-2026).
 */
import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { ScreenPicker } from "./ScreenPicker";
import { aplicarIdioma } from "../../lib/i18n";

afterEach(() => aplicarIdioma("es"));

describe("el cartel de la pantalla entera", () => {
  it("en español, con la tecla de cada pantalla y la de todas", () => {
    aplicarIdioma("es");
    render(<ScreenPicker numero={2} total={3} modo="still" ancho={1920} alto={1080} />);
    expect(screen.getByText("capturarla entera")).toBeInTheDocument();
    expect(screen.getByText("para llevarte las 3 de una vez")).toBeInTheDocument();
  });

  it("en inglés no queda nada en español", () => {
    aplicarIdioma("en");
    const { container } = render(<ScreenPicker numero={2} total={3} modo="still" ancho={1920} alto={1080} />);
    expect(screen.getByText("capture all of it")).toBeInTheDocument();
    expect(screen.getByText("to take all 3 at once")).toBeInTheDocument();
    expect(container.textContent).not.toMatch(/capturarla|llevarte|clic para/);
  });

  it("grabando dice lo que va a grabar, también en GIF", () => {
    aplicarIdioma("en");
    const { rerender } = render(<ScreenPicker numero={1} total={1} modo="video" ancho={1920} alto={1080} />);
    expect(screen.getByText("record all of it")).toBeInTheDocument();
    rerender(<ScreenPicker numero={1} total={1} modo="gif" ancho={1920} alto={1080} />);
    expect(screen.getByText("record all of it as a GIF")).toBeInTheDocument();
  });

  it("las teclas, a 12 px como poco", () => {
    render(<ScreenPicker numero={2} total={3} modo="still" ancho={1920} alto={1080} />);
    for (const tecla of document.querySelectorAll("kbd")) expect(tecla.className).toContain("text-[12px]");
  });
});
