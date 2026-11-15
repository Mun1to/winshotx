import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { ChevronDown, Clipboard, FolderDown, FolderOpen, Link2, Save, Unlink2 } from "lucide-react";
import { camaraDeLaSesion, exportMedia, pickDirectory, pickSaveFile, revealInExplorer } from "../../lib/ipc";
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
import { FondoPicker } from "./FondoPicker";
import { AjustesCursor } from "./AjustesCursor";
import { AjustesCamara } from "./AjustesCamara";
import { Presets } from "./Presets";
import { edicionDe, estiloDe, panelDe } from "../../lib/presets";
import type { Edicion } from "../../lib/contrato";
import { Opcion, Opciones } from "../ui/Opcion";
import { Segmented } from "../ui/Segmented";
import { RELLENO, ROJO } from "./colores";
// S11 (pistas)
import { conPistas, pistaDe } from "../../lib/pistas";
import { MandosDePistas } from "./MandosDePistas";
// S25 (marco)
import {
  carcasaParaPedir,
  geometria as geometriaDeLaCarcasa,
  MARCOS,
  ventanaOscura,
  type CarcasaElegida,
} from "../../lib/carcasa";

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

/** Los grupos del panel, en su orden (S39, segunda vuelta). */
type Grupo = "sonido" | "fondo" | "marco" | "puntero" | "clics" | "zoom" | "camara" | "archivo" | "estilos";

/** «Ninguno» en el selector del marco, que no admite `null` como valor. */
const SIN_CARCASA = "ninguno";

/**
 * Los grupos abiertos de fábrica: solo el fondo, que es lo que más cambia cómo se ve un vídeo.
 * Con los nueve títulos (una grabación con cámara) y el fondo abierto, el panel cabe sin rueda
 * en la ventana de 1.152 × 700 que abre una pantalla de 1920 al 125 %; con el sonido abierto
 * también, ya no. Antes, con todo abierto, medía 1.533 px.
 */
const ABIERTOS_DE_FABRICA: Partial<Record<Grupo, boolean>> = { fondo: true };

/**
 * Qué grupos se dejaron abiertos la última vez. Es comodidad de quien lo usa, nada más: sin
 * almacenamiento se abren los de fábrica.
 */
const CLAVE_GRUPOS = "winshotx.editor.grupos";
function leerAbiertos(): Partial<Record<Grupo, boolean>> {
  try {
    const guardado = localStorage.getItem(CLAVE_GRUPOS);
    return guardado ? { ...ABIERTOS_DE_FABRICA, ...JSON.parse(guardado) } : ABIERTOS_DE_FABRICA;
  } catch {
    return ABIERTOS_DE_FABRICA;
  }
}
function guardarAbiertos(abiertos: Partial<Record<Grupo, boolean>>) {
  try {
    localStorage.setItem(CLAVE_GRUPOS, JSON.stringify(abiertos));
  } catch {
    // Sin almacenamiento se abren los de fábrica la próxima vez, y ya está.
  }
}

/** Lo deprisa que se acerca y se aleja el zoom, en palabras. 1 es como siempre. */
const VELOCIDADES_DE_ZOOM: { valor: number; nombre: string }[] = [
  { valor: 0.6, nombre: "Lenta" },
  { valor: 1, nombre: "Normal" },
  { valor: 1.6, nombre: "Rápida" },
];

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
  /**
   * Lo nuevo del plan de CleanShot X y Screen Studio (zooms a mano, tramos, fondos propios,
   * webcam...), que el editor junta de sus piezas. Va tal cual en la petición; en una foto,
   * sin zooms ni tramos, que son cosas del tiempo.
   */
  edicion?: Edicion;
  /** Para las piezas que viven dentro de este panel (el fondo, el cursor): cambian lo suyo. */
  onEdicion?: (cambio: Partial<Edicion>) => void;
  /**
   * La grabación trae ratos de escritura. Las de antes no los traen, y sin ellos «Esconderlo
   * mientras escribes» y «Acercarse mientras escribes» no harían nada.
   */
  conEscritura?: boolean;
}

