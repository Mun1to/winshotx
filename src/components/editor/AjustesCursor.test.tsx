/**
 * Los mandos del cursor y los clics (S7).
 *
 * Lo que importa es lo que viaja en `edicion`: el suavizado elegido, el volumen del clic que
 * nace bajo y se recuerda, y que un mando que no puede hacer nada no salga.
 */
import { act, fireEvent, render, screen } from "@testing-library/react";
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

// S10 (sonidos)
const ipc = vi.hoisted(() => ({
  elegirSonidoDeClic: vi.fn<() => Promise<string | null>>(),
  importarSonidoDeClic: vi.fn<(origen: string) => Promise<string>>(),
}));
vi.mock("../../lib/ipc", async (original) => ({ ...(await original<typeof import("../../lib/ipc")>()), ...ipc }));
const oir = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("../../lib/sonidoDeClic", async (original) => ({
  ...(await original<typeof import("../../lib/sonidoDeClic")>()),
  oirClic: oir,
}));

describe("el sonido de cada clic", () => {
  it("con el sonido encendido se elige entre los tres, y suena al elegirlo", () => {
    const { onEdicion, rerender } = pintar();
    expect(screen.queryByRole("button", { name: "Grave" })).toBeNull();
    rerender(<AjustesCursor edicion={{ sonidoClic: 0.4 }} onEdicion={onEdicion} conPuntero conClics />);
    expect(screen.getByRole("button", { name: "Suave" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "Grave" }));
    expect(onEdicion).toHaveBeenLastCalledWith({ clic: { tipo: "grave" } });
    expect(oir).toHaveBeenLastCalledWith({ tipo: "grave" }, 0.4);
    rerender(<AjustesCursor edicion={{ sonidoClic: 0.4, clic: { tipo: "toque" } }} onEdicion={onEdicion} conPuntero conClics />);
    expect(screen.getByRole("button", { name: "Toque" })).toHaveAttribute("aria-pressed", "true");
  });

  it("«Propio…» trae el archivo, lo copia y lo deja puesto con su nombre", async () => {
    ipc.elegirSonidoDeClic.mockResolvedValueOnce("D:\\sonidos\\mi clic.mp3");
    ipc.importarSonidoDeClic.mockResolvedValueOnce("C:\\datos\\sonidos\\clic-0123456789abcdef-mi clic.wav");
    const { onEdicion, rerender } = pintar({ edicion: { sonidoClic: 0.3 } });
    fireEvent.click(screen.getByRole("button", { name: "Propio…" }));
    await vi.waitFor(() =>
      expect(onEdicion).toHaveBeenLastCalledWith({
        clic: { tipo: "propio", ruta: "C:\\datos\\sonidos\\clic-0123456789abcdef-mi clic.wav" },
      }),
    );
    expect(ipc.importarSonidoDeClic).toHaveBeenCalledWith("D:\\sonidos\\mi clic.mp3");
    rerender(
      <AjustesCursor
        edicion={{ sonidoClic: 0.3, clic: { tipo: "propio", ruta: "C:\\datos\\sonidos\\clic-0123456789abcdef-mi clic.wav" } }}
        onEdicion={onEdicion}
        conPuntero
        conClics
      />,
    );
    expect(screen.getByRole("button", { name: "Propio…" })).toHaveAttribute("aria-pressed", "true");
    await act(async () => {});
        expect(await screen.findByText("mi clic")).toBeInTheDocument();
  });

  it("si el archivo no vale lo dice, y cancelar no cambia nada", async () => {
    ipc.elegirSonidoDeClic.mockResolvedValueOnce(null);
    const { onEdicion } = pintar({ edicion: { sonidoClic: 0.3 } });
    fireEvent.click(screen.getByRole("button", { name: "Propio…" }));
    await act(async () => {});
    expect(onEdicion).not.toHaveBeenCalled();

    ipc.elegirSonidoDeClic.mockResolvedValueOnce("D:\\foto.mp3");
    ipc.importarSonidoDeClic.mockRejectedValueOnce("ese sonido no se puede abrir");
    fireEvent.click(screen.getByRole("button", { name: "Propio…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("ese sonido no se puede abrir");
    expect(onEdicion).not.toHaveBeenCalled();
  });

  it("los nombres están en inglés", () => {
    aplicarIdioma("en");
    pintar({ edicion: { sonidoClic: 0.3 } });
    for (const nombre of ["Soft", "Deep", "Tap", "Your own…"]) expect(screen.getByRole("button", { name: nombre })).toBeInTheDocument();
    expect(EN["Propio…"]).toBe("Your own…");
  });
});
