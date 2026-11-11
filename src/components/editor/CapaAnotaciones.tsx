import { useId, useRef, useState } from "react";
import {
  controlDeCurva,
  FOCO_OSCURECE,
  simplificarTrazo,
  type Anotacion,
  type Herramienta,
  type Punto,
} from "../../lib/anotaciones";

/**
 * La capa donde se dibuja encima de la vista previa.
 *
 * Se pinta con SVG y no con un `<canvas>` por una razón concreta: aquí solo hay que
 * ENSEÑAR las marcas mientras se colocan, y quien las dibuja de verdad sobre los píxeles
 * es Rust al exportar. Con SVG cada marca es un elemento que React sabe pintar, mover y
 * borrar solo; con un canvas habría que repintarlo entero a mano en cada movimiento del
 * ratón y llevar la cuenta de lo que hay debajo.
 *
 * **Las coordenadas se guardan de 0 a 1.** La vista previa casi nunca mide lo que va a
 * medir el archivo, así que guardarlas en píxeles obligaría a recalcularlas cada vez que
 * alguien toca «Dimensiones», y una sola que se olvidara dejaría la flecha señalando a
 * otro sitio.
 */
interface Props {
  herramienta: Herramienta | null;
  color: string;
  anotaciones: Anotacion[];
  onAnadir: (anotacion: Anotacion) => void;
  /** Lo que se escribe cuando la herramienta es el texto. */
  texto: string;
}

export function CapaAnotaciones({ herramienta, color, anotaciones, onAnadir, texto }: Props) {
  const marco = useRef<SVGSVGElement>(null);
  const [dibujando, setDibujando] = useState<Anotacion | null>(null);
  /**
   * Por dónde ha pasado el ratón, para el lápiz y la flecha curva. Va en una ref y no en
   * el estado porque cambia en cada movimiento y no se pinta tal cual: el lápiz pinta su
   * versión aclarada y la curva, la curva que sale de él.
   */
  const camino = useRef<Punto[]>([]);
  // Los `:` de useId no valen dentro de un `url(#...)` en todos los motores.
  const idDelVelo = `velo-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;

  /** El punto del ratón, en tanto por uno sobre la capa. */
  const punto = (e: React.PointerEvent): [number, number] => {
    const caja = marco.current?.getBoundingClientRect();
    if (!caja || caja.width === 0 || caja.height === 0) return [0, 0];
    return [
      Math.min(1, Math.max(0, (e.clientX - caja.left) / caja.width)),
      Math.min(1, Math.max(0, (e.clientY - caja.top) / caja.height)),
    ];
  };

  const empezar = (e: React.PointerEvent) => {
    if (!herramienta) return;
    e.preventDefault();
    const [x, y] = punto(e);
    // El texto no se arrastra: se pone donde se pulsa y se acabó.
    if (herramienta === "text") {
      if (texto.trim()) onAnadir({ kind: "text", x1: x, y1: y, x2: x, y2: y, color, text: texto });
      return;
    }
    // Y el paso tampoco, y además se numera solo. Numerar a mano es justo lo que se olvida
    // cuando alguien mete un paso en medio, y entonces hay dos treses en la misma imagen.
    if (herramienta === "step") {
      const siguiente = anotaciones.filter((a) => a.kind === "step").length + 1;
      onAnadir({ kind: "step", x1: x, y1: y, x2: x, y2: y, color, text: String(siguiente) });
      return;
    }
    camino.current = [[x, y]];
    setDibujando({ kind: herramienta, x1: x, y1: y, x2: x, y2: y, color, text: "" });
  };

  const mover = (e: React.PointerEvent) => {
    if (!dibujando) return;
    const [x, y] = punto(e);
    if (dibujando.kind === "pen" || dibujando.kind === "curve") {
      const ultimo = camino.current[camino.current.length - 1];
      // Un punto cada poco: el ratón manda muchos más de los que cambian el dibujo.
      if (!ultimo || Math.hypot(x - ultimo[0], y - ultimo[1]) > 0.002) {
        camino.current = [...camino.current, [x, y]];
      }
    }
    setDibujando(conCamino({ ...dibujando, x2: x, y2: y }, camino.current));
  };

  const soltar = () => {
    if (!dibujando) return;
    // Un clic sin arrastre no es una marca, es un clic: sin esto, cada vez que alguien
    // pulsa sin querer queda un rectángulo de cero píxeles imposible de ver y de borrar.
    // En el lápiz cuenta todo lo que se ha movido, no solo dónde acaba: un círculo a mano
    // termina donde empezó.
    const puntos = dibujando.kind === "pen" ? camino.current : [[dibujando.x1, dibujando.y1], [dibujando.x2, dibujando.y2]];
    const xs = puntos.map((p) => p[0]);
    const ys = puntos.map((p) => p[1]);
    const ancho = Math.max(...xs) - Math.min(...xs);
    const alto = Math.max(...ys) - Math.min(...ys);
    if (ancho > 0.01 || alto > 0.01) onAnadir(conCamino(dibujando, camino.current, true));
    camino.current = [];
    setDibujando(null);
  };

  const todas = dibujando ? [...anotaciones, dibujando] : anotaciones;
  // El foco va debajo de todo y todos juntos en un velo, igual que al exportar
  // (`encode::anotacion::pintar`): así no oscurece lo dibujado ni se tapan entre ellos.
  const focos = todas.filter((a) => a.kind === "focus");

  return (
    <svg
      ref={marco}
      onPointerDown={empezar}
      onPointerMove={mover}
      onPointerUp={soltar}
      onPointerLeave={soltar}
      viewBox="0 0 1000 1000"
      preserveAspectRatio="none"
      className={`absolute inset-0 size-full ${
        herramienta ? "cursor-crosshair" : "pointer-events-none"
      }`}
    >
      {focos.length > 0 && (
        <g data-marca="focus">
          <mask id={idDelVelo}>
            <rect x={0} y={0} width={1000} height={1000} fill="#ffffff" />
            {focos.map((f, i) => (
              <rect
                key={i}
                data-zona-clara=""
                x={Math.min(f.x1, f.x2) * 1000}
                y={Math.min(f.y1, f.y2) * 1000}
                width={Math.abs(f.x2 - f.x1) * 1000}
                height={Math.abs(f.y2 - f.y1) * 1000}
                fill="#000000"
              />
            ))}
          </mask>
          <rect
            data-velo=""
            x={0}
            y={0}
            width={1000}
            height={1000}
            fill="#000000"
            opacity={FOCO_OSCURECE}
            mask={`url(#${idDelVelo})`}
          />
        </g>
      )}
      {todas.map((a, i) => (
        <Marca key={i} anotacion={a} />
      ))}
    </svg>
  );
}

