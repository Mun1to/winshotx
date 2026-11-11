/**
 * Las cuentas de la pista de zooms: lo que se ve en la pista es lo que va a salir, así
 * que dos bloques nunca se pisan, ninguno se sale de la grabación y ninguno se queda sin
 * largo, por mucho que se arrastre.
 */
import { describe, expect, it } from "vitest";
import type { TramoZoom } from "./contrato";
import {
  anadir,
  bloqueEn,
  bloqueNuevo,
  estirar,
  medidasDeLupa,
  mover,
  ventanaDelZoom,
  ZOOM_MINIMO_MS,
  ZOOM_NUEVO_MS,
} from "./zooms";

const centro = { x: 0.5, y: 0.5 };
const bloque = (desdeMs: number, hastaMs: number): TramoZoom => ({
  desdeMs,
  hastaMs,
  nivel: 2,
  modo: "sigue",
  x: 0.5,
  y: 0.5,
  instantaneo: false,
  lupa: null,
});

describe("poner un bloque con un clic", () => {
  it("sale centrado donde se pulsa, de dos segundos, mirando donde estaba el ratón", () => {
    const nuevo = bloqueNuevo([], 5_000, 10_000, { x: 0.2, y: 0.7 })!;
    expect(nuevo).toMatchObject({ desdeMs: 4_000, hastaMs: 6_000, x: 0.2, y: 0.7, modo: "sigue" });
    expect(nuevo.hastaMs - nuevo.desdeMs).toBe(ZOOM_NUEVO_MS);
  });

  it("pegado al principio o al final, se mete dentro", () => {
    expect(bloqueNuevo([], 100, 10_000, centro)).toMatchObject({ desdeMs: 0, hastaMs: 2_000 });
    expect(bloqueNuevo([], 9_900, 10_000, centro)).toMatchObject({ desdeMs: 8_000, hastaMs: 10_000 });
  });

  it("en un hueco pequeño entre dos, coge lo que cabe; encima de otro, nada", () => {
    const zooms = [bloque(0, 3_000), bloque(4_000, 8_000)];
    expect(bloqueNuevo(zooms, 3_500, 10_000, centro)).toMatchObject({ desdeMs: 3_000, hastaMs: 4_000 });
    expect(bloqueNuevo(zooms, 1_000, 10_000, centro)).toBeNull();
    expect(bloqueNuevo([bloque(0, 3_000), bloque(3_100, 8_000)], 3_050, 10_000, centro)).toBeNull();
  });

  it("y queda en su sitio de la lista, por orden", () => {
    const { zooms, indice } = anadir([bloque(0, 1_000), bloque(5_000, 6_000)], bloque(2_000, 3_000));
    expect(indice).toBe(1);
    expect(zooms.map((z) => z.desdeMs)).toEqual([0, 2_000, 5_000]);
    expect(bloqueEn(zooms, 2_500)).toBe(1);
    expect(bloqueEn(zooms, 4_000)).toBe(-1);
  });
});

describe("mover y estirar", () => {
  const zooms = [bloque(1_000, 3_000), bloque(5_000, 7_000)];

  it("moverlo conserva lo que dura y se para contra el de al lado", () => {
    expect(mover(zooms, 0, 1_500, 10_000)[0]).toMatchObject({ desdeMs: 1_500, hastaMs: 3_500 });
    expect(mover(zooms, 0, 4_500, 10_000)[0]).toMatchObject({ desdeMs: 3_000, hastaMs: 5_000 });
    expect(mover(zooms, 1, 99_000, 10_000)[1]).toMatchObject({ desdeMs: 8_000, hastaMs: 10_000 });
    expect(mover(zooms, 0, -5_000, 10_000)[0]).toMatchObject({ desdeMs: 0, hastaMs: 2_000 });
  });

  it("estirarlo no pisa al vecino ni lo deja sin largo", () => {
    expect(estirar(zooms, 0, "hasta", 6_000, 10_000)[0].hastaMs).toBe(5_000);
    expect(estirar(zooms, 0, "hasta", 0, 10_000)[0].hastaMs).toBe(1_000 + ZOOM_MINIMO_MS);
    expect(estirar(zooms, 1, "desde", 2_000, 10_000)[1].desdeMs).toBe(3_000);
    expect(estirar(zooms, 1, "desde", 9_000, 10_000)[1].desdeMs).toBe(7_000 - ZOOM_MINIMO_MS);
  });
});

describe("lo que se dibuja al colocar un zoom fijo", () => {
  it("la ventana del zoom es la de Rust: centrada en el punto y dentro de la imagen", () => {
    expect(ventanaDelZoom(0.5, 0.5, 2)).toEqual({ x1: 0.25, y1: 0.25, x2: 0.75, y2: 0.75 });
    expect(ventanaDelZoom(0.05, 0.95, 2)).toEqual({ x1: 0, y1: 0.5, x2: 0.5, y2: 1 });
  });

  it("y la lupa mide lo mismo que en Rust: los rectángulos, más bajos que anchos", () => {
    const lupa = { forma: "circulo" as const, marco: true, tam: 0.25, aumento: 2 };
    expect(medidasDeLupa(lupa, 1920, 1080)).toEqual({ ancho: 480, alto: 480 });
    expect(medidasDeLupa({ ...lupa, forma: "rectangulo" }, 1920, 1080)).toEqual({ ancho: 480, alto: 288 });
  });
});
