import { useEffect, useRef, useState, type ReactNode } from "react";
import { EyeOff } from "lucide-react";
import type { Tapa } from "../../lib/contrato";
import { carriles, estirarTapa, quitarTapa } from "../../lib/tapar";
import { formatoTiempo } from "../../lib/tramo";
import { useT } from "../../lib/i18n";

/**
 * Lo tapado de un vídeo, en la línea de tiempo (D4): cada mosaico es un bloque en su rato.
 *
 * Se ven, se quitan (clic derecho, o Supr con el bloque elegido) y se alargan tirando de un
 * borde. No se mueven enteros a propósito: un mosaico tapa lo que había en ESE rato, y
 * llevárselo a otro sitio del tiempo lo dejaría tapando otra cosa y el dato al aire.
 *
 * Dos datos que se ven a la vez van en filas distintas, para que uno no tape al otro.
 * Elegir uno lleva la vista previa a su principio, que es donde se ve qué tapa.
 */

/** Lo que mide el borde de un bloque que se coge para alargarlo. */
const BORDE = 8;
const ALTO_FILA = 20;

interface Props {
  tapas: Tapa[];
  onTapas: (tapas: Tapa[]) => void;
  elegida: number | null;
  onElegir: (indice: number | null) => void;
  /** Lo que dura la grabación, en milisegundos. */
  total: number;
  /** El instante que hay bajo ese punto de la pantalla: la pista mide lo mismo que la tira. */
  msEn: (clientX: number) => number;
  /** Llevar la vista previa a ese instante. */
  onIr: (ms: number) => void;
}

export function PistaDeTapar({ tapas, onTapas, elegida, onElegir, total, msEn, onIr }: Props) {
  const t = useT();
  const [cogida, setCogida] = useState<{ indice: number; lado: "desde" | "hasta" } | null>(null);
  const vivo = useRef({ tapas, onTapas, elegida, onElegir, total, msEn });
  vivo.current = { tapas, onTapas, elegida, onElegir, total, msEn };

  // Tirar de un borde: se mueve con el ratón hasta soltarlo, aunque salga de la pista.
  useEffect(() => {
    if (!cogida) return;
    const mover = (e: PointerEvent) => {
      const v = vivo.current;
      v.onTapas(estirarTapa(v.tapas, cogida.indice, cogida.lado, v.msEn(e.clientX), v.total));
    };
    const soltar = () => setCogida(null);
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", soltar);
    window.addEventListener("pointercancel", soltar);
    return () => {
      window.removeEventListener("pointermove", mover);
      window.removeEventListener("pointerup", soltar);
      window.removeEventListener("pointercancel", soltar);
    };
  }, [cogida]);

  // Con uno elegido, Supr lo quita y Escape lo suelta, antes que el editor (que con Escape
  // se cierra y tira la grabación).
  useEffect(() => {
    if (elegida === null) return;
    const tecla = (e: KeyboardEvent) => {
      const etiqueta = (e.target as HTMLElement | null)?.tagName;
      if (etiqueta === "INPUT" || etiqueta === "TEXTAREA") return;
      const v = vivo.current;
      if (v.elegida === null) return;
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        e.stopPropagation();
        v.onTapas(quitarTapa(v.tapas, v.elegida));
        v.onElegir(null);
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        v.onElegir(null);
      }
    };
    window.addEventListener("keydown", tecla, true);
    return () => window.removeEventListener("keydown", tecla, true);
  }, [elegida]);

  if (tapas.length === 0) return null;
  const { carril, cuantos } = carriles(tapas);
  const pct = (ms: number) => `${(Math.min(total, Math.max(0, ms)) / Math.max(1, total)) * 100}%`;

  return (
    <div
      data-pista-tapar
      aria-label={t("Pista de lo tapado")}
      // Un clic en la pista vacía suelta el elegido, como en la de zooms.
      onPointerDown={(e) => {
        if (e.button === 0 && e.target === e.currentTarget) onElegir(null);
      }}
      className="relative mt-2 touch-none rounded-[5px] bg-white/[0.04] select-none"
      style={{ height: cuantos * (ALTO_FILA + 2) + 2 }}
    >
      {tapas.map((tapa, i) => (
        <BloqueDeTapa
          key={`${i}-${tapa.x}-${tapa.y}`}
          tapa={tapa}
          izquierda={pct(tapa.desdeMs)}
          ancho={pct(tapa.hastaMs - tapa.desdeMs)}
          arriba={2 + carril[i] * (ALTO_FILA + 2)}
          elegido={elegida === i}
          onCoger={(lado) => {
            onElegir(i);
            if (lado) setCogida({ indice: i, lado });
            else onIr(tapa.desdeMs);
          }}
          onQuitar={() => {
            onTapas(quitarTapa(tapas, i));
            onElegir(null);
          }}
        />
      ))}
    </div>
  );
}