/**
 * Lo que guarda el lápiz y la flecha curva además de las dos esquinas.
 *
 * El lápiz se queda con su trazo (aclarado al soltar, que es cuando ya no cambia), y la
 * curva con el punto de control que sale del camino: quien dibuja una curva no quiere
 * colocar puntos de control, quiere dibujar la curva.
 */
function conCamino(marca: Anotacion, camino: Punto[], final = false): Anotacion {
  if (marca.kind === "pen") {
    return { ...marca, puntos: final ? simplificarTrazo(camino) : camino };
  }
  if (marca.kind === "curve") {
    const extremos: Punto[] = [[marca.x1, marca.y1], ...camino.slice(1, -1), [marca.x2, marca.y2]];
    return { ...marca, puntos: [controlDeCurva(extremos)] };
  }
  return marca;
}

/** Lo que mide el aro de un paso, en el sistema de 1000 del SVG. */
const RADIO_PASO = 38;

/**
 * El centro de una marca de un clic, metido lo justo para que la marca quepa entera.
 *
 * Al exportar se hace lo mismo (`encode::anotacion::centro_metido`), y por la misma razón:
 * un disco partido por el borde no se lee, porque el número se va fuera y lo que queda es
 * una mancha. Aquí hace falta además para que la vista previa enseñe lo que va a salir.
 */
function metido(v: number, radio: number): number {
  return Math.min(1000 - radio, Math.max(radio, v));
}

