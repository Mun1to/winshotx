/**
 * Las cuentas del recorte.
 *
 * Son cuatro funciones puras y las cuatro deciden algo que despues no se puede corregir:
 * que trozo se exporta y de que tamanno. Se prueban aqui, sin ventana, porque el fallo
 * tipico no es visual, es de un pixel de mas o de una esquina invertida.
 */
import { describe, expect, it } from "vitest";
import { medida, ordenar, recortaAlgo, type Recorte } from "./recorte";

const MITAD_DERECHA: Recorte = { x1: 0.5, y1: 0, x2: 1, y2: 1 };

describe("ordenar las dos esquinas", () => {
  it("arrastrar de derecha a izquierda da el mismo marco", () => {
    // Arrastrar hacia atras es tan normal como hacia delante, y nadie va a repetirlo.
    const alReves: Recorte = { x1: 0.8, y1: 0.9, x2: 0.2, y2: 0.1 };
    expect(ordenar(alReves)).toEqual({ x1: 0.2, y1: 0.1, x2: 0.8, y2: 0.9 });
  });

  it("lo que se sale por los bordes se queda dentro", () => {
    expect(ordenar({ x1: -0.4, y1: -1, x2: 1.7, y2: 2 })).toEqual({
      x1: 0,
      y1: 0,
      x2: 1,
      y2: 1,
    });
  });
});

describe("cuanto mide el trozo", () => {
  it("la mitad de 800 son 400", () => {
    expect(medida(MITAD_DERECHA, 800, 600)).toEqual({ width: 400, height: 600 });
  });

  it("un arrastre de nada no deja un lado de cero", () => {
    // Un lado de cero revienta al codificador de video mucho despues, y para entonces ya
    // no se sabe de donde venia.
    const casi = { x1: 0.5, y1: 0.5, x2: 0.5, y2: 0.5 };
    expect(medida(casi, 800, 600)).toEqual({ width: 2, height: 2 });
  });

  it("se mide sobre la captura, no sobre la vista previa", () => {
    // El mismo marco sobre una captura mas grande da un trozo mas grande. Es toda la
    // razon de que las coordenadas vayan de 0 a 1.
    expect(medida(MITAD_DERECHA, 1920, 1200)).toEqual({ width: 960, height: 1200 });
  });
});

describe("si el marco recorta algo de verdad", () => {
  it("un clic sin arrastre, no", () => {
    expect(recortaAlgo({ x1: 0.3, y1: 0.3, x2: 0.302, y2: 0.31 })).toBe(false);
  });

  it("el marco que abarca la captura entera, tampoco", () => {
    expect(recortaAlgo({ x1: 0, y1: 0, x2: 1, y2: 1 })).toBe(false);
  });

  it("pero quitar solo una franja, si", () => {
    // Recortar la barra de tareas de abajo es un caso de verdad, y el alto cambia poco.
    expect(recortaAlgo({ x1: 0, y1: 0, x2: 1, y2: 0.94 })).toBe(true);
  });

  it("y la mitad, tambien", () => {
    expect(recortaAlgo(MITAD_DERECHA)).toBe(true);
  });
});
