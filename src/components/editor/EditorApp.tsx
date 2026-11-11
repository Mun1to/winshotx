import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Pause, Play, VideoOff } from "lucide-react";
import {
  discardSession,
  ffmpegAvailable,
  frameImage,
  getSettings,
  sessionCamera,
  sessionCameraConZooms,
  sessionFrames,
  sessionInfo,
  copiaTexto,
  sessionStudio,
  sessionZooms,
  // S7 (cursor)
  sessionCameraAlEscribir,
  sessionZoomsAlEscribir,
  tapaDatos,
} from "../../lib/ipc";
import { formatTimecode } from "../../lib/format";
import {
  EVENTS,
  type AvisoVistaPrevia,
  type FrameMeta,
  type MuestraCamara,
  type SessionInfo,
  type Settings,
  type StudioData,
} from "../../lib/types";
import { BarraAnotar } from "./BarraAnotar";
import { CapaAnotaciones } from "./CapaAnotaciones";
import { CapaEstudio } from "./CapaEstudio";
import { CapaRecorte } from "./CapaRecorte";
import { COLORES, COLOR_RESALTADO, type Anotacion, type Herramienta } from "../../lib/anotaciones";
import { ExportPanel } from "./ExportPanel";
import { LineaDeTiempo } from "./LineaDeTiempo";
import { PreviewCanvas } from "./PreviewCanvas";
import { medida as medidaDelRecorte, type Recorte } from "../../lib/recorte";
import { cursorEn, ESTUDIO_APAGADO, medidasConMarco, type Estudio } from "../../lib/estudio";
import { cssDelFondo } from "../../lib/fondos";
import { FondoVivo } from "./FondoVivo";
import type { Edicion, TramoZoom } from "../../lib/contrato";
import { useT } from "../../lib/i18n";
import {
  duracionDe,
  fotogramaVecino,
  indiceEn,
  limitarTramo,
  piezasDe,
  tramosDeEnvio,
  type Pieza,
} from "../../lib/tramo";
import { cambiar, NIVEL_NUEVO } from "../../lib/zooms";