function BloqueDeTapa({
  tapa,
  izquierda,
  ancho,
  arriba,
  elegido,
  onCoger,
  onQuitar,
}: {
  tapa: Tapa;
  izquierda: string;
  ancho: string;
  arriba: number;
  elegido: boolean;
  /** Por un borde, para alargarlo; por el medio (`null`), para elegirlo e ir a verlo. */
  onCoger: (lado: "desde" | "hasta" | null) => void;
  onQuitar: () => void;
}) {
  const t = useT();
  const etiqueta = t("Mosaico de {desde} a {hasta}", {
    desde: formatoTiempo(tapa.desdeMs),
    hasta: formatoTiempo(tapa.hastaMs),
  });
  return (
    <div
      role="button"
      tabIndex={0}
      data-tapa
      aria-label={etiqueta}
      aria-pressed={elegido}
      title={t("Tira de un borde para alargarlo; clic derecho o Supr para quitarlo")}
      onPointerDown={(e) => {
        e.stopPropagation();
        if (e.button !== 0) return;
        const caja = e.currentTarget.getBoundingClientRect();
        const dentro = e.clientX - caja.left;
        onCoger(dentro < BORDE ? "desde" : caja.width - dentro < BORDE ? "hasta" : null);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onCoger(null);
        }
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        onQuitar();
      }}
      style={{
        left: izquierda,
        width: ancho,
        top: arriba,
        height: ALTO_FILA,
        // Un mosaico pequeño dibujado: dice lo que es sin un rótulo, en un bloque que mide poco.
        backgroundImage:
          "conic-gradient(rgba(255,255,255,0.26) 25%, rgba(255,255,255,0.1) 0 50%, rgba(255,255,255,0.26) 0 75%, rgba(255,255,255,0.1) 0)",
        backgroundSize: "8px 8px",
      }}
      className={`group absolute flex min-w-[6px] cursor-pointer items-center overflow-hidden rounded-[4px] border-2 bg-neutral-700 px-1.5 outline-none transition-colors focus-visible:border-blue-300 ${
        elegido ? "border-white" : "border-white/35 hover:border-white/70"
      }`}
    >
      <EyeOff className="pointer-events-none size-3.5 shrink-0 text-white" aria-hidden="true" />
      {/* Los bordes que se cogen, que se ven al pasar por encima. */}
      <span className="pointer-events-none absolute inset-y-0.5 left-0.5 w-[3px] cursor-ew-resize rounded-full bg-white/0 group-hover:bg-white/80" />
      <span className="pointer-events-none absolute inset-y-0.5 right-0.5 w-[3px] cursor-ew-resize rounded-full bg-white/0 group-hover:bg-white/80" />
    </div>
  );
}

/**
 * Los mandos del mosaico elegido, debajo de la línea de tiempo: qué rato tapa, quitarlo, o
 * quitarlos todos de una vez.
 */
export function AjustesDeLaTapa({
  tapa,
  cuantas,
  onQuitar,
  onQuitarTodas,
  onListo,
}: {
  tapa: Tapa;
  cuantas: number;
  onQuitar: () => void;
  onQuitarTodas: () => void;
  onListo: () => void;
}) {
  const t = useT();
  return (
    <div data-ajustes-tapa className="flex flex-wrap items-center gap-x-6 gap-y-2">
      <span className="text-neutral-100">
        {t("Mosaico de {desde} a {hasta}", {
          desde: formatoTiempo(tapa.desdeMs),
          hasta: formatoTiempo(tapa.hastaMs),
        })}
      </span>
      <span className="text-neutral-300">{t("Tira de un borde para que tape más rato")}</span>
      <div className="ml-auto flex items-center gap-2">
        <Boton onClick={onQuitar}>{t("Quitar el mosaico")}</Boton>
        {cuantas > 1 && <Boton onClick={onQuitarTodas}>{t("Quitar todos ({n})", { n: cuantas })}</Boton>}
        <Boton onClick={onListo}>{t("Listo")}</Boton>
      </div>
    </div>
  );
}

function Boton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex h-8 items-center gap-1.5 rounded-lg bg-white/10 px-3 text-[13px] font-medium whitespace-nowrap text-neutral-100 transition-colors hover:bg-white/15 hover:text-white"
    >
      {children}
    </button>
  );
}
