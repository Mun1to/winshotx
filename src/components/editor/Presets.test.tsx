/**
 * Los estilos guardados (S9, D2): lo que importa es lo que se manda a Rust al guardar,
 * renombrar y borrar, y lo que le llega al panel al elegir uno.
 */
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Presets } from "./Presets";
import { aplicarIdioma } from "../../lib/i18n";
import { ESTILO_DE_FABRICA, type Estilo } from "../../lib/presets";
import { emite, llamadas, responde } from "../../test/preparar";

const TUTORIAL: Estilo = { ...ESTILO_DE_FABRICA, zoom: 2, aire: 0.05, fondo: "menta" };
const REDES: Estilo = { ...ESTILO_DE_FABRICA, puntero: 1.5, aros: true };

function pintar(estilo: Estilo = ESTILO_DE_FABRICA, presets = [{ nombre: "Tutorial", estilo: TUTORIAL }, { nombre: "Redes", estilo: REDES }]) {
  responde("get_settings", { presets });
  responde("presets_guardar", null);
  const onAplicar = vi.fn();
  render(<Presets estilo={estilo} onAplicar={onAplicar} />);
  return onAplicar;
}

const guardados = () =>
  llamadas.filter((l) => l.comando === "presets_guardar").map((l) => (l.args as { presets: { nombre: string; estilo: Estilo }[] }).presets);

beforeEach(() => {
  aplicarIdioma("es");
  llamadas.length = 0;
});

describe("la lista", () => {
  it("dice el estilo que hay puesto, o «Sin guardar»", async () => {
    pintar(TUTORIAL);
    expect(await screen.findByRole("button", { name: "Tutorial" })).toBeInTheDocument();
  });

  it("con lo de ahora sin guardar, lo dice", async () => {
    pintar({ ...ESTILO_DE_FABRICA, zoom: 3 });
    expect(await screen.findByRole("button", { name: "Sin guardar" })).toBeInTheDocument();
  });

  it("elegir uno se lo pone al panel y cierra la lista", async () => {
    const onAplicar = pintar();
    fireEvent.click(await screen.findByRole("button", { name: "Sin guardar" }));
    fireEvent.click(screen.getByRole("button", { name: "Redes" }));
    expect(onAplicar).toHaveBeenCalledWith(REDES);
    expect(screen.queryByRole("list", { name: "Estilos guardados" })).toBeNull();
  });

  it("sin ninguno guardado, solo sale «Guardar estilo como…»", async () => {
    pintar(ESTILO_DE_FABRICA, []);
    await waitFor(() => expect(llamadas.some((l) => l.comando === "get_settings")).toBe(true));
    expect(screen.queryByRole("button", { name: "Sin guardar" })).toBeNull();
    expect(screen.getByRole("button", { name: "Guardar estilo como…" })).toBeInTheDocument();
  });

  it("si otra ventana cambia la lista, la enseña", async () => {
    pintar(ESTILO_DE_FABRICA, []);
    await waitFor(() => expect(llamadas.some((l) => l.comando === "get_settings")).toBe(true));
    act(() => emite("winshotx://presets", [{ nombre: "Nuevo", estilo: {} }]));
    expect(await screen.findByRole("button", { name: "Nuevo" })).toBeInTheDocument();
  });
});

