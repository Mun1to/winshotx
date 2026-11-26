/**
 * El panel de exportar.
 *
 * Es la ultima pantalla antes de que salga el archivo, y casi todo lo que decide vive en
 * lo que se le manda a Rust, no en lo que se ve. Una casilla que se queda pegada del
 * formato anterior no se nota mirando: se nota cuando alguien abre el archivo.
 */
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ExportPanel } from "./ExportPanel";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas, responde } from "../../test/preparar";
import type { SessionInfo } from "../../lib/types";
import { conProporcion, medida, type Recorte } from "../../lib/recorte";
import type { Edicion } from "../../lib/contrato";

const GRABACION: SessionInfo = {
  id: "s1",
  region: { x: 0, y: 0, width: 800, height: 600 },
  fps: 30,
  frameCount: 90,
  durationMs: 3000,
  hasAudio: true,
  hasClicks: true,
  cursorBaked: false,
  format: "video",
  mp4Path: null,
};

function pintar(
  session: Partial<SessionInfo> = {},
  recorte: Recorte | null = null,
  tramo: { desdeMs?: number; hastaMs?: number } = {},
  editorExterno?: string,
) {
  return render(
    <ExportPanel
      anotaciones={[]}
      recorte={recorte}
      session={{ ...GRABACION, ...session }}
      inIndex={10}
      outIndex={80}
      {...tramo}
      currentIndex={42}
      fpsMax={60}
      hasFfmpeg={false}
      saveDirectory="C:\\capturas"
      editorExterno={editorExterno}
      onRecorte={alRecorte}
    />,
  );
}

/** Lo que el panel le pide al editor sobre el marco de recorte. */
const alRecorte = vi.fn();

/** El panel de una grabación hecha donde no se pueden leer las teclas (Wayland) y sin atajos. */
function pintarSinTeclas(session: Partial<SessionInfo> = {}) {
  return render(
    <ExportPanel
      anotaciones={[]}
      recorte={null}
      session={{ ...GRABACION, ...session }}
      inIndex={10}
      outIndex={80}
      currentIndex={42}
      fpsMax={60}
      hasFfmpeg={false}
      saveDirectory="C:\capturas"
      onRecorte={alRecorte}
      sinTeclas
    />,
  );
}

/** Lo que se le mando a Rust en la ultima exportacion. */
function loExportado() {
  const llamada = [...llamadas].reverse().find((l) => l.comando === "export_media");
  return (llamada?.args as { request: Record<string, unknown> })?.request;
}

/** El botón grande de guardar, que dice el formato: «Guardar MP4». */
const guardar = () => screen.getByRole("button", { name: /^Guardar (MP4|GIF|PNG|JPG)$/ });

/** Guarda y espera a que la petición haya salido hacia Rust. */
async function guardarYEsperar() {
  fireEvent.click(guardar());
  await waitFor(() => expect(loExportado()).toBeTruthy());
  return loExportado();
}

/** El título de uno de los grupos plegables del panel («Fondo», «Calidad y tamaño»...). */
const grupo = (nombre: string) =>
  screen.getAllByRole("button", { name: nombre }).find((boton) => boton.hasAttribute("data-grupo"))!;

/**
 * Abre uno de los grupos plegables del panel, si estaba cerrado (S39): lo de un grupo cerrado
 * no se puede pulsar, así que antes de buscar un mando hay que abrir el suyo, como haría una
 * persona.
 */
const ir = (nombre: string) => {
  const boton = grupo(nombre);
  if (boton.getAttribute("aria-expanded") !== "true") fireEvent.click(boton);
};

beforeEach(() => {
  aplicarIdioma("es");
  alRecorte.mockClear();
  localStorage.removeItem("winshotx.editor.grupos");
});

