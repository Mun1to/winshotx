import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { MuestraCamara, StudioData } from "../../lib/types";
import type { Edicion, TramoZoom } from "../../lib/contrato";
import { medidasDeLupa, ventanaDelZoom } from "../../lib/zooms";
import { useT } from "../../lib/i18n";
import {
  altoDelPuntero,
  arosEn,
  atajoEn,
  colocar,
  desenfoqueDeCamara,
  dibujoDe,
  encuadreEn,
  estelaDelPuntero,
  formaEn,
  movimientos,
  opacidadAlEscribir,
  opacidadDelAro,
  opacidadDelPuntero,
  ondaEn,
  ondasEn,
  punteroEn,
  radioDelAro,
  radioMaximo,
  sitioDelPuntero,
  sitioSuavizado,
  transformacion,
  type Estudio,
  type Forma,
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
  /**
   * Enseñar la imagen entera aunque haya zoom: mientras se dibuja o se recorta, lo que se
   * dibuja se mide sobre la captura entera y con la cámara acercada caería en otro sitio.
   */
  sinEncuadre?: boolean;
  /** Los bloques de la pista de zooms: de aquí salen las lupas que se pintan encima. */
  zooms?: TramoZoom[];
  /**
   * El bloque elegido en la pista. Si es fijo, y el vídeo está parado, se enseña la imagen
   * entera con lo que va a enseñar el zoom (o la lupa) y su punto, que se arrastra.
   */
  elegido?: TramoZoom | null;
  onPunto?: (x: number, y: number) => void;
  /**
   * Lo del cursor que viaja en `edicion` (S7): el suavizado, esconderlo al escribir, la onda
   * de los clics y la estela. Sin esto, lo de siempre.
   */
  edicion?: Edicion;
}

/** Lo que tarda una lupa en abrirse y en cerrarse. El mismo número que `LUPA_APARECE_MS` en Rust. */
const LUPA_APARECE_MS = 220;

