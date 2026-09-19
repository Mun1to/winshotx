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
import { falla, llamadas } from "../../test/preparar";

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
    fireEvent.click(screen.getByLabelText(/^Cerrar/));
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
    fireEvent.click(screen.getByLabelText(/^Copiar ·/));
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

  it("en ingles los botones hablan ingles", () => {
    aplicarIdioma("en");
    render(<PinWindow imagen={RUTA} />);
    expect(screen.getByLabelText(/^Copy ·/)).toBeInTheDocument();
    expect(screen.getByLabelText(/^Save ·/)).toBeInTheDocument();
    expect(screen.getByLabelText(/^Close ·/)).toBeInTheDocument();
    expect(screen.queryByLabelText(/^Copiar ·/)).toBeNull();
  });
});

describe("guardar la que estaba anclada", () => {
  // El PNG de una anclada vive en el temporal y se borra al arrancar. Sin esto, quien
  // ancla algo y luego decide que lo quiere tiene que copiarlo y pegarlo en otro programa.
  it("el boton la manda a la carpeta de capturas", async () => {
    render(<PinWindow imagen={RUTA} />);
    fireEvent.click(screen.getByLabelText(/^Guardar ·/));
    await screen.findByText("Guardada");
    expect(llamadas).toContainEqual({ comando: "save_pinned", args: { path: RUTA } });
  });

  it("Ctrl+S hace lo mismo", async () => {
    render(<PinWindow imagen={RUTA} />);
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await screen.findByText("Guardada");
    expect(llamadas.some((l) => l.comando === "save_pinned")).toBe(true);
  });

  it("y le quita a la ventana de debajo su cuadro de guardar la pagina", () => {
    // Sin `preventDefault`, el WebView2 abre su propio dialogo encima de la captura.
    render(<PinWindow imagen={RUTA} />);
    const evento = new KeyboardEvent("keydown", {
      key: "s",
      ctrlKey: true,
      cancelable: true,
      bubbles: true,
    });
    window.dispatchEvent(evento);
    expect(evento.defaultPrevented).toBe(true);
  });
});

describe("leer el texto de la anclada", () => {
  it("la tecla T lo copia, como en la barra de captura", async () => {
    render(<PinWindow imagen={RUTA} />);
    fireEvent.keyDown(window, { key: "t" });
    await screen.findByText("Texto copiado");
    expect(llamadas).toContainEqual({ comando: "pinned_text", args: { path: RUTA } });
  });

  it("Ctrl+T no, que esa es la de abrir pestanna del navegador de debajo", () => {
    render(<PinWindow imagen={RUTA} />);
    fireEvent.keyDown(window, { key: "t", ctrlKey: true });
    expect(llamadas.some((l) => l.comando === "pinned_text")).toBe(false);
  });
});

describe("cuando algo sale mal", () => {
  it("se ensenna el motivo, y en rojo", async () => {
    falla("pinned_text", "No he encontrado texto en esa captura.");
    render(<PinWindow imagen={RUTA} />);
    fireEvent.keyDown(window, { key: "t" });
    const aviso = await screen.findByRole("status");
    expect(aviso).toHaveTextContent("No he encontrado texto en esa captura.");
    expect(aviso.className).toContain("bg-red-500/90");
  });

  it("y traducido, porque el mensaje de Rust es la clave", async () => {
    aplicarIdioma("en");
    falla("pinned_text", "No he encontrado texto en esa captura.");
    render(<PinWindow imagen={RUTA} />);
    fireEvent.keyDown(window, { key: "t" });
    const aviso = await screen.findByRole("status");
    expect(aviso.textContent).not.toContain("captura");
  });

  it("un fallo al guardar no se traga en silencio", async () => {
    falla("save_pinned", "No he podido escribir en esa carpeta.");
    render(<PinWindow imagen={RUTA} />);
    fireEvent.click(screen.getByLabelText(/^Guardar ·/));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "No he podido escribir en esa carpeta.",
    );
  });
});
