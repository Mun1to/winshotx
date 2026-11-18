/**
 * El overlay de selección entero.
 *
 * Aquí se comprueban los dos gestos que ninguna pieza suelta puede ver, porque empiezan
 * en un componente y acaban en otro: salir con Escape, y empezar a recortar justo donde
 * está la barra de arriba.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { SelectionCanvas } from "./SelectionCanvas";
import { aplicarIdioma } from "../../lib/i18n";
import { emite, emitidos, escuchas, llamadas, responde, ventana } from "../../test/preparar";
import { EVENTO_PROPORCION, type Proporcion } from "../../lib/proporcion";
import { EVENTO_SELECCION, type SeleccionCompartida } from "../../lib/seleccionCompartida";
import {
  EVENTS,
  type MonitorInfo,
  type OverlayPayload,
  type Rect,
  type Settings,
} from "../../lib/types";

const ANCHO = window.innerWidth;
const ALTO = window.innerHeight;

/** La barra vive arriba y centrada: este punto cae dentro de ella. */
const EN_LA_BARRA = { x: Math.round(ANCHO / 2), y: 30 };

const AJUSTES = {
  captureFlow: "toolbar",
  showMagnifier: false,
  fps: 30,
  captureCursor: true,
  recordAudio: false,
  recordMicrophone: false,
  highlightClicks: false,
  highlightKeys: false,
} as unknown as Settings;

const MONITOR: MonitorInfo = {
  id: 1,
  label: "PANTALLA 1",
  x: 0,
  y: 0,
  width: ANCHO,
  height: ALTO,
  scale: 1,
  isPrimary: true,
};

const PAYLOAD: OverlayPayload = {
  monitor: MONITOR,
  // Una sola pantalla: todo lo de antes de las dos pantallas sigue igual.
  monitores: [MONITOR],
  generation: 1,
  windows: [],
  settings: AJUSTES,
  intent: "capture",
  screenNumber: 1,
  screenCount: 1,
  lastRegion: null,
};

beforeEach(() => {
  aplicarIdioma("es");
  responde("overlay_bootstrap", PAYLOAD);
  // El congelado llega en PNG por el IPC y se pasa a bitmap: aquí no hay decodificador de
  // imágenes, así que se dobla. El bitmap mide lo que la ventana para que la escala sea 1
  // y el tamaño que se lee sea el que se arrastra.
  responde("freeze_png", new Uint8Array([137, 80, 78, 71]).buffer);
  vi.stubGlobal("createImageBitmap", vi.fn(async () => ({ width: ANCHO, height: ALTO })));
  HTMLCanvasElement.prototype.getContext = (() => ({
    drawImage: () => {},
    getImageData: () => ({ data: new Uint8ClampedArray([0, 0, 0, 255]) }),
  })) as unknown as HTMLCanvasElement["getContext"];
});

/** Monta el overlay y espera a que el congelado esté puesto y la barra pintada. */
async function abrir() {
  const { container } = render(<SelectionCanvas monitorId={1} />);
  await screen.findByLabelText("Foto");
  return container.firstElementChild as HTMLElement;
}

/** Arrastra de un punto a otro empezando en `desde`, que puede ser cualquier elemento. */
function arrastrar(desde: Element, a: { x: number; y: number }, b: { x: number; y: number }) {
  fireEvent.pointerDown(desde, { clientX: a.x, clientY: a.y, buttons: 1 });
  fireEvent.pointerMove(desde, { clientX: b.x, clientY: b.y, buttons: 1 });
  fireEvent.pointerMove(window, { clientX: b.x, clientY: b.y, buttons: 1 });
  fireEvent.pointerUp(window, { clientX: b.x, clientY: b.y });
}

/** El tamaño que enseña la etiqueta del recorte, o null si no hay recorte. */
function recorte(): string | null {
  return screen.queryByText(/^\d+ × \d+$/)?.textContent ?? null;
}

describe("salir de la captura", () => {
  it("con UN solo Escape, aunque haya un recorte hecho", async () => {
    // Hasta el 31 de agosto de 2026 el primer Escape solo borraba el recorte y hacía
    // falta un segundo para salir. Quien pulsa Escape encima de una captura quiere irse.
    const lienzo = await abrir();
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    expect(recorte()).toBe("300 × 200");

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "cancel_capture")).toBe(true),
    );
  });

  it("y también sin nada seleccionado", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "cancel_capture")).toBe(true),
    );
  });

  /** Lo que se haya mandado a capturar o a grabar, que después de un Escape tiene que ser nada. */
  const capturado = () =>
    llamadas.filter((l) => l.comando === "capture_still" || l.comando === "start_recording");

  it("Escape en medio del arrastre, al vuelo: al soltar el ratón no se captura nada", async () => {
    // Munito, 16-nov: «haz que cuando le des al Esc no se capture nada». El Escape cerraba el
    // overlay, pero el gesto seguía vivo, y al soltar el botón se copiaba el recorte.
    responde("overlay_bootstrap", {
      ...PAYLOAD,
      settings: { ...AJUSTES, captureFlow: "instant" } as Settings,
    });
    const lienzo = await abrir();
    fireEvent.pointerDown(lienzo, { clientX: 200, clientY: 300, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 500, clientY: 500, buttons: 1 });
    fireEvent.keyDown(window, { key: "Escape" });
    fireEvent.pointerUp(window, { clientX: 500, clientY: 500 });
    fireEvent.pointerCancel(window);

    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "cancel_capture")).toBe(true),
    );
    // Lo que se dispara al soltar es asíncrono: se le da su vuelta antes de mirar.
    await new Promise((r) => setTimeout(r, 50));
    expect(capturado()).toEqual([]);
  });

  it("Escape en medio del arrastre en vídeo: al soltar no empieza a grabar", async () => {
    const lienzo = await abrir();
    emite(EVENTS.overlayMode, { mode: "video", fullScreen: false, withToolbar: true });
    await waitFor(() => expect(screen.getByLabelText("Vídeo")).toHaveAttribute("aria-pressed", "true"));
    fireEvent.pointerDown(lienzo, { clientX: 200, clientY: 300, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 500, clientY: 500, buttons: 1 });
    fireEvent.keyDown(window, { key: "Escape" });
    fireEvent.pointerUp(window, { clientX: 500, clientY: 500 });

    await new Promise((r) => setTimeout(r, 50));
    expect(capturado()).toEqual([]);
  });

  it("cerrada desde Rust a medio arrastre (el Escape de otra pantalla), tampoco", async () => {
    responde("overlay_bootstrap", {
      ...PAYLOAD,
      settings: { ...AJUSTES, captureFlow: "instant" } as Settings,
    });
    const lienzo = await abrir();
    fireEvent.pointerDown(lienzo, { clientX: 200, clientY: 300, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 500, clientY: 500, buttons: 1 });
    emite(EVENTS.overlayHide, null);
    fireEvent.pointerUp(window, { clientX: 500, clientY: 500 });

    await new Promise((r) => setTimeout(r, 50));
    expect(capturado()).toEqual([]);
  });

  it("y la siguiente vez que se abre, soltar vuelve a capturar", async () => {
    const alVuelo = { ...PAYLOAD, settings: { ...AJUSTES, captureFlow: "instant" } as Settings };
    responde("overlay_bootstrap", alVuelo);
    const lienzo = await abrir();
    fireEvent.pointerDown(lienzo, { clientX: 200, clientY: 300, buttons: 1 });
    fireEvent.keyDown(window, { key: "Escape" });
    fireEvent.pointerUp(window, { clientX: 200, clientY: 300 });

    emite(EVENTS.overlayShow, { ...alVuelo, generation: 2 });
    await screen.findByLabelText("Foto");
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    await waitFor(() => expect(capturado().map((l) => l.comando)).toEqual(["capture_still"]));
  });

  it("si tras esconderse a medio arrastre solo llega el mousedown, el arrastre vale igual", async () => {
    // WebKitGTK no vio soltar el botón con la ventana escondida: la pulsación siguiente no le
    // da `pointerdown`, y sin esto esa captura no respondía al primer arrastre.
    const alVuelo = { ...PAYLOAD, settings: { ...AJUSTES, captureFlow: "instant" } as Settings };
    responde("overlay_bootstrap", alVuelo);
    const lienzo = await abrir();
    fireEvent.pointerDown(lienzo, { clientX: 200, clientY: 300, buttons: 1 });
    fireEvent.keyDown(window, { key: "Escape" });

    emite(EVENTS.overlayShow, { ...alVuelo, generation: 2 });
    await screen.findByLabelText("Foto");
    fireEvent.mouseDown(lienzo, { clientX: 200, clientY: 300, button: 0, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 500, clientY: 500, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 500, clientY: 500 });
    await waitFor(() => expect(capturado().map((l) => l.comando)).toEqual(["capture_still"]));
  });

  it("una pulsación normal trae los dos avisos y sigue siendo un solo gesto", async () => {
    const lienzo = await abrir();
    fireEvent.pointerDown(lienzo, { clientX: 200, clientY: 300, buttons: 1 });
    fireEvent.mouseDown(lienzo, { clientX: 200, clientY: 300, button: 0, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 500, clientY: 500, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 600, clientY: 600, buttons: 1 });
    fireEvent.mouseDown(lienzo, { clientX: 600, clientY: 600, button: 0, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 600, clientY: 600 });
    expect(recorte()).toBe("400 × 300");
  });

  it("el mousedown de un asa no empieza otro recorte encima de estirar el que hay", async () => {
    const lienzo = await abrir();
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    const asa = [...lienzo.querySelectorAll<HTMLElement>("div")]
      .filter((d) => d.style.cursor === "ew-resize")
      .sort((a, b) => parseFloat(b.style.left) - parseFloat(a.style.left))[0];
    fireEvent.pointerDown(asa, { clientX: 500, clientY: 400, buttons: 1 });
    fireEvent.mouseDown(asa, { clientX: 500, clientY: 400, button: 0, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 600, clientY: 400, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 600, clientY: 400 });
    expect(recorte()).toBe("400 × 200");
  });

  it("el mousedown de otro botón, o de dos a la vez, no empieza ningún recorte", async () => {
    const lienzo = await abrir();
    fireEvent.mouseDown(lienzo, { clientX: 200, clientY: 300, button: 2, buttons: 2 });
    fireEvent.pointerMove(window, { clientX: 500, clientY: 500, buttons: 2 });
    fireEvent.pointerUp(window, { clientX: 500, clientY: 500 });
    fireEvent.mouseDown(lienzo, { clientX: 200, clientY: 300, button: 0, buttons: 3 });
    fireEvent.pointerMove(window, { clientX: 500, clientY: 500, buttons: 3 });
    fireEvent.pointerUp(window, { clientX: 500, clientY: 500 });
    expect(recorte()).toBeNull();
  });
});

