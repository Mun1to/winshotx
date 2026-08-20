import { useEffect, useRef } from "react";

interface Props {
  videoUrl: string | null;
  /** Fotograma que se muestra cuando no hay MP4 de referencia. */
  posterUrl: string | null;
  inMs: number;
  outMs: number;
  playing: boolean;
  /** Cambia cuando el usuario arrastra el playhead; provoca un seek. */
  seekMs: number;
  onTime: (ms: number) => void;
  onEnded: () => void;
}

export function PreviewCanvas({
  videoUrl,
  posterUrl,
  inMs,
  outMs,
  playing,
  seekMs,
  onTime,
  onEnded,
}: Props) {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const target = seekMs / 1000;
    if (Math.abs(video.currentTime - target) > 0.04) video.currentTime = target;
  }, [seekMs]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    if (playing) void video.play().catch(() => undefined);
    else video.pause();
  }, [playing]);

  if (!videoUrl) {
    return (
      <div className="flex h-full items-center justify-center">
        {posterUrl ? (
          <img src={posterUrl} alt="" className="max-h-full max-w-full object-contain" />
        ) : (
          <span className="text-xs text-neutral-500">Sin vista previa</span>
        )}
      </div>
    );
  }

  return (
    <video
      ref={videoRef}
      src={videoUrl}
      muted
      playsInline
      className="max-h-full max-w-full object-contain"
      onTimeUpdate={(e) => {
        const ms = e.currentTarget.currentTime * 1000;
        // El recorte manda: al llegar a la marca B se vuelve a la A.
        if (ms >= outMs) {
          e.currentTarget.currentTime = inMs / 1000;
          onEnded();
          return;
        }
        onTime(ms);
      }}
    />
  );
}
