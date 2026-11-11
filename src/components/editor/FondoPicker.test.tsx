/**
 * El selector de fondos del estudio.
 *
 * Lo que importa no se ve en una foto: qué se le pide a Rust y qué fondo sale hacia el
 * panel. Un fondo propio que no llega a `onElegido` se queda en la pantalla y no en el
 * archivo; una importación cancelada que se enseña como un error asusta a quien la canceló.
 */
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FondoPicker } from "./FondoPicker";
import { aplicarIdioma } from "../../lib/i18n";
import { cssDelFondo } from "../../lib/fondos";
import { EVENTO_PROGRESO_FONDO, FONDO_CANCELADO } from "../../lib/ipc";
import { emite, falla, llamadas, responde } from "../../test/preparar";
import type { FondoElegido } from "../../lib/contrato";

function pintar(props: Partial<React.ComponentProps<typeof FondoPicker>> = {}) {
  const onFondo = vi.fn();
  const onElegido = vi.fn();
  const pintado = render(
    <FondoPicker fondo="atardecer" conMarco elegido={null} onFondo={onFondo} onElegido={onElegido} {...props} />,
  );
  return { onFondo, onElegido, ...pintado };
}

/** Una promesa que se resuelve cuando lo diga la prueba: Rust a medio importar. */
function enMarcha<T>() {
  let resolver!: (valor: T) => void;
  let rechazar!: (motivo: unknown) => void;
  const promesa = new Promise<T>((si, no) => {
    resolver = si;
    rechazar = no;
  });
  return { promesa, resolver, rechazar };
}

const pedido = (comando: string) => llamadas.filter((l) => l.comando === comando);

beforeEach(() => aplicarIdioma("es"));