describe("la barra de arriba no se queda el sitio", () => {
  it("se puede empezar a recortar encima de ella", async () => {
    // La barra tapa una franja del centro de arriba, y ahí no había forma de arrastrar:
    // el botón se quedaba el gesto entero.
    const lienzo = await abrir();
    const boton = screen.getByLabelText("Foto");
    fireEvent.pointerDown(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y, buttons: 1 });
    fireEvent.pointerMove(lienzo, { clientX: EN_LA_BARRA.x + 260, clientY: 190, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: EN_LA_BARRA.x + 260, clientY: 190, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: EN_LA_BARRA.x + 260, clientY: 190 });

    expect(recorte()).toBe("260 × 160");
  });

  it("pero un clic seco sigue siendo del botón, y no recorta nada", async () => {
    await abrir();
    const boton = screen.getByLabelText("GIF");
    fireEvent.pointerDown(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y, buttons: 1 });
    fireEvent.pointerUp(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y });
    fireEvent.click(boton);

    expect(recorte()).toBeNull();
    expect(llamadas.some((l) => l.comando === "capture_still")).toBe(false);
  });

  it("y soltar el botón deja el ratón libre: moverlo después no dibuja solo", async () => {
    // El gesto queda apuntado hasta que se sepa qué es. Si no se borra al soltar, el
    // siguiente movimiento del ratón, ya sin botón, se ponía a recortar por su cuenta.
    const lienzo = await abrir();
    const boton = screen.getByLabelText("Foto");
    fireEvent.pointerDown(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y, buttons: 1 });
    fireEvent.pointerUp(boton, { clientX: EN_LA_BARRA.x, clientY: EN_LA_BARRA.y });

    fireEvent.pointerMove(lienzo, { clientX: 700, clientY: 600, buttons: 0 });
    expect(recorte()).toBeNull();
  });
});

describe("el interruptor de la barra de acciones", () => {
  it("apagado, soltar el recorte lo copia solo y no saca ninguna barra", async () => {
    // El camino entero: alguien pulsa el boton (en esta pantalla o en otra), llega el
    // evento, y la siguiente captura ya no pregunta nada.
    const lienzo = await abrir();
    emite(EVENTS.overlayMode, { mode: "still", fullScreen: false, withToolbar: false });

    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });

    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "capture_still")).toBe(true),
    );
    expect(screen.queryByLabelText("Copiar")).toBeNull();
  });

  it("Ctrl+Mayús+S guarda donde diga el usuario, y Ctrl+S en la carpeta de siempre", async () => {
    const lienzo = await abrir();
    emite(EVENTS.overlayMode, { mode: "still", fullScreen: false, withToolbar: true });
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    expect(await screen.findByLabelText("Guardar en…")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "s", ctrlKey: true, shiftKey: true });
    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "capture_still")).toBe(true),
    );
    const accion = (llamadas.find((l) => l.comando === "capture_still")?.args as { action: string }).action;
    expect(accion).toBe("save_as");
  });

  it("encendido, soltar el recorte saca la barra y no captura nada todavia", async () => {
    const lienzo = await abrir();
    emite(EVENTS.overlayMode, { mode: "still", fullScreen: false, withToolbar: true });

    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });

    expect(await screen.findByLabelText("Copiar")).toBeInTheDocument();
    expect(llamadas.some((l) => l.comando === "capture_still")).toBe(false);
  });

  it("al tocarlo se guarda en los ajustes, para que la proxima vez nazca asi", async () => {
    // Y se guarda con `set_capture_flow`, NO con `set_settings`: aquel reengancha los
    // atajos globales y el anillo, con la captura abierta delante.
    await abrir();
    fireEvent.click(screen.getByLabelText("Elegir qué hacer"));

    await waitFor(() => {
      const guardado = llamadas.find((l) => l.comando === "set_capture_flow");
      expect(guardado).toBeDefined();
      expect(guardado!.args).toEqual({ flow: "instant" });
    });
    expect(llamadas.some((l) => l.comando === "set_settings")).toBe(false);
  });

  it("la tecla B hace lo mismo que el boton", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "b" });
    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "set_capture_flow")).toBe(true),
    );
  });

  it("y los ajustes se abren desde la barra", async () => {
    await abrir();
    fireEvent.click(screen.getByLabelText("Ajustes"));
    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "open_settings")).toBe(true),
    );
  });
});

// Grabando se puede abrir el overlay para hacer una foto, pero otra grabación da «ya hay una
// grabación en curso»: la barra no la ofrece, ni con botón ni con tecla (revisión, 16-nov-2026).
describe("grabando, el overlay solo hace fotos", () => {
  it("no ofrece Vídeo ni GIF, y ni la V, ni la G, ni otra pantalla lo pasan a grabar", async () => {
    responde("overlay_bootstrap", { ...PAYLOAD, recording: true });
    const lienzo = await abrir();
    expect(screen.queryByLabelText("Vídeo")).toBeNull();
    expect(screen.queryByLabelText("GIF")).toBeNull();
    fireEvent.keyDown(window, { key: "v" });
    fireEvent.keyDown(window, { key: "g" });
    emite(EVENTS.overlayMode, { mode: "video", fullScreen: false, withToolbar: false });

    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });

    await waitFor(() => expect(llamadas.some((l) => l.comando === "capture_still")).toBe(true));
    expect(llamadas.some((l) => l.comando === "start_recording")).toBe(false);
  });
});

