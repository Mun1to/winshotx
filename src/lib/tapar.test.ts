import { describe, expect, it } from "vitest";
import type { Tapa } from "./contrato";
import { anadirTapas, carriles, estirarTapa, ladoDelMosaico, puesta, puestasEn, quitarTapa, TAPA_MINIMA_MS } from "./tapar";

const tapa = (desdeMs: number, hastaMs: number, x = 0.1, y = 0.1, ancho = 0.2, alto = 0.05): Tapa => ({
  desdeMs,
  hastaMs,
  x,
  y,
  ancho,
  alto,
});

describe("cuándo está puesta", () => {
  it("desde su principio y sin su final, como en Rust", () => {
    const t = tapa(1000, 2000);
    expect([999, 1000, 1999, 2000].map((ms) => puesta(t, ms))).toEqual([false, true, true, false]);
  });

  it("una sin tamaño no tapa nada", () => {
    expect(puesta(tapa(0, 1000, 0.1, 0.1, 0, 0.1), 500)).toBe(false);
  });

  it("dice cuáles hay en un instante, con su número", () => {
    const lista = [tapa(0, 1000), tapa(500, 1500, 0.5), tapa(2000, 3000)];
    expect(puestasEn(lista, 700).map((p) => p.indice)).toEqual([0, 1]);
  });
});

describe("pulsar «Tapar datos» otra vez", () => {
  it("no repite lo que ya estaba, aunque el usuario lo hubiera alargado", () => {
    const alargada = tapa(500, 4000);
    const salen = anadirTapas([alargada], [tapa(1000, 2000), tapa(1000, 2000, 0.6, 0.6)]);
    expect(salen).toHaveLength(2);
    expect(salen[0]).toEqual(alargada);
  });

  it("y lo nuevo va en orden de tiempo", () => {
    const salen = anadirTapas([tapa(3000, 4000)], [tapa(0, 500)]);
    expect(salen.map((t) => t.desdeMs)).toEqual([0, 3000]);
  });

  it("un dato en el mismo sitio pero en otro rato sí es otro bloque", () => {
    expect(anadirTapas([tapa(0, 1000)], [tapa(5000, 6000)])).toHaveLength(2);
  });
});

describe("alargar y quitar", () => {
  it("se alarga por cada borde, dentro de la grabación", () => {
    const lista = [tapa(1000, 2000)];
    expect(estirarTapa(lista, 0, "desde", -300, 5000)[0].desdeMs).toBe(0);
    expect(estirarTapa(lista, 0, "hasta", 9000, 5000)[0].hastaMs).toBe(5000);
    expect(estirarTapa(lista, 0, "hasta", 3200.4, 5000)[0].hastaMs).toBe(3200);
  });

  it("nunca se queda más corto que el mínimo ni del revés", () => {
    const lista = [tapa(1000, 2000)];
    expect(estirarTapa(lista, 0, "desde", 2500, 5000)[0].desdeMs).toBe(2000 - TAPA_MINIMA_MS);
    expect(estirarTapa(lista, 0, "hasta", 0, 5000)[0].hastaMs).toBe(1000 + TAPA_MINIMA_MS);
  });

  it("no toca el rectángulo: el dato sigue estando donde estaba", () => {
    const [t] = estirarTapa([tapa(1000, 2000, 0.3, 0.4)], 0, "hasta", 3000, 5000);
    expect([t.x, t.y, t.ancho, t.alto]).toEqual([0.3, 0.4, 0.2, 0.05]);
  });

  it("quitar quita solo ese", () => {
    expect(quitarTapa([tapa(0, 1), tapa(2, 3), tapa(4, 5)], 1).map((t) => t.desdeMs)).toEqual([0, 4]);
  });
});

describe("las filas de la pista", () => {
  it("dos que se ven a la vez van en filas distintas, y lo que viene después reusa la primera", () => {
    const { carril, cuantos } = carriles([tapa(0, 1000), tapa(500, 1500), tapa(1200, 2000)]);
    expect(carril).toEqual([0, 1, 0]);
    expect(cuantos).toBe(2);
  });

  it("no pasan de tres: lo que no cabe va en la última", () => {
    const { carril, cuantos } = carriles([tapa(0, 9), tapa(0, 9), tapa(0, 9), tapa(0, 9), tapa(0, 9)]);
    expect(cuantos).toBe(3);
    expect(carril).toEqual([0, 1, 2, 2, 2]);
  });

  it("sin ninguna, una fila", () => {
    expect(carriles([]).cuantos).toBe(1);
  });
});

it("el mosaico tiene el tamaño de cuadrado de Rust", () => {
  // Una línea de texto: dos filas de cuadros, no cuatro (con 6 se leían los números).
  expect(ladoDelMosaico(200, 30)).toBe(15);
  expect(ladoDelMosaico(400, 160)).toBe(20);
  expect(ladoDelMosaico(2000, 1000)).toBe(40);
});