describe("elegir el formato", () => {
  it("una grabacion empieza en MP4 y una captura suelta en PNG", () => {
    const { unmount } = pintar();
    expect(screen.getByRole("button", { name: "MP4" })).toHaveAttribute("aria-pressed", "true");
    unmount();
    pintar({ format: "still", frameCount: 1 });
    expect(screen.getByRole("button", { name: "PNG" })).toHaveAttribute("aria-pressed", "true");
  });

  it("estan los cuatro, GIF, MP4, PNG y JPG", () => {
    pintar();
    for (const f of ["GIF", "MP4", "PNG", "JPG"]) {
      expect(screen.getByRole("button", { name: f })).toBeInTheDocument();
    }
  });

  it("una captura suelta solo ofrece los dos de foto: un MP4 de un fotograma no es nada", () => {
    pintar({ format: "still", frameCount: 1 });
    expect(screen.getByRole("button", { name: "PNG" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "JPG" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "MP4" })).toBeNull();
    expect(screen.queryByRole("button", { name: "GIF" })).toBeNull();
  });

  it("cada formato dice lo que hace en palabras, no en siglas", () => {
    pintar();
    expect(screen.getByText("Vídeo con sonido. Se abre en cualquier sitio.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "GIF" }));
    expect(screen.getByText(/Se repite solo y sin sonido/)).toBeInTheDocument();
  });
});

describe("abrir en editor externo", () => {
  const nombre = "Abrir en editor externo";
  const editor = "C:\\Mis programas\\editor.exe";

  it("solo se ofrece con un editor configurado y el formato MP4", () => {
    const { unmount } = pintar();
    expect(screen.queryByRole("button", { name: nombre })).toBeNull();
    unmount();
    pintar({}, null, {}, editor);
    expect(screen.getByRole("button", { name: nombre })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "GIF" }));
    expect(screen.queryByRole("button", { name: nombre })).toBeNull();
  });

  it("pide exportar el tramo y conserva el acceso al MP4 si falla el editor", async () => {
    responde("export_media", {
      path: "C:\\capturas\\vídeo.mp4",
      bytes: 1024,
      copied: false,
      errorEditorExterno: "Acceso denegado",
    });
    pintar({}, null, { desdeMs: 300, hastaMs: 1600 }, editor);
    fireEvent.click(screen.getByRole("button", { name: nombre }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Acceso denegado");
    expect(screen.getByRole("button", { name: /abrir carpeta/ })).toBeInTheDocument();
    expect(llamadas.find((l) => l.comando === "export_media")?.args).toMatchObject({
      abrirEditorExterno: true,
      request: { format: "mp4", desdeMs: 300, hastaMs: 1600, copyToClipboard: false },
    });
  });
});

describe("una columna con sus grupos plegables, y guardar siempre a la vista", () => {
  it("el formato va arriba, y debajo cada grupo con su título a la vista", () => {
    pintar();
    const titulos = [...document.querySelectorAll("[data-grupo]")].map((boton) => boton.textContent);
    expect(titulos).toEqual([
      "Sonido",
      "Fondo",
      "Marco",
      "Puntero",
      "Clics y teclas",
      "Zoom",
      "Calidad y tamaño",
      "Estilos",
    ]);
    // El formato, antes que el primer grupo.
    const mp4 = screen.getByRole("button", { name: "MP4" });
    expect(mp4.compareDocumentPosition(grupo("Sonido")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("abre con el fondo, y la calidad y los fotogramas plegados en «Calidad y tamaño»", () => {
    pintar();
    expect(grupo("Fondo")).toHaveAttribute("aria-expanded", "true");
    expect(grupo("Sonido")).toHaveAttribute("aria-expanded", "false");
    expect(grupo("Calidad y tamaño")).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("slider", { name: "Calidad" })).toBeNull();
    ir("Calidad y tamaño");
    expect(screen.getByRole("slider", { name: "Calidad" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Fotogramas por segundo" })).toBeInTheDocument();
  });

  it("el zoom, el puntero y el sonido, cada uno en su grupo", () => {
    pintar();
    ir("Sonido");
    expect(screen.getByRole("switch", { name: "Con sonido" })).toBeInTheDocument();
    ir("Zoom");
    expect(screen.getByRole("slider", { name: "Acercarse a los clics" })).toBeInTheDocument();
    ir("Puntero");
    expect(screen.getByRole("slider", { name: "Tamaño" })).toBeInTheDocument();
  });

  it("una foto no lleva sonido, puntero, clics ni zoom", () => {
    pintar({ format: "still", frameCount: 1 });
    const titulos = [...document.querySelectorAll("[data-grupo]")].map((boton) => boton.textContent);
    expect(titulos).toEqual(["Fondo", "Marco", "Calidad y tamaño", "Estilos"]);
  });

  it("cada grupo se queda abierto o cerrado como lo dejaste la próxima vez", () => {
    const { unmount } = pintar();
    ir("Calidad y tamaño");
    fireEvent.click(grupo("Fondo"));
    unmount();
    pintar();
    expect(grupo("Calidad y tamaño")).toHaveAttribute("aria-expanded", "true");
    expect(grupo("Fondo")).toHaveAttribute("aria-expanded", "false");
  });

  it("ya no hay riel ni resumen de «Lo que va a salir»", () => {
    pintar();
    expect(screen.queryByRole("tab")).toBeNull();
    expect(document.querySelector("[data-lo-que-sale]")).toBeNull();
  });

  it("los botones de guardar están siempre, con su nombre escrito", () => {
    pintar();
    expect(guardar()).toHaveTextContent("Guardar MP4");
    expect(screen.getByRole("button", { name: "Copiar" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Guardar en…" })).toBeInTheDocument();
  });
});

describe("el tramo de la línea de tiempo", () => {
  it("viaja a Rust en milisegundos, que es lo que permite cortar dentro de una quietud", async () => {
    pintar({}, null, { desdeMs: 2_500, hastaMs: 7_000 });
    const req = await guardarYEsperar();
    expect(req).toMatchObject({ desdeMs: 2_500, hastaMs: 7_000, from: 10, to: 80 });
  });

  it("en una foto no viaja: una foto es un fotograma, no un tramo", async () => {
    pintar({}, null, { desdeMs: 2_500, hastaMs: 7_000 });
    fireEvent.click(screen.getByRole("button", { name: "PNG" }));
    const req = await guardarYEsperar();
    expect(req).toMatchObject({ desdeMs: null, hastaMs: null, from: 42, to: 42 });
  });

  it("lo que va a pesar sale de lo que dura el tramo, en milisegundos", () => {
    // A la calidad de fábrica un MP4 son unos 1,2 MB por segundo. Los fotogramas 10 a 80
    // son 2,4 segundos a 30 fps; el tramo de 1 a 3 segundos son 2, y es lo que se guarda.
    // En español, con coma y un espacio que no se parte entre la cifra y la unidad.
    const { unmount } = pintar();
    expect(screen.getByText("≈ 2,8\u00a0MB", { normalizer: (x) => x })).toBeInTheDocument();
    unmount();
    pintar({}, null, { desdeMs: 1_000, hastaMs: 3_000 });
    expect(screen.getByText("≈ 2,3\u00a0MB", { normalizer: (x) => x })).toBeInTheDocument();
  });
});

describe("lo nuevo del plan (zooms a mano, tramos, fondos propios)", () => {
  const EDICION: Edicion = {
    zooms: [{ desdeMs: 100, hastaMs: 900, nivel: 2, modo: "fijo", x: 0.5, y: 0.5, instantaneo: false }],
    tramos: [{ desdeMs: 0, hastaMs: 1_000, velocidad: 2 }],
    fondo: { tipo: "color", rgb: [10, 20, 30] },
  };

  function pintarCon(edicion: Edicion) {
    render(
      <ExportPanel
        anotaciones={[]}
        recorte={null}
        session={GRABACION}
        inIndex={10}
        outIndex={80}
        currentIndex={42}
        fpsMax={60}
        hasFfmpeg={false}
        saveDirectory="C:\\capturas"
        edicion={edicion}
      />,
    );
  }

  it("viaja a Rust tal cual, como campos sueltos de la petición", async () => {
    pintarCon(EDICION);
    const req = await guardarYEsperar();
    expect(req).toMatchObject(EDICION);
  });

  it("en una foto se queda el fondo, pero no los zooms ni los tramos, que son del tiempo", async () => {
    pintarCon(EDICION);
    fireEvent.click(screen.getByRole("button", { name: "PNG" }));
    const req = await guardarYEsperar();
    expect(req).toMatchObject({ zooms: null, tramos: null, fondo: EDICION.fondo });
  });

  /** El peso estimado, en bytes, tal como lo enseña el panel («≈ 2,4 MB» en español, «≈ 2.4 MB» en inglés). */
  function pesoEstimado() {
    const [, numero, unidad] = /≈ ([\d.,]+)\s(B|KB|MB|GB)/.exec(screen.getByText(/^≈ /).textContent ?? "") ?? [];
    return Number(numero.replace(",", ".")) * 1024 ** ["B", "KB", "MB", "GB"].indexOf(unidad);
  }

  it("el peso cuenta solo lo que sale: lo quitado no pesa y un trozo al doble, la mitad", () => {
    pintarCon({});
    const entero = pesoEstimado();
    cleanup();
    // Del tramo de 71 fotogramas a 30 fps (2,37 s) sale un segundo al doble: medio segundo.
    pintarCon({ tramos: [{ desdeMs: 0, hastaMs: 1_000, velocidad: 2 }] });
    const recortado = pesoEstimado();
    expect(recortado / entero).toBeGreaterThan(0.19);
    expect(recortado / entero).toBeLessThan(0.23);
  });

  it("con un trozo acelerado avisa de que ese trozo sale sin sonido, no el vídeo entero", () => {
    pintarCon({
      tramos: [
        { desdeMs: 0, hastaMs: 1_000, velocidad: 1 },
        { desdeMs: 1_000, hastaMs: 2_000, velocidad: 2 },
      ],
    });
    ir("Sonido");
    expect(screen.getByText("los trozos acelerados salen sin sonido")).toBeInTheDocument();
    expect(screen.queryByText("a otra velocidad el vídeo sale mudo")).toBeNull();
  });
});

describe("guardar donde uno quiera", () => {
  it("«Guardar en…» pregunta con el diálogo y exporta al archivo elegido", async () => {
    // Munir, el 19 de septiembre de 2026: «que puedas guardar las capturas donde quieras».
    // La carpeta de ajustes sigue siendo la de siempre; esto es para la captura que va a
    // un proyecto concreto, y tiene que llegar a Rust con el archivo exacto.
    responde("pick_save_file", "C:\\proyectos\\demo\\portada.mp4");
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "Guardar en…" }));
    await waitFor(() => expect(loExportado()).toBeTruthy());
    const pregunta = llamadas.find((l) => l.comando === "pick_save_file")?.args as {
      extension: string;
      directory: string;
    };
    expect(pregunta.extension).toBe("mp4");
    expect(pregunta.directory).toContain("capturas");
    expect(loExportado().file).toBe("C:\\proyectos\\demo\\portada.mp4");
  });

  it("si se cancela el diálogo no se exporta nada", async () => {
    responde("pick_save_file", null);
    pintar();
    fireEvent.keyDown(window, { key: "s", ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(llamadas.some((l) => l.comando === "pick_save_file")).toBe(true));
    expect(llamadas.some((l) => l.comando === "export_media")).toBe(false);
  });

  it("Ctrl+S a secas sigue yendo a la carpeta, sin preguntar", async () => {
    pintar();
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await waitFor(() => expect(loExportado()).toBeTruthy());
    expect(llamadas.some((l) => l.comando === "pick_save_file")).toBe(false);
    expect(loExportado().file).toBeNull();
  });

  it("«Copiar» guarda y además lo deja en el portapapeles", async () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "Copiar" }));
    await waitFor(() => expect(loExportado()).toBeTruthy());
    expect(loExportado().copyToClipboard).toBe(true);
  });
});

describe("JPG, la captura que se puede mandar por correo", () => {
  it("exporta el fotograma en el que se esta, no el trozo recortado", async () => {
    // Es la diferencia entera entre una foto y un video: `from` y `to` tienen que ser el
    // fotograma actual, no el recorte de A a B.
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "JPG" }));
    const req = await guardarYEsperar();
    expect(req.format).toBe("jpg");
    expect(req.from).toBe(42);
    expect(req.to).toBe(42);
  });

  it("lleva barra de calidad, que es su razon de existir", () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "JPG" }));
    ir("Calidad y tamaño");
    expect(screen.getByText("Calidad")).toBeInTheDocument();
  });

  it("y no lleva fotogramas por segundo, que una foto no tiene", () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "JPG" }));
    ir("Calidad y tamaño");
    expect(screen.queryByText("Fotogramas por segundo")).toBeNull();
  });

  it("el PNG sigue sin barra de calidad, porque no pierde nada", () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "PNG" }));
    ir("Calidad y tamaño");
    expect(screen.queryByText("Calidad")).toBeNull();
  });

  it("el video si lleva las dos barras", () => {
    pintar();
    ir("Calidad y tamaño");
    expect(screen.getByText("Calidad")).toBeInTheDocument();
    expect(screen.getByText("Fotogramas por segundo")).toBeInTheDocument();
  });
});

