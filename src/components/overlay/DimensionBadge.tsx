import type { Proporcion } from "../../lib/proporcion";

interface Props {
  /** Ya en pixeles fisicos, que es lo que el usuario espera leer. */
  width: number;
  height: number;
  /** La proporción fija, si la hay: se dice al lado de la medida, que es donde se mira. */
  proporcion?: Proporcion;
  /** Posicion en coordenadas CSS del overlay. */
  left: number;
  top: number;
  editable?: boolean;
}

export function DimensionBadge({ width, height, proporcion = "libre", left, top }: Props) {
  return (
    <div
      style={{ left, top }}
      className="pointer-events-none absolute z-30 flex items-center gap-1.5 rounded-md border border-white/10 bg-neutral-900/90 px-2 py-1 font-mono text-[12px] tabular-nums text-white shadow-lg backdrop-blur-md"
    >
      <span>
        {Math.round(width)} × {Math.round(height)}
      </span>
      {/* Libre no se dice: es lo de siempre, y una palabra más en cada recorte es ruido. En
          azul, el de lo que está puesto, como su botón de la barra de arriba. */}
      {proporcion !== "libre" && (
        <span data-proporcion className="font-sans font-semibold text-blue-300">
          · {proporcion}
        </span>
      )}
    </div>
  );
}