describe("la tecla que captura y abre el editor", () => {
  /**
   * Quien pulsa esa tecla ya sabe que va a anotar, asi que pararse en la barra a elegir
   * «editar» seria un clic de mas. Y manda sobre el perfil: aunque la barra este
   * encendida, esta captura no la ensenna.
   */
  it("al soltar el recorte se va al editor, sin pasar por la barra", async () => {
    responde("overlay_bootstrap", { ...PAYLOAD, intent: "edit" });
    const lienzo = await abrir();
    emite(EVENTS.overlayMode, { mode: "still", fullScreen: false, withToolbar: true });

    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });

    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "capture_still")).toBe(true),
    );
    const accion = (
      llamadas.find((l) => l.comando === "capture_still")?.args as { action: string }
    ).action;
    expect(accion).toBe("edit");
    expect(screen.queryByLabelText("Copiar")).toBeNull();
  });

  /** Y la tecla de siempre sigue sacando la barra: no se le ha quitado nada a nadie. */
  it("y el atajo de siempre sigue sacando la barra", async () => {
    const lienzo = await abrir();
    emite(EVENTS.overlayMode, { mode: "still", fullScreen: false, withToolbar: true });

    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });

    expect(await screen.findByLabelText("Copiar")).toBeInTheDocument();
    expect(llamadas.some((l) => l.comando === "capture_still")).toBe(false);
  });
});