describe("lo que no debe colarse en una foto", () => {
  it("el audio no viaja en un JPG aunque la grabacion lo tuviera", async () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "JPG" }));
    expect((await guardarYEsperar()).audio).toBe(false);
  });

  it("y en un MP4 si", async () => {
    pintar();
    expect((await guardarYEsperar()).audio).toBe(true);
  });

  it("y si se apaga el sonido, el MP4 sale mudo", async () => {
    pintar();
    ir("Sonido");
    fireEvent.click(screen.getByRole("switch", { name: "Con sonido" }));
    expect((await guardarYEsperar()).audio).toBe(false);
  });
});

describe("recortar la imagen", () => {
  const MITAD_DERECHA: Recorte = { x1: 0.5, y1: 0, x2: 1, y2: 1 };

  it("las dimensiones salen del trozo, no de la captura entera", () => {
    // Ensennar 800 x 600 despues de recortar la mitad seria ensennar el tamanno de algo
    // que ya no se va a exportar.
    pintar({}, MITAD_DERECHA);
    ir("Calidad y tamaño");
    expect(screen.getByLabelText("Ancho")).toHaveValue(400);
    expect(screen.getByLabelText("Alto")).toHaveValue(600);
  });

  it("y el recorte viaja a Rust tal cual, en tanto por uno", async () => {
    pintar({}, MITAD_DERECHA);
    expect((await guardarYEsperar()).crop).toEqual(MITAD_DERECHA);
  });

  it("el 50 % es la mitad del trozo, no la mitad de la captura entera", async () => {
    // Con el recorte puesto, escalar sobre la captura entera estiraria el trozo al doble.
    pintar({}, MITAD_DERECHA);
    ir("Calidad y tamaño");
    fireEvent.click(screen.getByRole("button", { name: "50%" }));
    expect(screen.getByLabelText("Ancho")).toHaveValue(200);
    expect(screen.getByLabelText("Alto")).toHaveValue(300);
  });

  it("sin recorte se manda null y salen las medidas de la region grabada", async () => {
    pintar();
    ir("Calidad y tamaño");
    expect(screen.getByLabelText("Ancho")).toHaveValue(800);
    expect((await guardarYEsperar()).crop).toBeNull();
  });
});

