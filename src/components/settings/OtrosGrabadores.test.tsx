/**
 * El aviso de los otros programas que graban lo último (NVIDIA y la Barra de juego).
 *
 * Lo que se mira: que no salga nada si no hay nadie más grabando, que la Barra de juego
 * lleve su botón a los ajustes de Windows, y que «Ya lo tengo apagado» calle a NVIDIA para
 * siempre, también al volver a abrir la ventana.
 */
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { OtrosGrabadores } from "./OtrosGrabadores";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas, responde } from "../../test/preparar";

async function pintar(nvidia: boolean, barraDeJuego: boolean) {
  responde("otros_grabadores", { nvidia, barraDeJuego });
  const vista = render(<OtrosGrabadores />);
  await act(async () => {});
  return vista;
}

beforeEach(() => {
  aplicarIdioma("es");
  window.localStorage.clear();
});

describe("el aviso de los otros grabadores", () => {
  it("no enseña nada si nadie más está grabando", async () => {
    const { container } = await pintar(false, false);
    expect(container).toBeEmptyDOMElement();
  });

  it("la Barra de juego lleva a los ajustes de Windows donde se apaga", async () => {
    await pintar(false, true);
    expect(screen.getByText("La Barra de juego también graba lo último")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Apagarla" }));
    expect(llamadas.some((l) => l.comando === "abrir_capturas_de_windows")).toBe(true);
  });

  it("NVIDIA se calla con «Ya lo tengo apagado», y sigue callado al volver", async () => {
    const primera = await pintar(true, false);
    expect(screen.getByText("NVIDIA también puede estar grabando")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Ya lo tengo apagado" }));
    expect(screen.queryByText("NVIDIA también puede estar grabando")).not.toBeInTheDocument();
    primera.unmount();

    await pintar(true, false);
    expect(screen.queryByText("NVIDIA también puede estar grabando")).not.toBeInTheDocument();
  });

  it("vuelve a mirar al volver a la ventana, que es cuando se ha ido a apagarlo", async () => {
    await pintar(false, true);
    responde("otros_grabadores", { nvidia: false, barraDeJuego: false });
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(screen.queryByText("La Barra de juego también graba lo último")).not.toBeInTheDocument();
  });
});
