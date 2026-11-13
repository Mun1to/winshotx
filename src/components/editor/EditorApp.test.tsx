/**
 * El editor entero.
 *
 * Aquí se comprueban dos cosas que ninguna pieza suelta puede ver: que las capas de
 * dibujar caen EXACTAMENTE encima de la imagen, y que las teclas hacen lo que dicen.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EditorApp } from "./EditorApp";
import { aplicarIdioma } from "../../lib/i18n";
import { emite, llamadas, responde } from "../../test/preparar";

const destruir = vi.fn();
/** Los títulos que el editor le ha puesto a su ventana, en orden. */
const titulos: string[] = [];
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    destroy: destruir,
    setTitle: (titulo: string) => {
      titulos.push(titulo);
      return Promise.resolve();
    },
    onCloseRequested: () => Promise.resolve(() => {}),
  }),
}));

/** Una captura vertical, que es donde el desajuste de las capas se veía a simple vista. */
const VERTICAL = { x: 0, y: 0, width: 600, height: 1000 };

function preparar(region = VERTICAL, mp4Path: string | null = null, format = "still") {
  responde("session_info", {
    id: "s1",
    region,
    fps: 30,
    frameCount: 3,
    durationMs: 100,
    hasAudio: false,
    format,
    mp4Path,
  });
  responde("session_frames", [
    { index: 0, timestampMs: 0, durationMs: 33, thumbPath: "C:\\t\\0.png" },
    { index: 1, timestampMs: 33, durationMs: 33, thumbPath: "C:\\t\\1.png" },
    { index: 2, timestampMs: 66, durationMs: 33, thumbPath: "C:\\t\\2.png" },
  ]);
  responde("get_settings", { saveDirectory: "C:\\capturas", theme: "oscuro", language: "es" });
  responde("ffmpeg_available", false);
}

/** El editor carga por `invoke`, así que hay que esperar a que deje de decir «preparando». */
async function abrir(region = VERTICAL) {
  preparar(region);
  const vista = render(<EditorApp sessionId="s1" />);
  await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
  return vista;
}

/** Lo mismo, pero con lo rescatado del anillo: una grabación a la que le falta el vídeo. */
async function abrirRescatada() {
  preparar(VERTICAL, null, "mp4");
  const vista = render(<EditorApp sessionId="s1" />);
  await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
  return vista;
}

beforeEach(() => {
  aplicarIdioma("es");
  destruir.mockClear();
});

describe("dónde caen las capas de dibujar", () => {
  it("la imagen y las capas viven en una caja con la proporción de la captura", async () => {
    // La vista previa se ajusta con `object-contain`: en un hueco de otra proporción deja
    // franjas. Con las capas estiradas al hueco entero, sus coordenadas de 0 a 1 contaban
    // esas franjas como parte de la captura y lo dibujado salía desplazado en el archivo.
    const { container } = await abrir();
    const caja = container.querySelector<HTMLElement>('[style*="aspect-ratio"]');
    expect(caja).not.toBeNull();
    expect(caja!.style.aspectRatio.replace(/\s/g, "")).toBe("600/1000");
    expect(caja!.querySelector("img")).not.toBeNull();
    expect(caja!.querySelectorAll("svg").length).toBeGreaterThanOrEqual(1);
  });

  it("y la proporción sale de la captura, no de un número fijo", async () => {
    const { container } = await abrir({ x: 0, y: 0, width: 1920, height: 1080 });
    const caja = container.querySelector<HTMLElement>('[style*="aspect-ratio"]');
    expect(caja!.style.aspectRatio.replace(/\s/g, "")).toBe("1920/1080");
  });
});

describe("recortar la imagen", () => {
  it("la tecla C enciende el marco de recorte", async () => {
    const { container } = await abrir();
    expect(container.querySelector('svg[role="figure"]')).toBeNull();
    fireEvent.keyDown(window, { key: "c" });
    expect(container.querySelector('svg[role="figure"]')).not.toBeNull();
  });

  it("y volver a pulsarla lo apaga", async () => {
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "c" });
    fireEvent.keyDown(window, { key: "c" });
    expect(container.querySelector('svg[role="figure"]')).toBeNull();
  });

  it("Ctrl+C no lo enciende, que esa es la de copiar", async () => {
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "c", ctrlKey: true });
    expect(container.querySelector('svg[role="figure"]')).toBeNull();
  });

  it("encender el recorte apaga la herramienta de dibujar", async () => {
    // Las dos se arrastran encima de la imagen: con las dos puestas, un arrastre haría
    // dos cosas a la vez.
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "2" });
    fireEvent.keyDown(window, { key: "c" });
    expect(container.querySelector('[aria-label^="Rectángulo"]')?.getAttribute("aria-pressed")).toBe(
      "false",
    );
  });
});

