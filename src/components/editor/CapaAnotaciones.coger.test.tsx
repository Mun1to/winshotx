/**
 * Lo dibujado se coge, se mueve y se reescribe (S23).
 *
 * Munito, con una foto de un texto rojo ya puesto: «haz que el texto y los elementos en
 * general se puedan mover libremente después de añadirlos». Lo que se comprueba aquí es lo
 * que puede salir mal: que pulsar encima coja la marca en vez de dibujar otra, que moverla
 * la mueva ENTERA (con su trazo y su curva), que el cambio salga una sola vez al soltar
 * (para que Ctrl+Z la devuelva de un golpe), y que el doble clic reescriba el mismo texto
 * en vez de poner otro.
 *
 * La capa mide 400 x 200 desde (100, 50), así que 40 píxeles a la derecha son 0,1 de ancho
 * y 20 hacia abajo, 0,1 de alto. El arrastre se sigue desde la ventana, que es donde llega
 * el ratón aunque se salga de la imagen.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CapaAnotaciones } from "./CapaAnotaciones";
import { aplicarIdioma } from "../../lib/i18n";
import type { Anotacion } from "../../lib/anotaciones";

beforeEach(() => {
  aplicarIdioma("es");
  Element.prototype.getBoundingClientRect = vi.fn(() => ({
    x: 100,
    y: 50,
    left: 100,
    top: 50,
    right: 500,
    bottom: 250,
    width: 400,
    height: 200,
    toJSON: () => ({}),
  })) as unknown as typeof Element.prototype.getBoundingClientRect;
});

function pintar(props: Partial<React.ComponentProps<typeof CapaAnotaciones>> = {}) {
  const onAnadir = vi.fn();
  const onElegir = vi.fn();
  const onCambiar = vi.fn();
  const onBorrar = vi.fn();
  const { container } = render(
    <CapaAnotaciones
      herramienta="box"
      color="#ef4444"
      anotaciones={[]}
      onAnadir={onAnadir}
      anchoImagen={1920}
      onElegir={onElegir}
      onCambiar={onCambiar}
      onBorrar={onBorrar}
      {...props}
    />,
  );
  return { container, svg: container.querySelector("svg")!, onAnadir, onElegir, onCambiar, onBorrar };
}

/** Lo que se pulsa de la marca `i`: su silueta invisible. */
const silueta = (container: HTMLElement, i: number) =>
  container.querySelector(`[data-agarre="${i}"] [data-silueta]`)!.firstElementChild!;
const zonaDe = (container: HTMLElement, i: number) =>
  container.querySelector(`[data-agarre="${i}"] [data-silueta]`) as SVGGElement;

const caja: Anotacion = { kind: "box", x1: 0.1, y1: 0.1, x2: 0.5, y2: 0.5, color: "#ef4444", text: "" };
const trazo: Anotacion = {
  kind: "pen",
  x1: 0.2,
  y1: 0.2,
  x2: 0.4,
  y2: 0.3,
  color: "#ef4444",
  text: "",
  puntos: [
    [0.2, 0.2],
    [0.3, 0.4],
    [0.4, 0.3],
  ],
};
const texto: Anotacion = { kind: "text", x1: 0.6, y1: 0.2, x2: 0.6, y2: 0.2, color: "#0a9bff", text: "aquí" };

