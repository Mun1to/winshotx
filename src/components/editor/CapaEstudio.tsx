import { useEffect, useRef, type RefObject } from "react";
import type { MuestraCamara, StudioData } from "../../lib/types";
import {
  altoDelPuntero,
  arosEn,
  atajoEn,
  colocar,
  cursorEn,
  encuadreEn,
  opacidadDelAro,
  radioDelAro,
  radioMaximo,
  transformacion,
  FLECHA,
  type Estudio,
} from "../../lib/estudio";

interface Props {
  /** El vídeo de la vista previa: a él se le aplica el encuadre de la cámara. */
  videoRef: RefObject<HTMLVideoElement | null>;
  datos: StudioData | null;
  /** La cámara del zoom, una muestra por fotograma, ya con el zoom y el recorte puestos. */
  camara: MuestraCamara[];
  estudio: Estudio;
  /** Lo que mide la grabación, para medir el aro y el puntero igual que al exportar. */
  ancho: number;
  alto: number;
  /** El instante que se enseña cuando el vídeo está parado. */
  msParado: number;
  reproduciendo: boolean;
}

/**
 * Lo que el estudio va a dibujar al exportar, dibujado encima de la vista previa.
 *
 * Corre a la velocidad de la pantalla, no de React: cada fotograma lee el tiempo del
 * vídeo, mueve el vídeo con una transformación CSS para enseñar el encuadre de la cámara,
 * y pinta el puntero, los aros y la pastilla en un canvas que va encima y NO se
 * transforma, porque un aro tiene que medir lo mismo con zoom y sin él. Nada de esto
 * pasa por `setState`: sesenta repintados por segundo del editor entero se notarían.
 */
export function CapaEstudio({
  videoRef,
  datos,
  camara,
  estudio,
  ancho,
  alto,
  msParado,
  reproduciendo,
}: Props) {
  const lienzo = useRef<HTMLCanvasElement>(null);
  // Lo último que se sabe, para que el bucle no tenga que reengancharse a cada cambio.
  const estado = useRef({ datos, camara, estudio, ancho, alto, msParado, reproduciendo });
  estado.current = { datos, camara, estudio, ancho, alto, msParado, reproduciendo };

  useEffect(() => {
    let vivo = true;
    let peticion = 0;

    const pintar = () => {
      if (!vivo) return;
      peticion = requestAnimationFrame(pintar);
      const canvas = lienzo.current;
      const video = videoRef.current;
      const { datos, camara, estudio, ancho, alto, msParado, reproduciendo } = estado.current;
      if (!canvas) return;

      const caja = canvas.getBoundingClientRect();
      const W = Math.max(1, Math.round(caja.width));
      const H = Math.max(1, Math.round(caja.height));
      const dpr = window.devicePixelRatio || 1;
      if (canvas.width !== W * dpr || canvas.height !== H * dpr) {
        canvas.width = W * dpr;
        canvas.height = H * dpr;
      }

      const ms = reproduciendo && video ? video.currentTime * 1000 : msParado;
      const encuadre = estudio.zoom > 1.05 ? encuadreEn(camara, ms) : null;
      if (video) {
        const css = transformacion(encuadre, W, H);
        if (video.style.transform !== css) video.style.transform = css;
      }

      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, W, H);
      if (!datos) return;

      // Lo que mide un píxel del vídeo exportado en la vista previa: aros y puntero se
      // miden al exportar sobre el vídeo entero, así que aquí se reducen en la misma
      // proporción en la que la vista previa es más pequeña que el vídeo.
      const escala = H / Math.max(1, alto);
      const aPantalla = (x: number, y: number) => {
        const sitio = colocar(x / Math.max(1, ancho), y / Math.max(1, alto), encuadre);
        return sitio ? { x: sitio.u * W, y: sitio.v * H } : null;
      };

      // El puntero va DEBAJO del aro, como al exportar: el aro se abre desde donde se
      // pulsó y taparlo sería tapar justo lo que se señala.
      if (estudio.cursor > 0) {
        const raton = cursorEn(datos.cursor, ms);
        const punta = raton ? aPantalla(raton[0], raton[1]) : null;
        if (punta) {
          const altoPx = altoDelPuntero(estudio.cursor, datos.clics, ms) * escala;
          flecha(ctx, punta.x, punta.y, altoPx);
        }
      }

      if (estudio.aros) {
        for (const aro of arosEn(datos.clics, ms)) {
          const centro = aPantalla(aro.x, aro.y);
          if (!centro) continue;
          const radio = radioDelAro(aro.avance, alto) * escala;
          const grosor = Math.min(5.5, Math.max(2.5, radioMaximo(alto) / 7)) * escala;
          const opacidad = opacidadDelAro(aro.avance);
          const color = aro.derecho ? "255, 176, 32" : "10, 155, 255";
          ctx.beginPath();
          ctx.arc(centro.x, centro.y, Math.max(0.5, radio), 0, Math.PI * 2);
          ctx.fillStyle = `rgba(${color}, ${(0.22 * opacidad).toFixed(3)})`;
          ctx.fill();
          ctx.lineWidth = Math.max(1, grosor);
          ctx.strokeStyle = `rgba(${color}, ${opacidad.toFixed(3)})`;
          ctx.stroke();
        }
      }

      if (estudio.teclas) {
        const atajo = atajoEn(datos.teclas, ms);
        if (atajo) pastilla(ctx, atajo.texto, atajo.opacidad, W, H, escala);
      }
    };

    peticion = requestAnimationFrame(pintar);
    return () => {
      vivo = false;
      cancelAnimationFrame(peticion);
      // Al irse, el vídeo se queda como estaba: sin encuadre.
      if (videoRef.current) videoRef.current.style.transform = "none";
    };
  }, [videoRef]);

  return (
    <canvas
      ref={lienzo}
      data-capa-estudio
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 size-full"
    />
  );
}

