import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (ruta: string) => `asset://localhost/${encodeURIComponent(ruta)}`,
}));

import { urlDeMedio } from "./medios";
import general from "../../src-tauri/tauri.conf.json";
import deLinux from "../../src-tauri/tauri.linux.conf.json";

afterEach(() => {
  delete window.__WINSHOTX_MEDIOS__;
});

describe("la dirección de un vídeo o de un sonido", () => {
  it("en Windows es la de siempre, por asset:", () => {
    expect(urlDeMedio("C:\\Temp\\winshotx\\sessions\\a\\vista.mp4")).toBe(
      "asset://localhost/C%3A%5CTemp%5Cwinshotx%5Csessions%5Ca%5Cvista.mp4",
    );
  });

  it("en Linux va al servidor de 127.0.0.1 que deja Rust en la ventana, con la ruta en un solo tramo", () => {
    window.__WINSHOTX_MEDIOS__ = "http://127.0.0.1:41234/0123abcd";
    expect(urlDeMedio("/home/ana/.cache/winshotx/sessions/a b/sonido-pc.wav")).toBe(
      "http://127.0.0.1:41234/0123abcd/%2Fhome%2Fana%2F.cache%2Fwinshotx%2Fsessions%2Fa%20b%2Fsonido-pc.wav",
    );
  });

  it("si Rust no ha podido abrir el servidor, no inventa una dirección", () => {
    window.__WINSHOTX_MEDIOS__ = "";
    expect(urlDeMedio("/tmp/a.wav")).toBe("asset://localhost/%2Ftmp%2Fa.wav");
  });
});

/**
 * La ventana solo deja reproducir de donde diga su política de contenido. La de Linux es la
 * general más el servidor de 127.0.0.1, y nada más: si alguien cambia la general, que esta
 * prueba le recuerde la otra.
 */
describe("la política de contenido de Linux", () => {
  it("es la general con el servidor local añadido a lo que se reproduce", () => {
    const esperada = general.app.security.csp.replace(
      "media-src 'self' asset: http://asset.localhost blob:",
      "media-src 'self' asset: http://asset.localhost blob: http://127.0.0.1:*",
    );
    expect(esperada).not.toBe(general.app.security.csp);
    expect(deLinux.app.security.csp).toBe(esperada);
  });

  it("no abre nada más que eso: ni guiones ni conexiones a 127.0.0.1", () => {
    const partes = Object.fromEntries(
      deLinux.app.security.csp.split(";").map((trozo) => {
        const [nombre, ...valores] = trozo.trim().split(" ");
        return [nombre, valores];
      }),
    );
    expect(partes["media-src"]).toContain("http://127.0.0.1:*");
    for (const nombre of ["default-src", "connect-src", "img-src", "style-src", "script-src"]) {
      expect(partes[nombre].join(" ")).not.toContain("127.0.0.1");
    }
  });
});