describe("coger lo dibujado", () => {
  it("pulsar encima de una marca la elige, y no empieza otra", () => {
    const { container, onAnadir, onElegir } = pintar({ anotaciones: [caja] });
    fireEvent.pointerDown(silueta(container, 0), { clientX: 140, clientY: 70 });
    fireEvent.pointerUp(window, { clientX: 140, clientY: 70 });
    expect(onElegir).toHaveBeenCalledWith(0);
    expect(onAnadir).not.toHaveBeenCalled();
  });

  it("coger una marca le quita el foco al campo del panel, para que Supr la borre a ella", () => {
    const { container } = pintar({ anotaciones: [caja] });
    const ancho = document.createElement("input");
    document.body.appendChild(ancho);
    ancho.focus();
    fireEvent.pointerDown(silueta(container, 0), { clientX: 140, clientY: 70 });
    fireEvent.pointerUp(window, { clientX: 140, clientY: 70 });
    expect(document.activeElement).not.toBe(ancho);
    ancho.remove();
  });

  it("pulsar donde no hay nada sigue dibujando, aunque haya marcas", () => {
    const { svg, onAnadir, onElegir } = pintar({ anotaciones: [caja] });
    fireEvent.pointerDown(svg, { clientX: 350, clientY: 180 });
    fireEvent.pointerMove(svg, { clientX: 450, clientY: 230 });
    fireEvent.pointerUp(svg);
    expect(onAnadir).toHaveBeenCalledTimes(1);
    expect(onElegir).not.toHaveBeenCalled();
  });

  it("arrastrarla la mueve entera, con su trazo, y el cambio sale una sola vez al soltar", () => {
    const { container, onCambiar } = pintar({ anotaciones: [trazo] });
    fireEvent.pointerDown(silueta(container, 0), { clientX: 200, clientY: 100 });
    for (let i = 1; i <= 4; i++) fireEvent.pointerMove(window, { clientX: 200 + i * 10, clientY: 100 + i * 5 });
    expect(onCambiar).not.toHaveBeenCalled();
    fireEvent.pointerUp(window, { clientX: 240, clientY: 120 });
    expect(onCambiar).toHaveBeenCalledTimes(1);
    const [indice, movida] = onCambiar.mock.calls[0] as [number, Anotacion];
    expect(indice).toBe(0);
    expect(movida.x1).toBeCloseTo(0.3, 5);
    expect(movida.y1).toBeCloseTo(0.3, 5);
    expect(movida.x2).toBeCloseTo(0.5, 5);
    expect(movida.puntos!.map(([x, y]) => [x.toFixed(3), y.toFixed(3)])).toEqual([
      ["0.300", "0.300"],
      ["0.400", "0.500"],
      ["0.500", "0.400"],
    ]);
  });

  it("mientras se arrastra se ve ya movida, antes de soltar", () => {
    const { container } = pintar({ anotaciones: [caja] });
    fireEvent.pointerDown(silueta(container, 0), { clientX: 140, clientY: 70 });
    fireEvent.pointerMove(window, { clientX: 180, clientY: 90 });
    const rect = container.querySelector('[data-agarre="0"] > rect')!;
    // 0,1 + 0,1, en el lienzo de 1000.
    expect(rect.getAttribute("x")).toBe("200");
    expect(rect.getAttribute("y")).toBe("200");
    fireEvent.pointerUp(window, { clientX: 180, clientY: 90 });
  });

  it("un clic sin arrastrar solo la elige: no la mueve ni cuenta como cambio", () => {
    const { container, onCambiar } = pintar({ anotaciones: [caja] });
    fireEvent.pointerDown(silueta(container, 0), { clientX: 140, clientY: 70 });
    // Un temblor de un píxel no es un arrastre.
    fireEvent.pointerMove(window, { clientX: 141, clientY: 70 });
    fireEvent.pointerUp(window, { clientX: 141, clientY: 70 });
    expect(onCambiar).not.toHaveBeenCalled();
  });

  it("contra el borde se para entera, sin aplastarse ni salirse", () => {
    const { container, onCambiar } = pintar({ anotaciones: [caja] });
    fireEvent.pointerDown(silueta(container, 0), { clientX: 140, clientY: 70 });
    fireEvent.pointerMove(window, { clientX: 9000, clientY: 9000 });
    fireEvent.pointerUp(window, { clientX: 9000, clientY: 9000 });
    const movida: Anotacion = onCambiar.mock.calls[0][1];
    expect(movida.x2).toBeCloseTo(1, 5);
    expect(movida.y2).toBeCloseTo(1, 5);
    expect(movida.x2 - movida.x1).toBeCloseTo(0.4, 5);
    expect(movida.y2 - movida.y1).toBeCloseTo(0.4, 5);
  });

  it("si el sistema se lleva el ratón a medias, vuelve a donde estaba", () => {
    const { container, onCambiar } = pintar({ anotaciones: [caja] });
    fireEvent.pointerDown(silueta(container, 0), { clientX: 140, clientY: 70 });
    fireEvent.pointerMove(window, { clientX: 200, clientY: 100 });
    fireEvent.pointerCancel(window);
    expect(onCambiar).not.toHaveBeenCalled();
    expect(container.querySelector('[data-agarre="0"] > rect')!.getAttribute("x")).toBe("100");
  });

  it("sin herramienta también se coge, y lo demás de la imagen sigue dejando pasar los clics", () => {
    const { container, svg, onElegir } = pintar({ anotaciones: [caja], herramienta: null });
    expect(svg.getAttribute("class")).toContain("pointer-events-none");
    expect(zonaDe(container, 0).style.pointerEvents).toBe("stroke");
    expect(zonaDe(container, 0).style.cursor).toBe("move");
    fireEvent.pointerDown(silueta(container, 0), { clientX: 140, clientY: 70 });
    expect(onElegir).toHaveBeenCalledWith(0);
  });

  it("una línea fina se coge aunque se falle por unos píxeles", () => {
    const linea: Anotacion = { kind: "line", x1: 0.1, y1: 0.5, x2: 0.9, y2: 0.5, color: "#ef4444", text: "" };
    const { container } = pintar({ anotaciones: [linea] });
    const visible = container.querySelector('[data-marca="line"]')!;
    // Lo que se pulsa es 6 píxeles más ancho a cada lado que lo que se ve, y en píxeles de
    // la pantalla, no del lienzo estirado.
    expect(Number(zonaDe(container, 0).getAttribute("stroke-width"))).toBe(
      Number(visible.getAttribute("stroke-width")) + 12,
    );
    expect(silueta(container, 0).getAttribute("vector-effect")).toBe("non-scaling-stroke");
  });

  it("el rectángulo se coge por su borde, y el resaltado, el tapado y el paso por dentro", () => {
    const varias: Anotacion[] = [
      caja,
      { kind: "highlight", x1: 0.1, y1: 0.6, x2: 0.3, y2: 0.7, color: "#fbbf24", text: "" },
      { kind: "blur", x1: 0.5, y1: 0.6, x2: 0.7, y2: 0.7, color: "", text: "" },
      { kind: "step", x1: 0.8, y1: 0.8, x2: 0.8, y2: 0.8, color: "#ef4444", text: "1" },
      { kind: "focus", x1: 0.2, y1: 0.2, x2: 0.4, y2: 0.4, color: "", text: "" },
    ];
    const { container } = pintar({ anotaciones: varias });
    // El foco por su borde: dentro de él es donde se dibuja lo que se quiere señalar.
    expect([0, 1, 2, 4].map((i) => zonaDe(container, i).style.pointerEvents)).toEqual([
      "stroke",
      "all",
      "all",
      "stroke",
    ]);
    // El paso va en HTML (issue 19), y se coge por todo su disco.
    const paso = container.querySelector<HTMLElement>('[data-marca="step"][data-agarre="3"]')!;
    expect(paso.style.pointerEvents).toBe("auto");
  });

  it("mientras se coloca el punto de un zoom, lo dibujado no se coge", () => {
    const { container } = pintar({ anotaciones: [caja, texto], herramienta: null, quieta: true });
    expect(zonaDe(container, 0).style.pointerEvents).toBe("none");
    expect(screen.getByText("aquí").style.pointerEvents).toBe("none");
  });

  it("la elegida lleva su halo pegado a su forma, por debajo de ella, y las demás no", () => {
    const { container } = pintar({ anotaciones: [caja, trazo], elegida: 1 });
    expect(container.querySelectorAll("[data-halo]")).toHaveLength(1);
    const halo = container.querySelector('[data-agarre="1"] [data-halo]')!;
    // La misma forma que la marca, no un recuadro alrededor.
    expect(halo.querySelector("polyline")).not.toBeNull();
    expect(halo.querySelector("rect")).toBeNull();
    const hijos = [...container.querySelector('[data-agarre="1"]')!.children];
    expect(hijos.indexOf(halo)).toBeLessThan(hijos.findIndex((h) => h.getAttribute("data-marca") === "pen"));
  });

  it("un texto elegido se marca siguiendo sus letras", () => {
    pintar({ anotaciones: [texto], elegida: 0 });
    expect(screen.getByText("aquí").style.filter).toContain("drop-shadow");
  });

  it("pulsar en otro sitio suelta lo elegido, pero tocar un botón de la barra no", () => {
    const { svg, onElegir } = pintar({ anotaciones: [caja], elegida: 0, herramienta: null });
    const boton = document.createElement("button");
    document.body.appendChild(boton);
    fireEvent.pointerDown(boton);
    expect(onElegir).not.toHaveBeenCalled();
    fireEvent.pointerDown(svg.parentElement!);
    expect(onElegir).toHaveBeenCalledWith(null);
    boton.remove();
  });
});