/** La flecha de Windows con la punta en `(x, y)`: relleno oscuro y borde blanco. */
function flecha(ctx: CanvasRenderingContext2D, x: number, y: number, alto: number) {
  if (alto < 4) return;
  ctx.beginPath();
  FLECHA.forEach(([u, v], i) => {
    const px = x + u * alto;
    const py = y + v * alto;
    if (i === 0) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  });
  ctx.closePath();
  ctx.lineJoin = "round";
  ctx.lineWidth = Math.max(1, alto * 0.11);
  ctx.strokeStyle = "rgba(255, 255, 255, 1)";
  ctx.stroke();
  ctx.fillStyle = "rgb(16, 16, 16)";
  ctx.fill();
}

/** La pastilla del atajo, abajo y centrada, a un octavo del alto: donde va al exportar. */
function pastilla(
  ctx: CanvasRenderingContext2D,
  texto: string,
  opacidad: number,
  W: number,
  H: number,
  escala: number,
) {
  const letra = Math.max(9, 22 * escala);
  const aireX = 16 * escala;
  const aireY = 10 * escala;
  ctx.font = `600 ${letra.toFixed(1)}px system-ui, "Segoe UI", sans-serif`;
  ctx.textBaseline = "middle";
  ctx.textAlign = "center";
  const anchoTexto = ctx.measureText(texto).width;
  const w = anchoTexto + aireX * 2;
  const h = letra + aireY * 2;
  const x = (W - w) / 2;
  const y = H - h - H / 8;
  const r = Math.min(h / 2, 10 * escala);
  ctx.globalAlpha = opacidad;
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
  ctx.fillStyle = "rgba(20, 20, 24, 0.86)";
  ctx.fill();
  ctx.lineWidth = 1;
  ctx.strokeStyle = "rgba(255, 255, 255, 0.18)";
  ctx.stroke();
  ctx.fillStyle = "#fff";
  ctx.fillText(texto, W / 2, y + h / 2);
  ctx.globalAlpha = 1;
}