describe("la lupa no se pone encima de lo que estas mirando", () => {
  /**
   * Munir, el 9 de septiembre de 2026: «cuando capturas una esquina de la pantalla no ves
   * lo que estás capturando».
   *
   * La lupa salía siempre abajo y a la derecha del cursor, y cerca de un borde solo se
   * recortaba para no salirse: en la esquina de abajo a la derecha acababa **encima del
   * propio cursor**, justo tapando el píxel que se está mirando. Que es para lo que sirve
   * la lupa.
   */
  const CON_LUPA: OverlayPayload = {
    ...PAYLOAD,
    settings: { ...AJUSTES, showMagnifier: true } as Settings,
  };

  /** El hueco que ocupa la lupa: el recuadro de 132 px, su barra de abajo y el margen. */
  const LUPA_ANCHO = 148;
  const LUPA_ALTO = 192;

  /** Dónde ha quedado la lupa, leyendo su estilo. */
  function lupa() {
    const marco = screen.getByText(/^#[0-9a-f]{6}$/i).closest("div.absolute") as HTMLElement;
    return { left: parseFloat(marco.style.left), top: parseFloat(marco.style.top) };
  }

  /** ¿El punto que se está mirando queda debajo de la lupa? */
  function tapa(punto: { x: number; y: number }) {
    const { left, top } = lupa();
    return (
      punto.x >= left &&
      punto.x <= left + LUPA_ANCHO &&
      punto.y >= top &&
      punto.y <= top + LUPA_ALTO
    );
  }

  beforeEach(() => {
    responde("overlay_bootstrap", CON_LUPA);
    // El doble de arriba solo sabe dibujar el fondo. La lupa pinta rejilla y recuadro, y
    // sin estos metodos el render revienta y la prueba falla por otra cosa.
    HTMLCanvasElement.prototype.getContext = (() => ({
      drawImage: () => {},
      getImageData: () => ({ data: new Uint8ClampedArray([0, 0, 0, 255]) }),
      clearRect: () => {},
      beginPath: () => {},
      moveTo: () => {},
      lineTo: () => {},
      stroke: () => {},
      strokeRect: () => {},
    })) as unknown as HTMLCanvasElement["getContext"];
  });

  it("en el centro se pone al lado, sin tapar nada", async () => {
    const lienzo = await abrir();
    const punto = { x: Math.round(ANCHO / 2), y: Math.round(ALTO / 2) };
    fireEvent.pointerMove(lienzo, { clientX: punto.x, clientY: punto.y });

    await waitFor(() => expect(lupa().left).toBeGreaterThan(punto.x));
    expect(tapa(punto)).toBe(false);
  });

  it("y en las cuatro esquinas TAMPOCO tapa el punto que miras", async () => {
    const lienzo = await abrir();
    const esquinas = [
      { x: 4, y: 4 },
      { x: ANCHO - 4, y: 4 },
      { x: 4, y: ALTO - 4 },
      { x: ANCHO - 4, y: ALTO - 4 },
    ];
    for (const punto of esquinas) {
      fireEvent.pointerMove(lienzo, { clientX: punto.x, clientY: punto.y });
      await waitFor(() => expect(lupa()).toBeDefined());
      expect({ esquina: punto, tapada: tapa(punto) }).toEqual({ esquina: punto, tapada: false });
    }
  });

  it("y nunca se sale de la pantalla", async () => {
    const lienzo = await abrir();
    for (const punto of [
      { x: 2, y: 2 },
      { x: ANCHO - 2, y: ALTO - 2 },
    ]) {
      fireEvent.pointerMove(lienzo, { clientX: punto.x, clientY: punto.y });
      await waitFor(() => expect(lupa()).toBeDefined());
      const { left, top } = lupa();
      expect(left).toBeGreaterThanOrEqual(0);
      expect(top).toBeGreaterThanOrEqual(0);
      expect(left + LUPA_ANCHO).toBeLessThanOrEqual(ANCHO);
      expect(top + LUPA_ALTO).toBeLessThanOrEqual(ALTO);
    }
  });
});

describe("la pantalla entera con la barra puesta", () => {
  /**
   * Munir, el 9 de septiembre de 2026: «cuando seleccionas todo con un click y tienes el
   * modo de la barra activado no aparece la barra para editar copiar etc».
   *
   * Con «pantalla entera» encendido, el clic se llevaba la pantalla al portapapeles y
   * cerraba, sin pasar por la barra, aunque el perfil elegido fuese el de la barra. Y
   * `Ctrl+A`, que hace lo mismo, sí la enseñaba: dos caminos al mismo sitio y cada uno
   * hacía una cosa.
   */
  /** Lo que mide la barra de acciones de alto, con sus botones. */
  const ALTO_BARRA = 52;

  const CON_BARRA: OverlayPayload = {
    ...PAYLOAD,
    settings: { ...AJUSTES, captureFlow: "toolbar" } as Settings,
  };

  beforeEach(() => responde("overlay_bootstrap", CON_BARRA));

  /**
   * Enciende «pantalla entera», que llega por el evento que comparten las pantallas, y
   * espera a que el estado esté puesto de verdad: con la pantalla entera el puntero pasa
   * a ser una mano, y eso se ve en el estilo del lienzo. Sin esperar a algo, el clic de
   * la prueba llega antes que el estado y se prueba otra cosa.
   */
  async function conPantallaEntera(withToolbar = true) {
    const lienzo = await abrir();
    emite(EVENTS.overlayMode, { mode: "still", fullScreen: true, withToolbar });
    await waitFor(() => expect(lienzo.style.cursor).toBe("pointer"));
    return lienzo;
  }

  it("el clic enseña la barra en vez de llevarse la pantalla y cerrar", async () => {
    const lienzo = await conPantallaEntera();
    fireEvent.pointerDown(lienzo, { clientX: 400, clientY: 300, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 400, clientY: 300 });

    await waitFor(() => expect(screen.getByLabelText("Copiar")).toBeDefined());
    expect(screen.getByLabelText("Editar")).toBeDefined();
    expect(llamadas.some((l) => l.comando === "capture_still")).toBe(false);
  });

  it("y la barra se ve entera, no colgando fuera de la pantalla", async () => {
    // Con el recorte ocupando la pantalla no cabe ni debajo ni encima, y la barra se
    // colocaba en top -10: existía en el DOM, con sus botones, pero no se veía ninguno.
    const lienzo = await conPantallaEntera();
    fireEvent.pointerDown(lienzo, { clientX: 400, clientY: 300, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 400, clientY: 300 });

    const barra = (await screen.findByLabelText("Copiar")).closest("div.absolute") as HTMLElement;
    const top = parseFloat(barra.style.top);
    // Volteada, `top` es el borde de ABAJO de la barra; si no, el de arriba.
    const volteada = barra.className.includes("-translate-y-full");
    const arriba = volteada ? top - ALTO_BARRA : top;
    expect({ arriba, abajo: arriba + ALTO_BARRA }).toEqual({
      arriba: expect.any(Number),
      abajo: expect.any(Number),
    });
    expect(arriba).toBeGreaterThanOrEqual(0);
    expect(arriba + ALTO_BARRA).toBeLessThanOrEqual(ALTO);
  });

  it("y lo que queda seleccionado es la pantalla entera", async () => {
    const lienzo = await conPantallaEntera();
    fireEvent.pointerDown(lienzo, { clientX: 400, clientY: 300, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 400, clientY: 300 });

    await waitFor(() => expect(recorte()).toBe(`${ANCHO} × ${ALTO}`));
  });

  it("pero sin barra, al vuelo, se la sigue llevando de un clic", async () => {
    responde("overlay_bootstrap", {
      ...PAYLOAD,
      settings: { ...AJUSTES, captureFlow: "instant" } as Settings,
    });
    const lienzo = await conPantallaEntera(false);
    fireEvent.pointerDown(lienzo, { clientX: 400, clientY: 300, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 400, clientY: 300 });

    await waitFor(() =>
      expect(llamadas.some((l) => l.comando === "capture_still")).toBe(true),
    );
  });
});

describe("solo la pantalla del ratón pinta la interfaz", () => {
  it("al irse el ratón a otra pantalla, esta se queda con el velo y nada más", async () => {
    // Un overlay por monitor: el que no tenía el ratón pintaba también la barra, la lupa
    // y el resalte, donde se quedó el cursor.
    const lienzo = await abrir();
    fireEvent.pointerLeave(lienzo);
    expect(screen.queryByLabelText("Foto")).toBeNull();

    fireEvent.pointerMove(lienzo, { clientX: 400, clientY: 300 });
    expect(screen.getByLabelText("Foto")).toBeTruthy();
  });
});

/** El botón de la proporción en la barra de arriba, que dice cuál va puesta en su `title`. */
function botonProporcion(): HTMLElement {
  return screen.getByLabelText("Proporción del recorte");
}

/** La medida que enseña la etiqueta, en números. */
function medida(): { ancho: number; alto: number } | null {
  const texto = recorte();
  if (!texto) return null;
  const [ancho, alto] = texto.split(" × ").map(Number);
  return { ancho, alto };
}

describe("la proporción fija (S27)", () => {
  it("la X a mitad de arrastrar pone 16:9 al momento, y se dice junto a la medida", async () => {
    const lienzo = await abrir();
    fireEvent.pointerDown(lienzo, { clientX: 200, clientY: 300, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 500, clientY: 500, buttons: 1 });
    expect(recorte()).toBe("300 × 200");

    fireEvent.keyDown(window, { key: "x" });
    // Sin mover el ratón: 300 de ancho y 200 de alto piden 19 y 22 escalones de 16 x 9, y
    // manda el que más ha avanzado para que el recuadro no se quede detrás del cursor.
    await waitFor(() => expect(recorte()).toBe("352 × 198"));
    expect(screen.getByText("· 16:9")).toBeInTheDocument();

    fireEvent.pointerMove(window, { clientX: 700, clientY: 420, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 700, clientY: 420 });
    const m = medida()!;
    expect(m.ancho * 9).toBe(m.alto * 16);
  });

  it("libre no se dice: la medida sale como siempre", async () => {
    const lienzo = await abrir();
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    expect(recorte()).toBe("300 × 200");
    expect(screen.queryByText(/^· /)).toBeNull();
  });

  it("pasa por las cinco en orden con la X, y con Mayús hacia atrás", async () => {
    await abrir();
    expect(botonProporcion().title).toBe("Proporción del recorte: libre · X");
    const vistas: string[] = [];
    for (let i = 0; i < 5; i++) {
      const antes = botonProporcion().title;
      fireEvent.keyDown(window, { key: "x" });
      await waitFor(() => expect(botonProporcion().title).not.toBe(antes));
      vistas.push(botonProporcion().title.replace("Proporción del recorte: ", "").replace(" · X", ""));
    }
    expect(vistas).toEqual(["16:9", "1:1", "9:16", "4:3", "libre"]);

    fireEvent.keyDown(window, { key: "X", shiftKey: true });
    await waitFor(() => expect(botonProporcion().title).toBe("Proporción del recorte: 4:3 · X"));
  });

  it("el botón de la barra hace lo mismo y enseña la que va puesta", async () => {
    await abrir();
    fireEvent.click(botonProporcion());
    await waitFor(() => expect(botonProporcion()).toHaveTextContent("16:9"));
    expect(botonProporcion()).toHaveAttribute("aria-pressed", "true");
  });

  it("con Ctrl o Alt la X no es suya", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "x", ctrlKey: true });
    fireEvent.keyDown(window, { key: "x", altKey: true });
    expect(botonProporcion().title).toBe("Proporción del recorte: libre · X");
  });

  it("un recorte ya hecho se pone a la nueva en su sitio y de tamaño parecido", async () => {
    const lienzo = await abrir();
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    fireEvent.keyDown(window, { key: "x" });
    // 300 x 200 son 60.000 píxeles: en 16:9, 320 x 180 con el mismo centro.
    await waitFor(() => expect(recorte()).toBe("320 × 180"));
  });

  it("las asas no le quitan la proporción, ni las de los lados ni las de las esquinas", async () => {
    const lienzo = await abrir();
    fireEvent.keyDown(window, { key: "x" });
    await waitFor(() => expect(botonProporcion()).toHaveTextContent("16:9"));
    arrastrar(lienzo, { x: 200, y: 200 }, { x: 520, y: 380 });
    expect(recorte()).toBe("320 × 180");

    const asas = (cursor: string) =>
      [...lienzo.querySelectorAll<HTMLElement>("div")].filter((d) => d.style.cursor === cursor);
    const masALaDerecha = (lista: HTMLElement[]) =>
      lista.sort((a, b) => parseFloat(b.style.left) - parseFloat(a.style.left))[0];

    fireEvent.pointerDown(masALaDerecha(asas("ew-resize")), { clientX: 520, clientY: 290, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 700, clientY: 333, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 700, clientY: 333 });
    let m = medida()!;
    expect(m.ancho * 9).toBe(m.alto * 16);
    expect(m.ancho).toBe(496);

    fireEvent.pointerDown(masALaDerecha(asas("nwse-resize")), { clientX: 696, clientY: 400, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 640, clientY: 461, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 640, clientY: 461 });
    m = medida()!;
    expect(m.ancho * 9).toBe(m.alto * 16);
  });

  it("Alt y las flechas la agrandan de escalón en escalón, sin perderla", async () => {
    const lienzo = await abrir();
    fireEvent.keyDown(window, { key: "x" });
    await waitFor(() => expect(botonProporcion()).toHaveTextContent("16:9"));
    arrastrar(lienzo, { x: 200, y: 200 }, { x: 520, y: 380 });
    fireEvent.keyDown(window, { key: "ArrowRight", altKey: true });
    await waitFor(() => expect(recorte()).toBe("336 × 189"));
    fireEvent.keyDown(window, { key: "ArrowUp", altKey: true, shiftKey: true });
    await waitFor(() => expect(recorte()).toBe("176 × 99"));
  });

  it("la medida solo dice la proporción cuando el recorte la tiene de verdad", async () => {
    // Ctrl+A coge la pantalla entera tal cual, con la proporción puesta o sin ella: la de
    // prueba mide 1024 x 768, que es 4:3 y no 16:9.
    await abrir();
    emite(EVENTO_PROPORCION, "16:9");
    await waitFor(() => expect(botonProporcion()).toHaveTextContent("16:9"));
    fireEvent.keyDown(window, { key: "a", ctrlKey: true });
    await waitFor(() => expect(recorte()).toBe(`${ANCHO} × ${ALTO}`));
    expect(screen.queryByText("· 16:9")).toBeNull();

    emite(EVENTO_PROPORCION, "4:3");
    await waitFor(() => expect(screen.getByText("· 4:3")).toBeInTheDocument());
  });

  it("con la captura con scroll no pinta nada: ni botón ni X", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "s" });
    await waitFor(() => expect(screen.queryByLabelText("Proporción del recorte")).toBeNull());
    fireEvent.keyDown(window, { key: "x" });
    fireEvent.keyDown(window, { key: "s" });
    await waitFor(() => expect(botonProporcion().title).toBe("Proporción del recorte: libre · X"));
  });

  it("si la barra de acciones sube encima del recorte, la medida se mete dentro y no queda tapada", async () => {
    // Un 9:16 alto pegado abajo: debajo no cabe la barra y se va encima, justo donde iba la
    // etiqueta. Visto en una ventana de verdad al 125 %.
    const lienzo = await abrir();
    emite(EVENTO_PROPORCION, "9:16");
    await waitFor(() => expect(botonProporcion()).toHaveTextContent("9:16"));
    arrastrar(lienzo, { x: 300, y: 200 }, { x: 600, y: ALTO - 10 });
    const barra = (await screen.findByLabelText("Copiar")).closest("div.absolute") as HTMLElement;
    expect(barra.className).toContain("-translate-y-full");
    const pieDeLaBarra = parseFloat(barra.style.top);
    const etiqueta = screen.getByText("· 9:16").closest("div") as HTMLElement;
    expect(parseFloat(etiqueta.style.top)).toBeGreaterThan(pieDeLaBarra);

    // Y con la barra debajo, la etiqueta sigue encima del recorte, como siempre.
    arrastrar(lienzo, { x: 300, y: 100 }, { x: 400, y: 200 });
    const otra = screen.getByText("· 9:16").closest("div") as HTMLElement;
    expect(parseFloat(otra.style.top)).toBeLessThan(100);
  });

  it("la eligen las otras pantallas y llega a esta", async () => {
    await abrir();
    emite(EVENTO_PROPORCION, "9:16");
    await waitFor(() => expect(botonProporcion()).toHaveTextContent("9:16"));
  });

  it("cada captura nueva empieza libre", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "x" });
    await waitFor(() => expect(botonProporcion()).toHaveTextContent("16:9"));
    emite(EVENTS.overlayShow, { ...PAYLOAD, generation: 2 });
    await waitFor(() => expect(botonProporcion().title).toBe("Proporción del recorte: libre · X"));
  });
});

