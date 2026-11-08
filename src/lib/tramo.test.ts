import { describe, expect, it } from "vitest";
import {
  TRAMO_MINIMO_MS,
  duracionDe,
  formatoTiempo,
  fotogramaVecino,
  indiceEn,
  limitarTramo,
  muestrasDeMiniaturas,
} from "./tramo";
import type { FrameMeta } from "./types";

/** Fotogramas seguidos con estas duraciones, como los da Rust. */
function fotogramas(duraciones: number[]): FrameMeta[] {
  let ms = 0;
  return duraciones.map((durationMs, index) => {
    const f = { index, timestampMs: ms, durationMs, thumbPath: `t${index}.png` };
    ms += durationMs;
    return f;
  });
}

describe("el fotograma que se ve en cada instante", () => {
  it("es el último que empezó antes, no el más cercano", () => {
    const f = fotogramas([100, 100, 100]);
    expect(indiceEn(f, 0)).toBe(0);
    expect(indiceEn(f, 99)).toBe(0);
    expect(indiceEn(f, 100)).toBe(1);
    expect(indiceEn(f, 190)).toBe(1);
  });

  it("dentro de una pantalla quieta es la quietud, aunque el siguiente esté cerca", () => {
    const f = fotogramas([100, 10_000, 100]);
    expect(indiceEn(f, 9_900)).toBe(1);
  });

  it("fuera de la grabación se queda en sus bordes", () => {
    const f = fotogramas([100, 100]);
    expect(indiceEn(f, -50)).toBe(0);
    expect(indiceEn(f, 99_999)).toBe(1);
    expect(indiceEn([], 10)).toBe(0);
  });
});

describe("lo que dura la grabación", () => {
  it("llega hasta el final del último fotograma, quietud incluida", () => {
    expect(duracionDe(fotogramas([100, 10_000, 100]))).toBe(10_200);
    expect(duracionDe([])).toBe(0);
  });
});

describe("el tramo que se queda", () => {
  it("nunca se sale de la grabación", () => {
    expect(limitarTramo(-500, 99_000, 10_000)).toEqual({ desdeMs: 0, hastaMs: 10_000 });
  });

  it("nunca baja del mínimo: el asa que se mueve se para, la otra no se mueve", () => {
    expect(limitarTramo(9_950, 10_000, 30_000, "desde")).toEqual({
      desdeMs: 10_000 - TRAMO_MINIMO_MS,
      hastaMs: 10_000,
    });
    expect(limitarTramo(5_000, 5_010, 30_000, "hasta")).toEqual({
      desdeMs: 5_000,
      hastaMs: 5_000 + TRAMO_MINIMO_MS,
    });
  });

  it("al final de la grabación, el mínimo se hace hacia atrás", () => {
    expect(limitarTramo(10_000, 10_000, 10_000)).toEqual({
      desdeMs: 10_000 - TRAMO_MINIMO_MS,
      hastaMs: 10_000,
    });
  });

  it("una grabación más corta que el mínimo se queda entera", () => {
    expect(limitarTramo(0, 50, 120)).toEqual({ desdeMs: 0, hastaMs: 120 });
  });
});

describe("las miniaturas de la tira", () => {
  it("se reparten por tiempo: una quietud larga ocupa lo que dura", () => {
    // Un segundo con diez fotogramas y nueve segundos quietos: de diez huecos, nueve son
    // la quietud. Repartidos por fotogramas, la quietud ocuparía uno de once.
    const f = fotogramas([...Array(10).fill(100), 9_000]);
    const muestras = muestrasDeMiniaturas(f, 10);
    expect(muestras.filter((i) => i === 10)).toHaveLength(9);
  });

  it("son tantas como huecos, aunque la grabación tenga dieciocho mil fotogramas", () => {
    const f = fotogramas(Array(18_000).fill(33));
    expect(muestrasDeMiniaturas(f, 14)).toHaveLength(14);
  });
});

describe("moverse de fotograma en fotograma", () => {
  const f = fotogramas([100, 10_000, 100]);

  it("hacia delante va al principio del siguiente", () => {
    expect(fotogramaVecino(f, 0, 1)).toBe(100);
    expect(fotogramaVecino(f, 5_000, 1)).toBe(10_100);
  });

  it("hacia atrás, a mitad de una quietud, va primero a su principio", () => {
    expect(fotogramaVecino(f, 5_000, -1)).toBe(100);
    expect(fotogramaVecino(f, 100, -1)).toBe(0);
  });

  it("y en los bordes no se sale", () => {
    expect(fotogramaVecino(f, 0, -1)).toBe(0);
    expect(fotogramaVecino(f, 10_100, 1)).toBe(10_100);
  });
});

describe("cómo se escribe un tiempo", () => {
  it("minutos, segundos y décimas", () => {
    expect(formatoTiempo(0)).toBe("0:00.0");
    expect(formatoTiempo(3_249)).toBe("0:03.2");
    expect(formatoTiempo(61_500)).toBe("1:01.5");
    expect(formatoTiempo(600_000)).toBe("10:00.0");
  });
});
