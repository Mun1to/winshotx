/**
 * S25 (marco): las medidas de la ventana y del portátil. Son los MISMOS números que comprueba
 * `las_medidas_son_las_mismas_que_dibuja_el_editor` en `encode/carcasa.rs`: si la vista previa
 * midiera otra cosa, lo que se ve no sería lo que sale en el archivo.
 */
import { afterEach, describe, expect, it } from "vitest";
import { botones, caminoDe, carcasaParaPedir, detallesDelPortatil, geometria, MARCOS, ventanaOscura } from "./carcasa";

describe("las medidas, iguales que en Rust", () => {
  it("la ventana a 1920 lleva la barra de 32 de Windows y un borde de un píxel", () => {
    const v = geometria("ventana", 1920, 1080);
    expect([v.ancho, v.alto, v.x, v.y]).toEqual([1922, 1114, 1, 33]);
    expect(v.radios).toEqual([0, 0, 7, 7]);
  });

  it("a 3840 crece como con el escalado de Windows al 200 %", () => {
    const v = geometria("ventana", 3840, 2160);
    expect([v.ancho, v.alto, v.x, v.y]).toEqual([3844, 2228, 2, 66]);
  });

  it("un trozo pequeño lleva la barra de 100 %, como una ventana de verdad", () => {
    const v = geometria("ventana", 400, 300);
    expect([v.ancho, v.alto, v.x, v.y]).toEqual([402, 334, 1, 33]);
  });

  it("el portátil: bisel, tapa y base, con la captura a su tamaño dentro", () => {
    const p = geometria("portatil", 1920, 1080);
    expect([p.ancho, p.alto, p.x, p.y]).toEqual([2184, 1225, 132, 58]);
    expect(p.sombra[0]).toEqual({ x: 86, y: 0, ancho: 2012, alto: 1196, radios: [38, 38, 9.5, 9.5] });
    expect(p.sombra[1]).toEqual({ x: 0, y: 1196, ancho: 2184, alto: 29, radios: [0, 0, 20, 20] });
    const pequeno = geometria("portatil", 300, 200);
    expect([pequeno.ancho, pequeno.alto, pequeno.x, pequeno.y]).toEqual([388, 248, 44, 19]);
  });

  it("los botones van a la derecha, cerrar en la esquina, y miden 10 a 100 %", () => {
    const g = geometria("ventana", 1920, 1080);
    const [min, max, cerrar] = botones(g, 1920, 1080);
    expect(min[0]).toBeLessThan(max[0]);
    expect(max[0]).toBeLessThan(cerrar[0]);
    expect(cerrar[2]).toBe(10);
    expect(cerrar[0] + cerrar[2] / 2).toBe(g.ancho - 1 - 23);
  });

  it("la cámara del portátil va en medio del bisel de arriba", () => {
    const g = geometria("portatil", 1280, 800);
    const d = detallesDelPortatil(g, 1280, 800);
    expect(d.camara.cx).toBe(g.ancho / 2);
    expect(d.camara.cy).toBe(g.y / 2);
  });
});

describe("lo que viaja y lo que se ofrece", () => {
  afterEach(() => localStorage.removeItem("winshotx.tema"));

  it("tres opciones, sin submenús", () => {
    expect(MARCOS.map((m) => m.nombre)).toEqual(["Ninguno", "Ventana", "Portátil"]);
  });

  it("la ventana viaja con su tema; el portátil y ninguno, sin él", () => {
    expect(carcasaParaPedir("ventana", true)).toEqual({ tipo: "ventana", oscura: true });
    expect(carcasaParaPedir("ventana", false)).toEqual({ tipo: "ventana", oscura: false });
    expect(carcasaParaPedir("portatil", true)).toEqual({ tipo: "portatil" });
    expect(carcasaParaPedir(null, true)).toBeNull();
  });

  it("el tema elegido en winshotx manda sobre el del sistema", () => {
    localStorage.setItem("winshotx.tema", "claro");
    expect(ventanaOscura()).toBe(false);
    localStorage.setItem("winshotx.tema", "oscuro");
    expect(ventanaOscura()).toBe(true);
  });

  it("el contorno en SVG lleva un arco por esquina redondeada y ninguno por las rectas", () => {
    const camino = caminoDe({ x: 0, y: 0, ancho: 100, alto: 50, radios: [0, 0, 8, 8] });
    expect(camino.match(/A/g)).toHaveLength(2);
    expect(caminoDe({ x: 0, y: 0, ancho: 100, alto: 50, radios: [8, 8, 8, 8] }, 0.5).match(/A4,4/g)).toHaveLength(4);
  });
});
