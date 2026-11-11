/**
 * «Tapar datos» en un vídeo (S9, D4), con el editor entero: busca en la grabación, lo que
 * encuentra sale en la línea de tiempo, se dice qué se ha tapado, y «Guardar» lo manda.
 */
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { EditorApp } from "./EditorApp";
import { aplicarIdioma } from "../../lib/i18n";
import { emite, llamadas, responde } from "../../test/preparar";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    destroy: vi.fn(),
    setTitle: () => Promise.resolve(),
    onCloseRequested: () => Promise.resolve(() => {}),
  }),
}));

const TAPAS = [
  { desdeMs: 2000, hastaMs: 5000, x: 0.1, y: 0.2, ancho: 0.3, alto: 0.05 },
  { desdeMs: 2000, hastaMs: 5000, x: 0.1, y: 0.4, ancho: 0.2, alto: 0.05 },
];

function preparar() {
  responde("session_info", {
    id: "s1",
    region: { x: 0, y: 0, width: 1280, height: 800 },
    fps: 10,
    frameCount: 100,
    durationMs: 10_000,
    hasAudio: false,
    hasClicks: false,
    cursorBaked: false,
    format: "mp4",
    mp4Path: "C:\t\preview.mp4",
  });
  responde(
    "session_frames",
    Array.from({ length: 100 }, (_, i) => ({ index: i, timestampMs: i * 100, durationMs: 100, thumbPath: "" })),
  );
  responde("get_settings", { saveDirectory: "C:\capturas", theme: "oscuro", language: "es" });
  responde("ffmpeg_available", false);
  responde("session_studio", null);
}

beforeEach(() => {
  aplicarIdioma("es");
  llamadas.length = 0;
});

it("en un vídeo busca en la grabación entera, enseña los mosaicos y los guarda", async () => {
  preparar();
  let contestar: (valor: unknown) => void = () => {};
  responde("tapar_datos_video", null);
  const { container } = render(<EditorApp sessionId="s1" />);
  await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());

  // Que la respuesta tarde, para ver el aviso de por dónde va.
  responde("tapar_datos_video", new Promise((listo) => (contestar = listo)));
  fireEvent.click(screen.getByRole("button", { name: /Tapar datos/ }));
  await waitFor(() => expect(llamadas.some((l) => l.comando === "tapar_datos_video")).toBe(true));
  expect(llamadas.some((l) => l.comando === "tapar_datos")).toBe(false);
  act(() => emite("winshotx://tapar-video", { hechos: 40, total: 100 }));
  expect(await screen.findByText("Buscando datos personales en el vídeo… 40 %")).toBeInTheDocument();

  await act(async () => contestar({ tapas: TAPAS, resumen: [["correo", 1], ["teléfono", 1]], lecturas: 9, fotogramas: 100, ms: 800 }));
  expect(await screen.findByText("Tapado en el vídeo: correo (1), teléfono (1)")).toBeInTheDocument();
  expect(container.querySelectorAll("[data-tapa]")).toHaveLength(2);
  // La vista previa se va al primero, que es donde se ve lo que ha hecho.
  expect((container.querySelector("video") as HTMLVideoElement).currentTime).toBeCloseTo(2, 2);
  expect(container.querySelector("[data-capa-tapar]")?.getAttribute("data-puestas")).toBe("2");

  fireEvent.click(screen.getByRole("button", { name: "Guardar MP4" }));
  await waitFor(() => expect(llamadas.some((l) => l.comando === "export_media")).toBe(true));
  const peticion = (llamadas.find((l) => l.comando === "export_media")?.args as { request: { tapar: unknown } }).request;
  expect(peticion.tapar).toEqual(TAPAS);
});

it("si no encuentra nada, lo dice y no pone nada", async () => {
  preparar();
  responde("tapar_datos_video", { tapas: [], resumen: [], lecturas: 3, fotogramas: 100, ms: 200 });
  const { container } = render(<EditorApp sessionId="s1" />);
  await waitFor(() => expect(screen.queryByText("Preparando la sesión…")).toBeNull());
  fireEvent.click(screen.getByRole("button", { name: /Tapar datos/ }));
  expect(await screen.findByText("No he encontrado datos personales en el vídeo")).toBeInTheDocument();
  expect(container.querySelector("[data-pista-tapar]")).toBeNull();
});
