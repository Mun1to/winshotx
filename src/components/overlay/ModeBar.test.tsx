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
import { ModeBar, SELECTOR_BARRA } from "./ModeBar";
import { aplicarIdioma } from "../../lib/i18n";
import type { CaptureMode } from "../../lib/types";

function pintar(modo: CaptureMode = "still", pantallaEntera = false, conBarra = true) {
  const acciones = {
    onChange: vi.fn(),
    onPantallaEntera: vi.fn(),
    onConBarra: vi.fn(),
    onAjustes: vi.fn(),
    onCancel: vi.fn(),
  };
  render(
    <ModeBar
      value={modo}
      pantallaEntera={pantallaEntera}
      conBarra={conBarra}
      dimmed={false}
      {...acciones}
    />,
  );
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
    expect(screen.getByLabelText("Choose what to do")).toBeInTheDocument();
    expect(screen.getByLabelText("Settings")).toBeInTheDocument();
    expect(screen.queryByLabelText("Pantalla entera")).toBeNull();
    expect(screen.queryByLabelText("Vídeo")).toBeNull();
    expect(screen.queryByLabelText("Elegir qué hacer")).toBeNull();
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

  it("la barra de acciones tambien es un interruptor, y dice como esta", () => {
    const { onConBarra } = pintar("still", false, true);
    const boton = screen.getByLabelText("Elegir qué hacer");
    expect(boton).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(boton);
    expect(onConBarra).toHaveBeenCalledWith(false);
  });

  it("apagada, pulsarla la vuelve a encender", () => {
    const { onConBarra } = pintar("still", false, false);
    expect(screen.getByLabelText("Elegir qué hacer")).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(screen.getByLabelText("Elegir qué hacer"));
    expect(onConBarra).toHaveBeenCalledWith(true);
  });

  it("grabando no sale: en video y GIF la barra siempre aparece", () => {
    // Ahi no es una preferencia, es donde se ajusta el recuadro antes de empezar a
    // grabar. Un interruptor que no hace nada es peor que no tenerlo.
    pintar("video");
    expect(screen.queryByLabelText("Elegir qué hacer")).toBeNull();
    pintar("gif");
    expect(screen.queryByLabelText("Elegir qué hacer")).toBeNull();
  });

  it("los ajustes se abren desde aqui, y ese boton no se queda pulsado", () => {
    // Desde una captura a pantalla completa no habia forma de llegar a los ajustes sin
    // cerrarla y buscar el icono de la bandeja.
    const { onAjustes } = pintar();
    const boton = screen.getByLabelText("Ajustes");
    expect(boton).not.toHaveAttribute("aria-pressed");
    fireEvent.click(boton);
    expect(onAjustes).toHaveBeenCalled();
  });

  it("deja pasar el arrastre al lienzo de detras", () => {
    // La barra tapa una franja del centro de arriba y ahi no habia forma de empezar a
    // recortar: se quedaba ella el gesto. Ahora lo deja pasar, y es el lienzo quien
    // decide si aquello fue un clic suyo o un arrastre (ver SelectionCanvas.test.tsx).
    const lienzo = vi.fn();
    render(
      <div onPointerDown={lienzo}>
        <ModeBar
          value="still"
          pantallaEntera={false}
          conBarra
          dimmed={false}
          onChange={vi.fn()}
          onPantallaEntera={vi.fn()}
          onConBarra={vi.fn()}
          onAjustes={vi.fn()}
          onCancel={vi.fn()}
        />
      </div>,
    );
    fireEvent.pointerDown(screen.getByLabelText("Foto"));
    expect(lienzo).toHaveBeenCalled();
  });

  it("se marca en el DOM con el mismo selector que busca el lienzo", () => {
    // La constante es la unica atadura entre las dos mitades: si el atributo se cae, el
    // lienzo deja de reconocer los gestos de la barra y un clic en un boton se lleva la
    // ventana de debajo.
    pintar();
    const barra = document.querySelector(SELECTOR_BARRA);
    expect(barra).not.toBeNull();
    expect(barra!.contains(screen.getByLabelText("Foto"))).toBe(true);
  });

  it("mientras se arrastra se quita del medio del todo", () => {
    // Atenuada no basta: lo que se esta recortando suele ser justo lo que hay debajo.
    render(
      <ModeBar
        value="still"
        pantallaEntera={false}
        conBarra
        dimmed
        onChange={vi.fn()}
        onPantallaEntera={vi.fn()}
        onConBarra={vi.fn()}
        onAjustes={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    const barra = document.querySelector(SELECTOR_BARRA)!;
    expect(barra).toHaveClass("opacity-0");
    expect(barra).toHaveClass("pointer-events-none");
  });
});