/** Lo abierta que está una lupa en ese instante, de 0 a 1, o `null` fuera de su bloque. */
function abierta(z: TramoZoom, ms: number): number | null {
  if (ms < z.desdeMs || ms > z.hastaMs) return null;
  if (z.instantaneo) return 1;
  const rampa = Math.max(1, Math.min(LUPA_APARECE_MS, (z.hastaMs - z.desdeMs) / 3));
  const t = Math.min(1, Math.max(0, Math.min((ms - z.desdeMs) / rampa, (z.hastaMs - ms) / rampa)));
  return t * t * (3 - 2 * t);
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
  sinEncuadre = false,
  zooms = [],
  elegido = null,
  onPunto,
  edicion,
}: Props) {
  const lienzo = useRef<HTMLCanvasElement>(null);
  /** Colocando el punto de un bloque fijo: la imagen entera, para ver dónde se pone. */
  const colocando = Boolean(elegido && elegido.modo === "fijo" && !reproduciendo && onPunto);
  const lupas = useMemo(() => zooms.filter((z) => z.lupa), [zooms]);
  /**
   * Las imágenes de verdad de los punteros, cargadas una vez. Se dibujan con `drawImage`,
   * que es lo que hace que el puntero de la vista previa sea el mismo que el de Windows y
   * el mismo que va a salir al exportar.
   */
  const imagenes = useMemo(() => {
    const mapa = new Map<number, HTMLImageElement>();
    for (const p of datos?.punteros ?? []) {
      const img = new Image();
      img.src = convertFileSrc(p.archivo);
      mapa.set(p.id, img);
    }
    return mapa;
  }, [datos]);
  /** Cuándo se movió el ratón, sacado una vez: con ello se sabe cuánto lleva quieto. */
  const movs = useMemo(() => movimientos(datos?.cursor ?? []), [datos]);
  // Lo último que se sabe, para que el bucle no tenga que reengancharse a cada cambio.
  const estado = useRef({ datos, camara, estudio, ancho, alto, msParado, reproduciendo, sinEncuadre, imagenes, movs, lupas, colocando, edicion });
  estado.current = { datos, camara, estudio, ancho, alto, msParado, reproduciendo, sinEncuadre, imagenes, movs, lupas, colocando, edicion };

  useEffect(() => {
    let vivo = true;
    let peticion = 0;

    const pintar = () => {
      if (!vivo) return;
      peticion = requestAnimationFrame(pintar);
      const canvas = lienzo.current;
      const video = videoRef.current;
      const { datos, camara, estudio, ancho, alto, msParado, reproduciendo, sinEncuadre, imagenes, movs, lupas, colocando, edicion } =
        estado.current;
      // Lo del cursor (S7): el suavizado elegido manda sobre el interruptor de antes.
      const suavizado = edicion?.suavizado ?? (estudio.suave ? "poco" : "nada");
      const estela = Math.min(1, Math.max(0, edicion?.estela ?? 0));
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
      // La cámara la pide el editor solo cuando hay algo que acercar (el zoom de los clics,
      // la tecla de zoom o un bloque de la pista): sin muestras, la imagen entera.
      const encuadre = camara.length > 0 && !sinEncuadre && !colocando ? encuadreEn(camara, ms) : null;
      if (video) {
        const css = transformacion(encuadre, W, H);
        if (video.style.transform !== css) video.style.transform = css;
        // La estela de la cámara (A4): mientras se mueve, el vídeo se emborrona lo que ha
        // recorrido en el último paso de exportar. En redondo, que es lo que sabe el navegador.
        const antes = encuadre && estela > 0 ? encuadreEn(camara, ms - 33) : null;
        const borroso = encuadre && estela > 0 ? desenfoqueDeCamara(antes, encuadre, W, H, estela) : 0;
        const filtro = borroso > 0.3 ? `blur(${borroso.toFixed(1)}px)` : "";
        if (video.style.filter !== filtro) video.style.filter = filtro;
      }

      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, W, H);

      // Las lupas de la pista, con la imagen del vídeo ampliada debajo de su lente.
      if (video && video.videoWidth > 0) {
        for (const z of lupas) {
          const abre = z.lupa ? abierta(z, ms) : null;
          if (!z.lupa || abre === null || abre < 0.02) continue;
          let u = z.x;
          let v = z.y;
          if (z.modo === "sigue" && datos) {
            const raton = sitioDelPuntero(datos.cursor, ms, true);
            if (raton) {
              u = raton[0] / Math.max(1, ancho);
              v = raton[1] / Math.max(1, alto);
            }
          }
          pintarLupa(ctx, video, z, u, v, abre, W, H, encuadre, ancho);
        }
      }

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
      // Suavizado y apagándose como al exportar: las dos cuentas son las de Rust.
      let opacidad = estudio.esconder ? opacidadDelPuntero(movs, datos.clics, ms) : 1;
      if (edicion?.esconderAlEscribir) opacidad *= opacidadAlEscribir(datos.escritura ?? [], movs, datos.clics, ms);
      if (estudio.cursor > 0 && opacidad > 0) {
        const raton = sitioSuavizado(datos.cursor, datos.clics, ms, suavizado);
        const punta = raton ? aPantalla(raton[0], raton[1]) : null;
        if (punta) {
          const altoPx = altoDelPuntero(estudio.cursor, datos.clics, ms) * escala;
          const id = punteroEn(datos.cambiosPuntero, ms);
          const leido = id === null ? undefined : datos.punteros.find((p) => p.id === id);
          const img = leido ? imagenes.get(leido.id) : undefined;
          const forma = formaEn(datos.formas, ms);
          const dibujar = (x: number, y: number, alfa: number) => {
            ctx.globalAlpha = alfa;
            if (leido && img && img.complete && img.naturalWidth > 0 && leido.alto > 0) {
              const s = altoPx / leido.alto;
              ctx.drawImage(img, x - leido.caliente[0] * s, y - leido.caliente[1] * s, leido.ancho * s, altoPx);
            } else {
              puntero(ctx, x, y, altoPx, forma);
            }
          };
          // La estela va detrás (A4): el mismo puntero donde estuvo hace nada, más claro.
          for (const copia of estelaDelPuntero(ms, estela)) {
            const antes = sitioSuavizado(datos.cursor, datos.clics, copia.t, suavizado);
            const donde = antes ? aPantalla(antes[0], antes[1]) : null;
            if (donde && Math.abs(donde.x - punta.x) + Math.abs(donde.y - punta.y) >= 2 * escala) {
              dibujar(donde.x, donde.y, opacidad * copia.alfa);
            }
          }
          dibujar(punta.x, punta.y, opacidad);
          ctx.globalAlpha = 1;
        }
      }

      // La onda de cada clic, debajo del aro y saliendo de él (C4).
      if (edicion?.ondaClic) {
        for (const onda of ondasEn(datos.clics, ms)) {
          const centro = aPantalla(onda.x, onda.y);
          if (!centro) continue;
          const { radio, opacidad, grosor } = ondaEn(onda.avance, alto);
          if (opacidad <= 0) continue;
          ctx.beginPath();
          ctx.arc(centro.x, centro.y, Math.max(0.5, radio * escala), 0, Math.PI * 2);
          ctx.lineWidth = Math.max(1, grosor * escala);
          ctx.strokeStyle = `rgba(${onda.derecho ? "255, 176, 32" : "10, 155, 255"}, ${opacidad.toFixed(3)})`;
          ctx.stroke();
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
      // Al irse, el vídeo se queda como estaba: sin encuadre ni estela.
      if (videoRef.current) {
        videoRef.current.style.transform = "none";
        videoRef.current.style.filter = "";
      }
    };
  }, [videoRef]);

  return (
    <>
      <canvas
        ref={lienzo}
        data-capa-estudio
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 size-full"
      />
      {colocando && !sinEncuadre && elegido && onPunto && (
        <PuntoFijo zoom={elegido} ancho={ancho} alto={alto} onPunto={onPunto} />
      )}
    </>
  );
}

/**
 * El punto de un zoom fijo o de una lupa fija, encima de la imagen entera: el recuadro (o
 * la lente) dice lo que se va a ver, y el punto se arrastra. Pulsar en cualquier sitio de
 * la imagen lo lleva ahí.
 */
function PuntoFijo({
  zoom,
  ancho,
  alto,
  onPunto,
}: {
  zoom: TramoZoom;
  ancho: number;
  alto: number;
  onPunto: (x: number, y: number) => void;
}) {
  const t = useT();
  const caja = useRef<HTMLDivElement>(null);
  const [arrastrando, setArrastrando] = useState(false);
  const alPunto = useRef(onPunto);
  alPunto.current = onPunto;

  const llevar = (clientX: number, clientY: number) => {
    const r = caja.current?.getBoundingClientRect();
    if (!r || r.width <= 0 || r.height <= 0) return;
    const x = Math.min(1, Math.max(0, (clientX - r.left) / r.width));
    const y = Math.min(1, Math.max(0, (clientY - r.top) / r.height));
    alPunto.current(Math.round(x * 1000) / 1000, Math.round(y * 1000) / 1000);
  };

  useEffect(() => {
    if (!arrastrando) return;
    const mover = (e: PointerEvent) => llevar(e.clientX, e.clientY);
    const soltar = () => setArrastrando(false);
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", soltar);
    window.addEventListener("pointercancel", soltar);
    return () => {
      window.removeEventListener("pointermove", mover);
      window.removeEventListener("pointerup", soltar);
      window.removeEventListener("pointercancel", soltar);
    };
  }, [arrastrando]);

  // Lo que se va a ver: el trozo que enseña el zoom, o la lente de la lupa en su sitio.
  let recuadro: { left: number; top: number; width: number; height: number };
  let redondo = "";
  if (zoom.lupa) {
    const m = medidasDeLupa(zoom.lupa, ancho, alto);
    const w = m.ancho / Math.max(1, ancho);
    const h = m.alto / Math.max(1, alto);
    const cx = Math.min(1 - w / 2, Math.max(w / 2, zoom.x));
    const cy = Math.min(1 - h / 2, Math.max(h / 2, zoom.y));
    recuadro = { left: cx - w / 2, top: cy - h / 2, width: w, height: h };
    redondo = zoom.lupa.forma === "circulo" ? "rounded-full" : zoom.lupa.forma === "redondeado" ? "rounded-[18%]" : "";
  } else {
    const v = ventanaDelZoom(zoom.x, zoom.y, zoom.nivel);
    recuadro = { left: v.x1, top: v.y1, width: v.x2 - v.x1, height: v.y2 - v.y1 };
  }
  const pc = (n: number) => `${n * 100}%`;

  return (
    <div
      ref={caja}
      data-punto-fijo
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        llevar(e.clientX, e.clientY);
        setArrastrando(true);
      }}
      className="absolute inset-0 cursor-crosshair touch-none"
    >
      <div
        className={`pointer-events-none absolute border-2 border-dashed border-white shadow-[0_0_0_1px_rgba(0,0,0,0.55),0_0_0_9999px_rgba(0,0,0,0.28)] ${redondo}`}
        style={{
          left: pc(recuadro.left),
          top: pc(recuadro.top),
          width: pc(recuadro.width),
          height: pc(recuadro.height),
        }}
      />
      <div
        role="slider"
        tabIndex={0}
        aria-label={t("Punto del zoom: arrástralo a donde tiene que mirar")}
        aria-valuetext={`${Math.round(zoom.x * 100)} %, ${Math.round(zoom.y * 100)} %`}
        onKeyDown={(e) => {
          const paso = e.shiftKey ? 0.05 : 0.01;
          const mueve: Record<string, [number, number]> = {
            ArrowLeft: [-paso, 0],
            ArrowRight: [paso, 0],
            ArrowUp: [0, -paso],
            ArrowDown: [0, paso],
          };
          const d = mueve[e.key];
          if (!d) return;
          e.preventDefault();
          e.stopPropagation();
          onPunto(Math.min(1, Math.max(0, zoom.x + d[0])), Math.min(1, Math.max(0, zoom.y + d[1])));
        }}
        className="absolute size-5 -translate-x-1/2 -translate-y-1/2 cursor-move rounded-full border-[3px] border-white bg-blue-500 shadow-[0_1px_6px_rgba(0,0,0,0.6)] outline-none focus-visible:ring-2 focus-visible:ring-blue-300"
        style={{ left: pc(zoom.x), top: pc(zoom.y) }}
      />
    </div>
  );
}

