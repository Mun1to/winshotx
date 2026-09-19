/**
 * Las cuentas de la vista previa del estudio.
 *
 * Tienen que decir lo mismo que Rust, porque lo que se ve aquí es la promesa de lo que
 * va a salir al exportar: un aro que en la vista previa mide el doble, o un puntero que
 * se queda un fotograma por detrás, es una vista previa que miente.
 */
import { describe, expect, it } from "vitest";
import {
  altoDelPuntero,
  arosEn,
  atajoEn,
  colocar,
  cursorEn,
  dibujoDe,
  encuadreEn,
  esEntero,
  formaEn,
  punteroEn,
  opacidadDelAro,
  punteroNormal,
  radioDelAro,
  radioMaximo,
  transformacion,
  ARO_MS,
  PULSANDO_MS,
  TECLA_MS,
} from "./estudio";
import type { MuestraCamara } from "./types";

const ENTERO: MuestraCamara = { ms: 0, x1: 0, y1: 0, x2: 1, y2: 1 };

describe("la cámara entre dos muestras", () => {
  const muestras: MuestraCamara[] = [
    { ms: 0, x1: 0, y1: 0, x2: 1, y2: 1 },
    { ms: 100, x1: 0.2, y1: 0.2, x2: 0.7, y2: 0.7 },
  ];

  it("sin muestras no hay encuadre", () => {
    expect(encuadreEn([], 50)).toBeNull();
  });

  it("antes de la primera y después de la última se queda en ellas", () => {
    expect(encuadreEn(muestras, -5)).toEqual(muestras[0]);
    expect(encuadreEn(muestras, 500)).toEqual(muestras[1]);
  });

  it("a mitad de camino está a mitad de camino", () => {
    const r = encuadreEn(muestras, 50)!;
    expect(r.x1).toBeCloseTo(0.1);
    expect(r.y1).toBeCloseTo(0.1);
    expect(r.x2).toBeCloseTo(0.85);
    expect(r.y2).toBeCloseTo(0.85);
  });

  it("la imagen entera es «sin zoom»", () => {
    expect(esEntero(ENTERO)).toBe(true);
    expect(esEntero(null)).toBe(true);
    expect(esEntero({ x1: 0.25, y1: 0.25, x2: 0.75, y2: 0.75 })).toBe(false);
  });
});

describe("la transformación que enseña el encuadre", () => {
  it("sin zoom no transforma nada", () => {
    expect(transformacion(ENTERO, 800, 600)).toBe("none");
    expect(transformacion(null, 800, 600)).toBe("none");
  });

  it("el cuarto central al doble: escala 2 y se desplaza un cuarto de caja", () => {
    const css = transformacion({ x1: 0.25, y1: 0.25, x2: 0.75, y2: 0.75 }, 800, 600);
    // Escalar por 2 y llevar la esquina (200, 150) al origen: -400, -300.
    expect(css).toBe("translate(-400.00px, -300.00px) scale(2.0000, 2.0000)");
  });
});

describe("llevar un punto al cuadro que se ve", () => {
  it("sin encuadre el punto se queda donde estaba", () => {
    expect(colocar(0.3, 0.4, null)).toEqual({ u: 0.3, v: 0.4 });
  });

  it("dentro del cuarto central, el centro sigue en el centro", () => {
    const r = { x1: 0.25, y1: 0.25, x2: 0.75, y2: 0.75 };
    expect(colocar(0.5, 0.5, r)).toEqual({ u: 0.5, v: 0.5 });
  });

  it("y lo que se quedó fuera no se dibuja", () => {
    const r = { x1: 0.25, y1: 0.25, x2: 0.75, y2: 0.75 };
    expect(colocar(0.1, 0.5, r)).toBeNull();
  });
});

