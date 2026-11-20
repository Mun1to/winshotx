/**
 * La bienvenida (rehecha en S38 con el idioma de los ajustes de la 0.5).
 *
 * Se comprueba lo que hace cada paso de verdad (lo que guarda y lo que pide a Rust), que los
 * pasos se recorren por la cabecera y por el pie, que lo que hay que mirar sale en ámbar y
 * nunca en verde, y que en inglés no se queda nada en español.
 */
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WelcomeApp } from "./WelcomeApp";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas, responde } from "../../test/preparar";
import type { Settings } from "../../lib/types";

const AJUSTES = {
  captureShortcut: "CmdOrCtrl+Shift+KeyS",
  recordShortcut: "CmdOrCtrl+Shift+KeyA",
  captureFlow: "toolbar",
  startWithWindows: false,
  onboarded: false,
} as unknown as Settings;

async function abrir(imprPant = { enabled: false, active: false, takenByWindows: true, storeBuild: false }) {
  responde("get_settings", AJUSTES);
  responde("set_settings", null);
  responde("shortcut_status", { capture: true, record: true, replay: false, printScreen: false, winShiftS: false });
  responde("print_screen_state", imprPant);
  const onDone = vi.fn();
  render(<WelcomeApp onDone={onDone} />);
  await screen.findByRole("navigation", { name: /Pasos de la bienvenida|Welcome steps/ });
  return onDone;
}

/** Lo último que se mandó a guardar. */
function guardado(): Partial<Settings> | undefined {
  const sets = llamadas.filter((l) => l.comando === "set_settings");
  return (sets[sets.length - 1]?.args as { settings?: Settings } | undefined)?.settings;
}

function pasoActual(): string | null {
  const nav = screen.getByRole("navigation", { name: /Pasos de la bienvenida|Welcome steps/ });
  return within(nav).getAllByRole("button").find((b) => b.getAttribute("aria-current") === "step")?.textContent ?? null;
}

beforeEach(() => aplicarIdioma("es"));

