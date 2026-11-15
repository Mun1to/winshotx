/**
 * La pantalla de ajustes, que es la unica de winshotx que se ve entera y con calma.
 *
 * Desde la 0.5 (S36, la variación 9 que eligió Munito) son ocho apartados en un riel de
 * iconos, uno a la vez, con el dibujo de «Así va a salir» a la derecha. Lo que se comprueba
 * aqui es que los ocho existen, que cada uno lleva lo suyo, que el dibujo cambia con su
 * ajuste, y que **en ingles no se queda ni una frase en castellano**. Eso ultimo no lo puede
 * ver la prueba del diccionario: una frase puede estar traducida en `textos-en.ts` y salir en
 * espannol igual, porque el componente la escribio a pelo sin pasarla por `t()`. Ha pasado.
 */
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsApp } from "./SettingsApp";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas, responde, ventana } from "../../test/preparar";
import type { ReplayStatus, Settings } from "../../lib/types";

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: () => Promise.resolve(null),
}));

/** El anillo apagado, que es como viene winshotx de fabrica. */
const PARADO: ReplayStatus = {
  running: false,
  seconds: 0,
  screen: 0,
  screenLabel: "",
  bytes: 0,
  bytesPerSecond: 0,
  width: 0,
  height: 0,
  bufferedMs: 0,
};

const AJUSTES: Settings = {
  captureShortcut: "Ctrl+Shift+2",
  editShortcut: "Super+Shift+KeyL",
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
  replayScreen: null,
  replayFps: 15,
  replayHeight: 0,
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
  // S2 (grabación)
  zoomShortcut: "CmdOrCtrl+Shift+9",
  recordCamera: false,
  cameraDevice: null,
};

/** Monta los ajustes con Rust contestando datos de mentira, y espera a que carguen. */
async function abrirAjustes(
  replay?: ReplayStatus,
  pantallas?: unknown[],
  ajustes?: Partial<Settings>,
  capturaModerna = true,
) {
  responde("get_settings", { ...AJUSTES, ...ajustes });
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
  responde("list_screens", pantallas ?? []);
  responde("captura_moderna", capturaModerna);
  render(<SettingsApp onVerBienvenida={vi.fn()} />);
  // La pantalla se pinta vacia y se rellena cuando contesta Rust.
  await waitFor(() => expect(riel()).toBeInTheDocument());
}

/** El riel de la izquierda, con un icono por apartado. */
const riel = () => screen.getByRole("navigation", { name: /Apartados de los ajustes|Settings sections/ });

/** Pulsa un apartado del riel por su nombre (el `aria-label` del icono). */
function ir(nombre: string | RegExp) {
  fireEvent.click(within(riel()).getByRole("button", { name: nombre }));
}

/** El panel de «Así va a salir» del apartado que se ve. */
const dibujo = () => document.querySelector<HTMLElement>("[data-dibujo-de]");

beforeEach(() => aplicarIdioma("es"));

describe("los ocho apartados", () => {
  it("estan los ocho, en el orden del riel, y ni uno mas", async () => {
    await abrirAjustes();
    const nombres = within(riel())
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label"));
    expect(nombres).toEqual([
      "Capturar",
      "Después de capturar",
      "Grabar",
      "Los últimos segundos",
      "Historial",
      "Teclas de Windows",
      "La app",
      "Ayuda y winshotx",
    ]);
  });

  it("solo iconos, con el nombre en el globo del ratón", async () => {
    await abrirAjustes();
    for (const boton of within(riel()).getAllByRole("button")) {
      // Ni una letra a la vista: el nombre va en el `title`, que es lo que sale al pasar.
      expect(boton.textContent).toBe("");
      expect(boton.getAttribute("title")).toBe(boton.getAttribute("aria-label"));
    }
  });

  it("en ingles se llaman por su nombre ingles", async () => {
    aplicarIdioma("en");
    await abrirAjustes();
    const nombres = within(riel())
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label"));
    expect(nombres).toEqual([
      "Capture",
      "After capturing",
      "Record",
      "The last few seconds",
      "History",
      "Windows keys",
      "The app",
      "Help and winshotx",
    ]);
  });

  it("empieza por Capturar, que es lo que se viene a cambiar", async () => {
    await abrirAjustes();
    expect(screen.getByRole("heading", { level: 1, name: "Capturar" })).toBeInTheDocument();
    expect(screen.getByText("Los atajos")).toBeInTheDocument();
    expect(within(riel()).getByRole("button", { name: "Capturar" })).toHaveAttribute("aria-current", "page");
  });

  it("pulsar un icono enseña ese apartado, con su título y su frase", async () => {
    await abrirAjustes();
    ir("Teclas de Windows");
    expect(screen.getByRole("heading", { level: 1, name: "Teclas de Windows" })).toBeInTheDocument();
    expect(screen.getByText("Qué teclas del sistema abren winshotx en vez de Windows.")).toBeInTheDocument();
    expect(within(riel()).getByRole("button", { name: "Teclas de Windows" })).toHaveAttribute("aria-current", "page");
    expect(within(riel()).getByRole("button", { name: "Capturar" })).not.toHaveAttribute("aria-current");
  });

  it("con las flechas se pasa de un apartado a otro", async () => {
    await abrirAjustes();
    fireEvent.keyDown(riel(), { key: "ArrowDown" });
    expect(screen.getByRole("heading", { level: 1, name: "Después de capturar" })).toBeInTheDocument();
    fireEvent.keyDown(riel(), { key: "ArrowUp" });
    fireEvent.keyDown(riel(), { key: "ArrowUp" });
    // Desde el primero, hacia arriba da la vuelta al último.
    expect(screen.getByRole("heading", { level: 1, name: "Ayuda y winshotx" })).toBeInTheDocument();
  });

  it("el historial y la ayuda no llevan dibujo; los demás, sí", async () => {
    await abrirAjustes();
    for (const [nombre, conDibujo] of [
      ["Capturar", true],
      ["Después de capturar", true],
      ["Grabar", true],
      ["Los últimos segundos", true],
      ["Historial", false],
      ["Teclas de Windows", true],
      ["La app", true],
      ["Ayuda y winshotx", false],
    ] as const) {
      ir(nombre);
      expect(Boolean(dibujo()), nombre).toBe(conDibujo);
    }
  });
});

