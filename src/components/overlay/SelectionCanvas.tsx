import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  cancelCapture,
  captureAllScreens,
  captureStill,
  copyColor,
  cronoMarca,
  freezeBytes,
  freezePng,
  openSettings,
  overlayBootstrap,
  overlayListo,
  setCaptureFlow,
  startRecording,
} from "../../lib/ipc";
import { clamp } from "../../lib/format";
import { useT } from "../../lib/i18n";
import {
  aPantalla,
  aVirtual,
  esDeEstaPantalla,
  ventanaBajoElPunto,
  ventanasDeEstaPantalla,
} from "../../lib/pantallas";
import {
  EVENTS,
  type CaptureMode,
  type OverlayModeState,
  type OverlayPayload,
  type Rect,
  type StillAction,
} from "../../lib/types";
import { BootScreen } from "./BootScreen";
import { DimensionBadge } from "./DimensionBadge";
import { FloatingToolbar } from "./FloatingToolbar";
import { Magnifier } from "./Magnifier";
import { ModeBar, SELECTOR_BARRA } from "./ModeBar";
import { ScreenPicker } from "./ScreenPicker";
import { SelectionHandles, type HandleId } from "./SelectionHandles";

type Mode =
  | { kind: "idle" }
  | { kind: "drawing"; originX: number; originY: number; candidate: Rect | null }
  | { kind: "moving"; grabX: number; grabY: number; base: Rect }
  | { kind: "resizing"; handle: HandleId; base: Rect };

const MIN_DRAG = 4; // por debajo de esto, un arrastre cuenta como clic

function normalize(ax: number, ay: number, bx: number, by: number): Rect {
  return {
    x: Math.min(ax, bx),
    y: Math.min(ay, by),
    width: Math.abs(bx - ax),
    height: Math.abs(by - ay),
  };
}

function applyHandle(base: Rect, handle: HandleId, x: number, y: number): Rect {
  let left = base.x;
  let top = base.y;
  let right = base.x + base.width;
  let bottom = base.y + base.height;
  if (handle.includes("w")) left = x;
  if (handle.includes("e")) right = x;
  if (handle.startsWith("n")) top = y;
  if (handle.startsWith("s")) bottom = y;
  return normalize(left, top, right, bottom);
}

/** Si un gesto de puntero nacio dentro de la barra de arriba. */
function enLaBarra(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(SELECTOR_BARRA) !== null;
}

function contains(rect: Rect, x: number, y: number): boolean {
  return x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height;
}

/**
 * El fondo del overlay tapa la pantalla entera: si no se pinta, el usuario se
 * queda con un rectangulo negro encima de todo. Por eso hay dos vias.
 *
 * Las dos van por el IPC y salen de la memoria de Rust: ya no hay ningun archivo. La
 * normal trae la pantalla en PNG (dos o tres megabytes); llevarla en crudo, 8 MB por
 * pantalla, era el trozo mas gordo de todo el camino del atajo: 300-400 ms con tres
 * pantallas pidiendola a la vez. La de respaldo la trae sin comprimir, en BMP, por si el
 * PNG fallara.
 */
async function loadFreeze(monitorId: number): Promise<Blob> {
  try {
    const bytes = await freezePng(monitorId);
    if (!bytes || bytes.byteLength === 0) throw new Error("el PNG ha llegado vacio");
    return new Blob([bytes], { type: "image/png" });
  } catch (pngError) {
    console.warn("el PNG del congelado ha fallado, se pide sin comprimir", pngError);
    const bytes = await freezeBytes(monitorId);
    return new Blob([bytes], { type: "image/bmp" });
  }
}

