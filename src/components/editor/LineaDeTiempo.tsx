import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as EventoDeTecla,
  type ReactNode,
  type RefObject,
} from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Gauge, Pin, Scissors, Search } from "lucide-react";
import type { FrameMeta } from "../../lib/types";
import type { TramoZoom } from "../../lib/contrato";
import {
  acelerarEscritura,
  cambiarPieza,
  duracionDe,
  duracionQueSale,
  formatoTiempo,
  limitarTramo,
  muestrasDeMiniaturas,
  partir,
  piezaEn,
  sePuedePartir,
  tramosDeEnvio,
  type Pieza,
} from "../../lib/tramo";
import { anadir, bloqueNuevo, cambiar, estirar, mover, NIVEL_NUEVO, quitar } from "../../lib/zooms";
import { useT } from "../../lib/i18n";
import { AjustesDelTrozo, AjustesDelZoom } from "./AjustesDeLaPista";

/** Lo que se está arrastrando: una de las dos asas o el cabezal. */
type Arrastre = "desde" | "hasta" | "cabezal" | null;

/** Lo que se hace con un bloque de zoom al arrastrarlo: moverlo entero o tirar de un borde. */
type ArrastreDeZoom = {
  indice: number;
  que: "mover" | "desde" | "hasta";
  /** Dónde se cogió, para mover lo mismo que se ha movido el ratón. */
  inicioX: number;
  bloque: TramoZoom;
  movido: boolean;
};

/** Lo que mide de ancho cada asa. Asoman por fuera de lo que se queda, no lo tapan. */
const ASA = 14;
/** Lo que mide el borde de un bloque de zoom que se coge para estirarlo. */
const BORDE_DE_ZOOM = 8;
/** La velocidad de «Acelerar lo escrito»: lo bastante para saltarse la espera, y aún se lee. */
export const VELOCIDAD_DE_LO_ESCRITO = 4;

interface Props {
  frames: FrameMeta[];
  /** Lo que se queda, en milisegundos de la grabación. */
  desdeMs: number;
  hastaMs: number;
  /** Dónde está la vista previa cuando no suena. Mientras suena, manda el vídeo. */
  posicionMs: number;
  /**
   * El vídeo, para seguirlo mientras suena sin pasar por React: el cabezal y el reloj se
   * mueven sesenta veces por segundo, y repintar el editor entero a ese ritmo es lo que
   * hace que una línea de tiempo vaya a saltos.
   */
  videoRef: RefObject<HTMLVideoElement | null>;
  reproduciendo: boolean;
  /** Ancho entre alto de lo grabado, para que las miniaturas no salgan deformadas. */
  proporcion: number;
  /** El botón de reproducir, o lo que haya en su sitio mientras se prepara el vídeo. */
  delante: ReactNode;
  onTramo: (desdeMs: number, hastaMs: number, mueve: "desde" | "hasta" | "las dos") => void;
  /** Llevar la vista previa a ese instante. */
  onIr: (ms: number) => void;
  /** Se ha llegado al final de lo que se queda mientras sonaba. */
  onFinal: () => void;
  /** Los bloques de la pista de zooms, en orden. Sin `onZooms` no hay pista. */
  zooms?: TramoZoom[];
  onZooms?: (zooms: TramoZoom[]) => void;
  /** El bloque elegido, cuyos mandos salen debajo. */
  elegido?: number | null;
  onElegir?: (indice: number | null) => void;
  /** Lo que se acerca un bloque recién puesto. */
  nivelNuevo?: number;
  /** Dónde estaba el ratón en ese instante, de 0 a 1: ahí mira un bloque recién puesto. */
  puntoEn?: (ms: number) => { x: number; y: number };
  /** Los trozos entre cortes de tijera. Sin `onPiezas` no hay tijeras. */
  piezas?: Pieza[];
  onPiezas?: (piezas: Pieza[]) => void;
  /** Los ratos en que se escribió, para acelerarlos de una vez. */
  escritura?: { desdeMs: number; hastaMs: number }[];
}

/**
 * La línea de tiempo del editor: lo grabado a lo ancho, dos asas para quedarse con un
 * trozo, la pista de zooms debajo y las tijeras para quitar o acelerar trozos del medio.
 *
 * Sustituye a la tira de antes, que ponía cada fotograma a 56 px y se desplazaba de lado:
 * un minuto de grabación medía cien mil píxeles, y para mover el final había que ir a
 * buscarlo con la rueda. Aquí la grabación entera cabe siempre en el ancho de la ventana,
 * se reparte por TIEMPO (diez segundos quietos ocupan lo que ocupan diez segundos) y las
 * asas se cogen sin apuntar: asoman por fuera, miden lo que un dedo, y mientras se
 * arrastran la vista previa enseña el sitio exacto del corte.
 */
