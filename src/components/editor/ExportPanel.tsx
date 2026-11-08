import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { ChevronDown, Clipboard, FolderDown, FolderOpen, Link2, Save, Unlink2 } from "lucide-react";
import { exportMedia, pickDirectory, pickSaveFile, revealInExplorer } from "../../lib/ipc";
import { formatBytes } from "../../lib/format";
import {
  EVENTS,
  type ExportFormat,
  type Background,
  type ExportProgress,
  type ExportResult,
  type SessionInfo,
} from "../../lib/types";
import { NumberField } from "../ui/NumberField";
import { Slider } from "../ui/Slider";
import { Toggle } from "../ui/Toggle";
import { useT } from "../../lib/i18n";
import {
  PROPORCIONES,
  conProporcion,
  esLaProporcion,
  medida as medidaDelRecorte,
  type Recorte,
} from "../../lib/recorte";
import type { Anotacion } from "../../lib/anotaciones";
import { punteroNormal, type Estudio } from "../../lib/estudio";

/**
 * Los fondos del marco, con el color con el que se pintan en el propio botón.
 *
 * Son cinco y no una rueda de color: con dieciséis millones a elegir, la pregunta deja de
 * ser «cuál de estos» y pasa a ser «cuál de todos», que es justo lo que hace que alguien
 * cierre el panel sin exportar nada.
 */
const FONDOS: { id: Background; label: string; muestra: string }[] = [
  { id: "blanco", label: "Blanco", muestra: "#ffffff" },
  { id: "negro", label: "Negro", muestra: "#121214" },
  { id: "gris", label: "Gris", muestra: "#e8e8ea" },
  { id: "atardecer", label: "Atardecer", muestra: "linear-gradient(135deg,#5874f5,#a855d9)" },
  { id: "menta", label: "Menta", muestra: "linear-gradient(135deg,#22c598,#3884e8)" },
];

/**
 * Los cuatro formatos, con lo que hace cada uno dicho en palabras: «H.264 por hardware»
 * no ayuda a nadie a elegir, «se abre en cualquier sitio» sí.
 */
const FORMATS: { id: ExportFormat; label: string; para: string }[] = [
  { id: "mp4", label: "MP4", para: "Vídeo con sonido. Se abre en cualquier sitio." },
  { id: "gif", label: "GIF", para: "Se repite solo y sin sonido. Para pegarlo en un chat." },
  { id: "png", label: "PNG", para: "Solo el fotograma que estás viendo, sin perder nada." },
  { id: "jpg", label: "JPG", para: "Solo el fotograma que estás viendo, mucho más ligero." },
];

/**
 * A qué velocidad se reproduce lo exportado.
 *
 * Cuatro pasos y no un deslizador: acelerar un tutorial es elegir entre «como fue», «sin
 * los tramos muertos» y «el resumen», no afinar un número. Y a cualquier velocidad que no
 * sea la de verdad el vídeo sale mudo, así que cuantas menos formas de acabar ahí sin
 * querer, mejor.
 */
const VELOCIDADES = [0.5, 1, 2, 4];

/** En una captura no hay fotogramas que elegir: es la imagen, y se dice así. */
const PARA_UNA_CAPTURA: Partial<Record<ExportFormat, string>> = {
  png: "La imagen tal cual, sin perder nada.",
  jpg: "La misma imagen, mucho más ligera.",
};

/** Los dos formatos que sacan UN fotograma, no un trozo de grabación. */
const esUnaFoto = (formato: ExportFormat) => formato === "png" || formato === "jpg";

/** Si «Más ajustes» se quedó abierto la última vez. Es comodidad de quien lo usa, nada más. */
const CLAVE_MAS = "winshotx.editor.masAjustes";
function leerMas(): boolean {
  try {
    return localStorage.getItem(CLAVE_MAS) === "1";
  } catch {
    return false;
  }
}

