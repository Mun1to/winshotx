/**
 * La webcam encima de la vista previa.
 *
 * Lo que importa es lo que llega al editor (y de ahí a Rust): que sale la de fábrica, que
 * arrastrarla la mueve a donde se suelta y que la esquina la estira. En happy-dom nada mide
 * nada, así que el hueco y la imagen se miden de mentira: un hueco de 1000 x 600 con la
 * imagen de 800 x 450 dentro, a 100 y 50 de su esquina.
 */
import { useRef } from "react";
import { fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CapaCamara } from "./CapaCamara";
import { aplicarIdioma } from "../../lib/i18n";
import { marcoDeFabrica } from "../../lib/camara";
import type { MarcoCamara } from "../../lib/contrato";
import { responde } from "../../test/preparar";

const CAMARA = { ruta: "C:\\sesion\\camara.mp4", ancho: 1280, alto: 720, desfaseMs: 500, primerMs: 0 };

beforeEach(() => {
  aplicarIdioma("es");
  responde("camara_de_la_sesion", CAMARA);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    const hueco = this.hasAttribute("data-hueco");
    const [left, top, width, height] = hueco ? [0, 0, 1000, 600] : [100, 50, 800, 450];
    return { x: left, y: top, left, top, width, height, right: left + width, bottom: top + height, toJSON: () => ({}) };
  });
});
afterEach(() => vi.restoreAllMocks());

function Banco(props: { marco: MarcoCamara | null | undefined; onMarco: (m: MarcoCamara) => void; quieta?: boolean }) {
  const hueco = useRef<HTMLDivElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  return (
    <div ref={hueco} data-hueco className="relative">
      <video ref={video} data-pantalla />
      <div className="relative">
        <CapaCamara
          sessionId="s"
          videoRef={video}
          marco={props.marco}
          onMarco={props.onMarco}
          hueco={hueco}
          aire={0}
          quieta={props.quieta ?? false}
          msParado={0}
        />
      </div>
    </div>
  );
}

const lienzo = { ancho: 800, alto: 450 };

/** Espera a que salga algo de la capa: la cámara llega de Rust y la medida, tras pintar. */
const espera = <T extends Element>(caja: HTMLElement, selector: string) =>
  waitFor(() => {
    const encontrado = caja.querySelector<T>(selector);
    if (!encontrado) throw new Error(`todavía no hay ${selector}`);
    return encontrado;
  });