export function EditorApp({ sessionId }: { sessionId: string }) {
  const t = useT();
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [frames, setFrames] = useState<FrameMeta[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [hasFfmpeg, setHasFfmpeg] = useState(false);
  /**
   * Lo que se queda, en milisegundos de la grabación, y dónde está la vista previa.
   *
   * Por reloj y no por fotogramas: una pantalla quieta es UN fotograma que dura lo que
   * dure la quietud, y por fotogramas no se podía cortar dentro de ella. Los índices que
   * siguen haciendo falta (la foto de un fotograma, leer su texto) salen de aquí.
   */
  const [desdeMs, setDesdeMs] = useState(0);
  const [hastaMs, setHastaMs] = useState(0);
  const [posicionMs, setPosicionMs] = useState(0);
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
  /**
   * Lo que el panel ha decidido del estudio (zoom, puntero, aros, pastilla), para
   * dibujarlo encima de la vista previa. El panel manda; esto es el espejo.
   */
  const [estudio, setEstudio] = useState<Estudio>(ESTUDIO_APAGADO);
  /** Los clics, atajos y rastro del raton que se anotaron al grabar. */
  const [datosEstudio, setDatosEstudio] = useState<StudioData | null>(null);
  /** La camara del zoom, una muestra por fotograma, con el zoom y el recorte puestos. */
  const [camara, setCamara] = useState<MuestraCamara[]>([]);
  /**
   * Lo nuevo del plan de CleanShot X y Screen Studio que se manda al exportar (los zooms de
   * la pista, los tramos, el fondo propio...). Lo juntan la línea de tiempo y el panel.
   */
  const [edicion, setEdicion] = useState<Edicion>({});
  /**
   * La pista de zooms tocada a mano. `null` mientras nadie la toca: entonces la pista
   * enseña los que salen solos (`zoomsSolos`), y a Rust va `zooms: null`, que es exportar
   * igual que antes de que hubiera pista.
   */
  const [zoomsAMano, setZoomsAMano] = useState<TramoZoom[] | null>(null);
  /** Los zooms que salen solos, de los clics y de la tecla de zoom, como bloques. */
  const [zoomsSolos, setZoomsSolos] = useState<TramoZoom[]>([]);
  /** El bloque de la pista que está elegido, con sus mandos debajo. */
  const [zoomElegido, setZoomElegido] = useState<number | null>(null);
  /** Los trozos de las tijeras: lo que se queda, lo que se quita y a qué velocidad. */
  const [piezas, setPiezas] = useState<Pieza[]>([]);
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
        setHastaMs(duracionDe(frameList));
        setPiezas(piezasDe(duracionDe(frameList)));
      })
      .catch((e) => setError(String(e)));
  }, [sessionId]);

  // Lo anotado al grabar se pide una vez: no cambia. Sin ello la capa no dibuja nada.
  useEffect(() => {
    if (!session || session.format === "still") return;
    void sessionStudio(sessionId)
      .then((datos) => setDatosEstudio(datos ?? null))
      .catch(() => setDatosEstudio(null));
  }, [session, sessionId]);

  // S7 (cursor): acercarse mientras se escribe (A5) cambia los zooms que salen solos.
  const alEscribir = edicion.zoomAlEscribir ?? false;

  /**
   * La camara se vuelve a pedir cada vez que cambia el zoom o el recorte, con un respiro:
   * arrastrar el deslizador dispara decenas de cambios por segundo y cada uno recorre
   * todos los fotogramas en Rust. Sin zoom no se pide nada y el video se queda quieto.
   */
  useEffect(() => {
    // Con la pista tocada manda la pista; sin tocar, los que salen solos (el zoom de los
    // clics con el deslizador puesto, o las marcas de la tecla de zoom).
    const hayZoom = zoomsAMano
      ? zoomsAMano.some((z) => !z.lupa)
      : estudio.zoom > 1.05 || zoomsSolos.length > 0;
    if (!session || !hayZoom) {
      setCamara([]);
      return;
    }
    let vigente = true;
    const espera = window.setTimeout(() => {
      const pedir = zoomsAMano
        ? sessionCameraConZooms(sessionId, estudio.zoom, recorte, estudio.velocidadZoom, zoomsAMano)
        : alEscribir // S7 (cursor): con «acercarse al escribir», también esos zooms.
          ? sessionCameraAlEscribir(sessionId, estudio.zoom, recorte, estudio.velocidadZoom, true)
          : sessionCamera(sessionId, estudio.zoom, recorte, estudio.velocidadZoom);
      void pedir
        .then((muestras) => {
          if (vigente) setCamara(muestras ?? []);
        })
        .catch(() => undefined);
    }, 120);
    return () => {
      vigente = false;
      window.clearTimeout(espera);
    };
  }, [session, sessionId, estudio.zoom, estudio.velocidadZoom, recorte, zoomsAMano, zoomsSolos.length, alEscribir]);

  /**
   * Los zooms que salen solos, para enseñarlos en la pista como bloques: se piden otra vez
   * al mover el deslizador del zoom o el recorte, mientras la pista no se haya tocado.
   */
  useEffect(() => {
    if (!session || session.format === "still" || zoomsAMano) return;
    let vigente = true;
    const espera = window.setTimeout(() => {
      const pedir = alEscribir // S7 (cursor): con «acercarse al escribir», también esos bloques.
        ? sessionZoomsAlEscribir(sessionId, estudio.zoom, recorte, estudio.velocidadZoom, true)
        : sessionZooms(sessionId, estudio.zoom, recorte, estudio.velocidadZoom);
      void pedir
        .then((bloques) => {
          if (vigente) setZoomsSolos(bloques ?? []);
        })
        .catch(() => undefined);
    }, 120);
    return () => {
      vigente = false;
      window.clearTimeout(espera);
    };
  }, [session, sessionId, estudio.zoom, estudio.velocidadZoom, recorte, zoomsAMano, alEscribir]);

  /** Lo que se ve en la pista: lo tocado a mano, o lo que sale solo. */
  const zoomsMostrados = zoomsAMano ?? zoomsSolos;
  const bloqueElegido = zoomElegido !== null ? (zoomsMostrados[zoomElegido] ?? null) : null;

  // La pista y las tijeras viajan a Rust dentro de `edicion`, como lo demás del plan.
  useEffect(() => {
    setEdicion((antes) => ({ ...antes, zooms: zoomsAMano, tramos: tramosDeEnvio(piezas, desdeMs, hastaMs) }));
  }, [zoomsAMano, piezas, desdeMs, hastaMs]);

  /** Dónde estaba el ratón en ese instante, de 0 a 1: ahí mira un zoom recién puesto. */
  const puntoEn = useCallback(
    (ms: number) => {
      const raton = datosEstudio ? cursorEn(datosEstudio.cursor, ms) : null;
      if (!raton || !session) return { x: 0.5, y: 0.5 };
      return {
        x: raton[0] / Math.max(1, session.region.width),
        y: raton[1] / Math.max(1, session.region.height),
      };
    },
    [datosEstudio, session],
  );

  const totalMs = duracionDe(frames);
  /** El fotograma que se está viendo: el de la foto, el de leer el texto, el de tapar datos. */
  const currentIndex = indiceEn(frames, posicionMs);
  /** Y los fotogramas de los dos cortes, para lo que todavía va por fotogramas. */
  const inIndex = indiceEn(frames, desdeMs);
  const outIndex = indiceEn(frames, Math.max(desdeMs, hastaMs - 1));

  /** Lleva la vista previa a ese instante, parándola. */
  const ir = useCallback((ms: number) => {
    const video = videoRef.current;
    if (video) {
      video.pause();
      video.currentTime = ms / 1000;
    }
    setPosicionMs(ms);
    setSeekMs(ms);
  }, []);

  /**
   * Mover un corte lleva la vista previa a él: lo que se ve mientras se arrastra el asa es
   * el sitio exacto del corte, que es lo único que se quiere mirar en ese momento. Del
   * final se enseña lo último que se queda, no lo primero que se va.
   */
  const cambiarTramo = useCallback(
    (desde: number, hasta: number, mueve: "desde" | "hasta" | "las dos") => {
      setDesdeMs(desde);
      setHastaMs(hasta);
      if (mueve === "desde") ir(desde);
      else if (mueve === "hasta") ir(Math.max(desde, hasta - 1));
    },
    [ir],
  );

  /** I y O cortan donde está la vista previa, sin mover el otro corte. */
  const markIn = useCallback(() => {
    const nuevo = limitarTramo(posicionMs, hastaMs, totalMs, "desde");
    setDesdeMs(nuevo.desdeMs);
    setHastaMs(nuevo.hastaMs);
  }, [posicionMs, hastaMs, totalMs]);

  const markOut = useCallback(() => {
    const nuevo = limitarTramo(desdeMs, posicionMs, totalMs, "hasta");
    setDesdeMs(nuevo.desdeMs);
    setHastaMs(nuevo.hastaMs);
  }, [posicionMs, desdeMs, totalMs]);

  /** Al llegar al final de lo que se queda, se para ahí, enseñando lo último que entra. */
  const alFinal = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    video.pause();
    const ultimo = Math.max(desdeMs, hastaMs - 1);
    video.currentTime = ultimo / 1000;
    setPosicionMs(ultimo);
  }, [desdeMs, hastaMs]);

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
    // Fuera de lo que se queda, o ya en su final: se empieza por su principio. Es lo que
    // hace falta para mirar el recorte entero de un vistazo.
    const ms = video.currentTime * 1000;
    if (ms < desdeMs - 1 || ms >= hastaMs - 50) {
      video.currentTime = desdeMs / 1000;
      setPosicionMs(desdeMs);
    }
    setFalloDeVideo(null);
    void video.play().catch((fallo: unknown) => setFalloDeVideo(String(fallo)));
  }, [desdeMs, hastaMs]);


  /**
   * Lo que mide el trozo recortado, para ensennarlo en el boton.
   *
   * Sale del tamanno de la region grabada, no del de la vista previa: la vista previa
   * cambia con la ventana y el archivo no.
   */
  const [buscandoDatos, setBuscandoDatos] = useState(false);
  /** Lo ultimo que ha pasado, para decirlo sin tapar la imagen. Se va solo. */
  const [aviso, setAviso] = useState<string | null>(null);

  /**
   * Tapa de una vez los datos personales que haya en el fotograma que se está viendo.
   *
   * Lo que vuelve son marcas de difuminado normales, así que se ven antes de exportar y se
   * quitan con Ctrl+Z. Eso no es un detalle: el mosaico borra los píxeles de verdad, y
   * aplicarlo sin que nadie lo vea sería lo único de la aplicación que no tiene vuelta
   * atrás. Si no encuentra nada, lo dice, porque un botón que no hace nada visible es
   * indistinguible de un botón roto.
   */
  const taparDatos = useCallback(async () => {
    if (buscandoDatos) return;
    setBuscandoDatos(true);
    setAviso(null);
    try {
      const { marcas, resumen } = await tapaDatos(sessionId, currentIndex);
      if (marcas.length === 0) {
        setAviso(t("No he encontrado datos personales en esta imagen"));
        return;
      }
      setAnotaciones((previas) => [...previas, ...marcas]);
      const detalle = resumen.map(([clase, cuantos]) => `${cuantos} ${clase}`).join(", ");
      setAviso(t("Tapado: {detalle}", { detalle }));
    } catch (e) {
      setAviso(String(e));
    } finally {
      setBuscandoDatos(false);
    }
  }, [buscandoDatos, sessionId, currentIndex, t]);

  /** Copia el texto que haya escrito en el fotograma que se esta viendo. */
  const copiarTexto = useCallback(async () => {
    if (buscandoDatos) return;
    setBuscandoDatos(true);
    setAviso(null);
    try {
      const texto = await copiaTexto(sessionId, currentIndex);
      const palabras = texto.trim().split(/\s+/).length;
      setAviso(t("Texto copiado: {palabras} palabras", { palabras }));
    } catch (e) {
      setAviso(String(e));
    } finally {
      setBuscandoDatos(false);
    }
  }, [buscandoDatos, sessionId, currentIndex, t]);

  // El aviso se va solo: es algo que acaba de pasar, no un estado de la aplicacion, y uno
  // que se queda fijo acaba leyendose como si siguiera pasando.
  useEffect(() => {
    if (!aviso) return;
    const reloj = setTimeout(() => setAviso(null), 5000);
    return () => clearTimeout(reloj);
  }, [aviso]);

  const recorteEnPixeles = useMemo(() => {
    if (!recorte || !session) return "";
    const { width, height } = medidaDelRecorte(recorte, session.region.width, session.region.height);
    return `${width} × ${height}`;
  }, [recorte, session]);

  /**
   * Lo que ocupa la imagen dentro del hueco, sin deformarse y sin salirse, y con el marco
   * del estudio puesto, lo que ocupan su aire y el radio de sus esquinas: la imagen se
   * encoge lo justo para que imagen y marco quepan juntos, como en el archivo.
   */
  const cajaDeLaVista = useMemo(
    () =>
      session
        ? medidasConMarco(
            // El `p-4` del hueco son 16 px por lado que no puede ocupar la imagen.
            medidaDelHueco.width - 32,
            medidaDelHueco.height - 32,
            session.region.width,
            session.region.height,
            estudio.marco,
          )
        : { width: 0, height: 0, aire: 0, radio: 0 },
    [medidaDelHueco, session, estudio.marco],
  );

  // El volumen del estudio también se oye en la vista previa. Hasta el 100 %: un vídeo del
  // navegador no sube de ahí, y por encima solo lo nota el archivo.
  useEffect(() => {
    const video = videoRef.current;
    if (video) video.volume = Math.min(1, Math.max(0, estudio.volumen));
  }, [estudio.volumen, session?.mp4Path]);

  /** Lo que dura lo que se queda. */
  const keptMs = Math.max(0, hastaMs - desdeMs);

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
        markIn();
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
        markOut();
      } else if (e.key === "ArrowLeft") {
        ir(fotogramaVecino(frames, posicionMs, -1));
      } else if (e.key === "ArrowRight") {
        ir(fotogramaVecino(frames, posicionMs, 1));
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
  }, [posicionMs, frames, togglePlay, ir, markIn, markOut, recortando, herramienta, cerrar]);

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
  /**
   * Y por si ese aviso no llega: mientras no haya vídeo, se pregunta cada dos segundos.
   *
   * El aviso viaja por un evento, y un evento que se emite antes de que esta ventana
   * termine de montar su escucha no lo recoge nadie. Eso no pasaba cuando la vista previa
   * tardaba diez segundos, y empezó a pasar el día que empezó a tardar uno: el editor se
   * quedaba con «preparando el vídeo… 100%» puesto para siempre con el MP4 ya escrito en
   * el disco. Preguntar de vez en cuando cuesta nada y no depende de llegar a tiempo.
   */
  useEffect(() => {
    if (!session || session.mp4Path || session.format === "still" || previaFallida) return;
    const reloj = window.setInterval(() => {
      void sessionInfo(sessionId)
        .then((info) => {
          if (info.mp4Path) setSession(info);
        })
        .catch(() => undefined);
    }, 2000);
    return () => window.clearInterval(reloj);
  }, [session, sessionId, previaFallida]);

  /**
   * Las miniaturas de la tira llegan DESPUÉS de que el editor abra: al parar se abre
   * enseguida, sin esperarlas, y cuando están en disco Rust avisa y se vuelven a pedir los
   * fotogramas, ya con sus rutas. Hasta entonces la tira enseña huecos.
   */
  useEffect(() => {
    const unlisten = listen<string>(EVENTS.sessionThumbs, (e) => {
      if (e.payload !== sessionId) return;
      void sessionFrames(sessionId).then(setFrames).catch(() => undefined);
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

  /**
   * Lo que dice el vídeo de si suena. Al pararse, la vista previa se queda donde se quedó
   * el vídeo: mientras suena no se toca el estado (el cabezal lo mueve la línea de tiempo
   * por su cuenta), y repintar el editor cuatro veces por segundo no le hace falta a nadie.
   */
  const onPlaying = useCallback((suena: boolean) => {
    setPlaying(suena);
    const video = videoRef.current;
    if (!suena && video) setPosicionMs(Math.round(video.currentTime * 1000));
  }, []);

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

  // Una captura suelta no tiene tiempo dentro: ni se reproduce, ni se recorta por
  // fotogramas. Los mandos que no puede usar no se ensennan apagados, se quitan.
  const esFoto = session.format === "still";

  /**
   * El sitio del play cuenta en qué punto está el vídeo.
   *
   * Mientras se escribe la vista previa no hay nada que reproducir, así que ahí va el
   * anillo con lo que lleva hecho, en vez de un botón apagado que nadie sabe si está roto
   * o esperando. Y si no va a llegar, lo dice y punto: el recorte y la exportación
   * funcionan igual sin vista previa.
   */
  const sitioDelPlay = videoUrl ? (
    <button
      type="button"
      onClick={togglePlay}
      aria-label={playing ? t("Pausar") : t("Reproducir")}
      title={playing ? t("Pausar") : t("Reproducir")}
      className="flex size-10 items-center justify-center rounded-lg text-white transition-colors hover:bg-white/10"
    >
      {playing ? (
        <Pause className="size-[22px]" fill="currentColor" />
      ) : (
        <Play className="size-[22px] pl-0.5" fill="currentColor" />
      )}
    </button>
  ) : previaFallida ? (
    <span
      className="flex items-center gap-1.5 px-1 text-[13px] whitespace-nowrap text-amber-300"
      title={t("Se puede recortar y exportar igual: lo que no va a funcionar es el play")}
    >
      <VideoOff className="size-5" />
      {t("Sin vista previa")}
    </span>
  ) : (
    <span className="flex items-center gap-2 px-1 text-[13px] whitespace-nowrap text-neutral-200">
      <AnilloDeEspera porCiento={porCiento} />
      {porCiento > 0 ? `${t("Preparando el vídeo…")} ${porCiento}%` : t("Preparando el vídeo…")}
    </span>
  );

  return (
    <div className="flex h-full flex-col overflow-hidden bg-[#161618]">
      <div className="flex min-h-0 flex-1">
        <main className="flex min-w-0 flex-1 flex-col">
          {/*
            Arriba lo que se dibuja sobre la imagen, en medio la imagen, y abajo el tiempo.

            Las herramientas vivían entre el vídeo y la tira, y el play flotaba encima de la
            imagen en una pastilla con «Inicio», «Fin» y «Recortar»: tres sitios distintos
            para lo mismo, y uno de ellos tapando lo que se estaba mirando. Ahora el play,
            el reloj y los cortes están juntos, pegados a la línea de tiempo, que es donde
            los busca cualquiera que haya usado un editor de vídeo.
          */}
          <BarraAnotar
            activa={herramienta}
            onElegir={(cual) => {
              setHerramienta(cual);
              if (cual) setRecortando(false);
            }}
            color={colorMarca}
            onColor={setColorMarca}
            texto={textoMarca}
            onTexto={setTextoMarca}
            cuantas={anotaciones.length}
            onDeshacer={() => setAnotaciones((previas) => previas.slice(0, -1))}
            onBorrarTodo={() => setAnotaciones([])}
            onTaparDatos={() => void taparDatos()}
            onCopiarTexto={() => void copiarTexto()}
            buscandoDatos={buscandoDatos}
            encuadrando={recortando}
            encuadre={recorte ? recorteEnPixeles : null}
            onEncuadrar={() => {
              setRecortando((puesto) => !puesto);
              setHerramienta(null);
            }}
            onQuitarEncuadre={() => {
              setRecorte(null);
              setRecortando(false);
            }}
          />

          <div
            ref={hueco}
            className="relative grid min-h-0 flex-1 place-items-center bg-[repeating-conic-gradient(#18181a_0%_25%,#1d1d20_0%_50%)] bg-[length:24px_24px] p-4"
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
            {/*
              El marco del estudio, en vivo: el fondo con su aire alrededor, y la imagen con
              sus esquinas y su sombra. Sin marco es una caja que no pinta nada.
            */}
            <div
              data-marco={estudio.marco ? estudio.marco.fondo : undefined}
              style={
                estudio.marco && cajaDeLaVista.aire > 0
                  ? {
                      background: cssDelFondo(estudio.marco.fondo, edicion.fondo),
                      padding: cajaDeLaVista.aire,
                      // El vídeo de fondo va dentro con `-z-10`: sin esto se iría detrás de la página.
                      position: "relative",
                      isolation: "isolate",
                    }
                  : undefined
              }
            >
            {estudio.marco && cajaDeLaVista.aire > 0 && <FondoVivo elegido={edicion.fondo} />}
            <div
              // `overflow-hidden` porque la camara del zoom escala el video dentro de
              // esta caja, y lo que se sale por los lados no tiene que verse.
              className="relative overflow-hidden"
              style={{
                borderRadius: cajaDeLaVista.radio || undefined,
                // La misma sombra que pone Rust: cae hacia abajo un 22 % del aire y se
                // difumina en un 45 %.
                boxShadow:
                  estudio.marco?.sombra && cajaDeLaVista.aire > 0
                    ? `0 ${Math.round(cajaDeLaVista.aire * 0.22)}px ${Math.round(cajaDeLaVista.aire * 0.9)}px rgba(0, 0, 0, 0.42)`
                    : undefined,
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
                outMs={hastaMs}
                seekMs={seekMs}
                onFinal={alFinal}
                onPlaying={onPlaying}
                onFallo={setFalloDeVideo}
              />
              {session.format !== "still" && (
                <CapaEstudio
                  videoRef={videoRef}
                  datos={datosEstudio}
                  camara={camara}
                  estudio={estudio}
                  ancho={session.region.width}
                  alto={session.region.height}
                  msParado={posicionMs}
                  reproduciendo={playing}
                  sinEncuadre={herramienta !== null || recortando}
                  zooms={zoomsMostrados}
                  elegido={bloqueElegido}
                  onPunto={(x, y) => {
                    if (zoomElegido !== null) setZoomsAMano(cambiar(zoomsMostrados, zoomElegido, { x, y }));
                  }}
                  // S7 (cursor): el suavizado, esconderlo al escribir, la onda y la estela.
                  edicion={edicion}
                />
              )}
              {/* S8 (cámara en el Estudio) */}
              {/* S9 (presets y tapar) */}
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
            </div>
            {/* Lo que pasa AHORA se cuenta arriba, donde no hay nada que pulsar, y se va solo. */}
            {aviso && (
              <div className="pointer-events-none absolute top-3 left-1/2 max-w-[80%] -translate-x-1/2">
                <span className="block truncate rounded-lg bg-black/85 px-3 py-1.5 text-[13px] text-neutral-100 shadow-lg backdrop-blur-md">
                  {aviso}
                </span>
              </div>
            )}
            {videoUrl && falloDeVideo && (
              <div className="pointer-events-none absolute top-3 left-1/2 max-w-[80%] -translate-x-1/2">
                <span className="block truncate rounded-lg bg-red-950/85 px-3 py-1.5 text-[13px] text-red-200 shadow-lg backdrop-blur-md">
                  {t("Este vídeo no se puede reproducir")} · {falloDeVideo}
                </span>
              </div>
            )}
          </div>

          {/*
            Una captura suelta no tiene tiempo dentro: ni se reproduce, ni se recorta por
            tiempo. Los mandos que no puede usar no se enseñan apagados, se quitan.
          */}
          {!esFoto && (
            <LineaDeTiempo
              frames={frames}
              desdeMs={desdeMs}
              hastaMs={hastaMs}
              posicionMs={posicionMs}
              videoRef={videoRef}
              reproduciendo={playing}
              proporcion={session.region.width / Math.max(1, session.region.height)}
              delante={sitioDelPlay}
              onTramo={cambiarTramo}
              onIr={ir}
              onFinal={alFinal}
              zooms={zoomsMostrados}
              onZooms={setZoomsAMano}
              elegido={bloqueElegido ? zoomElegido : null}
              onElegir={setZoomElegido}
              nivelNuevo={estudio.zoom > 1.05 ? estudio.zoom : NIVEL_NUEVO}
              puntoEn={puntoEn}
              piezas={piezas}
              onPiezas={setPiezas}
              escritura={datosEstudio?.escritura ?? []}
              // S9 (presets y tapar)
            />
          )}
        </main>

        <ExportPanel
          recorte={recorte}
          anotaciones={anotaciones}
          session={session}
          inIndex={inIndex}
          outIndex={outIndex}
          desdeMs={desdeMs}
          hastaMs={hastaMs}
          currentIndex={currentIndex}
          fpsMax={Math.max(15, session.fps)}
          hasFfmpeg={hasFfmpeg}
          saveDirectory={settings.saveDirectory}
          onEstudio={setEstudio}
          edicion={edicion}
          onEdicion={(cambio) => setEdicion((antes) => ({ ...antes, ...cambio }))}
          onRecorte={(nuevo) => {
            setRecorte(nuevo);
            // Puesta la proporción, el marco se enciende para poder moverlo de sitio: es
            // lo primero que se quiere hacer después de elegirla.
            setRecortando(nuevo !== null);
            if (nuevo !== null) setHerramienta(null);
          }}
        />
      </div>
    </div>
  );
}
/**
 * Lo que lleva escrito la vista previa, dibujado donde va a estar el play.
 *
 * Se llena de verdad en vez de dar vueltas: una rueda girando no distingue «va por la
 * mitad» de «se ha colgado», y esto son unos segundos mirando un botón que todavía no
 * funciona. Sin número todavía (los primeros fotogramas) gira un cuarto de anillo, que al
 * menos dice que algo se está moviendo.
 */
function AnilloDeEspera({ porCiento }: { porCiento: number }) {
  const radio = 9;
  const vuelta = 2 * Math.PI * radio;
  const hecho = porCiento > 0 ? porCiento / 100 : 0.25;
  return (
    <svg
      viewBox="0 0 24 24"
      className={`size-5 shrink-0 ${porCiento > 0 ? "" : "animate-spin"}`}
      aria-hidden="true"
    >
      <circle
        cx="12"
        cy="12"
        r={radio}
        fill="none"
        strokeWidth="2.5"
        className="stroke-white/15"
      />
      <circle
        cx="12"
        cy="12"
        r={radio}
        fill="none"
        strokeWidth="2.5"
        strokeLinecap="round"
        className="stroke-blue-400 transition-[stroke-dashoffset] duration-300"
        strokeDasharray={vuelta}
        strokeDashoffset={vuelta * (1 - hecho)}
        transform="rotate(-90 12 12)"
      />
    </svg>
  );
}