describe("reescribir un texto con doble clic", () => {
  const otra: Anotacion = { kind: "box", x1: 0.1, y1: 0.1, x2: 0.3, y2: 0.3, color: "#ef4444", text: "" };

  const pintarTexto = (props: Partial<React.ComponentProps<typeof CapaAnotaciones>> = {}) =>
    pintar({ anotaciones: [texto], herramienta: null, ...props });

  function dobleClic(span: HTMLElement) {
    for (let i = 0; i < 2; i++) {
      fireEvent.pointerDown(span, { clientX: 345, clientY: 95 });
      fireEvent.pointerUp(window, { clientX: 345, clientY: 95 });
    }
  }

  it("abre el mismo campo de escribir, en su sitio, con su color y con lo que decía", () => {
    pintarTexto();
    dobleClic(screen.getByText("aquí"));
    const campo = screen.getByRole("textbox", { name: "Texto" }) as HTMLInputElement;
    expect(campo.value).toBe("aquí");
    expect(campo.style.left).toBe("60%");
    expect(campo.style.top).toBe("20%");
    expect(campo.style.color).toBe("#0a9bff");
    expect(document.activeElement).toBe(campo);
    // Y el texto de debajo no se ve a la vez, que serían dos.
    expect(screen.queryByText("aquí")).toBeNull();
  });

  it("Intro deja lo nuevo en el mismo texto, no un texto más", () => {
    const { onAnadir, onCambiar } = pintarTexto();
    dobleClic(screen.getByText("aquí"));
    const campo = screen.getByRole("textbox", { name: "Texto" });
    fireEvent.change(campo, { target: { value: "justo aquí" } });
    fireEvent.keyDown(campo, { key: "Enter" });
    expect(onAnadir).not.toHaveBeenCalled();
    expect(onCambiar).toHaveBeenCalledWith(0, { ...texto, text: "justo aquí" });
  });

  it("pulsar fuera también lo guarda", () => {
    const { onCambiar } = pintarTexto({ herramienta: "box" });
    dobleClic(screen.getByText("aquí"));
    const campo = screen.getByRole("textbox", { name: "Texto" });
    fireEvent.change(campo, { target: { value: "otro" } });
    fireEvent.blur(campo);
    expect(onCambiar).toHaveBeenCalledWith(0, { ...texto, text: "otro" });
  });

  it("Escape lo deja como estaba", () => {
    const { onCambiar, onBorrar } = pintarTexto();
    dobleClic(screen.getByText("aquí"));
    const campo = screen.getByRole("textbox", { name: "Texto" });
    fireEvent.change(campo, { target: { value: "no va" } });
    fireEvent.keyDown(campo, { key: "Escape" });
    fireEvent.blur(campo);
    expect(onCambiar).not.toHaveBeenCalled();
    expect(onBorrar).not.toHaveBeenCalled();
    expect(screen.getByText("aquí")).toBeInTheDocument();
  });

  it("vaciarlo del todo y pulsar Intro lo quita", () => {
    const { onBorrar, onCambiar } = pintarTexto();
    dobleClic(screen.getByText("aquí"));
    const campo = screen.getByRole("textbox", { name: "Texto" });
    fireEvent.change(campo, { target: { value: "  " } });
    fireEvent.keyDown(campo, { key: "Enter" });
    expect(onBorrar).toHaveBeenCalledWith(0);
    expect(onCambiar).not.toHaveBeenCalled();
  });

  it("abrirlo y cerrarlo sin tocar nada no es un cambio", () => {
    const { onCambiar } = pintarTexto();
    dobleClic(screen.getByText("aquí"));
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Texto" }), { key: "Enter" });
    expect(onCambiar).not.toHaveBeenCalled();
  });

  it("dos clics separados por más de medio segundo no son un doble clic", () => {
    const reloj = vi.spyOn(performance, "now");
    pintarTexto();
    const span = screen.getByText("aquí");
    reloj.mockReturnValue(1000);
    fireEvent.pointerDown(span, { clientX: 345, clientY: 95 });
    fireEvent.pointerUp(window, { clientX: 345, clientY: 95 });
    reloj.mockReturnValue(1700);
    fireEvent.pointerDown(span, { clientX: 345, clientY: 95 });
    fireEvent.pointerUp(window, { clientX: 345, clientY: 95 });
    reloj.mockRestore();
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("con la herramienta de texto, un clic encima elige el texto en vez de abrir otro", () => {
    const { onElegir, onAnadir } = pintarTexto({ herramienta: "text" });
    fireEvent.pointerDown(screen.getByText("aquí"), { clientX: 345, clientY: 95 });
    fireEvent.pointerUp(window, { clientX: 345, clientY: 95 });
    expect(onElegir).toHaveBeenCalledWith(0);
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(onAnadir).not.toHaveBeenCalled();
  });

  it("y se arrastra como las demás", () => {
    const { onCambiar } = pintarTexto();
    fireEvent.pointerDown(screen.getByText("aquí"), { clientX: 345, clientY: 95 });
    fireEvent.pointerMove(window, { clientX: 305, clientY: 135 });
    fireEvent.pointerUp(window, { clientX: 305, clientY: 135 });
    const movido: Anotacion = onCambiar.mock.calls[0][1];
    expect(movido.x1).toBeCloseTo(0.5, 5);
    expect(movido.y1).toBeCloseTo(0.4, 5);
    expect(movido.text).toBe("aquí");
  });

  it("con un texto reescribiéndose, pulsar otra marca lo guarda y nada más", () => {
    const { container, onCambiar, onElegir } = pintarTexto({ anotaciones: [texto, otra] });
    dobleClic(screen.getByText("aquí"));
    onElegir.mockClear();
    fireEvent.change(screen.getByRole("textbox", { name: "Texto" }), { target: { value: "hecho" } });
    fireEvent.pointerDown(silueta(container, 1), { clientX: 140, clientY: 70 });
    expect(onCambiar).toHaveBeenCalledWith(0, { ...texto, text: "hecho" });
    expect(onElegir).not.toHaveBeenCalledWith(1);
  });
});
