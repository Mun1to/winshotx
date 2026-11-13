import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import {
  controlDeCurva,
  FOCO_OSCURECE,
  simplificarTrazo,
  type Anotacion,
  type Herramienta,
  type Punto,
} from "../../lib/anotaciones";
import { useT } from "../../lib/i18n";

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
  /**
   * Lo que mide de ancho la imagen de verdad, en píxeles. Al exportar, la letra del texto
   * sale de ahí (`encode::anotacion::texto`), y la vista previa la saca igual para que lo
   * que se escribe encima mida lo que va a medir en el archivo.
   */
  anchoImagen: number;
}

/** Dónde se está escribiendo un texto, de 0 a 1, y lo que lleva escrito. */
interface Escribiendo {
  x: number;
  y: number;
  valor: string;
}

export function CapaAnotaciones({ herramienta, color, anotaciones, onAnadir, anchoImagen }: Props) {
  const t = useT();
  const marco = useRef<SVGSVGElement>(null);
  const [dibujando, setDibujando] = useState<Anotacion | null>(null);
  /**
   * El texto que se está escribiendo encima de la imagen. Va también en una ref porque se
   * guarda desde dos sitios que llegan juntos con el mismo clic (pulsar fuera en la imagen,
   * y el campo que pierde el foco por ese mismo clic): el primero lo guarda y lo vacía, y
   * el segundo ya no encuentra nada. Sin eso salían dos textos iguales, uno encima del otro.
   */
  const [escribiendo, setEscribiendo] = useState<Escribiendo | null>(null);
  const abierto = useRef<Escribiendo | null>(null);
  abierto.current = escribiendo;
  const campo = useRef<HTMLInputElement>(null);
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

  /** Deja el texto escrito como una marca más. Sin nada escrito, no deja nada. */
  const guardarTexto = () => {
    const sitio = abierto.current;
    if (!sitio) return;
    abierto.current = null;
    setEscribiendo(null);
    const escrito = sitio.valor.trim();
    if (escrito) onAnadir({ kind: "text", x1: sitio.x, y1: sitio.y, x2: sitio.x, y2: sitio.y, color, text: escrito });
  };

  const tirarTexto = () => {
    abierto.current = null;
    setEscribiendo(null);
  };

  // El foco va al campo en cuanto aparece, antes de que se pinte: con un fotograma de
  // retraso, lo que se tecleaba nada más pulsar se perdía (en Edge, «Hola» salía «la»). El
  // clic que lo abre no se lo quita, porque `empezar` cancela el `pointerdown` y con él el
  // `mousedown`, que es quien mueve el foco.
  const hayCampo = escribiendo !== null;
  useLayoutEffect(() => {
    if (hayCampo) campo.current?.focus();
  }, [hayCampo]);

  // Cambiar de herramienta con el texto a medias lo guarda, igual que pulsar fuera.
  useEffect(() => {
    if (herramienta !== "text") guardarTexto();
    // Solo al cambiar de herramienta; `guardarTexto` lee lo vivo de la ref.
  }, [herramienta]);

  const empezar = (e: React.PointerEvent) => {
    // Con un texto a medias, pulsar fuera de él lo guarda, y ese clic no hace nada más:
    // quien pulsa fuera para terminar no quiere empezar otro texto sin darse cuenta.
    if (abierto.current) {
      e.preventDefault();
      guardarTexto();
      return;
    }
    if (!herramienta) return;
    e.preventDefault();
    const [x, y] = punto(e);
    // El texto no se arrastra: se pulsa donde va, se escribe ahí mismo, y Intro o pulsar
    // fuera lo deja puesto.
    if (herramienta === "text") {
      setEscribiendo({ x, y, valor: "" });
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
  const textos = todas.filter((a) => a.kind === "text");

  return (
    <>
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
    {/*
      Los textos van en HTML y no en el SVG: el SVG va estirado a 1000 x 1000, y un texto
      ahí dentro salía ensanchado en una captura apaisada. Aquí miden lo que van a medir en
      el archivo, y el campo donde se escribe es la misma letra en el mismo sitio, así que
      al pulsar Intro no se mueve nada.
    */}
    {(textos.length > 0 || escribiendo) && (
      <div className="pointer-events-none absolute inset-0 overflow-hidden" style={{ containerType: "size" }}>
        {textos.map((a, i) => (
          <span key={i} data-marca="text" style={letraDelTexto(anchoImagen, a.x1, a.y1, a.color)}>
            {a.text}
          </span>
        ))}
        {escribiendo && (
          <input
            ref={campo}
            value={escribiendo.valor}
            onChange={(e) => {
              const valor = e.target.value;
              setEscribiendo((antes) => antes && { ...antes, valor });
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                guardarTexto();
              } else if (e.key === "Escape") {
                // Escape tira el texto a medias y nada más: que no le llegue al editor,
                // que con Escape suelta la herramienta o se cierra.
                e.preventDefault();
                e.stopPropagation();
                tirarTexto();
              }
            }}
            onBlur={guardarTexto}
            placeholder={t("Escribe aquí")}
            aria-label={t("Texto")}
            spellCheck={false}
            className="pointer-events-auto rounded-sm border-0 bg-transparent outline-[1.5px] outline-offset-2 outline-current outline-dashed placeholder:text-current placeholder:opacity-55"
            style={
              {
                ...letraDelTexto(anchoImagen, escribiendo.x, escribiendo.y, color),
                caretColor: color,
                // El campo crece con lo que se escribe, como el texto que va a quedar.
                fieldSizing: "content",
                minWidth: "1ch",
                margin: 0,
              } as CSSProperties
            }
          />
        )}
      </div>
    )}
    </>
  );
}

/**
 * La letra de un texto en la vista previa, sacada como la saca Rust al exportar
 * (`encode::anotacion::texto`): Segoe UI seminegrita, con la celda a un cuarentaidosavo del
 * ancho de la imagen y entre 14 y 72 píxeles de ella, con dos de aire, y metida dentro si
 * se sale por la derecha o por abajo (`esquina_metida`).
 *
 * Va en unidades del contenedor para no medir la capa: 100 cqw es su ancho, que es el de
 * la imagen, así que un píxel de la imagen son `100 / anchoImagen` cqw a cualquier tamaño.
 */
export function letraDelTexto(anchoImagen: number, x: number, y: number, color: string): CSSProperties {
  const px = 100 / Math.max(1, anchoImagen);
  const celda = `clamp(${14 * px}cqw, ${100 / 42}cqw, ${72 * px}cqw)`;
  return {
    position: "absolute",
    left: `${x * 100}%`,
    top: `${y * 100}%`,
    translate: `min(0px, calc(${(1 - x) * 100}cqw - 100%)) min(0px, calc(${(1 - y) * 100}cqh - 100%))`,
    padding: `${2 * px}cqw`,
    fontFamily: '"Segoe UI", system-ui, sans-serif',
    fontWeight: 600,
    // GDI pide la letra por el alto de su celda, y CSS por el de la eme: en Segoe UI la
    // celda mide 1,33 emes.
    fontSize: `calc(${celda} / 1.33)`,
    lineHeight: 1.33,
    color,
    whiteSpace: "pre",
  };
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
      // Lo pinta la capa de HTML de encima, a su tamaño de verdad.
      return null;
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
