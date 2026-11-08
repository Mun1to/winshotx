import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as EventoDeTecla,
  type ReactNode,
  type RefObject,
} from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { FrameMeta } from "../../lib/types";
import { duracionDe, formatoTiempo, limitarTramo, muestrasDeMiniaturas } from "../../lib/tramo";
import { useT } from "../../lib/i18n";

/** Lo que se está arrastrando: una de las dos asas o el cabezal. */
type Arrastre = "desde" | "hasta" | "cabezal" | null;

/** Lo que mide de ancho cada asa. Asoman por fuera de lo que se queda, no lo tapan. */
const ASA = 14;

interface Props {
  frames: FrameMeta[];
  /** Lo que se queda, en milisegundos de la grabación. */
  desdeMs: number;
  hastaMs: number;
  /** Dónde está la vista previa cuando no suena. Mientras suena, manda el vídeo. */
  posicionMs: number;
  /**
   * El vídeo, para seguirlo mientras suena sin pasar por React: el cabezal y el reloj se
   * mueven sesenta veces por segundo, y repintar el editor entero a ese ritmo es lo que
   * hace que una línea de tiempo vaya a saltos.
   */
  videoRef: RefObject<HTMLVideoElement | null>;
  reproduciendo: boolean;
  /** Ancho entre alto de lo grabado, para que las miniaturas no salgan deformadas. */
  proporcion: number;
  /** El botón de reproducir, o lo que haya en su sitio mientras se prepara el vídeo. */
  delante: ReactNode;
  onTramo: (desdeMs: number, hastaMs: number, mueve: "desde" | "hasta" | "las dos") => void;
  /** Llevar la vista previa a ese instante. */
  onIr: (ms: number) => void;
  /** Se ha llegado al final de lo que se queda mientras sonaba. */
  onFinal: () => void;
}

/**
 * La línea de tiempo del editor: lo grabado a lo ancho y dos asas para quedarse con un
 * trozo.
 *
 * Sustituye a la tira de antes, que ponía cada fotograma a 56 px y se desplazaba de lado:
 * un minuto de grabación medía cien mil píxeles, y para mover el final había que ir a
 * buscarlo con la rueda. Aquí la grabación entera cabe siempre en el ancho de la ventana,
 * se reparte por TIEMPO (diez segundos quietos ocupan lo que ocupan diez segundos) y las
 * asas se cogen sin apuntar: asoman por fuera, miden lo que un dedo, y mientras se
 * arrastran la vista previa enseña el sitio exacto del corte.
 */