describe("la cabecera", () => {
  it("lleva el nombre y la versión, que es lo primero que se mira al contar un fallo", async () => {
    await abrirAjustes();
    const cabecera = screen.getByRole("banner");
    expect(within(cabecera).getByText("winshotx")).toBeInTheDocument();
    expect(within(cabecera).getByText(/^\d+\.\d+\.\d+$/)).toBeInTheDocument();
  });

  it("estando al día no dice nada: ni «Al día» ni un tick", async () => {
    await abrirAjustes();
    // Se espera a que haya preguntado; si no, pasaría igual antes de mirar.
    await waitFor(() => expect(llamadas.some((l) => l.comando === "is_store_build")).toBe(true));
    expect(document.querySelector("[data-aviso-actualizacion]")).toBeNull();
    expect(screen.queryByText(/Al día/)).toBeNull();
  });

  it("guardar un cambio no enseña ningún «Guardado» en verde: el interruptor ya lo dice", async () => {
    await abrirAjustes();
    fireEvent.click(screen.getByRole("switch", { name: "Lupa de píxel" }));
    await waitFor(() => expect(llamadas.some((l) => l.comando === "set_settings")).toBe(true));
    expect(screen.queryByText("Guardado")).toBeNull();
  });

  it("ya no hay barra de abajo ni logo de fondo", async () => {
    await abrirAjustes();
    expect(screen.queryByRole("contentinfo")).toBeNull();
    expect(screen.queryByRole("button", { name: "Apoyar" })).toBeNull();
  });
});

describe("con la aplicacion en ingles", () => {
  /**
   * Frases que delatan que algo se quedo sin traducir. Se buscan en el texto de cada
   * apartado y en los nombres que solo oye el lector de pantalla: si alguna aparece, hay un
   * componente escribiendo espannol a pelo.
   */
  const DELATORAS = [
    "Al pulsar el atajo",
    "Los atajos",
    "Al capturar",
    "Sale la barra",
    "Se copia sola",
    "Automático",
    "segundos",
    "Carpeta",
    "Versión",
    "Así va a salir",
    "Buscar un ajuste",
    "y eliges",
    "en la foto",
    "Sonido y cámara",
    "Lo que guarda",
    "Apagado",
    "sigue siendo de Windows",
    "Guarda en",
    "Invítame",
    "Apartados",
    "Qué hace este ajuste",
  ];

  it("no queda ni una frase en castellano en ningún apartado", async () => {
    aplicarIdioma("en");
    await abrirAjustes();
    const coladas = new Set<string>();
    for (const boton of within(riel()).getAllByRole("button")) {
      fireEvent.click(boton);
      const textos = [
        document.body.textContent ?? "",
        ...[...document.querySelectorAll("[aria-label],[title]")].flatMap((el) => [
          el.getAttribute("aria-label") ?? "",
          el.getAttribute("title") ?? "",
        ]),
      ].join(" ");
      for (const frase of DELATORAS) if (textos.includes(frase)) coladas.add(`${boton.getAttribute("aria-label")}: ${frase}`);
    }
    expect([...coladas]).toEqual([]);
  });
});

