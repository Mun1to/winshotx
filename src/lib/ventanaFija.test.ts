import { createElement } from "react";
import { render, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import conf from "../../src-tauri/tauri.conf.json";
import { estirar, llamadas, ventana } from "../test/preparar";
import {
  ALTO_VENTANA,
  ANCHO_VENTANA,
  type Sitio,
  type Tamanno,
  colocar,
  esDeLaApp,
  useVentanaFija,
} from "./ventanaFija";

/**
 * La ventana de ajustes con su tamaño fijo (S40). El marco de Windows 11 (barra de título y
 * bordes) son unos 16 px de ancho y 40 de alto en físicos a 100 %; la pantalla, una de 1920 x
 * 1080 con la barra de tareas de 48 px abajo.
 */
const base: Sitio = {
  quiere: { ancho: ANCHO_VENTANA, alto: ALTO_VENTANA },
  escala: 1,
  marco: { ancho: 16, alto: 40 },
  fuera: { x: 502, y: 179, ancho: 916, alto: 763 },
  zona: { x: 0, y: 0, ancho: 1920, alto: 1032 },
};

describe("el tamaño de serie", () => {
  it("es el mismo con el que nace la ventana en tauri.conf.json", () => {
    // Si no, la ventana nace con uno y cambia al cargar la página, delante de quien la abre.
    const main = conf.app.windows.find((w) => w.label === "main");
    expect(main?.width).toBe(ANCHO_VENTANA);
    expect(main?.height).toBe(ALTO_VENTANA);
  });
});

describe("dónde y a qué tamaño se pone al abrir", () => {
  it("si ya nace a su tamaño, ni se mueve", () => {
    expect(colocar(base)).toEqual({ ancho: 900, alto: 723, x: 502, y: 179 });
  });

  it("con un tamaño guardado más grande, crece por igual hacia los cuatro lados", () => {
    const r = colocar({ ...base, quiere: { ancho: 1000, alto: 781 } });
    expect(r).toEqual({ ancho: 1000, alto: 781, x: 452, y: 150 });
  });

  it("a 125 % redondea hacia arriba, para que no salga la rueda por medio píxel", () => {
    const r = colocar({ ...base, escala: 1.25 });
    expect(r.ancho).toBe(1125);
    expect(r.alto).toBe(Math.ceil(723 * 1.25));
  });

  it("en un portátil de 1366 x 768 al 125 % se encoge a la zona útil y no se sale", () => {
    // 768 - 48 de barra de tareas = 720 físicos de zona útil; por dentro caben 720 - 40.
    const r = colocar({
      ...base,
      escala: 1.25,
      fuera: { x: 0, y: 0, ancho: 1141, alto: 892 },
      zona: { x: 0, y: 0, ancho: 1366, alto: 720 },
    });
    expect(r.alto).toBe(680);
    expect(r.y).toBe(0);
    expect(r.y + r.alto + 40).toBeLessThanOrEqual(720);
    expect(r.x + r.ancho + 16).toBeLessThanOrEqual(1366);
  });

  it("si se saldría por abajo, sube; y si no cabe ni así, se ve la barra de título", () => {
    expect(colocar({ ...base, fuera: { ...base.fuera, y: 600 } }).y).toBe(1032 - 763);
    expect(colocar({ ...base, zona: { x: 0, y: 30, ancho: 1920, alto: 500 } }).y).toBe(30);
  });

  it("en una pantalla a la izquierda de la principal, se queda en ella", () => {
    const r = colocar({
      ...base,
      fuera: { x: -1500, y: 179, ancho: 916, alto: 763 },
      zona: { x: -1920, y: 0, ancho: 1920, alto: 1032 },
    });
    expect(r.x).toBe(-1500);
    expect(r.x + r.ancho + 16).toBeLessThanOrEqual(0);
  });
});

describe("qué tamaño es de quien la usa", () => {
  const propio = { ancho: 900, alto: 681 };
  it("el que puso la app, con dos píxeles de margen por el redondeo de la escala, no", () => {
    expect(esDeLaApp({ ancho: 900, alto: 681 }, propio)).toBe(true);
    expect(esDeLaApp({ ancho: 901, alto: 679 }, propio)).toBe(true);
  });
  it("uno estirado a mano, sí", () => {
    expect(esDeLaApp({ ancho: 900, alto: 760 }, propio)).toBe(false);
    expect(esDeLaApp({ ancho: 1100, alto: 681 }, propio)).toBe(false);
  });
});

function Prueba({ guardada }: { guardada: Tamanno | null }) {
  useVentanaFija(true, guardada);
  return null;
}

const guardados = () => llamadas.filter((l) => l.comando === "guardar_ventana_ajustes").map((l) => l.args);

describe("la ventana de verdad (con el Tauri de mentira)", () => {
  it("abre a su tamaño de serie aunque naciera con otro", async () => {
    ventana.dentro = { width: 840, height: 640 };
    render(createElement(Prueba, { guardada: null }));
    await waitFor(() => expect(ventana.tamannos.at(-1)).toEqual({ width: 900, height: 723 }));
  });

  it("abre al tamaño guardado si alguien la estiró", async () => {
    render(createElement(Prueba, { guardada: { ancho: 1000, alto: 760 } }));
    await waitFor(() => expect(ventana.tamannos.at(-1)).toEqual({ width: 1000, height: 760 }));
  });

  it("a su tamaño ya, no la toca", async () => {
    render(createElement(Prueba, { guardada: null }));
    await waitFor(() => expect(ventana.oyentesTamanno.length).toBeGreaterThan(0));
    await new Promise((r) => setTimeout(r, 50));
    expect(ventana.tamannos).toEqual([]);
    expect(ventana.posiciones).toEqual([]);
  });

  it("si la estiran a mano, guarda su tamaño en lógicos, una vez al soltar", async () => {
    ventana.escala = 1.25;
    ventana.dentro = { width: 1125, height: 904 };
    render(createElement(Prueba, { guardada: null }));
    await waitFor(() => expect(ventana.oyentesTamanno.length).toBeGreaterThan(0));
    // Estirar son decenas de avisos seguidos: solo cuenta el último.
    estirar(1150, 900);
    estirar(1200, 950);
    estirar(1250, 1000);
    await waitFor(() => expect(guardados()).toEqual([{ ancho: 1000, alto: 800 }]), { timeout: 2000 });
  });

  it("su propio cambio de tamaño, minimizarla o maximizarla no se guardan", async () => {
    ventana.dentro = { width: 840, height: 640 };
    render(createElement(Prueba, { guardada: null }));
    await waitFor(() => expect(ventana.tamannos.at(-1)).toEqual({ width: 900, height: 723 }));
    estirar(900, 723);
    await new Promise((r) => setTimeout(r, 700));
    estirar(0, 0);
    await new Promise((r) => setTimeout(r, 700));
    ventana.maximizada = true;
    estirar(1920, 1009);
    await new Promise((r) => setTimeout(r, 700));
    expect(guardados()).toEqual([]);
  });
});
