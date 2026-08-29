import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Crop, Pause, Play, Scissors } from "lucide-react";
import {
  discardSession,
  ffmpegAvailable,
  frameImage,
  getSettings,
  sessionFrames,
  sessionInfo,
} from "../../lib/ipc";
import { clamp, formatTimecode } from "../../lib/format";
import {
  EVENTS,
  type AvisoVistaPrevia,
  type FrameMeta,
  type SessionInfo,
  type Settings,
} from "../../lib/types";
import { BarraAnotar } from "./BarraAnotar";
import { CapaAnotaciones } from "./CapaAnotaciones";
import { CapaRecorte } from "./CapaRecorte";
import { COLORES, COLOR_RESALTADO, type Anotacion, type Herramienta } from "../../lib/anotaciones";
import { ExportPanel } from "./ExportPanel";
import { FrameStrip } from "./FrameStrip";
import { PreviewCanvas } from "./PreviewCanvas";
import { medida as medidaDelRecorte, type Recorte } from "../../lib/recorte";
import { contener } from "../../lib/contener";
import { useT } from "../../lib/i18n";

export function EditorApp({ sessionId }: { sessionId: string }) {
  const t = useT();
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [frames, setFrames] = useState<FrameMeta[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [hasFfmpeg, setHasFfmpeg] = useState(false);
  const [inIndex, setInIndex] = useState(0);
  const [outIndex, setOutIndex] = useState(0);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [seekMs, setSeekMs] = useState(0);
  const [error, setError] = useState<string | null>(null);
  /**
   * El vídeo de la vista previa, para poder llamarlo desde el propio clic del botón.
   *
   * Vive aquí y no dentro de `PreviewCanvas` porque quien decide reproducir es el botón:
   * un `play()` disparado desde un efecto llega ya fuera del gesto de la persona.
   */
  const videoRef = useRef<HTMLVideoElement>(null);
  /** Lo que impide reproducir, si algo lo impide. Antes esto no se contaba a nadie. */
  const [falloDeVideo, setFalloDeVideo] = useState<string | null>(null);
  /** La vista previa se estaba escribiendo por detrás y no ha salido. */
  const [previaFallida, setPreviaFallida] = useState(false);
  /** Y lo que lleva escrito, mientras la escriben. */
  const [porCiento, setPorCiento] = useState(0);
  /** Lo dibujado encima, en el orden en que se hizo. */
  const [anotaciones, setAnotaciones] = useState<Anotacion[]>([]);
  const [herramienta, setHerramienta] = useState<Herramienta | null>(null);
  const [colorMarca, setColorMarca] = useState(COLORES[0]);
  const [textoMarca, setTextoMarca] = useState("");
  /** El trozo que se exporta, de 0 a 1. Sin marco puesto sale la captura entera. */
  const [recorte, setRecorte] = useState<Recorte | null>(null);
  const [recortando, setRecortando] = useState(false);
  /**
   * El hueco donde vive la vista previa, medido de verdad.
   *
   * Las capas de dibujar y de recortar tienen que caer EXACTAMENTE encima de la imagen, y
   * la imagen se contiene dentro del hueco dejando franjas. Se mide con un observador en
   * vez de dejárselo a `aspect-ratio` porque esa propiedad, en una caja que ademas tiene
   * contenido, se resuelve de maneras distintas segun el contenedor.
   */
  const hueco = useRef<HTMLDivElement>(null);
  const [medidaDelHueco, setMedidaDelHueco] = useState({ width: 0, height: 0 });

  useEffect(() => {
    if (!sessionId) {
      setError(t("Falta el identificador de sesión"));
      return;
    }
    Promise.all([
      sessionInfo(sessionId),
      sessionFrames(sessionId),
      getSettings(),
      ffmpegAvailable(),
    ])
      .then(([info, frameList, config, ffmpeg]) => {
        setSession(info);
        setFrames(frameList);
        setSettings(config);
        setHasFfmpeg(ffmpeg);
        setOutIndex(Math.max(0, frameList.length - 1));
      })
      .catch((e) => setError(String(e)));
  }, [sessionId]);

  /** El marcador A nunca puede caer fuera de la tira ni pasarse del B. */
  const markIn = useCallback(
    (index: number) => setInIndex(clamp(index, 0, outIndex)),
    [outIndex],
  );

  /** Y el B, ni por debajo del A ni mas alla del ultimo fotograma. */
  const markOut = useCallback(
    (index: number) => setOutIndex(clamp(index, inIndex, Math.max(0, frames.length - 1))),
    [inIndex, frames.length],
  );

  const scrub = useCallback(
    (index: number) => {
      setCurrentIndex(index);
      setSeekMs(frames[index]?.timestampMs ?? 0);
      videoRef.current?.pause();
    },
    [frames],
  );

  /**
   * Reproducir o parar, hablándole al vídeo directamente.
   *
   * El `play()` se lanza aquí dentro, en el mismo clic (o la misma tecla) que lo pide.
   * Antes salía de un efecto que corría después, y eso son dos problemas: la promesa que
   * devuelve se perdía en un `catch` vacío, así que un rechazo dejaba el botón mudo y sin
   * explicación. Ahora, si el navegador dice que no, se ve por qué.
   */
  const togglePlay = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    if (!video.paused) {
      video.pause();
      return;
    }
    if (currentIndex >= outIndex) {
      const vuelta = frames[inIndex]?.timestampMs ?? 0;
      setCurrentIndex(inIndex);
      setSeekMs(vuelta);
      video.currentTime = vuelta / 1000;
    }
    setFalloDeVideo(null);
    void video.play().catch((fallo: unknown) => setFalloDeVideo(String(fallo)));
  }, [currentIndex, outIndex, inIndex, frames]);


  /**
   * Lo que mide el trozo recortado, para ensennarlo en el boton.
   *
   * Sale del tamanno de la region grabada, no del de la vista previa: la vista previa
   * cambia con la ventana y el archivo no.
   */
  const recorteEnPixeles = useMemo(() => {
    if (!recorte || !session) return "";
    const { width, height } = medidaDelRecorte(recorte, session.region.width, session.region.height);
    return `${width} × ${height}`;
  }, [recorte, session]);

  /** Lo que ocupa la imagen dentro del hueco, sin deformarse y sin salirse. */
  const cajaDeLaVista = useMemo(
    () =>
      session
        ? contener(
            // El `p-4` del hueco son 16 px por lado que no puede ocupar la imagen.
            medidaDelHueco.width - 32,
            medidaDelHueco.height - 32,
            session.region.width,
            session.region.height,
          )
        : { width: 0, height: 0 },
    [medidaDelHueco, session],
  );

  /** Lo que dura el recorte de A a B, que es lo unico que se ensenna del tiempo. */
  const keptMs =
    (frames[outIndex]?.timestampMs ?? 0) +
    (frames[outIndex]?.durationMs ?? 0) -
    (frames[inIndex]?.timestampMs ?? 0);

  // El marco del sistema pinta el titulo, asi que lo que decia la barra propia (el tamanno
  // del recorte y lo que dura) se escribe ahi. Ademas es lo que sale en la barra de tareas
  // y al pasar con Alt+Tab, donde antes solo ponia "winshotx · editor".
  useEffect(() => {
    if (!session) return;
    const medida = `${session.region.width} × ${session.region.height}`;
    void getCurrentWindow().setTitle(
      `winshotx · ${t("Editor")} · ${medida} · ${formatTimecode(keptMs)}`,
    );
  }, [session, keptMs, t]);

  /**
   * Cerrar el editor tira la sesion, que son los fotogramas en crudo del disco.
   *
   * Va por `onCloseRequested` y no colgado de un boton propio porque ahora quien cierra es
   * el marco de Windows: su X, Alt+F4 y el menu de la barra de tareas pasan todos por
   * aqui, y colgarlo de un boton dibujado dejaria la carpeta llena en los otros tres.
   */
  const cerrar = useCallback(async () => {
    if (session) await discardSession(session.id);
    await getCurrentWindow().destroy();
  }, [session]);

  useEffect(() => {
    const sinOir = getCurrentWindow().onCloseRequested(async (evento) => {
      evento.preventDefault();
      await cerrar();
    });
    return () => {
      void sinOir.then((fn) => fn());
    };
  }, [cerrar]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.tagName === "INPUT") return;
      const key = e.key.toLowerCase();
      if (e.key === " ") {
        e.preventDefault();
        togglePlay();
      } else if (key === "i") {
        markIn(currentIndex);
      } else if (key === "z" && (e.ctrlKey || e.metaKey)) {
        // Deshacer la última marca. Va antes que las teclas sueltas para que `Ctrl+Z` no
        // caiga en ninguna otra rama.
        e.preventDefault();
        setAnotaciones((previas) => previas.slice(0, -1));
      } else if (key >= "1" && key <= "6" && !e.ctrlKey) {
        // Las seis herramientas de anotar, en el orden en que están en la barra.
        const cual = (["arrow", "box", "text", "highlight", "step", "blur"] as Herramienta[])[
          Number(key) - 1
        ];
        setHerramienta((puesta) => (puesta === cual ? null : cual));
      } else if (key === "c" && !e.ctrlKey && !e.metaKey) {
        // La otra mitad de recortar: A y B recortan el tiempo, esto recorta el espacio.
        setRecortando((puesto) => !puesto);
        setHerramienta(null);
      } else if (key === "o") {
        markOut(currentIndex);
      } else if (e.key === "ArrowLeft") {
        scrub(Math.max(0, currentIndex - 1));
      } else if (e.key === "ArrowRight") {
        scrub(Math.min(frames.length - 1, currentIndex + 1));
      } else if (e.key === "Escape") {
        // Escape sale primero de lo que se este haciendo. Cerrar el editor tira los
        // fotogramas, asi que no puede ser lo que pase al pulsar Escape sin querer
        // mientras se coloca un marco de recorte.
        if (recortando) setRecortando(false);
        else if (herramienta) setHerramienta(null);
        else void cerrar();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // `recortando` y `herramienta` entran aqui porque Escape los mira: sin ellos el
  // manejador se queda con la foto vieja del estado y cierra el editor, tirando los
  // fotogramas, en vez de soltar lo que se estuviera haciendo.
  }, [currentIndex, frames.length, togglePlay, scrub, markIn, markOut, recortando, herramienta, cerrar]);

  const videoUrl = useMemo(
    () => (session?.mp4Path ? convertFileSrc(session.mp4Path) : null),
    [session],
  );

  /**
   * Lo que se rescata de «los últimos segundos» llega sin vídeo de vista previa.
   *
   * Escribirlo cuesta unos 26 ms por fotograma, o sea doce segundos para medio minuto, y
   * hacer esperar todo eso con la ventana en blanco sería peor que abrirla ya. Se escribe
   * por detrás y cuando está listo llega este aviso: se vuelve a pedir la sesión y el play
   * empieza a funcionar sin que nadie tenga que cerrar y volver a abrir nada.
   */
  useEffect(() => {
    const unlisten = listen<AvisoVistaPrevia>(EVENTS.sessionPreview, (e) => {
      if (e.payload.sessionId !== sessionId) return;
      // El aviso llega también cuando ha ido mal, para poder decirlo. Un botón apagado
      // esperando a algo que no viene es exactamente lo que parece una app rota.
      if (e.payload.fallida) {
        setPreviaFallida(true);
        return;
      }
      if (!e.payload.listo) {
        setPorCiento(e.payload.porCiento);
        return;
      }
      void sessionInfo(sessionId).then(setSession).catch(() => undefined);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [sessionId]);
  // Sin MP4 la vista previa es una imagen: la miniatura de 80 px se veria borrosa,
  // asi que se pide el fotograma entero y se sustituye en cuanto llega.
  const [stillPath, setStillPath] = useState<string | null>(null);
  useEffect(() => {
    if (!session || session.mp4Path || !frames[currentIndex]) return;
    let cancelled = false;
    void frameImage(sessionId, currentIndex)
      .then((path) => {
        if (!cancelled) setStillPath(path);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [session, frames, currentIndex, sessionId]);

  const posterUrl = useMemo(() => {
    if (stillPath) return convertFileSrc(stillPath);
    return frames[currentIndex] ? convertFileSrc(frames[currentIndex].thumbPath) : null;
  }, [stillPath, frames, currentIndex]);

  useLayoutEffect(() => {
    const el = hueco.current;
    if (!el) return;
    const medir = () => {
      const caja = el.getBoundingClientRect();
      setMedidaDelHueco({ width: caja.width, height: caja.height });
    };
    medir();
    const observador = new ResizeObserver(medir);
    observador.observe(el);
    return () => observador.disconnect();
  }, [session]);

  const onTime = useCallback(
    (ms: number) => {
      // El video manda el tiempo; se traduce al frame mas cercano de la tira.
      let index = currentIndex;
      while (index + 1 < frames.length && frames[index + 1].timestampMs <= ms) index++;
      while (index > 0 && frames[index].timestampMs > ms) index--;
      if (index !== currentIndex) setCurrentIndex(index);
    },
    [frames, currentIndex],
  );

  if (error) {
    return (
      <div className="flex h-full items-center justify-center bg-[#161618] text-sm text-red-300">
        {error}
      </div>
    );
  }

  if (!session || !settings || frames.length === 0) {
    return (
      <div className="flex h-full items-center justify-center bg-[#161618] text-sm text-neutral-500">
        {t("Preparando la sesión…")}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-[#161618]">
      <div className="flex min-h-0 flex-1">
        <main className="flex min-w-0 flex-1 flex-col">
          <div
            ref={hueco}
            className="relative grid min-h-0 flex-1 place-items-center bg-[repeating-conic-gradient(#1c1c1c_0%_25%,#242424_0%_50%)] bg-[length:20px_20px] p-4"
          >
            {/*
              La caja mide EXACTAMENTE lo que la imagen, no lo que el hueco.

              La vista previa se ajusta con `object-contain`, así que en un hueco de otra
              proporción deja franjas a los lados o arriba y abajo. Las capas iban
              estiradas al hueco entero, y sus coordenadas de 0 a 1 contaban esas franjas
              como parte de la captura: una flecha puesta en el borde de la imagen se
              guardaba un poco más allá, y al exportar aparecía desplazada. Con un vídeo
              vertical en una ventana ancha, el desplazamiento era de media pantalla.

              Dándole aquí la proporción de la captura, la imagen llena la caja y las capas
              caen encima con exactitud, sin que nadie tenga que medir nada.
            */}
            <div
              className="relative"
              style={{
                aspectRatio: `${session.region.width} / ${session.region.height}`,
                // El `aspect-ratio` de arriba es el respaldo mientras no se ha medido nada.
                // Lo que manda son estos dos numeros, calculados sobre el hueco de verdad.
                ...(cajaDeLaVista.width
                  ? { width: cajaDeLaVista.width, height: cajaDeLaVista.height }
                  : { maxWidth: "100%", maxHeight: "100%" }),
              }}
            >
              <PreviewCanvas
                videoRef={videoRef}
                videoUrl={videoUrl}
                posterUrl={posterUrl}
                conSonido={session?.hasAudio ?? false}
                inMs={frames[inIndex]?.timestampMs ?? 0}
                outMs={(frames[outIndex]?.timestampMs ?? 0) + (frames[outIndex]?.durationMs ?? 0)}
                seekMs={seekMs}
                onTime={onTime}
                onEnded={() => setCurrentIndex(inIndex)}
                onPlaying={setPlaying}
                onFallo={setFalloDeVideo}
              />
              <CapaAnotaciones
                herramienta={herramienta}
                // El resaltado es un marcador, y un marcador es amarillo: no se elige.
                color={herramienta === "highlight" ? COLOR_RESALTADO : colorMarca}
                anotaciones={anotaciones}
                onAnadir={(marca) => setAnotaciones((previas) => [...previas, marca])}
                texto={textoMarca}
              />
              <CapaRecorte activa={recortando} recorte={recorte} onRecorte={setRecorte} />
            </div>
            {/*
              Mientras el vídeo no está, se DICE.

              Lo que se rescata del anillo abre el editor antes de tener vídeo, así que
              durante unos segundos el play no puede hacer nada. Un botón apagado sin
              explicación es indistinguible de un botón roto: es literalmente lo que pasó
              el 29 de agosto de 2026, «sigue sin dejarme darle al play».
            */}
            {!videoUrl && (
              <div className="pointer-events-none absolute bottom-14 left-1/2 -translate-x-1/2">
                <span className="rounded-full bg-black/75 px-3 py-1 text-[11px] text-neutral-300 backdrop-blur-md">
                  {previaFallida
                    ? t("No se ha podido preparar la reproducción")
                    : porCiento > 0
                      ? `${t("Preparando la reproducción…")} ${porCiento}%`
                      : t("Preparando la reproducción…")}
                </span>
              </div>
            )}
            {videoUrl && falloDeVideo && (
              <div className="pointer-events-none absolute bottom-14 left-1/2 max-w-[80%] -translate-x-1/2">
                <span className="block truncate rounded-full bg-red-950/80 px-3 py-1 text-[11px] text-red-200 backdrop-blur-md">
                  {t("No se ha podido reproducir")} · {falloDeVideo}
                </span>
              </div>
            )}
            <div className="absolute bottom-3 left-1/2 flex -translate-x-1/2 items-center gap-2 rounded-full border border-white/10 bg-neutral-900/85 px-2 py-1.5 shadow-xl backdrop-blur-md">
              <button
                type="button"
                onClick={togglePlay}
                disabled={!videoUrl}
                title={videoUrl ? undefined : t("preparando la reproducción…")}
                aria-label={playing ? t("Pausar") : t("Reproducir")}
                className="flex size-7 items-center justify-center rounded-full bg-white/10 text-white transition-colors hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-30 disabled:hover:bg-white/10"
              >
                {playing ? <Pause className="size-3.5" /> : <Play className="size-3.5 pl-px" />}
              </button>
              <span className="px-1 font-mono text-[11px] tabular-nums text-neutral-300">
                {formatTimecode(frames[currentIndex]?.timestampMs ?? 0)}
              </span>
              <span className="h-4 w-px bg-white/10" />
              <button
                type="button"
                onClick={() => markIn(currentIndex)}
                className="flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] text-neutral-300 transition-colors hover:bg-white/10 hover:text-white"
                title={t("Marcar inicio (I)")}
              >
                <Scissors className="size-3" /> A
              </button>
              <button
                type="button"
                onClick={() => markOut(currentIndex)}
                className="flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] text-neutral-300 transition-colors hover:bg-white/10 hover:text-white"
                title={t("Marcar final (O)")}
              >
                <Scissors className="size-3 -scale-x-100" /> B
              </button>
              <span className="h-4 w-px bg-white/10" />
              <button
                type="button"
                onClick={() => {
                  setRecortando((puesto) => !puesto);
                  setHerramienta(null);
                }}
                aria-pressed={recortando}
                className={`flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] transition-colors ${
                  recortando || recorte
                    ? "bg-blue-500 text-white"
                    : "text-neutral-300 hover:bg-white/10 hover:text-white"
                }`}
                title={t("Recortar la imagen (C)")}
              >
                <Crop className="size-3" />
                {recorte ? recorteEnPixeles : t("Recortar")}
              </button>
              {recorte && (
                <button
                  type="button"
                  onClick={() => {
                    setRecorte(null);
                    setRecortando(false);
                  }}
                  className="rounded-full px-1.5 py-0.5 text-[11px] text-neutral-400 transition-colors hover:bg-white/10 hover:text-white"
                  title={t("Quitar el recorte")}
                >
                  ×
                </button>
              )}
            </div>
          </div>

          <BarraAnotar
            activa={herramienta}
            onElegir={setHerramienta}
            color={colorMarca}
            onColor={setColorMarca}
            texto={textoMarca}
            onTexto={setTextoMarca}
            cuantas={anotaciones.length}
            onDeshacer={() => setAnotaciones((previas) => previas.slice(0, -1))}
            onBorrarTodo={() => setAnotaciones([])}
          />

          <FrameStrip
            frames={frames}
            inIndex={inIndex}
            outIndex={outIndex}
            currentIndex={currentIndex}
            onChangeIn={markIn}
            onChangeOut={markOut}
            onScrub={scrub}
          />
        </main>

        <ExportPanel
          recorte={recorte}
          anotaciones={anotaciones}
          session={session}
          inIndex={inIndex}
          outIndex={outIndex}
          currentIndex={currentIndex}
          fpsMax={Math.max(15, session.fps)}
          hasFfmpeg={hasFfmpeg}
          saveDirectory={settings.saveDirectory}
        />
      </div>
    </div>
  );
}
