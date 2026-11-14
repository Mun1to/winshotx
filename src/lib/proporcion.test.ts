/**
 * Las cuentas de la proporción fija, con números.
 *
 * Lo que importa de verdad es lo que se lleva el usuario: que el recorte, pasado a píxeles
 * físicos como hace `aVirtual`, sea un múltiplo exacto de la proporción y no se salga de
 * la pantalla. Por eso casi todas las comprobaciones pasan por `fisico`, que hace la misma
 * vuelta que el overlay antes de mandarlo a Rust.
 */
import { describe, expect, it } from "vitest";
import {
  PROPORCIONES,
  asaConProporcion,
  cambiarProporcion,
  crecerConProporcion,
  recorteConProporcion,
  siguienteProporcion,
  unidadDe,
  type Proporcion,
} from "./proporcion";
import { aVirtual } from "./pantallas";
import type { Rect } from "./types";

const ORIGEN = { x: 0, y: 0, width: 0, height: 0 };

/** Lo que recibe Rust: el rectángulo en físicos, redondeado como lo redondea el overlay. */
function fisico(rect: Rect, escala: number): Rect {
  return aVirtual(rect, ORIGEN, escala);
}

/** Si un rectángulo en físicos tiene esa proporción exacta, sin un píxel de error. */
function esExacta(r: Rect, proporcion: Proporcion): boolean {
  const [a, b] = proporcion.split(":").map(Number);
  return r.width * b === r.height * a && r.width % a === 0;
}

describe("el orden de la X", () => {
  it("libre, 16:9, 1:1, 9:16, 4:3 y vuelta a libre", () => {
    let p: Proporcion = "libre";
    const vistas: Proporcion[] = [];
    for (let i = 0; i < 5; i++) {
      p = siguienteProporcion(p);
      vistas.push(p);
    }
    expect(vistas).toEqual(["16:9", "1:1", "9:16", "4:3", "libre"]);
  });

  it("con Mayús, hacia atrás", () => {
    expect(siguienteProporcion("libre", true)).toBe("4:3");
    expect(siguienteProporcion("16:9", true)).toBe("libre");
  });

  it("grabando, el escalón es el doble, para que los dos lados salgan pares", () => {
    expect(unidadDe("16:9", false)).toEqual({ ancho: 16, alto: 9 });
    expect(unidadDe("16:9", true)).toEqual({ ancho: 32, alto: 18 });
    expect(unidadDe("libre", true)).toBeNull();
  });
});

describe("dibujar con la proporción puesta", () => {
  const limite = { ancho: 1920, alto: 1080 };

  it("cubre el cursor: manda el lado que más ha avanzado", () => {
    const r = recorteConProporcion({ x: 100, y: 100 }, { x: 420, y: 140 }, { ancho: 16, alto: 9 }, limite, 1);
    // 320 de ancho manda sobre 40 de alto: 20 escalones, 320 x 180.
    expect(r).toEqual({ x: 100, y: 100, width: 320, height: 180 });
  });

  it("hacia arriba y a la izquierda, el origen se queda en su esquina", () => {
    const r = recorteConProporcion({ x: 500, y: 500 }, { x: 400, y: 100 }, { ancho: 1, alto: 1 }, limite, 1);
    expect(r).toEqual({ x: 100, y: 100, width: 400, height: 400 });
  });

  it("no se sale de la pantalla: se queda en el más grande que cabe", () => {
    const r = recorteConProporcion({ x: 1800, y: 900 }, { x: 1900, y: 1070 }, { ancho: 9, alto: 16 }, limite, 1);
    const f = fisico(r, 1);
    expect(f.x + f.width).toBeLessThanOrEqual(1920);
    expect(f.y + f.height).toBeLessThanOrEqual(1080);
    expect(esExacta(f, "9:16")).toBe(true);
  });

  it.each([1, 1.25, 1.5, 1.75])("a %s, el recorte en físicos es exacto y cae en píxel entero", (escala) => {
    const pantalla = { ancho: Math.round(1536 * escala), alto: Math.round(864 * escala) };
    for (const proporcion of PROPORCIONES.filter((p) => p !== "libre")) {
      for (const grabando of [false, true]) {
        const unidad = unidadDe(proporcion, grabando)!;
        // Puntos con decimales a propósito: son los que dejaban un 16:9 en 126 x 71.
        const r = recorteConProporcion({ x: 137.3, y: 211.7 }, { x: 611.9, y: 433.1 }, unidad, pantalla, escala);
        const f = fisico(r, escala);
        expect({ proporcion, grabando, exacta: esExacta(f, proporcion) }).toEqual({
          proporcion,
          grabando,
          exacta: true,
        });
        if (grabando) expect([f.width % 2, f.height % 2]).toEqual([0, 0]);
        expect(f.x + f.width).toBeLessThanOrEqual(pantalla.ancho);
        expect(f.y + f.height).toBeLessThanOrEqual(pantalla.alto);
      }
    }
  });
});

