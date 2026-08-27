/**
 * La barra que sale arriba mientras se elige el recorte.
 *
 * Sus botones son solo iconos, asi que TODO lo que dicen vive en el `title` y en el
 * `aria-label`. Hasta el 27 de agosto de 2026 esos dos textos se pasaban en espannol a
 * pelo, sin traducir: con la aplicacion en ingles, la unica barra que se ve al capturar
 * hablaba castellano y ademas se lo leia asi a un lector de pantalla.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ModeBar } from "./ModeBar";
import { aplicarIdioma } from "../../lib/i18n";
import type { CaptureMode } from "../../lib/types";

function pintar(modo: CaptureMode = "still", pantallaEntera = false) {
  const acciones = {
    onChange: vi.fn(),
    onPantallaEntera: vi.fn(),
    onCancel: vi.fn(),
  };
  render(<ModeBar value={modo} pantallaEntera={pantallaEntera} dimmed={false} {...acciones} />);
  return acciones;
}

beforeEach(() => aplicarIdioma("es"));

describe("lo que dice la barra", () => {
  it("nombra los tres modos y la pantalla entera", () => {
    pintar();
    expect(screen.getByLabelText("Foto")).toBeInTheDocument();
    expect(screen.getByLabelText("Vídeo")).toBeInTheDocument();
    expect(screen.getByLabelText("GIF")).toBeInTheDocument();
    expect(screen.getByLabelText("Pantalla entera")).toBeInTheDocument();
  });

  it("en ingles no queda ni un texto en espannol", () => {
    aplicarIdioma("en");
    pintar();
    expect(screen.getByLabelText("Photo")).toBeInTheDocument();
    expect(screen.getByLabelText("Video")).toBeInTheDocument();
    expect(screen.getByLabelText("Whole screen")).toBeInTheDocument();
    expect(screen.getByLabelText("Leave without capturing")).toBeInTheDocument();
    expect(screen.queryByLabelText("Pantalla entera")).toBeNull();
    expect(screen.queryByLabelText("Vídeo")).toBeNull();
  });

  it("los tooltips llevan la tecla que hace lo mismo", () => {
    pintar();
    expect(screen.getByTitle("Foto del recorte · F")).toBeInTheDocument();
    expect(screen.getByTitle("Grabar el recorte en MP4 · V")).toBeInTheDocument();
    expect(screen.getByTitle("Pantalla entera, de un clic · P")).toBeInTheDocument();
  });

  it("y los tooltips tambien se traducen", () => {
    aplicarIdioma("en");
    pintar();
    expect(screen.getByTitle("Photo of the crop · F")).toBeInTheDocument();
    expect(screen.getByTitle("Whole screen, one click · P")).toBeInTheDocument();
  });
});

describe("lo que hace la barra", () => {
  it("marca como pulsado el modo que esta puesto, y solo ese", () => {
    pintar("video");
    expect(screen.getByLabelText("Vídeo")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByLabelText("Foto")).toHaveAttribute("aria-pressed", "false");
  });

  it("avisa del modo elegido al pulsarlo", () => {
    const { onChange } = pintar("still");
    fireEvent.click(screen.getByLabelText("GIF"));
    expect(onChange).toHaveBeenCalledWith("gif");
  });

  it("pantalla entera es un interruptor: pulsarlo estando puesto lo apaga", () => {
    const { onPantallaEntera } = pintar("still", true);
    fireEvent.click(screen.getByLabelText("Pantalla entera"));
    expect(onPantallaEntera).toHaveBeenCalledWith(false);
  });

  it("la barra no deja que el clic llegue al lienzo de detras", () => {
    // Sin esto, pulsar un boton de la barra empezaba a dibujar una seleccion debajo.
    const { onChange } = pintar();
    const barra = screen.getByLabelText("Foto").parentElement!.parentElement!;
    const evento = new Event("pointerdown", { bubbles: true, cancelable: true });
    barra.dispatchEvent(evento);
    expect(onChange).not.toHaveBeenCalled();
  });
});
