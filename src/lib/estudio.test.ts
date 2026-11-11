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
  sitioDelPuntero,
  movimientos,
  opacidadDelPuntero,
  medidasConMarco,
  sitioSuavizado,
  opacidadAlEscribir,
  FUNDIDO_ESCRIBIR_MS,
  ondaEn,
  ondasEn,
  ONDA_MS,
  estelaDelPuntero,
  desenfoqueDeCamara,
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

/**
 * El puntero suave, el que se esconde y el marco de la vista previa. Las cifras son las de
 * las pruebas de Rust (`encode::estudio::puntero_de_estudio`): la vista previa y el archivo
 * tienen que dar lo mismo.
 */
describe("el puntero del estudio, con las cifras de Rust", () => {
  const SALTO: [number, number, number][] = [
    [0, 0, 0],
    [100, 100, 0],
  ];

  it("sin suavizar está donde estaba", () => {
    expect(sitioDelPuntero(SALTO, 100, false)).toEqual([100, 0]);
    expect(sitioDelPuntero(SALTO, 99, false)).toEqual([0, 0]);
  });

  it("suavizado, un salto se reparte en el tiempo: 54,4 a los 100 ms", () => {
    const [x, y] = sitioDelPuntero(SALTO, 100, true)!;
    expect(x).toBeCloseTo(54.4, 3);
    expect(y).toBe(0);
    expect(sitioDelPuntero(SALTO, 230, true)).toEqual([100, 0]);
  });

  it("suavizado no va por detrás en un movimiento seguido", () => {
    const recta = Array.from({ length: 101 }, (_, k) => [k * 8, k * 8, 0] as [number, number, number]);
    expect(sitioDelPuntero(recta, 400, true)![0]).toBeCloseTo(400, 0);
  });

  it("suavizado quita el temblor de la mano", () => {
    const temblor = Array.from(
      { length: 100 },
      (_, k) => [k * 8, 200 + (k % 2 === 0 ? 4 : -4), 0] as [number, number, number],
    );
    expect(Math.abs(sitioDelPuntero(temblor, 400, true)![0] - 200)).toBeLessThan(0.6);
  });

  it("los movimientos son los cambios de sitio", () => {
    expect(
      movimientos([
        [0, 1, 1],
        [16, 1, 1],
        [32, 2, 1],
        [48, 2, 1],
        [64, 1, 1],
      ]),
    ).toEqual([0, 32, 64]);
  });

  it("quieto un rato se va apagando, y un clic lo despierta", () => {
    const movido = [0, 1000];
    expect(opacidadDelPuntero(movido, [], 2000)).toBe(1);
    expect(opacidadDelPuntero(movido, [], 2650)).toBeCloseTo(0.5, 6);
    expect(opacidadDelPuntero(movido, [], 2900)).toBe(0);
    expect(opacidadDelPuntero(movido, [{ ms: 2600, x: 0, y: 0, derecho: false }], 2900)).toBe(1);
  });
});

describe("la vista previa con el marco puesto", () => {
  it("sin marco, la imagen ocupa el hueco como siempre", () => {
    expect(medidasConMarco(800, 500, 1600, 1000, null)).toEqual({ width: 800, height: 500, aire: 0, radio: 0 });
  });

  it("con marco, imagen y aire caben juntos y el aire va en proporción", () => {
    const m = medidasConMarco(800, 500, 1600, 1000, { aire: 0.05, esquinas: 0.01, fondo: "noche", sombra: true });
    // Aquí manda el alto: 500 = w / 1,6 + 0,1 · w, así que la imagen mide 690 de ancho y
    // el aire 34 por lado.
    expect(m.width).toBe(690);
    expect(m.aire).toBe(34);
    expect(m.width + 2 * m.aire).toBeLessThanOrEqual(800);
    expect(m.height + 2 * m.aire).toBeLessThanOrEqual(500);
    expect(m.radio).toBe(7);
  });
});

