import { useEffect, useMemo, useRef, type RefObject } from "react";
import type { ClicElegido } from "../../lib/contrato";
import {
  bufferDelClic,
  clicsQueSuenan,
  contextoDeSonido,
  gananciaDelClic,
  relojDeSalida,
} from "../../lib/sonidoDeClic";
import type { Pieza } from "../../lib/tramo";

interface Props {
  /** El vídeo de la vista previa: los clics suenan a su compás. */
  videoRef: RefObject<HTMLVideoElement | null>;
  reproduciendo: boolean;
  /** Los clics de la grabación, en su reloj. */
  clics: number[];
  piezas: Pieza[];
  desdeMs: number;
  hastaMs: number;
  /** `edicion.sonidoClic`: de 0 a 1, y 0 es sin sonido. */
  volumen: number;
  clic: ClicElegido | null | undefined;
}

/** Lo que se adelanta al programar: un clic se pone en su instante exacto, no en la pintada que lo pasa. */
const ADELANTE_MS = 150;
/** Un salto del reloj de salida más grande que esto es que alguien ha movido el cabezal. */
const SALTO_MS = 250;

/**
 * El sonido de clic en la vista previa (S10), en el mismo instante y con el mismo volumen que
 * en el MP4. No pinta nada.
 *
 * En cada pintada mira por dónde va el vídeo y programa en WebAudio los clics de los próximos
 * `ADELANTE_MS`, cada uno en su instante: así no llegan tarde lo que tarda una pintada. Lo que
 * falta para un clic se mide en el reloj del vídeo que sale (`relojDeSalida`), que es el de la
 * reproducción: con lo cortado saltado y lo acelerado a su velocidad. Al parar o al mover el
 * cabezal, lo programado se quita.
 */
export function SonidoDeClics({ videoRef, reproduciendo, clics, piezas, desdeMs, hastaMs, volumen, clic }: Props) {
  const suenan = useMemo(() => clicsQueSuenan(clics, piezas, desdeMs, hastaMs), [clics, piezas, desdeMs, hastaMs]);
  const salida = useMemo(() => relojDeSalida(piezas, desdeMs, hastaMs), [piezas, desdeMs, hastaMs]);
  // El volumen en vivo: moverlo mientras suena cambia lo que viene sin volver a programar.
  const ganancia = useRef<GainNode | null>(null);
  const volumenVivo = useRef(volumen);
  volumenVivo.current = volumen;
  useEffect(() => {
    if (ganancia.current) ganancia.current.gain.value = gananciaDelClic(volumen);
  }, [volumen]);

  const activo = reproduciendo && volumen > 0 && suenan.length > 0;
  useEffect(() => {
    if (!activo) return;
    const ctx = contextoDeSonido();
    if (!ctx) return;
    let vivo = true;
    let id = 0;
    const programados = new Map<number, AudioBufferSourceNode>();
    const nodo = ctx.createGain();
    nodo.gain.value = gananciaDelClic(volumenVivo.current);
    nodo.connect(ctx.destination);
    ganancia.current = nodo;
    const quitar = () => {
      for (const fuente of programados.values()) {
        try {
          fuente.stop();
        } catch {
          // Ya había acabado.
        }
      }
      programados.clear();
    };

    void bufferDelClic(ctx, clic)
      .then((buffer) => {
        if (!vivo) return;
        let antes = -1;
        const paso = () => {
          id = requestAnimationFrame(paso);
          const video = videoRef.current;
          if (!video || video.paused || video.seeking) {
            quitar();
            antes = -1;
            return;
          }
          const ms = video.currentTime * 1000;
          const ahora = salida(ms);
          if (antes >= 0 && (ahora < antes - 1 || ahora > antes + SALTO_MS)) quitar();
          antes = ahora;
          for (const c of suenan) {
            if (c < ms - 5) continue;
            const falta = salida(c) - ahora;
            if (falta > ADELANTE_MS) break;
            if (programados.has(c)) continue;
            const fuente = ctx.createBufferSource();
            fuente.buffer = buffer;
            fuente.connect(nodo);
            fuente.start(ctx.currentTime + Math.max(0, falta) / 1000);
            programados.set(c, fuente);
          }
        };
        id = requestAnimationFrame(paso);
      })
      .catch(() => undefined);

    return () => {
      vivo = false;
      cancelAnimationFrame(id);
      quitar();
      nodo.disconnect();
      if (ganancia.current === nodo) ganancia.current = null;
    };
  }, [activo, clic, suenan, salida, videoRef]);

  return null;
}
