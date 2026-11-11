import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { createPortal } from "react-dom";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { MarcoCamara } from "../../lib/contrato";
import { camaraDeLaSesion } from "../../lib/ipc";
import {
  caja,
  esquinaDeAgarre,
  grosorDelBorde,
  instanteDeLaCamara,
  marcoDeFabrica,
  mover,
  radio,
  recorteVisible,
  redimensionar,
  type Caja,
  type CamaraDeLaSesion,
  type Medida,
} from "../../lib/camara";
import { useT } from "../../lib/i18n";

interface Props {
  sessionId: string;
  /** El vídeo de la vista previa: la cámara va a su compás, también al saltar un corte. */
  videoRef: RefObject<HTMLVideoElement | null>;
  /**
   * Cómo va la cámara. `undefined` es que nadie la ha tocado todavía: entonces se pone la
   * de fábrica. `null` es que se quitó.
   */
  marco: MarcoCamara | null | undefined;
  onMarco: (marco: MarcoCamara) => void;
  /**
   * El hueco de la vista previa. La burbuja se pinta ahí y no dentro de la imagen, porque
   * va encima del vídeo EXPORTADO, con su aire alrededor: dentro de la imagen se cortaría
   * por el borde de la captura.
   */
  hueco: RefObject<HTMLElement | null>;
  /** Los píxeles de aire del marco del estudio alrededor de la imagen, en la vista previa. */
  aire: number;
  /** Mientras se dibuja o se recorta no se agarra: lo que se arrastra es la marca. */
  quieta: boolean;
  /** El instante que se enseña cuando no hay vídeo de vista previa todavía. */
  msParado: number;
}

/** Lo que se está arrastrando: la burbuja entera o su esquina. */
interface Arrastre {
  modo: "mover" | "tamano";
  x0: number;
  y0: number;
  antes: Caja;
  marco: MarcoCamara;
  hacia: { haciaX: -1 | 1; haciaY: -1 | 1 };
}

/** Lo lejos que puede ir la cámara del vídeo antes de recolocarla, reproduciendo. */
const DESVIO_MAXIMO_S = 0.15;

/**
 * La webcam encima de la vista previa: en su forma, con su recorte, a su tamaño y en su
 * sitio, como va a salir en el archivo. Se mueve arrastrándola y cambia de tamaño por la
 * esquina que mira al centro del vídeo, que es la que tiene sitio para crecer.
 *
 * El `<video>` de la cámara va a su aire y se recoloca solo si se aleja del vídeo de la
 * pantalla: buscar un instante en cada fotograma haría que la cara fuera a saltos.
 */
