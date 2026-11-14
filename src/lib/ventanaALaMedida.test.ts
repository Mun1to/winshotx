import { describe, expect, it } from "vitest";
import { altoPedido, encajar, type Sitio } from "./ventanaALaMedida";

/**
 * La ventana de ajustes a la medida de su pestaña (S24). Las cifras son las medidas en Edge
 * sin ventana el 14-nov-2026: Capturar pide 613 px lógicos por dentro, Grabar 742, La app 575
 * y Teclas de Windows 306. El marco de Windows 11 (barra de título y bordes) son unos 40 px
 * físicos a 100 %.
 */
const base: Sitio = {
  pedido: 613,
  escala: 1,
  marco: 40,
  y: 140,
  altoFuera: 800,
  zona: { y: 0, alto: 1032 },
  conservarCentro: false,
};

describe("el alto que pide la ventana", () => {
  it("es lo de fuera del hueco más lo que mide la pestaña", () => {
    // Ventana de 760 con un hueco de 668 (cabecera 48 + barra 44): Capturar mide 521.
    expect(altoPedido({ altoVentana: 760, altoHueco: 668, altoContenido: 521 })).toBe(613);
  });

  it("deja la ventana pedida tal cual si cabe en la pantalla", () => {
    expect(encajar(base)).toEqual({ alto: 613, y: null });
  });

  it("al abrirse por primera vez, se queda centrada donde nació", () => {
    // Nace centrada con 760 + 40 por fuera; con 613 + 40 baja la mitad de lo que encoge.
    const r = encajar({ ...base, conservarCentro: true });
    expect(r.alto).toBe(613);
    expect(r.y).toBe(Math.round(140 + (800 - 653) / 2));
  });

  it("al cambiar de pestaña, conserva el borde de arriba para que las pestañas no se muevan", () => {
    expect(encajar({ ...base, pedido: 306 }).y).toBeNull();
    expect(encajar({ ...base, pedido: 742 }).y).toBeNull();
  });

  it("a 125 % y 150 % pide en físicos, redondeando hacia arriba para no sacar la rueda por medio píxel", () => {
    expect(encajar({ ...base, escala: 1.25 }).alto).toBe(767); // 613 x 1,25 = 766,25
    expect(encajar({ ...base, escala: 1.5, pedido: 742, zona: { y: 0, alto: 1400 } }).alto).toBe(1113);
    expect(encajar({ ...base, escala: 1.25, pedido: 600.4 }).alto).toBe(751);
  });

  it("en una pantalla baja no se sale por abajo: sube y, si no cabe, se queda con la zona útil", () => {
    // Un portátil de 1366 x 768 al 125 %: 768 - 60 de barra de tareas = 708 físicos útiles.
    const zona = { y: 0, alto: 708 };
    const r = encajar({ ...base, escala: 1.25, pedido: 742, zona, y: 200, altoFuera: 990 });
    expect(r.alto).toBe(708 - 40);
    expect(r.y).toBe(0);
  });

  it("si cabe pero el pie se iba por debajo de la barra de tareas, sube lo justo", () => {
    // Abierta abajo del todo, pasar de Capturar a Grabar la sacaba por abajo.
    const r = encajar({ ...base, pedido: 742, y: 400 });
    expect(r.alto).toBe(742);
    expect(r.y).toBe(1032 - 782);
  });

  it("nunca sube por encima de la pantalla, aunque sea en un monitor colocado encima del principal", () => {
    const zona = { y: -1080, alto: 1032 };
    const r = encajar({ ...base, zona, y: -1200 });
    expect(r.y).toBe(-1080);
  });

  it("el historial no pasa de su máximo aunque tenga un año de fotos", () => {
    expect(encajar({ ...base, pedido: 4000, maximo: 760 }).alto).toBe(760);
  });

  it("sin saber la pantalla, pide lo que mide el contenido", () => {
    expect(encajar({ ...base, pedido: 742, zona: null })).toEqual({ alto: 742, y: null });
  });
});
