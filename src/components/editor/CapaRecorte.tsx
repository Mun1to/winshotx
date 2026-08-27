import { useRef, useState } from "react";
import { ordenar, recortaAlgo, type Recorte } from "../../lib/recorte";
import { useT } from "../../lib/i18n";

/**
 * El marco que decide qué trozo de la captura se exporta.
 *
 * Va encima de la vista previa, como la capa de anotaciones, y se arrastra igual. Lo que
 * queda fuera se oscurece en vez de desaparecer: la captura entera se sigue viendo, así
 * que se puede recolocar el marco mirando lo que se está dejando fuera.
 *
 * **La vista previa no se recorta.** Las marcas dibujadas antes siguen donde estaban y
 * quien las vuelve a medir sobre el trozo es Rust al exportar. Recortar aquí obligaría a
 * mover todas las anotaciones cada vez que se toca una esquina del marco.
 */
interface Props {
  activa: boolean;
  recorte: Recorte | null;
  onRecorte: (recorte: Recorte | null) => void;
}

export function CapaRecorte({ activa, recorte, onRecorte }: Props) {
  const t = useT();
  const marco = useRef<SVGSVGElement>(null);
  const [arrastrando, setArrastrando] = useState<Recorte | null>(null);

  const punto = (e: React.PointerEvent): [number, number] => {
    const caja = marco.current?.getBoundingClientRect();
    if (!caja || caja.width === 0 || caja.height === 0) return [0, 0];
    return [
      Math.min(1, Math.max(0, (e.clientX - caja.left) / caja.width)),
      Math.min(1, Math.max(0, (e.clientY - caja.top) / caja.height)),
    ];
  };

  const empezar = (e: React.PointerEvent) => {
    if (!activa) return;
    e.preventDefault();
    const [x, y] = punto(e);
    setArrastrando({ x1: x, y1: y, x2: x, y2: y });
  };

  const mover = (e: React.PointerEvent) => {
    if (!arrastrando) return;
    const [x, y] = punto(e);
    setArrastrando({ ...arrastrando, x2: x, y2: y });
  };

  const soltar = () => {
    if (!arrastrando) return;
    // Un clic sin arrastre no recorta nada. Sin esto, pulsar sin querer con la herramienta
    // puesta dejaba la exportacion reducida a un pixel.
    onRecorte(recortaAlgo(arrastrando) ? ordenar(arrastrando) : null);
    setArrastrando(null);
  };

  const puesto = arrastrando ?? recorte;
  if (!activa && !puesto) return null;

  const o = puesto ? ordenar(puesto) : null;
  const caja = o
    ? { x: o.x1 * 1000, y: o.y1 * 1000, w: (o.x2 - o.x1) * 1000, h: (o.y2 - o.y1) * 1000 }
    : null;

  return (
    <svg
      ref={marco}
      onPointerDown={empezar}
      onPointerMove={mover}
      onPointerUp={soltar}
      onPointerLeave={soltar}
      viewBox="0 0 1000 1000"
      preserveAspectRatio="none"
      role="figure"
      aria-label={t("Lo que se va a exportar")}
      className={`absolute inset-0 size-full ${
        activa ? "cursor-crosshair" : "pointer-events-none"
      }`}
    >
      {caja && (
        <>
          {/* Lo de fuera, oscurecido con un agujero en medio: cuatro rectangulos serian
              cuatro sitios donde equivocarse al llegar a los bordes. */}
          <defs>
            <mask id="recorte-hueco">
              <rect x="0" y="0" width="1000" height="1000" fill="white" />
              <rect x={caja.x} y={caja.y} width={caja.w} height={caja.h} fill="black" />
            </mask>
          </defs>
          <rect
            x="0"
            y="0"
            width="1000"
            height="1000"
            fill="rgba(0,0,0,0.55)"
            mask="url(#recorte-hueco)"
          />
          <rect
            x={caja.x}
            y={caja.y}
            width={caja.w}
            height={caja.h}
            fill="none"
            stroke="#0a9bff"
            strokeWidth={2}
            vectorEffect="non-scaling-stroke"
          />
        </>
      )}
      {activa && !caja && (
        // Sin marco puesto y con la herramienta activa no hay nada que ver, y una capa
        // invisible que se come los clics parece que la aplicacion se ha colgado.
        <rect x="0" y="0" width="1000" height="1000" fill="rgba(10,155,255,0.06)" />
      )}
    </svg>
  );
}