describe("el zoom que se acerca a los clics", () => {
  it("sale con vídeo, apagado de fábrica", () => {
    // Encendido de fábrica sorprende: el vídeo se movería solo sin que nadie lo pida.
    pintar();
    ir("Zoom");
    expect(screen.getByText("Acercarse a los clics")).toBeInTheDocument();
    expect(screen.getByText("sin zoom")).toBeInTheDocument();
  });

  it("no sale en una foto, que no tiene tiempo dentro", () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "JPG" }));
    expect(screen.queryByText("Acercarse a los clics")).toBeNull();
  });

  it("ni cuando la grabación no tuvo un solo clic", () => {
    // Un interruptor que no puede hacer nada es peor que no tenerlo.
    pintar({ hasClicks: false });
    expect(screen.queryByText("Acercarse a los clics")).toBeNull();
  });

  it("apagado se manda 1, que en Rust significa no acercarse", async () => {
    pintar();
    expect((await guardarYEsperar()).zoom).toBe(1);
  });

  it("y en una foto se manda 1 aunque estuviera puesto", async () => {
    pintar();
    ir("Zoom");
    fireEvent.change(screen.getByLabelText("Acercarse a los clics"), { target: { value: "2" } });
    fireEvent.click(screen.getByRole("button", { name: "PNG" }));
    expect((await guardarYEsperar()).zoom).toBe(1);
  });

  it("puesto a 2 se manda 2", async () => {
    pintar();
    ir("Zoom");
    fireEvent.change(screen.getByLabelText("Acercarse a los clics"), { target: { value: "2" } });
    expect((await guardarYEsperar()).zoom).toBe(2);
  });
});

