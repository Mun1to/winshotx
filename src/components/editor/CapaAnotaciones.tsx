import { useRef, useState } from "react";
import type { Anotacion, Herramienta } from "../../lib/anotaciones";

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
    setDibujando({ kind: herramienta, x1: x, y1: y, x2: x, y2: y, color, text: "" });
  };

  const mover = (e: React.PointerEvent) => {
    if (!dibujando) return;
    const [x, y] = punto(e);
    setDibujando({ ...dibujando, x2: x, y2: y });
  };

  const soltar = () => {
    if (!dibujando) return;
    // Un clic sin arrastre no es una marca, es un clic: sin esto, cada vez que alguien
    // pulsa sin querer queda un rectángulo de cero píxeles imposible de ver y de borrar.
    const ancho = Math.abs(dibujando.x2 - dibujando.x1);
    const alto = Math.abs(dibujando.y2 - dibujando.y1);
    if (ancho > 0.01 || alto > 0.01) onAnadir(dibujando);
    setDibujando(null);
  };

  const todas = dibujando ? [...anotaciones, dibujando] : anotaciones;

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
      {todas.map((a, i) => (
        <Marca key={i} anotacion={a} />
      ))}
    </svg>
  );
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
    case "text":
      return (
        <text x={x1} y={y1} fill={a.color} fontSize={34} fontWeight={600} dominantBaseline="hanging">
          {a.text}
        </text>
      );
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
