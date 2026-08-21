import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Pause, Play, Square, Trash2 } from "lucide-react";
import { cancelRecording, pauseRecording, stopRecording } from "../../lib/ipc";
import { formatBytes, formatDuration } from "../../lib/format";
import { EVENTS, type RecordingTick } from "../../lib/types";
import { IconButton } from "../ui/IconButton";

/** A partir de aqui el cache de fotogramas empieza a comerse el disco de verdad. */
const CACHE_AVISO = 1_000_000_000;

/** Barra minima que acompanna a la grabacion: tiempo, tamanno y los tres botones. */
export function RecorderBar() {
  const [tick, setTick] = useState<RecordingTick>({
    elapsedMs: 0,
    frames: 0,
    bytes: 0,
    paused: false,
  });
  const [stopping, setStopping] = useState(false);

  useEffect(() => {
    const unlisten = listen<RecordingTick>(EVENTS.recordingTick, (e) => setTick(e.payload));
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void cancelRecording();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div
      data-tauri-drag-region
      className="flex h-full w-full items-center gap-2.5 overflow-hidden bg-[#161618] px-3 whitespace-nowrap"
    >
      <span data-tauri-drag-region className="flex items-center gap-2">
        <span
          className={`size-2.5 shrink-0 rounded-full ${
            tick.paused ? "bg-amber-400" : "animate-pulse bg-red-500"
          }`}
        />
        <span className="font-mono text-[15px] leading-none font-medium tabular-nums text-white">
          {formatDuration(tick.elapsedMs)}
        </span>
      </span>

      {/* Los contadores solo aparecen cuando hay algo que contar. */}
      {tick.frames > 0 && (
        <span
          data-tauri-drag-region
          title={
            tick.bytes > CACHE_AVISO
              ? "El cache sin perdida esta ocupando mucho disco: para y exporta"
              : undefined
          }
          className={`text-[11px] leading-none tabular-nums ${
            tick.bytes > CACHE_AVISO ? "text-amber-400" : "text-neutral-500"
          }`}
        >
          {tick.frames} f · {formatBytes(tick.bytes)}
        </span>
      )}

      <span className="ml-auto flex items-center gap-1">
        <IconButton
          icon={tick.paused ? Play : Pause}
          label={tick.paused ? "Reanudar" : "Pausar"}
          onClick={() => void pauseRecording(!tick.paused)}
          disabled={stopping}
        />
        <IconButton
          icon={Square}
          label={stopping ? "Guardando…" : "Parar"}
          accent
          showLabel
          disabled={stopping}
          onClick={() => {
            setStopping(true);
            void stopRecording().finally(() => setStopping(false));
          }}
        />
        <IconButton
          icon={Trash2}
          label="Descartar"
          shortcut="Esc"
          danger
          disabled={stopping}
          onClick={() => void cancelRecording()}
        />
      </span>
    </div>
  );
}