describe("los cuatro pasos", () => {
  it("van en la cabecera, en texto, y empieza por las teclas", async () => {
    await abrir();
    const nav = screen.getByRole("navigation", { name: "Pasos de la bienvenida" });
    expect(within(nav).getAllByRole("button").map((b) => b.textContent)).toEqual([
      "Tus teclas",
      "Al soltar",
      "Impr Pant",
      "Listo",
    ]);
    expect(pasoActual()).toBe("Tus teclas");
    expect(screen.getByRole("heading", { name: "winshotx ya está en marcha" })).toBeInTheDocument();
  });

  it("se recorren con el pie: Empezar, Siguiente, Atrás y, al final, Todo listo termina", async () => {
    const onDone = await abrir();
    // En el primero no hay «Atrás»: no hay adónde volver.
    expect(screen.queryByRole("button", { name: "Atrás" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Empezar" }));
    await screen.findByRole("heading", { name: "¿Cómo prefieres capturar?" });
    fireEvent.click(screen.getByRole("button", { name: "Atrás" }));
    await screen.findByRole("heading", { name: "winshotx ya está en marcha" });
    fireEvent.click(screen.getByRole("button", { name: "Empezar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Siguiente" }));
    await screen.findByRole("heading", { name: "¿Le quitamos la tecla a la Herramienta de Recortes?" });
    fireEvent.click(screen.getByRole("button", { name: "Siguiente" }));
    await screen.findByRole("heading", { name: "Listo, ya puedes capturar" });
    fireEvent.click(screen.getByRole("button", { name: "Todo listo" }));
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(guardado()?.onboarded).toBe(true);
  });

  it("y con la cabecera se salta a cualquiera", async () => {
    await abrir();
    const nav = screen.getByRole("navigation", { name: "Pasos de la bienvenida" });
    fireEvent.click(within(nav).getByRole("button", { name: "Listo" }));
    await screen.findByRole("heading", { name: "Listo, ya puedes capturar" });
    expect(pasoActual()).toBe("Listo");
  });
});

describe("lo que hace cada paso", () => {
  it("una combinación sugerida se pone con un clic y se guarda", async () => {
    await abrir();
    fireEvent.click(screen.getByRole("button", { name: "Ctrl Shift 2" }));
    await waitFor(() => expect(guardado()?.captureShortcut).toBe("CmdOrCtrl+Shift+Digit2"));
  });

  it("si otra aplicación tiene el atajo, lo dice en ámbar", async () => {
    responde("get_settings", AJUSTES);
    responde("shortcut_status", { capture: false, record: true, replay: false, printScreen: false, winShiftS: false });
    responde("print_screen_state", { enabled: false, active: false });
    render(<WelcomeApp onDone={vi.fn()} />);
    const aviso = await screen.findByText("Esta la tiene otra aplicación. Prueba con otra.");
    expect(aviso).toHaveClass("text-aviso");
  });

  it("al soltar: se elige con las opciones en texto, se guarda y el dibujo cambia", async () => {
    await abrir();
    fireEvent.click(screen.getByRole("button", { name: "Empezar" }));
    const grupo = await screen.findByRole("group", { name: "Al soltar el ratón" });
    expect(within(grupo).getByRole("button", { name: "Con barra" })).toHaveAttribute("aria-pressed", "true");
    expect(document.querySelector('[data-dibujo="barra"]')).not.toBeNull();

    fireEvent.click(within(grupo).getByRole("button", { name: "Al vuelo" }));
    await waitFor(() => expect(guardado()?.captureFlow).toBe("instant"));
    expect(within(grupo).getByRole("button", { name: "Al vuelo" })).toHaveAttribute("aria-pressed", "true");
    expect(document.querySelector('[data-dibujo="miniatura"]')).not.toBeNull();
    expect(screen.getByText("Ya está en el portapapeles")).toBeInTheDocument();
  });

  it("Impr Pant: arranca en lo que hay hoy y pedírsela a Windows llama a Rust", async () => {
    await abrir();
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "Impr Pant" }));
    const grupo = await screen.findByRole("group", { name: "Impr Pant abre" });
    expect(within(grupo).getByRole("button", { name: "Herramienta de Recortes" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    responde("use_print_screen", { enabled: true, active: false, takenByWindows: false });
    fireEvent.click(within(grupo).getByRole("button", { name: "winshotx" }));
    await waitFor(() =>
      expect(llamadas.find((l) => l.comando === "use_print_screen")?.args).toEqual({ enabled: true }),
    );
    // Pedida pero sin conseguir: se dice en ámbar, que es lo que hay que mirar.
    const aviso = await screen.findByText("Impr Pant no ha caído: hay otro programa que la tiene cogida.");
    expect(aviso).toHaveClass("text-aviso");
    expect(document.querySelector('[data-dibujo="impr-pant-winshotx"]')).not.toBeNull();
  });

  it("Impr Pant: desde la Store no se ofrece y se dice qué hacer", async () => {
    await abrir({ enabled: false, active: false, takenByWindows: true, storeBuild: true });
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "Impr Pant" }));
    await screen.findByRole("heading", { name: "¿Le quitamos la tecla a la Herramienta de Recortes?" });
    // Nada que pulsar: pedirla desde aquí diría que sí sin que Windows lo vea nunca.
    expect(screen.queryByRole("group", { name: "Impr Pant abre" })).toBeNull();
    const aviso = screen.getByText(
      "La versión de la Microsoft Store no puede quitarle Impr Pant a Windows. Para usarla, instala winshotx desde winshotx.com o con winget.",
    );
    expect(aviso).toHaveClass("text-aviso");
    expect(screen.getByText("Se queda en Windows: la versión de la Microsoft Store tampoco puede quitársela.")).toBeInTheDocument();
    expect(llamadas.some((l) => l.comando === "use_print_screen")).toBe(false);
  });

  it("el final cuenta lo que queda y arrancar con Windows se guarda", async () => {
    await abrir({ enabled: true, active: true, takenByWindows: false, storeBuild: false });
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "Listo" }));
    await screen.findByRole("heading", { name: "Listo, ya puedes capturar" });
    expect(screen.getByText("También con Impr Pant")).toBeInTheDocument();
    expect(screen.getByText("sale la barra para elegir")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("switch", { name: "Arrancar con Windows" }));
    await waitFor(() => expect(guardado()?.startWithWindows).toBe(true));
  });
});

