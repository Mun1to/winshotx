/**
 * El overlay de selección entero.
 *
 * Aquí se comprueban los dos gestos que ninguna pieza suelta puede ver, porque empiezan
 * en un componente y acaban en otro: salir con Escape, y empezar a recortar justo donde
 * está la barra de arriba.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SelectionCanvas } from "./SelectionCanvas";
import { aplicarIdioma } from "../../lib/i18n";
import { emite, llamadas, responde } from "../../test/preparar";
import { EVENTS, type OverlayPayload, type Settings } from "../../lib/types";

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

const PAYLOAD: OverlayPayload = {
  monitor: {
    id: 1,
    label: "PANTALLA 1",
    x: 0,
    y: 0,
    width: ANCHO,
    height: ALTO,
    scale: 1,
    isPrimary: true,
  },
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
  const LUPA_ALTO = 168;

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