describe("Escape sale de lo que se esté haciendo antes de cerrar", () => {
  // Cerrar el editor TIRA los fotogramas del disco. Un Escape sin querer mientras se
  // coloca un marco no puede llevarse por delante la grabación entera.
  it("con el recorte puesto, solo lo apaga", async () => {
    const { container } = await abrir();
    fireEvent.keyDown(window, { key: "c" });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(container.querySelector('svg[role="figure"]')).toBeNull();
    expect(destruir).not.toHaveBeenCalled();
  });

  it("con una herramienta elegida, solo la suelta", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "1" });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(destruir).not.toHaveBeenCalled();
  });

  it("y sin nada puesto, cierra y tira la sesión", async () => {
    await abrir();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(destruir).toHaveBeenCalled());
    expect(llamadas.some((l) => l.comando === "discard_session")).toBe(true);
  });
});

/**
 * El play.
 *
 * Quien mueve los fotogramas al reproducir es el `<video>`, así que sin vídeo el botón no
 * puede hacer nada. Lo que se rescata de «los últimos segundos» llega sin él (escribirlo
 * cuesta unos doce segundos para medio minuto y se hace por detrás), y eso el botón lo
 * tiene que DECIR en vez de quedarse quieto: pulsar y que no pase nada parece una app rota.
 */
describe("la vista previa del estudio", () => {
  it("una grabación lleva la capa que dibuja el zoom, el puntero y los aros", async () => {
    preparar(VERTICAL, "C:\\s\\preview.mp4", "video");
    responde("session_studio", { clics: [], teclas: [], cursor: [[0, 10, 10]] });
    const vista = render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    expect(vista.container.querySelector("[data-capa-estudio]")).not.toBeNull();
    // Y se pidió lo anotado al grabar, que es de donde sale todo.
    expect(llamadas.some((l) => l.comando === "session_studio")).toBe(true);
  });

  it("cuando Rust avisa de que las miniaturas ya están, se vuelven a pedir los fotogramas", async () => {
    preparar(VERTICAL, "C:\\s\\preview.mp4", "video");
    render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    const antes = llamadas.filter((l) => l.comando === "session_frames").length;
    emite("winshotx://session-thumbs", "s1");
    await waitFor(() =>
      expect(llamadas.filter((l) => l.comando === "session_frames").length).toBe(antes + 1),
    );
    // El aviso de otra sesión no hace nada.
    emite("winshotx://session-thumbs", "otra");
    expect(llamadas.filter((l) => l.comando === "session_frames").length).toBe(antes + 1);
  });

  it("una captura fija no la lleva: no tiene tiempo dentro", async () => {
    const vista = await abrir();
    expect(vista.container.querySelector("[data-capa-estudio]")).toBeNull();
    expect(llamadas.some((l) => l.comando === "session_studio")).toBe(false);
  });

  it("sin zoom no se pide la cámara; con zoom sí, con ese zoom", async () => {
    preparar(VERTICAL, "C:\\s\\preview.mp4", "video");
    responde("session_info", {
      id: "s1",
      region: VERTICAL,
      fps: 30,
      frameCount: 3,
      durationMs: 100,
      hasAudio: false,
      hasClicks: true,
      cursorBaked: false,
      format: "video",
      mp4Path: "C:\\s\\preview.mp4",
    });
    render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    expect(llamadas.some((l) => l.comando === "session_camera")).toBe(false);
    fireEvent.change(screen.getByLabelText("Acercarse a los clics"), { target: { value: "2" } });
    await waitFor(() => expect(llamadas.some((l) => l.comando === "session_camera")).toBe(true));
    const peticion = llamadas.find((l) => l.comando === "session_camera")?.args as { zoom: number };
    expect(peticion.zoom).toBe(2);
  });
});

