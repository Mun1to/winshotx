import { useEffect, useRef } from "react";

const SIZE = 132; // lado del recuadro en px CSS
const ZOOM = 6; // cada pixel fisico se pinta como 6x6

interface Props {
  /** Canvas offscreen con el freeze del monitor a resolucion fisica. */
  source: HTMLCanvasElement | null;
  /** Punto bajo el cursor, en pixeles fisicos del monitor. */
  px: number;
  py: number;
  /** Esquina del recuadro en coordenadas CSS del overlay. */
  left: number;
  top: number;
  hex: string;
}

export function Magnifier({ source, px, py, left, top, hex }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !source) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const span = Math.floor(SIZE / ZOOM); // pixeles fisicos visibles
    const half = Math.floor(span / 2);
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, SIZE, SIZE);
    ctx.drawImage(source, px - half, py - half, span, span, 0, 0, SIZE, SIZE);

    // Reticula: una cruz sobre el pixel exacto que se esta leyendo.
    ctx.strokeStyle = "rgba(255,255,255,0.28)";
    ctx.lineWidth = 1;
    for (let i = 0; i <= span; i++) {
      const p = i * ZOOM + 0.5;
      ctx.beginPath();
      ctx.moveTo(p, 0);
      ctx.lineTo(p, SIZE);
      ctx.moveTo(0, p);
      ctx.lineTo(SIZE, p);
      ctx.stroke();
    }
    ctx.strokeStyle = "#3b82f6";
    ctx.lineWidth = 2;
    ctx.strokeRect(half * ZOOM, half * ZOOM, ZOOM, ZOOM);
  }, [source, px, py]);

  return (
    <div
      style={{ left, top }}
      className="pointer-events-none absolute z-40 overflow-hidden rounded-xl border border-white/15 bg-neutral-900/90 shadow-2xl backdrop-blur-md"
    >
      <canvas ref={canvasRef} width={SIZE} height={SIZE} className="block" />
      <div className="flex items-center justify-between gap-3 border-t border-white/10 px-2 py-1.5 font-mono text-[10px] tabular-nums text-neutral-300">
        <span>
          {Math.round(px)}, {Math.round(py)}
        </span>
        <span className="flex items-center gap-1.5 uppercase">
          <span
            className="size-2.5 rounded-[3px] border border-white/20"
            style={{ backgroundColor: hex }}
          />
          {hex}
        </span>
      </div>
    </div>
  );
}
