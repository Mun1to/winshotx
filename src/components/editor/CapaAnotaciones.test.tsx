/**
 * La capa donde se dibujan las marcas sobre la vista previa.
 *
 * Lo que se comprueba aquí es lo que puede salir mal de verdad: que las coordenadas se
 * guarden **de 0 a 1** y no en píxeles (que es lo que hace que el mismo dibujo valga a
 * cualquier tamaño de exportación), y que un clic sin arrastre no deje una marca invisible
 * de cero píxeles que después no hay forma de quitar.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CapaAnotaciones } from "./CapaAnotaciones";
import { aplicarIdioma } from "../../lib/i18n";
import type { Anotacion } from "../../lib/anotaciones";

/**
 * En happy-dom nada tiene tamaño, así que `getBoundingClientRect` devuelve ceros y la
 * capa no sabría dividir. Se le da una caja de 400 x 200 empezando en (100, 50), que
 * además obliga a las cuentas a restar el origen y no solo a dividir.
 */
function conTamanno() {
    Element.prototype.getBoundingClientRect = vi.fn(() => ({
      x: 100,
      y: 50,
      left: 100,
      top: 50,
      right: 500,
      bottom: 250,
      width: 400,
      height: 200,
      toJSON: () => ({}),
    })) as unknown as typeof Element.prototype.getBoundingClientRect;
}

function pintar(props: Partial<React.ComponentProps<typeof CapaAnotaciones>> = {}) {
  const onAnadir = vi.fn();
  const { container } = render(
    <CapaAnotaciones
      herramienta="box"
      color="#ef4444"
      anotaciones={[]}
      onAnadir={onAnadir}
      texto=""
      {...props}
    />,
  );
  return { onAnadir, svg: container.querySelector("svg")! };
}

beforeEach(() => {
  aplicarIdioma("es");
  conTamanno();
});

describe("dibujar una marca", () => {
  it("guarda las esquinas de 0 a 1, no en píxeles", () => {
    const { onAnadir, svg } = pintar();
    // De la esquina de arriba a la izquierda al centro de la capa.
    fireEvent.pointerDown(svg, { clientX: 100, clientY: 50 });
    fireEvent.pointerMove(svg, { clientX: 300, clientY: 150 });
    fireEvent.pointerUp(svg);

    expect(onAnadir).toHaveBeenCalledTimes(1);
    const marca: Anotacion = onAnadir.mock.calls[0][0];
    expect(marca.x1).toBeCloseTo(0, 5);
    expect(marca.y1).toBeCloseTo(0, 5);
    expect(marca.x2).toBeCloseTo(0.5, 5);
    expect(marca.y2).toBeCloseTo(0.5, 5);
  });

  it("se queda con la herramienta y el color que estén puestos", () => {
    const { onAnadir, svg } = pintar({ herramienta: "highlight", color: "#fbbf24" });
    fireEvent.pointerDown(svg, { clientX: 120, clientY: 60 });
    fireEvent.pointerMove(svg, { clientX: 400, clientY: 200 });
    fireEvent.pointerUp(svg);
    expect(onAnadir.mock.calls[0][0]).toMatchObject({
      kind: "highlight",
      color: "#fbbf24",
    });
  });

  it("un clic sin arrastre no deja una marca de cero píxeles", () => {
    // Sin esto, cada pulsación sin querer dejaba un rectángulo invisible e imposible de
    // quitar, porque no se puede pulsar encima de algo que no se ve.
    const { onAnadir, svg } = pintar();
    fireEvent.pointerDown(svg, { clientX: 200, clientY: 100 });
    fireEvent.pointerUp(svg);
    expect(onAnadir).not.toHaveBeenCalled();
  });

  it("arrastrar fuera de la capa recorta al borde en vez de salirse", () => {
    const { onAnadir, svg } = pintar();
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    fireEvent.pointerMove(svg, { clientX: 9999, clientY: 9999 });
    fireEvent.pointerUp(svg);
    const marca: Anotacion = onAnadir.mock.calls[0][0];
    expect(marca.x2).toBe(1);
    expect(marca.y2).toBe(1);
  });

  it("sin herramienta elegida no se dibuja nada y los clics pasan de largo", () => {
    const { onAnadir, svg } = pintar({ herramienta: null });
    fireEvent.pointerDown(svg, { clientX: 120, clientY: 60 });
    fireEvent.pointerMove(svg, { clientX: 400, clientY: 200 });
    fireEvent.pointerUp(svg);
    expect(onAnadir).not.toHaveBeenCalled();
    expect(svg.getAttribute("class")).toContain("pointer-events-none");
  });
});

describe("el texto", () => {
  it("se pone de un clic, sin arrastrar", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text", texto: "mira esto" });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    expect(onAnadir).toHaveBeenCalledTimes(1);
    expect(onAnadir.mock.calls[0][0]).toMatchObject({ kind: "text", text: "mira esto" });
  });

  it("sin nada escrito no se pone nada", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text", texto: "   " });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    expect(onAnadir).not.toHaveBeenCalled();
  });
});

describe("lo que ya está dibujado", () => {
  const marcas: Anotacion[] = [
    { kind: "box", x1: 0.1, y1: 0.1, x2: 0.5, y2: 0.5, color: "#ef4444", text: "" },
    { kind: "text", x1: 0.6, y1: 0.2, x2: 0.6, y2: 0.2, color: "#0a9bff", text: "aquí" },
  ];

  it("se pinta todo lo que hay, en su orden", () => {
    const { svg } = pintar({ anotaciones: marcas, herramienta: null });
    expect(svg.querySelectorAll("rect")).toHaveLength(1);
    expect(screen.getByText("aquí")).toBeInTheDocument();
  });

  it("el rectángulo sale hueco, para no tapar lo que señala", () => {
    const { svg } = pintar({ anotaciones: [marcas[0]], herramienta: null });
    expect(svg.querySelector("rect")?.getAttribute("fill")).toBe("none");
  });

  it("y se coloca donde dicen sus coordenadas, escaladas al lienzo del SVG", () => {
    const { svg } = pintar({ anotaciones: [marcas[0]], herramienta: null });
    const rect = svg.querySelector("rect")!;
    // El SVG trabaja en un lienzo de 1000 x 1000, así que 0,1 son 100.
    expect(rect.getAttribute("x")).toBe("100");
    expect(rect.getAttribute("width")).toBe("400");
  });
});