export function LineaDeTiempo({
  frames,
  desdeMs,
  hastaMs,
  posicionMs,
  videoRef,
  reproduciendo,
  proporcion,
  delante,
  onTramo,
  onIr,
  onFinal,
  zooms = [],
  onZooms,
  elegido = null,
  onElegir,
  nivelNuevo = NIVEL_NUEVO,
  puntoEn,
  piezas,
  onPiezas,
  escritura = [],
}: Props) {
  const t = useT();
  const pista = useRef<HTMLDivElement>(null);
  const cabezal = useRef<HTMLDivElement>(null);
  const reloj = useRef<HTMLSpanElement>(null);
  const [arrastre, setArrastre] = useState<Arrastre>(null);
  /** El bloque que se está arrastrando, y desde dónde. El estado solo dice cuál, para pintarlo. */
  const arrastreDeZoom = useRef<ArrastreDeZoom | null>(null);
  const [zoomCogido, setZoomCogido] = useState<number | null>(null);
  const [anchoDePista, setAnchoDePista] = useState(0);
  const total = Math.max(1, duracionDe(frames));

  // Lo último que se sabe, para los oyentes de la ventana y el bucle del cabezal, que
  // viven más que un pintado y no pueden quedarse con los valores de cuando nacieron.
  const vivo = useRef({ desdeMs, hastaMs, onTramo, onIr, onFinal, zooms, onZooms, onElegir, elegido, piezas });
  vivo.current = { desdeMs, hastaMs, onTramo, onIr, onFinal, zooms, onZooms, onElegir, elegido, piezas };

  useLayoutEffect(() => {
    const el = pista.current;
    if (!el) return;
    const medir = () => setAnchoDePista(el.getBoundingClientRect().width);
    medir();
    const observador = new ResizeObserver(medir);
    observador.observe(el);
    return () => observador.disconnect();
  }, []);

  /**
   * Cuántas miniaturas caben sin deformarse, y cuál va en cada hueco.
   *
   * Son siempre unas pocas, las que caben, y no una por fotograma: con diez minutos de
   * grabación la tira de antes montaba dieciocho mil imágenes en la página.
   */
  const huecos = Math.max(1, Math.ceil(anchoDePista / Math.max(24, 56 * proporcion)));
  const muestras = useMemo(() => muestrasDeMiniaturas(frames, huecos), [frames, huecos]);

  /**
   * El instante que hay debajo del puntero, metido dentro de la grabación. La pista de
   * zooms va justo debajo de la tira y mide lo mismo, así que vale para las dos.
   */
  const msEn = useCallback(
    (clientX: number) => {
      const caja = pista.current?.getBoundingClientRect();
      if (!caja || caja.width <= 0) return 0;
      const parte = Math.min(1, Math.max(0, (clientX - caja.left) / caja.width));
      return Math.round(parte * total);
    },
    [total],
  );

  useEffect(() => {
    if (!arrastre) return;
    const mover = (e: PointerEvent) => {
      const ms = msEn(e.clientX);
      const { desdeMs: d, hastaMs: h } = vivo.current;
      if (arrastre === "cabezal") {
        vivo.current.onIr(ms);
        return;
      }
      const nuevo =
        arrastre === "desde" ? limitarTramo(ms, h, total, "desde") : limitarTramo(d, ms, total, "hasta");
      vivo.current.onTramo(nuevo.desdeMs, nuevo.hastaMs, arrastre);
    };
    const soltar = () => setArrastre(null);
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", soltar);
    window.addEventListener("pointercancel", soltar);
    return () => {
      window.removeEventListener("pointermove", mover);
      window.removeEventListener("pointerup", soltar);
      window.removeEventListener("pointercancel", soltar);
    };
  }, [arrastre, msEn, total]);

  /**
   * Arrastrar un bloque de zoom: entero, o de un borde. Se mueve lo mismo que el ratón, en
   * tiempo, y nunca encima de otro bloque. Un clic sin mover solo lo elige.
   */
  useEffect(() => {
    const cogido = arrastreDeZoom.current;
    if (zoomCogido === null || !cogido) return;
    const { indice, que, inicioX, bloque } = cogido;
    const moverRaton = (e: PointerEvent) => {
      const { zooms: lista, onZooms: cambiarZooms } = vivo.current;
      if (!cambiarZooms || !lista[indice]) return;
      // Un temblor de la mano al hacer clic no mueve nada: el clic solo elige.
      if (!cogido.movido && Math.abs(e.clientX - inicioX) < 3) return;
      cogido.movido = true;
      const delta = msEn(e.clientX) - msEn(inicioX);
      cambiarZooms(
        que === "mover"
          ? mover(lista, indice, bloque.desdeMs + delta, total)
          : estirar(lista, indice, que, (que === "desde" ? bloque.desdeMs : bloque.hastaMs) + delta, total),
      );
    };
    const soltar = () => {
      arrastreDeZoom.current = null;
      setZoomCogido(null);
    };
    window.addEventListener("pointermove", moverRaton);
    window.addEventListener("pointerup", soltar);
    window.addEventListener("pointercancel", soltar);
    return () => {
      window.removeEventListener("pointermove", moverRaton);
      window.removeEventListener("pointerup", soltar);
      window.removeEventListener("pointercancel", soltar);
    };
  }, [zoomCogido, msEn, total]);

  /**
   * Con un bloque elegido, Supr lo quita y Escape lo suelta. Se escucha antes que el
   * editor (en la captura): para el editor, Escape a secas es cerrar, y cerrar tira la
   * grabación. Soltar un bloque no puede costar eso.
   */
  useEffect(() => {
    if (elegido === null) return;
    const tecla = (e: KeyboardEvent) => {
      const etiqueta = (e.target as HTMLElement | null)?.tagName;
      if (etiqueta === "INPUT" || etiqueta === "TEXTAREA") return;
      const { zooms: lista, onZooms: cambiarZooms, onElegir: elegir, elegido: cual } = vivo.current;
      if (cual === null) return;
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        e.stopPropagation();
        cambiarZooms?.(quitar(lista, cual));
        elegir?.(null);
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        elegir?.(null);
      }
    };
    window.addEventListener("keydown", tecla, true);
    return () => window.removeEventListener("keydown", tecla, true);
  }, [elegido]);

  /**
   * Mientras suena, el cabezal y el reloj siguen al vídeo en cada pintada de la pantalla,
   * tocando el DOM a mano. Y al llegar al final de lo que se queda, se avisa: el aviso de
   * tiempo del propio vídeo llega cuatro veces por segundo, y con él el corte se oía hasta
   * un cuarto de segundo más allá de donde se soltó el asa.
   *
   * Con tijeras, suena como va a salir: lo quitado se salta, y lo acelerado va a su
   * velocidad y callado, como en el archivo.
   */
  useEffect(() => {
    if (!reproduciendo) return;
    let id = 0;
    let callado = false;
    const paso = () => {
      const video = videoRef.current;
      if (video) {
        let ms = video.currentTime * 1000;
        const trozos = vivo.current.piezas;
        if (trozos && trozos.length > 1) {
          let i = piezaEn(trozos, ms);
          if (trozos[i]?.cortada) {
            while (i < trozos.length && trozos[i].cortada) i++;
            ms = i < trozos.length ? trozos[i].desdeMs : total;
            if (ms < vivo.current.hastaMs) video.currentTime = ms / 1000;
          }
          const velocidad = trozos[i]?.velocidad ?? 1;
          if (video.playbackRate !== velocidad) video.playbackRate = velocidad;
          const callar = velocidad !== 1;
          if (callar !== callado && (callar || callado)) {
            video.muted = callar;
            callado = callar;
          }
        }
        if (ms >= vivo.current.hastaMs) {
          vivo.current.onFinal();
          return;
        }
        if (cabezal.current) cabezal.current.style.left = `${(ms / total) * 100}%`;
        if (reloj.current) reloj.current.textContent = formatoTiempo(ms);
      }
      id = requestAnimationFrame(paso);
    };
    id = requestAnimationFrame(paso);
    return () => {
      cancelAnimationFrame(id);
      // Parado, el vídeo vuelve a su velocidad y a su sonido.
      const video = videoRef.current;
      if (video && video.playbackRate !== 1) video.playbackRate = 1;
      if (video && callado) video.muted = false;
    };
  }, [reproduciendo, total, videoRef]);

  /** Las flechas mueven el asa que tiene el foco: una décima, o un segundo con Mayús. */
  const teclaEnAsa = (lado: "desde" | "hasta") => (e: EventoDeTecla<HTMLDivElement>) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    // Que no le llegue al editor, que con las flechas mueve el cabezal.
    e.preventDefault();
    e.stopPropagation();
    const paso = (e.shiftKey ? 1000 : 100) * (e.key === "ArrowLeft" ? -1 : 1);
    const nuevo =
      lado === "desde"
        ? limitarTramo(desdeMs + paso, hastaMs, total, "desde")
        : limitarTramo(desdeMs, hastaMs + paso, total, "hasta");
    onTramo(nuevo.desdeMs, nuevo.hastaMs, lado);
  };

  /** Dónde está el cabezal ahora mismo: mientras suena lo sabe el vídeo, no React. */
  const ahora = () => {
    const video = videoRef.current;
    return reproduciendo && video ? Math.round(video.currentTime * 1000) : posicionMs;
  };

  const pct = (ms: number) => `${(Math.min(total, Math.max(0, ms)) / total) * 100}%`;
  const conTijeras = Boolean(piezas && onPiezas);
  const trozos = piezas ?? [];
  const editado = conTijeras && tramosDeEnvio(trozos, desdeMs, hastaMs) !== null;
  const entero = desdeMs <= 0 && hastaMs >= total && !editado;
  const sale = conTijeras ? duracionQueSale(trozos, desdeMs, hastaMs) : hastaMs - desdeMs;
  const bajoElCabezal = trozos.length > 1 ? piezaEn(trozos, posicionMs) : -1;
  /** Si al quitar ese trozo quedaría algo: el último que se queda no se quita. */
  const quedaOtro = (i: number) =>
    trozos.some(
      (p, k) => k !== i && !p.cortada && Math.min(p.hastaMs, hastaMs) > Math.max(p.desdeMs, desdeMs),
    );
  const zoomElegido = elegido !== null && onZooms ? zooms[elegido] : undefined;

  /** Un clic en la pista de zooms pone uno ahí; con uno elegido, primero lo suelta. */
  const pulsarPistaDeZoom = (clientX: number) => {
    if (!onZooms) return;
    if (elegido !== null) {
      onElegir?.(null);
      return;
    }
    const ms = msEn(clientX);
    const nuevo = bloqueNuevo(zooms, ms, total, puntoEn?.(ms) ?? { x: 0.5, y: 0.5 }, nivelNuevo);
    if (!nuevo) return;
    const { zooms: lista, indice } = anadir(zooms, nuevo);
    onZooms(lista);
    onElegir?.(indice);
  };

  return (
    <section className="shrink-0 border-t border-white/8 bg-black/25 px-4 pt-3 pb-3">
      <div className="flex items-start gap-2">
        <div className="flex h-14 shrink-0 items-center gap-1.5">
          {delante}
          {/* Dónde está la vista previa. Mientras suena lo escribe el bucle de arriba. */}
          <span
            ref={reloj}
            className="w-[58px] text-[14px] font-medium tabular-nums text-neutral-100"
          >
            {formatoTiempo(posicionMs)}
          </span>
        </div>

        <div className="min-w-0 flex-1" style={{ paddingInline: ASA }}>
          <div
            ref={pista}
            data-pista
            onPointerDown={(e) => {
              // Pulsar en la tira lleva ahí la vista previa, y arrastrar la recorre.
              onIr(msEn(e.clientX));
              setArrastre("cabezal");
            }}
            className="relative h-14 cursor-pointer touch-none select-none"
          >
            <div className="absolute inset-0 flex overflow-hidden rounded-[5px] bg-white/5">
              {muestras.map((indice, hueco) => {
                const ruta = frames[indice]?.thumbPath;
                return ruta ? (
                  <img
                    key={hueco}
                    src={convertFileSrc(ruta)}
                    alt=""
                    draggable={false}
                    className="h-full min-w-0 flex-1 object-cover"
                  />
                ) : (
                  // Las miniaturas se hacen después de abrir el editor: hasta entonces,
                  // huecos que laten en vez de imágenes rotas.
                  <div
                    key={hueco}
                    data-miniatura-pendiente
                    className="h-full min-w-0 flex-1 animate-pulse border-r border-black/30 bg-white/5"
                  />
                );
              })}
            </div>

            {/*
              Los trozos de las tijeras, encima de las miniaturas: lo quitado tapado y rayado,
              lo acelerado con su velocidad escrita, y una raya blanca en cada corte.
            */}
            {trozos.length > 1 &&
              trozos.map((p, i) => (
                <div
                  key={`${p.desdeMs}-${p.hastaMs}`}
                  data-trozo={i}
                  data-cortado={p.cortada || undefined}
                  className="pointer-events-none absolute inset-y-0"
                  style={{ left: pct(p.desdeMs), width: pct(p.hastaMs - p.desdeMs) }}
                >
                  {p.cortada && (
                    <div className="absolute inset-0 bg-black/75 bg-[repeating-linear-gradient(135deg,transparent_0_6px,rgba(255,255,255,0.09)_6px_8px)]" />
                  )}
                  {!p.cortada && p.velocidad !== 1 && (
                    <span className="absolute top-1 left-1.5 text-[13px] font-semibold tabular-nums text-white [text-shadow:0_1px_3px_rgba(0,0,0,0.9)]">
                      {p.velocidad}×
                    </span>
                  )}
                  {i > 0 && <div className="absolute inset-y-0 left-0 w-[2px] -translate-x-1/2 bg-white/85" />}
                </div>
              ))}

            {/* Lo que se tira, apagado, y lo que se queda, con su marco. */}
            <div
              className="pointer-events-none absolute inset-y-0 left-0 rounded-l-[5px] bg-black/70"
              style={{ width: pct(desdeMs) }}
            />
            <div
              className="pointer-events-none absolute inset-y-0 right-0 rounded-r-[5px] bg-black/70"
              style={{ left: pct(hastaMs) }}
            />
            <div
              className="pointer-events-none absolute -inset-y-px border-y-[3px] border-blue-500"
              style={{ left: pct(desdeMs), right: `calc(100% - ${pct(hastaMs)})` }}
            />

            <Asa
              lado="desde"
              ms={desdeMs}
              total={total}
              left={`calc(${pct(desdeMs)} - ${ASA}px)`}
              arrastrando={arrastre === "desde"}
              etiqueta={t("Principio: arrástralo para recortar (tecla I)")}
              onCoger={() => setArrastre("desde")}
              onTecla={teclaEnAsa("desde")}
            />
            <Asa
              lado="hasta"
              ms={hastaMs}
              total={total}
              left={pct(hastaMs)}
              arrastrando={arrastre === "hasta"}
              etiqueta={t("Final: arrástralo para recortar (tecla O)")}
              onCoger={() => setArrastre("hasta")}
              onTecla={teclaEnAsa("hasta")}
            />

            <div
              ref={cabezal}
              data-cabezal
              className="pointer-events-none absolute -top-1.5 -bottom-1.5 z-20 w-[3px] -translate-x-1/2 rounded-full bg-white shadow-[0_0_0_1px_rgba(0,0,0,0.45)]"
              style={{ left: pct(posicionMs) }}
            />
          </div>

          {onZooms && (
            <div
              data-pista-zoom
              aria-label={t("Pista de zooms")}
              onPointerDown={(e) => {
                if (e.button !== 0) return;
                pulsarPistaDeZoom(e.clientX);
              }}
              className="relative mt-2 h-9 cursor-copy touch-none rounded-[5px] bg-white/[0.04] select-none"
            >
              {zooms.length === 0 && (
                <span className="pointer-events-none absolute inset-0 flex items-center px-3 text-[13px] text-neutral-300">
                  {t("Haz clic aquí para poner un zoom")}
                </span>
              )}
              {zooms.map((z, i) => (
                <BloqueDeZoom
                  key={i}
                  zoom={z}
                  izquierda={pct(z.desdeMs)}
                  ancho={pct(z.hastaMs - z.desdeMs)}
                  elegido={elegido === i}
                  arrastrando={zoomCogido === i}
                  onCoger={(clientX, que) => {
                    onElegir?.(i);
                    arrastreDeZoom.current = { indice: i, que, inicioX: clientX, bloque: z, movido: false };
                    setZoomCogido(i);
                  }}
                  onQuitar={() => {
                    onZooms(quitar(zooms, i));
                    onElegir?.(null);
                  }}
                />
              ))}
            </div>
          )}

          <div className="mt-2 flex min-h-8 items-center justify-between gap-3 text-[13px]">
            {zoomElegido ? (
              <div className="min-w-0 flex-1">
                <AjustesDelZoom
                  zoom={zoomElegido}
                  onCambio={(cambio) => onZooms?.(cambiar(zooms, elegido!, cambio))}
                  onQuitar={() => {
                    onZooms?.(quitar(zooms, elegido!));
                    onElegir?.(null);
                  }}
                  onListo={() => onElegir?.(null)}
                />
              </div>
            ) : (
              <>
                <div className="flex min-w-0 items-center gap-4">
                  {bajoElCabezal >= 0 && onPiezas ? (
                    <AjustesDelTrozo
                      pieza={trozos[bajoElCabezal]}
                      numero={bajoElCabezal + 1}
                      cuantas={trozos.length}
                      puedeQuitarse={quedaOtro(bajoElCabezal)}
                      onCambio={(cambio) => onPiezas(cambiarPieza(trozos, bajoElCabezal, cambio))}
                    />
                  ) : entero ? (
                    <span className="text-neutral-300">{t("Arrastra los bordes azules para recortar")}</span>
                  ) : (
                    <span className="text-neutral-300">
                      {t("Te quedas con {trozo} de {total}", {
                        trozo: formatoTiempo(sale),
                        total: formatoTiempo(total),
                      })}
                    </span>
                  )}
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  {onPiezas && (
                    <BotonDeLinea
                      onClick={() => onPiezas(partir(trozos, ahora()))}
                      apagado={!sePuedePartir(trozos, posicionMs)}
                      titulo={t("Corta el vídeo donde está el cabezal: después puedes quitar o acelerar cada trozo")}
                    >
                      <Scissors className="size-4" />
                      {t("Cortar aquí")}
                    </BotonDeLinea>
                  )}
                  {onPiezas && escritura.length > 0 && (
                    <BotonDeLinea
                      onClick={() => onPiezas(acelerarEscritura(trozos, escritura, VELOCIDAD_DE_LO_ESCRITO))}
                      titulo={t("Pone a {n}× los ratos en que escribías, sin sonido", { n: VELOCIDAD_DE_LO_ESCRITO })}
                    >
                      <Gauge className="size-4" />
                      {t("Acelerar lo escrito")}
                    </BotonDeLinea>
                  )}
                  {bajoElCabezal >= 0 && (
                    // Con trozos, lo que dura el vídeo que va a salir, que ya no es la grabación.
                    <span className="ml-1 whitespace-nowrap tabular-nums text-neutral-300">
                      {t("Sale de {dura}", { dura: formatoTiempo(sale) })}
                    </span>
                  )}
                  {desdeMs > 0 || hastaMs < total ? (
                    <button
                      type="button"
                      onClick={() => onTramo(0, total, "las dos")}
                      className="rounded-md px-2.5 py-1 font-medium whitespace-nowrap text-neutral-200 transition-colors hover:bg-white/10 hover:text-white"
                    >
                      {t("Usar el vídeo entero")}
                    </button>
                  ) : (
                    bajoElCabezal < 0 && <span className="ml-1 tabular-nums text-neutral-300">{formatoTiempo(total)}</span>
                  )}
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}

/**
 * Un bloque de la pista de zooms: se coge por el medio para moverlo y por los bordes para
 * estirarlo, el clic derecho lo quita, y con el foco, Supr también.
 */
function BloqueDeZoom({
  zoom,
  izquierda,
  ancho,
  elegido,
  arrastrando,
  onCoger,
  onQuitar,
}: {
  zoom: TramoZoom;
  izquierda: string;
  ancho: string;
  elegido: boolean;
  arrastrando: boolean;
  onCoger: (clientX: number, que: "mover" | "desde" | "hasta") => void;
  onQuitar: () => void;
}) {
  const t = useT();
  const nombre = zoom.lupa
    ? t("Lupa {n}×", { n: zoom.lupa.aumento.toFixed(1) })
    : t("Zoom {n}×", { n: zoom.nivel.toFixed(1) });
  const etiqueta = t("{nombre}, de {desde} a {hasta}", {
    nombre: zoom.modo === "fijo" ? `${nombre}, ${t("fijo")}` : nombre,
    desde: formatoTiempo(zoom.desdeMs),
    hasta: formatoTiempo(zoom.hastaMs),
  });
  return (
    <div
      role="button"
      tabIndex={0}
      data-zoom
      aria-label={etiqueta}
      aria-pressed={elegido}
      title={t("Arrástralo para moverlo, tira de un borde para alargarlo; clic derecho o Supr para quitarlo")}
      onPointerDown={(e) => {
        e.stopPropagation();
        if (e.button !== 0) return;
        const caja = e.currentTarget.getBoundingClientRect();
        const dentro = e.clientX - caja.left;
        const que = dentro < BORDE_DE_ZOOM ? "desde" : caja.width - dentro < BORDE_DE_ZOOM ? "hasta" : "mover";
        onCoger(e.clientX, que);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        onQuitar();
      }}
      style={{ left: izquierda, width: ancho }}
      className={`group absolute inset-y-1 flex cursor-grab items-center overflow-hidden rounded-[4px] border-2 px-2 outline-none transition-colors focus-visible:border-blue-200 ${
        elegido ? "border-blue-300 bg-blue-500/45" : "border-blue-400/55 bg-blue-500/25 hover:bg-blue-500/35"
      } ${arrastrando ? "cursor-grabbing" : ""}`}
    >
      {/* Corto, porque un bloque de dos segundos mide poco: el icono dice si es lupa o fijo. */}
      <span className="pointer-events-none flex min-w-0 items-center gap-1 text-[12px] font-semibold whitespace-nowrap tabular-nums text-white">
        {zoom.lupa ? (
          <Search className="size-3.5 shrink-0" aria-hidden="true" />
        ) : (
          zoom.modo === "fijo" && <Pin className="size-3.5 shrink-0" aria-hidden="true" />
        )}
        {/* «2×» y no «2.0×»: en un bloque estrecho cada letra cuenta. */}
        <span className="truncate">{Number((zoom.lupa ? zoom.lupa.aumento : zoom.nivel).toFixed(1))}×</span>
      </span>
      {/* Los bordes que se cogen para estirar, que se ven al pasar por encima. */}
      <span className="pointer-events-none absolute inset-y-1 left-0.5 w-[3px] rounded-full bg-white/0 group-hover:bg-white/70" />
      <span className="pointer-events-none absolute inset-y-1 right-0.5 w-[3px] rounded-full bg-white/0 group-hover:bg-white/70" />
    </div>
  );
}

function BotonDeLinea({
  onClick,
  apagado = false,
  titulo,
  children,
}: {
  onClick: () => void;
  apagado?: boolean;
  titulo: string;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={apagado}
      title={titulo}
      className="flex h-8 items-center gap-1.5 rounded-lg bg-white/10 px-3 text-[13px] font-medium whitespace-nowrap text-neutral-100 transition-colors hover:bg-white/15 hover:text-white disabled:opacity-40"
    >
      {children}
    </button>
  );
}

function Asa({
  lado,
  ms,
  total,
  left,
  arrastrando,
  etiqueta,
  onCoger,
  onTecla,
}: {
  lado: "desde" | "hasta";
  ms: number;
  total: number;
  left: string;
  arrastrando: boolean;
  etiqueta: string;
  onCoger: () => void;
  onTecla: (e: EventoDeTecla<HTMLDivElement>) => void;
}) {
  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label={etiqueta}
      title={etiqueta}
      aria-valuemin={0}
      aria-valuemax={total}
      aria-valuenow={Math.round(ms)}
      aria-valuetext={formatoTiempo(ms)}
      data-asa={lado}
      onPointerDown={(e) => {
        // Que no llegue a la tira, que movería el cabezal en vez de coger el asa.
        e.stopPropagation();
        onCoger();
      }}
      onKeyDown={onTecla}
      style={{ left, width: ASA }}
      className={`absolute -inset-y-px z-10 flex cursor-ew-resize items-center justify-center bg-blue-500 outline-none transition-colors hover:bg-blue-400 focus-visible:bg-blue-400 before:absolute before:inset-y-0 before:-inset-x-2 before:content-[''] ${
        lado === "desde" ? "rounded-l-[5px]" : "rounded-r-[5px]"
      } ${arrastrando ? "bg-blue-400" : ""}`}
    >
      <span className="h-5 w-[3px] rounded-full bg-white/90" />
      {/* Mientras se arrastra, el tiempo exacto del corte encima del asa. */}
      {arrastrando && (
        <span className="pointer-events-none absolute -top-8 left-1/2 -translate-x-1/2 rounded-md bg-neutral-950 px-2 py-1 text-[12px] font-medium whitespace-nowrap tabular-nums text-white shadow-lg">
          {formatoTiempo(ms)}
        </span>
      )}
    </div>
  );
}
