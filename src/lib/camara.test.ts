import { describe, expect, it } from "vitest";
import type { MarcoCamara } from "./contrato";
import {
  caja,
  esquinaDeAgarre,
  instanteDeLaCamara,
  marcoDeFabrica,
  mover,
  paraLosMandos,
  pasoDeLaCara,
  proporcion,
  radio,
  recorta,
  recorteVisible,
  redimensionar,
  SIN_RECORTE,
} from "./camara";

const marco = (forma: MarcoCamara["forma"], tam: number, x: number, y: number): MarcoCamara => ({
  forma,
  recorte: SIN_RECORTE,
  tam,
  x,
  y,
  borde: false,
});
const LIENZO = { ancho: 1920, alto: 1080 };
const CAM = { ancho: 1280, alto: 720 };

// Los mismos números que `encode::camara_estudio::pruebas` en Rust.
describe("la burbuja de la cámara", () => {
  it("de fábrica: un círculo de 345,6 px a 57,6 px de la derecha y de abajo", () => {
    const c = caja(marcoDeFabrica(1920, 1080), CAM, LIENZO);
    expect(c.ancho).toBeCloseTo(345.6, 2);
    expect(c.alto).toBeCloseTo(345.6, 2);
    expect(1920 - (c.x + c.ancho)).toBeCloseTo(57.6, 2);
    expect(1080 - (c.y + c.alto)).toBeCloseTo(57.6, 2);
  });

  it("el rectángulo sin recortar tiene la proporción de la cámara, y recortado la del recorte", () => {
    const r = caja(marco("rectangulo", 0.25, 0.5, 0.5), CAM, LIENZO);
    expect([r.ancho, r.alto]).toEqual([480, 270]);
    const recortado = { ...marco("redondeado", 0.25, 0.5, 0.5), recorte: { x1: 0, y1: 0, x2: 0.5, y2: 1 } };
    const m = caja(recortado, CAM, LIENZO);
    expect(m.alto).toBeCloseTo(480, 2);
    expect(m.ancho).toBeCloseTo(426.67, 2);
  });

  it("arrastrada contra un borde se queda dentro, y nunca mide más que el vídeo", () => {
    const c = caja(marco("cuadrado", 0.2, 1, 0), { ancho: 640, alto: 480 }, { ancho: 1000, alto: 500 });
    expect([c.x, c.y, c.ancho, c.alto]).toEqual([800, 0, 200, 200]);
    const enorme = caja(marco("circulo", 1, 0.5, 0.5), { ancho: 640, alto: 480 }, { ancho: 1000, alto: 500 });
    expect([enorme.ancho, enorme.alto]).toEqual([500, 500]);
    const roto = caja(marco("circulo", NaN, NaN, Infinity), { ancho: 640, alto: 480 }, { ancho: 1000, alto: 500 });
    expect(roto.ancho).toBeGreaterThan(0);
    expect(roto.y + roto.alto).toBeLessThanOrEqual(500);
  });

  it("el círculo enseña el centro cuadrado del recorte; el rectángulo, el recorte entero", () => {
    const v = recorteVisible(marco("circulo", 0.2, 0.5, 0.5), CAM);
    expect(v.x1).toBeCloseTo(0.21875, 4);
    expect(v.x2).toBeCloseTo(0.78125, 4);
    expect([v.y1, v.y2]).toEqual([0, 1]);
    const r = { ...marco("rectangulo", 0.2, 0.5, 0.5), recorte: { x1: 0.1, y1: 0.2, x2: 0.6, y2: 0.9 } };
    const vr = recorteVisible(r, CAM);
    expect(vr.x1).toBeCloseTo(0.1, 5);
    expect(vr.y2).toBeCloseTo(0.9, 5);
    // Un recorte sin anchura no deja la burbuja vacía: vale la imagen entera.
    expect(proporcion({ ...r, recorte: { x1: 0.4, y1: 0, x2: 0.4, y2: 1 } }, CAM)).toBeCloseTo(1280 / 720, 5);
  });

  // S13 (redondeo): los mismos números que `el_radio_de_cada_forma` en Rust.
  it("el radio: el círculo, media vuelta siempre; el cuadrado y el rectángulo, lo elegido; sin elegir, lo de antes", () => {
    const c = { x: 0, y: 0, ancho: 400, alto: 300 };
    const r = (forma: MarcoCamara["forma"], redondeo?: number) => radio({ forma, redondeo }, c);
    expect(r("circulo")).toBe(150);
    expect(r("circulo", 0)).toBe(150);
    expect(r("cuadrado")).toBe(0);
    expect(r("rectangulo")).toBe(0);
    expect(r("redondeado")).toBeCloseTo(54, 3);
    expect(r("cuadrado", 0.25)).toBe(75);
    expect(r("rectangulo", 0.5)).toBe(150);
    expect(r("rectangulo", 0.9)).toBe(150);
    expect(r("rectangulo", -1)).toBe(0);
    expect(r("rectangulo", NaN)).toBe(0);
    expect(r("redondeado", 0)).toBe(0);
    expect(r("redondeado", NaN)).toBeCloseTo(54, 3);
  });

  it("el «redondeado» de antes llega a los mandos como un rectángulo con sus esquinas, y sale igual", () => {
    const viejo = marco("redondeado", 0.25, 0.5, 0.5);
    const mandos = paraLosMandos(viejo);
    expect(mandos).toEqual({ ...viejo, forma: "rectangulo", redondeo: 0.18 });
    const c = caja(viejo, CAM, LIENZO);
    expect(caja(mandos, CAM, LIENZO)).toEqual(c);
    expect(radio(mandos, c)).toBeCloseTo(radio(viejo, c), 6);
    // Lo demás no se toca.
    const cuadrado = { ...marco("cuadrado", 0.2, 0.5, 0.5), redondeo: 0.3 };
    expect(paraLosMandos(cuadrado)).toBe(cuadrado);
  });

  it("el desfase: positivo es que la cámara arrancó tarde y va por detrás", () => {
    expect(instanteDeLaCamara(1000, 0, 0)).toBe(1000);
    expect(instanteDeLaCamara(1000, 0, 300)).toBe(700);
    expect(instanteDeLaCamara(100, 0, 300)).toBe(-200);
    expect(instanteDeLaCamara(1050, 50, -100)).toBe(1100);
  });

  it("solo recorta lo que deja algo fuera", () => {
    expect(recorta(SIN_RECORTE)).toBe(false);
    expect(recorta({ x1: 0.2, y1: 0, x2: 1, y2: 1 })).toBe(true);
    expect(recorta({ x1: 0.4, y1: 0, x2: 0.4, y2: 1 })).toBe(false);
  });
});

