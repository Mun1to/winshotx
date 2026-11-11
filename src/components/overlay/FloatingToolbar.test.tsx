/**
 * La barra de acciones que sale al soltar el recorte de una foto.
 *
 * Lo que se comprueba es el botón de leer los códigos QR (S4, E5): que solo sale cuando
 * quien monta la barra le da algo que hacer, y que habla el idioma de la app, porque es un
 * botón de solo icono y todo lo que dice vive en su `aria-label` y su `title`.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FloatingToolbar } from "./FloatingToolbar";
import { aplicarIdioma } from "../../lib/i18n";

function pintar(onQr?: () => void) {
  const nada = () => {};
  render(
    <FloatingToolbar
      left={0}
      top={0}
      flipped={false}
      busy={false}
      modo="still"
      onCopy={nada}
      onSave={nada}
      onSaveAs={nada}
      onEdit={nada}
      onPin={nada}
      onText={nada}
      onQr={onQr}
      onRecord={nada}
      onCancel={nada}
    />,
  );
}

beforeEach(() => aplicarIdioma("es"));

describe("leer el código QR", () => {
  it("sale al lado de copiar el texto y lo pide al pulsarlo", () => {
    const onQr = vi.fn();
    pintar(onQr);
    fireEvent.click(screen.getByLabelText("Leer el código QR"));
    expect(onQr).toHaveBeenCalledTimes(1);
    const botones = screen.getAllByRole("button").map((b) => b.getAttribute("aria-label"));
    expect(botones.indexOf("Leer el código QR")).toBe(botones.indexOf("Copiar el texto") + 1);
  });

  it("sin nada que hacer no sale, en vez de salir un botón muerto", () => {
    pintar();
    expect(screen.queryByLabelText("Leer el código QR")).toBeNull();
  });

  it("habla inglés con la app en inglés", () => {
    aplicarIdioma("en");
    pintar(vi.fn());
    const boton = screen.getByLabelText("Read the QR code");
    expect(boton.getAttribute("title")).toBe("Read the QR code");
  });
});
