/**
 * La capa donde se dibujan las marcas sobre la vista previa.
 *
 * Lo que se comprueba aquí es lo que puede salir mal de verdad: que las coordenadas se
 * guarden **de 0 a 1** y no en píxeles (que es lo que hace que el mismo dibujo valga a
 * cualquier tamaño de exportación), y que un clic sin arrastre no deje una marca invisible
 * de cero píxeles que después no hay forma de quitar.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CapaAnotaciones, estiloDelPaso, letraDelTexto, medidaDelPaso } from "./CapaAnotaciones";
import { aplicarIdioma } from "../../lib/i18n";
import { ESCALA_MAXIMA, ESCALA_MINIMA, type Anotacion } from "../../lib/anotaciones";

/**
 * En happy-dom nada tiene tamaño, así que `getBoundingClientRect` devuelve ceros y la
 * capa no sabría dividir. Se le da una caja de 400 x 200 empezando en (100, 50), que
 * además obliga a las cuentas a restar el origen y no solo a dividir.
 */
function conTamanno() {
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
}

function pintar(props: Partial<React.ComponentProps<typeof CapaAnotaciones>> = {}) {
  const onAnadir = vi.fn();
  const { container } = render(
    <CapaAnotaciones
      herramienta="box"
      color="#ef4444"
      anotaciones={[]}
      onAnadir={onAnadir}
      anchoImagen={1920}
      {...props}
    />,
  );
  return { onAnadir, svg: container.querySelector("svg")!, container };
}

beforeEach(() => {
  aplicarIdioma("es");
  conTamanno();
});

describe("dibujar una marca", () => {
  it("guarda las esquinas de 0 a 1, no en píxeles", () => {
    const { onAnadir, svg } = pintar();
    // De la esquina de arriba a la izquierda al centro de la capa.
    fireEvent.pointerDown(svg, { clientX: 100, clientY: 50 });
    fireEvent.pointerMove(svg, { clientX: 300, clientY: 150 });
    fireEvent.pointerUp(svg);

    expect(onAnadir).toHaveBeenCalledTimes(1);
    const marca: Anotacion = onAnadir.mock.calls[0][0];
    expect(marca.x1).toBeCloseTo(0, 5);
    expect(marca.y1).toBeCloseTo(0, 5);
    expect(marca.x2).toBeCloseTo(0.5, 5);
    expect(marca.y2).toBeCloseTo(0.5, 5);
  });

  it("se queda con la herramienta y el color que estén puestos", () => {
    const { onAnadir, svg } = pintar({ herramienta: "highlight", color: "#fbbf24" });
    fireEvent.pointerDown(svg, { clientX: 120, clientY: 60 });
    fireEvent.pointerMove(svg, { clientX: 400, clientY: 200 });
    fireEvent.pointerUp(svg);
    expect(onAnadir.mock.calls[0][0]).toMatchObject({
      kind: "highlight",
      color: "#fbbf24",
    });
  });

  it("un clic sin arrastre no deja una marca de cero píxeles", () => {
    // Sin esto, cada pulsación sin querer dejaba un rectángulo invisible e imposible de
    // quitar, porque no se puede pulsar encima de algo que no se ve.
    const { onAnadir, svg } = pintar();
    fireEvent.pointerDown(svg, { clientX: 200, clientY: 100 });
    fireEvent.pointerUp(svg);
    expect(onAnadir).not.toHaveBeenCalled();
  });

  it("arrastrar fuera de la capa recorta al borde en vez de salirse", () => {
    const { onAnadir, svg } = pintar();
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    fireEvent.pointerMove(svg, { clientX: 9999, clientY: 9999 });
    fireEvent.pointerUp(svg);
    const marca: Anotacion = onAnadir.mock.calls[0][0];
    expect(marca.x2).toBe(1);
    expect(marca.y2).toBe(1);
  });

  it("sin herramienta elegida no se dibuja nada y los clics pasan de largo", () => {
    const { onAnadir, svg } = pintar({ herramienta: null });
    fireEvent.pointerDown(svg, { clientX: 120, clientY: 60 });
    fireEvent.pointerMove(svg, { clientX: 400, clientY: 200 });
    fireEvent.pointerUp(svg);
    expect(onAnadir).not.toHaveBeenCalled();
    expect(svg.getAttribute("class")).toContain("pointer-events-none");
  });
});

