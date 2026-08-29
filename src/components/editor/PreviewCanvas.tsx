import { useEffect, type RefObject } from "react";
import { useT } from "../../lib/i18n";

interface Props {
  /**
   * El elemento de vídeo lo maneja el editor, no esta capa.
   *
   * Reproducir tiene que salir del CLIC, no de un efecto que corre después: un `play()`
   * lanzado fuera del gesto de la persona es un `play()` que el navegador puede rechazar,
   * y encima devuelve una promesa que se pierde en silencio. Con la referencia arriba, el
   * botón llama al vídeo directamente y el estado sale de lo que el vídeo dice que hace.
   */
  videoRef: RefObject<HTMLVideoElement | null>;
  videoUrl: string | null;
  /** Fotograma que se muestra cuando no hay MP4 de referencia. */
  posterUrl: string | null;
  /** Si la grabación trae sonido. Sin esto la vista previa siempre salía muda. */
  conSonido: boolean;
  inMs: number;
  outMs: number;
  /** Cambia cuando el usuario arrastra el playhead; provoca un seek. */
  seekMs: number;
  onTime: (ms: number) => void;
  onEnded: () => void;
  onPlaying: (reproduciendo: boolean) => void;
  /** Lo que diga el vídeo cuando no puede con el archivo, para poder enseñarlo. */
  onFallo: (motivo: string) => void;
}

export function PreviewCanvas({
  videoRef,
  videoUrl,
  posterUrl,
  conSonido,
  inMs,
  outMs,
  seekMs,
  onTime,
  onEnded,
  onPlaying,
  onFallo,
}: Props) {
  const t = useT();

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const target = seekMs / 1000;
    if (Math.abs(video.currentTime - target) > 0.04) video.currentTime = target;
  }, [seekMs, videoRef]);

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
      // El estado de reproducción sale del vídeo, que es quien sabe si está sonando:
      // pintarlo desde una variable aparte es cómo se acaba con un botón que dice
      // «pausa» sobre una imagen quieta.
      onPlay={() => onPlaying(true)}
      onPause={() => onPlaying(false)}
      onError={() => {
        const codigo = videoRef.current?.error;
        onFallo(codigo?.message || t("el vídeo de vista previa no se ha podido abrir"));
      }}
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
