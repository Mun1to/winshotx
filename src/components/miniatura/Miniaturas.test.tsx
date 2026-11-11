/**
 * Las miniaturas de la esquina después de capturar (S4, E2).
 *
 * Esta ventana no se abre en el escritorio de nadie para probarla: todo lo que hace se
 * comprueba aquí, con Rust de mentira. Lo que se mira es lo que se rompe sin que se vea en
 * una foto: que arrastrar se pida a Rust solo al moverse con el botón pulsado (también
 * empezando encima de un botón, y nunca con un clic), que cada botón pida su orden con SU miniatura, y que se vaya sola al
 * rato pero no con el ratón encima.
 */
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Miniaturas } from "./Miniaturas";
import { aplicarIdioma } from "../../lib/i18n";
import { emite, llamadas, responde } from "../../test/preparar";
import { EVENTO_MINIATURAS, type PilaMiniaturas } from "../../lib/ipc";

const DOS: PilaMiniaturas = {
  lista: [
    { id: 7, vista: "C:\\t\\winshotx\\miniaturas\\vista-7.png", guardada: false },
    { id: 8, vista: "C:\\t\\winshotx\\miniaturas\\vista-8.png", guardada: true },
  ],
  segundos: 10,
};

async function pintar(pila: PilaMiniaturas = DOS) {
  responde("miniaturas_lista", pila);
  const vista = render(<Miniaturas />);
  // La lista llega por una promesa: se deja que se resuelva.
  await act(async () => {});
  return vista;
}

const carta = (id: number) => document.querySelector(`[data-miniatura="${id}"]`)!;
const pedidas = (comando: string) => llamadas.filter((l) => l.comando === comando);

beforeEach(() => aplicarIdioma("es"));
afterEach(() => vi.useRealTimers());

describe("lo que se ve", () => {
  it("una carta por captura, la más nueva abajo, con su imagen pequeña", async () => {
    await pintar();
    const cartas = document.querySelectorAll("[data-miniatura]");
    expect([...cartas].map((c) => c.getAttribute("data-miniatura"))).toEqual(["7", "8"]);
    expect(carta(7).querySelector("img")).toHaveAttribute(
      "src",
      "asset://C:\\t\\winshotx\\miniaturas\\vista-7.png",
    );
  });

  it("los botones solo salen con el ratón encima", async () => {
    await pintar();
    const capa = carta(7).querySelector("div")!;
    expect(capa.className).toContain("opacity-0");
    fireEvent.pointerEnter(carta(7));
    expect(capa.className).toContain("opacity-100");
  });

  it("una ya guardada ofrece enseñarla en vez de guardarla otra vez", async () => {
    await pintar();
    expect(carta(7).textContent).toContain("Guardar");
    expect(carta(8).textContent).toContain("Mostrar");
  });

  it("cuando Rust avisa de una pila nueva, se pinta la nueva", async () => {
    await pintar();
    act(() => emite(EVENTO_MINIATURAS, { lista: [DOS.lista[1]], segundos: 10 }));
    expect(document.querySelectorAll("[data-miniatura]")).toHaveLength(1);
  });
});

describe("las acciones", () => {
  it("cada botón pide su orden con el número de SU miniatura", async () => {
    await pintar();
    fireEvent.pointerEnter(carta(8));
    fireEvent.click(carta(8).querySelector("button[aria-label='Editar']")!);
    fireEvent.click(carta(8).querySelector("button[aria-label='Anclar']")!);
    fireEvent.click(screen.getAllByText("Copiar")[1]);
    fireEvent.click(carta(8).querySelector("button[aria-label='Cerrar']")!);
    await act(async () => {});
    expect(pedidas("miniatura_editar")[0].args).toEqual({ id: 8 });
    expect(pedidas("miniatura_anclar")[0].args).toEqual({ id: 8 });
    expect(pedidas("miniatura_copiar")[0].args).toEqual({ id: 8 });
    expect(pedidas("miniatura_cerrar")[0].args).toEqual({ id: 8 });
  });

  it("copiar avisa de que se ha copiado", async () => {
    await pintar();
    fireEvent.pointerEnter(carta(7));
    fireEvent.click(screen.getAllByText("Copiar")[0]);
    await act(async () => {});
    expect(screen.getByRole("status")).toHaveTextContent("Copiada");
  });
});