describe("el texto", () => {
  /** Lo que hace la persona: pulsar en la imagen y escribir en el campo que sale ahí. */
  function escribir(svg: SVGSVGElement, que: string) {
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    const campo = screen.getByRole("textbox", { name: "Texto" });
    fireEvent.change(campo, { target: { value: que } });
    return campo as HTMLInputElement;
  }

  it("pulsar en la imagen abre un campo ahí mismo, sin poner nada todavía", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text" });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    const campo = screen.getByRole("textbox", { name: "Texto" });
    // En el centro de la capa: de 0 a 1, como las marcas.
    expect(campo.style.left).toBe("50%");
    expect(campo.style.top).toBe("50%");
    expect(onAnadir).not.toHaveBeenCalled();
  });

  it("Intro lo deja puesto donde se pulsó, con lo que se escribió", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text", color: "#0a9bff" });
    const campo = escribir(svg, "mira esto");
    fireEvent.keyDown(campo, { key: "Enter" });
    expect(onAnadir).toHaveBeenCalledTimes(1);
    expect(onAnadir.mock.calls[0][0]).toEqual({
      kind: "text",
      x1: 0.5,
      y1: 0.5,
      x2: 0.5,
      y2: 0.5,
      color: "#0a9bff",
      text: "mira esto",
    });
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("pulsar fuera, en la imagen, lo guarda una sola vez y no abre otro", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text" });
    const campo = escribir(svg, "hola");
    // El mismo clic llega dos veces: a la imagen, y al campo que pierde el foco.
    fireEvent.pointerDown(svg, { clientX: 150, clientY: 60 });
    fireEvent.blur(campo);
    expect(onAnadir).toHaveBeenCalledTimes(1);
    expect(onAnadir.mock.calls[0][0]).toMatchObject({ text: "hola", x1: 0.5 });
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("pulsar fuera de la imagen (la barra, el panel) también lo guarda", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text" });
    fireEvent.blur(escribir(svg, "fuera"));
    expect(onAnadir).toHaveBeenCalledTimes(1);
    expect(onAnadir.mock.calls[0][0]).toMatchObject({ text: "fuera" });
  });

  it("Escape lo tira, y no le llega al editor", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text" });
    const alEditor = vi.fn();
    window.addEventListener("keydown", alEditor);
    const campo = escribir(svg, "no va");
    fireEvent.keyDown(campo, { key: "Escape" });
    fireEvent.blur(campo);
    window.removeEventListener("keydown", alEditor);
    expect(onAnadir).not.toHaveBeenCalled();
    expect(alEditor).not.toHaveBeenCalled();
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("sin nada escrito no se pone nada", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text" });
    fireEvent.keyDown(escribir(svg, "   "), { key: "Enter" });
    expect(onAnadir).not.toHaveBeenCalled();
  });

  it("cambiar de herramienta con el texto a medias lo guarda", () => {
    const onAnadir = vi.fn();
    const props = { color: "#ef4444", anotaciones: [], onAnadir, anchoImagen: 1920 };
    const { container, rerender } = render(<CapaAnotaciones herramienta="text" {...props} />);
    escribir(container.querySelector("svg")!, "a medias");
    rerender(<CapaAnotaciones herramienta="box" {...props} />);
    expect(onAnadir).toHaveBeenCalledTimes(1);
    expect(onAnadir.mock.calls[0][0]).toMatchObject({ text: "a medias" });
  });

  it("el campo se lleva el foco en el acto, para que no se pierda lo primero que se teclea", () => {
    const { svg } = pintar({ herramienta: "text" });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    expect(document.activeElement).toBe(screen.getByRole("textbox", { name: "Texto" }));
  });

  it("la letra es la del archivo: un cuarentaidosavo del ancho, entre 14 y 72 píxeles", () => {
    // 1920 / 42 = 45,7 px de celda: manda el ancho. Y 100 / 42 cqw es eso mismo en la capa.
    const ancha = letraDelTexto(1920, 0.2, 0.3, "#22c55e");
    expect(ancha.fontSize).toBe(`calc(clamp(${14 * (100 / 1920)}cqw, ${100 / 42}cqw, ${72 * (100 / 1920)}cqw) / 1.33)`);
    expect(ancha).toMatchObject({ left: "20%", top: "30%", color: "#22c55e", fontWeight: 600 });
    // Una captura de 300 px de ancho daría 7 px de celda: se queda en 14, que son 14 / 300.
    expect(letraDelTexto(300, 0, 0, "#000000").fontSize).toContain(`clamp(${14 * (100 / 300)}cqw,`);
  });

  it("los textos ya puestos se pintan en su sitio y con su color", () => {
    const marcas: Anotacion[] = [{ kind: "text", x1: 0.2, y1: 0.3, x2: 0.2, y2: 0.3, color: "#22c55e", text: "listo" }];
    pintar({ anotaciones: marcas, herramienta: null });
    const texto = screen.getByText("listo");
    expect(texto.style.left).toBe("20%");
    expect(texto.style.top).toBe("30%");
  });
});