/**
 * Lo que se lleva el usuario: la región que el overlay manda a Rust para guardar o grabar,
 * a 100, 125 y 150 %. Se apunta en `%TEMP%\winshotx-proporcion\regiones.json`, y
 * `cargo test --lib las_regiones_del_overlay_salen_exactas -- --ignored --nocapture` la
 * recorta y la guarda en PNG por el camino de «Guardar», y mira el archivo.
 */
describe("la proporción en lo que se guarda y se graba, a 100, 125 y 150 %", () => {
  const ESCALAS = [1, 1.25, 1.5];
  const FIJAS: Proporcion[] = ["16:9", "1:1", "9:16", "4:3"];
  const apuntadas: {
    escala: number;
    pantalla: { ancho: number; alto: number };
    proporcion: Proporcion;
    grabando: boolean;
    region: Rect;
  }[] = [];

  afterAll(() => {
    const carpeta = join(tmpdir(), "winshotx-proporcion");
    mkdirSync(carpeta, { recursive: true });
    writeFileSync(join(carpeta, "regiones.json"), JSON.stringify(apuntadas, null, 2));
  });

  function exacta(r: Rect, proporcion: Proporcion): boolean {
    const [a, b] = proporcion.split(":").map(Number);
    return r.width * b === r.height * a && r.width % a === 0;
  }

  /** Abre el overlay con la pantalla a esa escala: el congelado mide la ventana por ella. */
  async function abrirA(escala: number) {
    vi.stubGlobal(
      "createImageBitmap",
      vi.fn(async () => ({ width: Math.round(ANCHO * escala), height: Math.round(ALTO * escala) })),
    );
    return abrir();
  }

  /** La región que se mandó a Rust con ese comando, ya en píxeles físicos. */
  function regionDe(comando: string): Rect {
    const llamada = llamadas.find((l) => l.comando === comando);
    expect(llamada, `no se llamó a ${comando}`).toBeDefined();
    return (llamada!.args as { region: Rect }).region;
  }

  const casos = ESCALAS.flatMap((escala) => FIJAS.map((proporcion) => ({ escala, proporcion })));

  it.each(casos)("foto a $escala en $proporcion: Guardar recorta esa proporción exacta", async ({ escala, proporcion }) => {
    const lienzo = await abrirA(escala);
    emite(EVENTO_PROPORCION, proporcion);
    await waitFor(() => expect(botonProporcion()).toHaveTextContent(proporcion));
    // Puntos impares a propósito: al pasar a físicos con decimales es donde se perdía.
    arrastrar(lienzo, { x: 137, y: 211 }, { x: 611, y: 433 });
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await waitFor(() => expect(llamadas.some((l) => l.comando === "capture_still")).toBe(true));

    const r = regionDe("capture_still");
    const pantalla = { ancho: Math.round(ANCHO * escala), alto: Math.round(ALTO * escala) };
    expect({ r, exacta: exacta(r, proporcion) }).toEqual({ r, exacta: true });
    expect(r.x + r.width).toBeLessThanOrEqual(pantalla.ancho);
    expect(r.y + r.height).toBeLessThanOrEqual(pantalla.alto);
    // Lo que dice la etiqueta es lo que se guarda.
    expect(recorte()).toBe(`${r.width} × ${r.height}`);
    apuntadas.push({ escala, pantalla, proporcion, grabando: false, region: r });
  });

  it.each(casos)("vídeo a $escala en $proporcion: los dos lados pares y la proporción exacta", async ({ escala, proporcion }) => {
    const lienzo = await abrirA(escala);
    emite(EVENTS.overlayMode, { mode: "video", fullScreen: false, withToolbar: true });
    emite(EVENTO_PROPORCION, proporcion);
    await waitFor(() => expect(botonProporcion()).toHaveTextContent(proporcion));
    // Grabando, soltar el ratón ya empieza a grabar.
    arrastrar(lienzo, { x: 137, y: 211 }, { x: 611, y: 433 });
    await waitFor(() => expect(llamadas.some((l) => l.comando === "start_recording")).toBe(true));

    const r = regionDe("start_recording");
    expect({ r, pares: [r.width % 2, r.height % 2] }).toEqual({ r, pares: [0, 0] });
    expect(exacta(r, proporcion)).toBe(true);
    apuntadas.push({
      escala,
      pantalla: { ancho: Math.round(ANCHO * escala), alto: Math.round(ALTO * escala) },
      proporcion,
      grabando: true,
      region: r,
    });
  });

  it("un recorte hecho en foto y pasado a vídeo se ajusta a lados pares antes de grabar", async () => {
    const lienzo = await abrirA(1.25);
    emite(EVENTO_PROPORCION, "16:9");
    await waitFor(() => expect(botonProporcion()).toHaveTextContent("16:9"));
    // 21 escalones de 16 x 9: 336 x 189, impar de alto.
    arrastrar(lienzo, { x: 100, y: 100 }, { x: 100 + (16 * 21) / 1.25, y: 120 });
    expect(recorte()).toBe("336 × 189");

    emite(EVENTS.overlayMode, { mode: "video", fullScreen: false, withToolbar: true });
    await waitFor(() => expect(Number(recorte()!.split(" × ")[1]) % 2).toBe(0));
    fireEvent.keyDown(window, { key: "Enter" });
    await waitFor(() => expect(llamadas.some((l) => l.comando === "start_recording")).toBe(true));
    const r = regionDe("start_recording");
    expect([r.width % 2, r.height % 2]).toEqual([0, 0]);
    expect(exacta(r, "16:9")).toBe(true);
  });
});