describe("el puntero dibujado", () => {
  it("viene encendido al tamanno normal, porque la grabacion ya no lo lleva cocido", () => {
    pintar({ cursorBaked: false, region: { x: 0, y: 0, width: 1920, height: 1080 } });
    ir("Puntero");
    const barra = screen.getByRole("slider", { name: "Tamaño" }) as HTMLInputElement;
    expect(Number(barra.value)).toBe(43);
  });

  it("y en una grabacion vieja con el cursor cocido empieza apagado, para no ensennar dos", () => {
    pintar({ cursorBaked: true });
    ir("Puntero");
    const barra = screen.getByRole("slider", { name: "Tamaño" }) as HTMLInputElement;
    expect(Number(barra.value)).toBe(0);
    expect(screen.getByText("el de Windows")).toBeInTheDocument();
  });

  it("sale aunque la grabacion no tenga un solo clic: un video sin clics tambien tiene raton", () => {
    pintar({ hasClicks: false });
    ir("Puntero");
    expect(screen.getByRole("slider", { name: "Tamaño" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Acercarse a los clics")).toBeNull();
  });

  it("lo que se decide se avisa hacia arriba, para la vista previa", () => {
    const avisos: unknown[] = [];
    render(
      <ExportPanel
        anotaciones={[]}
        recorte={null}
        session={GRABACION}
        inIndex={10}
        outIndex={80}
        currentIndex={42}
        fpsMax={60}
        hasFfmpeg={false}
        saveDirectory="C:\\capturas"
        onEstudio={(e) => avisos.push(e)}
      />,
    );
    ir("Zoom");
    fireEvent.change(screen.getByLabelText("Acercarse a los clics"), { target: { value: "2" } });
    const ultimo = avisos.at(-1) as { zoom: number; cursor: number };
    expect(ultimo.zoom).toBe(2);
    expect(ultimo.cursor).toBe(24);
  });
});

describe("la velocidad", () => {
  /** El botón de cada velocidad, que se lee «2×». */
  const boton = (v: string) => screen.getByRole("button", { name: `${v}×` });

  it("empieza en 1x, que es como se grabo", () => {
    pintar();
    ir("Calidad y tamaño");
    expect(boton("1")).toHaveAttribute("aria-pressed", "true");
  });

  it("no sale en una foto, que no tiene tiempo dentro", () => {
    pintar({ format: "still", frameCount: 1 });
    ir("Calidad y tamaño");
    expect(screen.queryByText("Velocidad")).not.toBeInTheDocument();
  });

  it("a 2x viaja a Rust, y el video se queda sin sonido", async () => {
    pintar();
    ir("Calidad y tamaño");
    fireEvent.click(boton("2"));
    // Acelerar la pista sin cambiarle el tono es otro problema entero, y dejarla tal cual
    // la despegaría de la imagen desde el primer segundo. Se dice y se quita.
    expect(await guardarYEsperar()).toMatchObject({ speed: 2, audio: false });
  });

  it("y se lo dice a la vista previa, para que no suene clics que no se guardan", () => {
    const avisos: { velocidad?: number }[] = [];
    render(
      <ExportPanel
        anotaciones={[]}
        recorte={null}
        session={GRABACION}
        inIndex={10}
        outIndex={80}
        currentIndex={42}
        fpsMax={60}
        hasFfmpeg={false}
        saveDirectory="C:\capturas"
        onEstudio={(e) => avisos.push(e)}
      />,
    );
    ir("Calidad y tamaño");
    expect(avisos.at(-1)?.velocidad).toBe(1);
    fireEvent.click(boton("2"));
    expect(avisos.at(-1)?.velocidad).toBe(2);
  });

  it("y lo avisa antes de exportar, no despues", () => {
    pintar();
    ir("Calidad y tamaño");
    fireEvent.click(boton("2"));
    expect(screen.getAllByText(/sale sin sonido|sale mudo/).length).toBeGreaterThan(0);
  });
});

describe("la proporcion de un clic", () => {
  it("9:16 pide el trozo vertical mas grande que cabe, centrado", () => {
    // Lo que se graba de un monitor sale apaisado SIEMPRE: para subir algo vertical hay
    // que recortar, y a ojo no se acierta un 9:16 exacto.
    pintar();
    ir("Calidad y tamaño");
    fireEvent.click(screen.getByRole("button", { name: "9:16" }));
    const puesto = alRecorte.mock.calls.at(-1)?.[0] as Recorte;
    // La sesion de prueba es de 800x600.
    expect(medida(puesto, 800, 600)).toEqual({ width: 338, height: 600 });
    expect(puesto.x1).toBeCloseTo(1 - puesto.x2, 6);
  });

  it("y el boton se queda marcado mientras ese recorte siga puesto", () => {
    const vertical = conProporcion(9 / 16, 800, 600);
    pintar({}, vertical);
    ir("Calidad y tamaño");
    expect(screen.getByRole("button", { name: "9:16" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "1:1" })).toHaveAttribute("aria-pressed", "false");
  });

  it("las dimensiones salen del trozo, no de la captura entera", () => {
    // El fallo que esto evita: exportar un 9:16 con el ancho de la pantalla, que estira
    // la imagen hasta dejarla irreconocible.
    pintar({}, conProporcion(9 / 16, 800, 600));
    ir("Calidad y tamaño");
    expect(screen.getByLabelText("Ancho")).toHaveValue(338);
    expect(screen.getByLabelText("Alto")).toHaveValue(600);
  });

  it("y la × lo quita entero", () => {
    pintar({}, conProporcion(1, 800, 600));
    ir("Calidad y tamaño");
    fireEvent.click(screen.getByRole("button", { name: "Quitar el recorte" }));
    expect(alRecorte).toHaveBeenCalledWith(null);
  });
});

describe("en inglés", () => {
  it("el panel no se deja ni una palabra en español", () => {
    aplicarIdioma("en");
    pintar();
    expect(grupo("Quality and size")).toBeInTheDocument();
    ir("Sound");
    expect(screen.getByRole("switch", { name: "With sound" })).toBeInTheDocument();
    expect(screen.getByText("Video with sound. Opens anywhere.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save to…" })).toBeInTheDocument();
  });
});

describe("el estudio", () => {
  it("abre por el fondo, apagado, y el movimiento suave viene en «Poco»", () => {
    pintar();
    expect(grupo("Fondo")).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: "Sin fondo" })).toHaveAttribute("aria-pressed", "true");
    ir("Puntero");
    // El suavizado de tres opciones sustituye al interruptor de antes: «Poco» es lo de siempre.
    expect(screen.queryByRole("switch", { name: /Movimiento suave/ })).toBeNull();
    expect(screen.getByRole("button", { name: "Poco" })).toHaveAttribute("aria-pressed", "true");
  });

  it("los estilos y los mandos del cursor están en el Estudio, y lo que piden viaja a Rust", async () => {
    // Con el editor de verdad: `edicion` vive arriba y el panel la cambia con `onEdicion`.
    function ConEdicion() {
      const [edicion, setEdicion] = useState<Edicion>({});
      return (
        <ExportPanel
          anotaciones={[]}
          recorte={null}
          session={GRABACION}
          inIndex={10}
          outIndex={80}
          currentIndex={42}
          fpsMax={60}
          hasFfmpeg={false}
          saveDirectory="C:\\capturas"
          edicion={edicion}
          onEdicion={(cambio) => setEdicion((antes) => ({ ...antes, ...cambio }))}
        />
      );
    }
    render(<ConEdicion />);
    ir("Estilos");
    expect(screen.getByRole("button", { name: "Guardar estilo como…" })).toBeInTheDocument();
    ir("Puntero");
    fireEvent.click(screen.getByRole("button", { name: "Mucho" }));
    ir("Clics y teclas");
    fireEvent.click(screen.getByRole("switch", { name: /Una onda en cada clic/ }));
    expect(await guardarYEsperar()).toMatchObject({ suavizado: "mucho", ondaClic: true, smoothCursor: true });
  });

  // En Wayland no se pueden leer las teclas al grabar: un interruptor de atajos que no puede
  // enseñar ninguno no se ofrece, y sin clics el grupo entero se quedaría vacío.
  it("sin teclas que enseñar no ofrece «Enseñar los atajos», y sin clics tampoco el grupo", () => {
    const { unmount } = pintarSinTeclas({ hasClicks: false });
    expect(screen.queryByRole("button", { name: "Clics y teclas" })).not.toBeInTheDocument();
    unmount();
    // Con clics el grupo sigue, con lo de los clics y sin el interruptor de los atajos.
    pintarSinTeclas({ hasClicks: true });
    ir("Clics y teclas");
    expect(screen.getByRole("switch", { name: "Marcar los clics" })).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: /Enseñar los atajos/ })).not.toBeInTheDocument();
  });

  it("y donde sí se leen, «Enseñar los atajos» sale aunque la grabación no traiga clics", () => {
    pintar({ hasClicks: false });
    ir("Clics y teclas");
    expect(screen.getByRole("switch", { name: /Enseñar los atajos/ })).toBeInTheDocument();
  });

  it("elegir un fondo pone aire, esquinas y sombra que se vean, y todo viaja a Rust", async () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "Noche" }));
    // Un 5 % del ancho de aire y algo más del 1 % de radio, sobre los 800 de la grabación.
    expect(await guardarYEsperar()).toMatchObject({ background: "noche", margin: 40, corners: 10, shadow: true });
  });

  it("y «Sin fondo» lo quita entero", async () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "Coral" }));
    fireEvent.click(screen.getByRole("button", { name: "Sin fondo" }));
    expect(await guardarYEsperar()).toMatchObject({ margin: 0, corners: 0 });
  });

  it("las esquinas solo salen con fondo: sin fondo no hay nada que asome por ellas", () => {
    pintar();
    expect(screen.queryByRole("slider", { name: "Esquinas redondeadas" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Océano" }));
    expect(screen.getByRole("slider", { name: "Esquinas redondeadas" })).toBeInTheDocument();
  });

  it("el puntero suave y el que se esconde viajan a Rust, y en una foto no", async () => {
    const { unmount } = pintar();
    ir("Puntero");
    fireEvent.click(screen.getByRole("switch", { name: /Esconderlo cuando no se mueve/ }));
    expect(await guardarYEsperar()).toMatchObject({ smoothCursor: true, hideIdleCursor: true });
    unmount();
    llamadas.length = 0;
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "PNG" }));
    expect(await guardarYEsperar()).toMatchObject({ smoothCursor: false, hideIdleCursor: false });
  });

  it("la velocidad del zoom solo sale con zoom, y viaja a Rust", async () => {
    pintar();
    ir("Zoom");
    expect(screen.queryByText("Velocidad del zoom")).toBeNull();
    fireEvent.change(screen.getByLabelText("Acercarse a los clics"), { target: { value: "2" } });
    fireEvent.click(screen.getByRole("button", { name: "Rápida" }));
    expect(await guardarYEsperar()).toMatchObject({ zoom: 2, zoomSpeed: 1.6 });
  });

  it("el volumen viaja a Rust en tanto por uno", async () => {
    pintar();
    ir("Sonido");
    fireEvent.change(screen.getByRole("slider", { name: "Volumen" }), { target: { value: "50" } });
    expect(await guardarYEsperar()).toMatchObject({ volume: 0.5 });
  });

  it("y sin sonido grabado no hay volumen que tocar", () => {
    pintar({ hasAudio: false });
    expect(screen.queryByRole("slider", { name: "Volumen" })).toBeNull();
  });

  it("lo que se decide se avisa hacia arriba con el marco en tanto por uno, para la vista previa", () => {
    const avisos: unknown[] = [];
    render(
      <ExportPanel
        anotaciones={[]}
        recorte={null}
        session={GRABACION}
        inIndex={10}
        outIndex={80}
        currentIndex={42}
        fpsMax={60}
        hasFfmpeg={false}
        saveDirectory="C:\capturas"
        onEstudio={(e) => avisos.push(e)}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Atardecer" }));
    const ultimo = avisos.at(-1) as { marco: { aire: number; esquinas: number; fondo: string } };
    expect(ultimo.marco.fondo).toBe("atardecer");
    expect(ultimo.marco.aire).toBeCloseTo(40 / 800, 6);
    expect(ultimo.marco.esquinas).toBeCloseTo(10 / 800, 6);
  });

  it("en inglés tampoco queda nada en español", () => {
    aplicarIdioma("en");
    pintar();
    expect(grupo("Background")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "No background" })).toBeInTheDocument();
    ir("Pointer");
    expect(screen.getByText("Smooth movement")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Little" })).toBeInTheDocument();
  });
});

