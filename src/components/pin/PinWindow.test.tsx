/**
 * La captura anclada.
 *
 * Es una ventana sin barra de titulo y sin nada visible encima de la imagen, asi que casi
 * todo lo que se puede hacer con ella son teclas y gestos. Eso es justo lo que se rompe
 * sin que nadie se entere, porque no se ve en una foto.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PinWindow } from "./PinWindow";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas } from "../../test/preparar";

const RUTA = "C:\\Users\\prueba\\AppData\\Local\\Temp\\winshotx\\pins\\pin-1.png";

const cerrar = vi.fn();
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ close: cerrar }),
}));

beforeEach(() => {
  aplicarIdioma("es");
  cerrar.mockClear();
});

describe("lo que se ve", () => {
  it("ensenna la imagen que le han dado, servida por el protocolo de archivos", () => {
    render(<PinWindow imagen={RUTA} />);
    expect(screen.getByRole("presentation", { hidden: true })).toHaveAttribute(
      "src",
      `asset://${RUTA}`,
    );
  });

  it("los botones estan escondidos hasta que el raton entra", () => {
    const { container } = render(<PinWindow imagen={RUTA} />);
    const botonera = container.querySelector(".absolute.end-1\\.5")!;
    expect(botonera.className).toContain("opacity-0");
    fireEvent.pointerEnter(container.firstElementChild!);
    expect(botonera.className).toContain("opacity-100");
  });

  it("y se esconden otra vez al salir, para no taparle nada a la captura", () => {
    const { container } = render(<PinWindow imagen={RUTA} />);
    const marco = container.firstElementChild!;
    fireEvent.pointerEnter(marco);
    fireEvent.pointerLeave(marco);
    expect(container.querySelector(".absolute.end-1\\.5")!.className).toContain("opacity-0");
  });

  it("la ventana entera se arrastra, no solo un trozo", () => {
    // Sin `data-tauri-drag-region` en el marco Y en la imagen, arrastrar desde el centro
    // no movia nada: el raton cae sobre la imagen, no sobre el marco.
    const { container } = render(<PinWindow imagen={RUTA} />);
    expect(container.firstElementChild).toHaveAttribute("data-tauri-drag-region");
    expect(screen.getByRole("presentation", { hidden: true })).toHaveAttribute(
      "data-tauri-drag-region",
    );
  });
});

describe("como se cierra", () => {
  it("con Escape, igual que todo lo demas de winshotx", () => {
    render(<PinWindow imagen={RUTA} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(cerrar).toHaveBeenCalled();
  });

  it("con el boton de la esquina", () => {
    render(<PinWindow imagen={RUTA} />);
    fireEvent.click(screen.getByLabelText("Cerrar"));
    expect(cerrar).toHaveBeenCalled();
  });

  it("y con doble clic encima, que es lo que la gente prueba sola", () => {
    const { container } = render(<PinWindow imagen={RUTA} />);
    fireEvent.doubleClick(container.firstElementChild!);
    expect(cerrar).toHaveBeenCalled();
  });

  it("una tecla cualquiera no la cierra", () => {
    render(<PinWindow imagen={RUTA} />);
    fireEvent.keyDown(window, { key: "a" });
    expect(cerrar).not.toHaveBeenCalled();
  });
});

describe("copiar lo que hay anclado", () => {
  it("el boton manda la ruta a Rust", async () => {
    render(<PinWindow imagen={RUTA} />);
    fireEvent.click(screen.getByLabelText("Copiar"));
    await screen.findByText("Copiada");
    expect(llamadas).toContainEqual({ comando: "copy_pinned", args: { path: RUTA } });
  });

  it("Ctrl+C hace lo mismo sin ir al raton", async () => {
    render(<PinWindow imagen={RUTA} />);
    fireEvent.keyDown(window, { key: "c", ctrlKey: true });
    await screen.findByText("Copiada");
    expect(llamadas.some((l) => l.comando === "copy_pinned")).toBe(true);
  });

  it("el aviso de copiada no sale hasta que se copia", () => {
    render(<PinWindow imagen={RUTA} />);
    expect(screen.queryByText("Copiada")).toBeNull();
  });

  it("en ingles los dos botones hablan ingles", () => {
    aplicarIdioma("en");
    render(<PinWindow imagen={RUTA} />);
    expect(screen.getByLabelText("Copy")).toBeInTheDocument();
    expect(screen.getByLabelText("Close")).toBeInTheDocument();
    expect(screen.queryByLabelText("Copiar")).toBeNull();
  });
});
