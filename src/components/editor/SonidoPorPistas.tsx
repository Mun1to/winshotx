import { useEffect, useMemo, useRef, type RefObject } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { VolumenDePista } from "../../lib/contrato";
import { seguirAlVideo, volumenEnVista, type EstadoDePista } from "../../lib/pistas";

interface Props {
  /** El vídeo de la vista previa: las pistas van a su compás, también al saltar un corte. */
  videoRef: RefObject<HTMLVideoElement | null>;
  /** Las rutas de los WAV de la sesión; la que no viene no suena. */
  sonidoPc: string | null | undefined;
  microfono: string | null | undefined;
  /** Sus mandos del panel, tal como van al archivo. */
  volumenPc: VolumenDePista | null | undefined;
  volumenMicrofono: VolumenDePista | null | undefined;
  /** La vista previa sin sonido (el botón de al lado del play): las dos pistas se callan. */
  callada?: boolean;
  /** El instante que se enseña cuando no hay vídeo de vista previa todavía. */
  msParado: number;
  /** Una pista que no se puede abrir: el editor vuelve a sonar por el vídeo, con todo junto. */
  onFallo?: (motivo: string) => void;
}

/**
 * S11 (pistas): el sonido del PC y la voz en la vista previa, cada uno en su `<audio>`, con el
 * volumen de su mando en vivo y al compás del vídeo (como la cámara en `CapaCamara`, que va a
 * su aire y solo se recoloca si se aleja). No pinta nada.
 */
export function SonidoPorPistas({
  videoRef,
  sonidoPc,
  microfono,
  volumenPc,
  volumenMicrofono,
  callada = false,
  msParado,
  onFallo,
}: Props) {
  const pc = useRef<HTMLAudioElement>(null);
  const voz = useRef<HTMLAudioElement>(null);
  // Lo que cambia en cada render, a mano para el bucle: así no se rehace a cada toque del mando.
  const vivo = useRef({ pc: 0, voz: 0, parado: msParado });
  vivo.current = callada
    ? { pc: 0, voz: 0, parado: msParado }
    : { pc: volumenEnVista(volumenPc), voz: volumenEnVista(volumenMicrofono), parado: msParado };

  useEffect(() => {
    const estados: [EstadoDePista, EstadoDePista] = [{ recolocadaMs: -Infinity }, { recolocadaMs: -Infinity }];
    const paso = () => {
      const ahora = performance.now();
      const video = videoRef.current;
      const { pc: vPc, voz: vVoz, parado } = vivo.current;
      if (pc.current) seguirAlVideo(pc.current, video, vPc, parado, ahora, estados[0]);
      if (voz.current) seguirAlVideo(voz.current, video, vVoz, parado, ahora, estados[1]);
    };
    // Además de en cada pintada, en cuanto el vídeo arranca, se para, salta o cambia de
    // velocidad: con el editor minimizado no hay pintadas, y el vídeo que se para en el corte
    // dejaba las pistas sonando hasta volver a la ventana. Y arrancar en el mismo aviso que el
    // vídeo le ahorra a la pista el fotograma de retraso.
    const AVISOS = ["play", "pause", "seeking", "ratechange", "volumechange"] as const;
    let enganchado: HTMLVideoElement | null = null;
    const enganchar = () => {
      const video = videoRef.current;
      if (video === enganchado) return;
      for (const aviso of AVISOS) enganchado?.removeEventListener(aviso, paso);
      for (const aviso of AVISOS) video?.addEventListener(aviso, paso);
      enganchado = video;
    };
    let id = 0;
    const vuelta = () => {
      id = requestAnimationFrame(vuelta);
      enganchar();
      paso();
    };
    id = requestAnimationFrame(vuelta);
    return () => {
      cancelAnimationFrame(id);
      for (const aviso of AVISOS) enganchado?.removeEventListener(aviso, paso);
      pc.current?.pause();
      voz.current?.pause();
    };
  }, [videoRef]);

  const urlPc = useMemo(() => (sonidoPc ? convertFileSrc(sonidoPc) : null), [sonidoPc]);
  const urlVoz = useMemo(() => (microfono ? convertFileSrc(microfono) : null), [microfono]);
  const fallo = (cual: string) => (e: React.SyntheticEvent<HTMLAudioElement>) =>
    onFallo?.(`${cual}: ${e.currentTarget.error?.message || "no se puede abrir"}`);

  return (
    <>
      {urlPc && <audio ref={pc} src={urlPc} preload="auto" data-sonido="pc" onError={fallo("pc")} />}
      {urlVoz && <audio ref={voz} src={urlVoz} preload="auto" data-sonido="microfono" onError={fallo("micrófono")} />}
    </>
  );
}