describe("el botón de reproducir", () => {
  const play = () => screen.getByRole("button", { name: /Reproducir|Play/ });
  const hayPlay = () => screen.queryByRole("button", { name: /Reproducir|Play/ });

  it("sin vídeo todavía, no hay botón que prometa nada", async () => {
    // Un botón apagado es indistinguible de uno roto: lo que hay mientras se escribe la
    // vista previa es el aviso de que viene, no un play muerto esperando un clic.
    await abrirRescatada();
    expect(hayPlay()).toBeNull();
  });

  it("y con el vídeo escrito, se puede pulsar", async () => {
    preparar(VERTICAL, "C:\\t\\preview.mp4", "mp4");
    render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    expect(play()).toBeEnabled();
  });

  it("mientras no está, se dice que está en camino", async () => {
    // Esta frase es toda la diferencia entre «espera unos segundos» y «esto no funciona».
    await abrirRescatada();
    expect(screen.getByText("Preparando el vídeo…")).toBeInTheDocument();
  });

  it("cuando avisan de que ya está, se enciende solo", async () => {
    await abrirRescatada();
    expect(hayPlay()).toBeNull();

    responde("session_info", {
      id: "s1",
      region: VERTICAL,
      fps: 30,
      frameCount: 3,
      durationMs: 100,
      hasAudio: false,
      format: "mp4",
      mp4Path: "C:\\t\\preview.mp4",
    });
    emite("winshotx://session-preview", { sessionId: "s1", porCiento: 100, listo: true, fallida: false });

    await waitFor(() => expect(play()).toBeEnabled());
    expect(screen.queryByText("Preparando el vídeo…")).toBeNull();
  });

  it("el aviso de otra sesión no lo toca", async () => {
    await abrirRescatada();
    emite("winshotx://session-preview", { sessionId: "otra", porCiento: 100, listo: true, fallida: false });
    expect(hayPlay()).toBeNull();
    expect(screen.getByText("Preparando el vídeo…")).toBeInTheDocument();
  });

  it("y si la vista previa no sale, lo dice en vez de esperar para siempre", async () => {
    await abrirRescatada();
    emite("winshotx://session-preview", { sessionId: "s1", porCiento: 0, listo: false, fallida: true });
    await waitFor(() => expect(screen.getByText("Sin vista previa")).toBeInTheDocument());
    // Y se dice lo que SÍ se puede hacer, que recortar y exportar siguen funcionando.
    expect(screen.getByTitle(/recortar y exportar igual/)).toBeInTheDocument();
  });

  it("y mientras se escribe, dice por dónde va", async () => {
    await abrirRescatada();
    emite("winshotx://session-preview", { sessionId: "s1", porCiento: 45, listo: false, fallida: false });
    await waitFor(() => expect(screen.getByText("Preparando el vídeo… 45%")).toBeInTheDocument());
    expect(hayPlay()).toBeNull();
  });

  it("una captura fija no enseña mandos de reproducción", async () => {
    // Una captura es un fotograma y ya: ni se reproduce, ni tiene principio y final que
    // marcar. Cuatro botones apagados que hay que leer para descartarlos son peor que
    // ninguno.
    await abrir();
    expect(screen.queryByText("Preparando el vídeo…")).toBeNull();
    expect(hayPlay()).toBeNull();
    expect(screen.queryByRole("button", { name: /Inicio|Start/ })).toBeNull();
    // Lo que sí sigue estando en una captura es encuadrar, que recorta la IMAGEN.
    expect(screen.getByRole("button", { name: /Encuadrar|Crop/ })).toBeInTheDocument();
  });

  it("una captura tampoco enseña la tira de fotogramas", async () => {
    // Una captura es UN fotograma: la tira se quedaba con una miniatura sola y un
    // «Fotograma 1 de 1» debajo, ocupando cien píxeles de los que mide la imagen.
    await abrir();
    expect(screen.queryByText(/Fotograma 1 de/)).toBeNull();
  });

  it("y si el aviso se pierde, el play se enciende igual", async () => {
    // El aviso viaja por un evento, y uno que se emite antes de que esta ventana monte su
    // escucha no lo recoge nadie. Paso de verdad el 17 de octubre de 2026, en cuanto la
    // vista previa empezo a tardar un segundo en vez de diez: el editor se quedo con
    // «preparando el video… 100%» y el MP4 ya estaba escrito en el disco.
    await abrirRescatada();
    expect(hayPlay()).toBeNull();

    // El backend ya tiene el video, pero aqui no llega ningun evento: solo la pregunta
    // que el editor hace por su cuenta cada dos segundos.
    responde("session_info", {
      id: "s1",
      region: VERTICAL,
      fps: 30,
      frameCount: 3,
      durationMs: 100,
      hasAudio: false,
      format: "mp4",
      mp4Path: "C:\\t\\preview.mp4",
    });

    await waitFor(() => expect(hayPlay()).not.toBeNull(), { timeout: 6000 });
  }, 10000);

  it("pulsarlo le habla al vídeo, en el mismo clic", async () => {
    // El `play()` tiene que salir del clic: lanzado desde un efecto de después llega ya
    // fuera del gesto de la persona, y ahí un navegador puede rechazarlo sin más.
    const suena = vi.spyOn(window.HTMLMediaElement.prototype, "play").mockResolvedValue();
    preparar(VERTICAL, "C:\\t\\preview.mp4", "mp4");
    render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());

    fireEvent.click(play());
    expect(suena).toHaveBeenCalled();
    suena.mockRestore();
  });

  it("y si el vídeo se niega, se cuenta el motivo", async () => {
    const suena = vi
      .spyOn(window.HTMLMediaElement.prototype, "play")
      .mockRejectedValue("NotAllowedError");
    preparar(VERTICAL, "C:\\t\\preview.mp4", "mp4");
    render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());

    fireEvent.click(play());
    await waitFor(() => expect(screen.getByText(/NotAllowedError/)).toBeInTheDocument());
    suena.mockRestore();
  });

  it("el reloj dice dónde estás, y debajo cuánto dura", async () => {
    // Un reloj suelto no distingue el principio de algo largo del final de algo corto.
    prepararLarga();
    render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    expect(screen.getByText("0:00.0")).toBeInTheDocument();
    expect(screen.getByText("0:10.0")).toBeInTheDocument();
  });
});

