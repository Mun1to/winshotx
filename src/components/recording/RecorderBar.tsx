import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Loader2, Mic, Pause, Play, Square, Trash2, Volume2 } from "lucide-react";
import { cancelRecording, pauseRecording, stopRecording } from "../../lib/ipc";
import { formatBytes, formatDuration } from "../../lib/format";
import { EVENTS, type RecordingTick } from "../../lib/types";
import { IconButton } from "../ui/IconButton";
import { useT } from "../../lib/i18n";

/** A partir de aqui el cache de fotogramas empieza a comerse el disco de verdad. */
export const CACHE_AVISO = 1_000_000_000;

/** Cuanto se queda armado el boton de descartar esperando el segundo clic. */
export const DESCARTAR_ARMADO_MS = 4_000;

const SIN_TICK: RecordingTick = {
  elapsedMs: 0,
  frames: 0,
  bytes: 0,
  paused: false,
  saving: false,
  format: "video",
  audio: false,
  microphone: false,
};

/**
 * La barra que acompanna a la grabacion: que se esta grabando, cuanto lleva, y los tres
 * botones. Pausar, parar y descartar.
 *
 * **Descartar pide dos clics.** Un solo Escape, o un clic despistado en la papelera,
 * tiraba diez minutos de grabacion sin preguntar nada. Ahora el primer clic arma el
 * boton (se pone rojo y dice «¿Descartar?») y el segundo, dentro de cuatro segundos, es
 * el que tira. Escape hace lo mismo: uno arma, dos descartan.
 *
 * **«Guardando…» se queda puesto.** El ultimo tick que manda Rust llega con `saving`, y
 * a partir de ahi la barra no vuelve atras aunque le llegue un tick viejo por el camino:
 * la grabacion ya esta parada y lo unico que va a pasar es que la barra se cierre.
 */
export function RecorderBar() {
  const t = useT();
  const [tick, setTick] = useState<RecordingTick>(SIN_TICK);
  const [guardando, setGuardando] = useState(false);
  /** Lo que se le ha pedido a Rust, sin esperar al siguiente tick para pintarlo. */
  const [pausa, setPausa] = useState(false);
  const [armado, setArmado] = useState(false);
  const desarmar = useRef<number | null>(null);

  useEffect(() => {
    const unlisten = listen<RecordingTick>(EVENTS.recordingTick, (e) => {
      setTick(e.payload);
      setPausa(e.payload.paused);
      if (e.payload.saving) setGuardando(true);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const parar = useCallback(() => {
    if (guardando) return;
    setGuardando(true);
    void stopRecording().catch(() => setGuardando(false));
  }, [guardando]);

  const pausar = useCallback(() => {
    if (guardando) return;
    const siguiente = !pausa;
    setPausa(siguiente);
    void pauseRecording(siguiente).catch(() => setPausa(!siguiente));
  }, [guardando, pausa]);

  /** Primer toque: se arma. Segundo toque, dentro del plazo: se tira la grabacion. */
  const descartar = useCallback(() => {
    if (guardando) return;
    if (armado) {
      if (desarmar.current !== null) window.clearTimeout(desarmar.current);
      void cancelRecording();
      return;
    }
    setArmado(true);
    desarmar.current = window.setTimeout(() => setArmado(false), DESCARTAR_ARMADO_MS);
  }, [guardando, armado]);

  useEffect(
    () => () => {
      if (desarmar.current !== null) window.clearTimeout(desarmar.current);
    },
    [],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") descartar();
      // Espacio pausa y reanuda, como en cualquier reproductor.
      if (e.key === " " && e.target === document.body) {
        e.preventDefault();
        pausar();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [descartar, pausar]);

  const cacheGrande = tick.bytes > CACHE_AVISO;

  return (
    <div
      data-tauri-drag-region
      className="flex h-full w-full items-center gap-2 overflow-hidden bg-[#161618] px-3 whitespace-nowrap"
    >
      <span data-tauri-drag-region className="flex items-center gap-2">
        {guardando ? (
          <Loader2 className="size-3.5 shrink-0 animate-spin text-neutral-400" aria-hidden="true" />
        ) : (
          <span
            className={`size-2.5 shrink-0 rounded-full ${
              pausa ? "bg-amber-400" : "animate-pulse bg-red-500"
            }`}
            aria-hidden="true"
          />
        )}
        <span
          role="timer"
          aria-label={t("Tiempo grabado")}
          className={`font-mono text-[15px] leading-none font-medium tabular-nums ${
            guardando ? "text-neutral-400" : "text-white"
          }`}
        >
          {formatDuration(tick.elapsedMs)}
        </span>
      </span>

      {/* Que se esta grabando: el formato, y el sonido solo si entra de verdad. */}
      {guardando ? (
        <span className="text-[11px] leading-none text-neutral-400">{t("Guardando…")}</span>
      ) : pausa ? (
        <span className="text-[11px] leading-none text-amber-400">{t("en pausa")}</span>
      ) : (
        <span data-tauri-drag-region className="flex items-center gap-1.5">
          <span
            title={tick.format === "gif" ? t("GIF animado") : t("Vídeo MP4")}
            className="rounded-[5px] border border-white/10 px-1.5 py-0.5 text-[10px] leading-none font-semibold tracking-wide text-neutral-300"
          >
            {tick.format === "gif" ? "GIF" : "MP4"}
          </span>
          {tick.audio && (
            <span title={t("Sonido del sistema grabándose")} aria-label={t("Sonido del sistema grabándose")} role="img">
              <Volume2 className="size-3.5 text-neutral-400" aria-hidden="true" />
            </span>
          )}
          {tick.microphone && (
            <span title={t("Micrófono grabándose")} aria-label={t("Micrófono grabándose")} role="img">
              <Mic className="size-3.5 text-neutral-400" aria-hidden="true" />
            </span>
          )}
          {/* El tamanno solo cuando importa: por encima del aviso, en ambar. */}
          {cacheGrande && (
            <span
              title={t("La caché sin pérdida está ocupando mucho disco: para y exporta")}
              className="text-[11px] leading-none tabular-nums text-amber-400"
            >
              {formatBytes(tick.bytes)}
            </span>
          )}
        </span>
      )}

      <span className="ml-auto flex items-center gap-1">
        <IconButton
          icon={pausa ? Play : Pause}
          label={pausa ? t("Reanudar") : t("Pausar")}
          shortcut={t("Espacio")}
          onClick={pausar}
          disabled={guardando}
        />
        <IconButton
          icon={Square}
          label={t("Parar")}
          accent
          showLabel={!armado}
          disabled={guardando}
          onClick={parar}
        />
        <button
          type="button"
          onClick={descartar}
          disabled={guardando}
          aria-label={armado ? t("¿Descartar?") : t("Descartar")}
          title={armado ? t("Otra vez para tirar la grabación") : `${t("Descartar")} · Esc`}
          className={`flex h-9 shrink-0 items-center gap-1.5 rounded-lg px-2.5 text-sm font-medium whitespace-nowrap transition-colors duration-100 disabled:pointer-events-none disabled:opacity-40 ${
            armado
              ? "bg-red-500 text-white hover:bg-red-400"
              : "text-red-400 hover:bg-red-500/15 hover:text-red-300"
          }`}
        >
          <Trash2 className="size-[18px] shrink-0" strokeWidth={1.9} />
          {armado && <span className="pr-0.5">{t("¿Descartar?")}</span>}
        </button>
      </span>
    </div>
  );
}
