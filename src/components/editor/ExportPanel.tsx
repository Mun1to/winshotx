import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Clipboard, Cpu, FolderOpen, Link2, Save, Sparkles, Unlink2, Zap } from "lucide-react";
import { exportMedia, pickDirectory, revealInExplorer } from "../../lib/ipc";
import { formatBytes } from "../../lib/format";
import {
  EVENTS,
  type ExportFormat,
  type Background,
  type ExportProgress,
  type ExportResult,
  type SessionInfo,
} from "../../lib/types";
import { NumberField } from "../ui/NumberField";
import { Slider } from "../ui/Slider";
import { Toggle } from "../ui/Toggle";
import { useT } from "../../lib/i18n";
import { medida as medidaDelRecorte, type Recorte } from "../../lib/recorte";
import type { Anotacion } from "../../lib/anotaciones";

/**
 * Los fondos del marco, con el color con el que se pintan en el propio botón.
 *
 * Son cinco y no una rueda de color: con dieciséis millones a elegir, la pregunta deja de
 * ser «cuál de estos» y pasa a ser «cuál de todos», que es justo lo que hace que alguien
 * cierre el panel sin exportar nada.
 */
const FONDOS: { id: Background; label: string; muestra: string }[] = [
  { id: "blanco", label: "Blanco", muestra: "#ffffff" },
  { id: "negro", label: "Negro", muestra: "#121214" },
  { id: "gris", label: "Gris", muestra: "#e8e8ea" },
  { id: "atardecer", label: "Atardecer", muestra: "linear-gradient(135deg,#5874f5,#a855d9)" },
  { id: "menta", label: "Menta", muestra: "linear-gradient(135deg,#22c598,#3884e8)" },
];

const FORMATS: { id: ExportFormat; label: string; hint: string }[] = [
  { id: "gif", label: "GIF", hint: "bucle, sin audio" },
  { id: "mp4", label: "MP4", hint: "H.264 por hardware" },
  { id: "png", label: "PNG", hint: "el fotograma actual, sin perder nada" },
  { id: "jpg", label: "JPG", hint: "el fotograma actual, mucho más ligero" },
];

/** Los dos formatos que sacan UN fotograma, no un trozo de grabación. */
const esUnaFoto = (formato: ExportFormat) => formato === "png" || formato === "jpg";

interface Props {
  /** Lo dibujado encima en el editor, que Rust pinta sobre cada fotograma. */
  anotaciones: Anotacion[];
  /** El trozo que se exporta, de 0 a 1. Sin marco puesto sale la captura entera. */
  recorte: Recorte | null;
  session: SessionInfo;
  inIndex: number;
  outIndex: number;
  currentIndex: number;
  fpsMax: number;
  hasFfmpeg: boolean;
  saveDirectory: string;
}