/**
 * Diez segundos de vídeo, a diez fotogramas por segundo, con su vista previa escrita.
 * Es la grabación sobre la que se prueba recortar desde la línea de tiempo.
 */
function prepararLarga() {
  responde("session_info", {
    id: "s1",
    region: { x: 0, y: 0, width: 1280, height: 800 },
    fps: 10,
    frameCount: 100,
    durationMs: 10_000,
    hasAudio: true,
    hasClicks: false,
    cursorBaked: false,
    format: "mp4",
    mp4Path: "C:\t\preview.mp4",
  });
  responde(
    "session_frames",
    Array.from({ length: 100 }, (_, i) => ({
      index: i,
      timestampMs: i * 100,
      durationMs: 100,
      thumbPath: `C:\t\${i}.png`,
    })),
  );
  responde("get_settings", { saveDirectory: "C:\capturas", theme: "oscuro", language: "es" });
  responde("ffmpeg_available", false);
  responde("session_studio", null);
}

describe("recortar desde la línea de tiempo", () => {
  /** La tira empieza en x = 100 y mide 1000 px: cada píxel son diez milisegundos. */
  const x = (parte: number) => 100 + parte * 1000;
  const original = HTMLElement.prototype.getBoundingClientRect;

  beforeEach(() => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      if (this.hasAttribute("data-pista")) {
        return { left: 100, top: 0, width: 1000, height: 56, right: 1100, bottom: 56, x: 100, y: 0, toJSON() {} } as DOMRect;
      }
      return original.call(this);
    });
  });

  async function abrirLarga() {
    prepararLarga();
    const vista = render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    return vista;
  }

  /** Arrastra un asa hasta esa parte de la tira, como haría el ratón. */
  function arrastrar(asa: RegExp, parte: number) {
    fireEvent.pointerDown(screen.getByRole("slider", { name: asa }));
    fireEvent.pointerMove(window, { clientX: x(parte) });
    fireEvent.pointerUp(window);
  }

  it("lo que se recorta con las asas es lo que se guarda, al milisegundo", async () => {
    await abrirLarga();
    arrastrar(/Principio/, 0.25);
    arrastrar(/Final/, 0.7);
    expect(screen.getByText("Te quedas con 0:04.5 de 0:10.0")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Guardar MP4" }));
    await waitFor(() => expect(llamadas.some((l) => l.comando === "export_media")).toBe(true));
    const peticion = (llamadas.find((l) => l.comando === "export_media")?.args as {
      request: Record<string, unknown>;
    }).request;
    // Los fotogramas viajan también, para quien todavía los use: el 25 empieza en 2,5 s
    // y el 69 es el último que empieza antes de los 7.
    expect(peticion).toMatchObject({ desdeMs: 2_500, hastaMs: 7_000, from: 25, to: 69 });
  });

  it("mover un asa enseña en la vista previa el sitio exacto del corte", async () => {
    const { container } = await abrirLarga();
    const video = container.querySelector("video") as HTMLVideoElement;
    arrastrar(/Principio/, 0.25);
    expect(video.currentTime).toBeCloseTo(2.5, 2);
    // Del final se enseña lo último que se queda, no lo primero que se va.
    arrastrar(/Final/, 0.7);
    expect(video.currentTime).toBeCloseTo(6.999, 2);
  });

  it("el play empieza por el principio de lo que se queda", async () => {
    const suena = vi.spyOn(window.HTMLMediaElement.prototype, "play").mockResolvedValue();
    const { container } = await abrirLarga();
    const video = container.querySelector("video") as HTMLVideoElement;
    arrastrar(/Principio/, 0.3);
    video.currentTime = 0;
    fireEvent.click(screen.getByRole("button", { name: "Reproducir" }));
    expect(video.currentTime).toBeCloseTo(3, 2);
    expect(suena).toHaveBeenCalled();
    suena.mockRestore();
  });

  it("I y O cortan donde está la vista previa", async () => {
    await abrirLarga();
    for (let i = 0; i < 5; i++) fireEvent.keyDown(window, { key: "ArrowRight" });
    fireEvent.keyDown(window, { key: "i" });
    expect(screen.getByText("Te quedas con 0:09.5 de 0:10.0")).toBeInTheDocument();
    for (let i = 0; i < 20; i++) fireEvent.keyDown(window, { key: "ArrowRight" });
    fireEvent.keyDown(window, { key: "o" });
    expect(screen.getByText("Te quedas con 0:02.0 de 0:10.0")).toBeInTheDocument();
  });

  it("«Usar el vídeo entero» deshace el recorte", async () => {
    await abrirLarga();
    arrastrar(/Principio/, 0.5);
    fireEvent.click(screen.getByRole("button", { name: "Usar el vídeo entero" }));
    expect(screen.getByText("Arrastra los bordes azules para recortar")).toBeInTheDocument();
  });

  it("y la ventana dice en su título lo que dura lo que se queda", async () => {
    await abrirLarga();
    arrastrar(/Final/, 0.4);
    // Es lo que se ve en la barra de tareas y con Alt+Tab.
    await waitFor(() => expect(titulos.at(-1)).toContain("0:04.00"));
  });
});

