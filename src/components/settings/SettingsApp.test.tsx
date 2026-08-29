/**
 * La pantalla de ajustes, que es la unica de winshotx que se ve entera y con calma.
 *
 * Lo que se comprueba aqui es que las cuatro secciones existen, que cada una lleva lo suyo
 * y que **en ingles no se queda ni una frase en castellano**. Eso ultimo no lo puede ver
 * la prueba del diccionario: una frase puede estar traducida en `textos-en.ts` y salir en
 * espannol igual, porque el componente la escribio a pelo sin pasarla por `t()`. Ha pasado.
 */
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsApp } from "./SettingsApp";
import { aplicarIdioma } from "../../lib/i18n";
import { responde } from "../../test/preparar";
import type { ReplayStatus, Settings } from "../../lib/types";

/** El anillo apagado, que es como viene winshotx de fabrica. */
const PARADO: ReplayStatus = {
  running: false,
  seconds: 0,
  screen: 0,
  screenLabel: "",
  bytes: 0,
  bufferedMs: 0,
};

const AJUSTES: Settings = {
  captureShortcut: "Ctrl+Shift+2",
  theme: "sistema",
  language: "sistema",
  recordShortcut: "Ctrl+Shift+5",
  replayShortcut: "Ctrl+Shift+6",
  saveDirectory: "C:\\Users\\prueba\\Pictures\\winshotx",
  copyAfterCapture: true,
  openEditorAfterRecording: true,
  captureCursor: false,
  recordAudio: true,
  recordMicrophone: false,
  highlightClicks: false,
  highlightKeys: false,
  fps: 30,
  replayEnabled: false,
  replaySeconds: 30,
  playSound: true,
  showMagnifier: true,
  startWithWindows: false,
  captureDelaySeconds: 0,
  hideDesktopIcons: false,
  captureFlow: "toolbar",
  printScreenCapture: false,
  takeWinShiftS: false,
  onboarded: true,
  snippingKeyRestore: null,
  disabledHotkeysRestore: null,
};

/** Monta los ajustes con Rust contestando datos de mentira, y espera a que carguen. */
async function abrirAjustes(replay?: ReplayStatus) {
  responde("get_settings", AJUSTES);
  responde("shortcut_status", {
    capture: true,
    record: true,
    replay: true,
    printScreen: false,
    winShiftS: false,
  });
  responde("cache_stats", { bytes: 12_345_678, sessions: 3 });
  responde("print_screen_state", { enabled: false, active: false });
  responde("just_updated", false);
  responde("replay_status", replay ?? PARADO);
  render(<SettingsApp onVerBienvenida={vi.fn()} />);
  // La pantalla se pinta vacia y se rellena cuando contesta Rust.
  await waitFor(() => expect(screen.getByRole("button", { name: /Capturar|Capture/ })).toBeInTheDocument());
}

beforeEach(() => aplicarIdioma("es"));

describe("las cuatro secciones", () => {
  it("estan las cuatro, y ni una mas", async () => {
    await abrirAjustes();
    const menu = screen.getByLabelText("Secciones de ajustes");
    const nombres = within(menu)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(nombres).toEqual(["Capturar", "Grabar", "Teclas de Windows", "La app"]);
  });

  it("en ingles se llaman por su nombre ingles", async () => {
    aplicarIdioma("en");
    await abrirAjustes();
    const menu = screen.getByLabelText("Settings sections");
    const nombres = within(menu)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(nombres).toEqual(["Capture", "Record", "Windows keys", "The app"]);
  });

  it("empieza por Capturar, que es lo que se viene a cambiar", async () => {
    await abrirAjustes();
    expect(screen.getByText("Al pulsar el atajo")).toBeInTheDocument();
  });
});

describe("con la aplicacion en ingles", () => {
  /**
   * Frases que delatan que algo se quedo sin traducir. Se buscan en el texto de la
   * pantalla entera: si alguna aparece, hay un componente escribiendo espannol a pelo.
   */
  const DELATORAS = [
    "Al pulsar el atajo",
    "Sale la barra",
    "Se copia sola",
    "Sin espera",
    "Automático",
    "segundos",
    "Carpeta",
    "Versión",
  ];

  it("no queda ni una frase en castellano por ningun rincon", async () => {
    aplicarIdioma("en");
    await abrirAjustes();
    const pantalla = document.body.textContent ?? "";
    const coladas = DELATORAS.filter((frase) => pantalla.includes(frase));
    expect(coladas).toEqual([]);
  });

  it("y la version se sigue leyendo, con su palabra delante", async () => {
    aplicarIdioma("en");
    await abrirAjustes();
    expect(screen.getByText(/^Version /)).toBeInTheDocument();
  });
});

