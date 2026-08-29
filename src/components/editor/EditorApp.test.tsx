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
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    destroy: destruir,
    setTitle: () => Promise.resolve(),
    onCloseRequested: () => Promise.resolve(() => {}),
  }),
}));

/** Una captura vertical, que es donde el desajuste de las capas se veía a simple vista. */
const VERTICAL = { x: 0, y: 0, width: 600, height: 1000 };

function preparar(region = VERTICAL, mp4Path: string | null = null) {
  responde("session_info", {
    id: "s1",
    region,
    fps: 30,
    frameCount: 3,
    durationMs: 100,
    hasAudio: false,
    format: "still",
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
describe("el botón de reproducir", () => {
  const play = () => screen.getByRole("button", { name: /Reproducir|Play/ });

  it("sin vídeo todavía, no promete lo que no puede hacer", async () => {
    await abrir();
    expect(play()).toBeDisabled();
    expect(play()).toHaveAttribute("title", "preparando la reproducción…");
  });

  it("y con el vídeo escrito, se puede pulsar", async () => {
    preparar(VERTICAL, "C:\\t\\preview.mp4");
    render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
    expect(play()).toBeEnabled();
  });

  it("mientras no está, se dice que está en camino", async () => {
    // Un botón apagado y callado es indistinguible de uno roto. Esta frase es toda la
    // diferencia entre «espera unos segundos» y «esto no funciona».
    await abrir();
    expect(screen.getByText("Preparando la reproducción…")).toBeInTheDocument();
  });

  it("cuando avisan de que ya está, se enciende solo", async () => {
    await abrir();
    expect(play()).toBeDisabled();

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
    expect(screen.queryByText("Preparando la reproducción…")).toBeNull();
  });

  it("el aviso de otra sesión no lo toca", async () => {
    await abrir();
    emite("winshotx://session-preview", { sessionId: "otra", porCiento: 100, listo: true, fallida: false });
    expect(play()).toBeDisabled();
  });

  it("y si la vista previa no sale, lo dice en vez de esperar para siempre", async () => {
    await abrir();
    emite("winshotx://session-preview", { sessionId: "s1", porCiento: 0, listo: false, fallida: true });
    await waitFor(() =>
      expect(screen.getByText("No se ha podido preparar la reproducción")).toBeInTheDocument(),
    );
  });

  it("y mientras se escribe, dice por dónde va", async () => {
    await abrir();
    emite("winshotx://session-preview", { sessionId: "s1", porCiento: 45, listo: false, fallida: false });
    await waitFor(() =>
      expect(screen.getByText("Preparando la reproducción… 45%")).toBeInTheDocument(),
    );
    expect(play()).toBeDisabled();
  });

  it("pulsarlo le habla al vídeo, en el mismo clic", async () => {
    // El `play()` tiene que salir del clic: lanzado desde un efecto de después llega ya
    // fuera del gesto de la persona, y ahí un navegador puede rechazarlo sin más.
    const suena = vi.spyOn(window.HTMLMediaElement.prototype, "play").mockResolvedValue();
    preparar(VERTICAL, "C:\\t\\preview.mp4");
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
    preparar(VERTICAL, "C:\\t\\preview.mp4");
    render(<EditorApp sessionId="s1" />);
    await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());

    fireEvent.click(play());
    await waitFor(() => expect(screen.getByText(/NotAllowedError/)).toBeInTheDocument());
    suena.mockRestore();
  });
});
