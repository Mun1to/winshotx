/**
 * El título de la ventana principal, que sale en la barra del escritorio y al pasar con Alt+Tab.
 *
 * Lo demás de la bienvenida y los ajustes se prueba en lo suyo: aquí solo importa que el título
 * hable el idioma de la app, también si se cambia con la ventana abierta.
 */
import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { aplicarIdioma } from "../lib/i18n";
import { responde } from "../test/preparar";

const titulos = vi.hoisted(() => [] as string[]);

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setTitle: (titulo: string) => {
      titulos.push(titulo);
      return Promise.resolve();
    },
  }),
}));
vi.mock("../lib/ventanaFija", () => ({ useVentanaFija: () => {} }));
vi.mock("./settings/SettingsApp", () => ({ SettingsApp: () => null }));
vi.mock("./welcome/WelcomeApp", () => ({ WelcomeApp: () => null }));

async function abrir(language: string, onboarded: boolean) {
  responde("get_settings", { language, onboarded, theme: "system" });
  render(<App />);
  await waitFor(() => expect(titulos.length).toBeGreaterThan(0));
}

const ultimo = () => titulos[titulos.length - 1];

beforeEach(() => {
  titulos.length = 0;
  aplicarIdioma("es");
});

describe("el título de la ventana", () => {
  it("la bienvenida se titula en inglés con la app en inglés", async () => {
    await abrir("en", false);
    expect(ultimo()).toBe("winshotx · Welcome");
  });

  it("los ajustes también", async () => {
    await abrir("en", true);
    expect(ultimo()).toBe("winshotx · Settings");
  });

  it("y en chino, en chino", async () => {
    await abrir("zh", false);
    expect(ultimo()).toBe("winshotx · 欢迎");
  });

  it("en español sigue en español", async () => {
    await abrir("es", true);
    expect(ultimo()).toBe("winshotx · Ajustes");
  });

  it("cambia en cuanto se cambia el idioma, sin cerrar la ventana", async () => {
    await abrir("es", true);
    act(() => aplicarIdioma("en"));
    await waitFor(() => expect(ultimo()).toBe("winshotx · Settings"));
  });
});