export function CapaCamara({ sessionId, videoRef, marco, onMarco, hueco, aire, quieta, msParado }: Props) {
  const t = useT();
  const [camara, setCamara] = useState<CamaraDeLaSesion | null>(null);
  const ancla = useRef<HTMLDivElement>(null);
  const cara = useRef<HTMLVideoElement>(null);
  /** Dónde está el vídeo exportado dentro del hueco, en píxeles de pantalla. */
  const [sitio, setSitio] = useState<{ left: number; top: number; ancho: number; alto: number } | null>(null);
  const arrastre = useRef<Arrastre | null>(null);
  /** El marco mientras se arrastra: se le pasa al editor al soltar, no en cada movimiento. */
  const [vivo, setVivo] = useState<MarcoCamara | null>(null);
  const ultimo = useRef<MarcoCamara | null>(null);
  const parado = useRef(msParado);
  parado.current = msParado;

  useEffect(() => {
    let vigente = true;
    void camaraDeLaSesion(sessionId)
      .then((c) => {
        if (vigente) setCamara(c ?? null);
      })
      .catch(() => undefined);
    return () => {
      vigente = false;
    };
  }, [sessionId]);

  // Lo que mide el vídeo exportado en la vista previa: la imagen y su aire alrededor.
  //
  // En un efecto normal y no en uno de maquetación: esos corren antes de que el padre
  // enganche su `ref`, y el hueco es del editor, que está por encima. Con uno de
  // maquetación la burbuja no salía nunca (lo cazó `CapaCamara.test.tsx`).
  useEffect(() => {
    const a = ancla.current;
    const h = hueco.current;
    if (!a || !h) return;
    const medir = () => {
      const ca = a.getBoundingClientRect();
      const ch = h.getBoundingClientRect();
      if (ca.width === 0 || ca.height === 0) {
        setSitio(null);
        return;
      }
      const nuevo = {
        left: ca.left - ch.left - aire,
        top: ca.top - ch.top - aire,
        ancho: ca.width + aire * 2,
        alto: ca.height + aire * 2,
      };
      setSitio((antes) =>
        antes &&
        Math.abs(antes.left - nuevo.left) < 0.5 &&
        Math.abs(antes.top - nuevo.top) < 0.5 &&
        Math.abs(antes.ancho - nuevo.ancho) < 0.5 &&
        Math.abs(antes.alto - nuevo.alto) < 0.5
          ? antes
          : nuevo,
      );
    };
    medir();
    const observador = new ResizeObserver(medir);
    observador.observe(a);
    observador.observe(h);
    return () => observador.disconnect();
  }, [hueco, aire]);

  // La sesión trae cámara y nadie la ha tocado: sale la de fábrica.
  useEffect(() => {
    if (camara && marco === undefined && sitio) onMarco(marcoDeFabrica(sitio.ancho, sitio.alto));
  }, [camara, marco, sitio, onMarco]);

  // La cara al compás del vídeo de la pantalla, a la velocidad de la pantalla y no de React.
  useEffect(() => {
    if (!camara) return;
    let id = 0;
    const vuelta = () => {
      id = requestAnimationFrame(vuelta);
      const v = cara.current;
      if (!v) return;
      const pantalla = videoRef.current;
      const ms = pantalla ? pantalla.currentTime * 1000 : parado.current;
      const quiere = Math.max(0, instanteDeLaCamara(ms, camara.primerMs, camara.desfaseMs) / 1000);
      const dura = Number.isFinite(v.duration) ? v.duration : Infinity;
      // Acabada la cámara se queda en su última imagen, como en el archivo.
      const meta = Math.min(quiere, Math.max(0, dura - 0.04));
      const suena = pantalla && !pantalla.paused && !pantalla.seeking && quiere < dura;
      if (!suena) {
        if (!v.paused) v.pause();
        if (!v.seeking && Math.abs(v.currentTime - meta) > 0.02) v.currentTime = meta;
        return;
      }
      if (v.playbackRate !== pantalla.playbackRate) v.playbackRate = pantalla.playbackRate;
      if (!v.seeking && Math.abs(v.currentTime - meta) > DESVIO_MAXIMO_S) v.currentTime = meta;
      if (v.paused) void v.play().catch(() => undefined);
    };
    id = requestAnimationFrame(vuelta);
    return () => cancelAnimationFrame(id);
  }, [camara, videoRef]);

  const url = useMemo(() => (camara ? convertFileSrc(camara.ruta) : null), [camara]);
  const mostrado = vivo ?? marco ?? null;
  const lienzo: Medida | null = sitio ? { ancho: sitio.ancho, alto: sitio.alto } : null;
  const medidaCamara: Medida | null = camara ? { ancho: camara.ancho, alto: camara.alto } : null;

  // El ancla mide la imagen; se pinta siempre, también sin cámara, para no perder la medida.
  const anclaDiv = <div ref={ancla} aria-hidden="true" className="pointer-events-none absolute inset-0" />;
  if (!camara || !mostrado || !lienzo || !medidaCamara || !sitio || !url || !hueco.current) return anclaDiv;

  const c = caja(mostrado, medidaCamara, lienzo);
  const v = recorteVisible(mostrado, medidaCamara);
  const anchoVideo = c.ancho / (v.x2 - v.x1);
  const altoVideo = c.alto / (v.y2 - v.y1);
  const r = radio(mostrado.forma, c);
  const hacia = esquinaDeAgarre(c, lienzo);
  // El asa: en el círculo, sobre su borde en la diagonal; en lo demás, en la esquina.
  const diagonal = mostrado.forma === "circulo" ? Math.SQRT1_2 : 1;
  const asaX = c.ancho / 2 + (hacia.haciaX * c.ancho * diagonal) / 2;
  const asaY = c.alto / 2 + (hacia.haciaY * c.alto * diagonal) / 2;

  const empezar = (e: React.PointerEvent, modo: Arrastre["modo"]) => {
    if (quieta || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    arrastre.current = { modo, x0: e.clientX, y0: e.clientY, antes: c, marco: mostrado, hacia };
  };

  const seguir = (e: React.PointerEvent) => {
    const a = arrastre.current;
    if (!a) return;
    const dx = e.clientX - a.x0;
    const dy = e.clientY - a.y0;
    let nuevo: MarcoCamara;
    if (a.modo === "mover") {
      nuevo = mover(a.marco, medidaCamara, lienzo, a.antes.x + a.antes.ancho / 2 + dx, a.antes.y + a.antes.alto / 2 + dy);
    } else {
      const esquinaX = a.hacia.haciaX > 0 ? a.antes.x + a.antes.ancho : a.antes.x;
      const esquinaY = a.hacia.haciaY > 0 ? a.antes.y + a.antes.alto : a.antes.y;
      nuevo = redimensionar(a.marco, medidaCamara, lienzo, a.antes, a.hacia, { x: esquinaX + dx, y: esquinaY + dy });
    }
    ultimo.current = nuevo;
    setVivo(nuevo);
  };

  const soltar = () => {
    if (!arrastre.current) return;
    arrastre.current = null;
    if (ultimo.current) onMarco(ultimo.current);
    ultimo.current = null;
    setVivo(null);
  };

  const arrastrando = vivo !== null;
  const borde = mostrado.borde ? grosorDelBorde(c) : 0;

  return (
    <>
      {anclaDiv}
      {createPortal(
        <div
          data-capa-camara
          className="pointer-events-none absolute"
          style={{ left: sitio.left, top: sitio.top, width: sitio.ancho, height: sitio.alto }}
        >
          <div
            data-camara
            role="group"
            aria-label={t("Mover la cámara")}
            title={quieta ? undefined : t("Mover la cámara")}
            onPointerDown={(e) => empezar(e, "mover")}
            onPointerMove={seguir}
            onPointerUp={soltar}
            onPointerCancel={soltar}
            className={`group absolute touch-none select-none ${
              quieta ? "pointer-events-none" : arrastrando ? "pointer-events-auto cursor-grabbing" : "pointer-events-auto cursor-grab"
            }`}
            style={{ left: c.x, top: c.y, width: c.ancho, height: c.alto }}
          >
            <div
              className="absolute inset-0 overflow-hidden bg-black"
              style={{ borderRadius: r }}
            >
              <video
                ref={cara}
                src={url}
                muted
                playsInline
                preload="auto"
                disablePictureInPicture
                aria-hidden="true"
                className="absolute max-w-none object-fill"
                style={{ left: -v.x1 * anchoVideo, top: -v.y1 * altoVideo, width: anchoVideo, height: altoVideo }}
              />
            </div>
            {/* El borde blanco por dentro y la raya oscura por fuera, como los pinta Rust; y
                al pasar el ratón, el contorno azul que dice que se puede agarrar. */}
            <div
              aria-hidden="true"
              className={`pointer-events-none absolute inset-0 outline-2 outline-offset-2 transition-[outline-color] duration-150 ${
                arrastrando ? "outline-blue-500" : quieta ? "outline-transparent" : "outline-transparent group-hover:outline-blue-500/80"
              }`}
              style={{
                borderRadius: r,
                outlineStyle: "solid",
                boxShadow: borde ? `inset 0 0 0 ${borde}px #fff, 0 0 0 1px rgb(0 0 0 / 0.32)` : undefined,
              }}
            />
            {!quieta && (
              <div
                data-asa-camara
                role="slider"
                aria-label={t("Cambiar el tamaño de la cámara")}
                aria-valuemin={8}
                aria-valuemax={50}
                aria-valuenow={Math.round(mostrado.tam * 100)}
                title={t("Cambiar el tamaño de la cámara")}
                // Solo el principio: lo que viene después sube hasta la burbuja, que lo sigue.
                onPointerDown={(e) => empezar(e, "tamano")}
                className={`absolute size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-blue-500 bg-white shadow-md transition-opacity duration-150 ${
                  arrastrando ? "opacity-100" : "opacity-0 group-hover:opacity-100"
                } ${hacia.haciaX * hacia.haciaY > 0 ? "cursor-nwse-resize" : "cursor-nesw-resize"}`}
                style={{ left: asaX, top: asaY }}
              />
            )}
          </div>
        </div>,
        hueco.current,
      )}
    </>
  );
}
