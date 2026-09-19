/**
 * El marco que decide qué trozo se exporta.
 *
 * Todo lo que hace es arrastrar, y arrastrar es justo lo que no se ve en una foto de
 * pantalla. Lo que se comprueba es lo que puede salir mal de verdad: que las coordenadas
 * salgan de 0 a 1, que un clic sin querer no reduzca la exportación a un píxel, y que la
 * capa no se coma los clics cuando no está puesta.
 */
import { fireEvent, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CapaRecorte } from "./CapaRecorte";
import { aplicarIdioma } from "../../lib/i18n";
import type { Recorte } from "../../lib/recorte";

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

function pintar(props: Partial<React.ComponentProps<typeof CapaRecorte>> = {}) {
  const onRecorte = vi.fn();
  const { container } = render(
    <CapaRecorte activa recorte={null} onRecorte={onRecorte} {...props} />,
  );
  return { onRecorte, svg: container.querySelector("svg") };
}

beforeEach(() => {
  aplicarIdioma("es");
  conTamanno();
});

describe("colocar el marco", () => {
  it("guarda las esquinas de 0 a 1, no en píxeles", () => {
    const { onRecorte, svg } = pintar();
    // De la esquina de arriba a la izquierda al centro de la capa.
    fireEvent.pointerDown(svg!, { clientX: 100, clientY: 50 });
    fireEvent.pointerMove(svg!, { clientX: 300, clientY: 150 });
    fireEvent.pointerUp(svg!);

    expect(onRecorte).toHaveBeenCalledTimes(1);
    const marco: Recorte = onRecorte.mock.calls[0][0];
    expect(marco.x1).toBeCloseTo(0, 5);
    expect(marco.x2).toBeCloseTo(0.5, 5);
    expect(marco.y2).toBeCloseTo(0.5, 5);
  });

  it("arrastrar de derecha a izquierda deja las esquinas en su orden", () => {
    const { onRecorte, svg } = pintar();
    fireEvent.pointerDown(svg!, { clientX: 400, clientY: 200 });
    fireEvent.pointerMove(svg!, { clientX: 200, clientY: 100 });
    fireEvent.pointerUp(svg!);
    const marco: Recorte = onRecorte.mock.calls[0][0];
    expect(marco.x1).toBeLessThan(marco.x2);
    expect(marco.y1).toBeLessThan(marco.y2);
  });

  it("un clic sin arrastre quita el recorte en vez de dejar uno de un píxel", () => {
    // Sin esto, pulsar sin querer con la herramienta puesta dejaba la exportación
    // reducida a un punto, y encima con el marco tan pequeño que no se veía para quitarlo.
    const { onRecorte, svg } = pintar({ recorte: { x1: 0.1, y1: 0.1, x2: 0.9, y2: 0.9 } });
    fireEvent.pointerDown(svg!, { clientX: 200, clientY: 100 });
    fireEvent.pointerUp(svg!);
    expect(onRecorte).toHaveBeenCalledWith(null);
  });

  it("soltar fuera de la capa cierra el marco en el borde, no lo deja a medias", () => {
    const { onRecorte, svg } = pintar();
    fireEvent.pointerDown(svg!, { clientX: 200, clientY: 100 });
    fireEvent.pointerMove(svg!, { clientX: 9999, clientY: 9999 });
    fireEvent.pointerLeave(svg!);
    const marco: Recorte = onRecorte.mock.calls[0][0];
    expect(marco.x2).toBe(1);
    expect(marco.y2).toBe(1);
  });
});

describe("cuándo estorba y cuándo no", () => {
  it("apagada y sin marco puesto, no pinta nada", () => {
    // Una capa invisible que se come los clics parece que el editor se ha colgado.
    const { svg } = pintar({ activa: false });
    expect(svg).toBeNull();
  });

  it("apagada pero con un marco ya puesto, se sigue viendo y deja pasar los clics", () => {
    // El marco tiene que verse mientras se anota o se ajusta el panel de exportar.
    const { svg } = pintar({ activa: false, recorte: { x1: 0.2, y1: 0.2, x2: 0.8, y2: 0.8 } });
    expect(svg).not.toBeNull();
    expect(svg!.getAttribute("class")).toContain("pointer-events-none");
  });

  it("apagada no acepta arrastres nuevos", () => {
    const { onRecorte, svg } = pintar({
      activa: false,
      recorte: { x1: 0.2, y1: 0.2, x2: 0.8, y2: 0.8 },
    });
    fireEvent.pointerDown(svg!, { clientX: 120, clientY: 60 });
    fireEvent.pointerMove(svg!, { clientX: 400, clientY: 200 });
    fireEvent.pointerUp(svg!);
    expect(onRecorte).not.toHaveBeenCalled();
  });
});

describe("lo que se ve", () => {
  it("lo de fuera del marco sale oscurecido", () => {
    const { svg } = pintar({ recorte: { x1: 0.25, y1: 0.25, x2: 0.75, y2: 0.75 } });
    expect(svg!.querySelector("mask")).not.toBeNull();
    expect(svg!.innerHTML).toContain("rgba(0,0,0,0.55)");
  });

  it("y el marco se coloca donde dicen sus coordenadas", () => {
    const { svg } = pintar({ recorte: { x1: 0.25, y1: 0.1, x2: 0.75, y2: 0.9 } });
    // El SVG trabaja en un lienzo de 1000 x 1000, así que 0,25 son 250.
    const borde = [...svg!.querySelectorAll("rect")].find((r) => r.getAttribute("stroke"));
    expect(borde?.getAttribute("x")).toBe("250");
    expect(borde?.getAttribute("width")).toBe("500");
  });

  it("en inglés la capa se anuncia en inglés", () => {
    aplicarIdioma("en");
    const { svg } = pintar();
    expect(svg!.getAttribute("aria-label")).toBe("What is going to be exported");
  });
});
