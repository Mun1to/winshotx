import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { convertFileSrc } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  cancelCapture,
  captureStill,
  freezeBytes,
  overlayBootstrap,
  startRecording,
} from "../../lib/ipc";
import { clamp } from "../../lib/format";
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
import { ModeBar } from "./ModeBar";
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

function contains(rect: Rect, x: number, y: number): boolean {
  return x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height;
}

/**
 * El fondo del overlay tapa la pantalla entera: si no se pinta, el usuario se
 * queda con un rectangulo negro encima de todo. Por eso hay dos vias.
 */
async function loadFreeze(path: string, monitorId: number): Promise<Blob> {
  try {
    // Via rapida: el protocolo asset sirve el archivo sin copiarlo por el IPC.
    // (Se probo lanzar esta via y la de IPC a la vez, tomando la que respondiera antes,
    // pero con las tres ventanas pidiendolo simultaneamente competian por el mismo disco
    // y salia peor que dejar una sola via fija: descartado.)
    //
    // El nombre del archivo es siempre el mismo entre capturas (freeze-N.bmp: el indice es
    // del monitor, no de la captura), asi que si el navegador cachea por URL sin mirar que
    // el contenido cambio, serviria el freeze de la vez anterior. `cache: "no-store"` mas
    // un parametro que cambia siempre es la doble seguridad de que esto nunca pasa.
    const url = convertFileSrc(path);
    const sinCache = `${url}${url.includes("?") ? "&" : "?"}t=${crypto.randomUUID()}`;
    const response = await fetch(sinCache, { cache: "no-store" });
    if (!response.ok) throw new Error(`asset devolvio ${response.status}`);
    const blob = await response.blob();
    if (blob.size === 0) throw new Error("el asset ha llegado vacio");
    return blob;
  } catch (assetError) {
    // Via de respaldo: los bytes por el IPC. No depende ni de la CSP ni del ambito del
    // protocolo asset.
    console.warn("el protocolo asset ha fallado, se tira del IPC", assetError);
    const bytes = await freezeBytes(monitorId);
    return new Blob([bytes], { type: "image/bmp" });
  }
}

