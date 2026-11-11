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

describe("las ultimas capturas", () => {
  const FOTOS = [
    { path: "C:/capturas/winshotx-2026-10-16-020301.png", nombre: "winshotx-2026-10-16-020301.png", haceSegundos: 30 },
    { path: "C:/capturas/winshotx-2026-10-15-193000.png", nombre: "winshotx-2026-10-15-193000.png", haceSegundos: 7200 },
  ];

  it("salen como miniaturas y al pulsar una se copia esa", async () => {
    responde("ultimas_capturas", FOTOS);
    await abrirMenu();
    const boton = await screen.findByLabelText(`Copiar ${FOTOS[1].nombre}`);
    fireEvent.click(boton);
    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "copiar_captura")).toBe(true),
    );
    const args = llamadas.find((l) => l.comando === "copiar_captura")?.args as { path: string };
    // La que se ha pulsado, no la primera de la lista.
    expect(args.path).toBe(FOTOS[1].path);
  });

  /**
   * La carpeta de capturas (Imágenes) no está en el permiso `asset` de `tauri.conf.json`,
   * que solo deja el temporal y los datos de winshotx: una foto leída de ahí sale rota en la
   * app instalada. La miniatura se le pide a Rust, que la deja en el temporal.
   */
  it("las miniaturas salen del temporal, no de la carpeta de capturas", async () => {
    responde("ultimas_capturas", FOTOS);
    responde("historial_miniatura", "C:/temp/winshotx/historial/vista.png");
    await abrirMenu();
    const boton = await screen.findByLabelText(`Copiar ${FOTOS[0].nombre}`);
    await waitFor(() =>
      expect(boton.querySelector("img")?.getAttribute("src")).toBe("asset://C:/temp/winshotx/historial/vista.png"),
    );
    const pedidas = llamadas.filter((l) => l.comando === "historial_miniatura").map((l) => l.args);
    expect(pedidas).toContainEqual({ ruta: FOTOS[0].path });
  });

  it("sin nada guardado todavia, el menu no ensenna la seccion", async () => {
    responde("ultimas_capturas", []);
    await abrirMenu();
    expect(screen.queryByText("Últimas capturas")).toBeNull();
  });

  /**
   * El historial es lo ultimo del menu y no puede tumbar lo primero: si la carpeta esta
   * en un disco que no responde, capturar y grabar tienen que seguir ahi.
   */
  it("si el historial falla, el menu sigue entero", async () => {
    responde("ultimas_capturas", null);
    await abrirMenu();
    expect(screen.getByText("Capturar región")).toBeInTheDocument();
    expect(screen.getByText("Grabar región")).toBeInTheDocument();
  });
});
