/**
 * Deshacer lo dibujado.
 *
 * Desde S23 lo dibujado se mueve, se reescribe y se borra suelto, y Ctrl+Z tiene que
 * devolver lo de antes del último cambio, no solo quitar la última marca. Y mover una
 * marca y moverla otra vez son dos Ctrl+Z, no doscientos (uno por movimiento del ratón):
 * eso lo asegura la capa, que manda el cambio una vez al soltar.
 */
import { describe, expect, it } from "vitest";
import { aplicarCambio, SIN_MARCAS, type CambioDeMarcas, type Marcas } from "./historialDeMarcas";
import { desplazar, renumerarPasos, type Anotacion } from "./anotaciones";

const flecha: Anotacion = { kind: "arrow", x1: 0.1, y1: 0.1, x2: 0.3, y2: 0.3, color: "#ef4444", text: "" };
const texto: Anotacion = { kind: "text", x1: 0.5, y1: 0.5, x2: 0.5, y2: 0.5, color: "#ef4444", text: "hola" };
const paso = (n: number, x: number): Anotacion => ({
  kind: "step",
  x1: x,
  y1: 0.5,
  x2: x,
  y2: 0.5,
  color: "#ef4444",
  text: String(n),
});

const seguidos = (...cambios: CambioDeMarcas[]): Marcas => cambios.reduce(aplicarCambio, SIN_MARCAS);
const poner = (...marcas: Anotacion[]): CambioDeMarcas => ({ tipo: "anadir", marcas });

describe("deshacer", () => {
  it("lo recién puesto se quita, como siempre", () => {
    const m = seguidos(poner(flecha), poner(texto), { tipo: "deshacer" });
    expect(m.lista).toEqual([flecha]);
  });

  it("lo movido vuelve a donde estaba", () => {
    const movida = desplazar(flecha, 0.2, 0.1);
    const m = seguidos(poner(flecha), { tipo: "cambiar", indice: 0, marca: movida });
    expect(m.lista[0].x1).toBeCloseTo(0.3, 5);
    expect(aplicarCambio(m, { tipo: "deshacer" }).lista).toEqual([flecha]);
  });

  it("y dos movimientos se deshacen de uno en uno", () => {
    const una = desplazar(flecha, 0.1, 0);
    const dos = desplazar(una, 0.1, 0);
    const m = seguidos(poner(flecha), { tipo: "cambiar", indice: 0, marca: una }, { tipo: "cambiar", indice: 0, marca: dos });
    expect(aplicarCambio(m, { tipo: "deshacer" }).lista).toEqual([una]);
  });

  it("lo reescrito vuelve a decir lo de antes", () => {
    const m = seguidos(poner(texto), { tipo: "cambiar", indice: 0, marca: { ...texto, text: "adiós" } }, { tipo: "deshacer" });
    expect(m.lista[0].text).toBe("hola");
  });

  it("lo borrado vuelve a su sitio, también en medio", () => {
    const m = seguidos(poner(flecha, texto, flecha), { tipo: "borrar", indice: 1 });
    expect(m.lista).toEqual([flecha, flecha]);
    expect(aplicarCambio(m, { tipo: "deshacer" }).lista).toEqual([flecha, texto, flecha]);
  });

  it("quitar todas también se deshace", () => {
    const m = seguidos(poner(flecha, texto), { tipo: "borrarTodo" }, { tipo: "deshacer" });
    expect(m.lista).toEqual([flecha, texto]);
  });

  it("lo que tapa «Tapar datos» de una vez se quita con un solo Ctrl+Z", () => {
    const tapas: Anotacion[] = [0.1, 0.4, 0.7].map((x) => ({ ...flecha, kind: "blur", x1: x, x2: x + 0.1 }));
    const m = seguidos(poner(flecha), poner(...tapas), { tipo: "deshacer" });
    expect(m.lista).toEqual([flecha]);
  });

  it("sin nada que deshacer no pasa nada", () => {
    expect(aplicarCambio(SIN_MARCAS, { tipo: "deshacer" })).toBe(SIN_MARCAS);
  });

  it("un cambio que no cambia nada no gasta un Ctrl+Z", () => {
    const m = seguidos(poner(flecha), { tipo: "cambiar", indice: 0, marca: { ...flecha } });
    expect(m.antes).toHaveLength(1);
  });
});

describe("elegir", () => {
  it("no es un cambio: Ctrl+Z no lo deshace", () => {
    const m = seguidos(poner(flecha), { tipo: "elegir", indice: 0 });
    expect(m.elegida).toBe(0);
    expect(m.antes).toHaveLength(1);
  });

  it("una que no existe no se elige", () => {
    expect(seguidos(poner(flecha), { tipo: "elegir", indice: 5 }).elegida).toBeNull();
  });

  it("borrar suelta lo elegido, y deshacer también", () => {
    const elegida = seguidos(poner(flecha, texto), { tipo: "elegir", indice: 1 });
    expect(aplicarCambio(elegida, { tipo: "borrar", indice: 1 }).elegida).toBeNull();
    // Después de deshacer, el sitio 1 podría ser otra marca, y Supr se llevaría la que no es.
    expect(aplicarCambio(elegida, { tipo: "deshacer" }).elegida).toBeNull();
  });

  it("mover la elegida la deja elegida", () => {
    const m = seguidos(poner(flecha), { tipo: "elegir", indice: 0 }, { tipo: "cambiar", indice: 0, marca: desplazar(flecha, 0.1, 0) });
    expect(m.elegida).toBe(0);
  });
});