export function LineaDeTiempo({
  frames,
  desdeMs,
  hastaMs,
  posicionMs,
  videoRef,
  reproduciendo,
  proporcion,
  delante,
  onTramo,
  onIr,
  onFinal,
}: Props) {
  const t = useT();
  const pista = useRef<HTMLDivElement>(null);
  const cabezal = useRef<HTMLDivElement>(null);
  const reloj = useRef<HTMLSpanElement>(null);
  const [arrastre, setArrastre] = useState<Arrastre>(null);
  const [anchoDePista, setAnchoDePista] = useState(0);
  const total = Math.max(1, duracionDe(frames));

  // Lo último que se sabe, para los oyentes de la ventana y el bucle del cabezal, que
  // viven más que un pintado y no pueden quedarse con los valores de cuando nacieron.
  const vivo = useRef({ desdeMs, hastaMs, onTramo, onIr, onFinal });
  vivo.current = { desdeMs, hastaMs, onTramo, onIr, onFinal };

  useLayoutEffect(() => {
    const el = pista.current;
    if (!el) return;
    const medir = () => setAnchoDePista(el.getBoundingClientRect().width);
    medir();
    const observador = new ResizeObserver(medir);
    observador.observe(el);
    return () => observador.disconnect();
  }, []);

  /**
   * Cuántas miniaturas caben sin deformarse, y cuál va en cada hueco.
   *
   * Son siempre unas pocas, las que caben, y no una por fotograma: con diez minutos de
   * grabación la tira de antes montaba dieciocho mil imágenes en la página.
   */
  const huecos = Math.max(1, Math.ceil(anchoDePista / Math.max(24, 56 * proporcion)));
  const muestras = useMemo(() => muestrasDeMiniaturas(frames, huecos), [frames, huecos]);

  /** El instante que hay debajo del puntero, metido dentro de la grabación. */
  const msEn = useCallback(
    (clientX: number) => {
      const caja = pista.current?.getBoundingClientRect();
      if (!caja || caja.width <= 0) return 0;
      const parte = Math.min(1, Math.max(0, (clientX - caja.left) / caja.width));
      return Math.round(parte * total);
    },
    [total],
  );

  useEffect(() => {
    if (!arrastre) return;
    const mover = (e: PointerEvent) => {
      const ms = msEn(e.clientX);
      const { desdeMs: d, hastaMs: h } = vivo.current;
      if (arrastre === "cabezal") {
        vivo.current.onIr(ms);
        return;
      }
      const nuevo =
        arrastre === "desde" ? limitarTramo(ms, h, total, "desde") : limitarTramo(d, ms, total, "hasta");
      vivo.current.onTramo(nuevo.desdeMs, nuevo.hastaMs, arrastre);
    };
    const soltar = () => setArrastre(null);
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", soltar);
    window.addEventListener("pointercancel", soltar);
    return () => {
      window.removeEventListener("pointermove", mover);
      window.removeEventListener("pointerup", soltar);
      window.removeEventListener("pointercancel", soltar);
    };
  }, [arrastre, msEn, total]);

  /**
   * Mientras suena, el cabezal y el reloj siguen al vídeo en cada pintada de la pantalla,
   * tocando el DOM a mano. Y al llegar al final de lo que se queda, se avisa: el aviso de
   * tiempo del propio vídeo llega cuatro veces por segundo, y con él el corte se oía hasta
   * un cuarto de segundo más allá de donde se soltó el asa.
   */
  useEffect(() => {
    if (!reproduciendo) return;
    let id = 0;
    const paso = () => {
      const video = videoRef.current;
      if (video) {
        const ms = video.currentTime * 1000;
        if (ms >= vivo.current.hastaMs) {
          vivo.current.onFinal();
          return;
        }
        if (cabezal.current) cabezal.current.style.left = `${(ms / total) * 100}%`;
        if (reloj.current) reloj.current.textContent = formatoTiempo(ms);
      }
      id = requestAnimationFrame(paso);
    };
    id = requestAnimationFrame(paso);
    return () => cancelAnimationFrame(id);
  }, [reproduciendo, total, videoRef]);

  /** Las flechas mueven el asa que tiene el foco: una décima, o un segundo con Mayús. */
  const teclaEnAsa = (lado: "desde" | "hasta") => (e: EventoDeTecla<HTMLDivElement>) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    // Que no le llegue al editor, que con las flechas mueve el cabezal.
    e.preventDefault();
    e.stopPropagation();
    const paso = (e.shiftKey ? 1000 : 100) * (e.key === "ArrowLeft" ? -1 : 1);
    const nuevo =
      lado === "desde"
        ? limitarTramo(desdeMs + paso, hastaMs, total, "desde")
        : limitarTramo(desdeMs, hastaMs + paso, total, "hasta");
    onTramo(nuevo.desdeMs, nuevo.hastaMs, lado);
  };

  const pct = (ms: number) => `${(Math.min(total, Math.max(0, ms)) / total) * 100}%`;
  const entero = desdeMs <= 0 && hastaMs >= total;

  return (
    <section className="shrink-0 border-t border-white/8 bg-black/25 px-4 pt-3 pb-3">
      <div className="flex items-start gap-2">
        <div className="flex h-14 shrink-0 items-center gap-1.5">
          {delante}
          {/* Dónde está la vista previa. Mientras suena lo escribe el bucle de arriba. */}
          <span
            ref={reloj}
            className="w-[58px] text-[14px] font-medium tabular-nums text-neutral-100"
          >
            {formatoTiempo(posicionMs)}
          </span>
        </div>

        <div className="min-w-0 flex-1" style={{ paddingInline: ASA }}>
          <div
            ref={pista}
            data-pista
            onPointerDown={(e) => {
              // Pulsar en la tira lleva ahí la vista previa, y arrastrar la recorre.
              onIr(msEn(e.clientX));
              setArrastre("cabezal");
            }}
            className="relative h-14 cursor-pointer touch-none select-none"
          >
            <div className="absolute inset-0 flex overflow-hidden rounded-[5px] bg-white/5">
              {muestras.map((indice, hueco) => {
                const ruta = frames[indice]?.thumbPath;
                return ruta ? (
                  <img
                    key={hueco}
                    src={convertFileSrc(ruta)}
                    alt=""
                    draggable={false}
                    className="h-full min-w-0 flex-1 object-cover"
                  />
                ) : (
                  // Las miniaturas se hacen después de abrir el editor: hasta entonces,
                  // huecos que laten en vez de imágenes rotas.
                  <div
                    key={hueco}
                    data-miniatura-pendiente
                    className="h-full min-w-0 flex-1 animate-pulse border-r border-black/30 bg-white/5"
                  />
                );
              })}
            </div>

            {/* Lo que se tira, apagado, y lo que se queda, con su marco. */}
            <div
              className="pointer-events-none absolute inset-y-0 left-0 rounded-l-[5px] bg-black/70"
              style={{ width: pct(desdeMs) }}
            />
            <div
              className="pointer-events-none absolute inset-y-0 right-0 rounded-r-[5px] bg-black/70"
              style={{ left: pct(hastaMs) }}
            />
            <div
              className="pointer-events-none absolute -inset-y-px border-y-[3px] border-blue-500"
              style={{ left: pct(desdeMs), right: `calc(100% - ${pct(hastaMs)})` }}
            />

            <Asa
              lado="desde"
              ms={desdeMs}
              total={total}
              left={`calc(${pct(desdeMs)} - ${ASA}px)`}
              arrastrando={arrastre === "desde"}
              etiqueta={t("Principio: arrástralo para recortar (tecla I)")}
              onCoger={() => setArrastre("desde")}
              onTecla={teclaEnAsa("desde")}
            />
            <Asa
              lado="hasta"
              ms={hastaMs}
              total={total}
              left={pct(hastaMs)}
              arrastrando={arrastre === "hasta"}
              etiqueta={t("Final: arrástralo para recortar (tecla O)")}
              onCoger={() => setArrastre("hasta")}
              onTecla={teclaEnAsa("hasta")}
            />

            <div
              ref={cabezal}
              data-cabezal
              className="pointer-events-none absolute -top-1.5 -bottom-1.5 z-20 w-[3px] -translate-x-1/2 rounded-full bg-white shadow-[0_0_0_1px_rgba(0,0,0,0.45)]"
              style={{ left: pct(posicionMs) }}
            />
          </div>

          <div className="mt-2 flex min-h-7 items-center justify-between gap-3 text-[13px]">
            {entero ? (
              <>
                <span className="text-neutral-300">
                  {t("Arrastra los bordes azules para recortar")}
                </span>
                <span className="tabular-nums text-neutral-300">{formatoTiempo(total)}</span>
              </>
            ) : (
              <>
                <span className="text-neutral-300">
                  {t("Te quedas con {trozo} de {total}", {
                    trozo: formatoTiempo(hastaMs - desdeMs),
                    total: formatoTiempo(total),
                  })}
                </span>
                <button
                  type="button"
                  onClick={() => onTramo(0, total, "las dos")}
                  className="rounded-md px-2.5 py-1 font-medium text-neutral-200 transition-colors hover:bg-white/10 hover:text-white"
                >
                  {t("Usar el vídeo entero")}
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}

function Asa({
  lado,
  ms,
  total,
  left,
  arrastrando,
  etiqueta,
  onCoger,
  onTecla,
}: {
  lado: "desde" | "hasta";
  ms: number;
  total: number;
  left: string;
  arrastrando: boolean;
  etiqueta: string;
  onCoger: () => void;
  onTecla: (e: EventoDeTecla<HTMLDivElement>) => void;
}) {
  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label={etiqueta}
      title={etiqueta}
      aria-valuemin={0}
      aria-valuemax={total}
      aria-valuenow={Math.round(ms)}
      aria-valuetext={formatoTiempo(ms)}
      data-asa={lado}
      onPointerDown={(e) => {
        // Que no llegue a la tira, que movería el cabezal en vez de coger el asa.
        e.stopPropagation();
        onCoger();
      }}
      onKeyDown={onTecla}
      style={{ left, width: ASA }}
      className={`absolute -inset-y-px z-10 flex cursor-ew-resize items-center justify-center bg-blue-500 outline-none transition-colors hover:bg-blue-400 focus-visible:bg-blue-400 before:absolute before:inset-y-0 before:-inset-x-2 before:content-[''] ${
        lado === "desde" ? "rounded-l-[5px]" : "rounded-r-[5px]"
      } ${arrastrando ? "bg-blue-400" : ""}`}
    >
      <span className="h-5 w-[3px] rounded-full bg-white/90" />
      {/* Mientras se arrastra, el tiempo exacto del corte encima del asa. */}
      {arrastrando && (
        <span className="pointer-events-none absolute -top-8 left-1/2 -translate-x-1/2 rounded-md bg-neutral-950 px-2 py-1 text-[12px] font-medium whitespace-nowrap tabular-nums text-white shadow-lg">
          {formatoTiempo(ms)}
        </span>
      )}
    </div>
  );
}