describe("los fondos de nombre", () => {
  it("siguen ahí y se eligen como siempre", () => {
    const { onFondo } = pintar();
    fireEvent.click(screen.getByRole("button", { name: "Menta" }));
    expect(onFondo).toHaveBeenCalledWith("menta");
    fireEvent.click(screen.getByRole("button", { name: "Sin fondo" }));
    expect(onFondo).toHaveBeenCalledWith(null);
  });

  it("con uno propio puesto, ninguno de nombre sale marcado", () => {
    pintar({ elegido: { tipo: "color", rgb: [1, 2, 3] } });
    expect(screen.getByRole("button", { name: "Atardecer" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("button", { name: /Más colores/ })).toHaveAttribute("aria-pressed", "true");
  });

  it("sin marco, «Sin fondo» es el marcado aunque haya uno propio guardado", () => {
    pintar({ conMarco: false, elegido: { tipo: "escritorio" } });
    expect(screen.getByRole("button", { name: "Sin fondo" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: /Escritorio/ })).toHaveAttribute("aria-pressed", "false");
  });
});

describe("una imagen tuya (B1)", () => {
  it("se elige, se copia a la carpeta de fondos y sale la copia", async () => {
    responde("elegir_archivo_de_fondo", "C:\\Users\\yo\\foto.png");
    responde("importar_imagen_de_fondo", "C:\\datos\\fondos\\imagen-1.jpg");
    const { onElegido } = pintar();
    fireEvent.click(screen.getByRole("button", { name: /Imagen/ }));
    await waitFor(() => expect(onElegido).toHaveBeenCalledWith({ tipo: "imagen", ruta: "C:\\datos\\fondos\\imagen-1.jpg" }));
    expect(pedido("elegir_archivo_de_fondo")[0].args).toEqual({ tipo: "imagen" });
    expect(pedido("importar_imagen_de_fondo")[0].args).toEqual({ origen: "C:\\Users\\yo\\foto.png" });
  });

  it("cerrar el diálogo sin elegir no toca nada", async () => {
    responde("elegir_archivo_de_fondo", null);
    const { onElegido } = pintar();
    fireEvent.click(screen.getByRole("button", { name: /Imagen/ }));
    await waitFor(() => expect(pedido("elegir_archivo_de_fondo")).toHaveLength(1));
    expect(pedido("importar_imagen_de_fondo")).toHaveLength(0);
    expect(onElegido).not.toHaveBeenCalled();
  });

  it("una imagen que no se puede abrir lo dice", async () => {
    responde("elegir_archivo_de_fondo", "C:\\roto.png");
    falla("importar_imagen_de_fondo", "esa imagen no se puede abrir");
    const { onElegido } = pintar();
    fireEvent.click(screen.getByRole("button", { name: /Imagen/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("esa imagen no se puede abrir");
    expect(onElegido).not.toHaveBeenCalled();
  });

  it("puesta, el botón enseña la imagen en pequeño en vez del dibujo", () => {
    pintar({ elegido: { tipo: "imagen", ruta: "C:\\datos\\fondos\\imagen-1.jpg" } });
    const boton = screen.getByRole("button", { name: /Imagen/ });
    expect(boton).toHaveAttribute("aria-pressed", "true");
    expect(boton.querySelector("span[style]")?.getAttribute("style")).toContain("imagen-1.jpg");
  });
});

describe("tu fondo de escritorio (B2)", () => {
  it("con un clic, y la vista previa enseña lo que tiene Windows", async () => {
    responde("fondo_escritorio", { tipo: "color", rgb: [0, 120, 215] });
    const { onElegido } = pintar();
    fireEvent.click(screen.getByRole("button", { name: /Escritorio/ }));
    await waitFor(() => expect(onElegido).toHaveBeenCalledWith({ tipo: "escritorio" }));
    expect(cssDelFondo("blanco", { tipo: "escritorio" })).toBe("rgb(0 120 215)");
  });
});

describe("un vídeo o un GIF tuyo (B5)", () => {
  it("enseña su progreso mientras se importa y al acabar queda puesto", async () => {
    const importa = enMarcha<unknown>();
    responde("elegir_archivo_de_fondo", "C:\\Videos\\olas.mp4");
    responde("importar_fondo_animado", importa.promesa);
    const { onElegido } = pintar();
    fireEvent.click(screen.getByRole("button", { name: /Vídeo o GIF/ }));
    await waitFor(() => expect(pedido("importar_fondo_animado")).toHaveLength(1));
    expect(pedido("elegir_archivo_de_fondo")[0].args).toEqual({ tipo: "video" });

    act(() => emite(EVENTO_PROGRESO_FONDO, { hecho: 0.42 }));
    expect(screen.getByRole("status")).toHaveTextContent("Importando el fondo 42 %");
    // Mientras importa no se puede empezar otra cosa.
    expect(screen.getByRole("button", { name: /Imagen/ })).toBeDisabled();

    await act(async () =>
      importa.resolver({ ruta: "C:\\datos\\fondos\\video-1.mp4", vista: "C:\\datos\\fondos\\video-1.jpg", ancho: 1920, alto: 1080, duraMs: 5000 }),
    );
    expect(onElegido).toHaveBeenCalledWith({ tipo: "video", ruta: "C:\\datos\\fondos\\video-1.mp4" });
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("se puede cancelar, y cancelar no se enseña como un fallo", async () => {
    const importa = enMarcha<unknown>();
    responde("elegir_archivo_de_fondo", "C:\\Videos\\largo.mkv");
    responde("importar_fondo_animado", importa.promesa);
    const { onElegido } = pintar();
    fireEvent.click(screen.getByRole("button", { name: /Vídeo o GIF/ }));
    await waitFor(() => expect(pedido("importar_fondo_animado")).toHaveLength(1));
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(pedido("cancelar_fondo_animado")).toHaveLength(1);
    await act(async () => importa.rechazar(FONDO_CANCELADO));
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(onElegido).not.toHaveBeenCalled();
  });

  it("un vídeo que Windows no sabe abrir lo dice", async () => {
    responde("elegir_archivo_de_fondo", "C:\\Videos\\raro.webm");
    falla("importar_fondo_animado", "Windows no sabe abrir ese vídeo");
    pintar();
    fireEvent.click(screen.getByRole("button", { name: /Vídeo o GIF/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Windows no sabe abrir ese vídeo");
  });

  it("cerrar el panel a mitad de importar la cancela", async () => {
    responde("elegir_archivo_de_fondo", "C:\\Videos\\olas.mp4");
    responde("importar_fondo_animado", enMarcha<unknown>().promesa);
    const { unmount } = pintar();
    fireEvent.click(screen.getByRole("button", { name: /Vídeo o GIF/ }));
    await waitFor(() => expect(pedido("importar_fondo_animado")).toHaveLength(1));
    unmount();
    expect(pedido("cancelar_fondo_animado")).toHaveLength(1);
  });
});

describe("cualquier color (B3)", () => {
  it("la paleta va detrás de «Más colores» y no está a la vista de entrada", () => {
    pintar();
    expect(screen.queryByRole("slider", { name: "Tono" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Más colores/ }));
    expect(screen.getByRole("slider", { name: "Tono" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Más colores/ })).toHaveAttribute("aria-expanded", "true");
  });

  it("un código pegado pone ese color, en degradado de entrada y liso si se pide", () => {
    const { onElegido } = pintar();
    fireEvent.click(screen.getByRole("button", { name: /Más colores/ }));
    const campo = screen.getByRole("textbox", { name: "Código del color" });
    fireEvent.change(campo, { target: { value: "#0c6" } });
    fireEvent.keyDown(campo, { key: "Enter" });
    expect(onElegido).toHaveBeenLastCalledWith({ tipo: "degradado", rgb: [0, 204, 102] });
    fireEvent.click(screen.getByRole("button", { name: "Liso" }));
    expect(onElegido).toHaveBeenLastCalledWith({ tipo: "color", rgb: [0, 204, 102] });
  });

  it("un código que no es un color se deshace sin cambiar nada", () => {
    const { onElegido } = pintar();
    fireEvent.click(screen.getByRole("button", { name: /Más colores/ }));
    const campo = screen.getByRole("textbox", { name: "Código del color" });
    fireEvent.change(campo, { target: { value: "azulito" } });
    fireEvent.blur(campo);
    expect(onElegido).not.toHaveBeenCalled();
    expect(campo).toHaveValue("#5874f5");
  });

  it("pulsar en el cuadro elige la saturación y el brillo de ese punto", () => {
    const elegido: FondoElegido = { tipo: "color", rgb: [255, 0, 0] };
    const { onElegido } = pintar({ elegido });
    const cuadro = screen.getByRole("slider", { name: "Saturación y brillo" });
    cuadro.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100, toJSON: () => ({}) });
    // La esquina de arriba a la derecha es el tono puro; la de abajo, negro.
    fireEvent.pointerDown(cuadro, { clientX: 200, clientY: 0, pointerId: 1 });
    expect(onElegido).toHaveBeenLastCalledWith({ tipo: "color", rgb: [255, 0, 0] });
    fireEvent.pointerDown(cuadro, { clientX: 0, clientY: 100, pointerId: 1 });
    expect(onElegido).toHaveBeenLastCalledWith({ tipo: "color", rgb: [0, 0, 0] });
    // Y con las flechas también, para quien no usa ratón.
    fireEvent.keyDown(cuadro, { key: "ArrowUp", shiftKey: true });
    expect(onElegido).toHaveBeenLastCalledWith({ tipo: "color", rgb: [26, 26, 26] });
  });

  it("el tono se mueve con las flechas y no se pierde en un gris", () => {
    const { onElegido } = pintar({ elegido: { tipo: "degradado", rgb: [255, 0, 0] } });
    const barra = screen.getByRole("slider", { name: "Tono" });
    fireEvent.keyDown(barra, { key: "ArrowRight", shiftKey: true });
    expect(onElegido).toHaveBeenLastCalledWith({ tipo: "degradado", rgb: [255, 128, 0] });
    expect(barra).toHaveAttribute("aria-valuenow", "30");
  });
});
