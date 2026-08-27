/**
 * La tira de miniaturas del editor.
 *
 * Se prueban dos cosas que ya se han roto: que la cabecera hable el idioma de la
 * aplicacion (estuvo escrita en espannol a pelo hasta el 27 de agosto de 2026, asi que
 * salia en castellano con la app en ingles), y que el marcador B no se coma un fotograma
 * de mas al soltarlo donde estaba.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FrameStrip } from "./FrameStrip";
import { aplicarIdioma } from "../../lib/i18n";
import type { FrameMeta } from "../../lib/types";

/** Una grabacion de `cuantos` fotogramas a 10 por segundo. */
function grabacion(cuantos: number): FrameMeta[] {
  return Array.from({ length: cuantos }, (_, i) => ({
    index: i,
    timestampMs: i * 100,
    durationMs: 100,
    path: `C:/ct/frames/${i}.png`,
    thumbPath: `C:/ct/thumbs/${i}.jpg`,
  }));
}

function pintar(props: Partial<React.ComponentProps<typeof FrameStrip>> = {}) {
  const frames = props.frames ?? grabacion(10);
  const cambios = {
    onChangeIn: vi.fn(),
    onChangeOut: vi.fn(),
    onScrub: vi.fn(),
  };
  render(
    <FrameStrip
      frames={frames}
      inIndex={0}
      outIndex={frames.length - 1}
      currentIndex={0}
      {...cambios}
      {...props}
    />,
  );
  return cambios;
}

beforeEach(() => aplicarIdioma("es"));

describe("la cabecera de la tira", () => {
  it("dice por que fotograma va, con el numero que ve una persona", () => {
    pintar({ currentIndex: 2 });
    // Tercero de la lista, pero para quien mira es el 3 de 10.
    expect(screen.getByText(/Fotograma 3 de 10/)).toBeInTheDocument();
  });

  it("en ingles no se queda ni una palabra en espannol", () => {
    aplicarIdioma("en");
    pintar({ currentIndex: 2, inIndex: 1, outIndex: 5 });
    expect(screen.getByText(/Frame 3 of 10/)).toBeInTheDocument();
    expect(screen.getByText(/Crop 2 to 6/)).toBeInTheDocument();
    expect(screen.getByText(/5 frames/)).toBeInTheDocument();
    expect(screen.queryByText(/Fotograma|Recorte|fotogramas/)).toBeNull();
  });

  it("dice «1 fotograma» en singular cuando solo queda uno", () => {
    pintar({ inIndex: 4, outIndex: 4 });
    expect(screen.getByText(/1 fotograma\b/)).toBeInTheDocument();
    expect(screen.queryByText(/1 fotogramas/)).toBeNull();
  });

  it("y «1 frame», no «1 frames», tambien en ingles", () => {
    aplicarIdioma("en");
    pintar({ inIndex: 4, outIndex: 4 });
    expect(screen.getByText(/1 frame\b/)).toBeInTheDocument();
  });

  it("cuenta la duracion de lo que se queda, no la de la grabacion entera", () => {
    // Del 2 al 5 son cuatro fotogramas de 100 ms: 0,40 s.
    pintar({ inIndex: 2, outIndex: 5 });
    expect(screen.getByText(/0:00\.40/)).toBeInTheDocument();
  });
});

describe("los marcadores del recorte", () => {
  /** La tira mide 56 px por fotograma, y en las pruebas no hay layout de verdad. */
  const ANCHO = 56;

  it("el marcador B se queda en su fotograma al soltarlo donde estaba", () => {
    // El fallo de siempre: B se dibuja en el borde DERECHO de su fotograma, asi que
    // redondear hacia abajo lo movia uno a la derecha y ese fotograma de mas se colaba
    // en la exportacion.
    const { onChangeOut } = pintar({ inIndex: 0, outIndex: 5 });
    fireEvent.pointerDown(screen.getByTitle("Marca B (tecla O)"));
    fireEvent.pointerMove(window, { clientX: (5 + 1) * ANCHO });
    expect(onChangeOut).toHaveBeenLastCalledWith(5);
  });

  it("el marcador A se queda en el fotograma sobre el que se suelta", () => {
    const { onChangeIn } = pintar();
    fireEvent.pointerDown(screen.getByTitle("Marca A (tecla I)"));
    fireEvent.pointerMove(window, { clientX: 3 * ANCHO + 10 });
    expect(onChangeIn).toHaveBeenLastCalledWith(3);
  });

  it("ninguno de los dos se sale de la tira por mucho que se arrastre", () => {
    const { onChangeIn, onChangeOut } = pintar();
    fireEvent.pointerDown(screen.getByTitle("Marca A (tecla I)"));
    fireEvent.pointerMove(window, { clientX: -5000 });
    expect(onChangeIn).toHaveBeenLastCalledWith(0);

    fireEvent.pointerUp(window);
    fireEvent.pointerDown(screen.getByTitle("Marca B (tecla O)"));
    fireEvent.pointerMove(window, { clientX: 5000 });
    expect(onChangeOut).toHaveBeenLastCalledWith(9);
  });

  it("los tooltips de los marcadores tambien se traducen", () => {
    aplicarIdioma("en");
    pintar();
    expect(screen.getByTitle("Mark A (key I)")).toBeInTheDocument();
    expect(screen.getByTitle("Mark B (key O)")).toBeInTheDocument();
  });
});