interface Props {
  /** Lo dibujado encima en el editor, que Rust pinta sobre cada fotograma. */
  anotaciones: Anotacion[];
  /** El trozo que se exporta, de 0 a 1. Sin marco puesto sale la captura entera. */
  recorte: Recorte | null;
  session: SessionInfo;
  inIndex: number;
  outIndex: number;
  /**
   * Lo que se queda, en milisegundos de la grabación. Es lo que manda al exportar: los
   * fotogramas `inIndex` y `outIndex` viajan también, pero un corte a mitad de una
   * pantalla quieta solo cabe en milisegundos.
   */
  desdeMs?: number;
  hastaMs?: number;
  currentIndex: number;
  fpsMax: number;
  hasFfmpeg: boolean;
  saveDirectory: string;
  /**
   * Lo que se decide del estudio, cada vez que cambia, para que la vista previa lo
   * dibuje. El panel sigue mandando; esto es un aviso hacia arriba.
   */
  onEstudio?: (estudio: Estudio) => void;
  /** Poner o quitar el marco de recorte desde aquí, con una proporción ya hecha. */
  onRecorte?: (recorte: Recorte | null) => void;
}

/**
 * Guardar lo que se ha hecho.
 *
 * Arriba lo que casi siempre se toca (el formato, el sonido, el zoom y el puntero) y abajo
 * los botones de guardar, siempre a la vista. Todo lo demás (calidad, fotogramas por
 * segundo, velocidad, tamaño, marco) va plegado en «Más ajustes»: estaba todo a la vez,
 * en seis bloques con rótulos en mayúsculas, y para llegar al de abajo había que bajar
 * con la rueda en una ventana normal.
 */