describe("lo que ya está dibujado", () => {
  const marcas: Anotacion[] = [
    { kind: "box", x1: 0.1, y1: 0.1, x2: 0.5, y2: 0.5, color: "#ef4444", text: "" },
    { kind: "text", x1: 0.6, y1: 0.2, x2: 0.6, y2: 0.2, color: "#0a9bff", text: "aquí" },
  ];

  it("se pinta todo lo que hay, en su orden", () => {
    const { svg } = pintar({ anotaciones: marcas, herramienta: null });
    // Sin contar la silueta invisible que lleva cada una para cogerla.
    expect([...svg.querySelectorAll("rect")].filter((r) => !r.closest("[data-silueta]"))).toHaveLength(1);
    expect(screen.getByText("aquí")).toBeInTheDocument();
  });

  it("el rectángulo sale hueco, para no tapar lo que señala", () => {
    const { svg } = pintar({ anotaciones: [marcas[0]], herramienta: null });
    expect(svg.querySelector("rect")?.getAttribute("fill")).toBe("none");
  });

  it("y se coloca donde dicen sus coordenadas, escaladas al lienzo del SVG", () => {
    const { svg } = pintar({ anotaciones: [marcas[0]], herramienta: null });
    const rect = svg.querySelector("rect")!;
    // El SVG trabaja en un lienzo de 1000 x 1000, así que 0,1 son 100.
    expect(rect.getAttribute("x")).toBe("100");
    expect(rect.getAttribute("width")).toBe("400");
  });
});

