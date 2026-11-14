/**
 * S25 (marco): la ventana o el portátil en la vista previa, dibujados en SVG con las mismas
 * medidas y colores que pinta Rust en el archivo (`src/lib/carcasa.ts`, espejo de
 * `encode/carcasa.rs`). La captura va encima, en el hueco: esto solo pinta lo de alrededor.
 */
import { useId } from "react";
import {
  botones,
  caminoDe,
  COLORES_DEL_PORTATIL,
  coloresDeLaVentana,
  detallesDelPortatil,
  escala,
  TRAZO,
  type CarcasaElegida,
  type Geometria,
} from "../../lib/carcasa";

interface Props {
  elegida: CarcasaElegida;
  oscura: boolean;
  geometria: Geometria;
  /** Lo que mide lo que se exporta, que es con lo que se miden la barra y el bisel. */
  ancho: number;
  alto: number;
  /** Píxeles de pantalla por cada píxel del archivo. */
  k: number;
  /** La sombra de debajo, la misma que pone Rust: cae un 22 % del aire y se difumina un 45 %. */
  sombra: number | null;
}

export function CarcasaVista({ elegida, oscura, geometria: g, ancho, alto, k, sombra }: Props) {
  const id = useId();
  const w = g.ancho * k;
  const h = g.alto * k;
  return (
    <svg
      data-carcasa={elegida === "ventana" ? (oscura ? "ventana-oscura" : "ventana-clara") : "portatil"}
      aria-hidden="true"
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      className="pointer-events-none absolute top-0 left-0"
      style={{
        overflow: "visible",
        filter: sombra
          ? `drop-shadow(0 ${Math.round(sombra * 0.22)}px ${Math.round(sombra * 0.45)}px rgba(0, 0, 0, 0.42))`
          : undefined,
      }}
    >
      {elegida === "ventana" ? (
        <Ventana g={g} k={k} oscura={oscura} ancho={ancho} alto={alto} />
      ) : (
        <Portatil g={g} k={k} ancho={ancho} alto={alto} degradado={`${id}-base`} />
      )}
    </svg>
  );
}

function Ventana({ g, k, oscura, ancho, alto }: { g: Geometria; k: number; oscura: boolean; ancho: number; alto: number }) {
  const colores = coloresDeLaVentana(oscura);
  const s = escala(ancho, alto);
  const b = g.x;
  const barra = g.y - b;
  const radio = g.sombra[0].radios[0];
  const dentro = Math.max(0, radio - b);
  const grosor = Math.max(1, TRAZO * s) * k;
  const [min, max, cerrar] = botones(g, ancho, alto);
  const medio = Math.max(1, TRAZO * s) / 2;
  return (
    <>
      <path d={caminoDe(g.sombra[0], k)} fill={colores.borde} />
      <path
        d={caminoDe({ x: b, y: b, ancho: g.ancho - 2 * b, alto: barra, radios: [dentro, dentro, 0, 0] }, k)}
        fill={colores.barra}
      />
      <g stroke={colores.glifo} strokeWidth={grosor} fill="none" strokeLinecap="butt">
        <line
          x1={min[0] * k}
          x2={(min[0] + min[2]) * k}
          y1={(min[1] + Math.floor(min[2] / 2) + medio) * k}
          y2={(min[1] + Math.floor(min[2] / 2) + medio) * k}
        />
        <rect
          x={(max[0] + medio) * k}
          y={(max[1] + medio) * k}
          width={(max[2] - 2 * medio) * k}
          height={(max[2] - 2 * medio) * k}
        />
        <line x1={(cerrar[0] + medio) * k} y1={(cerrar[1] + medio) * k} x2={(cerrar[0] + cerrar[2] - medio) * k} y2={(cerrar[1] + cerrar[2] - medio) * k} />
        <line x1={(cerrar[0] + cerrar[2] - medio) * k} y1={(cerrar[1] + medio) * k} x2={(cerrar[0] + medio) * k} y2={(cerrar[1] + cerrar[2] - medio) * k} />
      </g>
    </>
  );
}

function Portatil({ g, k, ancho, alto, degradado }: { g: Geometria; k: number; ancho: number; alto: number; degradado: string }) {
  const [tapa, base] = g.sombra;
  const d = detallesDelPortatil(g, ancho, alto);
  const c = COLORES_DEL_PORTATIL;
  return (
    <>
      <defs>
        <linearGradient id={degradado} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={c.baseArriba} />
          <stop offset="1" stopColor={c.baseAbajo} />
        </linearGradient>
      </defs>
      <path d={caminoDe(base, k)} fill={`url(#${degradado})`} />
      <path d={caminoDe(d.muesca, k)} fill={c.muesca} />
      <path d={caminoDe(tapa, k)} fill={c.canto} />
      <path d={caminoDe(d.bisel, k)} fill={c.bisel} />
      <circle cx={d.camara.cx * k} cy={d.camara.cy * k} r={d.camara.r * k} fill={c.camara} />
    </>
  );
}
