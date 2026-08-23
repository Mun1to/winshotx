import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Clipboard, Cpu, FolderOpen, Link2, Save, Sparkles, Unlink2, Zap } from "lucide-react";
import { exportMedia, pickDirectory, revealInExplorer } from "../../lib/ipc";
import { formatBytes } from "../../lib/format";
import {
  EVENTS,
  type ExportFormat,
  type ExportProgress,
  type ExportResult,
  type SessionInfo,
} from "../../lib/types";
import { NumberField } from "../ui/NumberField";
import { Slider } from "../ui/Slider";
import { Toggle } from "../ui/Toggle";

const FORMATS: { id: ExportFormat; label: string; hint: string }[] = [
  { id: "gif", label: "GIF", hint: "bucle, sin audio" },
  { id: "mp4", label: "MP4", hint: "H.264 por hardware" },
  { id: "png", label: "PNG", hint: "el fotograma actual" },
];

interface Props {
  session: SessionInfo;
  inIndex: number;
  outIndex: number;
  currentIndex: number;
  fpsMax: number;
  hasFfmpeg: boolean;
  saveDirectory: string;
}

export function ExportPanel({
  session,
  inIndex,
  outIndex,
  currentIndex,
  fpsMax,
  hasFfmpeg,
  saveDirectory,
}: Props) {
  const [format, setFormat] = useState<ExportFormat>(() => {
    if (session.format === "gif") return "gif";
    // Una sesion de un solo fotograma es una imagen: exportarla a MP4 no tiene sentido.
    if (session.format === "still" || session.frameCount <= 1) return "png";
    return "mp4";
  });
  const [useFfmpeg, setUseFfmpeg] = useState(false);
  const [quality, setQuality] = useState(80);
  const [fps, setFps] = useState(Math.min(session.fps, 30));
  const [width, setWidth] = useState(session.region.width);
  const [height, setHeight] = useState(session.region.height);
  const [locked, setLocked] = useState(true);
  const [audio, setAudio] = useState(session.hasAudio);
  const [loop, setLoop] = useState(true);
  const [directory, setDirectory] = useState(saveDirectory);
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [result, setResult] = useState<ExportResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const aspect = session.region.width / session.region.height;

  useEffect(() => {
    const unlisten = listen<ExportProgress>(EVENTS.exportProgress, (e) => {
      setProgress(e.payload.stage === "done" ? null : e.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const estimate = useMemo(() => {
    const frames = Math.max(1, outIndex - inIndex + 1);
    const seconds = frames / Math.max(1, session.fps);
    if (format === "png") return width * height * 3 * 0.35;
    if (format === "gif") return width * height * (quality / 100) * 0.12 * fps * seconds;
    const bitrate = 1_000_000 + (quality / 100) * 11_000_000;
    return (bitrate / 8) * seconds;
  }, [format, width, height, quality, fps, inIndex, outIndex, session.fps]);

  const run = async (copyToClipboard: boolean) => {
    // Sin esto, dos pulsaciones seguidas lanzan dos exportaciones a la vez.
    if (progress !== null) return;
    setError(null);
    setResult(null);
    setProgress({ stage: "reading", done: 0, total: 1 });
    try {
      const res = await exportMedia({
        sessionId: session.id,
        format,
        engine: useFfmpeg && hasFfmpeg ? "ffmpeg" : "native",
        from: format === "png" ? currentIndex : inIndex,
        to: format === "png" ? currentIndex : outIndex,
        width: Math.round(width),
        height: Math.round(height),
        fps,
        quality,
        audio: audio && format === "mp4",
        loop,
        destination: directory || null,
        copyToClipboard,
      });
      setResult(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setProgress(null);
    }
  };

  // Ctrl+S exporta sin ir al raton, igual que en el overlay. El ref evita volver
  // a registrar el listener en cada tecleo del panel.
  const runRef = useRef(run);
  runRef.current = run;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== "s") return;
      e.preventDefault();
      void runRef.current(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const setW = (value: number) => {
    setWidth(value);
    if (locked) setHeight(Math.max(2, Math.round(value / aspect)));
  };
  const setH = (value: number) => {
    setHeight(value);
    if (locked) setWidth(Math.max(2, Math.round(value * aspect)));
  };

  return (
    <aside className="flex w-[292px] shrink-0 flex-col gap-4 overflow-y-auto border-l border-white/8 bg-black/20 p-4">
      <div>
        <span className="mb-2 block text-xs font-semibold text-neutral-300">Formato</span>
        <div className="grid grid-cols-3 gap-1 rounded-lg bg-black/40 p-1">
          {FORMATS.map((f) => (
            <button
              key={f.id}
              type="button"
              onClick={() => setFormat(f.id)}
              title={f.hint}
              className={`rounded-md py-1.5 text-xs font-medium whitespace-nowrap transition-colors ${
                format === f.id
                  ? "bg-white/15 text-white shadow-sm"
                  : "text-neutral-400 hover:text-white"
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      {format !== "png" && (
        <>
          <Slider
            label="Calidad"
            hint={`${quality}%`}
            min={10}
            max={100}
            value={quality}
            onChange={setQuality}
          />
          <Slider
            label="Fotogramas por segundo"
            hint={`${fps} fps`}
            min={5}
            max={fpsMax}
            value={fps}
            onChange={setFps}
          />
        </>
      )}

      <div>
        <span className="mb-2 block text-xs font-semibold text-neutral-300">Dimensiones</span>
        <div className="flex items-end gap-2">
          <NumberField label="Ancho" value={width} onChange={setW} suffix="px" />
          <button
            type="button"
            onClick={() => setLocked((l) => !l)}
            title={locked ? "Proporción bloqueada" : "Proporción libre"}
            className={`mb-1.5 flex size-8 shrink-0 items-center justify-center rounded-md transition-colors ${
              locked ? "bg-white/10 text-blue-400" : "text-neutral-500 hover:text-white"
            }`}
          >
            {locked ? <Link2 className="size-4" /> : <Unlink2 className="size-4" />}
          </button>
          <NumberField label="Alto" value={height} onChange={setH} suffix="px" />
        </div>
        <div className="mt-1.5 flex gap-1">
          {[100, 75, 50].map((pct) => (
            <button
              key={pct}
              type="button"
              onClick={() => {
                setWidth(Math.round((session.region.width * pct) / 100));
                setHeight(Math.round((session.region.height * pct) / 100));
              }}
              className="rounded-md bg-white/5 px-2 py-1 text-[11px] text-neutral-400 transition-colors hover:bg-white/10 hover:text-white"
            >
              {pct}%
            </button>
          ))}
        </div>
      </div>

      <div className="space-y-0.5 border-t border-white/8 pt-3">
        {format === "mp4" && (
          <Toggle
            checked={audio}
            onChange={setAudio}
            label="Audio del sistema"
            hint={session.hasAudio ? undefined : "esta grabación se hizo sin audio"}
          />
        )}
        {format === "gif" && <Toggle checked={loop} onChange={setLoop} label="Bucle infinito" />}
        {hasFfmpeg && format !== "png" && (
          <Toggle
            checked={useFfmpeg}
            onChange={setUseFfmpeg}
            label="Motor FFmpeg"
            hint="calidad máxima, más lento"
          />
        )}
      </div>

      <button
        type="button"
        onClick={() => void pickDirectory().then((dir) => dir && setDirectory(dir))}
        className="flex items-center gap-2 rounded-lg border border-white/8 bg-black/30 px-2.5 py-2 text-left transition-colors hover:border-white/20"
      >
        <FolderOpen className="size-4 shrink-0 text-neutral-400" />
        <span className="truncate text-[11px] text-neutral-400" title={directory}>
          {directory || "Elegir carpeta…"}
        </span>
      </button>

      <div className="mt-auto space-y-2 pt-2">
        <div className="flex items-center justify-between text-[11px] text-neutral-500">
          <span className="flex items-center gap-1.5">
            {useFfmpeg && hasFfmpeg ? (
              <Cpu className="size-3.5" />
            ) : (
              <Zap className="size-3.5 text-amber-400" />
            )}
            {useFfmpeg && hasFfmpeg ? "FFmpeg" : "Nativo"}
          </span>
          <span className="tabular-nums">≈ {formatBytes(estimate)}</span>
        </div>

        {progress && (
          <div className="h-1 overflow-hidden rounded-full bg-white/10">
            <div
              className="h-full bg-blue-500 transition-[width] duration-150"
              style={{
                width: `${Math.round((progress.done / Math.max(1, progress.total)) * 100)}%`,
              }}
            />
          </div>
        )}

        <div className="flex gap-2">
          <button
            type="button"
            disabled={progress !== null}
            onClick={() => void run(false)}
            title="Exportar a la carpeta elegida (Ctrl+S)"
            className="flex h-9 flex-1 items-center justify-center gap-1.5 rounded-lg bg-blue-500 text-sm font-semibold whitespace-nowrap text-white transition-colors hover:bg-blue-400 disabled:opacity-50"
          >
            <Save className="size-4" /> Guardar
          </button>
          <button
            type="button"
            disabled={progress !== null}
            onClick={() => void run(true)}
            title="Exportar y copiar al portapapeles"
            className="flex h-9 items-center justify-center rounded-lg border border-white/10 px-3 text-neutral-300 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-50"
          >
            <Clipboard className="size-4" />
          </button>
        </div>

        {result && (
          <button
            type="button"
            onClick={() => void revealInExplorer(result.path)}
            className="flex w-full items-center gap-1.5 rounded-lg bg-emerald-500/10 px-2.5 py-2 text-left text-[11px] text-emerald-300 transition-colors hover:bg-emerald-500/20"
          >
            <Sparkles className="size-3.5 shrink-0" />
            <span className="truncate">
              {formatBytes(result.bytes)} · {result.copied ? "copiado · " : ""}abrir carpeta
            </span>
          </button>
        )}

        {error && (
          <p className="rounded-lg bg-red-500/10 px-2.5 py-2 text-[11px] text-red-300">{error}</p>
        )}
      </div>
    </aside>
  );
}
