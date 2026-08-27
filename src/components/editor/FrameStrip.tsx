import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { clamp, formatTimecode } from "../../lib/format";
import type { FrameMeta } from "../../lib/types";
import { useT } from "../../lib/i18n";

const THUMB_W = 56;
const THUMB_H = 40;

type Drag = "in" | "out" | "playhead" | null;

interface Props {
  frames: FrameMeta[];
  inIndex: number;
  outIndex: number;
  currentIndex: number;
  onChangeIn: (index: number) => void;
  onChangeOut: (index: number) => void;
  onScrub: (index: number) => void;
}

export function FrameStrip({
  frames,
  inIndex,
  outIndex,
  currentIndex,
  onChangeIn,
  onChangeOut,
  onScrub,
}: Props) {
  const t = useT();
  const trackRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<Drag>(null);

  const ultimo = Math.max(0, frames.length - 1);

  // Las rutas no cambian mientras dure la sesion, pero sin esto se recalculaban las de los
  // ochenta y pico fotogramas en cada tic de la reproduccion.
  const miniaturas = useMemo(() => frames.map((f) => convertFileSrc(f.thumbPath)), [frames]);

  /** Posicion del puntero dentro de la tira, en pixeles. */
  const xEnTira = useCallback((clientX: number) => {
    const track = trackRef.current;
    if (!track) return 0;
    const rect = track.getBoundingClientRect();
    return clientX - rect.left + track.scrollLeft;
  }, []);

  /** El fotograma sobre el que esta el puntero. */
  const indexAt = useCallback(
    (clientX: number) => clamp(Math.floor(xEnTira(clientX) / THUMB_W), 0, ultimo),
    [xEnTira, ultimo],
  );

  /**
   * Lo mismo para el marcador B, que se dibuja en el borde DERECHO de su fotograma.
   * Con el redondeo normal, soltarlo justo donde estaba lo movia un fotograma a la
   * derecha y ese fotograma de mas se colaba en la exportacion.
   */
  const indexAtEnd = useCallback(
    (clientX: number) => clamp(Math.ceil(xEnTira(clientX) / THUMB_W) - 1, 0, ultimo),
    [xEnTira, ultimo],
  );

  useEffect(() => {
    if (!drag) return;
    const onMove = (e: PointerEvent) => {
      // Los limites del recorte los aplica el editor: aqui solo se informa del
      // fotograma sobre el que se ha soltado el marcador.
      if (drag === "in") onChangeIn(indexAt(e.clientX));
      else if (drag === "out") onChangeOut(indexAtEnd(e.clientX));
      else onScrub(indexAt(e.clientX));
    };
    const onUp = () => setDrag(null);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [drag, indexAt, indexAtEnd, onChangeIn, onChangeOut, onScrub]);

  const width = Math.max(frames.length * THUMB_W, 1);
  const kept = frames.slice(inIndex, outIndex + 1);
  const keptMs =
    (frames[outIndex]?.timestampMs ?? 0) +
    (frames[outIndex]?.durationMs ?? 0) -
    (frames[inIndex]?.timestampMs ?? 0);

  return (
    <section className="shrink-0 border-t border-white/8 bg-black/25 px-3 pt-2 pb-3">
      <div className="mb-1.5 flex items-center justify-between text-[11px] tabular-nums text-neutral-500">
        <span>
          {t("Fotograma {actual} de {total}", {
            actual: currentIndex + 1,
            total: frames.length,
          })}{" "}
          · {formatTimecode(frames[currentIndex]?.timestampMs ?? 0)}
        </span>
        <span>
          {t("Recorte {desde} a {hasta}", { desde: inIndex + 1, hasta: outIndex + 1 })} ·{" "}
          {kept.length === 1
            ? t("{n} fotograma", { n: kept.length })
            : t("{n} fotogramas", { n: kept.length })}{" "}
          · {formatTimecode(keptMs)}
        </span>
      </div>

      <div
        ref={trackRef}
        onPointerDown={(e) => {
          onScrub(indexAt(e.clientX));
          setDrag("playhead");
        }}
        className="relative h-[52px] overflow-x-auto overflow-y-hidden rounded-lg border border-white/8 bg-black/40"
      >
        <div className="relative h-[42px]" style={{ width }}>
          {frames.map((frame, posicion) => (
            <img
              key={frame.index}
              src={miniaturas[posicion]}
              alt=""
              draggable={false}
              loading="lazy"
              style={{
                left: frame.index * THUMB_W,
                width: THUMB_W,
                height: THUMB_H,
              }}
              className="absolute top-px object-cover opacity-90"
            />
          ))}

          {/* Lo que se descarta se apaga, como en ScreenToGif. */}
          <div
            style={{ left: 0, width: inIndex * THUMB_W }}
            className="absolute inset-y-0 bg-black/65"
          />
          <div
            style={{
              left: (outIndex + 1) * THUMB_W,
              width: Math.max(0, width - (outIndex + 1) * THUMB_W),
            }}
            className="absolute inset-y-0 bg-black/65"
          />

          <div
            style={{ left: inIndex * THUMB_W, width: (outIndex - inIndex + 1) * THUMB_W }}
            className="pointer-events-none absolute inset-y-0 border-y-2 border-blue-500/70"
          />

          <Handle
            side="in"
            left={inIndex * THUMB_W}
            onGrab={() => setDrag("in")}
          />
          <Handle
            side="out"
            left={(outIndex + 1) * THUMB_W}
            onGrab={() => setDrag("out")}
          />

          <div
            style={{ left: currentIndex * THUMB_W + THUMB_W / 2 }}
            className="pointer-events-none absolute inset-y-0 w-px bg-white shadow-[0_0_6px_rgba(255,255,255,0.8)]"
          />
        </div>
      </div>
    </section>
  );
}

function Handle({
  side,
  left,
  onGrab,
}: {
  side: "in" | "out";
  left: number;
  onGrab: () => void;
}) {
  const t = useT();
  return (
    <div
      onPointerDown={(e) => {
        e.stopPropagation();
        onGrab();
      }}
      style={{ left }}
      className={`absolute inset-y-0 z-10 w-2.5 cursor-ew-resize bg-blue-500 ${
        side === "in" ? "-translate-x-full rounded-l-md" : "rounded-r-md"
      }`}
      title={side === "in" ? t("Marca A (tecla I)") : t("Marca B (tecla O)")}
    >
      <span className="absolute top-1/2 left-1/2 h-4 w-px -translate-x-1/2 -translate-y-1/2 bg-white/70" />
    </div>
  );
}