describe("el marco del estudio en la vista previa", () => {
  it("al elegir un fondo, la vista previa lo enseña alrededor de la imagen, con sus esquinas", async () => {
    // Sin medidas de pantalla el aire sale a cero: se le da al editor el hueco de una
    // ventana de verdad.
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      left: 0, top: 0, width: 1200, height: 700, right: 1200, bottom: 700, x: 0, y: 0, toJSON() {},
    } as DOMRect);
    prepararLarga();
    const { container } = render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    expect(container.querySelector("[data-marco]")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Océano" }));
    const marco = await waitFor(() => {
      const el = container.querySelector('[data-marco="oceano"]') as HTMLElement | null;
      expect(el).not.toBeNull();
      return el!;
    });
    expect(marco.style.background).toContain("linear-gradient");
    expect(marco.style.padding).not.toBe("");
    const imagen = marco.firstElementChild as HTMLElement;
    expect(imagen.style.borderRadius).not.toBe("");
    fireEvent.click(screen.getByRole("button", { name: "Sin fondo" }));
    await waitFor(() => expect(container.querySelector("[data-marco]")).toBeNull());
    vi.restoreAllMocks();
  });
});

// S11 (pistas)
describe("el sonido del PC y la voz en la vista previa", () => {
  async function abrirConPistas(pistas: Record<string, unknown>) {
    preparar(VERTICAL, "C:\\t\\preview.mp4", "mp4");
    responde("session_info", {
      id: "s1",
      region: VERTICAL,
      fps: 30,
      frameCount: 3,
      durationMs: 100,
      hasAudio: true,
      format: "mp4",
      mp4Path: "C:\\t\\preview.mp4",
      ...pistas,
    });
    const vista = render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    return vista;
  }

  it("cada pista suena en su <audio>, y el vídeo, que las trae sumadas, se calla", async () => {
    const { container } = await abrirConPistas({
      sonidoPc: "C:\\t\\sonido-pc.wav",
      microfono: "C:\\t\\microfono.wav",
    });
    await waitFor(() => expect(container.querySelectorAll("audio")).toHaveLength(2));
    expect(container.querySelector("audio[data-sonido=pc]")?.getAttribute("src")).toBe("asset://C:\\t\\sonido-pc.wav");
    expect(container.querySelector("audio[data-sonido=microfono]")?.getAttribute("src")).toBe(
      "asset://C:\\t\\microfono.wav",
    );
    expect(container.querySelector("video")!.volume).toBe(0);
  });

  it("una grabación de antes, con todo junto, suena por el vídeo como siempre", async () => {
    const { container } = await abrirConPistas({});
    expect(container.querySelectorAll("audio")).toHaveLength(0);
    expect(container.querySelector("video")!.volume).toBe(1);
  });

  it("si una pista no se puede abrir, vuelve a sonar el vídeo en vez de quedarse mudo", async () => {
    const aviso = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { container } = await abrirConPistas({ sonidoPc: "C:\\t\\sonido-pc.wav" });
    const pista = await waitFor(() => {
      const a = container.querySelector("audio");
      expect(a).not.toBeNull();
      return a!;
    });
    fireEvent.error(pista);
    await waitFor(() => expect(container.querySelectorAll("audio")).toHaveLength(0));
    expect(container.querySelector("video")!.volume).toBe(1);
    expect(aviso).toHaveBeenCalled();
    aviso.mockRestore();
  });
});