export function SelectionCanvas({ monitorId }: { monitorId: number }) {
  const t = useT();
  const [payload, setPayload] = useState<OverlayPayload | null>(null);
  const [selection, setSelection] = useState<Rect | null>(null);
  const [mode, setMode] = useState<Mode>({ kind: "idle" });
  const [cursor, setCursor] = useState({ x: 0, y: 0 });
  const [hex, setHex] = useState("#000000");
  // El manejador de teclas se registra una vez, asi que lee el color por referencia: con
  // el valor del cierre copiaria siempre el negro con el que arranca.
  const hexRef = useRef(hex);
  hexRef.current = hex;
  /** El ultimo color copiado, para poder decirlo. Se borra solo. */
  const [colorCopiado, setColorCopiado] = useState<string | null>(null);
  useEffect(() => {
    if (!colorCopiado) return;
    const id = setTimeout(() => setColorCopiado(null), 1600);
    return () => clearTimeout(id);
  }, [colorCopiado]);
  /** Qué se hará con el recorte. Lo elige la barra de arriba, antes de recortar. */
  const [modo, setModo] = useState<CaptureMode>("still");
  /** Coger la pantalla entera de un clic, sin arrastrar. */
  const [pantallaEntera, setPantallaEntera] = useState(false);
  /**
   * Si al soltar el recorte sale la barra para elegir que hacer con el.
   *
   * Es el ajuste `captureFlow` puesto donde se usa. Nace de los ajustes guardados y, al
   * tocarlo aqui, se guarda alli: quien lo apaga una vez lo tiene apagado mannana, sin
   * tener que acordarse de ir a la ventana de ajustes a cambiarlo de vuelta.
   */
  const [conBarra, setConBarra] = useState(true);
  const modoRef = useRef<CaptureMode>("still");
  modoRef.current = modo;
  const pantallaRef = useRef(false);
  pantallaRef.current = pantallaEntera;
  const conBarraRef = useRef(true);
  conBarraRef.current = conBarra;

  /**
   * Cambia el estado de la barra en TODAS las pantallas.
   *
   * Hay un overlay por monitor y cada uno tiene su propio React, asi que tocar el boton
   * aqui solo cambiaba esta pantalla. El evento vuelve tambien a quien lo manda, de modo
   * que aplicar el cambio es cosa del listener y no hay dos caminos que mantener.
   */
  const difundir = useCallback((cambio: Partial<OverlayModeState>) => {
    void emit(EVENTS.overlayMode, {
      mode: cambio.mode ?? modoRef.current,
      fullScreen: cambio.fullScreen ?? pantallaRef.current,
      withToolbar: cambio.withToolbar ?? conBarraRef.current,
    } satisfies OverlayModeState);
  }, []);

  /**
   * El interruptor de la barra de acciones: se enciende en todas las pantallas y se
   * guarda en los ajustes.
   *
   * Se guarda con `setCaptureFlow` y no con `setSettings`: aquel reengancha los tres
   * atajos globales y mira si hay que reiniciar el anillo cada vez que se llama, y esto
   * se pulsa con la captura abierta delante.
   */
  const cambiarBarra = useCallback(
    (valor: boolean) => {
      difundir({ withToolbar: valor });
      void setCaptureFlow(valor ? "toolbar" : "instant").catch((e) => {
        // Que no se haya podido guardar no deshace lo que se acaba de encender: la
        // captura de ahora sigue el interruptor igual, y lo unico que se pierde es que
        // la proxima vez vuelva a nacer asi.
        console.warn("no se ha podido guardar el perfil de captura", e);
      });
    },
    [difundir],
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);
  const [source, setSource] = useState<HTMLCanvasElement | null>(null);
  const selectionRef = useRef<Rect | null>(null);
  selectionRef.current = selection;
  /** El numero de ESTA pantalla, para saber si una orden por numero va con nosotros. */
  const numeroPantalla = useRef(0);

  /**
   * Donde se apreto el boton encima de la barra de arriba, mientras no se sepa que es.
   *
   * La barra ocupa una franja del centro de arriba y ahi no se podia empezar a recortar:
   * se quedaba ella el `pointerdown`. Ahora el gesto se decide al moverse, que es la
   * unica forma de que las dos cosas quepan en el mismo sitio: si el puntero se va, se
   * recorta por debajo de la barra; si se suelta sin moverse, el clic es del boton.
   */
  const origenBarra = useRef<{ x: number; y: number } | null>(null);

  /** Pixeles fisicos por pixel CSS: el freeze manda, el webview puede estar escalado por DPI. */
  const scale = useMemo(() => {
    if (!source) return 1;
    return source.width / Math.max(1, window.innerWidth);
  }, [source]);

  /** Si el fondo ya esta dibujado en `source`. Hasta entonces, la pantalla de arranque. */
  const [fondoListo, setFondoListo] = useState(false);
  /** Donde se cuelga el lienzo con la pantalla congelada, que es el fondo que se ve. */
  const fondoRef = useRef<HTMLDivElement>(null);

  /** Ventanas del sistema recortadas a este monitor, ya en coordenadas CSS locales. */
  const snapTargets = useMemo(
    () =>
      payload
        ? ventanasDeEstaPantalla(
            payload.windows,
            payload.monitor,
            scale,
            window.innerWidth,
            window.innerHeight,
          )
        : [],
    [payload, scale],
  );

  /** La ventana mas pequenna bajo el punto es la que esta encima. */
  const windowAt = useCallback(
    (x: number, y: number) => ventanaBajoElPunto(snapTargets, x, y),
    [snapTargets],
  );

  const hovered = useMemo(() => {
    if (selection || mode.kind !== "idle" || pantallaEntera) return null;
    return windowAt(cursor.x, cursor.y);
  }, [windowAt, cursor, selection, mode.kind, pantallaEntera]);

  /**
   * La ventana overlay se reutiliza entre capturas (ver `windows_mgr::open_overlays`): no
   * se desmonta ni se remonta, asi que este arranque no puede depender solo del montaje
   * inicial. `bootIdRef` distingue la invocacion vigente de una anterior que aun estuviera
   * a medias, para que una llegada tardia de la vieja no pise el estado de la nueva.
   */
  const bootIdRef = useRef(0);

  const boot = useCallback(async (payloadListo?: OverlayPayload) => {
    const miId = ++bootIdRef.current;
    const vigente = () => bootIdRef.current === miId;
    const quien = payloadListo?.monitor.id ?? monitorId;
    void cronoMarca(`js-evento-${quien}`);

    // Se limpia lo de la vez anterior ANTES de pedir nada: si esta ventana se reutiliza,
    // sin esto se veria un instante la captura vieja antes de que llegue la nueva.
    setPayload(null);
    setFondoListo(false);
    setSource(null);
    setSelection(null);
    setMode({ kind: "idle" });
    setPantallaEntera(false);
    setBusy(false);
    // `conBarra` NO se reinicia aqui: lo pone el payload unas lineas mas abajo con lo que
    // digan los ajustes, y ponerlo a un valor de fabrica antes seria ensennar la barra
    // encendida un instante a quien la tiene apagada.
    setError(null);
    setBootError(null);

    try {
      // Cuando se reutiliza una ventana, el backend manda el payload ya hecho en el
      // propio evento (ver EVENTS.overlayShow): pedirlo aparte por invoke seria una
      // vuelta de IPC completa que no hace falta. Solo en el primer montaje, cuando
      // nadie nos lo ha dado, se pide.
      const data = payloadListo ?? (await overlayBootstrap(monitorId));
      if (!vigente()) return;
      setPayload(data);
      numeroPantalla.current = data.screenNumber;
      // Quien pulsa el atajo de grabar quiere grabar: la barra abre en vídeo y ya.
      setModo(data.intent === "record" ? "video" : "still");
      setConBarra(data.settings.captureFlow === "toolbar");
      void getCurrentWindow().setFocus();

      // Aqui va el id de `data.monitor.id` (el de ESTA captura, siempre correcto),
      // NUNCA el prop `monitorId` de la URL: esta ventana se reutiliza entre capturas
      // (ver windows_mgr::open_overlays) y ese prop se fijo la primera vez que se creo,
      // que puede no coincidir con el id actual si el orden de los monitores del sistema
      // cambio entre medias. Pedir el id equivocado es ensennar el freeze de otro monitor.
      const blob = await loadFreeze(data.monitor.id);
      if (!vigente()) return;
      void cronoMarca(`js-bytes-${quien}`);

      // Un solo lienzo hace de fondo y de fuente para la lupa: decodificar una vez, no dos
      // (antes el fondo era un <img> con su propia decodificacion ademas de esta).
      const bitmap = await createImageBitmap(blob);
      if (!vigente()) return;
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      canvas.className = "pointer-events-none absolute inset-0 h-full w-full";
      canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
      bitmap.close?.();
      setSource(canvas);
      setFondoListo(true);
      void cronoMarca(`js-canvas-${quien}`);
    } catch (e) {
      if (vigente()) setBootError(String(e));
    }
  }, [monitorId]);

  useEffect(() => {
    void boot();
  }, [boot]);

  // El lienzo con la pantalla congelada se cuelga en el DOM a mano: React no lo redibuja,
  // y asi el mismo elemento que lee la lupa es el que se ve.
  useLayoutEffect(() => {
    const hueco = fondoRef.current;
    if (!hueco || !source || !fondoListo) return;
    hueco.replaceChildren(source);
    return () => {
      hueco.replaceChildren();
    };
  }, [source, fondoListo]);

  /**
   * Con el fondo colgado, se le dice a Rust que ya puede ensennar esta ventana: hasta
   * entonces esta aparcada fuera de las pantallas y nadie ve la pantalla de carga.
   *
   * Se espera al siguiente cuadro (`requestAnimationFrame`), que es el que lleva el lienzo
   * recien colgado: para cuando Rust reciba el aviso y mueva la ventana, ese cuadro ya
   * esta pintado. Con dos cuadros se esperaba de mas (medido: 16 ms). Y un navegador
   * aparcado fuera de las pantallas puede no pintar cuadros: si en 30 ms no ha llegado
   * ninguno, se avisa igual.
   */
  useEffect(() => {
    if (!fondoListo || !payload) return;
    const { id } = payload.monitor;
    const generacion = payload.generation;
    let avisado = false;
    const avisar = () => {
      if (avisado) return;
      avisado = true;
      void cronoMarca(`js-pintado-${id}`);
      void overlayListo(id, generacion);
    };
    const uno = requestAnimationFrame(avisar);
    const plazo = window.setTimeout(avisar, 30);
    return () => {
      cancelAnimationFrame(uno);
      window.clearTimeout(plazo);
    };
  }, [fondoListo, payload]);

  // La ventana ya estaba montada de una captura anterior: no llega un remontaje que
  // dispare el arranque solo, asi que el backend lo pide explicitamente por evento, con
  // el payload de la captura nueva ya dentro.
  //
  // `target` NO es opcional aqui, aunque el tipo lo deje pasar. Sin el, `listen` se
  // registra como `{ kind: 'Any' }` y eso significa recibir TODO lo que se emita, incluso
  // lo que iba dirigido a otra ventana (@tauri-apps/api/event.d.ts: "defaults to
  // { kind: 'Any' }"). Con un overlay por monitor, cada ventana recibia tambien los
  // payloads de las otras dos y se quedaba con el ultimo del bucle de `open_overlays`:
  // las tres acababan pintando y recortando la misma pantalla. Arreglarlo en Rust con
  // `emit_to` era necesario y no bastaba, porque el que se apuntaba a todo era este lado.
  // Se pasa la etiqueta como cadena a proposito: asi los dos lados usan `AnyLabel` con la
  // misma etiqueta y coinciden sin depender de que tipo de destino sea cada uno.
  useEffect(() => {
    const etiqueta = getCurrentWindow().label;
    const unlisten = listen<OverlayPayload>(EVENTS.overlayShow, (e) => void boot(e.payload), {
      target: etiqueta,
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [boot]);

  const toPhysical = useCallback(
    (rect: Rect): Rect => aVirtual(rect, payload!.monitor, scale),
    [payload, scale],
  );

  const capturarRegion = useCallback(
    async (rect: Rect, action: StillAction) => {
      if (!payload || busy) return;
      setBusy(true);
      setError(null);
      try {
        await captureStill(toPhysical(rect), action);
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [payload, busy, toPhysical],
  );

  /**
   * La ultima region capturada, traida a coordenadas CSS de ESTA pantalla, o null si fue
   * en otra. Es la vuelta de `toPhysical`: el payload la trae en coordenadas del
   * escritorio virtual justamente porque puede no ser de aqui.
   *
   * Se mira por el centro, igual que hace Rust al decidir de que pantalla recorta, para
   * que las dos mitades esten de acuerdo sobre de quien es una region.
   */
  const ultimaRegion = useMemo(() => {
    const r = payload?.lastRegion;
    if (!r || !payload) return null;
    if (!esDeEstaPantalla(r, payload.monitor)) return null;
    return aPantalla(r, payload.monitor, scale);
  }, [payload, scale]);

  const ultimaRegionRef = useRef<Rect | null>(null);
  ultimaRegionRef.current = ultimaRegion;

  const capturarTodasLasPantallas = useCallback(
    async (action: StillAction) => {
      if (busy) return;
      setBusy(true);
      setError(null);
      try {
        await captureAllScreens(action);
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [busy],
  );

  const runStill = useCallback(
    async (action: StillAction) => {
      if (!selection) return;
      await capturarRegion(selection, action);
    },
    [selection, capturarRegion],
  );

  /**
   * Perfil al vuelo: se suelta el raton y ya esta, sin barra y sin un solo clic mas. En
   * foto eso es la imagen en el portapapeles; en video o GIF, la grabacion arrancando.
   * Con el otro perfil sale la barra, que ademas deja ajustar el recuadro antes: grabando
   * eso importa mas que en una foto, porque lo que salga mal se ve minutos despues.
   */
  const alVuelo = !conBarra;

  const grabarRegion = useCallback(
    async (rect: Rect, format: "gif" | "video") => {
      if (!payload || busy) return;
      setBusy(true);
      setError(null);
      try {
        await startRecording(toPhysical(rect), {
          format,
          fps: payload.settings.fps,
          captureCursor: payload.settings.captureCursor,
          // El interruptor de audio del overlay se fue con la barra vieja: mientras el
          // ajuste diga "todavia no disponible", esto es siempre false.
          audio: payload.settings.recordAudio && format === "video",
          // El GIF no lleva sonido, asi que el microfono tampoco tiene sentido ahi.
          microphone: payload.settings.recordMicrophone && format === "video",
          // Los aros si valen igual en GIF: el fotograma es el mismo.
          highlightClicks: payload.settings.highlightClicks,
          highlightKeys: payload.settings.highlightKeys,
        });
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [payload, busy, toPhysical],
  );

  const nudge = useCallback((dx: number, dy: number, resize: boolean) => {
    setSelection((prev) => {
      if (!prev) return prev;
      if (resize) {
        return {
          ...prev,
          width: Math.max(2, prev.width + dx),
          height: Math.max(2, prev.height + dy),
        };
      }
      return { ...prev, x: prev.x + dx, y: prev.y + dy };
    });
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const step = e.shiftKey ? 10 : 1;
      switch (e.key) {
        case "Escape":
          e.preventDefault();
          // Un solo Escape se sale, haya recorte hecho o no. Antes el primero borraba el
          // recorte y hacia falta otro para cerrar: quien pulsa Escape encima de una
          // captura quiere irse, y el recorte se rehace arrastrando otra vez, sin tener
          // que borrarlo antes.
          void cancelCapture();
          return;
        case "Enter":
          e.preventDefault();
          // Enter hace lo que diga la barra de arriba: copiar la foto, o empezar a grabar.
          if (modo !== "still") {
            if (selectionRef.current) void grabarRegion(selectionRef.current, modo);
          } else {
            void runStill("copy");
          }
          return;
        case "ArrowLeft":
          e.preventDefault();
          nudge(-step, 0, e.altKey);
          return;
        case "ArrowRight":
          e.preventDefault();
          nudge(step, 0, e.altKey);
          return;
        case "ArrowUp":
          e.preventDefault();
          nudge(0, -step, e.altKey);
          return;
        case "ArrowDown":
          e.preventDefault();
          nudge(0, step, e.altKey);
          return;
      }
      const key = e.key.toLowerCase();
      if (key === "s" && e.ctrlKey) {
        e.preventDefault();
        void runStill("save");
      } else if (key === "a" && e.ctrlKey) {
        e.preventDefault();
        const todo = { x: 0, y: 0, width: window.innerWidth, height: window.innerHeight };
        if (!alVuelo) setSelection(todo);
        else if (modo !== "still") void grabarRegion(todo, modo);
        else void capturarRegion(todo, "copy");
      } else if (key === "e") {
        void runStill("edit");
      } else if (key === "a") {
        // Anclar. `Ctrl+A` de aqui arriba es otra cosa (toda la pantalla), y no chocan:
        // la letra sola actua sobre el recorte y con Ctrl cambia lo que se recorta.
        void runStill("pin");
      } else if (key === "t") {
        // El texto de la captura al portapapeles, leido por el motor de Windows.
        void runStill("text");
      } else if (key === "c") {
        // El color que hay debajo del cursor. La lupa ya lo ensennaba y no habia forma de
        // llevarselo; asi el cuentagotas no es una herramienta mas, es una tecla.
        //
        // No cierra la seleccion: quien esta cogiendo un color suele coger varios, y
        // cerrar despues del primero obligaria a volver a lanzar el atajo cada vez.
        void copyColor(hexRef.current)
          .then(() => setColorCopiado(hexRef.current))
          .catch((err) => setError(String(err)));
      } else if (key === "p") {
        difundir({ fullScreen: !pantallaRef.current });
      } else if (key === "b") {
        // La barra de acciones, encendida o apagada. Con `Alt` no: es una letra sola como
        // las demas de aqui, y el `b` de "barra" no lo usa nada mas.
        cambiarBarra(!conBarraRef.current);
      } else if (key === "f") {
        difundir({ mode: "still" });
      } else if (key === "g") {
        difundir({ mode: "gif" });
      } else if (key === "v") {
        difundir({ mode: "video" });
      } else if (key === "r") {
        // La misma zona otra vez, sin volver a arrastrar. Solo responde la pantalla donde
        // cayo aquella region: en las demas no hay nada que repetir, asi que no hacen nada
        // en vez de capturar algo que no es.
        const previa = ultimaRegionRef.current;
        if (!previa) return;
        e.preventDefault();
        // Mismo reparto que `Ctrl+A`: con la barra se deja puesta para retocarla, y con el
        // perfil "se copia sola" se hace y ya, que es lo que ese perfil promete.
        if (!alVuelo) setSelection(previa);
        else if (modoRef.current !== "still") void grabarRegion(previa, modoRef.current);
        else void capturarRegion(previa, "copy");
      } else if (key === "0") {
        // Todas las pantallas de golpe, en una sola imagen. El `0` va con los numeros de
        // pantalla y significa "ninguna en concreto: todas".
        //
        // Esta no necesita ir por evento como las otras: no hay que decidir a que ventana
        // le toca, porque Rust las junta todas leyendo las capturas congeladas. La que
        // tiene el foco la pide y ya.
        //
        // Solo tiene sentido en foto: grabar es de una region de una pantalla, asi que en
        // video o GIF esta tecla no hace nada en vez de hacer algo raro.
        if (modoRef.current !== "still") return;
        e.preventDefault();
        void capturarTodasLasPantallas("copy");
      } else if (key >= "1" && key <= "9") {
        // El numero que se ve en cada pantalla es la tecla que se la lleva. La orden va
        // por evento porque la tecla solo llega a la pantalla que tiene el foco, y casi
        // nunca es la que se quiere capturar.
        e.preventDefault();
        void emit(EVENTS.overlayTakeScreen, Number(key));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [
    runStill,
    nudge,
    alVuelo,
    modo,
    capturarRegion,
    capturarTodasLasPantallas,
    grabarRegion,
    difundir,
    cambiarBarra,
  ]);

  const readHex = useCallback(
    (cssX: number, cssY: number) => {
      if (!source) return;
      const ctx = source.getContext("2d", { willReadFrequently: true });
      if (!ctx) return;
      const px = clamp(Math.floor(cssX * scale), 0, source.width - 1);
      const py = clamp(Math.floor(cssY * scale), 0, source.height - 1);
      const data = ctx.getImageData(px, py, 1, 1).data;
      const toHex = (c: number) => c.toString(16).padStart(2, "0");
      setHex("#" + toHex(data[0]) + toHex(data[1]) + toHex(data[2]));
    },
    [source, scale],
  );

  // Durante el gesto los eventos se escuchan en window, no en el div: asi el arrastre
  // no se pierde aunque el webview no conceda la captura de puntero.
  useEffect(() => {
    if (mode.kind === "idle") return;

    const onMove = (e: PointerEvent) => {
      const x = e.clientX;
      const y = e.clientY;
      setCursor({ x, y });
      if (mode.kind === "drawing") {
        // La lupa sigue en pantalla mientras se dibuja, asi que el color tiene
        // que seguir al cursor en vez de quedarse en el del primer clic.
        readHex(x, y);
        setSelection(normalize(mode.originX, mode.originY, x, y));
      } else if (mode.kind === "moving") {
        setSelection({
          x: clamp(x - mode.grabX, 0, window.innerWidth - mode.base.width),
          y: clamp(y - mode.grabY, 0, window.innerHeight - mode.base.height),
          width: mode.base.width,
          height: mode.base.height,
        });
      } else if (mode.kind === "resizing") {
        setSelection(applyHandle(mode.base, mode.handle, x, y));
      }
    };

    const onUp = () => {
      if (mode.kind === "drawing") {
        const drawn = selectionRef.current;
        const dibujado = drawn && drawn.width >= MIN_DRAG && drawn.height >= MIN_DRAG;
        // Clic seco: si hay una ventana debajo, se selecciona entera (estilo ShareX).
        if (!dibujado) setSelection(mode.candidate);
        const elegido = dibujado ? drawn : mode.candidate;
        // Al soltar manda el modo elegido arriba: grabar empieza aqui mismo, y la foto
        // solo se copia sola si ese es el perfil. Si no, sale la barra de la seleccion.
        if (elegido) {
          if (modo !== "still") void grabarRegion(elegido, modo);
          else if (alVuelo) void capturarRegion(elegido, "copy");
        }
      }
      setMode({ kind: "idle" });
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }, [mode, readHex, alVuelo, modo, capturarRegion, grabarRegion]);

  /** Se lleva esta pantalla entera con lo que diga la barra: foto, video o GIF. */
  const llevarsePantalla = useCallback(async () => {
    const todo = {
      x: 0,
      y: 0,
      width: window.innerWidth,
      height: window.innerHeight,
    };
    difundir({ fullScreen: false });
    if (modoRef.current !== "still") await grabarRegion(todo, modoRef.current);
    else await capturarRegion(todo, "copy");
  }, [difundir, capturarRegion, grabarRegion]);

  // Las dos ordenes que llegan de las otras pantallas: que cambie la barra, y que esta
  // pantalla en concreto se capture entera porque alguien pulso su numero.
  useEffect(() => {
    const modos = listen<OverlayModeState>(EVENTS.overlayMode, ({ payload: p }) => {
      setModo(p.mode);
      setPantallaEntera(p.fullScreen);
      setConBarra(p.withToolbar);
      if (p.fullScreen) setSelection(null);
    });
    const numeros = listen<number>(EVENTS.overlayTakeScreen, ({ payload: n }) => {
      if (n === numeroPantalla.current) void llevarsePantalla();
    });
    return () => {
      void modos.then((fn) => fn());
      void numeros.then((fn) => fn());
    };
  }, [llevarsePantalla]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (busy) return;
    // Lo que nace encima de la barra todavia no es nada: se apunta donde empezo y se
    // espera a ver si el puntero se mueve (recorte por debajo) o se suelta (clic del
    // boton). Ver `origenBarra`.
    if (enLaBarra(e.target)) {
      origenBarra.current = { x: e.clientX, y: e.clientY };
      return;
    }
    // Con "pantalla entera" puesto no hay nada que arrastrar: donde caiga el clic, esa
    // pantalla se lleva, y se lleva YA. Un clic es un clic, tambien con el perfil de la
    // barra: quien pide la pantalla entera ya ha dicho lo que quiere.
    if (pantallaEntera) {
      void llevarsePantalla();
      return;
    }
    const x = e.clientX;
    const y = e.clientY;
    const current = selectionRef.current;
    if (current && contains(current, x, y)) {
      setMode({ kind: "moving", grabX: x - current.x, grabY: y - current.y, base: current });
      return;
    }
    setMode({ kind: "drawing", originX: x, originY: y, candidate: windowAt(x, y)?.rect ?? null });
    setSelection({ x, y, width: 0, height: 0 });
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const desdeLaBarra = origenBarra.current;
    if (desdeLaBarra) {
      // `buttons` a cero es que ya se solto: aquello era un clic de la barra y este
      // movimiento no arrastra nada. Sin esta linea, mover el raton despues de pulsar un
      // boton se ponia a dibujar un recorte solo.
      if (e.buttons === 0 || pantallaEntera || busy) {
        origenBarra.current = null;
      } else if (
        Math.abs(e.clientX - desdeLaBarra.x) >= MIN_DRAG ||
        Math.abs(e.clientY - desdeLaBarra.y) >= MIN_DRAG
      ) {
        origenBarra.current = null;
        // Sin candidata: quien arrastra desde la barra quiere SU recorte, no la ventana
        // que haya debajo.
        setMode({
          kind: "drawing",
          originX: desdeLaBarra.x,
          originY: desdeLaBarra.y,
          candidate: null,
        });
        setSelection(normalize(desdeLaBarra.x, desdeLaBarra.y, e.clientX, e.clientY));
        return;
      }
    }
    if (mode.kind !== "idle") return;
    setCursor({ x: e.clientX, y: e.clientY });
    readHex(e.clientX, e.clientY);
  };

  if (!payload || !fondoListo) {
    return <BootScreen error={bootError} />;
  }

  const active =
    selection !== null && selection.width >= MIN_DRAG && selection.height >= MIN_DRAG
      ? selection
      : null;
  const highlight = !active && hovered ? hovered : null;
  const toolbarFlip = active ? active.y + active.height + 62 > window.innerHeight : false;
  const magnifierVisible =
    payload.settings.showMagnifier && !pantallaEntera && (!active || mode.kind === "drawing");

  return (
    <div
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      className="relative h-screen w-screen overflow-hidden"
      style={{ cursor: pantallaEntera ? "pointer" : active ? "default" : "crosshair" }}
    >
      {/* El fondo: el lienzo con la pantalla congelada, colgado aqui por el efecto de
          arriba. Un lienzo y no un <img>, para decodificar la imagen una sola vez. */}
      <div ref={fondoRef} className="pointer-events-none absolute inset-0" />

      {/* Sin seleccion: velo uniforme. Con seleccion: el velo lo dibuja la sombra del recuadro. */}
      {!active && <div className="pointer-events-none absolute inset-0 bg-black/45" />}

      {/* La ventana de debajo del cursor, con su nombre: el marco solo, y encima azul
          fuerte, no decia de que era ni que se le podia hacer clic. */}
      {highlight && (
        <div
          style={{
            left: highlight.rect.x,
            top: highlight.rect.y,
            width: highlight.rect.width,
            height: highlight.rect.height,
          }}
          className="pointer-events-none absolute rounded-[3px] border border-white/45 bg-white/[0.06]"
        >
          {highlight.title && (
            <span
              // Fuera del marco cuando cabe: dentro se pone justo encima de la barra de
              // titulo de la ventana y se leen dos titulos pisados uno sobre otro.
              className={`absolute left-0 max-w-[min(360px,100%)] truncate rounded-md bg-neutral-900/90 px-2 py-1 text-[11px] text-neutral-200 shadow-lg backdrop-blur-sm ${
                highlight.rect.y > 28 ? "-top-7" : "top-1.5 left-1.5"
              }`}
            >
              {highlight.title}
            </span>
          )}
        </div>
      )}

      {active && (
        <>
          <div
            style={{
              left: active.x,
              top: active.y,
              width: active.width,
              height: active.height,
              boxShadow: "0 0 0 100vmax rgba(0,0,0,0.45)",
            }}
            className="pointer-events-none absolute border border-blue-500/90"
          />
          <SelectionHandles
            rect={active}
            onGrab={(handle) => setMode({ kind: "resizing", handle, base: active })}
          />
          <DimensionBadge
            width={active.width * scale}
            height={active.height * scale}
            left={active.x + 2}
            top={active.y > 26 ? active.y - 26 : active.y + 6}
          />
        </>
      )}

      {/*
        La cruceta: dos guias que cruzan la pantalla por donde esta el cursor. Alinear el
        borde de una seleccion con algo que esta al otro lado de la pantalla se hacia a
        ojo; con la guia se ve si estan en la misma linea antes de soltar.
        Se pinta cuando se pinta la lupa, que es cuando hace falta precision, y desaparece
        con la seleccion hecha para no ensuciar lo que se esta mirando.
      */}
      {magnifierVisible && (
        <div className="pointer-events-none absolute inset-0 z-30">
          <div
            className="absolute inset-x-0 h-px bg-white/25 mix-blend-difference"
            style={{ top: cursor.y }}
          />
          <div
            className="absolute inset-y-0 w-px bg-white/25 mix-blend-difference"
            style={{ left: cursor.x }}
          />
        </div>
      )}

      {magnifierVisible && source && (
        <Magnifier
          source={source}
          px={cursor.x * scale}
          py={cursor.y * scale}
          left={clamp(cursor.x + 18, 0, window.innerWidth - 148)}
          top={clamp(cursor.y + 18, 0, window.innerHeight - 168)}
          hex={hex}
        />
      )}

      <AnimatePresence>
        {active && mode.kind === "idle" && !alVuelo && (
          <FloatingToolbar
            key="toolbar"
            left={clamp(active.x + active.width / 2, 190, window.innerWidth - 190)}
            top={toolbarFlip ? active.y - 10 : active.y + active.height + 10}
            flipped={toolbarFlip}
            busy={busy}
            modo={modo}
            onCopy={() => void runStill("copy")}
            onSave={() => void runStill("save")}
            onEdit={() => void runStill("edit")}
            onPin={() => void runStill("pin")}
            onText={() => void runStill("text")}
            onRecord={() => {
              if (selection && modo !== "still") void grabarRegion(selection, modo);
            }}
            onCancel={() => void cancelCapture()}
          />
        )}
      </AnimatePresence>

      {/*
        El color copiado. Va donde los errores y con la misma forma, porque es lo mismo:
        algo que acaba de pasar y que hay que contar sin tapar la pantalla.
      */}
      {colorCopiado && !error && (
        <div className="pointer-events-none absolute inset-x-0 top-8 flex justify-center">
          <div className="flex items-center gap-2 rounded-xl border border-white/15 bg-neutral-900/90 px-4 py-2.5 text-xs text-neutral-200 shadow-2xl backdrop-blur-md">
            <span
              className="size-3 rounded-[4px] border border-white/25"
              style={{ backgroundColor: colorCopiado }}
            />
            <span className="font-mono uppercase">{colorCopiado}</span>
            <span className="text-neutral-400">{t("copiado")}</span>
          </div>
        </div>
      )}

      {error && (
        <div className="pointer-events-none absolute inset-x-0 top-8 flex justify-center">
          <div className="max-w-xl rounded-xl border border-red-500/30 bg-red-950/90 px-4 py-2.5 text-xs text-red-200 shadow-2xl backdrop-blur-md">
            {/* Los errores llegan de Rust escritos en espannol. Como la clave del
                diccionario ES la frase espannola, pasarlos por `t` traduce los que
                alguien ha traducido y deja el resto tal cual, que es mejor que verlos
                todos en castellano con la aplicacion en ingles. */}
            {t(error)}
          </div>
        </div>
      )}

      {/*
        El fantasma de la ultima captura, donde estuvo, con su tecla encima. Una tecla que
        no se ve no la usa nadie, y esto se explica solo: ahi tenias lo de antes, pulsa R y
        vuelve. Se va en cuanto se empieza a arrastrar, para no estorbar.
      */}
      {ultimaRegion && !selection && mode.kind === "idle" && !pantallaEntera && !hovered && (
        <div
          className="pointer-events-none absolute rounded-[3px] border border-dashed border-white/35"
          style={{
            left: ultimaRegion.x,
            top: ultimaRegion.y,
            width: ultimaRegion.width,
            height: ultimaRegion.height,
          }}
        >
          <span className="absolute -top-7 left-0 flex items-center gap-1.5 rounded-full border border-white/10 bg-neutral-900/85 px-2.5 py-1 text-[11px] whitespace-nowrap text-neutral-400 shadow-lg backdrop-blur-md">
            <kbd className="rounded-[4px] border border-white/15 bg-white/10 px-1.5 py-0.5 text-[10px] leading-none font-medium text-neutral-200">
              R
            </kbd>
            la de antes
          </span>
        </div>
      )}

      {pantallaEntera && !active && (
        <ScreenPicker
          numero={payload.screenNumber}
          total={payload.screenCount}
          modo={modo}
          ancho={payload.monitor.width}
          alto={payload.monitor.height}
        />
      )}

      <ModeBar
        value={modo}
        onChange={(m) => difundir({ mode: m })}
        pantallaEntera={pantallaEntera}
        onPantallaEntera={(v) => difundir({ fullScreen: v })}
        conBarra={conBarra}
        onConBarra={cambiarBarra}
        onAjustes={() => void openSettings()}
        onCancel={() => void cancelCapture()}
        dimmed={mode.kind !== "idle"}
      />
    </div>
  );
}