export function ExportPanel({
  anotaciones,
  recorte,
  session,
  inIndex,
  outIndex,
  desdeMs,
  hastaMs,
  currentIndex,
  fpsMax,
  hasFfmpeg,
  saveDirectory,
  onEstudio,
  onRecorte,
}: Props) {
  const t = useT();
  const esCaptura = session.format === "still";
  const [format, setFormat] = useState<ExportFormat>(() => {
    if (session.format === "gif") return "gif";
    // Una sesion de un solo fotograma es una imagen: exportarla a MP4 no tiene sentido.
    if (esCaptura || session.frameCount <= 1) return "png";
    return "mp4";
  });
  const [useFfmpeg, setUseFfmpeg] = useState(false);
  const [quality, setQuality] = useState(80);
  const [fps, setFps] = useState(Math.min(session.fps, 30));
  const [width, setWidth] = useState(session.region.width);
  const [height, setHeight] = useState(session.region.height);
  const [locked, setLocked] = useState(true);
  const [audio, setAudio] = useState(session.hasAudio);
  const [velocidad, setVelocidad] = useState(1);
  /**
   * Cuánto se acerca la cámara a cada clic. 1 es no acercarse.
   *
   * **Apagado de fábrica y a propósito.** Grabando una ventana pequeña no hace falta y
   * marea, y quien no lo espere se encuentra un vídeo que se mueve solo. Los clics quedaron
   * anotados al grabar, así que encenderlo aquí no obliga a repetir nada.
   */
  const [zoom, setZoom] = useState(1);
  /**
   * Los aros de los clics y la pastilla de los atajos.
   *
   * Se dibujan al exportar, así que se deciden aquí. Antes se pintaban sobre los fotogramas
   * mientras se grababa y quedaban cocidos: había que acertar antes de empezar.
   */
  const [aros, setAros] = useState(false);
  const [teclas, setTeclas] = useState(false);
  /**
   * El alto del puntero dibujado, en píxeles. Cero es no dibujarlo.
   *
   * Se dibuja aquí en vez de usar el que capturó Windows, y por eso se puede hacer grande
   * sin pixelarlo. Viene encendido al tamaño normal porque la grabación ya no mete el
   * puntero de Windows en los fotogramas: sin esto el vídeo saldría sin ratón. Solo las
   * grabaciones viejas, que lo llevan cocido, empiezan en cero para no enseñar dos.
   */
  const [cursor, setCursor] = useState(() =>
    session.cursorBaked || esCaptura ? 0 : punteroNormal(session.region.height),
  );
  /** Aire alrededor de la captura. Cero significa sin marco, que es lo de siempre. */
  const [margen, setMargen] = useState(0);
  const [fondo, setFondo] = useState<Background>("blanco");
  const [sombra, setSombra] = useState(true);
  const [loop, setLoop] = useState(true);
  const [directory, setDirectory] = useState(saveDirectory);
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [result, setResult] = useState<ExportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [mas, setMas] = useState(leerMas);

  useEffect(() => {
    try {
      localStorage.setItem(CLAVE_MAS, mas ? "1" : "0");
    } catch {
      // Sin almacenamiento se abre plegado la próxima vez, y ya está.
    }
  }, [mas]);

  useEffect(() => {
    onEstudio?.({ zoom, cursor, aros, teclas });
  }, [zoom, cursor, aros, teclas, onEstudio]);

  /**
   * Lo que mide de verdad lo que se va a exportar, con el recorte ya aplicado.
   *
   * Todo lo de este panel se mide sobre esto y no sobre la region grabada: quien recorta
   * un trozo y ve «1920 x 1200» en las dimensiones ve el tamanno de algo que ya no existe.
   */
  const fuente = useMemo(
    () =>
      recorte
        ? medidaDelRecorte(recorte, session.region.width, session.region.height)
        : { width: session.region.width, height: session.region.height },
    [recorte, session.region.width, session.region.height],
  );

  const aspect = fuente.width / fuente.height;

  /**
   * Poner o quitar el recorte devuelve las dimensiones al tamanno nuevo.
   *
   * Es lo que espera cualquiera, y ademas evita el caso raro: conservar el ancho anterior
   * dejaria un trozo pequenno estirado al tamanno de la captura entera sin que nadie lo
   * haya pedido.
   */
  useEffect(() => {
    setWidth(fuente.width);
    setHeight(fuente.height);
  }, [fuente.width, fuente.height]);

  useEffect(() => {
    const unlisten = listen<ExportProgress>(EVENTS.exportProgress, (e) => {
      // El "done" del backend llega antes de copiar al portapapeles y antes de que el
      // comando devuelva: si soltara aqui la guarda, un segundo clic arrancaria otra
      // exportacion sobre la misma sesion. Quien la suelta es el finally de run().
      if (e.payload.stage !== "done") setProgress(e.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  /** Lo que dura lo que se va a guardar, en segundos de la grabación. */
  const segundos = useMemo(() => {
    if (desdeMs !== undefined && hastaMs !== undefined) return Math.max(0, hastaMs - desdeMs) / 1000;
    return Math.max(1, outIndex - inIndex + 1) / Math.max(1, session.fps);
  }, [desdeMs, hastaMs, inIndex, outIndex, session.fps]);

  const estimate = useMemo(() => {
    // A 2x el vídeo dura la mitad, así que también pesa la mitad.
    const seconds = segundos / velocidad;
    // El marco crece por fuera, así que el archivo también: la estimación cuenta con él.
    const w = width + margen * 2;
    const h = height + margen * 2;
    if (format === "png") return w * h * 3 * 0.35;
    // Medido sobre capturas de verdad: un JPEG de calidad 85 se queda en torno a
    // 0,13 bytes por pixel, que es donde sale este factor.
    if (format === "jpg") return w * h * (quality / 100) * 0.15;
    if (format === "gif") return w * h * (quality / 100) * 0.12 * fps * seconds;
    const bitrate = 1_000_000 + (quality / 100) * 11_000_000;
    return (bitrate / 8) * seconds;
  }, [format, width, height, margen, quality, fps, segundos, velocidad]);

  const run = async (copyToClipboard: boolean, file: string | null = null) => {
    // Sin esto, dos pulsaciones seguidas lanzan dos exportaciones a la vez.
    if (progress !== null) return;
    setError(null);
    setResult(null);
    setProgress({ stage: "reading", done: 0, total: 1 });
    try {
      const foto = esUnaFoto(format);
      const res = await exportMedia({
        sessionId: session.id,
        format,
        engine: useFfmpeg && hasFfmpeg ? "ffmpeg" : "native",
        from: foto ? currentIndex : inIndex,
        to: foto ? currentIndex : outIndex,
        // Una foto es UN fotograma: el tramo de tiempo no pinta nada en ella.
        desdeMs: foto ? null : (desdeMs ?? null),
        hastaMs: foto ? null : (hastaMs ?? null),
        width: Math.round(width),
        height: Math.round(height),
        fps,
        quality,
        // A otra velocidad no hay sonido: acelerar la pista sin cambiarle el tono es otro
        // problema entero, y dejarla tal cual la despegaría de la imagen enseguida.
        audio: audio && format === "mp4" && velocidad === 1,
        speed: foto ? 1 : velocidad,
        // Una foto no tiene zoom: la cámara solo se mueve con el tiempo pasando.
        zoom: foto ? 1 : zoom,
        // Una foto no tiene ni aros ni pastilla: los dos duran un rato y aquí no pasa
        // el tiempo.
        clicks: !foto && aros,
        keys: !foto && teclas,
        cursor: foto ? 0 : cursor,
        loop,
        margin: margen,
        background: fondo,
        shadow: sombra,
        annotations: anotaciones,
        crop: recorte,
        destination: directory || null,
        file,
        copyToClipboard,
      });
      setResult(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setProgress(null);
    }
  };

  /**
   * «Guardar en…»: el diálogo de Windows para elegir carpeta y nombre de una vez, y
   * después la exportación normal a ese archivo. Cancelar el diálogo no exporta nada.
   */
  const runAs = async (copyToClipboard: boolean) => {
    if (progress !== null) return;
    const file = await pickSaveFile(format, directory || null);
    if (!file) return;
    await run(copyToClipboard, file);
  };

  // Ctrl+S exporta sin ir al raton, igual que en el overlay, y Ctrl+Mayús+S pregunta
  // dónde. El ref evita volver a registrar el listener en cada tecleo del panel.
  const runRef = useRef({ run, runAs });
  runRef.current = { run, runAs };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== "s") return;
      e.preventDefault();
      if (e.shiftKey) void runRef.current.runAs(false);
      else void runRef.current.run(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const setW = (value: number) => {
    setWidth(value);
    if (locked) setHeight(Math.max(2, Math.round(value / aspect)));
  };
  const setH = (value: number) => {
    setHeight(value);
    if (locked) setWidth(Math.max(2, Math.round(value * aspect)));
  };

  // Una captura es un fotograma: un MP4 o un GIF de un fotograma no es nada que alguien
  // quiera, así que ni se ofrecen.
  const formatos = esCaptura ? FORMATS.filter((f) => esUnaFoto(f.id)) : FORMATS;
  const foto = esUnaFoto(format);
  const ocupado = progress !== null;

  return (
    <aside className="flex w-[300px] shrink-0 flex-col border-l border-white/8 bg-black/20">
      {/*
        Guardar va justo debajo de los ajustes, y se queda pegado abajo si los ajustes no
        caben.

        Clavado siempre al pie dejaba un hueco en medio con «Más ajustes» plegado; y suelto
        sin más, con todo abierto en una ventana normal, se iba por debajo del borde: la
        acción principal del editor escondida detrás de una rueda de ratón.
      */}
      <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="px-4 pt-3 pb-4">
        {/* El formato, en texto con su raya debajo: es una pestaña, no un interruptor. */}
        <div className="flex gap-5 border-b border-white/8">
          {formatos.map((f) => (
            <button
              key={f.id}
              type="button"
              onClick={() => setFormat(f.id)}
              aria-pressed={format === f.id}
              className={`-mb-px border-b-[3px] pt-1 pb-2 text-[15px] font-semibold transition-colors ${
                format === f.id
                  ? "border-blue-500 text-white"
                  : "border-transparent text-neutral-400 hover:text-neutral-100"
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
        <p className="mt-2.5 text-[13px] leading-snug text-neutral-300">
          {t(
            (esCaptura && PARA_UNA_CAPTURA[format]) ||
              (FORMATS.find((f) => f.id === format)?.para ?? ""),
          )}
        </p>

        <div className="mt-4 space-y-4">
          {format === "mp4" && (
            <Toggle
              checked={audio}
              onChange={setAudio}
              label={t("Sonido")}
              hint={
                !session.hasAudio
                  ? t("esta grabación se hizo sin audio")
                  : velocidad !== 1
                    ? t("a otra velocidad el vídeo sale mudo")
                    : undefined
              }
            />
          )}

          {/* Lo del estudio, solo con tiempo dentro: se dibuja al exportar, y se ve en la
              vista previa según se toca. El zoom solo con clics, que un deslizador que no
              puede hacer nada es peor que no tenerlo; el puntero sale siempre, porque una
              grabación sin clics también tiene ratón. */}
          {!foto && session.hasClicks && (
            <Slider
              label={t("Acercarse a los clics")}
              hint={zoom <= 1.05 ? t("sin zoom") : `${zoom.toFixed(1)}×`}
              min={1}
              max={3}
              step={0.1}
              value={zoom}
              onChange={setZoom}
            />
          )}
          {!foto && (
            <div>
              <Slider
                label={t("Puntero dibujado")}
                hint={
                  cursor === 0
                    ? session.cursorBaked
                      ? t("el de Windows")
                      : t("sin puntero")
                    : `${cursor} px`
                }
                min={0}
                max={80}
                step={4}
                value={cursor}
                onChange={setCursor}
              />
              {cursor > 0 && session.cursorBaked && (
                <p className="mt-1.5 text-[12px] leading-snug text-amber-300">
                  {t("Esta grabación ya lleva el cursor de Windows dentro: vas a ver dos.")}
                </p>
              )}
            </div>
          )}
        </div>

        <button
          type="button"
          onClick={() => setMas((abierto) => !abierto)}
          aria-expanded={mas}
          className="mt-4 flex w-full items-center justify-between rounded-md py-2 text-left text-[14px] font-semibold text-neutral-100 transition-colors hover:text-white"
        >
          {t("Más ajustes")}
          <ChevronDown
            className={`size-[18px] text-neutral-300 transition-transform duration-200 ${mas ? "rotate-180" : ""}`}
          />
        </button>

        {/*
          Plegado con la fila de la rejilla de 0fr a 1fr: crece poco a poco y lo de debajo
          baja con él, en vez de aparecer de golpe. Cerrado no se puede pulsar ni llegar con
          el tabulador a nada de dentro.
        */}
        <div
          className="grid transition-[grid-template-rows] duration-[240ms] ease-in-out"
          style={{ gridTemplateRows: mas ? "1fr" : "0fr" }}
        >
          <div className="min-h-0 overflow-hidden" inert={!mas} aria-hidden={!mas}>
            <div className="space-y-4 pt-1 pb-1">
              {format !== "png" && (
                <Slider
                  label={t("Calidad")}
                  hint={`${quality}%`}
                  min={10}
                  max={100}
                  value={quality}
                  onChange={setQuality}
                />
              )}

              {/* Una foto no tiene fotogramas por segundo, ni velocidad. */}
              {!foto && (
                <Slider
                  label={t("Fotogramas por segundo")}
                  hint={`${fps} fps`}
                  min={5}
                  max={fpsMax}
                  value={fps}
                  onChange={setFps}
                />
              )}

              {!foto && (
                <Grupo titulo={t("Velocidad")}>
                  <Opciones>
                    {VELOCIDADES.map((v) => (
                      <Opcion key={v} puesta={velocidad === v} onClick={() => setVelocidad(v)}>
                        {v}×
                      </Opcion>
                    ))}
                  </Opciones>
                  {velocidad !== 1 && (
                    <p className="mt-1.5 text-[12px] text-neutral-300">
                      {velocidad > 1
                        ? t("dura {n} veces menos, y sale sin sonido", { n: velocidad })
                        : t("dura el doble, y sale sin sonido")}
                    </p>
                  )}
                </Grupo>
              )}

              <Grupo titulo={t("Tamaño")}>
                {/*
                  La proporción, de un clic.

                  Lo que se graba de un monitor sale SIEMPRE apaisado, así que para subir
                  algo vertical hay que encuadrar, y encuadrar a ojo un 9:16 exacto no lo
                  acierta nadie. Esto coloca el trozo más grande con esa forma, centrado, y
                  después se arrastra donde haga falta sin perder la proporción.
                */}
                {onRecorte && (
                  <Opciones>
                    {PROPORCIONES.map((p) => (
                      <Opcion
                        key={p.id}
                        puesta={
                          recorte !== null &&
                          esLaProporcion(p.valor, recorte, session.region.width, session.region.height)
                        }
                        onClick={() =>
                          onRecorte(conProporcion(p.valor, session.region.width, session.region.height))
                        }
                      >
                        {p.id}
                      </Opcion>
                    ))}
                    {recorte !== null && (
                      <button
                        type="button"
                        onClick={() => onRecorte(null)}
                        title={t("Quitar el recorte")}
                        aria-label={t("Quitar el recorte")}
                        className="ml-auto rounded-md px-1.5 text-[16px] leading-none text-neutral-300 transition-colors hover:bg-white/10 hover:text-white"
                      >
                        ×
                      </button>
                    )}
                  </Opciones>
                )}
                <div className="mt-2.5 flex items-end gap-2">
                  <NumberField label={t("Ancho")} value={width} onChange={setW} suffix="px" />
                  <button
                    type="button"
                    onClick={() => setLocked((l) => !l)}
                    title={locked ? t("Proporción bloqueada") : t("Proporción libre")}
                    className={`mb-1.5 flex size-8 shrink-0 items-center justify-center rounded-md transition-colors ${
                      locked ? "text-blue-400 hover:bg-white/10" : "text-neutral-400 hover:text-white"
                    }`}
                  >
                    {locked ? <Link2 className="size-4" /> : <Unlink2 className="size-4" />}
                  </button>
                  <NumberField label={t("Alto")} value={height} onChange={setH} suffix="px" />
                </div>
                <Opciones>
                  {[100, 75, 50].map((pct) => (
                    <Opcion
                      key={pct}
                      puesta={false}
                      onClick={() => {
                        // Sobre lo que se va a exportar, no sobre la captura entera: con un
                        // recorte puesto, el 50 % de la captura estiraria el trozo al doble.
                        setWidth(Math.round((fuente.width * pct) / 100));
                        setHeight(Math.round((fuente.height * pct) / 100));
                      }}
                    >
                      {pct}%
                    </Opcion>
                  ))}
                </Opciones>
              </Grupo>

              {!foto && (
                <div className="space-y-1">
                  {session.hasClicks && (
                    <Toggle checked={aros} onChange={setAros} label={t("Marcar los clics")} />
                  )}
                  <Toggle
                    checked={teclas}
                    onChange={setTeclas}
                    label={t("Enseñar los atajos")}
                    // Este aviso viaja con el interruptor: es la regla que impide que una
                    // contraseña tecleada mientras se grababa acabe dentro del vídeo.
                    hint={t("solo con Ctrl, Alt o Win: lo que escribes no sale")}
                  />
                </div>
              )}

              <Grupo titulo={t("Marco")}>
                <Slider
                  label={t("Aire alrededor")}
                  hint={margen === 0 ? t("sin marco") : `${margen} px`}
                  min={0}
                  max={160}
                  value={margen}
                  onChange={setMargen}
                />
                {/* Los fondos solo aparecen cuando hay aire que pintar: cinco botones de
                    color para el fondo de un marco que no existe son cinco botones de ruido. */}
                {margen > 0 && (
                  <>
                    <div className="mt-2.5 flex gap-1.5">
                      {FONDOS.map((f) => (
                        <button
                          key={f.id}
                          type="button"
                          onClick={() => setFondo(f.id)}
                          title={t(f.label)}
                          aria-label={t(f.label)}
                          aria-pressed={fondo === f.id}
                          style={{ background: f.muestra }}
                          className={`size-8 rounded-md border-2 transition-colors ${
                            fondo === f.id ? "border-blue-500" : "border-white/15 hover:border-white/40"
                          }`}
                        />
                      ))}
                    </div>
                    <div className="mt-1.5">
                      <Toggle checked={sombra} onChange={setSombra} label={t("Sombra")} />
                    </div>
                  </>
                )}
              </Grupo>

              {format === "gif" && <Toggle checked={loop} onChange={setLoop} label={t("Bucle infinito")} />}
              {hasFfmpeg && !foto && (
                <Toggle
                  checked={useFfmpeg}
                  onChange={setUseFfmpeg}
                  label={t("Motor FFmpeg")}
                  hint={t("calidad máxima, más lento")}
                />
              )}
            </div>
          </div>
        </div>
      </div>

      <div className="sticky bottom-0 space-y-2.5 border-t border-white/8 bg-[#141416] p-4">
        {progress && (
          <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
            <div
              className="h-full bg-blue-500 transition-[width] duration-150"
              style={{
                width: `${Math.round((progress.done / Math.max(1, progress.total)) * 100)}%`,
              }}
            />
          </div>
        )}

        <button
          type="button"
          disabled={ocupado}
          onClick={() => void run(false)}
          title={t("Exportar a la carpeta elegida (Ctrl+S)")}
          className="flex h-11 w-full items-center justify-center gap-2 rounded-lg bg-blue-500 text-[15px] font-semibold whitespace-nowrap text-white transition-colors hover:bg-blue-400 disabled:opacity-50"
        >
          <Save className="size-[18px]" /> {t("Guardar")} {format.toUpperCase()}
        </button>

        {/* Las otras dos maneras de guardar, con su nombre: con solo el icono, el de copiar
            y el de elegir carpeta no se distinguían sin pasar el ratón por encima. */}
        <div className="grid grid-cols-2 gap-2">
          <BotonSegundo
            onClick={() => void run(true)}
            apagado={ocupado}
            titulo={t("Exportar y copiar al portapapeles")}
            icono={<Clipboard className="size-4" />}
          >
            {t("Copiar")}
          </BotonSegundo>
          <BotonSegundo
            onClick={() => void runAs(false)}
            apagado={ocupado}
            titulo={t("Elegir dónde guardar (Ctrl+Mayús+S)")}
            icono={<FolderDown className="size-4" />}
          >
            {t("Guardar en…")}
          </BotonSegundo>
        </div>

        <div className="flex items-center justify-between gap-3 text-[12px] text-neutral-300">
          <button
            type="button"
            onClick={() => void pickDirectory().then((dir) => dir && setDirectory(dir))}
            title={t("Elegir dónde se guarda")}
            className="flex min-w-0 items-center gap-1.5 transition-colors hover:text-white"
          >
            <FolderOpen className="size-4 shrink-0" />
            <span className="truncate" title={directory}>
              {directory || t("Elegir carpeta…")}
            </span>
          </button>
          <span className="shrink-0 tabular-nums">≈ {formatBytes(estimate)}</span>
        </div>

        {result && (
          <div className="space-y-1.5">
            {/* Copiar es lo que más se usa para mandarle el vídeo a alguien, así que se
                dice en su propia línea y no escondido entre el tamaño y la carpeta. Un
                vídeo no se pega como imagen: va como archivo (para el explorador y los
                chats) y como su ruta escrita (para cualquier caja de texto). */}
            {result.copied && (
              <p className="flex items-center gap-1.5 text-[13px] font-medium text-emerald-300">
                <Clipboard className="size-4 shrink-0" />
                {format === "png" ? t("Copiado: ya se puede pegar") : t("Copiado: el archivo y su ruta")}
              </p>
            )}
            {/* Y si no se pudo, se dice por qué. Antes el fallo se tragaba y pulsar copiar
                parecía no hacer absolutamente nada. */}
            {result.copyError && (
              <p className="text-[12px] text-red-300">
                {t("No se ha podido copiar")}: {t(result.copyError)}
              </p>
            )}
            <button
              type="button"
              onClick={() => void revealInExplorer(result.path)}
              className="flex w-full items-center gap-1.5 rounded-md py-1 text-left text-[13px] font-medium text-neutral-100 transition-colors hover:text-white"
            >
              <FolderOpen className="size-4 shrink-0 text-blue-400" />
              <span className="truncate">
                {t("Guardado")} · {formatBytes(result.bytes)} · {t("abrir carpeta")}
              </span>
            </button>
          </div>
        )}

        {error && <p className="text-[12px] text-red-300">{error}</p>}
      </div>
      </div>
    </aside>
  );
}

function Grupo({ titulo, children }: { titulo: string; children: ReactNode }) {
  return (
    <div>
      <span className="mb-2 block text-[13px] font-medium text-neutral-200">{titulo}</span>
      {children}
    </div>
  );
}

/** Una fila de opciones en texto, con la elegida subrayada: nada de pastillas. */
function Opciones({ children }: { children: ReactNode }) {
  return <div className="flex items-center gap-4">{children}</div>;
}

function Opcion({
  puesta,
  onClick,
  children,
}: {
  puesta: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={puesta}
      className={`border-b-2 py-1 text-[14px] font-semibold tabular-nums transition-colors ${
        puesta ? "border-blue-500 text-white" : "border-transparent text-neutral-400 hover:text-neutral-100"
      }`}
    >
      {children}
    </button>
  );
}

function BotonSegundo({
  onClick,
  apagado,
  titulo,
  icono,
  children,
}: {
  onClick: () => void;
  apagado: boolean;
  titulo: string;
  icono: ReactNode;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={apagado}
      title={titulo}
      className="flex h-10 items-center justify-center gap-1.5 rounded-lg bg-white/10 text-[14px] font-medium whitespace-nowrap text-neutral-100 transition-colors hover:bg-white/15 hover:text-white disabled:opacity-50"
    >
      {icono}
      {children}
    </button>
  );
}
