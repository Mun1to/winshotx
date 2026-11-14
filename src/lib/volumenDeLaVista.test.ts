/**
 * S31: el volumen de la vista previa del editor. Lo que se oye al mirar, nunca lo que se guarda.
 */
import { describe, expect, it } from "vitest";
import {
  NIVEL_DE_FABRICA,
  alternar,
  conNivel,
  escalon,
  flechaDeOtro,
  guardar,
  leer,
  loQueSuena,
  mover,
} from "./volumenDeLaVista";

describe("lo que se recuerda", () => {
  it("sin nada guardado nace a la mitad, nunca a tope", () => {
    // Regla de la casa: lo que suena nace bajo («tú añades un vídeo y de repente sale todo volumen»).
    expect(NIVEL_DE_FABRICA).toBeLessThan(1);
    expect(leer()).toEqual({ nivel: NIVEL_DE_FABRICA, callada: false });
  });

  it("se guarda y se vuelve a leer igual", () => {
    guardar({ nivel: 0.3, callada: true });
    expect(leer()).toEqual({ nivel: 0.3, callada: true });
  });

  it("quien la dejó callada con el botón de antes la sigue teniendo callada", () => {
    // La 0.4.8 solo guardaba el silencio, en esta misma clave.
    localStorage.setItem("winshotx.vistaCallada", "1");
    expect(leer()).toEqual({ nivel: NIVEL_DE_FABRICA, callada: true });
  });

  it("un valor raro en el almacén no rompe nada: se queda entre 0 y 1", () => {
    localStorage.setItem("winshotx.volumenVista", "7");
    expect(leer().nivel).toBe(1);
    localStorage.setItem("winshotx.volumenVista", "-2");
    expect(leer().nivel).toBe(0);
    localStorage.setItem("winshotx.volumenVista", "hola");
    expect(leer().nivel).toBe(NIVEL_DE_FABRICA);
  });
});

describe("la M y el altavoz", () => {
  it("callar se guarda el nivel, y volver lo recupera", () => {
    const callada = alternar({ nivel: 0.3, callada: false });
    expect(loQueSuena(callada)).toBe(0);
    expect(alternar(callada)).toEqual({ nivel: 0.3, callada: false });
  });

  it("con el deslizador en 0, volver a oír no se queda mudo: vuelve a la mitad", () => {
    expect(alternar({ nivel: 0, callada: false })).toEqual({ nivel: NIVEL_DE_FABRICA, callada: false });
    expect(alternar({ nivel: 0, callada: true })).toEqual({ nivel: NIVEL_DE_FABRICA, callada: false });
  });
});

describe("el deslizador y las flechas", () => {
  it("mover el deslizador con la vista callada la vuelve a oír, a lo que marca", () => {
    expect(conNivel({ nivel: 0.8, callada: true }, 0.2)).toEqual({ nivel: 0.2, callada: false });
  });

  it("las flechas suben y bajan de diez en diez, sin decimales sueltos", () => {
    expect(mover({ nivel: 0.2, callada: false }, 1)).toEqual({ nivel: 0.3, callada: false });
    expect(mover({ nivel: 0.3, callada: false }, -1)).toEqual({ nivel: 0.2, callada: false });
  });

  it("y no se salen de 0 a 100", () => {
    expect(mover({ nivel: 0.95, callada: false }, 1).nivel).toBe(1);
    expect(mover({ nivel: 0.05, callada: false }, -1).nivel).toBe(0);
  });

  it("subir con la vista callada sube desde lo que se oye, que es nada", () => {
    // Igual que el deslizador, que callada marca 0.
    expect(mover({ nivel: 0.8, callada: true }, 1)).toEqual({ nivel: 0.1, callada: false });
  });

  it("y bajar con la vista callada no toca nada: la M sigue volviendo a lo de antes", () => {
    const callada = { nivel: 0.8, callada: true };
    expect(mover(callada, -1)).toBe(callada);
  });
});

describe("lo que enseña el altavoz", () => {
  it("sin sonido, bajo o alto", () => {
    expect(escalon({ nivel: 0.7, callada: true })).toBe("sin");
    expect(escalon({ nivel: 0, callada: false })).toBe("sin");
    expect(escalon({ nivel: 0.5, callada: false })).toBe("bajo");
    expect(escalon({ nivel: 0.51, callada: false })).toBe("alto");
  });
});

describe("de quién es una flecha", () => {
  it("en un campo, un asa de la línea de tiempo o un texto que se edita, es suya", () => {
    document.body.innerHTML = `
      <textarea></textarea>
      <div role="slider" tabindex="0"><span>asa</span></div>
      <div contenteditable="true"><b>texto</b></div>
      <button>Reproducir</button>`;
    expect(flechaDeOtro(document.querySelector("textarea"))).toBe(true);
    expect(flechaDeOtro(document.querySelector("[role=slider] span"))).toBe(true);
    expect(flechaDeOtro(document.querySelector("[contenteditable] b"))).toBe(true);
  });

  it("en un botón o en ninguna parte, es del volumen", () => {
    document.body.innerHTML = "<button>Reproducir</button>";
    expect(flechaDeOtro(document.querySelector("button"))).toBe(false);
    expect(flechaDeOtro(document.body)).toBe(false);
    expect(flechaDeOtro(null)).toBe(false);
  });
});
