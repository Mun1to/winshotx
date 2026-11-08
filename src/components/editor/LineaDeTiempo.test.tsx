/**
 * La línea de tiempo del editor.
 *
 * Lo que se prueba es lo que Munir pidió con estas palabras: «que al hacer un vídeo se
 * pueda recortar desde el timeline». La tira de antes ya tenía dos marcadores, pero ponía
 * cada fotograma a 56 px y se desplazaba de lado: con un minuto de grabación, el final
 * estaba a cien mil píxeles. Aquí se comprueba que la grabación entera cabe, que las asas
 * cortan donde se sueltan, que no se pisan, y que con el teclado también se puede.
 */
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useRef, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LineaDeTiempo } from "./LineaDeTiempo";
import { aplicarIdioma } from "../../lib/i18n";
import { TRAMO_MINIMO_MS } from "../../lib/tramo";
import type { FrameMeta } from "../../lib/types";

/** Fotogramas seguidos con estas duraciones, como los da Rust. */
function fotogramas(duraciones: number[]): FrameMeta[] {
  let ms = 0;
  return duraciones.map((durationMs, index) => {
    const f = { index, timestampMs: ms, durationMs, thumbPath: `C:/t/${index}.jpg` };
    ms += durationMs;
    return f;
  });
}

/** Diez segundos, a diez fotogramas por segundo. */
const DIEZ_SEGUNDOS = fotogramas(Array(100).fill(100));

/** La tira empieza en x = 100 y mide 1000: así cada píxel son diez milisegundos. */
const IZQUIERDA = 100;
const ANCHO = 1000;
const x = (parte: number) => IZQUIERDA + parte * ANCHO;

const original = HTMLElement.prototype.getBoundingClientRect;
beforeEach(() => {
  aplicarIdioma("es");
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.hasAttribute("data-pista")) {
      return {
        left: IZQUIERDA,
        top: 0,
        width: ANCHO,
        height: 56,
        right: IZQUIERDA + ANCHO,
        bottom: 56,
        x: IZQUIERDA,
        y: 0,
        toJSON() {},
      } as DOMRect;
    }
    return original.call(this);
  });
});
afterEach(() => vi.restoreAllMocks());

/**
 * La línea montada con su estado de verdad, como la usa el editor: lo que devuelve
 * `onTramo` vuelve a entrar, así que lo que se ve después de arrastrar es lo de verdad.
 */
function Montada({
  frames = DIEZ_SEGUNDOS,
  desde = 0,
  hasta,
  alTramo,
  alIr = () => {},
  alFinal = () => {},
  reproduciendo = false,
  video,
}: {
  frames?: FrameMeta[];
  desde?: number;
  hasta?: number;
  alTramo?: (d: number, h: number, mueve: string) => void;
  alIr?: (ms: number) => void;
  alFinal?: () => void;
  reproduciendo?: boolean;
  video?: { currentTime: number };
}) {
  const total = frames[frames.length - 1].timestampMs + frames[frames.length - 1].durationMs;
  const [tramo, setTramo] = useState({ desde, hasta: hasta ?? total });
  const videoRef = useRef<HTMLVideoElement | null>((video ?? null) as HTMLVideoElement | null);
  return (
    <LineaDeTiempo
      frames={frames}
      desdeMs={tramo.desde}
      hastaMs={tramo.hasta}
      posicionMs={0}
      videoRef={videoRef}
      reproduciendo={reproduciendo}
      proporcion={16 / 10}
      delante={<button type="button">play</button>}
      onTramo={(d, h, mueve) => {
        setTramo({ desde: d, hasta: h });
        alTramo?.(d, h, mueve);
      }}
      onIr={alIr}
      onFinal={alFinal}
    />
  );
}

const principio = () => screen.getByRole("slider", { name: /Principio|Start/ });
const final = () => screen.getByRole("slider", { name: /Final|End/ });