describe("la cámara en la vista previa", () => {
  it("sin tocar nada, sale la de fábrica: pequeña y abajo a la derecha", async () => {
    const onMarco = vi.fn();
    render(<Banco marco={undefined} onMarco={onMarco} />);
    await waitFor(() => expect(onMarco).toHaveBeenCalled());
    expect(onMarco.mock.calls[0][0]).toEqual(marcoDeFabrica(lienzo.ancho, lienzo.alto));
  });

  it("quitada no se pinta ni vuelve la de fábrica", async () => {
    const onMarco = vi.fn();
    const { container } = render(<Banco marco={null} onMarco={onMarco} />);
    await new Promise((r) => setTimeout(r, 30));
    expect(container.querySelector("[data-camara]")).toBeNull();
    expect(onMarco).not.toHaveBeenCalled();
  });

  it("sin cámara en la sesión no se pinta ni se pone nada", async () => {
    responde("camara_de_la_sesion", null);
    const onMarco = vi.fn();
    const { container } = render(<Banco marco={undefined} onMarco={onMarco} />);
    await new Promise((r) => setTimeout(r, 30));
    expect(container.querySelector("[data-camara]")).toBeNull();
    expect(onMarco).not.toHaveBeenCalled();
  });

  it("cae donde dicen sus medidas: 144 px de lado, a 24 px de la derecha y de abajo", async () => {
    const { container } = render(<Banco marco={marcoDeFabrica(800, 450)} onMarco={vi.fn()} />);
    const burbuja = await espera<HTMLElement>(container, "[data-camara]");
    expect(burbuja.style.width).toBe("144px");
    // La capa deja pasar el ratón y la burbuja no: sin esto la heredaba y no se agarraba
    // (aquí no se ve, porque happy-dom no aplica CSS; lo cazó `probar-editor.mjs` en Edge).
    expect(burbuja.className).toContain("pointer-events-auto");
    expect(Number.parseFloat(burbuja.style.left) + 144).toBeCloseTo(800 - 24, 3);
    expect(Number.parseFloat(burbuja.style.top) + 144).toBeCloseTo(450 - 24, 3);
    // Y el vídeo es el de la cámara, no el de la pantalla.
    expect(burbuja.querySelector("video")?.getAttribute("src")).toBe("asset://C:\\sesion\\camara.mp4");
  });

  it("la cara se recorta con clip-path y no solo con el redondeo, que Edge con tarjeta gráfica ignora", async () => {
    // Con solo `border-radius` + `overflow`, en una ventana de Edge de verdad la cara salía
    // cuadrada y cortada por el borde de la captura (12 de noviembre de 2026).
    const { container } = render(<Banco marco={marcoDeFabrica(800, 450)} onMarco={vi.fn()} />);
    const recorte = await espera<HTMLElement>(container, "[data-recorte-camara]");
    expect(recorte.style.clipPath).toBe("inset(0 round 72px)");
    expect(recorte.querySelector("video")).not.toBeNull();
  });

  it("arrastrarla la mueve a donde se suelta, y el editor se entera al soltar", async () => {
    const onMarco = vi.fn();
    const { container } = render(<Banco marco={marcoDeFabrica(800, 450)} onMarco={onMarco} />);
    const burbuja = await espera<HTMLElement>(container, "[data-camara]");
    fireEvent.pointerDown(burbuja, { clientX: 700, clientY: 350, pointerId: 1 });
    fireEvent.pointerMove(burbuja, { clientX: 400, clientY: 200, pointerId: 1 });
    expect(onMarco).not.toHaveBeenCalled();
    fireEvent.pointerUp(burbuja, { clientX: 400, clientY: 200, pointerId: 1 });
    expect(onMarco).toHaveBeenCalledTimes(1);
    const movido = onMarco.mock.calls[0][0] as MarcoCamara;
    // El centro de fábrica es (704, 354): 300 a la izquierda y 150 arriba.
    expect(movido.x).toBeCloseTo(404 / 800, 4);
    expect(movido.y).toBeCloseTo(204 / 450, 4);
    expect(movido.tam).toBeCloseTo(0.18, 5);
  });

  it("la esquina que mira al centro la estira, con la de enfrente quieta", async () => {
    const onMarco = vi.fn();
    const { container } = render(<Banco marco={marcoDeFabrica(800, 450)} onMarco={onMarco} />);
    const asa = await espera<HTMLElement>(container, "[data-asa-camara]");
    fireEvent.pointerDown(asa, { clientX: 0, clientY: 0, pointerId: 1 });
    fireEvent.pointerMove(asa, { clientX: -56, clientY: -56, pointerId: 1 });
    fireEvent.pointerUp(asa, { clientX: -56, clientY: -56, pointerId: 1 });
    expect(onMarco).toHaveBeenCalledTimes(1);
    const estirado = onMarco.mock.calls[0][0] as MarcoCamara;
    // De 144 a 200 px de lado, con la esquina de abajo a la derecha en (776, 426).
    expect(estirado.tam).toBeCloseTo(200 / 800, 4);
    expect(estirado.x * 800 + 200 / 2).toBeCloseTo(776, 2);
    expect(estirado.y * 450 + 200 / 2).toBeCloseTo(426, 2);
  });

  it("mientras se dibuja o se recorta no se agarra", async () => {
    const onMarco = vi.fn();
    const { container } = render(<Banco marco={marcoDeFabrica(800, 450)} onMarco={onMarco} quieta />);
    const burbuja = await espera<HTMLElement>(container, "[data-camara]");
    expect(container.querySelector("[data-asa-camara]")).toBeNull();
    fireEvent.pointerDown(burbuja, { clientX: 700, clientY: 350, pointerId: 1 });
    fireEvent.pointerMove(burbuja, { clientX: 400, clientY: 200, pointerId: 1 });
    fireEvent.pointerUp(burbuja, { clientX: 400, clientY: 200, pointerId: 1 });
    expect(onMarco).not.toHaveBeenCalled();
  });

  it("con la pantalla parada, la cara está en su instante, con el desfase descontado", async () => {
    const { container } = render(<Banco marco={marcoDeFabrica(800, 450)} onMarco={vi.fn()} />);
    const pantalla = container.querySelector<HTMLVideoElement>("[data-pantalla]")!;
    pantalla.currentTime = 2;
    const cara = await espera<HTMLVideoElement>(container, "[data-camara] video");
    // La cámara arrancó medio segundo tarde: a los 2 s de pantalla va por el 1,5.
    await waitFor(() => expect(cara.currentTime).toBeCloseTo(1.5, 3));
  });
});