describe("las asas con la proporción puesta", () => {
  const limite = { ancho: 1920, alto: 1080 };
  const unidad = { ancho: 16, alto: 9 };
  const base = { x: 400, y: 300, width: 320, height: 180 };

  it("la esquina de enfrente se queda quieta", () => {
    const r = asaConProporcion(base, "se", { x: 1040, y: 400 }, unidad, limite, 1);
    expect(r.x).toBe(400);
    expect(r.y).toBe(300);
    expect(esExacta(fisico(r, 1), "16:9")).toBe(true);
    expect(r.width).toBe(640);
  });

  it("un lado mueve ese lado, y el otro eje crece por igual arriba y abajo", () => {
    const r = asaConProporcion(base, "e", { x: 1040, y: 999 }, unidad, limite, 1);
    expect(r).toEqual({ x: 400, y: 210, width: 640, height: 360 });
  });

  it("y si así se sale por arriba, se arrima al borde", () => {
    const pegado = { x: 400, y: 10, width: 320, height: 180 };
    const r = asaConProporcion(pegado, "e", { x: 1040, y: 0 }, unidad, limite, 1);
    expect(r.y).toBe(0);
    expect(r.height).toBe(360);
  });

  it("a 125 %, también exacto", () => {
    const r = asaConProporcion(base, "w", { x: 101.3, y: 0 }, unidad, { ancho: 2400, alto: 1350 }, 1.25);
    expect(esExacta(fisico(r, 1.25), "16:9")).toBe(true);
  });
});

describe("cambiar la proporción de un recorte que ya está hecho", () => {
  const limite = { ancho: 1920, alto: 1080 };

  it("se queda en su sitio y de tamaño parecido", () => {
    const antes = { x: 200, y: 300, width: 300, height: 200 };
    const r = cambiarProporcion(antes, { ancho: 16, alto: 9 }, limite, 1);
    expect(r).toEqual({ x: 190, y: 310, width: 320, height: 180 });
  });

  it("y dar la vuelta entera con la X no lo va encogiendo", () => {
    let r: Rect = { x: 600, y: 300, width: 480, height: 270 };
    for (let vuelta = 0; vuelta < 3; vuelta++) {
      for (const p of ["1:1", "9:16", "4:3", "16:9"] as const) {
        r = cambiarProporcion(r, unidadDe(p, false)!, limite, 1);
      }
    }
    expect(r.width).toBeGreaterThanOrEqual(464);
    expect(r.width).toBeLessThanOrEqual(496);
  });

  it("si no cabe, el más grande que cabe, dentro de la pantalla", () => {
    const r = cambiarProporcion({ x: 0, y: 0, width: 1920, height: 1080 }, { ancho: 9, alto: 16 }, limite, 1);
    expect(r.height).toBeLessThanOrEqual(1080);
    expect(r.y).toBeGreaterThanOrEqual(0);
    expect(esExacta(fisico(r, 1), "9:16")).toBe(true);
  });
});

describe("Alt y las flechas", () => {
  it("crecen y encogen las dos medidas juntas, de escalón en escalón", () => {
    const limite = { ancho: 1920, alto: 1080 };
    const r = { x: 100, y: 100, width: 320, height: 180 };
    expect(crecerConProporcion(r, 1, { ancho: 16, alto: 9 }, limite, 1)).toEqual({ x: 100, y: 100, width: 336, height: 189 });
    expect(crecerConProporcion(r, -10, { ancho: 16, alto: 9 }, limite, 1)).toEqual({ x: 100, y: 100, width: 160, height: 90 });
  });
});
