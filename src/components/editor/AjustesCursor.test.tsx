/**
 * Los mandos del cursor y los clics (S7).
 *
 * Lo que importa es lo que viaja en `edicion`: el suavizado elegido, el volumen del clic que
 * nace bajo y se recuerda, y que un mando que no puede hacer nada no salga.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AjustesCursor, VOLUMEN_CLIC_DE_SERIE } from "./AjustesCursor";
import { aplicarIdioma } from "../../lib/i18n";
import type { Edicion } from "../../lib/contrato";
import { EN } from "../../lib/textos-en";

function pintar(props: Partial<React.ComponentProps<typeof AjustesCursor>> = {}) {
  const onEdicion = vi.fn();
  const pintado = render(
    <AjustesCursor edicion={{}} onEdicion={onEdicion} conPuntero conClics {...props} />,
  );
  return { onEdicion, ...pintado };
}

beforeEach(() => aplicarIdioma("es"));

afterEach(() => {
  localStorage.clear();
  aplicarIdioma("es");
});

describe("los mandos del cursor", () => {
  it("el suavizado sale en «poco» sin tocar nada, que es lo de antes, y elegir otro lo manda", () => {
    const { onEdicion } = pintar();
    expect(screen.getByRole("button", { name: "Poco" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "Mucho" }));
    expect(onEdicion).toHaveBeenLastCalledWith({ suavizado: "mucho" });
    fireEvent.click(screen.getByRole("button", { name: "Nada" }));
    expect(onEdicion).toHaveBeenLastCalledWith({ suavizado: "nada" });
  });

  it("el sonido de clic nace bajo, se recuerda, y apagarlo es volumen cero", () => {
    const { onEdicion, rerender } = pintar();
    expect(screen.queryByRole("slider", { name: "Volumen del clic" })).toBeNull();
    fireEvent.click(screen.getByRole("switch", { name: /Sonido de clic/ }));
    expect(onEdicion).toHaveBeenLastCalledWith({ sonidoClic: VOLUMEN_CLIC_DE_SERIE });
    expect(VOLUMEN_CLIC_DE_SERIE).toBeLessThanOrEqual(0.3);

    rerender(<AjustesCursor edicion={{ sonidoClic: 0.3 }} onEdicion={onEdicion} conPuntero conClics />);
    fireEvent.change(screen.getByRole("slider", { name: "Volumen del clic" }), { target: { value: "55" } });
    expect(onEdicion).toHaveBeenLastCalledWith({ sonidoClic: 0.55 });
    fireEvent.click(screen.getByRole("switch", { name: /Sonido de clic/ }));
    expect(onEdicion).toHaveBeenLastCalledWith({ sonidoClic: 0 });

    // Encendido otra vez, vuelve al volumen que se eligió.
    rerender(<AjustesCursor edicion={{ sonidoClic: 0 }} onEdicion={onEdicion} conPuntero conClics />);
    fireEvent.click(screen.getByRole("switch", { name: /Sonido de clic/ }));
    expect(onEdicion).toHaveBeenLastCalledWith({ sonidoClic: 0.55 });
  });

  it("la estela, la onda, esconderlo y acercarse al escribir viajan en edicion", () => {
    const { onEdicion } = pintar();
    fireEvent.change(screen.getByRole("slider", { name: "Estela al moverse" }), { target: { value: "40" } });
    expect(onEdicion).toHaveBeenLastCalledWith({ estela: 0.4 });
    fireEvent.click(screen.getByRole("switch", { name: /Una onda en cada clic/ }));
    expect(onEdicion).toHaveBeenLastCalledWith({ ondaClic: true });
    fireEvent.click(screen.getByRole("switch", { name: /Esconderlo mientras escribes/ }));
    expect(onEdicion).toHaveBeenLastCalledWith({ esconderAlEscribir: true });
    fireEvent.click(screen.getByRole("switch", { name: /Acercarse mientras escribes/ }));
    expect(onEdicion).toHaveBeenLastCalledWith({ zoomAlEscribir: true });
  });

  it("lo que no puede hacer nada no sale", () => {
    pintar({ conPuntero: false, conClics: false, conEscritura: false });
    expect(screen.queryByRole("button", { name: "Poco" })).toBeNull();
    expect(screen.queryByRole("slider", { name: "Estela al moverse" })).toBeNull();
    expect(screen.queryByRole("switch")).toBeNull();
  });

  it("sin puntero pero con zoom, la estela sigue (es también de la cámara)", () => {
    pintar({ conPuntero: false, conClics: false, conEscritura: false, conZoom: true });
    expect(screen.getByRole("slider", { name: "Estela al moverse" })).toBeTruthy();
  });

  it("en inglés y en chino no se cuela ninguna frase en español", () => {
    const frases = [
      "Movimiento suave",
      "Nada",
      "Poco",
      "Mucho",
      "Esconderlo mientras escribes",
      "vuelve al mover el ratón",
      "Estela al moverse",
      "sin estela",
      "Una onda en cada clic",
      "Sonido de clic",
      "se oye en el vídeo, también si no grabaste sonido",
      "Volumen del clic",
      "Acercarse mientras escribes",
      "al cursor de texto, o al ratón si no se ve",
    ];
    for (const frase of frases) expect(EN[frase], frase).toBeTruthy();
    aplicarIdioma("en");
    const edicion: Edicion = { sonidoClic: 0.3 };
    render(<AjustesCursor edicion={edicion} onEdicion={() => {}} conPuntero conClics />);
    expect(screen.getByRole("button", { name: "Lots" })).toBeTruthy();
    expect(screen.getByRole("switch", { name: /Click sound/ })).toBeTruthy();
  });
});