describe("los pasos numerados", () => {
  it("al borrar uno de en medio, los de detrás corren su número", () => {
    const m = seguidos(poner(paso(1, 0.1), flecha, paso(2, 0.3), paso(3, 0.5)), { tipo: "borrar", indice: 2 });
    expect(m.lista.filter((a) => a.kind === "step").map((a) => a.text)).toEqual(["1", "2"]);
    // Y el que corre es el que estaba en 0,5, no otro.
    expect(m.lista[2]).toMatchObject({ x1: 0.5, text: "2" });
  });

  it("y deshacer el borrado los deja como estaban", () => {
    const m = seguidos(poner(paso(1, 0.1), paso(2, 0.3), paso(3, 0.5)), { tipo: "borrar", indice: 0 }, { tipo: "deshacer" });
    expect(m.lista.map((a) => a.text)).toEqual(["1", "2", "3"]);
  });

  it("si ya iban seguidos, no cambia nada", () => {
    const lista = [paso(1, 0.1), flecha, paso(2, 0.3)];
    expect(renumerarPasos(lista)).toEqual(lista);
  });
});

describe("mover una marca", () => {
  it("mueve las dos esquinas", () => {
    const m = desplazar(flecha, 0.2, -0.05);
    expect([m.x1, m.y1, m.x2, m.y2].map((v) => v.toFixed(3))).toEqual(["0.300", "0.050", "0.500", "0.250"]);
  });

  it("la curva se lleva su punto de control, aunque caiga fuera de la imagen", () => {
    const curva: Anotacion = { ...flecha, kind: "curve", x1: 0.2, y1: 0.9, x2: 0.8, y2: 0.9, puntos: [[0.5, -0.2]] };
    const m = desplazar(curva, 0.1, 0.05);
    expect(m.puntos![0][0]).toBeCloseTo(0.6, 5);
    expect(m.puntos![0][1]).toBeCloseTo(-0.15, 5);
    expect(m.y1).toBeCloseTo(0.95, 5);
  });

  it("contra el borde se para la marca entera, también el lápiz por su punto más lejano", () => {
    const lapiz: Anotacion = {
      ...flecha,
      kind: "pen",
      x1: 0.1,
      y1: 0.5,
      x2: 0.3,
      y2: 0.5,
      puntos: [
        [0.1, 0.5],
        [0.6, 0.2],
        [0.3, 0.5],
      ],
    };
    const m = desplazar(lapiz, 1, -1);
    // El punto de 0,6 es el que toca el borde derecho, y el de 0,2 el de arriba.
    expect(m.puntos![1][0]).toBeCloseTo(1, 5);
    expect(m.puntos![1][1]).toBeCloseTo(0, 5);
    expect(m.x1).toBeCloseTo(0.5, 5);
    expect(m.y1).toBeCloseTo(0.3, 5);
  });

  it("no cambia la marca de antes", () => {
    const antes = JSON.stringify(flecha);
    desplazar(flecha, 0.3, 0.3);
    expect(JSON.stringify(flecha)).toBe(antes);
  });
});

/** S24: con algo elegido, pinchar un color se lo cambia, y Ctrl+Z se lo devuelve. */
describe("cambiar el color de lo elegido", () => {
  const AZUL = "#0a9bff";
  const colorear = (indice: number, color = AZUL): CambioDeMarcas => ({ tipo: "colorear", indice, color });

  it("le cambia el color y la deja elegida", () => {
    const m = seguidos(poner(flecha, texto), { tipo: "elegir", indice: 1 }, colorear(1));
    expect(m.lista[1]).toEqual({ ...texto, color: AZUL });
    expect(m.lista[0]).toEqual(flecha);
    expect(m.elegida).toBe(1);
  });

  it("Ctrl+Z le devuelve el color de antes", () => {
    const m = seguidos(poner(flecha), colorear(0), { tipo: "deshacer" });
    expect(m.lista).toEqual([flecha]);
  });

  it("vale para todo lo que se dibuja con color: texto, flechas, líneas, cajas, círculos, trazos y números", () => {
    for (const kind of ["text", "arrow", "curve", "line", "box", "ellipse", "pen", "step"] as const) {
      const marca: Anotacion = { ...flecha, kind };
      expect(seguidos(poner(marca), colorear(0)).lista[0].color, kind).toBe(AZUL);
    }
  });

  it("no toca el resaltado, el mosaico ni el foco, que no llevan el color que se elige", () => {
    for (const kind of ["highlight", "blur", "focus"] as const) {
      const marca: Anotacion = { ...flecha, kind, color: "#fbbf24" };
      const antes = seguidos(poner(marca));
      expect(aplicarCambio(antes, colorear(0)), kind).toBe(antes);
    }
  });

  it("su mismo color no gasta un Ctrl+Z", () => {
    const antes = seguidos(poner(flecha));
    expect(aplicarCambio(antes, colorear(0, flecha.color))).toBe(antes);
  });

  it("una que no existe no se colorea", () => {
    const antes = seguidos(poner(flecha));
    expect(aplicarCambio(antes, colorear(3))).toBe(antes);
  });
});