describe("la grabación entera cabe en la tira", () => {
  it("diez minutos son una docena de miniaturas, no dieciocho mil", () => {
    // La tira de antes montaba una imagen por fotograma: diez minutos a 30 fps eran
    // dieciocho mil imágenes en la página y un millón de píxeles de ancho.
    const { container } = render(<Montada frames={fotogramas(Array(18_000).fill(33))} />);
    const miniaturas = container.querySelectorAll("img, [data-miniatura-pendiente]");
    expect(miniaturas.length).toBeGreaterThan(5);
    expect(miniaturas.length).toBeLessThan(20);
  });

  it("y no se desplaza de lado: no hay nada que buscar con la rueda", () => {
    const { container } = render(<Montada />);
    expect(container.querySelector(".overflow-x-auto")).toBeNull();
  });

  it("una pantalla quieta ocupa lo que dura, aunque sea un solo fotograma", () => {
    // Un segundo moviéndose y nueve quietos: la quietud se lleva casi toda la tira.
    const { container } = render(<Montada frames={fotogramas([...Array(10).fill(100), 9_000])} />);
    const fuentes = [...container.querySelectorAll("img")].map((img) => img.getAttribute("src"));
    const quieta = fuentes.filter((src) => src?.includes("/10.jpg")).length;
    expect(quieta / fuentes.length).toBeGreaterThan(0.8);
  });
});

describe("recortar arrastrando las asas", () => {
  it("el asa del principio corta donde se suelta", () => {
    const alTramo = vi.fn();
    render(<Montada alTramo={alTramo} />);
    fireEvent.pointerDown(principio());
    fireEvent.pointerMove(window, { clientX: x(0.25) });
    fireEvent.pointerUp(window);
    expect(alTramo).toHaveBeenLastCalledWith(2_500, 10_000, "desde");
  });

  it("y la del final, también", () => {
    const alTramo = vi.fn();
    render(<Montada alTramo={alTramo} />);
    fireEvent.pointerDown(final());
    fireEvent.pointerMove(window, { clientX: x(0.7) });
    fireEvent.pointerUp(window);
    expect(alTramo).toHaveBeenLastCalledWith(0, 7_000, "hasta");
  });

  it("ninguna se pasa de la otra: se para dejando lo mínimo", () => {
    const alTramo = vi.fn();
    render(<Montada desde={2_000} hasta={6_000} alTramo={alTramo} />);
    fireEvent.pointerDown(principio());
    fireEvent.pointerMove(window, { clientX: x(0.9) });
    expect(alTramo).toHaveBeenLastCalledWith(6_000 - TRAMO_MINIMO_MS, 6_000, "desde");
  });

  it("ni se sale de la grabación por mucho que se arrastre", () => {
    const alTramo = vi.fn();
    render(<Montada alTramo={alTramo} />);
    fireEvent.pointerDown(final());
    fireEvent.pointerMove(window, { clientX: 99_999 });
    expect(alTramo).toHaveBeenLastCalledWith(0, 10_000, "hasta");
    fireEvent.pointerUp(window);
    fireEvent.pointerDown(principio());
    fireEvent.pointerMove(window, { clientX: -99_999 });
    expect(alTramo).toHaveBeenLastCalledWith(0, 10_000, "desde");
  });

  it("mientras se arrastra, el asa dice el tiempo exacto del corte", () => {
    render(<Montada />);
    fireEvent.pointerDown(principio());
    fireEvent.pointerMove(window, { clientX: x(0.321) });
    expect(screen.getAllByText("0:03.2").length).toBeGreaterThan(0);
  });

  it("al soltar, arrastrar ya no mueve nada", () => {
    const alTramo = vi.fn();
    render(<Montada alTramo={alTramo} />);
    fireEvent.pointerDown(principio());
    fireEvent.pointerUp(window);
    alTramo.mockClear();
    fireEvent.pointerMove(window, { clientX: x(0.5) });
    expect(alTramo).not.toHaveBeenCalled();
  });
});

