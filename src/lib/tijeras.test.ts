/**
 * Las tijeras de la línea de tiempo (D1) y acelerar trozos (D3): lo que se manda a Rust es
 * exactamente lo que se queda, y sin cortar nada se manda `null`, que es exportar como antes.
 */
import { describe, expect, it } from "vitest";
import {
  acelerarEscritura,
  cambiarPieza,
  duracionQueSale,
  partir,
  piezaEn,
  piezasDe,
  sePuedePartir,
  TRAMO_MINIMO_MS,
  tramosDeEnvio,
} from "./tramo";

describe("cortar con las tijeras", () => {
  it("una pieza se parte en dos donde está el cabezal", () => {
    const piezas = partir(piezasDe(10_000), 4_000);
    expect(piezas.map((p) => [p.desdeMs, p.hastaMs])).toEqual([
      [0, 4_000],
      [4_000, 10_000],
    ]);
    expect(piezaEn(piezas, 3_999)).toBe(0);
    expect(piezaEn(piezas, 4_000)).toBe(1);
    expect(piezaEn(piezas, 10_000)).toBe(1);
  });

  it("pegado a un corte no corta: un trozo sin largo no se ve ni se puede quitar", () => {
    const piezas = partir(piezasDe(10_000), 4_000);
    expect(sePuedePartir(piezas, 4_000 + TRAMO_MINIMO_MS - 1)).toBe(false);
    expect(sePuedePartir(piezas, 50)).toBe(false);
    expect(sePuedePartir(piezas, 6_000)).toBe(true);
  });
});

describe("lo que se manda a Rust", () => {
  it("sin cortar ni acelerar nada, null: el vídeo sale como siempre", () => {
    expect(tramosDeEnvio(piezasDe(10_000), 0, 10_000)).toBeNull();
    // Partido pero sin tocar, tampoco: dos trozos seguidos a la misma velocidad son uno.
    expect(tramosDeEnvio(partir(piezasDe(10_000), 5_000), 2_000, 9_000)).toBeNull();
  });

  it("un trozo quitado del medio deja dos, dentro de las asas", () => {
    const piezas = cambiarPieza(partir(partir(piezasDe(10_000), 3_000), 6_000), 1, { cortada: true });
    expect(tramosDeEnvio(piezas, 1_000, 9_000)).toEqual([
      { desdeMs: 1_000, hastaMs: 3_000, velocidad: 1 },
      { desdeMs: 6_000, hastaMs: 9_000, velocidad: 1 },
    ]);
    expect(duracionQueSale(piezas, 1_000, 9_000)).toBe(5_000);
  });

  it("un trozo al doble dura la mitad", () => {
    const piezas = cambiarPieza(partir(piezasDe(10_000), 4_000), 1, { velocidad: 2 });
    expect(tramosDeEnvio(piezas, 0, 10_000)).toEqual([
      { desdeMs: 0, hastaMs: 4_000, velocidad: 1 },
      { desdeMs: 4_000, hastaMs: 10_000, velocidad: 2 },
    ]);
    expect(duracionQueSale(piezas, 0, 10_000)).toBe(7_000);
  });
});

describe("acelerar lo escrito", () => {
  it("corta en cada rato de escritura y solo esos van rápidos", () => {
    const piezas = acelerarEscritura(
      piezasDe(10_000),
      [
        { desdeMs: 2_000, hastaMs: 3_500 },
        { desdeMs: 7_000, hastaMs: 8_000 },
        // Un rato de cien milisegundos no merece un trozo.
        { desdeMs: 9_000, hastaMs: 9_100 },
      ],
      4,
    );
    expect(piezas.map((p) => [p.desdeMs, p.hastaMs, p.velocidad])).toEqual([
      [0, 2_000, 1],
      [2_000, 3_500, 4],
      [3_500, 7_000, 1],
      [7_000, 8_000, 4],
      [8_000, 10_000, 1],
    ]);
  });

  it("y lo que estaba quitado sigue quitado", () => {
    const quitado = cambiarPieza(partir(partir(piezasDe(10_000), 2_000), 4_000), 1, { cortada: true });
    const piezas = acelerarEscritura(quitado, [{ desdeMs: 2_000, hastaMs: 4_000 }], 4);
    expect(piezas[1]).toMatchObject({ cortada: true, velocidad: 1 });
  });
});