describe("los pasos numerados", () => {
  it("se ponen con un clic, sin arrastrar", () => {
    conTamanno();
    const { onAnadir, svg } = pintar({ herramienta: "step" });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    expect(onAnadir).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "step", x1: 0.5, y1: 0.5, text: "1" }),
    );
  });

  /**
   * Numerar a mano es justo lo que se olvida cuando alguien mete un paso en medio, y
   * entonces hay dos treses en la misma imagen. La cuenta la lleva la capa.
   */
  it("el siguiente sigue la cuenta de los que ya hay", () => {
    conTamanno();
    const puestos: Anotacion[] = [
      { kind: "step", x1: 0.1, y1: 0.1, x2: 0.1, y2: 0.1, color: "#ef4444", text: "1" },
      { kind: "box", x1: 0, y1: 0, x2: 0.5, y2: 0.5, color: "#ef4444", text: "" },
      { kind: "step", x1: 0.2, y1: 0.2, x2: 0.2, y2: 0.2, color: "#ef4444", text: "2" },
    ];
    const { onAnadir, svg } = pintar({ herramienta: "step", anotaciones: puestos });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    // Tres marcas puestas, pero solo dos son pasos: el que entra es el 3.
    expect(onAnadir).toHaveBeenCalledWith(expect.objectContaining({ text: "3" }));
  });

  /**
   * Uno puesto contra el borde cabe entero.
   *
   * Si no, se ve un cuarto de disco y el número se va fuera de la captura, que es lo que
   * pedía arreglar la issue 1. Al exportar se hace la misma cuenta (`centro_metido`), así
   * que lo que se ve en la vista previa es lo que sale en el archivo.
   */
  it("uno contra el borde se mete para que quepa entero", () => {
    // En una de 1920 el disco llega a su tope de 60 px y el aro va 3 px por fuera: 63 px, que
    // son 3,28 cqw.
    const aro = (medidaDelPaso(1920).aro * 100) / 1920;
    const enLaEsquina = estiloDelPaso(1920, 0, 0);
    expect(enLaEsquina.left).toBe(`clamp(${aro}cqw, 0cqw, ${100 - aro}cqw)`);
    expect(enLaEsquina.top).toBe(`clamp(${aro}cqw, 0cqh, calc(100cqh - ${aro}cqw))`);
  });

  it("y uno que ya cabía no se mueve de donde se puso", () => {
    const aro = (medidaDelPaso(1920).aro * 100) / 1920;
    const enMedio = estiloDelPaso(1920, 0.5, 0.25);
    expect(enMedio.left).toBe(`clamp(${aro}cqw, 50cqw, ${100 - aro}cqw)`);
    expect(enMedio.top).toBe(`clamp(${aro}cqw, 25cqh, calc(100cqh - ${aro}cqw))`);
  });

  it("se dibuja con su número dentro para verlo antes de exportar", () => {
    const puesto: Anotacion[] = [
      { kind: "step", x1: 0.5, y1: 0.5, x2: 0.5, y2: 0.5, color: "#0a9bff", text: "4" },
    ];
    pintar({ herramienta: null, anotaciones: puesto });
    expect(screen.getByText("4")).toBeInTheDocument();
  });

  /**
   * La issue 19: en una captura apaisada el paso salía ovalado, con el número estirado. El
   * SVG va estirado a 1000 x 1000 (`preserveAspectRatio="none"`), y un círculo dibujado ahí
   * dentro se estira con él. Al exportar salía redondo; en la vista previa, no.
   */
  it("sale redondo aunque la captura sea apaisada (issue 19)", () => {
    const puesto: Anotacion[] = [
      { kind: "step", x1: 0.5, y1: 0.5, x2: 0.5, y2: 0.5, color: "#ef4444", text: "1" },
    ];
    const { container } = pintar({ herramienta: null, anotaciones: puesto });
    // Nada del paso queda en el SVG estirado.
    expect(container.querySelector("svg ellipse")).toBeNull();
    expect(container.querySelector('[data-marca="step"]')).not.toBeNull();
    // Y su caja es igual de ancha que de alta, en la misma unidad: la del ancho de la capa.
    const estilo = estiloDelPaso(1920, 0.5, 0.5);
    expect(estilo.width).toBe(estilo.height);
    expect(String(estilo.width)).toMatch(/cqw$/);
  });

  /** Lo mismo que pinta Rust (`encode::anotacion::paso`), para que lo que se ve sea lo que sale. */
  it("mide lo que va a medir en el archivo", () => {
    // Una de 1300: 50 px de disco, sin llegar a ningún tope.
    const { radio, aro, letra } = medidaDelPaso(1300);
    expect(radio).toBeCloseTo(1300 / 26);
    // El grosor del trazo ahí son 3 px, y el aro blanco va un 60 % de eso por fuera.
    expect(aro).toBeCloseTo(1300 / 26 + 1.8);
    // Con la cuenta en f32, como Rust: 50 x 1,15 da justo 57,5 y sube a 58 (en doble, 57,49...).
    expect(letra).toBe(58);
    // En una de 1920 el disco ya está en su tope, y el trazo es de 5 px.
    expect(medidaDelPaso(1920).aro).toBeCloseTo(63);
    // Entre 15 y 60 px de radio, como al exportar.
    expect(medidaDelPaso(200).radio).toBe(15);
    expect(medidaDelPaso(4000).radio).toBe(60);
  });
});

/**
 * Las issues 18 y 19: el texto y los pasos salían siempre a un tamaño que nadie podía
 * cambiar. Ahora cada uno lleva el suyo (`escala`, de 1 de serie), que multiplica el tamaño
 * de siempre: así sigue creciendo con la imagen y exportar al 50 % no lo deja enorme.
 */
