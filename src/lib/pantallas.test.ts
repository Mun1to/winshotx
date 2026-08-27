/**
 * Las coordenadas entre pantallas, que es donde este proyecto se ha equivocado tres veces.
 *
 * Las tres han sido lo mismo: dar por hecho que una pantalla empieza en un numero positivo.
 * En un escritorio con varios monitores, el que esta a la izquierda o encima del principal
 * empieza en negativo, asi que casi todas las pruebas de aqui usan un monitor vertical a la
 * izquierda con x = -1200.
 */
import { describe, expect, it } from "vitest";
import {
  aPantalla,
  aVirtual,
  esDeEstaPantalla,
  ventanaBajoElPunto,
  ventanasDeEstaPantalla,
} from "./pantallas";

/** El principal, 1920 x 1200, empezando en el origen. */
const PRINCIPAL = { x: 0, y: 0, width: 1920, height: 1200 };
/** El vertical de la izquierda, girado: empieza en x negativo, que es la trampa. */
const IZQUIERDA = { x: -1200, y: 0, width: 1200, height: 1920 };

describe("ir y volver entre la pantalla y el escritorio virtual", () => {
  it("suma el origen del monitor al pasar a coordenadas del escritorio", () => {
    const dentro = { x: 100, y: 50, width: 400, height: 300 };
    expect(aVirtual(dentro, PRINCIPAL, 1)).toEqual({ x: 100, y: 50, width: 400, height: 300 });
  });

  it("en el monitor de la izquierda salen coordenadas negativas, y esta bien", () => {
    const dentro = { x: 100, y: 50, width: 400, height: 300 };
    expect(aVirtual(dentro, IZQUIERDA, 1)).toEqual({
      x: -1100,
      y: 50,
      width: 400,
      height: 300,
    });
  });

  it("multiplica por la escala cuando Windows tiene la pantalla ampliada", () => {
    // Al 150 %, un pixel CSS son 1,5 fisicos.
    const dentro = { x: 100, y: 100, width: 200, height: 200 };
    expect(aVirtual(dentro, PRINCIPAL, 1.5)).toEqual({
      x: 150,
      y: 150,
      width: 300,
      height: 300,
    });
  });

  it("nunca deja un lado por debajo de dos pixeles", () => {
    const resbalon = { x: 10, y: 10, width: 0, height: 1 };
    const salida = aVirtual(resbalon, PRINCIPAL, 1);
    expect(salida.width).toBe(2);
    expect(salida.height).toBe(2);
  });

  it("la vuelta deshace la ida, tambien en el monitor de coordenadas negativas", () => {
    const dentro = { x: 300, y: 200, width: 500, height: 400 };
    const fuera = aVirtual(dentro, IZQUIERDA, 1);
    expect(aPantalla(fuera, IZQUIERDA, 1)).toEqual(dentro);
  });
});

describe("de que pantalla es una region", () => {
  it("una region entera dentro del principal es del principal", () => {
    const region = { x: 100, y: 100, width: 400, height: 300 };
    expect(esDeEstaPantalla(region, PRINCIPAL)).toBe(true);
    expect(esDeEstaPantalla(region, IZQUIERDA)).toBe(false);
  });

  it("una region en coordenadas negativas es del monitor de la izquierda", () => {
    const region = { x: -1000, y: 400, width: 300, height: 200 };
    expect(esDeEstaPantalla(region, IZQUIERDA)).toBe(true);
    expect(esDeEstaPantalla(region, PRINCIPAL)).toBe(false);
  });

  it("una region a caballo va a la pantalla donde este su centro, y solo a esa", () => {
    // Empieza en el monitor de la izquierda y termina en el principal. El centro cae
    // en x = 0, o sea en el principal.
    const region = { x: -200, y: 300, width: 400, height: 200 };
    expect(esDeEstaPantalla(region, PRINCIPAL)).toBe(true);
    expect(esDeEstaPantalla(region, IZQUIERDA)).toBe(false);
  });

  it("el borde derecho de una pantalla ya es de la siguiente", () => {
    // El pixel 1920 es el primero del monitor de al lado, no el ultimo de este.
    const pegada = { x: 1918, y: 0, width: 4, height: 4 };
    expect(esDeEstaPantalla(pegada, PRINCIPAL)).toBe(false);
  });
});

describe("las ventanas del sistema que se ven en esta pantalla", () => {
  const VENTANAS = [
    { title: "en el principal", rect: { x: 200, y: 100, width: 800, height: 600 } },
    { title: "en el de la izquierda", rect: { x: -900, y: 200, width: 600, height: 900 } },
    { title: "invisible de un pixel", rect: { x: 300, y: 300, width: 1, height: 1 } },
  ];

  it("se queda con las que asoman por aqui y las trae a coordenadas de esta pantalla", () => {
    const vistas = ventanasDeEstaPantalla(VENTANAS, PRINCIPAL, 1, 1920, 1200);
    expect(vistas.map((v) => v.title)).toEqual(["en el principal"]);
    expect(vistas[0].rect).toEqual({ x: 200, y: 100, width: 800, height: 600 });
  });

  it("en el monitor de la izquierda, la ventana negativa sale con coordenadas positivas", () => {
    const vistas = ventanasDeEstaPantalla(VENTANAS, IZQUIERDA, 1, 1200, 1920);
    expect(vistas.map((v) => v.title)).toEqual(["en el de la izquierda"]);
    // -900 esta a 300 pixeles del borde izquierdo de un monitor que empieza en -1200.
    expect(vistas[0].rect.x).toBe(300);
  });

  it("descarta las ventanas invisibles de uno o dos pixeles", () => {
    const vistas = ventanasDeEstaPantalla(VENTANAS, PRINCIPAL, 1, 1920, 1200);
    expect(vistas.some((v) => v.title === "invisible de un pixel")).toBe(false);
  });

  it("una ventana que asoma a medias por el borde se queda", () => {
    const asoma = [{ title: "a medias", rect: { x: -100, y: 100, width: 400, height: 300 } }];
    expect(ventanasDeEstaPantalla(asoma, PRINCIPAL, 1, 1920, 1200)).toHaveLength(1);
  });
});

describe("que ventana coge el punto", () => {
  const APILADAS = [
    { title: "la de detras", rect: { x: 0, y: 0, width: 1000, height: 800 } },
    { title: "la de delante", rect: { x: 100, y: 100, width: 300, height: 200 } },
  ];

  it("coge la mas pequenna, que es la que esta encima", () => {
    expect(ventanaBajoElPunto(APILADAS, 200, 150)?.title).toBe("la de delante");
  });

  it("fuera de la pequenna coge la grande", () => {
    expect(ventanaBajoElPunto(APILADAS, 700, 600)?.title).toBe("la de detras");
  });

  it("donde no hay ninguna devuelve null en vez de inventarse una", () => {
    expect(ventanaBajoElPunto(APILADAS, 1500, 900)).toBeNull();
  });

  it("el borde de abajo y el de la derecha ya estan fuera de la ventana", () => {
    // 100 + 300 = 400: el pixel 400 es el primero de fuera.
    expect(ventanaBajoElPunto([APILADAS[1]], 400, 150)).toBeNull();
    expect(ventanaBajoElPunto([APILADAS[1]], 399, 150)?.title).toBe("la de delante");
  });
});