describe("arrastrar a otra app", () => {
  it("se pide a Rust al moverse con el botón pulsado, una sola vez", async () => {
    await pintar();
    const c = carta(7);
    fireEvent.pointerDown(c, { button: 0, buttons: 1, clientX: 50, clientY: 50 });
    fireEvent.pointerMove(c, { buttons: 1, clientX: 52, clientY: 51 });
    expect(pedidas("miniatura_arrastrar")).toHaveLength(0);
    fireEvent.pointerMove(c, { buttons: 1, clientX: 70, clientY: 60 });
    fireEvent.pointerMove(c, { buttons: 1, clientX: 90, clientY: 70 });
    await act(async () => {});
    expect(pedidas("miniatura_arrastrar")).toEqual([{ comando: "miniatura_arrastrar", args: { id: 7 } }]);
    expect(c.textContent).toContain("Suéltala en cualquier app");
  });

  it("moverse sin el botón pulsado no arrastra nada", async () => {
    await pintar();
    const c = carta(7);
    fireEvent.pointerMove(c, { buttons: 0, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(c, { buttons: 0, clientX: 90, clientY: 90 });
    expect(pedidas("miniatura_arrastrar")).toHaveLength(0);
  });

  it("pulsar un botón y moverse un poco sigue siendo un clic, no un arrastre", async () => {
    await pintar();
    const c = carta(7);
    fireEvent.pointerEnter(c);
    const copiar = screen.getAllByText("Copiar")[0];
    fireEvent.pointerDown(copiar, { button: 0, buttons: 1, clientX: 100, clientY: 70 });
    fireEvent.pointerMove(c, { buttons: 1, clientX: 106, clientY: 74 });
    expect(pedidas("miniatura_arrastrar")).toHaveLength(0);
    fireEvent.click(copiar);
    expect(pedidas("miniatura_copiar")).toHaveLength(1);
  });

  it("cogida por «Copiar» y movida de verdad, se arrastra y no copia", async () => {
    // Con el ratón encima, los botones ocupan el centro: es por donde la coge todo el mundo.
    await pintar();
    const c = carta(7);
    fireEvent.pointerEnter(c);
    const copiar = screen.getAllByText("Copiar")[0];
    fireEvent.pointerDown(copiar, { button: 0, buttons: 1, clientX: 100, clientY: 70 });
    fireEvent.pointerMove(c, { buttons: 1, clientX: 120, clientY: 80 });
    expect(pedidas("miniatura_arrastrar")).toEqual([{ comando: "miniatura_arrastrar", args: { id: 7 } }]);
    fireEvent.click(copiar);
    expect(pedidas("miniatura_copiar")).toHaveLength(0);
  });
});

describe("se va sola", () => {
  it("al rato que diga «Ajustes»", async () => {
    vi.useFakeTimers();
    await pintar({ lista: [DOS.lista[0]], segundos: 5 });
    act(() => vi.advanceTimersByTime(4900));
    expect(pedidas("miniatura_cerrar")).toHaveLength(0);
    act(() => vi.advanceTimersByTime(200));
    expect(pedidas("miniatura_cerrar")[0].args).toEqual({ id: 7 });
  });

  it("pero no con el ratón encima, y al salir cuenta otra vez entera", async () => {
    vi.useFakeTimers();
    await pintar({ lista: [DOS.lista[0]], segundos: 5 });
    act(() => vi.advanceTimersByTime(3000));
    fireEvent.pointerEnter(carta(7));
    act(() => vi.advanceTimersByTime(30_000));
    expect(pedidas("miniatura_cerrar")).toHaveLength(0);
    fireEvent.pointerLeave(carta(7));
    act(() => vi.advanceTimersByTime(4900));
    expect(pedidas("miniatura_cerrar")).toHaveLength(0);
    act(() => vi.advanceTimersByTime(200));
    expect(pedidas("miniatura_cerrar")).toHaveLength(1);
  });

  it("con «Hasta cerrarla» no se va nunca", async () => {
    vi.useFakeTimers();
    await pintar({ lista: [DOS.lista[0]], segundos: 0 });
    act(() => vi.advanceTimersByTime(600_000));
    expect(pedidas("miniatura_cerrar")).toHaveLength(0);
  });
});

describe("en inglés", () => {
  it("no queda nada en español, tampoco en lo que solo lee un lector de pantalla", async () => {
    aplicarIdioma("en");
    await pintar();
    fireEvent.pointerEnter(carta(7));
    const textos = [
      document.body.textContent ?? "",
      ...[...document.querySelectorAll("[aria-label],[title]")].flatMap((el) => [
        el.getAttribute("aria-label") ?? "",
        el.getAttribute("title") ?? "",
      ]),
    ].join(" ");
    for (const espanol of ["Copiar", "Guardar", "Mostrar", "Cerrar", "Editar", "Anclar", "Arrástrala"]) {
      expect(textos).not.toContain(espanol);
    }
  });
});
