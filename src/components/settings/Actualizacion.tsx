import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Download, RefreshCw, RotateCcw } from "lucide-react";
import { isStoreBuild } from "../../lib/ipc";
import { EVENTS } from "../../lib/types";
import { useT } from "../../lib/i18n";

/** Cada cuánto se mira solo, mientras la ventana de ajustes siga abierta. */
const CADA_MS = 6 * 60 * 60 * 1000;

export type Fase =
  | { tipo: "quieto" }
  /** Instalada desde la Microsoft Store: de actualizar se encarga ella. */
  | { tipo: "tienda" }
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
 *
 * Va como gancho y no como fila porque se pinta en dos sitios: el aviso de la cabecera, que
 * está siempre a la vista, y la fila de la versión en «Ayuda y winshotx»; lo que hay que
 * compartir es el estado, no una manera de pintarlo.
 */
export function useActualizacion(version: string, recienActualizado = false) {
  const t = useT();
  const [fase, setFase] = useState<Fase>(
    recienActualizado ? { tipo: "acabadeactualizarse" } : { tipo: "quieto" },
  );
  const mirando = useRef(false);

  /** `null` mientras no se ha preguntado; luego ya no vuelve a cambiar. */
  const tienda = useRef<boolean | null>(null);

  const mirar = useCallback(async (aMano: boolean) => {
    if (mirando.current) return;
    mirando.current = true;
    if (aMano) setFase({ tipo: "mirando" });
    try {
      // Se pregunta aqui dentro y no en un efecto aparte a proposito: preguntarlo fuera
      // deja una ventana de unos milisegundos en la que este `mirar` ya ha salido
      // corriendo sin saber la respuesta, y en la Store acabaria ofreciendo un boton de
      // actualizar que no puede escribir en su propia carpeta.
      if (tienda.current === null) tienda.current = await isStoreBuild();
      if (tienda.current) {
        setFase({ tipo: "tienda" });
        return;
      }
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
    else void isStoreBuild().then((esTienda) => (tienda.current = esTienda));
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

  const instalar = useCallback((update: Update) => {
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
  }, []);

  const texto = () => {
    switch (fase.tipo) {
      case "mirando":
        return t("mirando si hay versión nueva…");
      case "tienda":
        return t("la Microsoft Store se encarga de actualizarla");
      case "aldia":
        return t("estás en la última versión");
      case "acabadeactualizarse":
        return t("actualizado a la {v}", { v: version });
      case "hay":
        return t("la {v} ya está disponible", { v: fase.update.version });
      case "bajando":
        return t("descargando la {v}… {pct} %", { v: fase.version, pct: fase.pct });
      case "listo":
        return t("instalada, solo falta reiniciar");
      case "error":
        return fase.mensaje;
      default:
        return "";
    }
  };

  const alDia = fase.tipo === "aldia" || fase.tipo === "acabadeactualizarse";

  return { fase, mirar, instalar, texto: texto(), alDia };
}

/**
 * Lo de actualizar en la cabecera de los ajustes, que casi siempre no tiene nada que decir.
 *
 * **Estando al día no sale nada** (S36, 0.5): «Al día» con su tick era un «todo va bien» que
 * no pide nada, y los gustos de Munito lo quitan de todas partes. Buscar a mano se hace desde
 * la fila de la versión, en «Ayuda y winshotx», y desde el menú de la bandeja.
 *
 * Cuando SÍ hay algo que decir (hay versión nueva, se está bajando, falta reiniciar, ha
 * fallado o se acaba de actualizar), sale la frase con su botón: eso es justo lo que no puede
 * pasar desapercibido, y la cabecera es lo primero que se ve.
 */
export function AvisoActualizacion({
  fase,
  texto,
  instalar,
  mirar,
}: {
  fase: Fase;
  texto: string;
  instalar: (update: Update) => void;
  mirar: () => void;
}) {
  const t = useT();
  if (fase.tipo === "tienda" || fase.tipo === "quieto" || fase.tipo === "mirando" || fase.tipo === "aldia") {
    return null;
  }
  // El fallo, en corto y con el detalle en el globo: el mensaje de error entero va en la fila
  // de la versión, donde cabe, y aquí empujaría al buscador fuera de la cabecera.
  const corto = fase.tipo === "error" ? t("No se pudo mirar si hay versión nueva") : texto;
  return (
    <span data-aviso-actualizacion className="flex items-center gap-3">
      <span
        title={fase.tipo === "error" ? texto : undefined}
        className={`whitespace-nowrap text-[13px] ${
          fase.tipo === "error"
            ? "text-[color:light-dark(#b91c1c,#f87171)]"
            : fase.tipo === "hay" || fase.tipo === "listo"
              ? "font-semibold text-titulo"
              : "text-apagado"
        }`}
      >
        {corto}
      </span>
      {/* Recién actualizada no hay nada que pulsar: la frase dice por qué se ha abierto sola. */}
      {fase.tipo !== "acabadeactualizarse" && <BotonActualizar fase={fase} instalar={instalar} mirar={mirar} />}
    </span>
  );
}

/**
 * El botón, que cambia con la fase: buscar, actualizar, la barra de descarga y reiniciar.
 *
 * Actualizar y reiniciar van rellenos del azul de la marca, que son la acción que se espera;
 * buscar va como los demás botones de las filas.
 */
export function BotonActualizar({
  fase,
  instalar,
  mirar,
}: {
  fase: Fase;
  instalar: (update: Update) => void;
  mirar: () => void;
}) {
  const t = useT();
  const base =
    "flex items-center gap-1.5 whitespace-nowrap rounded-lg border px-3 py-1.5 text-[13px] font-semibold transition-colors disabled:opacity-50";
  const lleno = `${base} border-transparent bg-marca text-white hover:brightness-110`;

  if (fase.tipo === "tienda") return null;

  if (fase.tipo === "hay") {
    return (
      <button type="button" onClick={() => instalar(fase.update)} className={lleno}>
        <Download className="size-4" /> {t("Actualizar ahora")}
      </button>
    );
  }

  if (fase.tipo === "bajando") {
    return (
      <span className="flex items-center gap-2">
        <span className="h-1.5 w-24 overflow-hidden rounded-full bg-realce">
          <span
            className="block h-full bg-marca transition-[width] duration-150"
            style={{ width: `${fase.pct}%` }}
          />
        </span>
        <span className="text-[13px] tabular-nums text-apagado">{fase.pct} %</span>
      </span>
    );
  }

  if (fase.tipo === "listo") {
    return (
      <button type="button" onClick={() => void relaunch()} className={lleno}>
        <RotateCcw className="size-4" /> {t("Reiniciar")}
      </button>
    );
  }

  return (
    <button
      type="button"
      onClick={mirar}
      disabled={fase.tipo === "mirando"}
      className={`${base} border-linea bg-realce text-titulo hover:bg-activo`}
    >
      <RefreshCw className={`size-4 ${fase.tipo === "mirando" ? "animate-spin" : ""}`} />
      {t("Buscar actualizaciones")}
    </button>
  );
}