/** El contorno de una lupa, para recortar por él y para dibujar su marco. */
function camino(ctx: CanvasRenderingContext2D, forma: FormaDeLupa, cx: number, cy: number, w: number, h: number) {
  ctx.beginPath();
  if (forma === "circulo") {
    ctx.arc(cx, cy, Math.min(w, h) / 2, 0, Math.PI * 2);
  } else if (forma === "redondeado") {
    ctx.roundRect(cx - w / 2, cy - h / 2, w, h, (Math.min(w, h) / 2) * 0.36);
  } else {
    ctx.rect(cx - w / 2, cy - h / 2, w, h);
  }
}
type FormaDeLupa = NonNullable<TramoZoom["lupa"]>["forma"];

/**
 * Una lupa en la vista previa: el trozo del vídeo de alrededor de su punto, ampliado, dentro
 * de su forma. Las mismas medidas y el mismo marco que pinta Rust (`lupa::pintar`).
 */
function pintarLupa(
  ctx: CanvasRenderingContext2D,
  video: HTMLVideoElement,
  z: TramoZoom,
  u: number,
  v: number,
  abre: number,
  W: number,
  H: number,
  encuadre: ReturnType<typeof encuadreEn>,
  anchoGrabado: number,
) {
  const lupa = z.lupa;
  if (!lupa) return;
  const sitio = colocar(u, v, encuadre);
  if (!sitio) return;
  const medida = medidasDeLupa(lupa, W, H);
  const lw = medida.ancho * abre;
  const lh = medida.alto * abre;
  if (lw < 4 || lh < 4) return;
  const aumento = 1 + (lupa.aumento - 1) * abre;
  const cx = sitio.u * W;
  const cy = sitio.v * H;
  const lx = Math.min(W - lw / 2, Math.max(lw / 2, cx));
  const ly = Math.min(H - lh / 2, Math.max(lh / 2, cy));
  // Lo que enseña el lienzo del vídeo: con la cámara acercada, solo su encuadre.
  const e = encuadre ?? { x1: 0, y1: 0, x2: 1, y2: 1 };
  const vw = video.videoWidth;
  const vh = video.videoHeight;
  const sw = (lw / aumento) * ((e.x2 - e.x1) * vw) / W;
  const sh = (lh / aumento) * ((e.y2 - e.y1) * vh) / H;
  const sx = (e.x1 + (cx / W) * (e.x2 - e.x1)) * vw - sw / 2;
  const sy = (e.y1 + (cy / H) * (e.y2 - e.y1)) * vh - sh / 2;
  ctx.save();
  camino(ctx, lupa.forma, lx, ly, lw, lh);
  ctx.clip();
  ctx.drawImage(video, sx, sy, sw, sh, lx - lw / 2, ly - lh / 2, lw, lh);
  ctx.restore();
  if (lupa.marco) {
    // El borde de Rust se mide en píxeles del vídeo exportado; aquí, en los de la vista.
    const borde = Math.max(1.5, (Math.min(6, Math.max(2, anchoGrabado * 0.0028)) * W) / Math.max(1, anchoGrabado));
    camino(ctx, lupa.forma, lx, ly, lw - borde, lh - borde);
    ctx.lineWidth = borde;
    ctx.strokeStyle = "#fff";
    ctx.stroke();
    camino(ctx, lupa.forma, lx, ly, lw + 1, lh + 1);
    ctx.lineWidth = 1;
    ctx.strokeStyle = "rgba(0, 0, 0, 0.32)";
    ctx.stroke();
  }
}

/**
 * El puntero con su punto caliente en `(x, y)`: la flecha y la barra de texto van oscuras
 * con borde blanco, la manita va blanca con borde oscuro, como las de Windows.
 */
function puntero(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  alto: number,
  forma: Forma,
) {
  if (alto < 4) return;
  const { puntos, caliente, claro } = dibujoDe(forma);
  ctx.beginPath();
  puntos.forEach(([u, v], i) => {
    const px = x + (u - caliente[0]) * alto;
    const py = y + (v - caliente[1]) * alto;
    if (i === 0) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  });
  ctx.closePath();
  ctx.lineJoin = "round";
  ctx.lineWidth = Math.max(1, alto * 0.11);
  ctx.strokeStyle = claro ? "rgb(16, 16, 16)" : "rgba(255, 255, 255, 1)";
  ctx.stroke();
  ctx.fillStyle = claro ? "rgb(250, 250, 250)" : "rgb(16, 16, 16)";
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