describe("moverla y cambiarle el tamaño", () => {
  it("se mueve a donde se suelta, sin salirse", () => {
    const m = mover(marco("circulo", 0.2, 0.5, 0.5), CAM, LIENZO, 300, 300);
    expect(m.x).toBeCloseTo(300 / 1920, 5);
    expect(m.y).toBeCloseTo(300 / 1080, 5);
    const fuera = mover(marco("circulo", 0.2, 0.5, 0.5), CAM, LIENZO, -50, 5000);
    expect(fuera.x).toBeCloseTo(192 / 1920, 5);
    expect(fuera.y).toBeCloseTo((1080 - 192) / 1080, 5);
  });

  it("la esquina de agarre es la que mira al centro del vídeo", () => {
    expect(esquinaDeAgarre(caja(marcoDeFabrica(1920, 1080), CAM, LIENZO), LIENZO)).toEqual({ haciaX: -1, haciaY: -1 });
    expect(esquinaDeAgarre(caja(marco("circulo", 0.2, 0.1, 0.1), CAM, LIENZO), LIENZO)).toEqual({ haciaX: 1, haciaY: 1 });
  });

  it("estirar la esquina hasta el puntero deja quieta la de enfrente", () => {
    const fabrica = marcoDeFabrica(1920, 1080);
    const antes = caja(fabrica, CAM, LIENZO);
    const hacia = esquinaDeAgarre(antes, LIENZO);
    // La esquina de arriba a la izquierda, 100 px más arriba y a la izquierda.
    const nuevo = redimensionar(fabrica, CAM, LIENZO, antes, hacia, { x: antes.x - 100, y: antes.y - 100 });
    const despues = caja(nuevo, CAM, LIENZO);
    expect(despues.ancho).toBeCloseTo(antes.ancho + 100, 1);
    expect(despues.x + despues.ancho).toBeCloseTo(antes.x + antes.ancho, 1);
    expect(despues.y + despues.alto).toBeCloseTo(antes.y + antes.alto, 1);
    // Y no baja del mínimo ni pasa del máximo del mando.
    const diminuto = redimensionar(fabrica, CAM, LIENZO, antes, hacia, { x: antes.x + antes.ancho, y: antes.y + antes.alto });
    expect(diminuto.tam).toBeCloseTo(0.08, 5);
    const gigante = redimensionar(fabrica, CAM, LIENZO, antes, hacia, { x: -5000, y: -5000 });
    expect(gigante.tam).toBeCloseTo(0.5, 5);
  });
});

describe("la cara al compás de la pantalla", () => {
  // La cámara arrancó 2 s después que la pantalla, y su archivo dura 30 s.
  const paso = (ms: number, sonando = true) => pasoDeLaCara(ms, 100, 2000, 30, sonando);

  it("mientras la cámara no ha arrancado se queda QUIETA en su primera imagen, no se reproduce", () => {
    // Reproducirla ahí la hacía saltar a 0 una y otra vez: el mismo trocito durante 2 s.
    expect(paso(100)).toEqual({ meta: 0, suena: false });
    expect(paso(1500)).toEqual({ meta: 0, suena: false });
  });

  it("en cuanto arranca, se reproduce desde su instante", () => {
    expect(paso(2100)).toEqual({ meta: 0, suena: true });
    expect(paso(3100)).toEqual({ meta: 1, suena: true });
  });

  it("con la pantalla parada, quieta en su sitio; acabada, en su última imagen", () => {
    expect(paso(3100, false)).toEqual({ meta: 1, suena: false });
    const fin = paso(40_000);
    expect(fin.suena).toBe(false);
    expect(fin.meta).toBeCloseTo(29.96, 5);
  });
});