/** Una marca dibujada, en el sistema de 1000 x 1000 del SVG. */
function Marca({ anotacion: a }: { anotacion: Anotacion }) {
  const [x1, y1, x2, y2] = [a.x1 * 1000, a.y1 * 1000, a.x2 * 1000, a.y2 * 1000];
  const [izq, arr] = [Math.min(x1, x2), Math.min(y1, y2)];
  const [ancho, alto] = [Math.abs(x2 - x1), Math.abs(y2 - y1)];

  switch (a.kind) {
    case "box":
      return (
        <rect
          x={izq}
          y={arr}
          width={ancho}
          height={alto}
          fill="none"
          stroke={a.color}
          strokeWidth={6}
          vectorEffect="non-scaling-stroke"
        />
      );
    case "highlight":
      return <rect x={izq} y={arr} width={ancho} height={alto} fill={a.color} opacity={0.32} />;
    case "blur":
      // En la vista previa el difuminado se enseña como una zona tramada: el mosaico de
      // verdad lo hace Rust sobre los píxeles, y fingirlo aquí con un filtro daría una
      // idea equivocada de lo que va a salir.
      return (
        <g>
          <rect x={izq} y={arr} width={ancho} height={alto} fill="#0f0f12" opacity={0.82} />
          <rect
            x={izq}
            y={arr}
            width={ancho}
            height={alto}
            fill="none"
            stroke="#ffffff"
            strokeWidth={2}
            strokeDasharray="8 6"
            vectorEffect="non-scaling-stroke"
          />
        </g>
      );
    case "arrow":
      return (
        <g stroke={a.color} strokeWidth={6} fill="none" vectorEffect="non-scaling-stroke">
          <line x1={x1} y1={y1} x2={x2} y2={y2} vectorEffect="non-scaling-stroke" />
          {puntaDeFlecha(x1, y1, x2, y2).map(([px, py], i) => (
            <line
              key={i}
              x1={x2}
              y1={y2}
              x2={px}
              y2={py}
              vectorEffect="non-scaling-stroke"
              strokeLinecap="round"
            />
          ))}
        </g>
      );
    case "curve": {
      // La misma cuadrática que pinta Rust (`punto_de_curva`): la `Q` del SVG es esa curva.
      // La punta mira desde el punto de control, que es hacia donde llega la curva.
      const [cx, cy] = a.puntos?.[0] ? [a.puntos[0][0] * 1000, a.puntos[0][1] * 1000] : [(x1 + x2) / 2, (y1 + y2) / 2];
      const [dx, dy] = Math.hypot(x2 - cx, y2 - cy) > 0.5 ? [cx, cy] : [x1, y1];
      return (
        <g data-marca="curve" stroke={a.color} strokeWidth={6} fill="none" strokeLinecap="round">
          <path d={`M ${x1} ${y1} Q ${cx} ${cy} ${x2} ${y2}`} vectorEffect="non-scaling-stroke" />
          {puntaDeFlecha(dx, dy, x2, y2).map(([px, py], i) => (
            <line key={i} x1={x2} y1={y2} x2={px} y2={py} vectorEffect="non-scaling-stroke" />
          ))}
        </g>
      );
    }
    case "line":
      return (
        <line
          data-marca="line"
          x1={x1}
          y1={y1}
          x2={x2}
          y2={y2}
          stroke={a.color}
          strokeWidth={6}
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
      );
    case "ellipse":
      return (
        <ellipse
          data-marca="ellipse"
          cx={izq + ancho / 2}
          cy={arr + alto / 2}
          rx={ancho / 2}
          ry={alto / 2}
          fill="none"
          stroke={a.color}
          strokeWidth={6}
          vectorEffect="non-scaling-stroke"
        />
      );
    case "pen":
      return (
        <polyline
          data-marca="pen"
          points={(a.puntos?.length ? a.puntos : [[a.x1, a.y1], [a.x2, a.y2]])
            .map(([px, py]) => `${px * 1000},${py * 1000}`)
            .join(" ")}
          fill="none"
          stroke={a.color}
          strokeWidth={6}
          strokeLinecap="round"
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
        />
      );
    case "focus":
      // Lo pinta el velo de arriba, todos los focos juntos y por debajo de las demás.
      return null;
    case "text":
      return (
        <text x={x1} y={y1} fill={a.color} fontSize={34} fontWeight={600} dominantBaseline="hanging">
          {a.text}
        </text>
      );
    case "step": {
      // El disco lleva un aro blanco por fuera, igual que al exportar: sobre una captura
      // oscura un disco de color sin borde se pierde, y sobre una clara se pierde el
      // número. El SVG va estirado a 1000 x 1000, así que el círculo se dibuja con dos
      // radios y sale ovalado si la captura no es cuadrada; al exportar lo pinta Rust
      // redondo, que es lo que cuenta.
      const [cx, cy] = [metido(x1, RADIO_PASO), metido(y1, RADIO_PASO)];
      return (
        <g>
          <ellipse cx={cx} cy={cy} rx={RADIO_PASO} ry={RADIO_PASO} fill="#ffffff" />
          <ellipse cx={cx} cy={cy} rx={32} ry={32} fill={a.color} />
          <text
            x={cx}
            y={cy}
            fill="#ffffff"
            fontSize={38}
            fontWeight={700}
            textAnchor="middle"
            dominantBaseline="central"
          >
            {a.text}
          </text>
        </g>
      );
    }
    default:
      return null;
  }
}

/** Los dos extremos de la punta, treinta grados a cada lado de la línea. */
function puntaDeFlecha(x1: number, y1: number, x2: number, y2: number): [number, number][] {
  const angulo = Math.atan2(y2 - y1, x2 - x1);
  const largo = 48;
  return [-0.52, 0.52].map((lado) => {
    const a = angulo + Math.PI + lado;
    return [x2 + Math.cos(a) * largo, y2 + Math.sin(a) * largo] as [number, number];
  });
}
