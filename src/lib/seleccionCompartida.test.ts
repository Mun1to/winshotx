/**
 * Las cuentas de la seleccion que cruza de una pantalla a otra, con las pantallas de Munito
 * medidas el 16 de noviembre de 2026: la principal en el origen, la de la izquierda en x
 * NEGATIVA y mas baja que la principal (deja un hueco arriba a la izquierda), y el portatil
 * debajo al 125 %.
 */
import { describe, expect, it } from "vitest";
import {
  IMAN,
  SIN_AVISO,
  aFisico,
  apartarDeOtras,
  cajaDe,
  conIman,
  cursorDelGesto,
  enUnaPantalla,
  etiquetaDeOverlay,
  interseccion,
  monitoresDe,
  pantallaDeLaMedida,
  pantallaDelPunto,
  pantallaQueManda,
  pantallasQueToca,
  puntoACss,
  puntoAFisico,
  queAvisar,
  tocaOtraPantalla,
} from "./seleccionCompartida";
import { aPantalla, aVirtual } from "./pantallas";
import type { MonitorInfo } from "./types";

function monitor(id: number, x: number, y: number, width: number, height: number, scale = 1): MonitorInfo {
  return { id, label: `M${id}`, x, y, width, height, scale, isPrimary: id === 1 };
}

const PRINCIPAL = monitor(1, 0, 0, 1920, 1080);
const IZQUIERDA = monitor(2, -1920, 483, 1920, 1080);
const PORTATIL = monitor(3, 0, 1080, 1920, 1200, 1.25);
const LAS_SUYAS = [PRINCIPAL, IZQUIERDA, PORTATIL];

describe("lo basico", () => {
  it("la etiqueta es la de la ventana que abre Rust", () => {
    expect(etiquetaDeOverlay(3)).toBe("overlay-3");
  });

  it("un payload viejo sin la lista es una sola pantalla; con ella, la propia manda sobre la suya", () => {
    expect(monitoresDe(PRINCIPAL)).toEqual([PRINCIPAL]);
    expect(monitoresDe(PRINCIPAL, [])).toEqual([PRINCIPAL]);
    expect(monitoresDe(PRINCIPAL, [IZQUIERDA])).toEqual([PRINCIPAL, IZQUIERDA]);
    const vieja = { ...PRINCIPAL, label: "otra" };
    expect(monitoresDe(PRINCIPAL, [vieja, PORTATIL])).toEqual([PRINCIPAL, PORTATIL]);
  });

  it("la interseccion, tambien con x negativas, y nada si solo se tocan por el borde", () => {
    const r = { x: -150, y: 700, width: 450, height: 200 };
    expect(interseccion(r, IZQUIERDA)).toEqual({ x: -150, y: 700, width: 150, height: 200 });
    expect(interseccion(r, PRINCIPAL)).toEqual({ x: 0, y: 700, width: 300, height: 200 });
    expect(interseccion({ x: 0, y: 0, width: 1920, height: 1080 }, PORTATIL)).toBeNull();
  });

  it("la caja del escritorio entero, con el hueco de arriba a la izquierda dentro", () => {
    expect(cajaDe(LAS_SUYAS)).toEqual({ x: -1920, y: 0, width: 3840, height: 2280 });
    expect(cajaDe([])).toBeNull();
  });

  it("de CSS del portatil a fisicos y vuelta, exacto", () => {
    const r = { x: 125, y: 890, width: 375, height: 440 };
    const css = aPantalla(r, PORTATIL, 1.25);
    expect(css).toEqual({ x: 100, y: -152, width: 300, height: 352 });
    expect(aFisico(css, PORTATIL, 1.25)).toEqual(r);
    // Lo que se manda a Rust es lo mismo.
    expect(aVirtual(css, PORTATIL, 1.25)).toEqual(r);
    // Y el raton, con la escala de SU pantalla.
    expect(puntoAFisico({ x: 400, y: -152 }, PORTATIL, 1.25)).toEqual({ x: 500, y: 890 });
    expect(puntoACss({ x: 500, y: 890 }, PORTATIL, 1.25)).toEqual({ x: 400, y: -152 });
  });

  it("un recorte que empieza mide cero, sin el minimo de dos de aVirtual", () => {
    expect(aFisico({ x: 10, y: 10, width: 0, height: 0 }, PRINCIPAL, 1)).toEqual({ x: 10, y: 10, width: 0, height: 0 });
  });
});

