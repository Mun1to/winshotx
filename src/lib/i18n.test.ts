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
import { ZH } from "./textos-zh";

beforeEach(() => aplicarIdioma("es"));

describe("t, el traductor", () => {
  it("en espannol devuelve la clave tal cual, porque la clave ES la frase", () => {
    expect(t("Al pulsar el atajo")).toBe("Al pulsar el atajo");
  });

  it("en ingles devuelve lo que dice el catalogo", () => {
    aplicarIdioma("en");
    expect(t("Al pulsar el atajo")).toBe("When you press the shortcut");
  });

  it("en chino devuelve lo que dice SU catalogo, no el ingles", () => {
    // La traduccion la aporto wang13zhe en la propuesta 2 del repositorio. La prueba
    // existe porque un tercer idioma se rompe en silencio: si `t` se quedara mirando el
    // catalogo ingles, la aplicacion saldria en ingles sin que fallara nada.
    aplicarIdioma("zh");
    expect(t("Al pulsar el atajo")).toBe("按下快捷键时");
    expect(t("Capturar región")).toBe("区域截图");
  });

  it("y lo que al chino le falta sale en espannol, igual que al ingles", () => {
    aplicarIdioma("zh");
    expect(t("Una frase que nadie ha traducido todavia")).toBe(
      "Una frase que nadie ha traducido todavia",
    );
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
    expect(t("Te quedas con {trozo} de {total}", { trozo: "0:03.2", total: "0:30.0" })).toBe(
      "Keeping 0:03.2 of 0:30.0",
    );
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
      .filter(([ruta]) => !/\.test\.tsx?$/.test(ruta) && !/textos-(en|zh)/.test(ruta))
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
      .filter(([es]) => !["GIF", "MP4", "PNG", "FFmpeg", "winshotx", "Editor", "Zoom", "Normal", "Coral"].includes(es));
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

/**
 * Lo mismo para el chino, que hasta hoy no lo miraba nadie.
 *
 * El 21 de octubre de 2026 un colaborador (`wang13zhe`) abrio un pull request contando a
 * mano que al catalogo chino le faltaban 17 frases. Tenia razon, y ademas arrastraba 5
 * claves huerfanas de frases que ya se habian reescrito en espannol. Nada de eso rompia
 * nada: lo que falta cae a espannol, asi que la aplicacion seguia funcionando y las pruebas
 * seguian verdes mientras un idioma entero se quedaba atras en silencio.
 *
 * Que esto lo tuviera que contar una persona de fuera, a mano, es el fallo que arregla este
 * bloque.
 */
describe("la salud del catalogo chino", () => {
  it("no tiene claves que ya no use nadie", () => {
    const codigo = codigoDeLaApp();
    const huerfanas = Object.keys(ZH).filter((clave) => !codigo.includes(clave));
    expect(huerfanas).toEqual([]);
  });

  it("cada traduccion conserva los marcadores de su original", () => {
    // Un {nombre} perdido al traducir sale como un hueco en la pantalla de quien lee en
    // chino, y aqui no hay nadie que lo lea antes que el.
    const marcadores = (frase: string) => (frase.match(/\{[a-z]+\}/g) ?? []).sort();
    const rotas = Object.entries(ZH).filter(
      ([es, zh]) => marcadores(es).join() !== marcadores(zh).join(),
    );
    expect(rotas).toEqual([]);
  });

  it("traduce todo lo que traduce el ingles", () => {
    // Las dos listas se mantienen a la par: si se annade una frase y solo se traduce a un
    // idioma, esto lo dice en el momento, en vez de que lo cuente alguien meses despues.
    //
    // Si algun dia se decide que el chino va por libre y puede ir por detras, esta prueba
    // es la que se quita, y las dos de arriba se quedan.
    const faltan = Object.keys(EN).filter((clave) => !(clave in ZH));
    expect(faltan).toEqual([]);
  });

  it("y no traduce nada que el ingles no tenga", () => {
    const sobran = Object.keys(ZH).filter((clave) => !(clave in EN));
    expect(sobran).toEqual([]);
  });
});
