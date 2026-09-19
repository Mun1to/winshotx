/**
 * Los dos idiomas.
 *
 * Aqui se comprueban la mecanica (`t`, los marcadores, el idioma que se hereda de Windows)
 * y la salud del catalogo. Que una PANTALLA salga entera en ingles se comprueba en la
 * prueba de esa pantalla, no aqui: una traduccion puede estar en el diccionario y no
 * llegar nunca a la pantalla porque el componente escribio la frase a pelo, que es
 * justo el fallo que este archivo no puede ver y el de al lado si.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { aplicarIdioma, idiomaActivo, t } from "./i18n";
import { EN } from "./textos-en";

beforeEach(() => aplicarIdioma("es"));

describe("t, el traductor", () => {
  it("en espannol devuelve la clave tal cual, porque la clave ES la frase", () => {
    expect(t("Al pulsar el atajo")).toBe("Al pulsar el atajo");
  });

  it("en ingles devuelve lo que dice el catalogo", () => {
    aplicarIdioma("en");
    expect(t("Al pulsar el atajo")).toBe("When you press the shortcut");
  });

  it("lo que falta en el catalogo sale en espannol, no en blanco", () => {
    aplicarIdioma("en");
    expect(t("Una frase que nadie ha traducido todavia")).toBe(
      "Una frase que nadie ha traducido todavia",
    );
  });

  it("cambia los marcadores por su valor", () => {
    expect(t("Paso {n} de {total}", { n: 2, total: 4 })).toBe("Paso 2 de 4");
  });

  it("cambia el marcador aunque salga dos veces", () => {
    expect(t("{n} y {n}", { n: 7 })).toBe("7 y 7");
  });

  it("los marcadores tambien funcionan sobre la frase ya traducida", () => {
    aplicarIdioma("en");
    expect(t("Fotograma {actual} de {total}", { actual: 3, total: 82 })).toBe("Frame 3 of 82");
  });

  it("un marcador que no se le pasa se queda escrito, para que se vea el olvido", () => {
    expect(t("Paso {n} de {total}", { n: 1 })).toBe("Paso 1 de {total}");
  });
});

describe("de donde sale el idioma", () => {
  it("«sistema» sigue al navegador de la ventana, que trae el de Windows", () => {
    aplicarIdioma("sistema");
    // happy-dom dice en-US, asi que el sistema resuelve a ingles.
    expect(idiomaActivo()).toBe("en");
  });

  it("elegir un idioma manda sobre el del sistema", () => {
    aplicarIdioma("es");
    expect(idiomaActivo()).toBe("es");
  });

  it("deja el idioma escrito en el <html>, para que el navegador parta bien las palabras", () => {
    aplicarIdioma("en");
    expect(document.documentElement.lang).toBe("en");
    aplicarIdioma("es");
    expect(document.documentElement.lang).toBe("es");
  });

  it("lo recuerda para la proxima ventana que se abra", () => {
    aplicarIdioma("en");
    expect(localStorage.getItem("winshotx.idioma")).toBe("en");
  });
});

/**
 * Todo el codigo de winshotx: la interfaz y tambien Rust.
 *
 * Rust entra porque los mensajes de error nacen alli, escritos en espannol, y el frontend
 * los pasa por `t` al pintarlos. Sin mirar `src-tauri/`, la prueba de claves huerfanas
 * daria por muerta cada traduccion de un error del backend.
 *
 * Se lee con `import.meta.glob` de Vite y no con `node:fs` a proposito: el codigo de
 * `src/` corre dentro de una ventana, donde no hay sistema de archivos, y el tsconfig lo
 * prohibe justamente para que nadie importe medio Node ahi sin darse cuenta.
 */
function codigoDeLaApp(): string {
  const interfaz = import.meta.glob("/src/**/*.{ts,tsx}", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>;
  const backend = import.meta.glob("/src-tauri/src/**/*.rs", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>;
  return [
    ...Object.entries(interfaz)
      .filter(([ruta]) => !/\.test\.tsx?$/.test(ruta) && !ruta.includes("textos-en"))
      .map(([, texto]) => texto),
    ...Object.values(backend),
  ].join("\n");
}

describe("la salud del catalogo ingles", () => {
  it("no tiene claves que ya no use nadie", () => {
    // Cambiar una palabra en espannol deja su clave huerfana y la frase sin traducir, sin
    // que nada se rompa. Esta prueba es la unica forma de enterarse.
    const codigo = codigoDeLaApp();
    const huerfanas = Object.keys(EN).filter((clave) => !codigo.includes(clave));
    expect(huerfanas).toEqual([]);
  });

  it("ninguna traduccion se dejo la frase en espannol", () => {
    const iguales = Object.entries(EN)
      .filter(([es, en]) => es === en)
      // Estas se escriben igual en los dos idiomas y no son un olvido.
      .filter(([es]) => !["GIF", "MP4", "PNG", "FFmpeg", "winshotx", "Editor"].includes(es));
    expect(iguales).toEqual([]);
  });

  it("cada traduccion conserva los marcadores de su original", () => {
    // Perder un {n} al traducir deja un hueco en la frase inglesa que nadie ve hasta que
    // le sale a un usuario.
    const marcadores = (frase: string) => (frase.match(/\{[a-z]+\}/g) ?? []).sort();
    const rotas = Object.entries(EN).filter(
      ([es, en]) => marcadores(es).join() !== marcadores(en).join(),
    );
    expect(rotas).toEqual([]);
  });
});
