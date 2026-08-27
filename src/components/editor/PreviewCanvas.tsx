import { useEffect, useRef } from "react";
import { useT } from "../../lib/i18n";

interface Props {
  videoUrl: string | null;
  /** Fotograma que se muestra cuando no hay MP4 de referencia. */
  posterUrl: string | null;
  /** Si la grabación trae sonido. Sin esto la vista previa siempre salía muda. */
  conSonido: boolean;
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
  conSonido,
  inMs,
  outMs,
  playing,
  seekMs,
  onTime,
  onEnded,
}: Props) {
  const t = useT();
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

  // La caja de fuera ya tiene la proporcion de la captura, asi que la imagen y el video
  // la llenan entera. El `object-contain` se queda por si acaso: si algun dia la
  // proporcion no cuadrara, es preferible una franja a una imagen deformada.
  if (!videoUrl) {
    return posterUrl ? (
      <img src={posterUrl} alt="" className="size-full object-contain" />
    ) : (
      <div className="flex size-full items-center justify-center">
        <span className="text-xs text-neutral-500">{t("Sin vista previa")}</span>
      </div>
    );
  }

  return (
    <video
      ref={videoRef}
      src={videoUrl}
      // Solo se silencia si no hay nada que oír. Estaba fijo en `muted` de cuando el
      // audio del sistema no existía, así que una grabación con sonido se veía muda y
      // parecía que el sonido no se había grabado.
      muted={!conSonido}
      playsInline
      className="size-full object-contain"
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