describe("el tamaño del texto y de los pasos", () => {
  const paso: Anotacion = { kind: "step", x1: 0.5, y1: 0.5, x2: 0.5, y2: 0.5, color: "#ef4444", text: "1" };

  it("un paso al doble mide el doble, aro y número incluidos", () => {
    const uno = medidaDelPaso(1920);
    const doble = medidaDelPaso(1920, 2);
    expect(doble.radio).toBeCloseTo(uno.radio * 2);
    expect(doble.aro).toBeCloseTo(uno.aro * 2);
    expect(doble.letra).toBe(Math.round(uno.radio * 2 * 1.15));
  });

  it("un texto al doble lleva la letra al doble", () => {
    expect(letraDelTexto(1920, 0.2, 0.3, "#22c55e", 2).fontSize).toBe(
      `calc(clamp(${14 * (100 / 1920)}cqw, ${100 / 42}cqw, ${72 * (100 / 1920)}cqw) * 2 / 1.33)`,
    );
  });

  it("lo nuevo sale al tamaño elegido, y al de siempre sin guardar nada de más", () => {
    const { onAnadir, svg } = pintar({ herramienta: "step", escala: 1.5 });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    expect(onAnadir).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "step", escala: 1.5 }));

    const deSiempre = pintar({ herramienta: "step" });
    fireEvent.pointerDown(deSiempre.svg, { clientX: 300, clientY: 150 });
    expect(deSiempre.onAnadir.mock.calls[0][0]).not.toHaveProperty("escala");
  });

  it("un texto nuevo sale al tamaño elegido", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text", escala: 2 });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    const campo = screen.getByRole("textbox");
    fireEvent.change(campo, { target: { value: "grande" } });
    fireEvent.keyDown(campo, { key: "Enter" });
    expect(onAnadir).toHaveBeenCalledWith(expect.objectContaining({ kind: "text", text: "grande", escala: 2 }));
  });

  it("solo lo elegido lleva el asa para estirarlo", () => {
    const sinElegir = pintar({ herramienta: null, anotaciones: [paso] });
    expect(sinElegir.container.querySelector("[data-estirar]")).toBeNull();
    const elegido = pintar({ herramienta: null, anotaciones: [paso], elegida: 0 });
    expect(elegido.container.querySelector('[data-agarre="0"] [data-estirar]')).not.toBeNull();
  });

  /**
   * Con el asa se estira desde el centro del paso: lo que se aleja el ratón del centro es lo
   * que crece. Todo mide 400 x 200 desde (100, 50), así que el centro cae en (300, 150).
   */
  it("arrastrar el asa lo estira, y el cambio sale una vez, al soltar", () => {
    const onCambiar = vi.fn();
    const { container } = pintar({ herramienta: null, anotaciones: [paso], elegida: 0, onCambiar });
    const asa = container.querySelector("[data-estirar]")!;
    fireEvent.pointerDown(asa, { button: 0, clientX: 400, clientY: 150 });
    fireEvent.pointerMove(window, { clientX: 450, clientY: 150 });
    fireEvent.pointerMove(window, { clientX: 500, clientY: 150 });
    expect(onCambiar).not.toHaveBeenCalled();
    fireEvent.pointerUp(window, { clientX: 500, clientY: 150 });
    expect(onCambiar).toHaveBeenCalledTimes(1);
    expect(onCambiar).toHaveBeenCalledWith(0, expect.objectContaining({ kind: "step", escala: 2 }));
  });

  it("estirar se para en sus topes", () => {
    const onCambiar = vi.fn();
    const { container } = pintar({ herramienta: null, anotaciones: [paso], elegida: 0, onCambiar });
    const asa = container.querySelector("[data-estirar]")!;
    fireEvent.pointerDown(asa, { button: 0, clientX: 301, clientY: 150 });
    fireEvent.pointerMove(window, { clientX: 1000, clientY: 150 });
    fireEvent.pointerUp(window, { clientX: 1000, clientY: 150 });
    expect(onCambiar).toHaveBeenLastCalledWith(0, expect.objectContaining({ escala: ESCALA_MAXIMA }));
  });

  it("pulsar el asa no mueve la marca ni la suelta", () => {
    const onCambiar = vi.fn();
    const onElegir = vi.fn();
    const { container } = pintar({ herramienta: null, anotaciones: [paso], elegida: 0, onCambiar, onElegir });
    const asa = container.querySelector("[data-estirar]")!;
    fireEvent.pointerDown(asa, { button: 0, clientX: 400, clientY: 150 });
    fireEvent.pointerUp(window, { clientX: 400, clientY: 150 });
    expect(onCambiar).not.toHaveBeenCalled();
    expect(onElegir).not.toHaveBeenCalledWith(null);
  });
});

/**
 * Las marcas nuevas (flecha curva, línea, elipse, lápiz y foco).
 *
 * Lo que importa de ellas es que la vista previa enseñe lo que va a salir en el archivo,
 * y aquí no hay píxeles que comparar: la vista previa es SVG. Así que las dos pruebas, esta
 * y `mismas_marcas_que_la_vista_previa` en Rust, leen las mismas marcas con los puntos por
 * donde tienen que pasar y los que tienen que quedar limpios. Esta mide la geometría del
 * SVG; la de Rust, los píxeles.
 */
import prueba from "../../lib/marcas-de-prueba.json";
import { FOCO_OSCURECE, puntoDeCurva, type Punto } from "../../lib/anotaciones";

