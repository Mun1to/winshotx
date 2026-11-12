/**
 * Los mandos de la webcam en el Estudio: lo que importa es lo que sale hacia la petición
 * (`onEdicion`), que es lo que acaba en Rust al guardar.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AjustesCamara } from "./AjustesCamara";
import { aplicarIdioma } from "../../lib/i18n";
import { marcoDeFabrica, SIN_RECORTE } from "../../lib/camara";
import type { Edicion, MarcoCamara } from "../../lib/contrato";
import { responde } from "../../test/preparar";

const CAMARA = { ruta: "C:\\sesion\\camara.mp4", ancho: 1280, alto: 720, desfaseMs: 0, primerMs: 0 };
const PUESTA: MarcoCamara = { ...marcoDeFabrica(1920, 1080) };

beforeEach(() => {
  aplicarIdioma("es");
  responde("camara_de_la_sesion", CAMARA);
});
afterEach(() => vi.restoreAllMocks());

function pintar(edicion: Edicion = { camara: PUESTA }) {
  const onEdicion = vi.fn();
  const pintado = render(<AjustesCamara sessionId="s" edicion={edicion} onEdicion={onEdicion} />);
  return { onEdicion, ...pintado };
}

describe("los mandos de la cámara", () => {
  it("sin cámara en la sesión no sale nada", async () => {
    responde("camara_de_la_sesion", null);
    const { container } = pintar();
    await new Promise((r) => setTimeout(r, 30));
    expect(container).toBeEmptyDOMElement();
  });

  it("con cámara sale encendida, también antes de que la vista previa ponga la de fábrica", async () => {
    pintar({});
    const interruptor = await screen.findByRole("switch", { name: /La cámara/ });
    expect(interruptor).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("button", { name: "Círculo" })).toHaveAttribute("aria-pressed", "true");
  });

  it("las cuatro formas se leen con su nombre, no solo con un dibujo", async () => {
    // Con cuatro dibujos finos sin nombre, Munito creyó que solo había círculo (12-nov).
    pintar({});
    const grupo = await screen.findByRole("group", { name: "Forma de la cámara" });
    expect(grupo).toHaveTextContent("Círculo");
    expect(grupo).toHaveTextContent("Cuadrado");
    expect(grupo).toHaveTextContent("Rectángulo");
    expect(grupo).toHaveTextContent("Redondeado");
  });

  it("la forma, el tamaño y el borde van a la petición", async () => {
    const { onEdicion } = pintar();
    fireEvent.click(await screen.findByRole("button", { name: "Rectángulo redondeado" }));
    expect(onEdicion).toHaveBeenLastCalledWith({ camara: { ...PUESTA, forma: "redondeado" } });
    fireEvent.change(screen.getByRole("slider", { name: "Tamaño" }), { target: { value: "30" } });
    expect(onEdicion).toHaveBeenLastCalledWith({ camara: { ...PUESTA, tam: 0.3 } });
    fireEvent.click(screen.getByRole("switch", { name: "Con borde" }));
    expect(onEdicion).toHaveBeenLastCalledWith({ camara: { ...PUESTA, borde: true } });
  });

  it("apagarla manda null, y encenderla otra vez vuelve a la que había", async () => {
    const colocada: MarcoCamara = { ...PUESTA, forma: "cuadrado", x: 0.2, y: 0.3, tam: 0.25 };
    const { onEdicion, rerender } = pintar({ camara: colocada });
    fireEvent.click(await screen.findByRole("switch", { name: /La cámara/ }));
    expect(onEdicion).toHaveBeenLastCalledWith({ camara: null });
    rerender(<AjustesCamara sessionId="s" edicion={{ camara: null }} onEdicion={onEdicion} />);
    expect(screen.queryByRole("button", { name: "Círculo" })).toBeNull();
    fireEvent.click(screen.getByRole("switch", { name: /La cámara/ }));
    expect(onEdicion).toHaveBeenLastCalledWith({ camara: colocada });
  });

  it("arrastrar sobre la imagen de la cámara la recorta, y «Quitar el recorte» la deja entera", async () => {
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue({
      x: 0, y: 0, left: 0, top: 0, width: 200, height: 150, right: 200, bottom: 150, toJSON: () => ({}),
    } as DOMRect);
    const { onEdicion, rerender } = pintar();
    const capa = await screen.findByRole("figure");
    fireEvent.pointerDown(capa, { clientX: 20, clientY: 30 });
    fireEvent.pointerMove(capa, { clientX: 100, clientY: 150 });
    fireEvent.pointerUp(capa, { clientX: 100, clientY: 150 });
    const recortada = onEdicion.mock.lastCall?.[0].camara as MarcoCamara;
    expect(recortada.recorte.x1).toBeCloseTo(0.1, 5);
    expect(recortada.recorte.y1).toBeCloseTo(0.2, 5);
    expect(recortada.recorte.x2).toBeCloseTo(0.5, 5);
    expect(recortada.recorte.y2).toBeCloseTo(1, 5);
    rerender(<AjustesCamara sessionId="s" edicion={{ camara: recortada }} onEdicion={onEdicion} />);
    fireEvent.click(screen.getByRole("button", { name: "Quitar el recorte" }));
    expect(onEdicion).toHaveBeenLastCalledWith({ camara: { ...recortada, recorte: SIN_RECORTE } });
  });

  it("en inglés no se cuela ninguna frase en español", async () => {
    aplicarIdioma("en");
    const { container } = pintar();
    await screen.findByRole("switch", { name: /The camera/ });
    await waitFor(() => expect(container.textContent).toContain("With a border"));
    for (const frase of ["cámara", "arrástrala", "Arrastra", "Recorte", "Con borde", "Tamaño"]) {
      expect(container.textContent).not.toContain(frase);
    }
  });
});