// S11 (pistas)
describe("el sonido del PC y tu micrófono, por separado", () => {
  const CON_PISTAS: Partial<SessionInfo> = {
    sonidoPc: "C:\\sesion\\sonido-pc.wav",
    microfono: "C:\\sesion\\microfono.wav",
  };

  /** El panel con su `edicion` de verdad, como en el editor: lo que cambia se queda. */
  function pintarConPistas(session: Partial<SessionInfo> = CON_PISTAS) {
    function ConEdicion() {
      const [edicion, setEdicion] = useState<Edicion>({});
      return (
        <ExportPanel
          anotaciones={[]}
          recorte={null}
          session={{ ...GRABACION, ...session }}
          inIndex={10}
          outIndex={80}
          currentIndex={42}
          fpsMax={60}
          hasFfmpeg={false}
          saveDirectory="C:\capturas"
          edicion={edicion}
          onEdicion={(cambio) => setEdicion((antes) => ({ ...antes, ...cambio }))}
        />
      );
    }
    const pintado = render(<ConEdicion />);
    // Los mandos de las pistas viven en el grupo «Sonido», plegado de fábrica (y abierto si se
    // dejó abierto en otra prueba de este mismo bloque).
    const sonido = document.querySelector('[data-grupo="sonido"]')!;
    if (sonido.getAttribute("aria-expanded") !== "true") fireEvent.click(sonido);
    return pintado;
  }

  it("donde iba «Sonido» hay dos mandos, encendidos y a su volumen de grabación", () => {
    pintarConPistas();
    expect(screen.queryByRole("switch", { name: "Con sonido" })).not.toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Sonido del PC" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("switch", { name: "Tu micrófono" })).toHaveAttribute("aria-checked", "true");
    const volumenes = screen.getAllByRole("slider", { name: "Volumen" });
    expect(volumenes).toHaveLength(2);
    expect(volumenes.map((v) => (v as HTMLInputElement).value)).toEqual(["100", "100"]);
  });

  it("sin tocar nada, se guardan las dos como se grabaron", async () => {
    pintarConPistas();
    const req = await guardarYEsperar();
    expect(req.audio).toBe(true);
    expect(req.sonidoPc).toEqual({ encendida: true, volumen: 1 });
    expect(req.microfono).toEqual({ encendida: true, volumen: 1 });
  });

  it("apagar la voz y bajar el PC viaja a Rust, y la voz apagada no enseña su volumen", async () => {
    pintarConPistas();
    fireEvent.click(screen.getByRole("switch", { name: "Tu micrófono" }));
    expect(screen.getAllByRole("slider", { name: "Volumen" })).toHaveLength(1);
    fireEvent.change(screen.getByRole("slider", { name: "Volumen" }), { target: { value: "40" } });
    expect(screen.getByText("40 %")).toBeInTheDocument();
    const req = await guardarYEsperar();
    expect(req.audio).toBe(true);
    expect(req.sonidoPc).toEqual({ encendida: true, volumen: 0.4 });
    expect(req.microfono).toEqual({ encendida: false, volumen: 1 });
  });

  it("con las dos apagadas el MP4 sale mudo", async () => {
    pintarConPistas();
    fireEvent.click(screen.getByRole("switch", { name: "Sonido del PC" }));
    fireEvent.click(screen.getByRole("switch", { name: "Tu micrófono" }));
    expect((await guardarYEsperar()).audio).toBe(false);
  });

  // Lo del 14-nov: el PC a -56 dB (Brave al 10 % en el mezclador) y el micro en ceros. Sin
  // aviso, parecía que la vista previa no sonaba.
  it("una pista grabada muda o casi muda lo dice debajo de su mando, y por qué", () => {
    pintarConPistas({ ...CON_PISTAS, picoPc: 0.0016, picoMicrofono: 0 });
    expect(
      screen.getByText("Se grabó casi sin sonido: la app que sonaba estaba muy baja en el mezclador de Windows."),
    ).toBeInTheDocument();
    expect(
      screen.getByText("El micrófono no mandó nada: mira que no esté silenciado ni sin permiso en Windows."),
    ).toBeInTheDocument();
    // Apagada ya no va a sonar: el aviso sobra.
    fireEvent.click(screen.getByRole("switch", { name: "Tu micrófono" }));
    expect(screen.queryByText(/El micrófono no mandó nada/)).not.toBeInTheDocument();
  });

  it("una pista que se oye, o una sesión de antes sin el dato, no avisa de nada", () => {
    pintarConPistas({ ...CON_PISTAS, picoPc: 0.4, picoMicrofono: null });
    expect(screen.queryByText(/sin sonido|no mandó nada|No sonó nada/)).not.toBeInTheDocument();
  });

  // Revisión del 16-nov: al subirlo, el pico ya se oye y el aviso de casi mudo desaparecía, así
  // que nadie sabía que el sonido se había tocado.
  it("el sonido del PC subido al grabar lo dice debajo de su mando, con cuánto", () => {
    pintarConPistas({ ...CON_PISTAS, picoPc: 0.49, subidaPc: 36.5 });
    expect(
      screen.getByText(
        "Llegó casi mudo (la app que sonaba estaba muy baja en el mezclador de Windows) y se subió 37 dB al grabar.",
      ),
    ).toBeInTheDocument();
  });

  it("el PC en silencio exacto dice que no sonó nada", () => {
    pintarConPistas({ ...CON_PISTAS, picoPc: 0 });
    expect(screen.getByText("No sonó nada en el PC mientras grababas.")).toBeInTheDocument();
  });

  it("solo salen los mandos de lo que se grabó: sin el PC, solo «Tu micrófono»", async () => {
    pintarConPistas({ sonidoPc: null, microfono: "C:\\sesion\\microfono.wav" });
    expect(screen.queryByRole("switch", { name: "Sonido del PC" })).not.toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Tu micrófono" })).toBeInTheDocument();
    const req = await guardarYEsperar();
    expect(req.sonidoPc).toBeNull();
    expect(req.microfono).toEqual({ encendida: true, volumen: 1 });
  });

  it("una grabación de antes, con todo junto, sigue con «Sonido» y su volumen de siempre", async () => {
    pintarConPistas({ sonidoPc: undefined, microfono: undefined });
    expect(screen.getByRole("switch", { name: "Con sonido" })).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Tu micrófono" })).not.toBeInTheDocument();
    const req = await guardarYEsperar();
    expect(req.audio).toBe(true);
    expect(req.sonidoPc).toBeNull();
    expect(req.microfono).toBeNull();
  });

  it("a otra velocidad no hay volumen que tocar, y lo dice una vez", () => {
    pintarConPistas();
    ir("Calidad y tamaño");
    fireEvent.click(screen.getByRole("button", { name: "2×" }));
    ir("Sonido");
    expect(screen.queryAllByRole("slider", { name: "Volumen" })).toHaveLength(0);
    expect(screen.getAllByText("a otra velocidad el vídeo sale mudo")).toHaveLength(1);
  });

  it("en inglés y en chino, con sus nombres", () => {
    aplicarIdioma("en");
    const { unmount } = pintarConPistas();
    expect(screen.getByRole("switch", { name: "PC sound" })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Your microphone" })).toBeInTheDocument();
    unmount();
    aplicarIdioma("zh");
    pintarConPistas();
    expect(screen.getByRole("switch", { name: "电脑声音" })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "你的麦克风" })).toBeInTheDocument();
  });
});

