import { useCallback, useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Pause, Play, Scissors } from "lucide-react";
import {
  discardSession,
  ffmpegAvailable,
  frameImage,
  getSettings,
  sessionFrames,
  sessionInfo,
} from "../../lib/ipc";
import { clamp, formatTimecode } from "../../lib/format";
import type { FrameMeta, SessionInfo, Settings } from "../../lib/types";
import { ExportPanel } from "./ExportPanel";
import { FrameStrip } from "./FrameStrip";
import { PreviewCanvas } from "./PreviewCanvas";
import { TitleBar } from "./TitleBar";

export function EditorApp({ sessionId }: { sessionId: string }) {
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [frames, setFrames] = useState<FrameMeta[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [hasFfmpeg, setHasFfmpeg] = useState(false);
  const [inIndex, setInIndex] = useState(0);
  const [outIndex, setOutIndex] = useState(0);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [seekMs, setSeekMs] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!sessionId) {
      setError("Falta el identificador de sesión");
      return;
    }
    Promise.all([
      sessionInfo(sessionId),
      sessionFrames(sessionId),
      getSettings(),
      ffmpegAvailable(),
    ])
      .then(([info, frameList, config, ffmpeg]) => {
        setSession(info);
        setFrames(frameList);
        setSettings(config);
        setHasFfmpeg(ffmpeg);
        setOutIndex(Math.max(0, frameList.length - 1));
      })
      .catch((e) => setError(String(e)));
  }, [sessionId]);

  /** El marcador A nunca puede caer fuera de la tira ni pasarse del B. */
  const markIn = useCallback(
    (index: number) => setInIndex(clamp(index, 0, outIndex)),
    [outIndex],
  );

  /** Y el B, ni por debajo del A ni mas alla del ultimo fotograma. */
  const markOut = useCallback(
    (index: number) => setOutIndex(clamp(index, inIndex, Math.max(0, frames.length - 1))),
    [inIndex, frames.length],
  );

  const scrub = useCallback(
    (index: number) => {
      setCurrentIndex(index);
      setSeekMs(frames[index]?.timestampMs ?? 0);
      setPlaying(false);
    },
    [frames],
  );

  const togglePlay = useCallback(() => {
    setPlaying((p) => {
      if (!p && currentIndex >= outIndex) {
        setCurrentIndex(inIndex);
        setSeekMs(frames[inIndex]?.timestampMs ?? 0);
      }
      return !p;
    });
  }, [currentIndex, outIndex, inIndex, frames]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.tagName === "INPUT") return;
      const key = e.key.toLowerCase();
      if (e.key === " ") {
        e.preventDefault();
        togglePlay();
      } else if (key === "i") {
        markIn(currentIndex);
      } else if (key === "o") {
        markOut(currentIndex);
      } else if (e.key === "ArrowLeft") {
        scrub(Math.max(0, currentIndex - 1));
      } else if (e.key === "ArrowRight") {
        scrub(Math.min(frames.length - 1, currentIndex + 1));
      } else if (e.key === "Escape") {
        void getCurrentWindow().close();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [currentIndex, frames.length, togglePlay, scrub, markIn, markOut]);

  const videoUrl = useMemo(
    () => (session?.mp4Path ? convertFileSrc(session.mp4Path) : null),
    [session],
  );
  // Sin MP4 la vista previa es una imagen: la miniatura de 80 px se veria borrosa,
  // asi que se pide el fotograma entero y se sustituye en cuanto llega.
  const [stillPath, setStillPath] = useState<string | null>(null);
  useEffect(() => {
    if (!session || session.mp4Path || !frames[currentIndex]) return;
    let cancelled = false;
    void frameImage(sessionId, currentIndex)
      .then((path) => {
        if (!cancelled) setStillPath(path);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [session, frames, currentIndex, sessionId]);

  const posterUrl = useMemo(() => {
    if (stillPath) return convertFileSrc(stillPath);
    return frames[currentIndex] ? convertFileSrc(frames[currentIndex].thumbPath) : null;
  }, [stillPath, frames, currentIndex]);

  const onTime = useCallback(
    (ms: number) => {
      // El video manda el tiempo; se traduce al frame mas cercano de la tira.
      let index = currentIndex;
      while (index + 1 < frames.length && frames[index + 1].timestampMs <= ms) index++;
      while (index > 0 && frames[index].timestampMs > ms) index--;
      if (index !== currentIndex) setCurrentIndex(index);
    },
    [frames, currentIndex],
  );

  if (error) {
    return (
      <div className="flex h-full items-center justify-center bg-[#161618] text-sm text-red-300">
        {error}
      </div>
    );
  }

  if (!session || !settings || frames.length === 0) {
    return (
      <div className="flex h-full items-center justify-center bg-[#161618] text-sm text-neutral-500">
        Preparando la sesión…
      </div>
    );
  }

  const keptMs =
    (frames[outIndex]?.timestampMs ?? 0) +
    (frames[outIndex]?.durationMs ?? 0) -
    (frames[inIndex]?.timestampMs ?? 0);

  return (
    <div className="flex h-full flex-col overflow-hidden bg-[#161618]">
      <TitleBar
        title="Editor"
        subtitle={`${session.region.width} × ${session.region.height} · ${formatTimecode(keptMs)}`}
        onClose={() => {
          void discardSession(session.id);
          void getCurrentWindow().close();
        }}
      />

      <div className="flex min-h-0 flex-1">
        <main className="flex min-w-0 flex-1 flex-col">
          <div className="relative flex min-h-0 flex-1 items-center justify-center bg-[repeating-conic-gradient(#1c1c1c_0%_25%,#242424_0%_50%)] bg-[length:20px_20px] p-4">
            <PreviewCanvas
              videoUrl={videoUrl}
              posterUrl={posterUrl}
              inMs={frames[inIndex]?.timestampMs ?? 0}
              outMs={(frames[outIndex]?.timestampMs ?? 0) + (frames[outIndex]?.durationMs ?? 0)}
              playing={playing}
              seekMs={seekMs}
              onTime={onTime}
              onEnded={() => setCurrentIndex(inIndex)}
            />
            <div className="absolute bottom-3 left-1/2 flex -translate-x-1/2 items-center gap-2 rounded-full border border-white/10 bg-neutral-900/85 px-2 py-1.5 shadow-xl backdrop-blur-md">
              <button
                type="button"
                onClick={togglePlay}
                aria-label={playing ? "Pausar" : "Reproducir"}
                className="flex size-7 items-center justify-center rounded-full bg-white/10 text-white transition-colors hover:bg-white/20"
              >
                {playing ? <Pause className="size-3.5" /> : <Play className="size-3.5 pl-px" />}
              </button>
              <span className="px-1 font-mono text-[11px] tabular-nums text-neutral-300">
                {formatTimecode(frames[currentIndex]?.timestampMs ?? 0)}
              </span>
              <span className="h-4 w-px bg-white/10" />
              <button
                type="button"
                onClick={() => markIn(currentIndex)}
                className="flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] text-neutral-300 transition-colors hover:bg-white/10 hover:text-white"
                title="Marcar inicio (I)"
              >
                <Scissors className="size-3" /> A
              </button>
              <button
                type="button"
                onClick={() => markOut(currentIndex)}
                className="flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] text-neutral-300 transition-colors hover:bg-white/10 hover:text-white"
                title="Marcar final (O)"
              >
                <Scissors className="size-3 -scale-x-100" /> B
              </button>
            </div>
          </div>

          <FrameStrip
            frames={frames}
            inIndex={inIndex}
            outIndex={outIndex}
            currentIndex={currentIndex}
            onChangeIn={markIn}
            onChangeOut={markOut}
            onScrub={scrub}
          />
        </main>

        <ExportPanel
          session={session}
          inIndex={inIndex}
          outIndex={outIndex}
          currentIndex={currentIndex}
          fpsMax={Math.max(15, session.fps)}
          hasFfmpeg={hasFfmpeg}
          saveDirectory={settings.saveDirectory}
        />
      </div>
    </div>
  );
}