describe("moverse por la grabación", () => {
  it("pulsar en la tira lleva la vista previa ahí, y arrastrar la recorre", () => {
    const alIr = vi.fn();
    const { container } = render(<Montada alIr={alIr} />);
    const pista = container.querySelector("[data-pista]")!;
    fireEvent.pointerDown(pista, { clientX: x(0.5) });
    expect(alIr).toHaveBeenLastCalledWith(5_000);
    fireEvent.pointerMove(window, { clientX: x(0.8) });
    expect(alIr).toHaveBeenLastCalledWith(8_000);
  });

  it("coger un asa no mueve la vista previa por la tira", () => {
    const alIr = vi.fn();
    render(<Montada alIr={alIr} />);
    fireEvent.pointerDown(principio());
    expect(alIr).not.toHaveBeenCalled();
  });
});

describe("con el teclado", () => {
  it("las flechas mueven el asa que tiene el foco: una décima, o un segundo con Mayús", () => {
    const alTramo = vi.fn();
    render(<Montada alTramo={alTramo} />);
    fireEvent.keyDown(principio(), { key: "ArrowRight" });
    expect(alTramo).toHaveBeenLastCalledWith(100, 10_000, "desde");
    fireEvent.keyDown(final(), { key: "ArrowLeft", shiftKey: true });
    expect(alTramo).toHaveBeenLastCalledWith(100, 9_000, "hasta");
  });

  it("y el asa dice su tiempo a quien no la ve", () => {
    render(<Montada desde={1_500} />);
    expect(principio()).toHaveAttribute("aria-valuetext", "0:01.5");
  });
});

describe("lo que se dice debajo", () => {
  it("sin recortar, cómo se recorta", () => {
    render(<Montada />);
    expect(screen.getByText("Arrastra los bordes azules para recortar")).toBeInTheDocument();
  });

  it("recortado, cuánto te quedas, y se puede volver al vídeo entero", () => {
    const alTramo = vi.fn();
    render(<Montada desde={2_000} hasta={7_000} alTramo={alTramo} />);
    expect(screen.getByText("Te quedas con 0:05.0 de 0:10.0")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Usar el vídeo entero" }));
    expect(alTramo).toHaveBeenLastCalledWith(0, 10_000, "las dos");
    expect(screen.getByText("Arrastra los bordes azules para recortar")).toBeInTheDocument();
  });

  it("en inglés no se queda ni una palabra en español", () => {
    aplicarIdioma("en");
    render(<Montada desde={2_000} />);
    expect(screen.getByText("Keeping 0:08.0 of 0:10.0")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use the whole video" })).toBeInTheDocument();
    expect(principio()).toHaveAccessibleName("Start: drag it to trim (key I)");
    expect(final()).toHaveAccessibleName("End: drag it to trim (key O)");
  });
});

describe("mientras suena", () => {
  it("al pasar el final de lo que se queda, avisa para parar ahí", async () => {
    // El aviso de tiempo del propio vídeo llega cuatro veces por segundo: con él, el corte
    // se oía hasta un cuarto de segundo de más.
    const alFinal = vi.fn();
    const video = { currentTime: 7.05 };
    render(<Montada hasta={7_000} reproduciendo video={video} alFinal={alFinal} />);
    await waitFor(() => expect(alFinal).toHaveBeenCalled());
  });

  it("y antes del final, el cabezal sigue al vídeo sin avisar de nada", async () => {
    const alFinal = vi.fn();
    const video = { currentTime: 2.5 };
    const { container } = render(<Montada reproduciendo video={video} alFinal={alFinal} />);
    await waitFor(() =>
      expect((container.querySelector("[data-cabezal]") as HTMLElement).style.left).toBe("25%"),
    );
    await act(() => new Promise((listo) => setTimeout(listo, 50)));
    expect(alFinal).not.toHaveBeenCalled();
  });
});
