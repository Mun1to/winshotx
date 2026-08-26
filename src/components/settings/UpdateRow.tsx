import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Check, Download, RefreshCw, RotateCcw } from "lucide-react";
import { EVENTS } from "../../lib/types";
import { Row } from "./Section";

/** Cada cuánto se mira solo, mientras la ventana de ajustes siga abierta. */
const CADA_MS = 6 * 60 * 60 * 1000;

type Fase =
  | { tipo: "quieto" }
  | { tipo: "mirando" }
  | { tipo: "aldia" }
  | { tipo: "acabadeactualizarse" }
  | { tipo: "hay"; update: Update }
  | { tipo: "bajando"; version: string; pct: number }
  | { tipo: "listo"; version: string }
  | { tipo: "error"; mensaje: string };

/**
 * Actualización desde la propia app, como en Adeorq: se mira solo al abrir los
 * ajustes, pero no se instala nada hasta que Munir pulsa el botón.
 *
 * En una compilación de desarrollo no hay endpoint y `check()` falla; ahí se
 * queda callado, porque avisar de eso sería ruido en cada arranque.
 */
export function UpdateRow({
  version,
  recienActualizado = false,
}: {
  version: string;
  recienActualizado?: boolean;
}) {
  const [fase, setFase] = useState<Fase>(
    recienActualizado ? { tipo: "acabadeactualizarse" } : { tipo: "quieto" },
  );
  const mirando = useRef(false);

  const mirar = useCallback(async (aMano: boolean) => {
    if (mirando.current) return;
    mirando.current = true;
    if (aMano) setFase({ tipo: "mirando" });
    try {
      const update = await check();
      setFase(update ? { tipo: "hay", update } : { tipo: "aldia" });
    } catch (e) {
      // Sin conexión, o compilación sin endpoint: solo se dice si lo pidió él.
      setFase(aMano ? { tipo: "error", mensaje: String(e) } : { tipo: "quieto" });
    } finally {
      mirando.current = false;
    }
  }, []);

  useEffect(() => {
    // Si la ventana se ha abierto sola por una actualizacion, lo primero que hay que
    // decir es eso. Buscar version nueva en ese mismo instante no aporta nada y ademas
    // taparia la unica frase que explica por que ha aparecido la ventana.
    if (!recienActualizado) void mirar(false);
    const timer = window.setInterval(() => void mirar(false), CADA_MS);
    const unlisten = listen(EVENTS.checkUpdate, () => void mirar(true));
    // Y al volver a abrir la ventana: si no, una version publicada hace un rato no
    // aparece hasta que salte el temporizador de seis horas.
    const alVolver = listen(EVENTS.settingsShown, () => void mirar(false));
    return () => {
      window.clearInterval(timer);
      void unlisten.then((fn) => fn());
      void alVolver.then((fn) => fn());
    };
  }, [mirar, recienActualizado]);

  const instalar = (update: Update) => {
    setFase({ tipo: "bajando", version: update.version, pct: 0 });
    let total = 0;
    let hechos = 0;
    void update
      .downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? 0;
        else if (e.event === "Progress") {
          hechos += e.data.chunkLength;
          if (total > 0) {
            const pct = Math.min(99, Math.round((hechos / total) * 100));
            setFase({ tipo: "bajando", version: update.version, pct });
          }
        } else if (e.event === "Finished") {
          setFase({ tipo: "bajando", version: update.version, pct: 100 });
        }
      })
      .then(() => setFase({ tipo: "listo", version: update.version }))
      .catch((e) => setFase({ tipo: "error", mensaje: String(e) }));
  };

  const hint = () => {
    switch (fase.tipo) {
      case "mirando":
        return "mirando si hay versión nueva…";
      case "aldia":
        return "estás en la última versión";
      case "acabadeactualizarse":
        return `actualizado a la ${version}`;
      case "hay":
        return `la ${fase.update.version} ya está disponible`;
      case "bajando":
        return `descargando la ${fase.version}… ${fase.pct} %`;
      case "listo":
        return "instalada, solo falta reiniciar";
      case "error":
        return fase.mensaje;
      default:
        return `winshotx ${version}`;
    }
  };

  return (
    <Row
      icon={
        fase.tipo === "aldia" || fase.tipo === "acabadeactualizarse" ? (
          <Check className="size-4 text-emerald-400" />
        ) : (
          <Download className="size-4" />
        )
      }
      label="Actualizaciones"
      hint={hint()}
      tone={
        fase.tipo === "aldia" || fase.tipo === "acabadeactualizarse"
          ? "ok"
          : fase.tipo === "error"
            ? "error"
            : "normal"
      }
      control={<Boton fase={fase} instalar={instalar} mirar={() => void mirar(true)} />}
    />
  );
}

function Boton({
  fase,
  instalar,
  mirar,
}: {
  fase: Fase;
  instalar: (update: Update) => void;
  mirar: () => void;
}) {
  const base =
    "flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] transition-colors disabled:opacity-50";

  if (fase.tipo === "hay") {
    return (
      <button
        type="button"
        onClick={() => instalar(fase.update)}
        className={`${base} bg-blue-500 font-semibold text-white hover:bg-blue-400`}
      >
        <Download className="size-3.5" /> Actualizar ahora
      </button>
    );
  }

  if (fase.tipo === "bajando") {
    return (
      <span className="flex items-center gap-2">
        <span className="h-1 w-20 overflow-hidden rounded-full bg-white/10">
          <span
            className="block h-full bg-blue-500 transition-[width] duration-150"
            style={{ width: `${fase.pct}%` }}
          />
        </span>
        <span className="text-[11px] tabular-nums text-neutral-400">{fase.pct} %</span>
      </span>
    );
  }

  if (fase.tipo === "listo") {
    return (
      <button
        type="button"
        onClick={() => void relaunch()}
        className={`${base} bg-emerald-500/15 font-semibold text-emerald-300 hover:bg-emerald-500/25`}
      >
        <RotateCcw className="size-3.5" /> Reiniciar
      </button>
    );
  }

  return (
    <button
      type="button"
      onClick={mirar}
      disabled={fase.tipo === "mirando"}
      className={`${base} border border-white/10 text-neutral-300 hover:bg-white/10 hover:text-white`}
    >
      <RefreshCw className={`size-3.5 ${fase.tipo === "mirando" ? "animate-spin" : ""}`} />
      Buscar
    </button>
  );
}
