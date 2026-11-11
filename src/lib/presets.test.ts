import { describe, expect, it } from "vitest";
import {
  borrar,
  edicionDe,
  esElMismo,
  ESTILO_DE_FABRICA,
  estiloDe,
  estiloDeLasCapturas,
  guardarComo,
  leerEstilo,
  leerPresets,
  marcoCapturasDe,
  panelDe,
  renombrar,
  type EstadoDelPanel,
  type Estilo,
} from "./presets";
import { punteroNormal } from "./estudio";

const PANEL: EstadoDelPanel = {
  zoom: 2,
  velocidadZoom: 1.6,
  cursor: 44,
  aros: true,
  teclas: true,
  suave: true,
  esconder: false,
  margen: 96,
  esquinas: 24,
  fondo: "atardecer",
  sombra: true,
};
const GRANDE = { ancho: 1920, altoGrabado: 1080 };
const estilo = (cambio: Partial<Estilo> = {}): Estilo => ({ ...ESTILO_DE_FABRICA, ...cambio });

describe("leer un estilo guardado", () => {
  it("uno viejo al que le faltan campos se lee con lo de fábrica", () => {
    const leido = leerEstilo({ zoom: 2, fondo: "menta" });
    expect(leido.zoom).toBe(2);
    expect(leido.fondo).toBe("menta");
    expect(leido.suave).toBe(true);
    expect(leido.estela).toBe(0);
    expect(leido.camara).toBeNull();
  });

  it("lo roto o de otro tipo sale de fábrica, nunca un error", () => {
    const leido = leerEstilo({ zoom: "mucho", aire: 9, fondo: "fucsia", sonidoClic: NaN, fondoElegido: { tipo: "color", rgb: [1, 2] } });
    expect(leido).toEqual({ ...ESTILO_DE_FABRICA, aire: 0.5 });
    expect(leerEstilo(null)).toEqual(ESTILO_DE_FABRICA);
    expect(leerEstilo([1, 2])).toEqual(ESTILO_DE_FABRICA);
  });

  it("los fondos propios y la cámara se leen enteros", () => {
    const leido = leerEstilo({
      fondoElegido: { tipo: "degradado", rgb: [10, 20, 30] },
      camara: { forma: "redondeado", recorte: { x1: 0.1, y1: 0, x2: 0.9, y2: 1 }, tam: 0.25, x: 0.8, y: 0.7, borde: true },
      suavizado: "mucho",
    });
    expect(leido.fondoElegido).toEqual({ tipo: "degradado", rgb: [10, 20, 30] });
    expect(leido.camara?.forma).toBe("redondeado");
    expect(leido.suavizado).toBe("mucho");
  });

  it("la lista se queda sin los que no tienen nombre", () => {
    const lista = leerPresets([{ nombre: " Tutorial ", estilo: { zoom: 2 } }, { nombre: "" }, { estilo: {} }, "x"]);
    expect(lista.map((p) => p.nombre)).toEqual(["Tutorial"]);
    expect(leerPresets(undefined)).toEqual([]);
  });
});

describe("guardar, renombrar y borrar", () => {
  it("guardar con un nombre que ya está lo sustituye en su sitio, sin mirar mayúsculas", () => {
    let lista = guardarComo([], "Tutorial", estilo({ zoom: 2 }));
    lista = guardarComo(lista, "Redes", estilo());
    lista = guardarComo(lista, "tutorial", estilo({ zoom: 3 }));
    expect(lista.map((p) => [p.nombre, p.estilo.zoom])).toEqual([
      ["tutorial", 3],
      ["Redes", 1],
    ]);
    expect(guardarComo(lista, "   ", estilo())).toBe(lista);
  });

  it("no renombra a un nombre vacío ni a uno que ya tiene otro", () => {
    const lista = guardarComo(guardarComo([], "A", estilo()), "B", estilo());
    expect(renombrar(lista, 0, "B")).toBe(lista);
    expect(renombrar(lista, 0, "  ")).toBe(lista);
    expect(renombrar(lista, 0, "a")[0].nombre).toBe("a");
    expect(renombrar(lista, 0, " C ")[0].nombre).toBe("C");
    expect(borrar(lista, 0).map((p) => p.nombre)).toEqual(["B"]);
  });
});

