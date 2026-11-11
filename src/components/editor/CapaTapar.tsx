import { useEffect, useRef, type RefObject } from "react";
import type { Tapa } from "../../lib/contrato";
import { ladoDelMosaico, puestasEn } from "../../lib/tapar";

interface Props {
  /** El vídeo de la vista previa: de él sale lo que se emborrona, y su encuadre. */
  videoRef: RefObject<HTMLVideoElement | null>;
  tapas: Tapa[];
  /** Lo que mide la grabación, para que los cuadrados midan lo mismo que al exportar. */
  ancho: number;
  alto: number;
  /** El instante que se enseña con el vídeo parado. */
  msParado: number;
  reproduciendo: boolean;
  /** El mosaico elegido en la línea de tiempo, que se marca con su borde. */
  elegida?: number | null;
}

/**
 * El mosaico de lo tapado, encima de la vista previa (D4): lo que va a salir, a la vista.
 *
 * Es un mosaico de verdad y no una zona oscura, porque lo que se quiere ver aquí es qué
 * queda tapado y cómo, y en un vídeo eso cambia de un instante a otro. Se pinta en un lienzo
 * a partir del propio vídeo: el trozo se encoge a un cuadrado por celda y se vuelve a
 * estirar sin suavizar, con celdas del tamaño de las de Rust (`ladoDelMosaico`).
 *
 * Cada mosaico lleva aquí un borde discontinuo fino (el elegido, más marcado), que no sale en
 * el archivo: sobre un fondo liso el mosaico no se distingue de lo que había.
 *
 * Va sobre la imagen grabada: copia el encuadre de la cámara del zoom que `CapaEstudio` le
 * pone al vídeo, así que con zoom el mosaico se acerca con el dato, igual que en el archivo.
 * Mientras el vídeo no se puede leer, la zona va tapada en oscuro: nunca se enseña sin tapar.
 */
export function CapaTapar({ videoRef, tapas, ancho, alto, msParado, reproduciendo, elegida = null }: Props) {
  const lienzo = useRef<HTMLCanvasElement>(null);
  const vivo = useRef({ msParado, reproduciendo, elegida });
  vivo.current = { msParado, reproduciendo, elegida };

  useEffect(() => {
    if (tapas.length === 0) return;
    const chico = document.createElement("canvas");
    let id = 0;
    const pintar = () => {
      const el = lienzo.current;
      const video = videoRef.current;
      if (el) {
        const { msParado: parado, reproduciendo: suena, elegida: cual } = vivo.current;
        const ms = suena && video ? video.currentTime * 1000 : parado;
        // El encuadre de la cámara del zoom, el mismo que lleva el vídeo.
        const encuadre = video?.style.transform || "none";
        if (el.style.transform !== encuadre) el.style.transform = encuadre;
        const puestas = puestasEn(tapas, ms);
        el.dataset.puestas = String(puestas.length);
        const escala = window.devicePixelRatio || 1;
        const W = Math.max(1, Math.round(el.clientWidth * escala));
        const H = Math.max(1, Math.round(el.clientHeight * escala));
        if (el.width !== W || el.height !== H) {
          el.width = W;
          el.height = H;
        }
        const ctx = el.getContext("2d");
        if (ctx) {
          ctx.clearRect(0, 0, W, H);
          const listo = !!video && video.readyState >= 2 && video.videoWidth > 0;
          for (const { tapa: t, indice } of puestas) {
            const dx = t.x * W;
            const dy = t.y * H;
            const dw = t.ancho * W;
            const dh = t.alto * H;
            if (listo && video) {
              const lado = ladoDelMosaico(t.ancho * ancho, t.alto * alto);
              chico.width = Math.max(1, Math.ceil((t.ancho * ancho) / lado));
              chico.height = Math.max(1, Math.ceil((t.alto * alto) / lado));
              const c = chico.getContext("2d");
              if (c) {
                const vw = video.videoWidth;
                const vh = video.videoHeight;
                c.drawImage(video, t.x * vw, t.y * vh, t.ancho * vw, t.alto * vh, 0, 0, chico.width, chico.height);
                ctx.imageSmoothingEnabled = false;
                ctx.drawImage(chico, dx, dy, dw, dh);
              }
            } else {
              ctx.fillStyle = "rgba(15, 15, 18, 0.92)";
              ctx.fillRect(dx, dy, dw, dh);
            }
            // El borde solo está aquí, no en el archivo: sobre un fondo liso el mosaico no
            // se distingue de lo que había, y lo que se quiere ver es QUÉ se tapa. El elegido,
            // más marcado.
            ctx.save();
            ctx.setLineDash([8 * escala, 6 * escala]);
            ctx.lineWidth = (indice === cual ? 2 : 1) * escala;
            ctx.strokeStyle = indice === cual ? "#ffffff" : "rgba(255, 255, 255, 0.6)";
            ctx.strokeRect(dx, dy, dw, dh);
            ctx.restore();
          }
        }
      }
      id = requestAnimationFrame(pintar);
    };
    id = requestAnimationFrame(pintar);
    return () => cancelAnimationFrame(id);
  }, [tapas, videoRef, ancho, alto]);

  if (tapas.length === 0) return null;
  return (
    <canvas
      ref={lienzo}
      data-capa-tapar
      data-puestas={puestasEn(tapas, msParado).length}
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 size-full"
      style={{ transformOrigin: "0 0" }}
    />
  );
}