/** Muchos puntos de lo que dibuja una marca, en el lienzo de 1000 x 1000 del SVG. */
function recorrido(svg: SVGSVGElement, kind: string): Punto[] {
  const num = (el: Element, a: string) => Number(el.getAttribute(a));
  const muestras = (n: number, f: (t: number) => Punto) =>
    Array.from({ length: n + 1 }, (_, i) => f(i / n));
  const tramos = (puntos: Punto[]) =>
    puntos.slice(1).flatMap((p, i) =>
      muestras(200, (t) => [
        puntos[i][0] + (p[0] - puntos[i][0]) * t,
        puntos[i][1] + (p[1] - puntos[i][1]) * t,
      ]),
    );
  const el = svg.querySelector(`[data-marca="${kind}"]`)!;
  expect(el, `no se ha pintado ninguna «${kind}»`).not.toBeNull();
  switch (kind) {
    case "curve": {
      const [x0, y0, cx, cy, x2, y2] = el.querySelector("path")!.getAttribute("d")!.match(/-?[\d.]+/g)!.map(Number);
      return muestras(2000, (t) => puntoDeCurva([x0, y0], [cx, cy], [x2, y2], t));
    }
    case "line":
      return tramos([
        [num(el, "x1"), num(el, "y1")],
        [num(el, "x2"), num(el, "y2")],
      ]);
    case "ellipse":
      return muestras(2000, (t) => [
        num(el, "cx") + num(el, "rx") * Math.cos(t * Math.PI * 2),
        num(el, "cy") + num(el, "ry") * Math.sin(t * Math.PI * 2),
      ]);
    case "pen":
      return tramos(
        el
          .getAttribute("points")!
          .split(" ")
          .map((p) => p.split(",").map(Number) as Punto),
      );
    default:
      throw new Error(kind);
  }
}

/** Lo que dista un punto (de 0 a 1) de lo dibujado, en unidades del lienzo de 1000. */
function distancia(camino: Punto[], p: number[]): number {
  return Math.min(...camino.map(([x, y]) => Math.hypot(x - p[0] * 1000, y - p[1] * 1000)));
}