describe("del panel al estilo y vuelta", () => {
  it("se guarda en proporción: el aire y las esquinas sobre el ancho, el puntero sobre el normal", () => {
    const e = estiloDe(PANEL, {}, GRANDE);
    expect(e.aire).toBeCloseTo(0.05, 4);
    expect(e.esquinas).toBeCloseTo(0.0125, 4);
    expect(e.puntero).toBeCloseTo(44 / punteroNormal(1080), 2);
  });

  it("aplicado a otro tamaño, sale igual de bonito: el 5 % de aire es el 5 %", () => {
    const e = estiloDe(PANEL, {}, GRANDE);
    const chico = panelDe(e, { ancho: 800, altoGrabado: 500 });
    expect(chico.margen).toBe(40);
    expect(chico.esquinas).toBe(10);
    // El puntero va de 4 en 4 y en proporción a lo grabado.
    expect(chico.cursor % 4).toBe(0);
    expect(chico.cursor).toBeLessThan(PANEL.cursor);
  });

  it("de vuelta al mismo tamaño, el panel queda como estaba", () => {
    expect(panelDe(estiloDe(PANEL, {}, GRANDE), GRANDE)).toEqual(PANEL);
  });

  it("sin puntero y sin fondo se quedan apagados al aplicarlo", () => {
    const apagado = panelDe(estilo({ puntero: 0, aire: 0 }), GRANDE);
    expect(apagado.cursor).toBe(0);
    expect(apagado.margen).toBe(0);
  });

  it("lleva lo del plan que no es de la grabación, y nunca zooms, cortes ni tapas", () => {
    const e = estiloDe(PANEL, {
      fondo: { tipo: "color", rgb: [1, 2, 3] },
      estela: 0.4,
      zooms: [],
      tramos: [{ desdeMs: 0, hastaMs: 10, velocidad: 2 }],
      tapar: [{ desdeMs: 0, hastaMs: 10, x: 0, y: 0, ancho: 1, alto: 1 }],
    }, GRANDE);
    expect(e.fondoElegido).toEqual({ tipo: "color", rgb: [1, 2, 3] });
    const edicion = edicionDe(e);
    expect(edicion.estela).toBe(0.4);
    expect(edicion).not.toHaveProperty("zooms");
    expect(edicion).not.toHaveProperty("tramos");
    expect(edicion).not.toHaveProperty("tapar");
  });
});

describe("saber qué estilo hay puesto", () => {
  it("recién aplicado sigue diciendo su nombre, aunque se redondee al ponerlo", () => {
    const guardado = estilo({ puntero: 1.37, aire: 0.0517, zoom: 2 });
    const puesto = estiloDe(panelDe(guardado, GRANDE), edicionDe(guardado), GRANDE);
    expect(esElMismo(guardado, puesto)).toBe(true);
  });

  it("lo que no se ve no cuenta: sin aire, el color del fondo da igual", () => {
    expect(esElMismo(estilo({ aire: 0, fondo: "menta" }), estilo({ aire: 0, fondo: "noche" }))).toBe(true);
    expect(esElMismo(estilo({ aire: 0.05, fondo: "menta" }), estilo({ aire: 0.05, fondo: "noche" }))).toBe(false);
  });

  it("un cambio de verdad es otro estilo", () => {
    expect(esElMismo(estilo({ puntero: 1 }), estilo({ puntero: 1.5 }))).toBe(false);
    expect(esElMismo(estilo({ aros: true }), estilo())).toBe(false);
  });

  it("guardado sin cámara y puesto en una grabación con cámara, sigue diciendo su nombre", () => {
    // Probado en pantalla: «Prueba de noche» se aplicaba bien, pero el botón decía «Sin guardar».
    const guardado = estilo({ fondo: "oceano", aire: 0.05, estela: 0.6, ondaClic: true });
    const camara: Estilo["camara"] = {
      forma: "circulo",
      recorte: { x1: 0, y1: 0, x2: 1, y2: 1 },
      tam: 0.18,
      x: 0.9,
      y: 0.85,
      borde: false,
    };
    const { camara: _sin, ...resto } = edicionDe(guardado);
    const puesto = estiloDe(panelDe(guardado, GRANDE), { ...resto, camara }, GRANDE);
    expect(esElMismo(guardado, puesto)).toBe(true);
    // Pero dos cámaras distintas sí son dos estilos distintos.
    const otra: Estilo["camara"] = { ...camara!, forma: "cuadrado" };
    expect(esElMismo(estilo({ camara }), estilo({ camara: otra }))).toBe(false);
  });
});

describe("el estilo en las capturas", () => {
  it("coge el fondo, el propio y el aire, pasado a tanto por ciento del lado corto", () => {
    const marco = marcoCapturasDe(estilo({ aire: 0.05, fondo: "menta", fondoElegido: { tipo: "color", rgb: [9, 9, 9] } }));
    expect(marco).toEqual({ aire: 9, fondo: "menta", elegido: { tipo: "color", rgb: [9, 9, 9] } });
    expect(marcoCapturasDe(estilo({ aire: 0 })).aire).toBe(0);
  });

  it("sabe cuál de la lista tienen puesto las capturas", () => {
    const lista = guardarComo(guardarComo([], "Uno", estilo({ aire: 0.05 })), "Dos", estilo({ aire: 0.05, fondo: "noche" }));
    expect(estiloDeLasCapturas(lista, marcoCapturasDe(lista[1].estilo))).toBe(1);
    expect(estiloDeLasCapturas(lista, { aire: 9, fondo: "coral" })).toBe(-1);
    expect(estiloDeLasCapturas(lista, { aire: 0, fondo: "noche" })).toBe(-1);
  });
});