export function ExportPanel({
  anotaciones,
  recorte,
  session,
  inIndex,
  outIndex,
  currentIndex,
  fpsMax,
  hasFfmpeg,
  saveDirectory,
}: Props) {
  const t = useT();
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
  /**
   * Cuánto se acerca la cámara a cada clic. 1 es no acercarse.
   *
   * **Apagado de fábrica y a propósito.** Grabando una ventana pequeña no hace falta y
   * marea, y quien no lo espere se encuentra un vídeo que se mueve solo. Los clics quedaron
   * anotados al grabar, así que encenderlo aquí no obliga a repetir nada.
   */
  const [zoom, setZoom] = useState(1);
  /** Aire alrededor de la captura. Cero significa sin marco, que es lo de siempre. */
  const [margen, setMargen] = useState(0);
  const [fondo, setFondo] = useState<Background>("blanco");
  const [sombra, setSombra] = useState(true);
  const [loop, setLoop] = useState(true);
  const [directory, setDirectory] = useState(saveDirectory);
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [result, setResult] = useState<ExportResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  /**
   * Lo que mide de verdad lo que se va a exportar, con el recorte ya aplicado.
   *
   * Todo lo de este panel se mide sobre esto y no sobre la region grabada: quien recorta
   * un trozo y ve «1920 x 1200» en las dimensiones ve el tamanno de algo que ya no existe.
   */
  const fuente = useMemo(
    () =>
      recorte
        ? medidaDelRecorte(recorte, session.region.width, session.region.height)
        : { width: session.region.width, height: session.region.height },
    [recorte, session.region.width, session.region.height],
  );

  const aspect = fuente.width / fuente.height;

  /**
   * Poner o quitar el recorte devuelve las dimensiones al tamanno nuevo.
   *
   * Es lo que espera cualquiera, y ademas evita el caso raro: conservar el ancho anterior
   * dejaria un trozo pequenno estirado al tamanno de la captura entera sin que nadie lo
   * haya pedido.
   */
  useEffect(() => {
    setWidth(fuente.width);
    setHeight(fuente.height);
  }, [fuente.width, fuente.height]);

  useEffect(() => {
    const unlisten = listen<ExportProgress>(EVENTS.exportProgress, (e) => {
      // El "done" del backend llega antes de copiar al portapapeles y antes de que el
      // comando devuelva: si soltara aqui la guarda, un segundo clic arrancaria otra
      // exportacion sobre la misma sesion. Quien la suelta es el finally de run().
      if (e.payload.stage !== "done") setProgress(e.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const estimate = useMemo(() => {
    const frames = Math.max(1, outIndex - inIndex + 1);
    const seconds = frames / Math.max(1, session.fps);
    // El marco crece por fuera, así que el archivo también: la estimación cuenta con él.
    const w = width + margen * 2;
    const h = height + margen * 2;
    if (format === "png") return w * h * 3 * 0.35;
    // Medido sobre capturas de verdad: un JPEG de calidad 85 se queda en torno a
    // 0,13 bytes por pixel, que es donde sale este factor.
    if (format === "jpg") return w * h * (quality / 100) * 0.15;
    if (format === "gif") return w * h * (quality / 100) * 0.12 * fps * seconds;
    const bitrate = 1_000_000 + (quality / 100) * 11_000_000;
    return (bitrate / 8) * seconds;
  }, [format, width, height, margen, quality, fps, inIndex, outIndex, session.fps]);

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
        from: esUnaFoto(format) ? currentIndex : inIndex,
        to: esUnaFoto(format) ? currentIndex : outIndex,
        width: Math.round(width),
        height: Math.round(height),
        fps,
        quality,
        audio: audio && format === "mp4",
        // Una foto no tiene zoom: la cámara solo se mueve con el tiempo pasando.
        zoom: esUnaFoto(format) ? 1 : zoom,
        loop,
        margin: margen,
        background: fondo,
        shadow: sombra,
        annotations: anotaciones,
        crop: recorte,
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
        <span className="mb-2 block text-xs font-semibold text-neutral-300">{t("Formato")}</span>
        <div className="grid grid-cols-4 gap-1 rounded-lg bg-black/40 p-1">
          {FORMATS.map((f) => (
            <button
              key={f.id}
              type="button"
              onClick={() => setFormat(f.id)}
              title={t(f.hint)}
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
        <Slider
          label={t("Calidad")}
          hint={`${quality}%`}
          min={10}
          max={100}
          value={quality}
          onChange={setQuality}
        />
      )}

      {/* Una foto no tiene fotogramas por segundo. */}
      {!esUnaFoto(format) && (
        <Slider
          label={t("Fotogramas por segundo")}
          hint={`${fps} fps`}
          min={5}
          max={fpsMax}
          value={fps}
          onChange={setFps}
        />
      )}

      <div>
        <span className="mb-2 block text-xs font-semibold text-neutral-300">{t("Dimensiones")}</span>
        <div className="flex items-end gap-2">
          <NumberField label={t("Ancho")} value={width} onChange={setW} suffix="px" />
          <button
            type="button"
            onClick={() => setLocked((l) => !l)}
            title={locked ? t("Proporción bloqueada") : t("Proporción libre")}
            className={`mb-1.5 flex size-8 shrink-0 items-center justify-center rounded-md transition-colors ${
              locked ? "bg-white/10 text-blue-400" : "text-neutral-500 hover:text-white"
            }`}
          >
            {locked ? <Link2 className="size-4" /> : <Unlink2 className="size-4" />}
          </button>
          <NumberField label={t("Alto")} value={height} onChange={setH} suffix="px" />
        </div>
        <div className="mt-1.5 flex gap-1">
          {[100, 75, 50].map((pct) => (
            <button
              key={pct}
              type="button"
              onClick={() => {
                // Sobre lo que se va a exportar, no sobre la captura entera: con un
                // recorte puesto, el 50 % de la captura estiraria el trozo al doble.
                setWidth(Math.round((fuente.width * pct) / 100));
                setHeight(Math.round((fuente.height * pct) / 100));
              }}
              className="rounded-md bg-white/5 px-2 py-1 text-[11px] text-neutral-400 transition-colors hover:bg-white/10 hover:text-white"
            >
              {pct}%
            </button>
          ))}
        </div>
      </div>

      {/* El zoom solo sale con vídeo y solo si hubo clics: un interruptor que no puede
          hacer nada es peor que no tenerlo. */}
      {!esUnaFoto(format) && session.hasClicks && (
        <div className="border-t border-white/8 pt-3">
          <Slider
            label={t("Acercarse a los clics")}
            hint={zoom <= 1.05 ? t("sin zoom") : `${zoom.toFixed(1)}×`}
            min={1}
            max={3}
            step={0.1}
            value={zoom}
            onChange={setZoom}
          />
          <p className="mt-1 text-[11px] leading-snug text-neutral-500">
            {t("La cámara se acerca sola a donde pulsaste y vuelve. Se decide aquí, no al grabar.")}
          </p>
        </div>
      )}

      <div className="border-t border-white/8 pt-3">
        <Slider
          label={t("Aire alrededor")}
          hint={margen === 0 ? t("sin marco") : `${margen} px`}
          min={0}
          max={160}
          value={margen}
          onChange={setMargen}
        />
        {/* Los fondos solo aparecen cuando hay aire que pintar: cinco botones de color
            para elegir el fondo de un marco que no existe son cinco botones de ruido. */}
        {margen > 0 && (
          <>
            <div className="mt-2 flex gap-1.5">
              {FONDOS.map((f) => (
                <button
                  key={f.id}
                  type="button"
                  onClick={() => setFondo(f.id)}
                  title={t(f.label)}
                  aria-label={t(f.label)}
                  aria-pressed={fondo === f.id}
                  style={{ background: f.muestra }}
                  className={`size-7 rounded-lg border-2 transition-colors ${
                    fondo === f.id ? "border-blue-500" : "border-white/15 hover:border-white/40"
                  }`}
                />
              ))}
            </div>
            <div className="mt-1.5">
              <Toggle checked={sombra} onChange={setSombra} label={t("Sombra")} />
            </div>
          </>
        )}
      </div>

      <div className="space-y-0.5 border-t border-white/8 pt-3">
        {format === "mp4" && (
          <Toggle
            checked={audio}
            onChange={setAudio}
            label={t("Audio del sistema")}
            hint={session.hasAudio ? undefined : t("esta grabación se hizo sin audio")}
          />
        )}
        {format === "gif" && <Toggle checked={loop} onChange={setLoop} label={t("Bucle infinito")} />}
        {hasFfmpeg && !esUnaFoto(format) && (
          <Toggle
            checked={useFfmpeg}
            onChange={setUseFfmpeg}
            label={t("Motor FFmpeg")}
            hint={t("calidad máxima, más lento")}
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
          {directory || t("Elegir carpeta…")}
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
            {useFfmpeg && hasFfmpeg ? "FFmpeg" : t("Nativo")}
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
            title={t("Exportar a la carpeta elegida (Ctrl+S)")}
            className="flex h-9 flex-1 items-center justify-center gap-1.5 rounded-lg bg-blue-500 text-sm font-semibold whitespace-nowrap text-white transition-colors hover:bg-blue-400 disabled:opacity-50"
          >
            <Save className="size-4" /> {t("Guardar")}
          </button>
          <button
            type="button"
            disabled={progress !== null}
            onClick={() => void run(true)}
            title={t("Exportar y copiar al portapapeles")}
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
              {formatBytes(result.bytes)} · {result.copied ? t("copiado · ") : ""}
              {t("abrir carpeta")}
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