describe("las marcas nuevas", () => {
  it("dibujan lo mismo que Rust: pasan por donde tienen que pasar", () => {
    expect(prueba.focoOscurece).toBe(FOCO_OSCURECE);
    for (const caso of prueba.casos) {
      const { svg } = pintar({ herramienta: null, anotaciones: [caso.marca as Anotacion] });
      if (caso.marca.kind === "focus") {
        const velo = svg.querySelector("[data-velo]")!;
        expect(Number(velo.getAttribute("opacity"))).toBe(prueba.focoOscurece);
        const zonas = [...svg.querySelectorAll("[data-zona-clara]")].map((z) =>
          ["x", "y", "width", "height"].map((a) => Number(z.getAttribute(a))),
        );
        const clara = ([x, y]: number[]) =>
          zonas.some(([zx, zy, w, h]) => x * 1000 >= zx && x * 1000 <= zx + w && y * 1000 >= zy && y * 1000 <= zy + h);
        for (const p of caso.claro ?? []) expect(clara(p), `${p} tenía que quedar claro`).toBe(true);
        for (const p of caso.oscuro ?? []) expect(clara(p), `${p} tenía que quedar oscuro`).toBe(false);
      } else {
        const camino = recorrido(svg, caso.marca.kind);
        for (const p of caso.pasaPor ?? []) {
          expect(distancia(camino, p), `«${caso.nombre}» tenía que pasar por ${p}`).toBeLessThan(4);
        }
        for (const p of caso.limpio ?? []) {
          expect(distancia(camino, p), `«${caso.nombre}» pasa por ${p}`).toBeGreaterThan(20);
        }
      }
      document.body.innerHTML = "";
    }
  });

  it("el foco va debajo de todo, como al exportar", () => {
    const { svg } = pintar({
      herramienta: null,
      anotaciones: [
        { kind: "box", x1: 0.1, y1: 0.1, x2: 0.3, y2: 0.3, color: "#ef4444", text: "" },
        { kind: "focus", x1: 0.5, y1: 0.5, x2: 0.9, y2: 0.9, color: "#ef4444", text: "" },
      ],
    });
    expect(svg.firstElementChild?.getAttribute("data-marca")).toBe("focus");
  });

  it("dos focos son un solo velo con dos huecos", () => {
    const { svg } = pintar({
      herramienta: null,
      anotaciones: [
        { kind: "focus", x1: 0.1, y1: 0.1, x2: 0.3, y2: 0.3, color: "", text: "" },
        { kind: "focus", x1: 0.6, y1: 0.6, x2: 0.9, y2: 0.9, color: "", text: "" },
      ],
    });
    expect(svg.querySelectorAll("[data-velo]")).toHaveLength(1);
    expect(svg.querySelectorAll("[data-zona-clara]")).toHaveLength(2);
  });

  it("el lápiz guarda su trazo, aclarado y de 0 a 1", () => {
    const { onAnadir, svg } = pintar({ herramienta: "pen" });
    // Una línea recta con muchos puntos: aclarada se queda en sus dos extremos.
    fireEvent.pointerDown(svg, { clientX: 100, clientY: 150 });
    for (let x = 110; x <= 400; x += 10) fireEvent.pointerMove(svg, { clientX: x, clientY: 150 });
    fireEvent.pointerUp(svg);
    const marca: Anotacion = onAnadir.mock.calls[0][0];
    expect(marca.kind).toBe("pen");
    expect(marca.puntos).toHaveLength(2);
    expect(marca.puntos![0][0]).toBeCloseTo(0, 5);
    expect(marca.puntos![1][0]).toBeCloseTo(0.75, 5);
    expect(marca.puntos![1][1]).toBeCloseTo(0.5, 5);
  });

  it("un círculo a mano que acaba donde empezó también es una marca", () => {
    const { onAnadir, svg } = pintar({ herramienta: "pen" });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    for (let i = 1; i <= 24; i++) {
      const a = (i / 24) * Math.PI * 2;
      fireEvent.pointerMove(svg, { clientX: 300 + Math.sin(a) * 40, clientY: 150 - (1 - Math.cos(a)) * 40 });
    }
    fireEvent.pointerUp(svg);
    expect(onAnadir).toHaveBeenCalledTimes(1);
    expect(onAnadir.mock.calls[0][0].puntos.length).toBeGreaterThan(8);
  });

  it("la flecha curva sale de dibujar la curva, sin colocar puntos de control", () => {
    const { onAnadir, svg } = pintar({ herramienta: "curve" });
    // Un arco hacia arriba de la izquierda a la derecha.
    fireEvent.pointerDown(svg, { clientX: 140, clientY: 200 });
    for (let i = 1; i <= 20; i++) {
      const t = i / 20;
      fireEvent.pointerMove(svg, { clientX: 140 + t * 320, clientY: 200 - Math.sin(t * Math.PI) * 100 });
    }
    fireEvent.pointerUp(svg);
    const marca: Anotacion = onAnadir.mock.calls[0][0];
    expect(marca.kind).toBe("curve");
    // La curva pasa a la mitad por donde pasó el ratón a la mitad: (300, 100) en la capa,
    // que es (0,5, 0,25).
    const medio = puntoDeCurva([marca.x1, marca.y1], marca.puntos![0], [marca.x2, marca.y2], 0.5);
    expect(medio[0]).toBeCloseTo(0.5, 1);
    expect(medio[1]).toBeCloseTo(0.25, 1);
  });

  it("una flecha curva dibujada recta sale recta", () => {
    const { onAnadir, svg } = pintar({ herramienta: "curve" });
    fireEvent.pointerDown(svg, { clientX: 140, clientY: 150 });
    for (let x = 150; x <= 460; x += 10) fireEvent.pointerMove(svg, { clientX: x, clientY: 150 + (x % 20 === 0 ? 1 : 0) });
    fireEvent.pointerUp(svg);
    const marca: Anotacion = onAnadir.mock.calls[0][0];
    // El control en medio de los extremos: una cuadrática así es una recta.
    expect(marca.puntos![0][0]).toBeCloseTo((marca.x1 + marca.x2) / 2, 5);
    expect(marca.puntos![0][1]).toBeCloseTo((marca.y1 + marca.y2) / 2, 5);
  });
});

/**
 * Lo que mide un paso aquí y al exportar, con los mismos casos que lee Rust
 * (`el_paso_mide_lo_mismo_que_en_la_vista_previa`): si un lado cambia sus cuentas y el otro
 * no, se pone roja una de las dos.
 */
