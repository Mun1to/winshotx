import { describe, expect, it } from "vitest";
import { colocarTarjeta, type SitioTarjeta } from "./GuidedTour";

/**
 * Dónde cae la tarjeta del tour, con las medidas de las fotos del banco (ventana de 900 y cada
 * apartado a su alto). Las fotos lo enseñaron: con la ventana a la medida, un grupo alto no
 * deja sitio ni debajo ni encima, y la tarjeta acababa tapando lo que explicaba.
 */
const tapa = (s: SitioTarjeta, alto: number, m: { top: number; left: number; width: number; height: number }) => {
  const x = Math.min(s.left + s.width, m.left + m.width) - Math.max(s.left, m.left);
  const y = Math.min(s.top + alto, m.top + m.height) - Math.max(s.top, m.top);
  return x > 0 && y > 0 ? x * y : 0;
};

describe("dónde va la tarjeta del tour", () => {
  it("si cabe debajo, va debajo y centrada con lo que explica", () => {
    const marco = { top: 6, left: 276, width: 418, height: 44 };
    const s = colocarTarjeta(marco, null, 170, 900, 586);
    expect(s.top).toBe(6 + 44 + 14);
    expect(s.width).toBe(330);
  });

  it("un grupo alto con el dibujo al lado: a la derecha, sin tapar el grupo ni salirse", () => {
    // «Al soltar el ratón» en Después de capturar: 547 de alto, el grupo casi entero.
    const marco = { top: 148, left: 80, width: 488, height: 374 };
    const pestana = { top: 122, left: 10, width: 44, height: 44 };
    const s = colocarTarjeta(marco, pestana, 170, 900, 547);
    expect(tapa(s, 170, marco)).toBe(0);
    expect(tapa(s, 170, pestana)).toBe(0);
    expect(s.left + s.width).toBeLessThanOrEqual(900);
    expect(s.width).toBeGreaterThanOrEqual(280);
  });

  it("si no cabe en ningún sitio, la que menos tapa y a lo ancho, y así acaba cabiendo encima", () => {
    // La del café en inglés, medida en el banco: «Acerca de» ocupa todo el ancho de Ayuda
    // (no hay dibujo). Estrechada al lado medía 300 de alto y no cabía en ningún sitio, y se
    // quedaba ahí, encima de las filas; a lo ancho mide 245 y cabe encima del grupo.
    const marco = { top: 295, left: 80, width: 804, height: 237 };
    const primera = colocarTarjeta(marco, null, 300, 900, 556);
    expect(primera).toEqual({ top: 8, left: expect.any(Number), width: 330 });
    const luego = colocarTarjeta(marco, null, 245, 900, 556);
    expect(luego.top).toBe(295 - 245 - 14);
    expect(tapa(luego, 245, marco)).toBe(0);
  });
});
