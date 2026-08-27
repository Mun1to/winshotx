/**
 * El panel de exportar.
 *
 * Es la ultima pantalla antes de que salga el archivo, y casi todo lo que decide vive en
 * lo que se le manda a Rust, no en lo que se ve. Una casilla que se queda pegada del
 * formato anterior no se nota mirando: se nota cuando alguien abre el archivo.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { ExportPanel } from "./ExportPanel";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas } from "../../test/preparar";
import type { SessionInfo } from "../../lib/types";

const GRABACION: SessionInfo = {
  id: "s1",
  region: { x: 0, y: 0, width: 800, height: 600 },
  fps: 30,
  frameCount: 90,
  durationMs: 3000,
  hasAudio: true,
  format: "video",
  mp4Path: null,
};

function pintar(session: Partial<SessionInfo> = {}) {
  return render(
    <ExportPanel
      anotaciones={[]}
      session={{ ...GRABACION, ...session }}
      inIndex={10}
      outIndex={80}
      currentIndex={42}
      fpsMax={60}
      hasFfmpeg={false}
      saveDirectory="C:\\capturas"
    />,
  );
}

/** Lo que se le mando a Rust en la ultima exportacion. */
function loExportado() {
  const llamada = [...llamadas].reverse().find((l) => l.comando === "export_media");
  return (llamada?.args as { request: Record<string, unknown> })?.request;
}

beforeEach(() => aplicarIdioma("es"));

describe("elegir el formato", () => {
  it("una grabacion empieza en MP4 y una captura suelta en PNG", () => {
    const { unmount } = pintar();
    expect(screen.getByText("MP4").className).toContain("bg-white/15");
    unmount();
    pintar({ format: "still", frameCount: 1 });
    expect(screen.getByText("PNG").className).toContain("bg-white/15");
  });

  it("estan los cuatro, GIF, MP4, PNG y JPG", () => {
    pintar();
    for (const f of ["GIF", "MP4", "PNG", "JPG"]) {
      expect(screen.getByText(f)).toBeInTheDocument();
    }
  });
});

describe("JPG, la captura que se puede mandar por correo", () => {
  it("exporta el fotograma en el que se esta, no el trozo recortado", async () => {
    // Es la diferencia entera entre una foto y un video: `from` y `to` tienen que ser el
    // fotograma actual, no el recorte de A a B.
    pintar();
    fireEvent.click(screen.getByText("JPG"));
    fireEvent.click(screen.getByRole("button", { name: /Guardar/ }));
    await screen.findByText(/Guardar/);
    const req = loExportado();
    expect(req.format).toBe("jpg");
    expect(req.from).toBe(42);
    expect(req.to).toBe(42);
  });

  it("lleva barra de calidad, que es su razon de existir", () => {
    pintar();
    fireEvent.click(screen.getByText("JPG"));
    expect(screen.getByText("Calidad")).toBeInTheDocument();
  });

  it("y no lleva fotogramas por segundo, que una foto no tiene", () => {
    pintar();
    fireEvent.click(screen.getByText("JPG"));
    expect(screen.queryByText("Fotogramas por segundo")).toBeNull();
  });

  it("el PNG sigue sin barra de calidad, porque no pierde nada", () => {
    pintar();
    fireEvent.click(screen.getByText("PNG"));
    expect(screen.queryByText("Calidad")).toBeNull();
  });

  it("el video si lleva las dos barras", () => {
    pintar();
    expect(screen.getByText("Calidad")).toBeInTheDocument();
    expect(screen.getByText("Fotogramas por segundo")).toBeInTheDocument();
  });
});

describe("lo que no debe colarse en una foto", () => {
  it("el audio no viaja en un JPG aunque la grabacion lo tuviera", async () => {
    pintar();
    fireEvent.click(screen.getByText("JPG"));
    fireEvent.click(screen.getByRole("button", { name: /Guardar/ }));
    await screen.findByText(/Guardar/);
    expect(loExportado().audio).toBe(false);
  });

  it("y en un MP4 si", async () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: /Guardar/ }));
    await screen.findByText(/Guardar/);
    expect(loExportado().audio).toBe(true);
  });
});