/**
 * Una captura que empieza en una pantalla y sigue en la de al lado (plano dos pantallas).
 *
 * Con las pantallas de Munito medidas el 16 de noviembre de 2026: la principal en el origen,
 * la de la izquierda en x NEGATIVA y mas baja, y el portatil debajo al 125 %. Para la
 * vecina de la derecha, que el no tiene, una igual que la principal.
 *
 * Las regiones que se mandan a guardar al cruzar se apuntan en
 * `%TEMP%\winshotx-dos-pantallas\regiones.json`, para que la prueba de Rust las recorte de
 * punta a punta por el camino de «Guardar» y mire el PNG.
 */
describe("dos pantallas: el recorte sigue en la de al lado", () => {
  function monitor(id: number, x: number, y: number, width: number, height: number, scale = 1): MonitorInfo {
    return { id, label: `PANTALLA ${id}`, x, y, width, height, scale, isPrimary: id === 1 };
  }
  const PRINCIPAL = monitor(1, 0, 0, 1920, 1080);
  const IZQUIERDA = monitor(2, -1920, 483, 1920, 1080);
  const PORTATIL = monitor(3, 0, 1080, 1920, 1200, 1.25);
  const DERECHA = monitor(2, 1920, 0, 1920, 1080);
  const LAS_SUYAS = [PRINCIPAL, IZQUIERDA, PORTATIL];

  const apuntadas: { nombre: string; monitores: MonitorInfo[]; region: Rect }[] = [];
  afterAll(() => {
    const carpeta = join(tmpdir(), "winshotx-dos-pantallas");
    mkdirSync(carpeta, { recursive: true });
    writeFileSync(join(carpeta, "regiones.json"), JSON.stringify(apuntadas, null, 2));
  });

  /** El navegador de pruebas mide 1024 x 768: aqui cada pantalla mide lo suyo, y se devuelve. */
  const navegador = (window as unknown as {
    happyDOM: { setViewport(v: { width: number; height: number }): void };
  }).happyDOM;
  const VISTA_ANTES = { width: window.innerWidth, height: window.innerHeight };
  afterEach(() => navegador.setViewport(VISTA_ANTES));

  beforeEach(() => {
    // La lupa pinta rejilla y recuadro: con el doble de arriba solo, revienta el render.
    HTMLCanvasElement.prototype.getContext = (() => ({
      drawImage: () => {},
      getImageData: () => ({ data: new Uint8ClampedArray([0, 0, 0, 255]) }),
      clearRect: () => {},
      beginPath: () => {},
      moveTo: () => {},
      lineTo: () => {},
      stroke: () => {},
      strokeRect: () => {},
    })) as unknown as HTMLCanvasElement["getContext"];
  });

  /**
   * Abre el overlay de `yo` como lo abre Rust: su ventana mide la pantalla en CSS, su
   * congelado en fisicos (de ahi sale la escala), se llama `overlay-<id>` y el payload trae
   * todas las pantallas.
   */
  async function abrirEn(yo: MonitorInfo, todas: MonitorInfo[], ajustes: Partial<Settings> = {}) {
    navegador.setViewport({ width: Math.round(yo.width / yo.scale), height: Math.round(yo.height / yo.scale) });
    vi.stubGlobal("createImageBitmap", vi.fn(async () => ({ width: yo.width, height: yo.height })));
    ventana.etiqueta = `overlay-${yo.id}`;
    responde("overlay_bootstrap", {
      ...PAYLOAD,
      monitor: yo,
      monitores: todas,
      screenCount: todas.length,
      settings: { ...AJUSTES, ...ajustes } as Settings,
    } satisfies OverlayPayload);
    const { container } = render(<SelectionCanvas monitorId={yo.id} />);
    await waitFor(() => expect(container.querySelector("canvas")).not.toBeNull());
    expect(window.innerWidth).toBe(Math.round(yo.width / yo.scale));
    return container.firstElementChild as HTMLElement;
  }

  /** Lo que se le ha contado a una ventana sobre el recorte, en orden. */
  function avisosA(etiqueta: string): SeleccionCompartida[] {
    return emitidos
      .filter((e) => e.destino === etiqueta && e.evento === EVENTO_SELECCION)
      .map((e) => e.carga as SeleccionCompartida);
  }

  async function regionDe(comando: string): Promise<Rect> {
    await waitFor(() => expect(llamadas.some((l) => l.comando === comando)).toBe(true));
    return (llamadas.find((l) => l.comando === comando)!.args as { region: Rect }).region;
  }

  /** El recuadro azul de la seleccion, si se pinta en esta pantalla. */
  function recuadro(lienzo: HTMLElement): HTMLElement | null {
    return lienzo.querySelector<HTMLElement>('[class~="border-blue-500/90"]');
  }

  it("de la principal a la de la derecha: la vecina se entera y Ctrl+S guarda las dos", async () => {
    const lienzo = await abrirEn(PRINCIPAL, [PRINCIPAL, DERECHA]);
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 1920 + 150, y: 500 });

    const fisico = { x: 200, y: 300, width: 1870, height: 200 };
    expect(avisosA("overlay-2").at(-1)).toEqual({
      generacion: 1,
      emisor: 1,
      rect: fisico,
      fase: "quieta",
      cursor: null,
    });
    // La medida es la del recorte entero, y la barra sale aqui, que se lleva mas trozo.
    expect(recorte()).toBe("1870 × 200");
    expect(await screen.findByLabelText("Copiar")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    const r = await regionDe("capture_still");
    expect(r).toEqual(fisico);
    apuntadas.push({ nombre: "principal hacia la derecha, 100 %", monitores: [PRINCIPAL, DERECHA], region: r });
  });

  it("en medio del gesto, la vecina recibe el recorte y el raton en el cuadro siguiente", async () => {
    const lienzo = await abrirEn(PRINCIPAL, [PRINCIPAL, DERECHA]);
    fireEvent.pointerDown(lienzo, { clientX: 200, clientY: 300, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 1920 + 150, clientY: 500, buttons: 1 });
    await waitFor(() =>
      expect(avisosA("overlay-2").at(-1)).toEqual({
        generacion: 1,
        emisor: 1,
        rect: { x: 200, y: 300, width: 1870, height: 200 },
        fase: "dibujando",
        cursor: { x: 2070, y: 500 },
      }),
    );
    // Tres movimientos seguidos en el mismo cuadro son un solo aviso, con el ultimo.
    const antes = avisosA("overlay-2").length;
    for (const x of [2100, 2150, 2200]) {
      fireEvent.pointerMove(window, { clientX: x, clientY: 500, buttons: 1 });
    }
    await waitFor(() => expect(avisosA("overlay-2").at(-1)!.cursor).toEqual({ x: 2200, y: 500 }));
    expect(avisosA("overlay-2").length).toBe(antes + 1);
    fireEvent.pointerUp(window, { clientX: 2200, clientY: 500 });
    expect(avisosA("overlay-2").at(-1)).toMatchObject({ fase: "quieta", cursor: null });
  });

  it("pasarse 10 px del borde se queda en el borde (el iman); 150, cruza", async () => {
    const lienzo = await abrirEn(PRINCIPAL, [PRINCIPAL, DERECHA]);
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 1920 + 10, y: 500 });
    expect(recorte()).toBe("1720 × 200");
    expect(avisosA("overlay-2").at(-1)!.rect).toEqual({ x: 200, y: 300, width: 1720, height: 200 });

    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    const r = await regionDe("capture_still");
    expect(r.x + r.width).toBe(1920);
    apuntadas.push({ nombre: "iman: 10 px de mas se quedan en el borde", monitores: [PRINCIPAL, DERECHA], region: r });

    // Fuera del recorte de antes: dentro seria moverlo.
    arrastrar(lienzo, { x: 200, y: 600 }, { x: 1920 + 150, y: 800 });
    expect(recorte()).toBe("1870 × 200");
  });

  it("hacia la de la izquierda, con x negativa y mas baja que la principal", async () => {
    const lienzo = await abrirEn(PRINCIPAL, LAS_SUYAS);
    arrastrar(lienzo, { x: 300, y: 700 }, { x: -150, y: 900 });

    const fisico = { x: -150, y: 700, width: 450, height: 200 };
    expect(avisosA("overlay-2").at(-1)).toMatchObject({ rect: fisico, fase: "quieta" });
    // Al portatil tambien se le cuenta: la seleccion es una para todas.
    expect(avisosA("overlay-3").at(-1)).toMatchObject({ rect: fisico, fase: "quieta" });

    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    const r = await regionDe("capture_still");
    expect(r).toEqual(fisico);
    apuntadas.push({ nombre: "principal hacia la izquierda, x negativa", monitores: LAS_SUYAS, region: r });
  });

  it("desde el portatil al 125 %, hacia arriba a la principal", async () => {
    const lienzo = await abrirEn(PORTATIL, LAS_SUYAS);
    // 152 CSS hacia arriba son 190 fisicos dentro de la principal.
    arrastrar(lienzo, { x: 100, y: 200 }, { x: 400, y: -152 });

    const fisico = { x: 125, y: 890, width: 375, height: 440 };
    expect(avisosA("overlay-1").at(-1)).toMatchObject({ rect: fisico, fase: "quieta", emisor: 3 });

    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    const r = await regionDe("capture_still");
    expect(r).toEqual(fisico);
    apuntadas.push({ nombre: "portatil al 125 % hacia la principal", monitores: LAS_SUYAS, region: r });
  });

  it("y desde el portatil, 10 CSS hacia arriba (12,5 fisicos) se quedan en su borde", async () => {
    const lienzo = await abrirEn(PORTATIL, LAS_SUYAS);
    arrastrar(lienzo, { x: 100, y: 200 }, { x: 400, y: -10 });
    expect(avisosA("overlay-1").at(-1)!.rect).toEqual({ x: 125, y: 1080, width: 375, height: 250 });
  });

  it("dentro de una sola pantalla, un gesto son dos avisos: al empezar y al soltar", async () => {
    const lienzo = await abrirEn(PRINCIPAL, LAS_SUYAS);
    fireEvent.pointerDown(lienzo, { clientX: 200, clientY: 300, buttons: 1 });
    for (let x = 220; x <= 500; x += 20) {
      fireEvent.pointerMove(window, { clientX: x, clientY: 300 + x / 2, buttons: 1 });
    }
    fireEvent.pointerUp(window, { clientX: 500, clientY: 550 });
    // Lo que va por cuadro (rAF) tendria su oportunidad aqui: no tiene que haber ninguno.
    await new Promise((listo) => setTimeout(listo, 50));

    for (const etiqueta of ["overlay-2", "overlay-3"]) {
      expect(avisosA(etiqueta).map((a) => a.fase)).toEqual(["dibujando", "quieta"]);
    }
    expect(avisosA("overlay-2")[1].rect).toEqual({ x: 200, y: 300, width: 300, height: 250 });
    // Y a si misma no se manda nada.
    expect(avisosA("overlay-1")).toEqual([]);
  });

  it("en video no cruza: se para en el borde y graba eso", async () => {
    const lienzo = await abrirEn(PRINCIPAL, [PRINCIPAL, DERECHA]);
    emite(EVENTS.overlayMode, { mode: "video", fullScreen: false, withToolbar: true });
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 1920 + 150, y: 500 });
    expect(await regionDe("start_recording")).toEqual({ x: 200, y: 300, width: 1720, height: 200 });
  });

  it("con 16:9 tampoco: el recorte cabe en su pantalla", async () => {
    const lienzo = await abrirEn(PRINCIPAL, [PRINCIPAL, DERECHA]);
    emite(EVENTO_PROPORCION, "16:9");
    await waitFor(() => expect(botonProporcion()).toHaveTextContent("16:9"));
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 1920 + 150, y: 500 });
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    const r = await regionDe("capture_still");
    expect(r.x + r.width).toBeLessThanOrEqual(1920);
    expect(r.width * 9).toBe(r.height * 16);
  });

  it("estirar con un asa: 10 px de mas se quedan en el borde, 150 cruzan", async () => {
    const lienzo = await abrirEn(PRINCIPAL, [PRINCIPAL, DERECHA]);
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 600, y: 500 });
    const asaDerecha = () =>
      [...lienzo.querySelectorAll<HTMLElement>("div")]
        .filter((d) => d.style.cursor === "ew-resize")
        .sort((a, b) => parseFloat(b.style.left) - parseFloat(a.style.left))[0];

    fireEvent.pointerDown(asaDerecha(), { clientX: 600, clientY: 400, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 1930, clientY: 400, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 1930, clientY: 400 });
    expect(avisosA("overlay-2").at(-1)!.rect).toEqual({ x: 200, y: 300, width: 1720, height: 200 });

    fireEvent.pointerDown(asaDerecha(), { clientX: 1920, clientY: 400, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 2070, clientY: 400, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 2070, clientY: 400 });
    expect(avisosA("overlay-2").at(-1)).toMatchObject({
      rect: { x: 200, y: 300, width: 1870, height: 200 },
      fase: "quieta",
    });
  });

  it("mover un recorte de una pantalla no lo pasa a la de al lado", async () => {
    const lienzo = await abrirEn(PRINCIPAL, [PRINCIPAL, DERECHA]);
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    fireEvent.pointerDown(lienzo, { clientX: 300, clientY: 400, buttons: 1 });
    fireEvent.pointerMove(window, { clientX: 300 + 1700, clientY: 400, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 300 + 1700, clientY: 400 });
    expect(avisosA("overlay-2").at(-1)).toMatchObject({
      rect: { x: 1620, y: 300, width: 300, height: 200 },
      fase: "quieta",
    });
  });

  it("pasar a video con un recorte que cruza lo deja en el trozo de la que manda", async () => {
    const lienzo = await abrirEn(PRINCIPAL, [PRINCIPAL, DERECHA]);
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 1920 + 150, y: 500 });
    expect(recorte()).toBe("1870 × 200");

    emite(EVENTS.overlayMode, { mode: "video", fullScreen: false, withToolbar: true });
    await waitFor(() => expect(recorte()).toBe("1720 × 200"));
    // Y la vecina se entera: el recorte vuelve a ser uno solo.
    expect(avisosA("overlay-2").at(-1)).toMatchObject({
      rect: { x: 200, y: 300, width: 1720, height: 200 },
      fase: "quieta",
    });
    fireEvent.keyDown(window, { key: "Enter" });
    expect(await regionDe("start_recording")).toEqual({ x: 200, y: 300, width: 1720, height: 200 });
  });

  it("la que no manda no lo toca, y si se graba antes del aviso, sale de una sola pantalla", async () => {
    const lienzo = await abrirEn(PRINCIPAL, [PRINCIPAL, DERECHA]);
    // 120 de ancho aqui y 600 en la de la derecha: manda la derecha.
    arrastrar(lienzo, { x: 1800, y: 300 }, { x: 1920 + 600, y: 500 });
    expect(screen.queryByLabelText("Copiar")).toBeNull();
    const avisos = avisosA("overlay-2").length;

    emite(EVENTS.overlayMode, { mode: "video", fullScreen: false, withToolbar: true });
    await waitFor(() => expect(screen.getByLabelText("Vídeo")).toHaveAttribute("aria-pressed", "true"));
    expect(recorte()).toBe("720 × 200");
    expect(avisosA("overlay-2").length).toBe(avisos);

    fireEvent.keyDown(window, { key: "Enter" });
    expect(await regionDe("start_recording")).toEqual({ x: 1920, y: 300, width: 600, height: 200 });
  });

  describe("la vecina, montada como el portatil", () => {
    const CRUZA = { x: 300, y: 900, width: 800, height: 400 };
    const carga = (cambios: Partial<SeleccionCompartida> = {}): SeleccionCompartida => ({
      generacion: 1,
      emisor: 1,
      rect: CRUZA,
      fase: "quieta",
      cursor: null,
      ...cambios,
    });

    it("escucha solo lo suyo: con su etiqueta como destino", async () => {
      await abrirEn(PORTATIL, LAS_SUYAS);
      const suya = escuchas.find((e) => e.evento === EVENTO_SELECCION);
      expect(suya?.opciones?.target).toBe("overlay-3");
    });

    it("recibe el recorte y pinta su trozo, la barra, y Ctrl+S manda el recorte exacto", async () => {
      const lienzo = await abrirEn(PORTATIL, LAS_SUYAS);
      emite(EVENTO_SELECCION, carga(), "overlay-3");

      await waitFor(() => expect(recuadro(lienzo)).not.toBeNull());
      // En sus CSS al 125 %: empieza 144 por encima de su borde de arriba.
      const r = recuadro(lienzo)!;
      expect([r.style.left, r.style.top, r.style.width, r.style.height]).toEqual([
        "240px",
        "-144px",
        "640px",
        "320px",
      ]);
      // 800 x 220 aqui contra 800 x 180 en la principal: la barra es de esta.
      const barra = (await screen.findByLabelText("Copiar")).closest("div.absolute") as HTMLElement;
      expect(parseFloat(barra.style.top)).toBeGreaterThanOrEqual(0);
      // La medida va en la de la esquina de arriba a la izquierda, que es la principal.
      expect(recorte()).toBeNull();
      // Lo que llega no se reenvia.
      expect(emitidos.filter((e) => e.evento === EVENTO_SELECCION)).toEqual([]);

      fireEvent.keyDown(window, { key: "s", ctrlKey: true });
      const region = await regionDe("capture_still");
      expect(region).toEqual(CRUZA);
      apuntadas.push({ nombre: "la vecina (portatil al 125 %) guarda lo que cruza", monitores: LAS_SUYAS, region });
    });

    it("mientras la otra arrastra no ofrece la barra, y la lupa sale donde esta el raton", async () => {
      const lienzo = await abrirEn(PORTATIL, LAS_SUYAS, { showMagnifier: true });
      emite(EVENTO_SELECCION, carga({ fase: "dibujando", cursor: { x: 500, y: 1300 } }), "overlay-3");
      await waitFor(() => expect(recuadro(lienzo)).not.toBeNull());
      expect(screen.queryByLabelText("Copiar")).toBeNull();
      const lupa = screen.getByText(/^#[0-9a-f]{6}$/i).closest("div.absolute") as HTMLElement;
      // El raton en (500, 1300) fisicos es (400, 176) en sus CSS: la lupa, a su lado.
      expect(parseFloat(lupa.style.left)).toBeGreaterThan(400);

      emite(EVENTO_SELECCION, carga(), "overlay-3");
      expect(await screen.findByLabelText("Copiar")).toBeInTheDocument();
      expect(screen.queryByText(/^#[0-9a-f]{6}$/i)).toBeNull();
    });

    it("lo de otra captura, el eco propio y lo que va a otra ventana se ignoran", async () => {
      const lienzo = await abrirEn(PORTATIL, LAS_SUYAS);
      emite(EVENTO_SELECCION, carga({ generacion: 0 }), "overlay-3");
      emite(EVENTO_SELECCION, carga({ emisor: 3 }), "overlay-3");
      emite(EVENTO_SELECCION, carga(), "overlay-2");
      await new Promise((listo) => setTimeout(listo, 20));
      expect(recuadro(lienzo)).toBeNull();

      emite(EVENTO_SELECCION, carga(), "overlay-3");
      await waitFor(() => expect(recuadro(lienzo)).not.toBeNull());
    });

    it("lo que no asoma por aqui no se pinta, y el velo la tapa entera", async () => {
      const lienzo = await abrirEn(PORTATIL, LAS_SUYAS);
      emite(EVENTO_SELECCION, carga({ rect: { x: 200, y: 300, width: 300, height: 200 } }), "overlay-3");
      await new Promise((listo) => setTimeout(listo, 20));
      expect(recuadro(lienzo)).toBeNull();
      expect(lienzo.querySelector('[class~="bg-black/45"]')).not.toBeNull();
      expect(screen.queryByLabelText("Copiar")).toBeNull();
    });
  });
});

describe("revisión: la primera pulsación tras Esc cae encima de lo que ya hay", () => {
  // En ese estado WebKitGTK solo manda `mousedown`, sin `pointerdown`.
  const suelto = { button: 0, buttons: 1 };

  it("un mousedown suelto en Copiar no borra el recorte", async () => {
    const lienzo = await abrir();
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    expect(recorte()).toBe("300 × 200");
    fireEvent.mouseDown(lienzo, { clientX: 500, clientY: 500, button: 2, buttons: 2 });
    const copiar = screen.getAllByRole("button").find((b) => /Copiar/.test(b.textContent ?? "") || /Copiar/.test(b.getAttribute("aria-label") ?? "") || /Copiar/.test(b.getAttribute("title") ?? ""));
    if (!copiar) throw new Error("sin boton Copiar: " + screen.getAllByRole("button").map((b) => b.getAttribute("aria-label") ?? b.textContent).join(" | "));
    fireEvent.mouseDown(copiar, { clientX: 700, clientY: 560, ...suelto });
    expect(recorte()).toBe("300 × 200");
  });

  it("un mousedown suelto en un asa no borra el recorte", async () => {
    const lienzo = await abrir();
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    expect(recorte()).toBe("300 × 200");
    fireEvent.mouseDown(lienzo, { clientX: 500, clientY: 500, button: 2, buttons: 2 });
    const asa = [...lienzo.querySelectorAll<HTMLElement>("div")]
      .filter((d) => d.style.cursor === "ew-resize")
      .sort((a, b) => parseFloat(b.style.left) - parseFloat(a.style.left))[0];
    fireEvent.mouseDown(asa, { clientX: 504, clientY: 400, ...suelto });
    expect(recorte()).toBe("300 × 200");
  });

  it("y el asa pulsada así estira el recorte, no lo mueve", async () => {
    const lienzo = await abrir();
    arrastrar(lienzo, { x: 200, y: 300 }, { x: 500, y: 500 });
    fireEvent.mouseDown(lienzo, { clientX: 500, clientY: 500, button: 2, buttons: 2 });
    const asa = [...lienzo.querySelectorAll<HTMLElement>("div")]
      .filter((d) => d.style.cursor === "ew-resize")
      .sort((a, b) => parseFloat(b.style.left) - parseFloat(a.style.left))[0];
    fireEvent.mouseDown(asa, { clientX: 500, clientY: 400, ...suelto });
    fireEvent.pointerMove(window, { clientX: 600, clientY: 400, buttons: 1 });
    fireEvent.pointerUp(window, { clientX: 600, clientY: 400 });
    expect(recorte()).toBe("400 × 200");
  });
});
