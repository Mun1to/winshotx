/**
 * El buscador de los ajustes (S36): que encuentre en los tres idiomas, sin tildes, con varias
 * palabras en cualquier orden, y que ponga primero lo que lo lleva en el nombre.
 */
import { describe, expect, it } from "vitest";
import { EN } from "../../lib/textos-en";
import { ZH } from "../../lib/textos-zh";
import { APARTADOS, BUSCABLES, buscarAjustes, type AjusteBuscable } from "./mapaDeAjustes";

const nombres = (b: AjusteBuscable) => [b.nombre, EN[b.nombre] ?? "", ZH[b.nombre] ?? ""];
const ids = (consulta: string, hay?: (id: string) => boolean) => buscarAjustes(consulta, nombres, hay).map((b) => b.id);

describe("buscar un ajuste", () => {
  it("sin escribir nada no ofrece nada", () => {
    expect(ids("")).toEqual([]);
    expect(ids("   ")).toEqual([]);
  });

  it("encuentra por el nombre, sin tildes y sin mayúsculas", () => {
    expect(ids("camara")).toContain("graCamara");
    expect(ids("CÁMARA")).toContain("graCamara");
  });

  it("primero lo que lo lleva en el nombre", () => {
    const sonido = ids("sonido");
    // «Sonido de obturador» y «Sonido al empezar» se llaman así; el audio del sistema solo
    // lo lleva en sus palabras.
    expect(sonido.indexOf("desSonido")).toBeLessThan(sonido.indexOf("graAudio"));
    expect(sonido.indexOf("graSonido")).toBeLessThan(sonido.indexOf("graAudio"));
  });

  it("en inglés y en chino, aunque la app esté en otro idioma", () => {
    expect(ids("folder")).toContain("appCarpeta");
    expect(ids("shutter")).toContain("desSonido");
    expect(ids("文件夹")).toContain("appCarpeta");
    expect(ids("摄像头")).toContain("graCamara");
  });

  it("con varias palabras, todas tienen que estar, en cualquier orden", () => {
    expect(ids("cursor incluir")).toEqual(["capCursor"]);
    expect(ids("cursor carpeta")).toEqual([]);
  });

  it("no ofrece una fila que hoy no sale", () => {
    expect(ids("pantalla")).toContain("ultPantalla");
    expect(ids("pantalla", (id) => id !== "ultPantalla")).not.toContain("ultPantalla");
  });

  it("cada ajuste es de un apartado que existe, y ningún id se repite", () => {
    const apartados = new Set(APARTADOS.map((a) => a.id));
    for (const b of BUSCABLES) expect(apartados.has(b.apartado), b.id).toBe(true);
    expect(new Set(BUSCABLES.map((b) => b.id)).size).toBe(BUSCABLES.length);
  });

  it("cada nombre está traducido en los dos diccionarios, salvo el que se escribe igual", () => {
    const faltan = BUSCABLES.filter((b) => b.nombre !== "winshotx {version}" && (!EN[b.nombre] || !ZH[b.nombre]));
    expect(faltan.map((b) => b.nombre)).toEqual([]);
  });
});