describe("de que pantalla es cada cosa", () => {
  it("el punto: en una, en el hueco, y el borde es de la de al lado", () => {
    expect(pantallaDelPunto({ x: -10, y: 600 }, LAS_SUYAS)?.id).toBe(2);
    expect(pantallaDelPunto({ x: -10, y: 300 }, LAS_SUYAS)).toBeNull();
    expect(pantallaDelPunto({ x: 500, y: 1080 }, LAS_SUYAS)?.id).toBe(3);
    expect(pantallaDelPunto({ x: 1920, y: 500 }, LAS_SUYAS)).toBeNull();
  });

  it("las que toca un recorte que baja de la principal al portatil y a la izquierda", () => {
    const r = { x: -500, y: 900, width: 600, height: 670 };
    expect(pantallasQueToca(r, LAS_SUYAS).map((m) => m.id)).toEqual([1, 2, 3]);
  });

  it("manda la que se lleva mas trozo", () => {
    // 1720 x 200 en la principal y 150 x 200 en la de al lado.
    const derecha = monitor(2, 1920, 0, 1920, 1080);
    expect(pantallaQueManda({ x: 200, y: 300, width: 1870, height: 200 }, [PRINCIPAL, derecha])?.id).toBe(1);
    expect(pantallaQueManda({ x: 1800, y: 300, width: 720, height: 200 }, [PRINCIPAL, derecha])?.id).toBe(2);
    // Del portatil hacia arriba: 375 x 190 en la principal y 375 x 250 en el portatil.
    expect(pantallaQueManda({ x: 125, y: 890, width: 375, height: 440 }, LAS_SUYAS)?.id).toBe(3);
  });

  it("empate de area: la del centro; y si el centro no es de ninguna, la de id menor", () => {
    const derecha = monitor(5, 1920, 0, 1920, 1080);
    // 100 y 100 de ancho: el centro (1920) ya es de la de la derecha.
    expect(pantallaQueManda({ x: 1820, y: 0, width: 200, height: 100 }, [PRINCIPAL, derecha])?.id).toBe(5);
    // Dos pantallas apiladas con un hueco de 100 entre medias: el centro cae en el hueco.
    const abajo = monitor(4, 0, 1180, 1920, 1080);
    expect(pantallaQueManda({ x: 0, y: 980, width: 100, height: 300 }, [abajo, PRINCIPAL])?.id).toBe(1);
    expect(pantallaQueManda({ x: -3000, y: 0, width: 10, height: 10 }, LAS_SUYAS)).toBeNull();
  });

  it("la medida va en la pantalla de la esquina de arriba a la izquierda, o en la que manda si cae en un hueco", () => {
    expect(pantallaDeLaMedida({ x: 125, y: 890, width: 375, height: 440 }, LAS_SUYAS)?.id).toBe(1);
    // Empieza en el hueco de arriba a la izquierda y se lleva mas de la principal.
    expect(pantallaDeLaMedida({ x: -100, y: 300, width: 600, height: 400 }, LAS_SUYAS)?.id).toBe(1);
  });

  it("si toca otra pantalla, por el recorte o por el raton", () => {
    const dentro = { x: 200, y: 300, width: 300, height: 200 };
    expect(tocaOtraPantalla(dentro, { x: 500, y: 500 }, 1, LAS_SUYAS)).toBe(false);
    expect(tocaOtraPantalla(dentro, { x: -5, y: 600 }, 1, LAS_SUYAS)).toBe(true);
    expect(tocaOtraPantalla({ ...dentro, height: 800 }, null, 1, LAS_SUYAS)).toBe(true);
    // El raton en el hueco no es otra pantalla.
    expect(tocaOtraPantalla(dentro, { x: -5, y: 300 }, 1, LAS_SUYAS)).toBe(false);
  });
});

