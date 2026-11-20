import { describe, expect, it } from "vitest";
import { fraseDeComoParar } from "./comoParar";
import { EN } from "./textos-en";
import { ZH } from "./textos-zh";
import type { ComoParar } from "./types";

const como = (atajo: string | null, bandeja: boolean, escritorio: boolean): ComoParar => ({
  atajo,
  bandeja,
  escritorio,
  orden: "winshotx --record",
});

describe("la frase de cómo se para una grabación", () => {
  it("nombra la tecla y la bandeja cuando las hay", () => {
    expect(fraseDeComoParar(como("Ctrl+Shift+5", true, false))).toBe(
      "Para parar la grabación, pulsa {atajo} o usa el icono de winshotx en la bandeja.",
    );
    expect(fraseDeComoParar(como("Ctrl+Shift+5", false, false))).toBe("Para parar la grabación, pulsa {atajo}.");
  });

  // El GNOME de Fedora: sin bandeja. Nombrarla sería mandar a la persona a buscar un icono
  // que no está.
  it("sin bandeja no la nombra, y dice lo del escritorio si lo tiene", () => {
    const frase = fraseDeComoParar(como("Ctrl+Shift+5", false, true));
    expect(frase).toBe("Para parar la grabación, pulsa {atajo} o deja de compartir la pantalla desde el escritorio.");
    expect(frase).not.toContain("bandeja");
  });

  it("sin tecla ni bandeja siempre queda la orden", () => {
    expect(fraseDeComoParar(como(null, false, false))).toBe("Para parar la grabación, ejecuta {orden}.");
    expect(fraseDeComoParar(como(null, false, true))).toContain("{orden}");
    // Con bandeja no hace falta mandar a nadie a una consola.
    expect(fraseDeComoParar(como(null, true, false))).not.toContain("{orden}");
    expect(fraseDeComoParar(como(null, true, true))).not.toContain("{orden}");
  });

  it("nunca nombra una tecla que no hay", () => {
    for (const bandeja of [true, false]) {
      for (const escritorio of [true, false]) {
        expect(fraseDeComoParar(como(null, bandeja, escritorio))).not.toContain("{atajo}");
        expect(fraseDeComoParar(como("F9", bandeja, escritorio))).toContain("{atajo}");
      }
    }
  });

  // La clave del diccionario es la frase: una que no esté saldría en español con la app en
  // inglés. Y la traducción tiene que llevar las mismas marcas, o la tecla no se pintaría.
  it("cada caso está en inglés y en chino, con las mismas marcas", () => {
    const marcas = (texto: string) => (texto.match(/\{\w+\}/g) ?? []).sort();
    const frases = new Set<string>();
    for (const atajo of ["Ctrl+Shift+5", null]) {
      for (const bandeja of [true, false]) {
        for (const escritorio of [true, false]) {
          frases.add(fraseDeComoParar(como(atajo, bandeja, escritorio)));
        }
      }
    }
    expect(frases.size).toBe(8);
    for (const frase of frases) {
      for (const diccionario of [EN, ZH]) {
        const traducida = diccionario[frase];
        expect(traducida, frase).toBeTruthy();
        expect(marcas(traducida), frase).toEqual(marcas(frase));
      }
    }
  });
});