describe("guardar como", () => {
  it("guarda lo de ahora con su nombre, al final de la lista", async () => {
    const ahora: Estilo = { ...ESTILO_DE_FABRICA, estela: 0.4, teclas: true };
    pintar(ahora);
    fireEvent.click(await screen.findByRole("button", { name: "Guardar estilo como…" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Nombre del estilo" }), { target: { value: "  Demo  " } });
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Nombre del estilo" }), { key: "Enter" });
    await waitFor(() => expect(guardados()).toHaveLength(1));
    const lista = guardados()[0];
    expect(lista.map((p) => p.nombre)).toEqual(["Tutorial", "Redes", "Demo"]);
    expect(lista[2].estilo).toEqual(ahora);
  });

  it("con un nombre que ya está, avisa y lo reemplaza en su sitio", async () => {
    pintar(REDES);
    fireEvent.click(await screen.findByRole("button", { name: "Guardar estilo como…" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Nombre del estilo" }), { target: { value: "tutorial" } });
    expect(screen.getByText("Ya hay uno con ese nombre: se cambia por lo de ahora")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Reemplazar" }));
    await waitFor(() => expect(guardados()).toHaveLength(1));
    expect(guardados()[0].map((p) => p.nombre)).toEqual(["tutorial", "Redes"]);
    expect(guardados()[0][0].estilo).toEqual(REDES);
  });

  it("Escape cancela sin guardar y sin llegarle al editor", async () => {
    pintar();
    fireEvent.click(await screen.findByRole("button", { name: "Guardar estilo como…" }));
    const fuera = vi.fn();
    window.addEventListener("keydown", fuera);
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Nombre del estilo" }), { key: "Escape" });
    window.removeEventListener("keydown", fuera);
    expect(fuera).not.toHaveBeenCalled();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(guardados()).toHaveLength(0);
  });
});

describe("renombrar y borrar", () => {
  it("renombra con Enter", async () => {
    pintar();
    fireEvent.click(await screen.findByRole("button", { name: "Sin guardar" }));
    fireEvent.click(screen.getByRole("button", { name: "Cambiar el nombre de «Redes»" }));
    const campo = screen.getByRole("textbox", { name: "Nombre nuevo de «Redes»" });
    fireEvent.change(campo, { target: { value: "Instagram" } });
    fireEvent.keyDown(campo, { key: "Enter" });
    await waitFor(() => expect(guardados()).toHaveLength(1));
    expect(guardados()[0].map((p) => p.nombre)).toEqual(["Tutorial", "Instagram"]);
  });

  it("no deja renombrar a un nombre que ya tiene otro", async () => {
    pintar();
    fireEvent.click(await screen.findByRole("button", { name: "Sin guardar" }));
    fireEvent.click(screen.getByRole("button", { name: "Cambiar el nombre de «Redes»" }));
    const campo = screen.getByRole("textbox", { name: "Nombre nuevo de «Redes»" });
    fireEvent.change(campo, { target: { value: "TUTORIAL" } });
    fireEvent.keyDown(campo, { key: "Enter" });
    expect(screen.getByText("Ya hay otro estilo con ese nombre")).toBeInTheDocument();
    expect(guardados()).toHaveLength(0);
  });

  it("borrar pide un segundo clic", async () => {
    pintar();
    fireEvent.click(await screen.findByRole("button", { name: "Sin guardar" }));
    fireEvent.click(screen.getByRole("button", { name: "Borrar «Tutorial»" }));
    expect(guardados()).toHaveLength(0);
    expect(screen.getByText("Pulsa otra vez para borrar «Tutorial»")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Borrar «Tutorial»" }));
    await waitFor(() => expect(guardados()).toHaveLength(1));
    expect(guardados()[0].map((p) => p.nombre)).toEqual(["Redes"]);
  });
});

it("en inglés no queda nada en español", async () => {
  aplicarIdioma("en");
  pintar();
  fireEvent.click(await screen.findByRole("button", { name: "Not saved" }));
  fireEvent.click(screen.getByRole("button", { name: "Save style as…" }));
  const textos = [
    document.body.textContent ?? "",
    ...[...document.querySelectorAll("[aria-label],[title],[placeholder]")].flatMap((el) => [
      el.getAttribute("aria-label") ?? "",
      el.getAttribute("title") ?? "",
      el.getAttribute("placeholder") ?? "",
    ]),
  ].join(" ");
  for (const espanol of ["Sin guardar", "Guardar", "Nombre del estilo", "Cambiar el nombre", "Borrar", "Cancelar"]) {
    expect(textos).not.toContain(espanol);
  }
});
