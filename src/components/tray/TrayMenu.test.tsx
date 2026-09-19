/**
 * El menú de la bandeja.
 *
 * Lo que hay que comprobar aquí no es cómo se ve, es que cada entrada acabe en la acción
 * que dice su nombre: este menú sustituye al del sistema, así que si una entrada se
 * equivoca de acción no hay ningún otro sitio desde donde arreglarlo.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { TrayMenu } from "./TrayMenu";
import { aplicarIdioma } from "../../lib/i18n";
import { emite, llamadas, responde } from "../../test/preparar";
import type { Settings, TrayMenuState } from "../../lib/types";

const ESTADO: TrayMenuState = {
  version: "0.2.6",
  recording: false,
  replay: true,
  captureShortcut: "Ctrl+Shift+2",
  recordShortcut: "Ctrl+Shift+5",
  replayShortcut: "Ctrl+Shift+6",
};

const AJUSTES = {
  theme: "oscuro",
  language: "es",
  replayEnabled: true,
} as unknown as Settings;

async function abrirMenu(estado: Partial<TrayMenuState> = {}) {
  responde("tray_menu_state", { ...ESTADO, ...estado });
  responde("get_settings", { ...AJUSTES, replayEnabled: { ...ESTADO, ...estado }.replay });
  render(<TrayMenu />);
  await waitFor(() => expect(screen.getByText("winshotx")).toBeInTheDocument());
}

const accionesPedidas = () =>
  llamadas.filter((l) => l.comando === "tray_menu_action").map((l) => l.args);

beforeEach(() => aplicarIdioma("es"));

describe("lo que enseña", () => {
  it("la versión, para no tener que abrir los ajustes a mirarla", async () => {
    await abrirMenu();
    expect(screen.getByText("0.2.6")).toBeInTheDocument();
  });

  it("el atajo de cada cosa, que un menú de Windows no sabe enseñar", async () => {
    await abrirMenu();
    expect(screen.getByText("Ctrl+Shift+2")).toBeInTheDocument();
    expect(screen.getByText("Ctrl+Shift+5")).toBeInTheDocument();
  });

  it("con el anillo apagado no ofrece rescatar nada", async () => {
    // Rescatar sin nada grabado no puede hacer nada: la entrada no existe.
    await abrirMenu({ replay: false });
    expect(screen.queryByText("Quedarme con lo último")).toBeNull();
    expect(screen.getByRole("switch")).toHaveAttribute("aria-checked", "false");
  });

  it("y grabando, la entrada de grabar es la de parar", async () => {
    await abrirMenu({ recording: true });
    expect(screen.getByText("Parar")).toBeInTheDocument();
    expect(screen.queryByText("Grabar región")).toBeNull();
  });
});

describe("lo que hace cada entrada", () => {
  it("capturar, grabar y rescatar piden su acción por su nombre", async () => {
    await abrirMenu();
    fireEvent.click(screen.getByText("Capturar región"));
    fireEvent.click(screen.getByText("Grabar región"));
    fireEvent.click(screen.getByText("Quedarme con lo último"));
    await waitFor(() => expect(accionesPedidas()).toHaveLength(3));
    expect(accionesPedidas()).toEqual([
      { action: "capture" },
      { action: "record" },
      { action: "replay" },
    ]);
  });

  it("y la carpeta, los ajustes, las actualizaciones y salir", async () => {
    await abrirMenu();
    fireEvent.click(screen.getByText("Abrir la carpeta"));
    fireEvent.click(screen.getByText("Ajustes"));
    fireEvent.click(screen.getByText("Buscar actualizaciones"));
    fireEvent.click(screen.getByText("Salir"));
    await waitFor(() => expect(accionesPedidas()).toHaveLength(4));
    expect(accionesPedidas()).toEqual([
      { action: "folder" },
      { action: "settings" },
      { action: "update" },
      { action: "quit" },
    ]);
  });

  it("el interruptor del anillo va por el mismo camino que el de los ajustes", async () => {
    // Y no por un atajo suyo: si Rust reacciona a `set_settings`, encenderlo desde aquí
    // tiene que ser exactamente lo mismo que encenderlo desde su fila.
    await abrirMenu();
    fireEvent.click(screen.getByRole("switch"));

    await waitFor(() => {
      const guardado = llamadas.filter((l) => l.comando === "set_settings").at(-1);
      expect(guardado?.args).toMatchObject({
        settings: expect.objectContaining({ replayEnabled: false }),
      });
    });
  });
});

describe("al volver a abrirse", () => {
  it("relee el estado, porque la ventana se reutiliza", async () => {
    await abrirMenu({ replay: false });
    expect(screen.queryByText("Quedarme con lo último")).toBeNull();

    // El anillo se ha encendido desde los ajustes mientras el menú estaba escondido.
    responde("tray_menu_state", { ...ESTADO, replay: true });
    emite("winshotx://tray-menu-opened", null);

    await waitFor(() =>
      expect(screen.getByText("Quedarme con lo último")).toBeInTheDocument(),
    );
  });
});
