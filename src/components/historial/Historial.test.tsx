/**
 * El historial de fotos y vídeos (S4, E6), con Rust de mentira.
 *
 * Lo que se comprueba es lo que se rompe sin verse: que cada carta pida su miniatura con su
 * ruta, que el filtro pida a Rust lo filtrado, que borrar pida dos clics y quite la carta,
 * que «Ver más» siga donde se quedó, y que una fecha se escriba como la diría una persona.
 */
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { Historial, cuando } from "./Historial";
import { aplicarIdioma, t } from "../../lib/i18n";
import { falla, llamadas, responde } from "../../test/preparar";
import type { EntradaHistorial } from "../../lib/ipc";

const CARPETA = "C:\\Users\\yo\\Pictures\\winshotx";
const HOY = Date.now();

function entrada(nombre: string, tipo: "foto" | "video", haceHoras = 1): EntradaHistorial {
  return {
    ruta: `${CARPETA}\\${nombre}`,
    nombre,
    tipo,
    bytes: 2_400_000,
    cuandoMs: HOY - haceHoras * 3_600_000,
  };
}

const TRES = [entrada("a.png", "foto"), entrada("b.mp4", "video", 30), entrada("c.png", "foto", 80)];

async function pintar(entradas = TRES, total = entradas.length) {
  responde("historial_lista", { entradas, total });
  responde("historial_miniatura", "C:\\t\\winshotx\\historial\\x.png");
  const vista = render(<Historial carpeta={CARPETA} />);
  await act(async () => {});
  return vista;
}

const pedidas = (comando: string) => llamadas.filter((l) => l.comando === comando);

beforeEach(() => aplicarIdioma("es"));

describe("la rejilla", () => {
  it("una carta por archivo, en el orden que da Rust, cada una con su miniatura", async () => {
    await pintar();
    const cartas = [...document.querySelectorAll("[data-entrada]")].map((c) => c.getAttribute("data-entrada"));
    expect(cartas).toEqual(["a.png", "b.mp4", "c.png"]);
    expect(pedidas("historial_miniatura").map((l) => l.args)).toEqual(TRES.map((e) => ({ ruta: e.ruta })));
    expect(document.querySelector("[data-entrada='a.png'] img")).toHaveAttribute(
      "src",
      "asset://C:\\t\\winshotx\\historial\\x.png",
    );
  });

  it("pide la primera página entera, sin filtro", async () => {
    await pintar();
    expect(pedidas("historial_lista")[0].args).toEqual({ filtro: null, desde: 0, cuantas: 48 });
  });

  it("el filtro pide a Rust solo los vídeos", async () => {
    await pintar();
    fireEvent.click(screen.getByRole("tab", { name: "Vídeos" }));
    await act(async () => {});
    expect(pedidas("historial_lista").at(-1)!.args).toEqual({ filtro: "video", desde: 0, cuantas: 48 });
    expect(screen.getByRole("tab", { name: "Vídeos" })).toHaveAttribute("aria-selected", "true");
  });

  it("«Ver más» sale si quedan, y pide a partir de donde se quedó", async () => {
    await pintar(TRES, 50);
    fireEvent.click(screen.getByText("Ver más"));
    await act(async () => {});
    expect(pedidas("historial_lista").at(-1)!.args).toEqual({ filtro: null, desde: 3, cuantas: 48 });
  });

  it("y no sale si ya están todas", async () => {
    await pintar();
    expect(screen.queryByText("Ver más")).toBeNull();
  });

  it("sin nada guardado lo dice, en vez de una rejilla vacía", async () => {
    await pintar([]);
    expect(screen.getByText("Todavía no has guardado nada")).toBeInTheDocument();
  });

  it("si Windows no da la miniatura, sale el dibujo de su tipo", async () => {
    falla("historial_miniatura", "Windows no da miniatura");
    responde("historial_lista", { entradas: [TRES[1]], total: 1 });
    render(<Historial carpeta={CARPETA} />);
    await act(async () => {});
    expect(document.querySelector("[data-entrada='b.mp4'] button svg")).not.toBeNull();
  });
});