describe("el ratón y los aros", () => {
  const rastro: [number, number, number][] = [
    [0, 10, 10],
    [33, 20, 20],
    [66, 30, 30],
  ];

  it("el ratón es la última anotación que no es posterior", () => {
    expect(cursorEn(rastro, 40)).toEqual([20, 20]);
    expect(cursorEn(rastro, 66)).toEqual([30, 30]);
    expect(cursorEn(rastro, -1)).toBeNull();
  });

  it("un aro solo se ve mientras dura", () => {
    const clics = [{ ms: 1000, x: 5, y: 5, derecho: false }];
    expect(arosEn(clics, 999)).toHaveLength(0);
    expect(arosEn(clics, 1000)).toHaveLength(1);
    expect(arosEn(clics, 1000 + ARO_MS)).toHaveLength(0);
  });

  it("el aro crece con el vídeo, con tope, y se abre desde el punto", () => {
    expect(radioMaximo(1080)).toBeCloseTo(25.92);
    expect(radioMaximo(400)).toBe(14);
    expect(radioMaximo(4000)).toBe(44);
    expect(radioDelAro(0, 1080)).toBeLessThan(radioDelAro(1, 1080));
    expect(radioDelAro(1, 1080)).toBeCloseTo(radioMaximo(1080));
  });

  it("y se va apagando al final", () => {
    expect(opacidadDelAro(0)).toBeCloseTo(0.85);
    expect(opacidadDelAro(0.9)).toBeLessThan(opacidadDelAro(0.4));
    expect(opacidadDelAro(1)).toBeCloseTo(0);
  });
});

describe("la pastilla y el puntero", () => {
  it("la pastilla es el último atajo vivo, entera casi todo el rato", () => {
    const atajos = [
      { texto: "Ctrl + C", ms: 1000, x: 0, y: 0 },
      { texto: "Ctrl + V", ms: 1500, x: 0, y: 0 },
    ];
    expect(atajoEn(atajos, 1200)?.texto).toBe("Ctrl + C");
    expect(atajoEn(atajos, 1600)?.texto).toBe("Ctrl + V");
    expect(atajoEn(atajos, 1600)?.opacidad).toBe(1);
    expect(atajoEn(atajos, 1500 + TECLA_MS - 10)?.opacidad).toBeLessThan(0.2);
    expect(atajoEn(atajos, 1500 + TECLA_MS)).toBeNull();
  });

  it("el puntero se encoge mientras dura el clic", () => {
    const clics = [{ ms: 1000, x: 5, y: 5, derecho: false }];
    expect(altoDelPuntero(40, clics, 500)).toBe(40);
    expect(altoDelPuntero(40, clics, 1000)).toBeLessThan(40);
    expect(altoDelPuntero(40, clics, 1000 + PULSANDO_MS)).toBe(40);
  });

  it("la forma es la del último cambio, y sin nada anotado la flecha", () => {
    expect(formaEn(undefined, 100)).toBe("flecha");
    expect(formaEn([], 100)).toBe("flecha");
    const formas: [number, number][] = [
      [100, 1],
      [900, 2],
    ];
    expect(formaEn(formas, 50)).toBe("flecha");
    expect(formaEn(formas, 100)).toBe("texto");
    expect(formaEn(formas, 899)).toBe("texto");
    expect(formaEn(formas, 900)).toBe("mano");
  });

  it("el puntero leído es el del último cambio, y el ilegible no cuenta", () => {
    expect(punteroEn(undefined, 5)).toBeNull();
    expect(punteroEn([], 5)).toBeNull();
    const cambios: [number, number][] = [
      [100, 0],
      [900, 4294967295],
      [1500, 1],
    ];
    expect(punteroEn(cambios, 50)).toBeNull();
    expect(punteroEn(cambios, 100)).toBe(0);
    expect(punteroEn(cambios, 899)).toBe(0);
    expect(punteroEn(cambios, 900)).toBeNull();
    expect(punteroEn(cambios, 5000)).toBe(1);
  });

  it("cada forma tiene su dibujo y su punto caliente donde lo tiene Windows", () => {
    expect(dibujoDe("flecha").caliente).toEqual([0, 0]);
    expect(dibujoDe("texto").caliente).toEqual([0.5, 0.5]);
    expect(dibujoDe("mano").caliente[1]).toBe(0);
    expect(dibujoDe("mano").claro).toBe(true);
    expect(dibujoDe("texto").puntos.length).toBeGreaterThan(4);
  });

  it("el puntero normal es la misma cuenta que en Rust", () => {
    expect(punteroNormal(1080)).toBe(43);
    expect(punteroNormal(400)).toBe(24);
    expect(punteroNormal(4320)).toBe(64);
  });
});
