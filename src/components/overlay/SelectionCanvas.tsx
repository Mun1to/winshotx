import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  cancelCapture,
  captureStill,
  freezeBytes,
  overlayBootstrap,
  startRecording,
} from "../../lib/ipc";
import { clamp } from "../../lib/format";
import type { OverlayPayload, Rect, StillAction } from "../../lib/types";
import { BootScreen } from "./BootScreen";
import { DimensionBadge } from "./DimensionBadge";
import { FloatingToolbar } from "./FloatingToolbar";
import { Magnifier } from "./Magnifier";
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
    // Via rapida: el protocolo asset sirve el PNG sin copiarlo por el IPC.
    const response = await fetch(convertFileSrc(path));
    if (!response.ok) throw new Error(`asset devolvio ${response.status}`);
    const blob = await response.blob();
    if (blob.size === 0) throw new Error("el asset ha llegado vacio");
    return blob;
  } catch (assetError) {
    // Via de respaldo: los bytes por el IPC. Mas lenta, pero no depende ni de la
    // CSP ni del ambito del protocolo asset.
    console.warn("el protocolo asset ha fallado, se tira del IPC", assetError);
    const bytes = await freezeBytes(monitorId);
    return new Blob([bytes], { type: "image/png" });
  }
}

export function SelectionCanvas({ monitorId }: { monitorId: number }) {
  const [payload, setPayload] = useState<OverlayPayload | null>(null);
  const [selection, setSelection] = useState<Rect | null>(null);
  const [mode, setMode] = useState<Mode>({ kind: "idle" });
  const [cursor, setCursor] = useState({ x: 0, y: 0 });
  const [hex, setHex] = useState("#000000");
  const [audio, setAudio] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);
  const [source, setSource] = useState<HTMLCanvasElement | null>(null);
  const selectionRef = useRef<Rect | null>(null);
  selectionRef.current = selection;

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
    (x: number, y: number): Rect | null => {
      const inside = snapTargets.filter((w) => contains(w.rect, x, y));
      if (inside.length === 0) return null;
      return inside.reduce((best, w) =>
        w.rect.width * w.rect.height < best.rect.width * best.rect.height ? w : best,
      ).rect;
    },
    [snapTargets],
  );

  const hovered = useMemo(() => {
    if (selection || mode.kind !== "idle") return null;
    return windowAt(cursor.x, cursor.y);
  }, [windowAt, cursor, selection, mode.kind]);

  useEffect(() => {
    let cancelled = false;
    let objectUrl: string | null = null;

    const boot = async () => {
      const data = await overlayBootstrap(monitorId);
      if (cancelled) return;
      setPayload(data);
      setAudio(data.settings.recordAudio);
      void getCurrentWindow().setFocus();

      // El PNG se pasa a un blob del mismo origen: cargado directamente desde el
      // protocolo asset, el canvas quedaria contaminado y la lupa no podria leer
      // ni un pixel.
      const blob = await loadFreeze(data.freezePath, monitorId);
      if (cancelled) return;
      objectUrl = URL.createObjectURL(blob);
      setFreezeUrl(objectUrl);

      const bitmap = await createImageBitmap(blob);
      if (cancelled) return;
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
      setSource(canvas);
    };

    void boot().catch((e) => {
      if (!cancelled) setBootError(String(e));
    });
    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [monitorId]);

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

  const runStill = useCallback(
    async (action: StillAction) => {
      if (!selection || !payload || busy) return;
      setBusy(true);
      setError(null);
      try {
        await captureStill(toPhysical(selection), action);
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [selection, payload, busy, toPhysical],
  );

  const runRecord = useCallback(
    async (format: "gif" | "video") => {
      if (!selection || !payload || busy) return;
      setBusy(true);
      setError(null);
      try {
        await startRecording(toPhysical(selection), {
          format,
          fps: payload.settings.fps,
          captureCursor: payload.settings.captureCursor,
          audio: audio && format === "video",
        });
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [selection, payload, busy, audio, toPhysical],
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
          void runStill("copy");
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
        setSelection({ x: 0, y: 0, width: window.innerWidth, height: window.innerHeight });
      } else if (key === "e") {
        void runStill("edit");
      } else if (key === "g") {
        void runRecord("gif");
      } else if (key === "v") {
        void runRecord("video");
      } else if (key === "m") {
        setAudio((a) => !a);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [runStill, runRecord, nudge]);

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
        if (!drawn || drawn.width < MIN_DRAG || drawn.height < MIN_DRAG) {
          // Clic seco: si hay una ventana debajo, se selecciona entera (estilo ShareX).
          setSelection(mode.candidate);
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
  }, [mode, readHex]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (busy) return;
    const x = e.clientX;
    const y = e.clientY;
    const current = selectionRef.current;
    if (current && contains(current, x, y)) {
      setMode({ kind: "moving", grabX: x - current.x, grabY: y - current.y, base: current });
      return;
    }
    setMode({ kind: "drawing", originX: x, originY: y, candidate: windowAt(x, y) });
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
    payload.settings.showMagnifier && (!active || mode.kind === "drawing");

  return (
    <div
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      className="relative h-screen w-screen overflow-hidden"
      style={{ cursor: active ? "default" : "crosshair" }}
    >
      <img
        src={freezeUrl}
        alt=""
        draggable={false}
        className="pointer-events-none absolute inset-0 h-full w-full"
      />

      {/* Sin seleccion: velo uniforme. Con seleccion: el velo lo dibuja la sombra del recuadro. */}
      {!active && <div className="pointer-events-none absolute inset-0 bg-black/45" />}

      {highlight && (
        <div
          style={{
            left: highlight.x,
            top: highlight.y,
            width: highlight.width,
            height: highlight.height,
          }}
          className="pointer-events-none absolute rounded-[2px] border-2 border-blue-500/80 bg-blue-500/5"
        />
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
        {active && mode.kind === "idle" && (
          <FloatingToolbar
            key="toolbar"
            left={clamp(active.x + active.width / 2, 190, window.innerWidth - 190)}
            top={toolbarFlip ? active.y - 10 : active.y + active.height + 10}
            flipped={toolbarFlip}
            audio={audio}
            busy={busy}
            onCopy={() => void runStill("copy")}
            onSave={() => void runStill("save")}
            onEdit={() => void runStill("edit")}
            onRecordGif={() => void runRecord("gif")}
            onRecordVideo={() => void runRecord("video")}
            onToggleAudio={() => setAudio((a) => !a)}
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

      {!active && (
        <div className="pointer-events-none absolute inset-x-0 bottom-10 flex justify-center">
          <div className="rounded-full border border-white/10 bg-neutral-900/85 px-4 py-2 text-xs text-neutral-300 shadow-xl backdrop-blur-md">
            Arrastra para seleccionar · clic sobre una ventana para capturarla entera · Esc para
            salir
          </div>
        </div>
      )}
    </div>
  );
}