describe("lo que ensenna de la maquina", () => {
  it("dice cuanto ocupa la cache en unidades que se entienden", async () => {
    await abrirAjustes();
    ir("La app");
    // 12.345.678 bytes son 11,8 MB.
    // Con el espacio que no se parte entre la cifra y la unidad (la lectura lo deja en uno normal).
    expect(screen.getByText(/11,8 MB/)).toBeInTheDocument();
  });

  it("ensenna la carpeta donde caen las capturas", async () => {
    await abrirAjustes();
    ir("La app");
    expect(screen.getByText(/Pictures.winshotx/)).toBeInTheDocument();
  });

  it("y en la ayuda, el numero de version", async () => {
    await abrirAjustes();
    ir("Ayuda y winshotx");
    expect(screen.getByText(/^winshotx \d+\.\d+\.\d+$/)).toBeInTheDocument();
  });

  it("dice a donde lleva cada boton que saca de la app, antes de pulsarlo", async () => {
    // Un boton que abre el navegador y no dice a donde no se pulsa, y con razon. La unica
    // que no lo lleva escrito es la del codigo, que no tiene sitio con sus dos botones.
    await abrirAjustes();
    ir("Ayuda y winshotx");
    expect(screen.getByText("winshotx.com")).toBeInTheDocument();
    expect(screen.getByText("buymeacoffee.com/munito")).toBeInTheDocument();
  });

  it("invitar a un cafe abre buymeacoffee, y no otra cosa", async () => {
    await abrirAjustes();
    ir("Ayuda y winshotx");
    fireEvent.click(screen.getByRole("button", { name: "Invitar" }));
    await waitFor(() => {
      expect(llamadas.some((l) => l.comando === "open_url")).toBe(true);
    });
    const abierta = llamadas.filter((l) => l.comando === "open_url").at(-1);
    expect(abierta?.args).toEqual({ url: "https://buymeacoffee.com/munito" });
  });

  it("y los otros tres botones llevan a los otros tres sitios", async () => {
    await abrirAjustes();
    ir("Ayuda y winshotx");
    for (const [boton, url] of [
      ["La web", "https://winshotx.com"],
      ["El código", "https://github.com/Mun1to/winshotx"],
      ["Contar un fallo", "https://github.com/Mun1to/winshotx/issues/new"],
    ] as const) {
      fireEvent.click(screen.getByRole("button", { name: boton }));
      await waitFor(() => {
        expect(llamadas.filter((l) => l.comando === "open_url").at(-1)?.args).toEqual({ url });
      });
    }
  });

  it("buscar actualizaciones a mano contesta en la misma fila, y solo cuando se pide", async () => {
    await abrirAjustes();
    ir("Ayuda y winshotx");
    // Mirar solo al abrir no cuenta nada: sin pedirlo, «estás en la última versión» sería un
    // «todo va bien» que no pide nada.
    await waitFor(() => expect(llamadas.some((l) => l.comando === "is_store_build")).toBe(true));
    expect(screen.queryByText("estás en la última versión")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    await waitFor(() => expect(screen.getByText("estás en la última versión")).toBeInTheDocument());
  });
});

/**
 * El buscador es una paleta que salta al ajuste y lo ilumina (la de la variación 8, que eligió
 * Munito para la 9). Se prueba como lo usa una persona: con el teclado.
 */
describe("el buscador", () => {
  const paleta = () => screen.queryByRole("dialog", { name: "Buscar un ajuste" });
  const campo = () => within(paleta()!).getByRole("combobox");
  const opciones = () => within(paleta()!).queryAllByRole("option");

  it("se abre con Ctrl + F y con el campo de la cabecera, y se cierra con Esc", async () => {
    await abrirAjustes();
    expect(paleta()).toBeNull();
    fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    expect(paleta()).not.toBeNull();
    await waitFor(() => expect(campo()).toHaveFocus());
    fireEvent.keyDown(campo(), { key: "Escape" });
    expect(paleta()).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Buscar un ajuste/ }));
    expect(paleta()).not.toBeNull();
  });

  it("dice qué escribir y, si no encuentra nada, lo dice", async () => {
    await abrirAjustes();
    fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    expect(within(paleta()!).getByText(/Escribe lo que buscas/)).toBeInTheDocument();
    fireEvent.change(campo(), { target: { value: "zzzz" } });
    expect(within(paleta()!).getByText("Nada con «zzzz». Prueba con otra palabra.")).toBeInTheDocument();
  });

  it("cada resultado dice su nombre y su apartado, con las flechas se elige y Enter salta e ilumina la fila", async () => {
    await abrirAjustes();
    fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    fireEvent.change(campo(), { target: { value: "sonido" } });
    const nombres = opciones().map((o) => o.textContent);
    // Lo que se llama así va primero, cada uno con su apartado a la derecha.
    expect(nombres[0]).toBe("Sonido de obturadorDespués de capturar");
    expect(nombres[1]).toBe("Sonido al empezarGrabar");
    expect(opciones()[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(campo(), { key: "ArrowDown" });
    expect(opciones()[1]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(campo(), { key: "ArrowUp" });
    fireEvent.keyDown(campo(), { key: "ArrowUp" });
    expect(opciones()[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(campo(), { key: "ArrowDown" });
    fireEvent.keyDown(campo(), { key: "Enter" });
    // Se cierra, lleva a Grabar y la fila del sonido al empezar va encendida.
    expect(paleta()).toBeNull();
    expect(screen.getByRole("heading", { level: 1, name: "Grabar" })).toBeInTheDocument();
    const fila = document.querySelector('[data-fila="graSonido"]')!;
    expect(fila.className).toContain("ring-marca/50");
    expect(document.querySelectorAll('[data-fila][class*="ring-marca"]')).toHaveLength(1);
  });

  it("la luz se apaga en cuanto se toca otra cosa", async () => {
    await abrirAjustes();
    fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    fireEvent.change(campo(), { target: { value: "carpeta" } });
    fireEvent.keyDown(campo(), { key: "Enter" });
    const fila = () => document.querySelector('[data-fila="appCarpeta"]')!;
    expect(fila().className).toContain("ring-marca/50");
    // Se engancha un instante después de encenderse, para no cazar el mismo Enter.
    await new Promise((r) => setTimeout(r, 10));
    fireEvent.pointerDown(document.body);
    await waitFor(() => expect(fila().className).not.toContain("ring-marca"));
  });

  it("busca en inglés aunque la app esté en español, y al revés", async () => {
    await abrirAjustes();
    fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    fireEvent.change(campo(), { target: { value: "folder" } });
    expect(opciones()[0].textContent).toContain("Carpeta de destino");
  });

  it("en inglés, los resultados salen en inglés", async () => {
    aplicarIdioma("en");
    await abrirAjustes();
    fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    fireEvent.change(within(screen.getByRole("dialog", { name: "Search a setting" })).getByRole("combobox"), {
      target: { value: "cámara" },
    });
    const texto = screen.getByRole("dialog", { name: "Search a setting" }).textContent ?? "";
    expect(texto).toContain("Record the camera too");
    expect(texto).not.toContain("Grabar también");
  });

  it("cada ajuste que ofrece tiene su fila en su apartado, para poder iluminarla", async () => {
    // Con tres pantallas, para que salga también la de elegir pantalla.
    responde("list_cameras", []);
    await abrirAjustes(undefined, TRES_PANTALLAS);
    const { BUSCABLES } = await import("./mapaDeAjustes");
    const sinFila: string[] = [];
    for (const b of BUSCABLES.filter((x) => x.apartado !== "historial")) {
      fireEvent.click(within(riel()).getByRole("button", { name: { capturar: "Capturar", despues: "Después de capturar", grabar: "Grabar", ultimos: "Los últimos segundos", historial: "Historial", teclas: "Teclas de Windows", app: "La app", ayuda: "Ayuda y winshotx" }[b.apartado] }));
      await waitFor(() => expect(screen.getByRole("heading", { level: 1 })).toBeInTheDocument());
      if (!document.querySelector(`[data-fila="${b.id}"]`)) sinFila.push(b.id);
    }
    expect(sinFila).toEqual([]);
  });
});

/**
 * «Así va a salir» se dibuja con los ajustes de verdad: lo que se comprueba es que cambie al
 * tocar su fila, que es lo único que le da sentido. Si se quedara quieto, mentiría.
 */
describe("el dibujo de cada apartado", () => {
  it("en Capturar, la lupa sale en el dibujo y se va al apagarla", async () => {
    await abrirAjustes();
    expect(dibujo()?.querySelector('[data-dibujo="lupa"]')).not.toBeNull();
    fireEvent.click(screen.getByRole("switch", { name: "Lupa de píxel" }));
    await waitFor(() => expect(dibujo()?.querySelector('[data-dibujo="lupa"]')).toBeNull());
  });

  it("con espera, la cuenta atrás sale en el dibujo y en la frase", async () => {
    await abrirAjustes();
    fireEvent.click(within(screen.getByRole("group", { name: "Esperar antes de capturar" })).getByRole("button", { name: "3 s" }));
    await waitFor(() => expect(dibujo()?.querySelector('[data-dibujo="cuenta"]')?.textContent).toBe("3"));
    expect(dibujo()?.textContent).toContain("Cuenta 3 s antes de hacer la foto");
  });

  it("en Después, el cursor sale en la foto solo si se incluye", async () => {
    await abrirAjustes(undefined, undefined, { captureCursor: true });
    ir("Después de capturar");
    expect(dibujo()?.querySelector('[data-dibujo="cursor"]')).not.toBeNull();
    // Se apaga en Capturar, que es donde vive la fila, y el dibujo de Después lo refleja.
    ir("Capturar");
    fireEvent.click(screen.getByRole("switch", { name: "Incluir el cursor" }));
    ir("Después de capturar");
    expect(dibujo()?.querySelector('[data-dibujo="cursor"]')).toBeNull();
  });

  it("en Grabar, la cámara aparece en el dibujo al encenderla", async () => {
    responde("list_cameras", [{ id: "a", nombre: "HP True Vision FHD Camera" }]);
    await abrirAjustes();
    ir("Grabar");
    expect(dibujo()?.querySelector('[data-dibujo="camara"]')).toBeNull();
    expect(dibujo()?.textContent).toContain("Sin cámara");
    fireEvent.click(screen.getByRole("switch", { name: "Grabar también la cámara" }));
    await waitFor(() => expect(dibujo()?.querySelector('[data-dibujo="camara"]')).not.toBeNull());
    expect(dibujo()?.textContent).toContain("Con tu cámara en una esquina");
  });

  it("en Teclas, Impr Pant se ilumina al dársela a winshotx", async () => {
    await abrirAjustes();
    ir("Teclas de Windows");
    const tecla = () => dibujo()?.querySelector('[data-dibujo="impr-pant"] [data-viva]');
    expect(tecla()).toBeNull();
    expect(dibujo()?.textContent).toContain("Impr Pant sigue siendo de Windows");
    // Rust escribe el ajuste él mismo; la ventana lo vuelve a leer después.
    responde("use_print_screen", { enabled: true, active: true });
    responde("get_settings", { ...AJUSTES, printScreenCapture: true });
    fireEvent.click(screen.getByRole("switch", { name: "Usar también Impr Pant" }));
    await waitFor(() => expect(tecla()?.textContent).toBe("Impr Pant"));
    expect(dibujo()?.textContent).toContain("Impr Pant abre winshotx");
  });

  it("las frases ponen en negrita lo que cambia, sin dejar los asteriscos", async () => {
    await abrirAjustes();
    const negrita = within(dibujo()!).getByText("no sale");
    expect(negrita.tagName).toBe("STRONG");
    expect(dibujo()?.textContent).not.toContain("**");
  });
});

/**
 * Los ultimos segundos es la unica funcion que trabaja sin que nadie se lo haya pedido en
 * ese momento, asi que lo que se comprueba aqui es que la pantalla lo CUENTE: que esta
 * encendido, sobre que pantalla y con cuanto guardado. Un interruptor mudo seria peor que
 * no tener la funcion.
 */
describe("los ultimos segundos", () => {
  async function irAUltimos(replay?: ReplayStatus, pantallas?: unknown[], ajustes?: Partial<Settings>) {
    await abrirAjustes(replay, pantallas, ajustes);
    ir(/^(Los últimos segundos|The last few seconds)$/);
  }

  const CORRIENDO: ReplayStatus = {
    running: true,
    seconds: 30,
    screen: 2,
    screenLabel: "DELL U2723QE",
    bytes: 48_234_496,
    bytesPerSecond: 3_400_000,
    width: 1280,
    height: 720,
    bufferedMs: 30_000,
  };

  it("apagado, el dibujo dice lo que va a hacer antes de que nadie lo encienda", async () => {
    await irAUltimos();
    expect(dibujo()?.textContent).toContain("Apagado: no graba nada hasta que lo enciendas");
  });

  /**
   * Lo que de verdad cuesta tenerlo puesto no son los megabytes guardados, que se quedan
   * quietos en cuanto el anillo se llena: es lo que le escribe al disco cada segundo, que
   * sigue pasando toda la tarde. Por eso la fila enseña las dos cosas y el tamaño real.
   */
  it("encendido, dice de dónde graba, a qué tamaño y lo que le cuesta al disco, en gris y no en verde", async () => {
    await irAUltimos(CORRIENDO);
    // El número de pantalla, no el nombre que le pone Windows: «\.\DISPLAY3» no le
    // dice nada a nadie, y el número es el mismo que sale al elegirla para capturar.
    const dato = screen.getByText(/pantalla 2 · 1280 × 720 · 3,2 MB\/s/);
    expect(dato.className).toContain("text-apagado");
    expect(dato.className).not.toMatch(/emerald|green/);
  });

  it("si hay menos de lo que promete el ajuste, dice cuanto hay de verdad", async () => {
    // Pasa mientras se llena, y pasa para siempre en una pantalla que cambia tanto que el
    // tope de disco tira lo viejo antes de tiempo. Prometer treinta y dar ocho seria
    // mentir justo cuando alguien va a pulsar la tecla.
    await irAUltimos({ ...CORRIENDO, bufferedMs: 8_400 });
    expect(screen.getByText(/ahora mismo hay 8 s guardados/)).toBeInTheDocument();
  });

  it("apagado, la tecla no promete nada que no vaya a pasar", async () => {
    await irAUltimos();
    expect(screen.getByText("primero hay que encenderlo aquí arriba")).toBeInTheDocument();
  });

  /**
   * Munir tuvo el anillo semanas a 60 fotogramas y resolución nativa, que entonces era el
   * 86% de un núcleo todo el día, y lo que notó fue que el ordenador iba con lag y que el
   * atajo tardaba en abrir la captura. Nadie elige eso sabiendo lo que cuesta: se elige
   * porque suena a mejor. Lo dice la propia fila, en ámbar; desde la 0.5, sin la cifra de
   * septiembre, que ya no es la de ahora (S29 lo bajó), hasta medirlo otra vez.
   */
  it("la combinación más cara dice lo que cuesta, y avisa en ámbar, sin una cifra vieja", async () => {
    await irAUltimos(undefined, undefined, { replayFps: 60, replayHeight: 0 });
    const coste = screen.getByText("la más fluida; gasta más cuanto más se mueve la pantalla");
    expect(coste.className).toContain("text-aviso");
    expect(document.body.textContent).not.toMatch(/\d+ ?% de un núcleo/);
  });

  it("y la combinación de en medio dice la suya", async () => {
    await irAUltimos(undefined, undefined, { replayFps: 30, replayHeight: 1080 });
    expect(screen.getByText("gasta poco con la pantalla quieta")).toBeInTheDocument();
  });

  /** Solo se midieron esas dos: de las otras siete se diría algo que nadie ha comprobado. */
  it("y de las que no se midieron no se dice nada", async () => {
    await irAUltimos(undefined, undefined, { replayFps: 15, replayHeight: 720 });
    expect(screen.queryByText(/fluida|gasta/)).toBeNull();
  });

  it("en ingles no se cuela ni una palabra en castellano", async () => {
    aplicarIdioma("en");
    await irAUltimos(CORRIENDO);
    const pantalla = document.body.textContent ?? "";
    const coladas = [
      "Los últimos segundos",
      "Grabar siempre",
      "Cuánto se guarda",
      "Quedarme con",
      "Grabando siempre",
      "hace 30 s",
      "ahora",
    ].filter((frase) => pantalla.includes(frase));
    expect(coladas).toEqual([]);
  });
});

/** Tres monitores como los de Munir, con el principal marcado. */
const TRES_PANTALLAS = [
  { id: 0, label: "\\.\DISPLAY1", x: 0, y: 0, width: 1920, height: 1080, scale: 1, isPrimary: true },
  { id: 1, label: "\\.\DISPLAY2", x: 1920, y: 0, width: 1080, height: 1920, scale: 1, isPrimary: false },
  { id: 2, label: "\\.\DISPLAY3", x: -1920, y: 0, width: 1920, height: 1080, scale: 1, isPrimary: false },
];

describe("elegir pantalla y calidad", () => {
  async function irAUltimos(pantallas: unknown[] = TRES_PANTALLAS) {
    await abrirAjustes(undefined, pantallas);
    ir("Los últimos segundos");
  }

  it("con una sola pantalla no pregunta cuál: elegir entre una es ruido", async () => {
    await irAUltimos([TRES_PANTALLAS[0]]);
    expect(screen.queryByText("Qué pantalla")).not.toBeInTheDocument();
  });

  it("con tres, se elige, y de fábrica va donde esté el ratón", async () => {
    await irAUltimos();
    expect(screen.getByText("Qué pantalla")).toBeInTheDocument();
    const elegida = screen.getByRole("button", { name: "Ratón" });
    expect(elegida).toHaveAttribute("aria-pressed", "true");
  });

  it("al elegir la segunda, viaja a Rust por su número interno", async () => {
    await irAUltimos();
    fireEvent.click(screen.getByRole("button", { name: "2" }));
    const ultima = [...llamadas].reverse().find((l) => l.comando === "set_settings");
    expect((ultima?.args as { settings: Record<string, unknown> }).settings).toMatchObject({
      replayScreen: 1,
    });
  });

  /**
   * Los segundos no dicen nada de lo que cuestan: una partida escribe diez veces más que un
   * escritorio. Este es el número que decide si alguien deja esto puesto o no.
   */
  it("la calidad dice cuánto disco puede llegar a comerse", async () => {
    await irAUltimos();
    // 30 s a 15 fps por 2,3 MB son unos 987 MB.
    expect(screen.getByText(/hasta 98[0-9],\d MB en disco/)).toBeInTheDocument();
  });

  /**
   * «La pantalla 2» no dice nada si no sabes cuál es la 2. Enseñar el número EN esa
   * pantalla es la única forma que no admite duda, y es lo que pidió Munir al verlo.
   */
  it("al elegir una pantalla, la enseña en la propia pantalla", async () => {
    await irAUltimos();
    fireEvent.click(screen.getByRole("button", { name: "2" }));
    const aviso = [...llamadas].reverse().find((l) => l.comando === "show_screen_number");
    expect(aviso?.args).toEqual({ screen: 1 });
  });

  it("y con «Ratón» no enseña nada, porque no hay ninguna que señalar", async () => {
    await irAUltimos();
    fireEvent.click(screen.getByRole("button", { name: "Ratón" }));
    expect(llamadas.filter((l) => l.comando === "show_screen_number")).toEqual([]);
  });

  /** Es el ajuste que más disco ahorra: 720p deja el fotograma en menos de la mitad. */
  it("la calidad se elige por tamaño, no por un número suelto", async () => {
    await irAUltimos();
    const grupo = screen.getByRole("group", { name: "Calidad" });
    expect(within(grupo).getByRole("button", { name: "Nativa" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(grupo).getByRole("button", { name: "720p" }));
    const ultima = [...llamadas].reverse().find((l) => l.comando === "set_settings");
    expect((ultima?.args as { settings: Record<string, unknown> }).settings).toMatchObject({
      replayHeight: 720,
    });
  });

  it("y ese número sube al subir la fluidez", async () => {
    await irAUltimos();
    const grupo = screen.getByRole("group", { name: "Fluidez" });
    fireEvent.click(within(grupo).getByRole("button", { name: "60" }));
    const ultima = [...llamadas].reverse().find((l) => l.comando === "set_settings");
    expect((ultima?.args as { settings: Record<string, unknown> }).settings).toMatchObject({
      replayFps: 60,
    });
  });
});

describe("cuando este Windows no tiene la captura moderna", () => {
  /**
   * El aviso existe porque la diferencia se ve en la foto y no hay nada que tocar: sin
   * decirlo, las capturas salen con trozos en negro y parece un fallo de winshotx. Munir
   * lo tuvo asi meses hasta que mando una foto de Adeorq.
   */
  it("lo dice en Capturar, que es donde se viene a buscar por que sale mal una foto", async () => {
    await abrirAjustes(undefined, undefined, undefined, false);
    await waitFor(() =>
      expect(screen.getByText("Este Windows usa la captura antigua")).toBeInTheDocument(),
    );
    expect(screen.getByText(/trozos en negro/)).toBeInTheDocument();
  });

  it("y cuando si la tiene, que es lo normal, no se dice nada", async () => {
    await abrirAjustes();
    // Se espera a que conteste Rust antes de dar por bueno que no esta: sin esto, la
    // prueba pasaria igual mirando la pantalla antes de que llegara la respuesta.
    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "captura_moderna")).toBe(true),
    );
    expect(screen.queryByText("Este Windows usa la captura antigua")).not.toBeInTheDocument();
  });

  it("en ingles tampoco se queda en castellano", async () => {
    aplicarIdioma("en");
    await abrirAjustes(undefined, undefined, undefined, false);
    await waitFor(() =>
      expect(screen.getByText("This Windows uses the old capture")).toBeInTheDocument(),
    );
    expect(screen.queryByText(/trozos en negro/)).not.toBeInTheDocument();
  });
});

// S2 (grabación)
describe("la tecla de zoom y la cámara, en Grabar", () => {
  async function irAGrabar(ajustes?: Partial<Settings>, camaras: unknown = []) {
    responde("list_cameras", camaras);
    await abrirAjustes(undefined, undefined, ajustes);
    ir("Grabar");
  }

  it("la tecla de zoom sale con la de fábrica, sin aviso", async () => {
    await irAGrabar();
    expect(screen.getByText("Tecla de zoom")).toBeInTheDocument();
    expect(screen.queryByText(/es la de grabar/)).toBeNull();
  });

  it("avisa si la tecla de zoom es la de grabar, que la pararía", async () => {
    await irAGrabar({ zoomShortcut: "CmdOrCtrl+Shift+Digit5", recordShortcut: "CmdOrCtrl+Shift+5" });
    expect(screen.getByText("es la de grabar: al pulsarla pararías la grabación")).toBeInTheDocument();
  });

  it("sin cámara lo dice, y el interruptor sigue ahí", async () => {
    await irAGrabar();
    expect(screen.getByText("no hay ninguna cámara conectada")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Grabar también la cámara" })).toBeInTheDocument();
  });

  it("con una cámara encendida dice cuál, y no ofrece elegir", async () => {
    await irAGrabar({ recordCamera: true }, [{ id: "a", nombre: "HP True Vision FHD Camera" }]);
    await waitFor(() => expect(screen.getByText("HP True Vision FHD Camera")).toBeInTheDocument());
    expect(screen.queryByText("Qué cámara")).not.toBeInTheDocument();
  });

  it("con dos, se elige y se guarda la elegida", async () => {
    await irAGrabar({ recordCamera: true }, [
      { id: "a", nombre: "HP True Vision FHD Camera" },
      { id: "b", nombre: "Logitech HD Pro Webcam C920" },
    ]);
    await waitFor(() => expect(screen.getByText("Qué cámara")).toBeInTheDocument());
    // Se ve con cuál se graba, y «Cambiar» pasa a la siguiente.
    expect(screen.getByText("HP True Vision FHD Camera")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cambiar" }));
    await waitFor(() => {
      const guardado = llamadas.filter((l) => l.comando === "set_settings").at(-1);
      expect((guardado?.args as { settings: Settings }).settings.cameraDevice).toBe("b");
    });
  });

  it("en inglés no se cuela ni una frase de las nuevas", async () => {
    aplicarIdioma("en");
    responde("list_cameras", []);
    await abrirAjustes();
    ir("Record");
    const pantalla = document.body.textContent ?? "";
    for (const frase of ["Tecla de zoom", "Grabar también la cámara", "no hay ninguna cámara", "Sin cámara"]) {
      expect(pantalla).not.toContain(frase);
    }
    expect(pantalla).toContain("Zoom key");
    expect(pantalla).toContain("Record the camera too");
  });
});

/**
 * S24: la ventana mide lo que mide su apartado. Aquí no hay maquetación (happy-dom no mide
 * nada), así que se le dice cuánto mide cada apartado y se mira qué pide la ventana. Lo de
 * medir de verdad se comprueba en Edge sin ventana (`scripts/probar-medida-ajustes.mjs`).
 */
describe("la ventana a la medida de su apartado", () => {
  /** Lo que mide cada apartado por dentro, sin la cabecera. */
  const MIDE: Record<string, number> = { capturar: 521, grabar: 650, teclas: 416 };
  /** La cabecera, lo único que va fuera del hueco con rueda desde que no hay barra de abajo. */
  const FUERA = 58;
  afterEach(() => vi.restoreAllMocks());

  function medirComoEdge() {
    const real = Element.prototype.getBoundingClientRect;
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
      if (!this.hasAttribute("data-contenido-ajustes")) return real.call(this);
      const alto = MIDE[this.getAttribute("data-apartado") ?? ""] ?? MIDE.capturar;
      return { x: 0, y: FUERA, top: FUERA, left: 0, right: 836, bottom: FUERA + alto, width: 836, height: alto, toJSON: () => ({}) } as DOMRect;
    });
    vi.spyOn(document.documentElement, "clientHeight", "get").mockImplementation(() => ventana.dentro.height);
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function (this: HTMLElement) {
      return this.hasAttribute("data-scroll-ajustes") ? ventana.dentro.height - FUERA : 0;
    });
  }

  it("abre con el alto de Capturar y con los 900 de ancho del diseño nuevo", async () => {
    medirComoEdge();
    await abrirAjustes();
    // La bienvenida deja la ventana en 840: los ajustes piden 900 al abrirse.
    await waitFor(() => expect(ventana.tamannos.at(-1)).toEqual({ width: 900, height: FUERA + MIDE.capturar }));
    // Y se ensancha por igual a los dos lados: 30 px a la izquierda.
    expect(ventana.posiciones.at(-1)?.x).toBe(70);
  });

  it("crece para Grabar y encoge para Teclas, sin mover el borde de arriba ni el ancho", async () => {
    medirComoEdge();
    await abrirAjustes();
    await waitFor(() => expect(ventana.tamannos.at(-1)?.height).toBe(FUERA + MIDE.capturar));
    const posiciones = ventana.posiciones.length;
    ir("Grabar");
    await waitFor(() => expect(ventana.tamannos.at(-1)).toEqual({ width: 900, height: FUERA + MIDE.grabar }));
    ir("Teclas de Windows");
    await waitFor(() => expect(ventana.tamannos.at(-1)).toEqual({ width: 900, height: FUERA + MIDE.teclas }));
    // La primera vez se recentra; después los iconos se quedan debajo del ratón.
    expect(ventana.posiciones.length).toBe(posiciones);
  });

  it("si la estiran a mano más estrecha, no se la vuelve a ensanchar", async () => {
    medirComoEdge();
    await abrirAjustes();
    await waitFor(() => expect(ventana.tamannos.at(-1)?.width).toBe(900));
    ventana.dentro = { ...ventana.dentro, width: 800 };
    ir("Grabar");
    await waitFor(() => expect(ventana.tamannos.at(-1)).toEqual({ width: 800, height: FUERA + MIDE.grabar }));
  });

  it("en una pantalla baja se queda dentro de la zona útil", async () => {
    medirComoEdge();
    ventana.escala = 1.25;
    await abrirAjustes();
    ir("Grabar");
    // La pantalla de prueba tiene 1032 de zona útil y 40 de marco: a 125 %, 708 lógicos son
    // 885 físicos, que caben. Lo que no cabe lo prueba `ventanaALaMedida.test.ts`.
    await waitFor(() => expect(ventana.tamannos.at(-1)?.height).toBe(Math.ceil((FUERA + MIDE.grabar) * 1.25)));
  });
});

/**
 * El tour se cierra donde se pide ayuda, y esa es la unica parada que tiene un boton que
 * saca de la aplicacion. Lo que se comprueba es que se llega hasta el final, que en cada
 * parada se ve en qué apartado se está, y que ese boton lleva donde dice: el resto del
 * recorrido se mira en foto, porque es colocacion.
 */
describe("el tour de los ajustes", () => {
  async function empezar() {
    await abrirAjustes();
    ir("Ayuda y winshotx");
    fireEvent.click(screen.getByRole("button", { name: "Empezar" }));
  }

  async function hastaElFinal() {
    await empezar();
    // «Siguiente» hasta que el boton cambia a «Listo», que es como se sabe que es la
    // ultima. Contar las paradas a mano dejaria la prueba pidiendo un numero cada vez que
    // se anade una.
    for (let i = 0; i < 20; i += 1) {
      const siguiente = screen.queryByRole("button", { name: "Siguiente" });
      if (!siguiente) break;
      fireEvent.click(siguiente);
    }
    expect(screen.getByRole("button", { name: "Listo" })).toBeInTheDocument();
  }

  it("empieza en Capturar, aunque se abra desde la ayuda", async () => {
    await empezar();
    await waitFor(() => expect(screen.getByRole("heading", { level: 1, name: "Capturar" })).toBeInTheDocument());
    expect(screen.getByRole("dialog", { name: "Todo está en estos ocho" })).toBeInTheDocument();
  });

  it("acaba contando como se ayuda, y el boton lleva al cafe", async () => {
    await hastaElFinal();
    fireEvent.click(screen.getByRole("button", { name: /Invítame a un café/ }));
    await waitFor(() => {
      expect(llamadas.filter((l) => l.comando === "open_url").at(-1)?.args).toEqual({
        url: "https://buymeacoffee.com/munito",
      });
    });
  });

  it("y ninguna otra parada pide nada", async () => {
    await empezar();
    expect(screen.queryByRole("button", { name: /Invítame a un café/ })).not.toBeInTheDocument();
  });

  it("la primera ilumina el riel entero; las demás, lo suyo y su icono del riel", async () => {
    await empezar();
    const agujeros = () =>
      [...document.querySelectorAll("[data-tour-velo] [data-agujero]")].map((a) => a.getAttribute("data-agujero"));
    await waitFor(() => expect(agujeros()).toEqual(["objetivo"]));
    for (let i = 0; i < 20; i += 1) {
      const siguiente = screen.queryByRole("button", { name: "Siguiente" });
      if (!siguiente) break;
      fireEvent.click(siguiente);
      await waitFor(() => expect(agujeros()).toEqual(["objetivo", "pestana"]));
    }
  });
});