export function SelectionCanvas({ monitorId }: { monitorId: number }) {
  const [payload, setPayload] = useState<OverlayPayload | null>(null);
  const [selection, setSelection] = useState<Rect | null>(null);
  const [mode, setMode] = useState<Mode>({ kind: "idle" });
  const [cursor, setCursor] = useState({ x: 0, y: 0 });
  const [hex, setHex] = useState("#000000");
  /** Qué se hará con el recorte. Lo elige la barra de arriba, antes de recortar. */
  const [modo, setModo] = useState<CaptureMode>("still");
  /** Coger la pantalla entera de un clic, sin arrastrar. */
  const [pantallaEntera, setPantallaEntera] = useState(false);
  const modoRef = useRef<CaptureMode>("still");
  modoRef.current = modo;
  const pantallaRef = useRef(false);
  pantallaRef.current = pantallaEntera;

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
    } satisfies OverlayModeState);
  }, []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);
  const [source, setSource] = useState<HTMLCanvasElement | null>(null);
  const selectionRef = useRef<Rect | null>(null);
  selectionRef.current = selection;
  /** El numero de ESTA pantalla, para saber si una orden por numero va con nosotros. */
  const numeroPantalla = useRef(0);

  /** Pixeles fisicos por pixel CSS: el freeze manda, el webview puede estar escalado por DPI. */
  const scale = useMemo(() => {
    if (!source) return 1;
    return source.width / Math.max(1, window.innerWidth);
  }, [source]);

  const [freezeUrl, setFreezeUrl] = useState<string | null>(null);

  /** Ventanas del sistema recortadas a este monitor, ya en coordenadas CSS locales. */
  const snapTargets = useMemo(() => {
    if (!payload) return [];
    const m = payload.monitor;
    return payload.windows
      .map((w) => ({
        title: w.title,
        rect: {
          x: (w.rect.x - m.x) / scale,
          y: (w.rect.y - m.y) / scale,
          width: w.rect.width / scale,
          height: w.rect.height / scale,
        },
      }))
      .filter(
        (w) =>
          w.rect.width > 8 &&
          w.rect.height > 8 &&
          w.rect.x < window.innerWidth &&
          w.rect.y < window.innerHeight &&
          w.rect.x + w.rect.width > 0 &&
          w.rect.y + w.rect.height > 0,
      );
  }, [payload, scale]);

  /** La ventana mas pequenna bajo el punto es la que esta encima. */
  const windowAt = useCallback(
    (x: number, y: number): { title: string; rect: Rect } | null => {
      const inside = snapTargets.filter((w) => contains(w.rect, x, y));
      if (inside.length === 0) return null;
      return inside.reduce((best, w) =>
        w.rect.width * w.rect.height < best.rect.width * best.rect.height ? w : best,
      );
    },
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
  const freezeUrlRef = useRef<string | null>(null);
  freezeUrlRef.current = freezeUrl;

  const boot = useCallback(async (payloadListo?: OverlayPayload) => {
    const miId = ++bootIdRef.current;
    const vigente = () => bootIdRef.current === miId;

    // Se limpia lo de la vez anterior ANTES de pedir nada: si esta ventana se reutiliza,
    // sin esto se veria un instante la captura vieja antes de que llegue la nueva.
    if (freezeUrlRef.current) URL.revokeObjectURL(freezeUrlRef.current);
    setPayload(null);
    setFreezeUrl(null);
    setSource(null);
    setSelection(null);
    setMode({ kind: "idle" });
    setPantallaEntera(false);
    setBusy(false);
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
      void getCurrentWindow().setFocus();

      // El PNG se pasa a un blob del mismo origen: cargado directamente desde el
      // protocolo asset, el canvas quedaria contaminado y la lupa no podria leer
      // ni un pixel.
      //
      // Aqui va el id de `data.monitor.id` (el de ESTA captura, siempre correcto),
      // NUNCA el prop `monitorId` de la URL: esta ventana se reutiliza entre capturas
      // (ver windows_mgr::open_overlays) y ese prop se fijo la primera vez que se creo,
      // que puede no coincidir con el id actual si el orden de los monitores del sistema
      // cambio entre medias. Solo importa para la via de respaldo por IPC (loadFreeze),
      // y pedir ahi el id equivocado significaba ensennar el freeze de otro monitor.
      const blob = await loadFreeze(data.freezePath, data.monitor.id);
      if (!vigente()) return;
      const objectUrl = URL.createObjectURL(blob);
      setFreezeUrl(objectUrl);

      const bitmap = await createImageBitmap(blob);
      if (!vigente()) return;
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
      setSource(canvas);
    } catch (e) {
      if (vigente()) setBootError(String(e));
    }
  }, [monitorId]);

  useEffect(() => {
    void boot();
  }, [boot]);

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

  useEffect(() => {
    return () => {
      if (freezeUrlRef.current) URL.revokeObjectURL(freezeUrlRef.current);
    };
  }, []);

  const toPhysical = useCallback(
    (rect: Rect): Rect => {
      const m = payload!.monitor;
      return {
        x: Math.round(m.x + rect.x * scale),
        y: Math.round(m.y + rect.y * scale),
        width: Math.max(2, Math.round(rect.width * scale)),
        height: Math.max(2, Math.round(rect.height * scale)),
      };
    },
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
  const alVuelo = payload?.settings.captureFlow === "instant";

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
          if (selectionRef.current) setSelection(null);
          else void cancelCapture();
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
      } else if (key === "p") {
        difundir({ fullScreen: !pantallaRef.current });
      } else if (key === "f") {
        difundir({ mode: "still" });
      } else if (key === "g") {
        difundir({ mode: "gif" });
      } else if (key === "v") {
        difundir({ mode: "video" });
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
  }, [runStill, nudge, alVuelo, modo, capturarRegion, grabarRegion, difundir]);

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
    if (mode.kind !== "idle") return;
    setCursor({ x: e.clientX, y: e.clientY });
    readHex(e.clientX, e.clientY);
  };

  if (!payload || !freezeUrl) {
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
      {/* La key fuerza a React a desmontar y crear un <img> nuevo en cada captura, en vez
          de reutilizar el elemento cambiandole el src: asi no hay forma de que el
          navegador siga pintando el frame anterior mientras decide si actualizar. */}
      <img
        key={freezeUrl}
        src={freezeUrl}
        alt=""
        draggable={false}
        className="pointer-events-none absolute inset-0 h-full w-full"
      />

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
            onRecord={() => {
              if (selection && modo !== "still") void grabarRegion(selection, modo);
            }}
            onCancel={() => void cancelCapture()}
          />
        )}
      </AnimatePresence>

      {error && (
        <div className="pointer-events-none absolute inset-x-0 top-8 flex justify-center">
          <div className="max-w-xl rounded-xl border border-red-500/30 bg-red-950/90 px-4 py-2.5 text-xs text-red-200 shadow-2xl backdrop-blur-md">
            {error}
          </div>
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
        onCancel={() => void cancelCapture()}
        dimmed={mode.kind !== "idle"}
      />
    </div>
  );
}
