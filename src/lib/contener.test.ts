/**
 * Dónde cae la imagen dentro de su hueco.
 *
 * Es una cuenta de dos líneas y decide algo que no se ve hasta que alguien abre el archivo
 * exportado: si las capas de dibujar caen encima de la captura o un poco más allá.
 */
import { describe, expect, it } from "vitest";
import { contener } from "./contener";

describe("contener una imagen en su hueco", () => {
  it("una captura más ancha que el hueco se ajusta al ancho", () => {
    // 1600 x 900 en un hueco de 800 x 600: manda el ancho, y sobran franjas arriba y abajo.
    expect(contener(800, 600, 1600, 900)).toEqual({ width: 800, height: 450 });
  });

  it("y una más alta se ajusta al alto", () => {
    // Este es el caso que rompía el editor: una captura vertical en una ventana ancha.
    expect(contener(1000, 500, 600, 1000)).toEqual({ width: 300, height: 500 });
  });

  it("la que ya encaja se queda como está", () => {
    expect(contener(800, 600, 400, 300)).toEqual({ width: 800, height: 600 });
  });

  it("nunca se sale del hueco", () => {
    const caja = contener(300, 200, 4000, 30);
    expect(caja.width).toBeLessThanOrEqual(300);
    expect(caja.height).toBeLessThanOrEqual(200);
  });

  it("y nunca deforma la captura", () => {
    const caja = contener(1000, 700, 1920, 1080);
    expect(caja.width / caja.height).toBeCloseTo(1920 / 1080, 2);
  });

  it("un hueco todavía sin medir no da una caja de un punto", () => {
    // La primera pasada del navegador da ceros. Devolver 1 x 1 dejaría las capas en un
    // punto, y cualquier arrastre dividiría por casi cero.
    expect(contener(0, 0, 1920, 1080)).toEqual({ width: 0, height: 0 });
  });

  it("una captura de lado cero tampoco revienta la cuenta", () => {
    expect(contener(800, 600, 0, 100)).toEqual({ width: 0, height: 0 });
  });
});