describe("el paso y sus topes, contra la prueba compartida", () => {
  it("mide lo mismo que en el archivo y tiene los mismos topes", () => {
    expect([prueba.escalaMinima, prueba.escalaMaxima]).toEqual([ESCALA_MINIMA, ESCALA_MAXIMA]);
    expect(prueba.pasos.length).toBeGreaterThan(0);
    for (const p of prueba.pasos) {
      const m = medidaDelPaso(p.ancho, p.escala);
      expect(m.radio, `radio en ${p.ancho} al ${p.escala}`).toBeCloseTo(p.radio, 2);
      expect(m.aro, `aro en ${p.ancho} al ${p.escala}`).toBeCloseTo(p.aro, 2);
      expect(m.letra, `letra en ${p.ancho} al ${p.escala}`).toBe(p.letra);
    }
  });
});

/**
 * Lo que avisó ProNotion en la #19: otras marcas tenían el mismo fallo que el paso. La punta de
 * la flecha se calculaba en el SVG estirado a 1000 x 1000, y en una captura apaisada salía más
 * cerrada en horizontal y más abierta en vertical que en el archivo, donde Rust la pinta a
 * 0,52 rad de la línea (`encode::anotacion::punta`). Ahora abre eso mismo y sus dos alas miden
 * igual. Lo que mide no es lo del archivo, y a propósito: la vista previa dibuja todos los
 * trazos más gruesos (6 px de pantalla), y su punta va a juego, un 4,8 % del ancho.
 */
describe("la punta de la flecha en una captura apaisada", () => {
  const [ancho, alto] = [1920, 1080];
  const enPx = ([x, y]: number[]) => [(x * ancho) / 1000, (y * alto) / 1000];
  const num = (l: Element, a: string) => Number(l.getAttribute(a));

  /** Las alas de la punta, en píxeles de la imagen: el ángulo con la línea y lo que miden. */
  function alas(marca: Anotacion, selector: string) {
    const { container } = pintar({ herramienta: null, anotaciones: [marca], anchoImagen: ancho, altoImagen: alto });
    const lineas = [...container.querySelectorAll(selector)];
    // La última línea de la marca acaba en la punta, y las dos alas salen de ella.
    const alasSvg = lineas.slice(-2);
    const [px2, py2] = enPx([num(alasSvg[0], "x1"), num(alasSvg[0], "y1")]);
    return { alas: alasSvg.map((l) => enPx([num(l, "x2"), num(l, "y2")]).map((v, k) => v - [px2, py2][k])), punta: [px2, py2] };
  }

  function comprueba(desde: number[], { alas: [a, b], punta }: ReturnType<typeof alas>) {
    // De la punta hacia atrás, por donde llega la línea.
    const atras = Math.atan2(desde[1] - punta[1], desde[0] - punta[0]);
    for (const ala of [a, b]) {
      expect(Math.hypot(ala[0], ala[1])).toBeCloseTo(0.048 * ancho, 1);
      const abre = Math.abs(Math.atan2(Math.sin(Math.atan2(ala[1], ala[0]) - atras), Math.cos(Math.atan2(ala[1], ala[0]) - atras)));
      expect(abre).toBeCloseTo(0.52, 2);
    }
  }

  it("la recta, en horizontal y en vertical, abre lo mismo que en el archivo", () => {
    const horizontal: Anotacion = { kind: "arrow", x1: 0.1, y1: 0.5, x2: 0.9, y2: 0.5, color: "#ef4444", text: "" };
    comprueba([0.1 * ancho, 0.5 * alto], alas(horizontal, '[data-agarre="0"] > g:not([data-silueta]) > line'));
    const vertical: Anotacion = { kind: "arrow", x1: 0.5, y1: 0.1, x2: 0.5, y2: 0.9, color: "#ef4444", text: "" };
    comprueba([0.5 * ancho, 0.1 * alto], alas(vertical, '[data-agarre="0"] > g:not([data-silueta]) > line'));
    const diagonal: Anotacion = { kind: "arrow", x1: 0.2, y1: 0.2, x2: 0.7, y2: 0.8, color: "#ef4444", text: "" };
    comprueba([0.2 * ancho, 0.2 * alto], alas(diagonal, '[data-agarre="0"] > g:not([data-silueta]) > line'));
  });

  it("y la curva, mirando desde su punto de control", () => {
    const curva: Anotacion = {
      kind: "curve", x1: 0.1, y1: 0.8, x2: 0.9, y2: 0.8, color: "#ef4444", text: "", puntos: [[0.5, 0.2]],
    };
    comprueba([0.5 * ancho, 0.2 * alto], alas(curva, '[data-marca="curve"] > line'));
  });
});
