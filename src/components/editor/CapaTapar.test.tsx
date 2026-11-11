/**
 * La capa del mosaico en la vista previa (S9, D4). El mosaico de verdad se pinta en un lienzo
 * y eso se mira en Edge (`probar-editor.mjs`); aquí, cuándo está puesto cada uno y que copia
 * el encuadre del zoom del vídeo, para irse con el dato.
 */
import { render } from "@testing-library/react";
import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { CapaTapar } from "./CapaTapar";
import type { Tapa } from "../../lib/contrato";

const TAPAS: Tapa[] = [
  { desdeMs: 1000, hastaMs: 2000, x: 0.1, y: 0.1, ancho: 0.2, alto: 0.05 },
  { desdeMs: 1500, hastaMs: 3000, x: 0.5, y: 0.5, ancho: 0.2, alto: 0.05 },
];

function pintar(msParado: number, tapas = TAPAS) {
  const videoRef = createRef<HTMLVideoElement>();
  return render(
    <CapaTapar videoRef={videoRef} tapas={tapas} ancho={1280} alto={800} msParado={msParado} reproduciendo={false} />,
  ).container;
}

describe("la capa del mosaico", () => {
  it("cuenta las que están puestas en el instante que se enseña", () => {
    for (const [ms, puestas] of [[500, 0], [1000, 1], [1700, 2], [2500, 1], [3000, 0]] as const) {
      const lienzo = pintar(ms).querySelector("[data-capa-tapar]")!;
      expect(lienzo.getAttribute("data-puestas"), `a los ${ms} ms`).toBe(String(puestas));
      lienzo.parentElement!.remove();
    }
  });

  it("sin nada tapado no pinta nada", () => {
    expect(pintar(0, []).querySelector("[data-capa-tapar]")).toBeNull();
  });

  it("no se queda con el ratón: lo de debajo se sigue pudiendo dibujar", () => {
    const lienzo = pintar(1000).querySelector("[data-capa-tapar]")!;
    expect(lienzo.className).toContain("pointer-events-none");
  });

  it("copia el encuadre del zoom que lleva el vídeo, para que el mosaico se acerque con el dato", async () => {
    const video = document.createElement("video");
    video.style.transform = "translate(-100px, -50px) scale(2, 2)";
    const videoRef = { current: video };
    const { container } = render(
      <CapaTapar videoRef={videoRef} tapas={TAPAS} ancho={1280} alto={800} msParado={1000} reproduciendo={false} />,
    );
    const lienzo = container.querySelector<HTMLCanvasElement>("[data-capa-tapar]")!;
    await new Promise((listo) => requestAnimationFrame(() => requestAnimationFrame(listo)));
    expect(lienzo.style.transform).toBe("translate(-100px, -50px) scale(2, 2)");
    expect(lienzo.style.transformOrigin).toBe("0 0");
  });
});