describe("silenciar la vista previa", () => {
  async function abrirVideo(pistas: Record<string, unknown> = {}) {
    preparar(VERTICAL, "C:\t\preview.mp4", "mp4");
    responde("session_info", {
      id: "s1",
      region: VERTICAL,
      fps: 30,
      frameCount: 3,
      durationMs: 100,
      hasAudio: true,
      format: "mp4",
      mp4Path: "C:\t\preview.mp4",
      ...pistas,
    });
    const vista = render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    return vista;
  }

  beforeEach(() => localStorage.removeItem("winshotx.vistaCallada"));

  it("el botón de al lado del play la calla y la vuelve a oír, cambiando de nombre", async () => {
    const { container } = await abrirVideo();
    const video = container.querySelector("video")!;
    expect(video.volume).toBe(1);

    fireEvent.click(screen.getByRole("button", { name: "Silenciar la vista previa (M)" }));
    expect(video.volume).toBe(0);
    const oir = screen.getByRole("button", { name: "Oír la vista previa (M)" });
    expect(oir).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(oir);
    expect(video.volume).toBe(1);
  });

  it("la M hace lo mismo, como en cualquier reproductor", async () => {
    const { container } = await abrirVideo();
    fireEvent.keyDown(window, { key: "m" });
    expect(container.querySelector("video")!.volume).toBe(0);
    fireEvent.keyDown(window, { key: "M" });
    expect(container.querySelector("video")!.volume).toBe(1);
  });

  it("solo calla lo que se oye aquí: lo que se guarda sigue con su volumen", async () => {
    await abrirVideo();
    fireEvent.click(screen.getByRole("button", { name: "Silenciar la vista previa (M)" }));
    fireEvent.click(screen.getByRole("button", { name: "Guardar MP4" }));
    await waitFor(() => expect(llamadas.some((l) => l.comando === "export_media")).toBe(true));
    const peticion = (llamadas.find((l) => l.comando === "export_media")?.args as {
      request: Record<string, unknown>;
    }).request;
    expect(peticion.volume).toBe(1);
  });

  it("con el PC y la voz por separado, calla las dos pistas", async () => {
    const { container } = await abrirVideo({ sonidoPc: "C:\t\sonido-pc.wav", microfono: "C:\t\microfono.wav" });
    await waitFor(() => expect(container.querySelectorAll("audio")).toHaveLength(2));
    fireEvent.click(screen.getByRole("button", { name: "Silenciar la vista previa (M)" }));
    await waitFor(() => {
      for (const pista of container.querySelectorAll("audio")) expect(pista.volume).toBe(0);
    });
  });

  it("se recuerda para el vídeo siguiente", async () => {
    const primero = await abrirVideo();
    fireEvent.click(screen.getByRole("button", { name: "Silenciar la vista previa (M)" }));
    primero.unmount();

    const { container } = await abrirVideo();
    expect(screen.getByRole("button", { name: "Oír la vista previa (M)" })).toBeInTheDocument();
    expect(container.querySelector("video")!.volume).toBe(0);
  });
});