// S7 (cursor): las mismas cifras que `encode::estudio::puntero_de_estudio` en Rust.
describe("el cursor del estudio (S7)", () => {
  const clic = (ms: number) => ({ ms, x: 100, y: 0, derecho: false });

  it("«mucho» mira más lejos y se ciñe al clic", () => {
    const salto: [number, number, number][] = [
      [0, 0, 0],
      [1000, 100, 0],
    ];
    expect(sitioSuavizado(salto, [], 1000, "poco")![0]).toBeCloseTo(53.125, 3);
    expect(sitioSuavizado(salto, [], 1000, "mucho")![0]).toBeCloseTo(51.51515, 3);
    expect(sitioSuavizado(salto, [], 1000, "nada")).toEqual([100, 0]);
    expect(sitioSuavizado(salto, [clic(1040)], 1000, "mucho")![0]).toBeCloseTo(58.33333, 3);
    expect(sitioSuavizado(salto, [clic(1040)], 1040, "mucho")).toEqual([100, 0]);
    // «poco» es el movimiento suave de antes.
    expect(sitioSuavizado(salto, [], 1000, "poco")).toEqual(sitioDelPuntero(salto, 1000, true));
  });

  it("al escribir se va con un fundido y un clic lo trae un rato", () => {
    const escrito = [{ desdeMs: 1000, hastaMs: 3000, caret: null }];
    const o = (ms: number, clics: ReturnType<typeof clic>[] = []) => opacidadAlEscribir(escrito, [0], clics, ms);
    expect(o(900)).toBe(1);
    expect(o(1075)).toBeCloseTo(0.5, 6);
    expect(o(2000)).toBe(0);
    expect(o(3100)).toBeCloseTo(2 / 3, 6);
    expect(o(3150)).toBe(1);
    const enMedio = [clic(2000)];
    expect(o(1925, enMedio)).toBeCloseTo(0.5, 6);
    expect(o(2000, enMedio)).toBe(1);
    expect(o(2800, enMedio)).toBe(1);
    expect(o(2875, enMedio)).toBeCloseTo(0.5, 6);
    expect(o(2950, enMedio)).toBe(0);
    expect(FUNDIDO_ESCRIBIR_MS).toBe(150);
  });

  it("la onda sale del aro y se apaga", () => {
    const empieza = ondaEn(0, 1080);
    expect(empieza.radio).toBeCloseTo(15.552, 3);
    expect(empieza.opacidad).toBeCloseTo(0.6, 6);
    expect(empieza.grosor).toBeCloseTo(2.592, 3);
    const medio = ondaEn(0.5, 1080);
    expect(medio.radio).toBeCloseTo(60.912, 2);
    expect(medio.opacidad).toBeCloseTo(0.15, 6);
    expect(medio.grosor).toBeCloseTo(1.944, 3);
    expect(ondaEn(1, 1080).opacidad).toBe(0);
    expect(ondasEn([clic(1000)], 1000 + ONDA_MS)).toEqual([]);
    expect(ondasEn([clic(1000)], 1325)[0].avance).toBeCloseTo(0.5, 6);
  });

  it("la estela son cuatro copias cada vez más claras", () => {
    expect(estelaDelPuntero(1000, 0)).toEqual([]);
    const copias = estelaDelPuntero(1000, 1);
    expect(copias.map((c) => c.t)).toEqual([930, 948, 965, 983]);
    expect(copias[0].alfa).toBeCloseTo(0.11, 6);
    expect(copias[3].alfa).toBeCloseTo(0.44, 6);
    expect(estelaDelPuntero(1000, 0.5)[0].t).toBe(965);
    expect(estelaDelPuntero(20, 1)).toHaveLength(1);
  });

  it("la cámara que se mueve emborrona la vista previa, y quieta no", () => {
    const ahora = { x1: 0.25, y1: 0, x2: 0.75, y2: 1 };
    expect(desenfoqueDeCamara(ahora, ahora, 800, 450, 1)).toBe(0);
    const antes = { x1: 0.2, y1: 0, x2: 0.7, y2: 1 };
    // 0,05 de 0,5 de ancho son 80 de 800 píxeles: un desenfoque de 23,2.
    expect(desenfoqueDeCamara(antes, ahora, 800, 450, 1)).toBeCloseTo(23.2, 3);
    expect(desenfoqueDeCamara(antes, ahora, 800, 450, 0)).toBe(0);
    expect(desenfoqueDeCamara(null, null, 800, 450, 1)).toBe(0);
  });
});