/**
 * Guardar lo que se ha hecho.
 *
 * Arriba el formato y debajo, en una columna, cada grupo con su título a la vista (sonido,
 * fondo, marco, puntero, clics y teclas, zoom, cámara, calidad y tamaño, y estilos), cada uno
 * plegable y recordando cómo se dejó. Abajo los botones de guardar, siempre a la vista. Estaba
 * todo a la vez, en seis bloques con rótulos en mayúsculas, y para llegar al de abajo había
 * que bajar con la rueda en una ventana normal.
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
  edicion,
  onEdicion,
  conEscritura = true,
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
  /** El radio de las esquinas sobre el fondo, en píxeles del archivo. */
  const [esquinas, setEsquinas] = useState(0);
  // S25 (marco)
  /**
   * La ventana o el portátil alrededor de la captura. Solo con fondo: sin aire detrás, sus
   * esquinas y su sombra no tienen dónde verse, así que sin fondo cuenta como «Ninguno».
   */
  const [carcasa, setCarcasa] = useState<CarcasaElegida | null>(null);
  const carcasaPuesta = margen > 0 ? carcasa : null;
  /**
   * La ventana sale clara u oscura como las de winshotx (en automático, como las de Windows):
   * así se parece a las ventanas de quien la usa sin un cuarto botón. Se sigue al sistema
   * mientras el editor está abierto, para que lo que se ve sea lo que sale.
   */
  const [oscura, setOscura] = useState(ventanaOscura);
  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const sistema = window.matchMedia("(prefers-color-scheme: dark)");
    const mirar = () => setOscura(ventanaOscura());
    sistema.addEventListener("change", mirar);
    window.addEventListener("storage", mirar);
    return () => {
      sistema.removeEventListener("change", mirar);
      window.removeEventListener("storage", mirar);
    };
  }, []);
  /**
   * Quitarle al puntero los temblores de la mano. **Encendido de fábrica**, como en Screen
   * Studio y en Cap: es lo que hace que un vídeo de pantalla se vea hecho a propósito, y
   * solo mueve el puntero unas centésimas.
   *
   * Ya no tiene interruptor: lo sustituye el suavizado de tres opciones de `AjustesCursor`
   * (`edicion.suavizado`), que manda cuando viene, y «Poco» es exactamente esto encendido.
   * Se queda porque es lo que viaja sin tocar nada y lo que guarda un estilo.
   */
  const [suave, setSuave] = useState(true);
  /** Apagar el puntero cuando lleva un rato quieto. Apagado: sorprende si no se pide. */
  const [esconder, setEsconder] = useState(false);
  const [velocidadZoom, setVelocidadZoom] = useState(1);
  const [volumen, setVolumen] = useState(1);
  // S11 (pistas): con el PC y la voz por separado, cada uno lleva su mando (encendido y
  // volumen) donde iba «Sonido». Viven en `edicion` porque la vista previa los oye igual.
  const pistasAparte = conPistas(session);
  const pc = pistaDe(edicion?.sonidoPc);
  const voz = pistaDe(edicion?.microfono);
  const suenaAlguna = (!!session.sonidoPc && pc.encendida) || (!!session.microfono && voz.encendida);
  const [abiertos, setAbiertos] = useState(leerAbiertos);
  useEffect(() => guardarAbiertos(abiertos), [abiertos]);
  /** Las dos cosas que necesita cada grupo plegable: si está abierto y cómo cambiarlo. */
  const plegado = (grupo: Grupo) => ({
    grupo,
    abierto: !!abiertos[grupo],
    onCambiar: () => setAbiertos((antes) => ({ ...antes, [grupo]: !antes[grupo] })),
  });
  /** Si la grabación trae webcam: sin ella, el grupo «Cámara» no sale. */
  const [hayCamara, setHayCamara] = useState(false);
  useEffect(() => {
    let vigente = true;
    void camaraDeLaSesion(session.id)
      .then((c) => {
        if (vigente) setHayCamara(!!c);
      })
      .catch(() => undefined);
    return () => {
      vigente = false;
    };
  }, [session.id]);
  const [loop, setLoop] = useState(true);
  const [directory, setDirectory] = useState(saveDirectory);
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [result, setResult] = useState<ExportResult | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  useEffect(() => {
    onEstudio?.({
      zoom,
      cursor,
      aros,
      teclas,
      suave,
      esconder,
      velocidadZoom,
      volumen,
      velocidad,
      marco:
        margen > 0
          ? {
              aire: margen / Math.max(1, width),
              esquinas: esquinas / Math.max(1, width),
              fondo,
              sombra,
              // S25 (marco): con el tamaño del archivo, que es con lo que se mide la carcasa.
              carcasa: carcasaPuesta ? { elegida: carcasaPuesta, oscura, ancho: width, alto: height } : null,
            }
          : null,
    });
  }, [zoom, cursor, aros, teclas, suave, esconder, velocidadZoom, volumen, velocidad, margen, esquinas, fondo, sombra, width, height, carcasaPuesta, oscura, onEstudio]);

  /**
   * Elegir un fondo sin aire pone el aire, las esquinas y la sombra que se ven bien a ese
   * tamaño: un fondo sin aire alrededor no se ve, y quien lo elige quiere verlo. Un 5 % del
   * ancho de aire y algo más del 1 % de radio, que es por donde van los de Screen Studio.
   */
  const elegirFondo = (elegido: Background | null) => {
    if (elegido === null) {
      setMargen(0);
      // S25 (marco): sin fondo no hay marco; volver a poner un fondo no lo trae de vuelta.
      setCarcasa(null);
      return;
    }
    setFondo(elegido);
    if (margen === 0) {
      setMargen(Math.min(200, Math.max(16, Math.round(width * 0.05))));
      setEsquinas(Math.min(48, Math.max(6, Math.round(width * 0.012))));
      setSombra(true);
    }
  };

  /**
   * El estilo de lo que hay puesto, para «Guardar estilo como…», y lo que hace poner uno
   * guardado. Se guarda en proporción del vídeo (ver `presets.ts`), y por eso se pasan las
   * medidas: el ancho del archivo y el alto de lo grabado, que es con lo que se mide el puntero.
   */
  const medidas = { ancho: width, altoGrabado: session.region.height };
  const estiloAhora = estiloDe(
    { zoom, velocidadZoom, cursor, aros, teclas, suave, esconder, margen, esquinas, fondo, sombra, carcasa: carcasaPuesta },
    edicion,
    medidas,
  );
  const aplicarEstilo = (estilo: Parameters<typeof panelDe>[0]) => {
    const p = panelDe(estilo, medidas);
    setZoom(p.zoom);
    setVelocidadZoom(p.velocidadZoom);
    // Con el cursor de Windows cocido en los fotogramas saldrían dos punteros.
    if (!session.cursorBaked && !esCaptura) setCursor(p.cursor);
    setAros(p.aros);
    setTeclas(p.teclas);
    setSuave(p.suave);
    setEsconder(p.esconder);
    setMargen(p.margen);
    setEsquinas(p.esquinas);
    setFondo(p.fondo);
    setSombra(p.sombra);
    // S25 (marco)
    setCarcasa(p.carcasa ?? null);
    // El fondo propio, el suavizado, esconder al escribir, el clic, la onda, la estela, acercarse
    // al escribir y la cámara. Nunca los zooms, los cortes ni lo tapado, que son de este vídeo.
    // Un estilo sin cámara (guardado con una grabación que no la tenía, o sin tocarla) no
    // apaga la de esta: solo la coloca el que la trae.
    const { camara, ...resto } = edicionDe(estilo);
    onEdicion?.(camara ? { ...resto, camara } : resto);
  };

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

  /**
   * Lo que dura lo que se va a guardar, en segundos, antes de la velocidad general. Con
   * tijeras, los trozos que se quedan, cada uno a su velocidad (`duracionQueSale`): lo
   * quitado no pesa, y un trozo al doble pesa la mitad.
   */
  const tramos = edicion?.tramos;
  const segundos = useMemo(() => {
    if (tramos && tramos.length > 0) {
      return tramos.reduce((total, tramo) => total + (tramo.hastaMs - tramo.desdeMs) / tramo.velocidad, 0) / 1000;
    }
    if (desdeMs !== undefined && hastaMs !== undefined) return Math.max(0, hastaMs - desdeMs) / 1000;
    return Math.max(1, outIndex - inIndex + 1) / Math.max(1, session.fps);
  }, [tramos, desdeMs, hastaMs, inIndex, outIndex, session.fps]);
  /** Algún trozo va acelerado: ese trozo sale sin sonido y el resto suena. */
  const trozosAcelerados = !!tramos?.some((tramo) => tramo.velocidad !== 1);

  const estimate = useMemo(() => {
    // A 2x el vídeo dura la mitad, así que también pesa la mitad.
    const seconds = segundos / velocidad;
    // El marco crece por fuera, así que el archivo también: la estimación cuenta con él.
    // S25 (marco): y con la ventana o el portátil, que crecen por fuera de la captura.
    const pieza = carcasaPuesta ? geometriaDeLaCarcasa(carcasaPuesta, width, height) : { ancho: width, alto: height };
    const w = pieza.ancho + margen * 2;
    const h = pieza.alto + margen * 2;
    if (format === "png") return w * h * 3 * 0.35;
    // Medido sobre capturas de verdad: un JPEG de calidad 85 se queda en torno a
    // 0,13 bytes por pixel, que es donde sale este factor.
    if (format === "jpg") return w * h * (quality / 100) * 0.15;
    if (format === "gif") return w * h * (quality / 100) * 0.12 * fps * seconds;
    const bitrate = 1_000_000 + (quality / 100) * 11_000_000;
    return (bitrate / 8) * seconds;
  }, [format, width, height, margen, carcasaPuesta, quality, fps, segundos, velocidad]);

  const run = async (copyToClipboard: boolean, file: string | null = null) => {
    // Sin esto, dos pulsaciones seguidas lanzan dos exportaciones a la vez.
    if (progress !== null) return;
    setError(null);
    setResult(null);
    setProgress({ stage: "reading", done: 0, total: 1 });
    try {
      const foto = esUnaFoto(format);
      const res = await exportMedia({
        ...(foto ? { ...edicion, zooms: null, tramos: null } : edicion),
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
        audio: (pistasAparte ? suenaAlguna : audio) && format === "mp4" && velocidad === 1,
        speed: foto ? 1 : velocidad,
        // Una foto no tiene zoom: la cámara solo se mueve con el tiempo pasando.
        zoom: foto ? 1 : zoom,
        // Una foto no tiene ni aros ni pastilla: los dos duran un rato y aquí no pasa
        // el tiempo.
        clicks: !foto && aros,
        keys: !foto && teclas,
        cursor: foto ? 0 : cursor,
        smoothCursor: !foto && suave,
        hideIdleCursor: !foto && esconder,
        corners: margen > 0 ? esquinas : 0,
        volume: volumen,
        zoomSpeed: velocidadZoom,
        loop,
        margin: margen,
        background: fondo,
        shadow: sombra,
        annotations: anotaciones,
        crop: recorte,
        destination: directory || null,
        file,
        copyToClipboard,
        // S11 (pistas): cada pista con su mando, solo las que trae la grabación.
        sonidoPc: session.sonidoPc ? pc : null,
        microfono: session.microfono ? voz : null,
        // S25 (marco): la ventana con el tema de ahora, el mismo que enseña la vista previa.
        carcasa: carcasaParaPedir(carcasaPuesta, oscura),
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
    <aside className="flex w-[300px] shrink-0 flex-col border-l border-linea">
      {/*
        Una columna, como la de antes de la 0.5, que es la que se entiende: el formato arriba, y
        debajo cada grupo con su título a la vista y plegable. La primera vuelta de la 0.5 lo
        escondía detrás de un riel de ocho iconos sin nombre (en los ajustes funciona porque hay
        buscador y dibujo; aquí no). Con todo abierto medía 1.533 px en una ventana de 760: por
        eso cada grupo se pliega y recuerda cómo se dejó.
      */}
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="px-4 pt-3 pb-2.5">
          {/* El formato, en texto con su raya debajo: es una pestaña, no un interruptor. */}
          <div className="flex gap-5 border-b border-linea" role="group" aria-label={t("Formato")}>
            {formatos.map((f) => (
              <button
                key={f.id}
                type="button"
                onClick={() => setFormat(f.id)}
                aria-pressed={format === f.id}
                className={`-mb-px border-b-[3px] pt-1 pb-2 text-[15px] font-semibold transition-colors ${
                  format === f.id
                    ? "border-marca text-titulo"
                    : "border-transparent text-apagado hover:text-titulo"
                }`}
              >
                {f.label}
              </button>
            ))}
          </div>
          <p className="mt-2.5 text-[13px] leading-snug text-suave">
            {t(
              (esCaptura && PARA_UNA_CAPTURA[format]) ||
                (FORMATS.find((f) => f.id === format)?.para ?? ""),
            )}
          </p>
        </div>

        {format === "mp4" && (
          <Plegable titulo={t("Sonido")} {...plegado("sonido")}>
            {!pistasAparte && (
              <Toggle
                checked={audio}
                onChange={setAudio}
                label={t("Con sonido")}
                hint={
                  !session.hasAudio
                    ? t("esta grabación se hizo sin audio")
                    : velocidad !== 1
                      ? t("a otra velocidad el vídeo sale mudo")
                      : trozosAcelerados
                        ? t("los trozos acelerados salen sin sonido")
                        : undefined
                }
              />
            )}
            {/* S11 (pistas): el sonido del PC y tu micrófono, cada uno con lo suyo. */}
            {pistasAparte && (
              <MandosDePistas
                pc={session.sonidoPc ? pc : null}
                voz={session.microfono ? voz : null}
                conVolumen={velocidad === 1}
                aviso={
                  velocidad !== 1
                    ? t("a otra velocidad el vídeo sale mudo")
                    : trozosAcelerados
                      ? t("los trozos acelerados salen sin sonido")
                      : undefined
                }
                picoPc={session.picoPc}
                picoVoz={session.picoMicrofono}
                onPc={(sonidoPc) => onEdicion?.({ sonidoPc })}
                onVoz={(microfono) => onEdicion?.({ microfono })}
              />
            )}
            {session.hasAudio && !pistasAparte && audio && velocidad === 1 && (
              <Slider
                label={t("Volumen")}
                hint={`${Math.round(volumen * 100)} %`}
                min={0}
                max={200}
                step={5}
                value={Math.round(volumen * 100)}
                onChange={(v) => setVolumen(v / 100)}
              />
            )}
          </Plegable>
        )}

        <Plegable titulo={t("Fondo")} {...plegado("fondo")}>
          <FondoPicker
            fondo={fondo}
            conMarco={margen > 0}
            elegido={edicion?.fondo ?? null}
            onFondo={(f) => {
              onEdicion?.({ fondo: null });
              elegirFondo(f);
            }}
            onElegido={(e) => {
              onEdicion?.({ fondo: e });
              // Pone el aire si estaba a cero: un fondo propio sin aire no se ve.
              elegirFondo(fondo);
            }}
          />
          {/* Lo del marco solo con fondo: aire y esquinas de un marco que no existe son
              controles que no hacen nada. */}
          {margen > 0 && (
            <Slider
              label={t("Aire alrededor")}
              hint={`${margen} px`}
              min={4}
              max={200}
              value={margen}
              onChange={setMargen}
            />
          )}
          {/* S25 (marco): con ventana o portátil, las esquinas son las suyas. */}
          {margen > 0 && !carcasaPuesta && (
            <Slider
              label={t("Esquinas redondeadas")}
              hint={esquinas === 0 ? t("rectas") : `${esquinas} px`}
              min={0}
              max={60}
              value={esquinas}
              onChange={setEsquinas}
            />
          )}
          {margen > 0 && <Toggle checked={sombra} onChange={setSombra} label={t("Sombra")} />}
        </Plegable>

        {/* S25 (marco): la captura dentro de una ventana de Windows 11 o de un portátil. Elegir
            uno sin fondo pone el fondo, como elegir un fondo pone el aire: sin nada detrás no
            se vería. */}
        <Plegable titulo={t("Marco")} {...plegado("marco")}>
          <Segmented
            value={carcasaPuesta ?? SIN_CARCASA}
            etiqueta={t("Marco")}
            options={MARCOS.map((m) => ({ value: m.id ?? SIN_CARCASA, label: t(m.nombre) }))}
            onChange={(id) => {
              const elegida = id === SIN_CARCASA ? null : (id as CarcasaElegida);
              setCarcasa(elegida);
              if (elegida && margen === 0) elegirFondo(fondo);
            }}
          />
        </Plegable>

        {/* Una foto no tiene puntero, ni clics, ni zoom: son cosas del tiempo. */}
        {!foto && (
          <Plegable titulo={t("Puntero")} {...plegado("puntero")}>
            <div>
              <Slider
                label={t("Tamaño")}
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
                <p className="mt-2 text-[13px] leading-snug text-aviso">
                  {t("Esta grabación ya lleva el cursor de Windows dentro: vas a ver dos.")}
                </p>
              )}
            </div>
            {cursor > 0 && (
              <Toggle
                checked={esconder}
                onChange={setEsconder}
                label={t("Esconderlo cuando no se mueve")}
                hint={t("se va tras un segundo y medio quieto")}
              />
            )}
            {/* El suavizado de tres opciones, esconderlo al escribir y la estela. */}
            <AjustesCursor
              parte="puntero"
              edicion={edicion ?? {}}
              onEdicion={(cambio) => onEdicion?.(cambio)}
              conPuntero={cursor > 0}
              conClics={session.hasClicks}
              conZoom={zoom > 1.05 || (edicion?.zooms?.length ?? 0) > 0}
              conEscritura={conEscritura}
            />
          </Plegable>
        )}

        {!foto && (
          <Plegable titulo={t("Clics y teclas")} {...plegado("clics")}>
            {session.hasClicks && <Toggle checked={aros} onChange={setAros} label={t("Marcar los clics")} />}
            {/* La onda y el sonido de los clics. */}
            <AjustesCursor
              parte="clics"
              edicion={edicion ?? {}}
              onEdicion={(cambio) => onEdicion?.(cambio)}
              conPuntero={cursor > 0}
              conClics={session.hasClicks}
              conEscritura={conEscritura}
            />
            <Toggle
              checked={teclas}
              onChange={setTeclas}
              label={t("Enseñar los atajos")}
              // Este aviso viaja con el interruptor: es la regla que impide que una
              // contraseña tecleada mientras se grababa acabe dentro del vídeo.
              hint={t("solo con Ctrl, Alt o Win: lo que escribes no sale")}
            />
          </Plegable>
        )}

        {!foto && (session.hasClicks || conEscritura) && (
          <Plegable titulo={t("Zoom")} {...plegado("zoom")}>
            {/* El zoom de los clics solo con clics: un deslizador que no puede hacer nada es
                peor que no tenerlo. */}
            {session.hasClicks && (
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
            {session.hasClicks && zoom > 1.05 && (
              <div>
                <Etiqueta>{t("Velocidad del zoom")}</Etiqueta>
                <Segmented
                  value={velocidadZoom}
                  etiqueta={t("Velocidad del zoom")}
                  options={VELOCIDADES_DE_ZOOM.map((v) => ({ value: v.valor, label: t(v.nombre) }))}
                  onChange={setVelocidadZoom}
                />
              </div>
            )}
            <AjustesCursor
              parte="zoom"
              edicion={edicion ?? {}}
              onEdicion={(cambio) => onEdicion?.(cambio)}
              conPuntero={cursor > 0}
              conClics={session.hasClicks}
              conEscritura={conEscritura}
            />
          </Plegable>
        )}

        {/* La webcam: el grupo solo sale si la grabación la trae. */}
        {!foto && hayCamara && (
          <Plegable titulo={t("Cámara")} {...plegado("camara")}>
            <AjustesCamara sessionId={session.id} edicion={edicion} onEdicion={onEdicion} />
          </Plegable>
        )}

        <Plegable titulo={t("Calidad y tamaño")} {...plegado("archivo")}>
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
            <div>
              <Etiqueta>{t("Velocidad")}</Etiqueta>
              <Segmented
                value={velocidad}
                etiqueta={t("Velocidad")}
                options={VELOCIDADES.map((v) => ({ value: v, label: `${v}×` }))}
                onChange={setVelocidad}
              />
              {velocidad !== 1 && (
                <p className="mt-1.5 text-[13px] text-suave">
                  {velocidad > 1
                    ? t("dura {n} veces menos, y sale sin sonido", { n: velocidad })
                    : t("dura el doble, y sale sin sonido")}
                </p>
              )}
            </div>
          )}
          <div>
            <Etiqueta>{t("Tamaño")}</Etiqueta>
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
                    onClick={() => onRecorte(conProporcion(p.valor, session.region.width, session.region.height))}
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
                    className="ml-auto rounded-md px-1.5 text-[16px] leading-none text-suave transition-colors hover:bg-realce hover:text-titulo"
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
                className={`mb-1 flex size-8 shrink-0 items-center justify-center rounded-md transition-colors ${
                  locked ? "text-marca hover:bg-realce" : "text-apagado hover:text-titulo"
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
          </div>
          {format === "gif" && <Toggle checked={loop} onChange={setLoop} label={t("Bucle infinito")} />}
          {hasFfmpeg && !foto && (
            <Toggle
              checked={useFfmpeg}
              onChange={setUseFfmpeg}
              label={t("Motor FFmpeg")}
              hint={t("calidad máxima, más lento")}
            />
          )}
        </Plegable>

        {/* Guardar el estilo va al final, después de lo que lo forma: se lee en su orden. */}
        <Plegable titulo={t("Estilos")} {...plegado("estilos")}>
          <div>
            <p className="mb-3 text-[13px] leading-snug text-suave">
              {t("Guarda el fondo, el puntero, los clics y el zoom con un nombre, para ponérselos a otro vídeo")}
            </p>
            <Presets enLista estilo={estiloAhora} onAplicar={aplicarEstilo} />
          </div>
        </Plegable>
      </div>

      {/* Guardar, siempre a la vista, abajo: los tres botones, la carpeta y lo que va a pesar. */}
      <div className="space-y-2.5 border-t border-linea px-4 pt-3 pb-3">

        {progress && (
          <div className="h-1.5 overflow-hidden rounded-full bg-realce">
            <div
              className="h-full bg-marca transition-[width] duration-150"
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
          className={`flex h-11 w-full items-center justify-center gap-2 rounded-lg text-[15px] font-semibold whitespace-nowrap transition-colors disabled:opacity-50 ${RELLENO}`}
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

        <div className="flex items-center justify-between gap-3 text-[13px] text-suave">
          <button
            type="button"
            onClick={() => void pickDirectory().then((dir) => dir && setDirectory(dir))}
            title={t("Elegir dónde se guarda")}
            className="flex min-w-0 items-center gap-1.5 transition-colors hover:text-titulo"
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
                chats) y como su ruta escrita (para cualquier caja de texto). Sin verde de
                «todo va bien»: lo dice la frase. */}
            {result.copied && (
              <p className="flex items-center gap-1.5 text-[13px] font-medium text-texto">
                <Clipboard className="size-4 shrink-0" />
                {format === "png" ? t("Copiado: ya se puede pegar") : t("Copiado: el archivo y su ruta")}
              </p>
            )}
            {/* Y si no se pudo, se dice por qué. Antes el fallo se tragaba y pulsar copiar
                parecía no hacer absolutamente nada. */}
            {result.copyError && (
              <p className={`text-[13px] ${ROJO}`}>
                {t("No se ha podido copiar")}: {t(result.copyError)}
              </p>
            )}
            <button
              type="button"
              onClick={() => void revealInExplorer(result.path)}
              className="flex w-full items-center gap-1.5 rounded-md py-1 text-left text-[13px] font-medium text-titulo transition-colors hover:text-marca"
            >
              <FolderOpen className="size-4 shrink-0 text-marca" />
              <span className="truncate">
                {t("Guardado")} · {formatBytes(result.bytes)} · {t("abrir carpeta")}
              </span>
            </button>
          </div>
        )}

        {error && <p className={`text-[13px] ${ROJO}`}>{error}</p>}
      </div>
    </aside>
  );
}

/**
 * Un grupo que se pliega: el título con su flecha, y debajo lo de dentro, que crece poco a
 * poco con la fila de la rejilla de 0fr a 1fr y empuja lo de debajo, en vez de aparecer de
 * golpe. Cerrado no se puede pulsar ni llegar con el tabulador a nada de dentro.
 *
 * Abrir uno que se queda por debajo del borde (los de abajo, con otros abiertos encima) lo trae
 * a la vista al acabar de crecer: si no, se pulsa y parece que no pasa nada.
 */
function Plegable({
  grupo,
  titulo,
  abierto,
  onCambiar,
  children,
}: {
  grupo: Grupo;
  titulo: string;
  abierto: boolean;
  onCambiar: () => void;
  children: ReactNode;
}) {
  const caja = useRef<HTMLDivElement>(null);
  const abrir = () => {
    onCambiar();
    if (abierto) return;
    // Lo de dentro tarda 240 ms en crecer: se mira dónde ha quedado cuando ya mide lo suyo.
    setTimeout(() => caja.current?.scrollIntoView?.({ block: "nearest", behavior: "smooth" }), 260);
  };
  return (
    <div ref={caja} className="border-t border-linea px-4">
      <button
        type="button"
        data-grupo={grupo}
        onClick={abrir}
        aria-expanded={abierto}
        className="group flex w-full items-center justify-between py-[7px] text-left text-[14px] font-semibold text-titulo"
      >
        {titulo}
        <ChevronDown
          className={`size-[18px] text-apagado transition-[transform,color] duration-200 group-hover:text-titulo ${
            abierto ? "rotate-180" : ""
          }`}
        />
      </button>
      <div
        className="grid transition-[grid-template-rows] duration-[240ms] ease-in-out"
        style={{ gridTemplateRows: abierto ? "1fr" : "0fr" }}
      >
        <div className="min-h-0 overflow-hidden" inert={!abierto} aria-hidden={!abierto}>
          <div className="space-y-4 pt-0.5 pb-3.5">{children}</div>
        </div>
      </div>
    </div>
  );
}

/** El nombre de una fila que lleva sus opciones debajo (el marco, la velocidad, el tamaño). */
function Etiqueta({ children }: { children: ReactNode }) {
  return <span className="mb-1 block text-[15px] font-medium text-titulo">{children}</span>;
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
      className="flex h-10 items-center justify-center gap-1.5 rounded-lg bg-realce text-[14px] font-medium whitespace-nowrap text-titulo transition-colors hover:bg-activo disabled:opacity-50"
    >
      {icono}
      {children}
    </button>
  );
}