describe("el dibujo de la barra de tareas de Windows", () => {
  const barraDeWindows = () => document.querySelector('[data-dibujo="winshotx-en-la-bandeja"]');
  const dibujoDeLaZona = () => document.querySelector('[data-dibujo="seleccion"]');
  const irAlFinal = async () => {
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "Listo" }));
    await screen.findByRole("heading", { name: "Listo, ya puedes capturar" });
  };

  it("en Windows sale en el primer paso", async () => {
    await abrir();
    expect(barraDeWindows()).not.toBeNull();
    expect(dibujoDeLaZona()).not.toBeNull();
  });

  it("en Linux con X11 se dibuja la zona, sin la barra ni en las teclas ni al final", async () => {
    responde("plataforma", { sistema: "linux", wayland: false });
    await abrir();
    await screen.findByRole("heading", { name: "winshotx ya está en marcha" });
    await waitFor(() => expect(screen.queryByRole("button", { name: "Impr Pant" })).toBeNull());
    expect(barraDeWindows()).toBeNull();
    expect(dibujoDeLaZona()).not.toBeNull();
    await irAlFinal();
    expect(barraDeWindows()).toBeNull();
  });

  it("en Wayland, igual", async () => {
    responde("plataforma", { sistema: "linux", wayland: true });
    responde("teclas_del_escritorio", {
      capture: { texto: "Press <Shift><Control>2", atajo: "CmdOrCtrl+Shift+Digit2" },
      edit: null,
      aMano: false,
      orden: "winshotx",
    });
    await abrir();
    await screen.findByText("Capturar y abrir el editor");
    expect(barraDeWindows()).toBeNull();
    expect(dibujoDeLaZona()).not.toBeNull();
    await irAlFinal();
    expect(barraDeWindows()).toBeNull();
  });
});

describe("el aspecto de la 0.5", () => {
  /**
   * Lo que se escribe en el código de la bienvenida, no lo que pinta: el campo de los atajos
   * es de los ajustes y lleva sus 11 px dentro (aquí se suben a 12 por fuera).
   */
  const codigo = Object.values(
    import.meta.glob(["./*.tsx", "!./*.test.tsx"], { query: "?raw", import: "default", eager: true }) as Record<
      string,
      string
    >,
  ).join("\n");

  it("sin verde de «todo va bien»", () => {
    expect(codigo).not.toMatch(/emerald|green-\d/);
  });

  it("ninguna letra por debajo de 12 px", () => {
    expect(codigo).not.toMatch(/text-\[(?:[0-9]|1[01])(?:\.\d+)?px\]|text-xs\b/);
  });

  it("sin mayúsculas espaciadas", () => {
    expect(codigo).not.toMatch(/uppercase|tracking-wide/);
  });
});

describe("en inglés", () => {
  const DELATORAS = [
    "Tus teclas",
    "Lo que vas a ver",
    "Capturar una región",
    "o prueba",
    "Empezar",
    "Siguiente",
    "Atrás",
    "Al soltar",
    "Herramienta de Recortes",
    "Mayús",
    "Dónde se queda",
    "Todo listo",
  ];

  it("no se queda ni una frase en español en ningún paso", async () => {
    aplicarIdioma("en");
    await abrir();
    const nav = screen.getByRole("navigation", { name: "Welcome steps" });
    for (const nombre of ["Your keys", "When you let go", "Print Screen", "Done"]) {
      fireEvent.click(within(nav).getByRole("button", { name: nombre }));
      await waitFor(() => expect(pasoActual()).toBe(nombre));
      const texto = document.body.textContent ?? "";
      for (const delatora of DELATORAS) expect(texto, `${nombre}: «${delatora}»`).not.toContain(delatora);
    }
  });
});