describe("las acciones", () => {
  it("un clic en la carta la abre", async () => {
    await pintar();
    fireEvent.click(screen.getByLabelText("Abrir a.png"));
    expect(pedidas("historial_abrir")[0].args).toEqual({ ruta: TRES[0].ruta });
  });

  it("copiar y mostrar en la carpeta, con su ruta", async () => {
    await pintar();
    const carta = document.querySelector("[data-entrada='b.mp4']")!;
    fireEvent.click(carta.querySelector("button[aria-label='Copiar']")!);
    fireEvent.click(carta.querySelector("button[aria-label='Mostrar en la carpeta']")!);
    await act(async () => {});
    expect(pedidas("historial_copiar")[0].args).toEqual({ ruta: TRES[1].ruta });
    expect(pedidas("historial_mostrar")[0].args).toEqual({ ruta: TRES[1].ruta });
    expect(screen.getByRole("status")).toHaveTextContent("Copiado");
  });

  it("borrar pide dos clics, y al segundo la manda a la papelera y la quita", async () => {
    await pintar();
    const carta = () => document.querySelector("[data-entrada='c.png']");
    fireEvent.click(carta()!.querySelector("button[aria-label='Mandar a la papelera']")!);
    expect(pedidas("historial_borrar")).toHaveLength(0);
    fireEvent.click(screen.getByText("¿Borrar?"));
    await act(async () => {});
    expect(pedidas("historial_borrar")[0].args).toEqual({ ruta: TRES[2].ruta });
    expect(carta()).toBeNull();
    expect(screen.getByRole("status")).toHaveTextContent("Enviada a la papelera");
  });

  it("si borrar falla, la carta se queda y se dice por qué", async () => {
    falla("historial_borrar", "no se ha podido mandar a la papelera (5)");
    await pintar();
    fireEvent.click(document.querySelector("[data-entrada='c.png'] button[aria-label='Mandar a la papelera']")!);
    fireEvent.click(screen.getByText("¿Borrar?"));
    await act(async () => {});
    expect(document.querySelector("[data-entrada='c.png']")).not.toBeNull();
    expect(screen.getByRole("status")).toHaveTextContent("no se ha podido mandar a la papelera");
  });
});

describe("las fechas", () => {
  const ahora = new Date(2026, 10, 11, 12, 0);
  it("hoy y ayer con su palabra, lo demás con el día", () => {
    aplicarIdioma("es");
    expect(cuando(new Date(2026, 10, 11, 9, 5).getTime(), t, ahora)).toBe("Hoy, 09:05");
    expect(cuando(new Date(2026, 10, 10, 18, 30).getTime(), t, ahora)).toBe("Ayer, 18:30");
    expect(cuando(new Date(2026, 10, 3, 7, 0).getTime(), t, ahora)).toMatch(/^3 nov\.?, 07:00$/);
  });

  it("con el año solo si no es este", () => {
    aplicarIdioma("es");
    expect(cuando(new Date(2025, 0, 2, 7, 0).getTime(), t, ahora)).toContain("2025");
  });

  it("en inglés, en inglés", () => {
    aplicarIdioma("en");
    expect(cuando(new Date(2026, 10, 11, 9, 5).getTime(), t, ahora)).toBe("Today, 09:05");
  });
});

describe("en inglés", () => {
  it("no queda nada en español, tampoco en lo que solo lee un lector de pantalla", async () => {
    aplicarIdioma("en");
    await pintar();
    const textos = [
      document.body.textContent ?? "",
      ...[...document.querySelectorAll("[aria-label],[title]")].flatMap((el) => [
        el.getAttribute("aria-label") ?? "",
        el.getAttribute("title") ?? "",
      ]),
    ].join(" ");
    for (const espanol of ["Todo", "Fotos", "Vídeos", "Abrir la carpeta", "Copiar", "Mostrar en la carpeta", "papelera", "Hoy", "Ayer"]) {
      expect(textos).not.toContain(espanol);
    }
  });
});