describe("lo que ensenna de la maquina, en «La app»", () => {
  /** Los archivos y la version viven en la ultima seccion, no en la primera. */
  async function irALaApp() {
    await abrirAjustes();
    fireEvent.click(screen.getByRole("button", { name: "La app" }));
  }

  it("dice cuanto ocupa la cache en unidades que se entienden", async () => {
    await irALaApp();
    // 12.345.678 bytes son 11,8 MB.
    expect(screen.getByText(/11[,.]8 MB/)).toBeInTheDocument();
  });

  it("ensenna la carpeta donde caen las capturas", async () => {
    await irALaApp();
    expect(screen.getByText(/Pictures.winshotx/)).toBeInTheDocument();
  });

  it("y el numero de version, que es lo primero que se mira al reportar un fallo", async () => {
    await irALaApp();
    expect(screen.getByText(/^Versión \d+\.\d+\.\d+$/)).toBeInTheDocument();
  });
});

/**
 * Los ultimos segundos es la unica funcion que trabaja sin que nadie se lo haya pedido en
 * ese momento, asi que lo que se comprueba aqui es que la pantalla lo CUENTE: que esta
 * encendido, sobre que pantalla y con cuanto guardado. Un interruptor mudo seria peor que
 * no tener la funcion.
 */
describe("los ultimos segundos, en «Grabar»", () => {
  async function irAGrabar(replay?: ReplayStatus) {
    await abrirAjustes(replay);
    // El nombre del boton cambia con el idioma, y una de estas pruebas va en ingles.
    fireEvent.click(screen.getByRole("button", { name: /^(Grabar|Record)$/ }));
  }

  const CORRIENDO: ReplayStatus = {
    running: true,
    seconds: 30,
    screen: 2,
    screenLabel: "DELL U2723QE",
    bytes: 48_234_496,
    bufferedMs: 30_000,
  };

  it("apagado, explica lo que va a hacer antes de que nadie lo encienda", async () => {
    await irAGrabar();
    expect(screen.getByText("Los últimos segundos")).toBeInTheDocument();
    expect(screen.getByText(/graba sin parar y tira lo viejo/)).toBeInTheDocument();
  });

  it("encendido, dice que pantalla vigila y cuanto esta ocupando", async () => {
    await irAGrabar(CORRIENDO);
    // El número de pantalla, no el nombre que le pone Windows: «\.\DISPLAY3» no le
    // dice nada a nadie, y el número es el mismo que sale al elegirla para capturar.
    expect(screen.getByText(/vigilando la pantalla 2/)).toBeInTheDocument();
    expect(screen.getByText(/46[,.]0 MB/)).toBeInTheDocument();
  });

  it("si hay menos de lo que promete el ajuste, dice cuanto hay de verdad", async () => {
    // Pasa mientras se llena, y pasa para siempre en una pantalla que cambia tanto que el
    // tope de disco tira lo viejo antes de tiempo. Prometer treinta y dar ocho seria
    // mentir justo cuando alguien va a pulsar la tecla.
    await irAGrabar({ ...CORRIENDO, bufferedMs: 8_400 });
    expect(screen.getByText(/ahora mismo hay 8 s guardados/)).toBeInTheDocument();
  });

  it("apagado, la tecla no promete nada que no vaya a pasar", async () => {
    await irAGrabar();
    expect(screen.getByText("primero hay que encenderlo aquí arriba")).toBeInTheDocument();
  });

  it("en ingles no se cuela ni una palabra en castellano", async () => {
    aplicarIdioma("en");
    await irAGrabar(CORRIENDO);
    const pantalla = document.body.textContent ?? "";
    const coladas = [
      "Los últimos segundos",
      "Grabar siempre",
      "Cuánto se guarda",
      "Quedarme con",
      "vigilando",
    ].filter((frase) => pantalla.includes(frase));
    expect(coladas).toEqual([]);
  });
});
