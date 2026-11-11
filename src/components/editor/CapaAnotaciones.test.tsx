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
import { CapaAnotaciones } from "./CapaAnotaciones";
import { aplicarIdioma } from "../../lib/i18n";
import type { Anotacion } from "../../lib/anotaciones";

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
      texto=""
      {...props}
    />,
  );
  return { onAnadir, svg: container.querySelector("svg")! };
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
  it("se pone de un clic, sin arrastrar", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text", texto: "mira esto" });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    expect(onAnadir).toHaveBeenCalledTimes(1);
    expect(onAnadir.mock.calls[0][0]).toMatchObject({ kind: "text", text: "mira esto" });
  });

  it("sin nada escrito no se pone nada", () => {
    const { onAnadir, svg } = pintar({ herramienta: "text", texto: "   " });
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 150 });
    expect(onAnadir).not.toHaveBeenCalled();
  });
});

describe("lo que ya está dibujado", () => {
  const marcas: Anotacion[] = [
    { kind: "box", x1: 0.1, y1: 0.1, x2: 0.5, y2: 0.5, color: "#ef4444", text: "" },
    { kind: "text", x1: 0.6, y1: 0.2, x2: 0.6, y2: 0.2, color: "#0a9bff", text: "aquí" },
  ];

  it("se pinta todo lo que hay, en su orden", () => {
    const { svg } = pintar({ anotaciones: marcas, herramienta: null });
    expect(svg.querySelectorAll("rect")).toHaveLength(1);
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
    conTamanno();
    const enLaEsquina: Anotacion[] = [
      { kind: "step", x1: 0, y1: 0, x2: 0, y2: 0, color: "#ef4444", text: "1" },
    ];
    const { svg } = pintar({ herramienta: null, anotaciones: enLaEsquina });
    const aro = svg.querySelector("ellipse")!;
    expect(Number(aro.getAttribute("cx"))).toBeGreaterThanOrEqual(38);
    expect(Number(aro.getAttribute("cy"))).toBeGreaterThanOrEqual(38);
  });

  it("y uno que ya cabía no se mueve de donde se puso", () => {
    conTamanno();
    const enMedio: Anotacion[] = [
      { kind: "step", x1: 0.5, y1: 0.5, x2: 0.5, y2: 0.5, color: "#ef4444", text: "1" },
    ];
    const { svg } = pintar({ herramienta: null, anotaciones: enMedio });
    const aro = svg.querySelector("ellipse")!;
    expect(aro.getAttribute("cx")).toBe("500");
    expect(aro.getAttribute("cy")).toBe("500");
  });

  it("se dibuja con su número dentro para verlo antes de exportar", () => {
    conTamanno();
    const puesto: Anotacion[] = [
      { kind: "step", x1: 0.5, y1: 0.5, x2: 0.5, y2: 0.5, color: "#0a9bff", text: "4" },
    ];
    pintar({ herramienta: null, anotaciones: puesto });
    expect(screen.getByText("4")).toBeInTheDocument();
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