// S25 (marco): la captura dentro de una ventana de Windows 11 o de un portátil.
describe("el marco de ventana o de portátil", () => {
  beforeEach(() => {
    localStorage.setItem("winshotx.tema", "claro");
    // El marco va en su propio grupo plegable (S39): aquí se mira abierto.
    localStorage.setItem("winshotx.editor.grupos", JSON.stringify({ marco: true }));
  });

  it("sale con sus tres opciones, en «Ninguno», y sin marco no viaja nada", async () => {
    pintar();
    for (const nombre of ["Ninguno", "Ventana", "Portátil"]) {
      expect(screen.getByRole("button", { name: nombre })).toBeInTheDocument();
    }
    expect(screen.getByRole("button", { name: "Ninguno" })).toHaveAttribute("aria-pressed", "true");
    expect((await guardarYEsperar()).carcasa).toBeNull();
  });

  it("elegir la ventana sin fondo pone el fondo, y viaja con el tema de winshotx", async () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "Ventana" }));
    expect(screen.getByRole("button", { name: "Ventana" })).toHaveAttribute("aria-pressed", "true");
    expect(await guardarYEsperar()).toMatchObject({ margin: 40, carcasa: { tipo: "ventana", oscura: false } });
  });

  it("con el tema oscuro, la ventana oscura", async () => {
    localStorage.setItem("winshotx.tema", "oscuro");
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "Ventana" }));
    expect(await guardarYEsperar()).toMatchObject({ carcasa: { tipo: "ventana", oscura: true } });
  });

  it("el portátil viaja como portátil, también en una foto", async () => {
    pintar({ format: "still", frameCount: 1 });
    fireEvent.click(screen.getByRole("button", { name: "Portátil" }));
    expect(await guardarYEsperar()).toMatchObject({ format: "png", carcasa: { tipo: "portatil" } });
  });

  it("con marco, las esquinas son las suyas: el deslizador se va y vuelve sin él", () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "Océano" }));
    expect(screen.getByRole("slider", { name: "Esquinas redondeadas" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Portátil" }));
    expect(screen.queryByRole("slider", { name: "Esquinas redondeadas" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Ninguno" }));
    expect(screen.getByRole("slider", { name: "Esquinas redondeadas" })).toBeInTheDocument();
  });

  it("«Sin fondo» quita también el marco, y volver a poner un fondo no lo trae", async () => {
    pintar();
    fireEvent.click(screen.getByRole("button", { name: "Ventana" }));
    fireEvent.click(screen.getByRole("button", { name: "Sin fondo" }));
    expect(screen.getByRole("button", { name: "Ninguno" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "Coral" }));
    expect(await guardarYEsperar()).toMatchObject({ background: "coral", carcasa: null });
  });

  it("el peso que se estima cuenta con la barra de la ventana", () => {
    pintar({ format: "still", frameCount: 1 });
    const sin = screen.getByText(/≈/).textContent;
    fireEvent.click(screen.getByRole("button", { name: "Blanco" }));
    const conFondo = screen.getByText(/≈/).textContent;
    fireEvent.click(screen.getByRole("button", { name: "Ventana" }));
    const conVentana = screen.getByText(/≈/).textContent;
    expect(conFondo).not.toBe(sin);
    expect(conVentana).not.toBe(conFondo);
  });

  it("en inglés y en chino", () => {
    aplicarIdioma("en");
    const { unmount } = pintar();
    expect(screen.getByRole("button", { name: "Window" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Laptop" })).toBeInTheDocument();
    unmount();
    aplicarIdioma("zh");
    pintar();
    expect(screen.getByRole("button", { name: "笔记本电脑" })).toBeInTheDocument();
  });
});

describe("en Linux, si falta el codificador del sistema se dice antes de guardar", () => {
  const guardar = (formato: string) => screen.getByRole("button", { name: new RegExp(`Guardar ${formato}`) });
  /** La orden entera, tal como se copia: va partida en palabras para que no se corte por un guion. */
  const orden = (texto: string) =>
    screen.getByText((_, el) => el?.tagName === "CODE" && el.textContent === texto);

  it("sin codificador de vídeo: lo dice con su orden, no deja guardar el MP4 y el GIF sigue saliendo", async () => {
    responde("falta_para_el_mp4", { que: "video", orden: "sudo apt install gstreamer1.0-plugins-bad" });
    pintar();
    expect(
      await screen.findByText(/Para guardar en MP4 falta el codificador de vídeo H\.264 del sistema\./),
    ).toBeInTheDocument();
    expect(orden("sudo apt install gstreamer1.0-plugins-bad")).toBeInTheDocument();
    expect(guardar("MP4")).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "GIF" }));
    expect(guardar("GIF")).toBeEnabled();
    expect(screen.queryByText(/falta el codificador de vídeo/)).not.toBeInTheDocument();
  });

  it("sin codificador de vídeo, ni Ctrl+S ni los otros dos botones piden el MP4, y el GIF sí sale con Ctrl+S", async () => {
    responde("falta_para_el_mp4", { que: "video", orden: "sudo apt install gstreamer1.0-plugins-bad" });
    pintar();
    await screen.findByText(/Para guardar en MP4 falta el codificador de vídeo/);
    expect(screen.getByRole("button", { name: /Copiar/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Guardar en…/ })).toBeDisabled();
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    fireEvent.keyDown(window, { key: "s", ctrlKey: true, shiftKey: true });
    await new Promise((r) => setTimeout(r, 30));
    expect(llamadas.some((l) => l.comando === "export_media")).toBe(false);
    expect(llamadas.some((l) => l.comando === "pick_save_file")).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "GIF" }));
    expect(screen.getByRole("button", { name: /Copiar/ })).toBeEnabled();
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await waitFor(() => expect(loExportado()).toBeTruthy());
    expect(loExportado().format).toBe("gif");
  });

  it("la orden se parte por los espacios y nunca por un guion, y al copiarla sale entera", async () => {
    responde("falta_para_el_mp4", { que: "video", orden: "sudo dnf install gstreamer1-plugin-openh264" });
    pintar();
    const codigo = await waitFor(() => orden("sudo dnf install gstreamer1-plugin-openh264"));
    const palabras = [...codigo.querySelectorAll("span")];
    expect(palabras.map((p) => p.textContent)).toEqual(["sudo", "dnf", "install", "gstreamer1-plugin-openh264"]);
    for (const p of palabras) expect(p.className).toContain("whitespace-nowrap");
  });

  it("sin codificador de sonido: el MP4 se guarda, pero sin sonido, y lo avisa", async () => {
    responde("falta_para_el_mp4", { que: "sonido", orden: null });
    pintar();
    expect(await screen.findByText(/el MP4 se guardará sin sonido/)).toBeInTheDocument();
    expect(screen.getByText(/Instala los plugins «bad» de GStreamer/)).toBeInTheDocument();
    expect(guardar("MP4")).toBeEnabled();
    fireEvent.click(guardar("MP4"));
    await waitFor(() => expect(loExportado()).toBeTruthy());
    expect(loExportado().audio).toBe(false);
  });

  it("en un AppImage que no ve los codificadores del sistema no manda instalar nada: dice qué paquete usar", async () => {
    responde("falta_para_el_mp4", { que: "video", orden: null, appimage: true });
    pintar();
    expect(
      await screen.findByText(
        "Esta AppImage no puede usar los codificadores de tu sistema, y sin ellos no hay MP4. Instala winshotx con el paquete .deb o .rpm.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Se instala con/)).not.toBeInTheDocument();
    expect(screen.queryByText(/plugins «bad»/)).not.toBeInTheDocument();
    expect(guardar("MP4")).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "GIF" }));
    expect(guardar("GIF")).toBeEnabled();
  });

  it("una grabación sin sonido no avisa de que falta el de sonido: no pierde nada", async () => {
    responde("falta_para_el_mp4", { que: "sonido", orden: "sudo apt install gstreamer1.0-plugins-bad" });
    pintar({ hasAudio: false });
    fireEvent.click(guardar("MP4"));
    await waitFor(() => expect(loExportado()).toBeTruthy());
    expect(screen.queryByText(/codificador de sonido/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Se instala con/)).not.toBeInTheDocument();
  });

  it("si no falta nada (o en Windows) no se dice nada y el MP4 lleva su sonido", async () => {
    responde("falta_para_el_mp4", null);
    pintar();
    fireEvent.click(guardar("MP4"));
    await waitFor(() => expect(loExportado()).toBeTruthy());
    expect(loExportado().audio).toBe(true);
    expect(screen.queryByText(/falta/i)).not.toBeInTheDocument();
  });

  it("el aviso sale en el idioma de la app", async () => {
    aplicarIdioma("en");
    responde("falta_para_el_mp4", { que: "video", orden: "sudo dnf install gstreamer1-plugin-openh264" });
    pintar();
    expect(await screen.findByText(/To save as MP4, the system's H\.264 video encoder is missing\./)).toBeInTheDocument();
    expect(orden("sudo dnf install gstreamer1-plugin-openh264")).toBeInTheDocument();
  });
});