describe("el iman del borde", () => {
  it("pasarse menos de 24 px de la pantalla deja el lado en el borde; desde 24, cruza", () => {
    expect(conIman({ x: 1930, y: 500 }, PRINCIPAL)).toEqual({ x: 1920, y: 500 });
    expect(conIman({ x: 1920 + IMAN - 1, y: 500 }, PRINCIPAL)).toEqual({ x: 1920, y: 500 });
    expect(conIman({ x: 1920 + IMAN, y: 500 }, PRINCIPAL)).toEqual({ x: 1920 + IMAN, y: 500 });
    expect(conIman({ x: 2070, y: 500 }, PRINCIPAL)).toEqual({ x: 2070, y: 500 });
  });

  it("hacia la izquierda en x negativa, y tambien si debajo hay un hueco y no otra pantalla", () => {
    // A la altura 600 hay pantalla a la izquierda; a la 300, el hueco.
    expect(conIman({ x: -10, y: 600 }, PRINCIPAL)).toEqual({ x: 0, y: 600 });
    expect(conIman({ x: -10, y: 300 }, PRINCIPAL)).toEqual({ x: 0, y: 300 });
    expect(conIman({ x: -150, y: 600 }, PRINCIPAL)).toEqual({ x: -150, y: 600 });
  });

  it("eje por eje: cruza en x y en y se queda en el borde del portatil", () => {
    expect(conIman({ x: 2070, y: 1085 }, PRINCIPAL)).toEqual({ x: 2070, y: 1080 });
    expect(conIman({ x: -100, y: 1090 }, PRINCIPAL)).toEqual({ x: -100, y: 1080 });
  });

  it("desde el portatil al 125 %, hacia arriba: 12,5 fisicos se quedan, 190 cruzan", () => {
    const arriba = puntoAFisico({ x: 400, y: -10 }, PORTATIL, 1.25);
    expect(conIman(arriba, PORTATIL)).toEqual({ x: 500, y: 1080 });
    expect(conIman({ x: 500, y: 890 }, PORTATIL)).toEqual({ x: 500, y: 890 });
  });

  it("y nunca fuera del escritorio: a la derecha de la principal no hay nada", () => {
    const escritorio = cajaDe(LAS_SUYAS)!;
    expect(cursorDelGesto({ x: 2500, y: 500 }, PRINCIPAL, escritorio)).toEqual({ x: 1920, y: 500 });
    expect(cursorDelGesto({ x: -2500, y: -40 }, PRINCIPAL, escritorio)).toEqual({ x: -1920, y: 0 });
  });

  it("al mover: asomar 10 px a una pantalla que no tocaba lo aparta; 30, no", () => {
    // Un recorte que coge la principal y la izquierda, bajado 10 px dentro del portatil.
    const r = { x: -300, y: 1000, width: 600, height: 90 };
    expect(apartarDeOtras(r, [1, 2], LAS_SUYAS)).toEqual({ ...r, y: 990 });
    const lejos = { ...r, y: 1020 };
    expect(apartarDeOtras(lejos, [1, 2], LAS_SUYAS)).toBe(lejos);
    // Y si no asoma a nada, es el mismo objeto.
    const quieto = { x: 100, y: 100, width: 200, height: 200 };
    expect(apartarDeOtras(quieto, [1], LAS_SUYAS)).toBe(quieto);
  });
});

describe("lo que se graba es de una sola pantalla", () => {
  it("si cruza, el trozo de la que manda; si no, tal cual", () => {
    const derecha = monitor(2, 1920, 0, 1920, 1080);
    const dos = [PRINCIPAL, derecha];
    expect(enUnaPantalla({ x: 200, y: 300, width: 1870, height: 200 }, dos)).toEqual({ x: 200, y: 300, width: 1720, height: 200 });
    expect(enUnaPantalla({ x: 1800, y: 300, width: 720, height: 200 }, dos)).toEqual({ x: 1920, y: 300, width: 600, height: 200 });
    const una = { x: 2000, y: 300, width: 100, height: 100 };
    expect(enUnaPantalla(una, dos)).toBe(una);
  });
});

describe("cuando se avisa a las demas", () => {
  const r = { x: 200, y: 300, width: 300, height: 200 };

  it("al empezar y al acabar un gesto, ya", () => {
    expect(queAvisar({ rect: r, fase: "dibujando", cursor: null }, SIN_AVISO, false)).toBe("ya");
    const enMedio = { rect: r, fase: "dibujando" as const, cursor: { x: 1, y: 1 }, tocaba: false };
    expect(queAvisar({ rect: r, fase: "quieta", cursor: null }, enMedio, false)).toBe("ya");
  });

  it("en medio, solo si toca otra pantalla o la tocaba en el ultimo aviso", () => {
    const antes = { rect: r, fase: "dibujando" as const, cursor: { x: 500, y: 500 }, tocaba: false };
    const ahora = { rect: { ...r, width: 400 }, fase: "dibujando" as const, cursor: { x: 600, y: 500 } };
    expect(queAvisar(ahora, antes, false)).toBe("nada");
    expect(queAvisar(ahora, antes, true)).toBe("luego");
    expect(queAvisar(ahora, { ...antes, tocaba: true }, false)).toBe("luego");
  });

  it("sin gesto, solo si el recorte ha cambiado: lo que acaba de llegar de otra no se repite", () => {
    const llegado = { rect: r, fase: "quieta" as const, cursor: null, tocaba: false };
    expect(queAvisar({ rect: { ...r }, fase: "quieta", cursor: null }, llegado, true)).toBe("nada");
    expect(queAvisar({ rect: { ...r, x: 201 }, fase: "quieta", cursor: null }, llegado, false)).toBe("ya");
    expect(queAvisar({ rect: null, fase: "quieta", cursor: null }, SIN_AVISO, false)).toBe("nada");
  });
});
