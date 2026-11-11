import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Check, Loader2, X } from "lucide-react";
import { scrollDescartar, scrollTerminar } from "../../lib/ipc";
import { useT } from "../../lib/i18n";
import { EVENTO_SCROLL_PROGRESO, type EstadoScroll, type ProgresoScroll } from "../../lib/scroll";

/** Cuánto se queda armada la X esperando el segundo clic, como en la barra de grabar. */
export const DESCARTAR_ARMADO_MS = 4_000;

/** Lo que dice la barra en cada momento. Frases cortas: la barra mide 420 px. */
const QUE_PASA: Record<EstadoScroll, string> = {
  bajando: "Bajando sola",
  tu: "Desplázala tú",
  perdida: "Sube un poco, me he perdido",
  "no-baja": "No baja sola: desplázala tú",
  cosiendo: "Cosiendo…",
};

/**
 * La barrita que acompaña a la captura con scroll, debajo de la zona.
 *
 * Dice qué está pasando y cuánto mide ya la imagen, y tiene las dos salidas: «Terminar»,
 * que se queda con lo que lleve y lo abre en el editor, y la X, que lo tira. Esc hace lo
 * mismo que «Terminar» desde cualquier sitio: la ventana no coge el foco nunca (lo tiene
 * la página que baja), así que la tecla la escucha Rust.
 *
 * **La X pide dos toques**, igual que la de grabar: una captura larga no se pierde por un
 * clic despistado.
 */
export function BarraScroll() {
  const t = useT();
  const [progreso, setProgreso] = useState<ProgresoScroll>({ alto: 0, estado: "bajando" });
  const [terminando, setTerminando] = useState(false);
  const [armado, setArmado] = useState(false);
  const desarmar = useRef<number | null>(null);

  useEffect(() => {
    // Con su etiqueta: sin `target`, `listen` recibe también lo que va a otras ventanas.
    const unlisten = listen<ProgresoScroll>(
      EVENTO_SCROLL_PROGRESO,
      (e) => {
        setProgreso(e.payload);
        if (e.payload.estado === "cosiendo") setTerminando(true);
      },
      { target: getCurrentWindow().label },
    );
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(
    () => () => {
      if (desarmar.current !== null) window.clearTimeout(desarmar.current);
    },
    [],
  );

  const terminar = useCallback(() => {
    if (terminando) return;
    setTerminando(true);
    void scrollTerminar().catch(() => setTerminando(false));
  }, [terminando]);

  const descartar = useCallback(() => {
    if (terminando) return;
    if (armado) {
      if (desarmar.current !== null) window.clearTimeout(desarmar.current);
      void scrollDescartar();
      return;
    }
    setArmado(true);
    desarmar.current = window.setTimeout(() => setArmado(false), DESCARTAR_ARMADO_MS);
  }, [terminando, armado]);

  const estado: EstadoScroll = terminando ? "cosiendo" : progreso.estado;
  const aviso = estado === "perdida" || estado === "no-baja";

  return (
    <div
      data-tauri-drag-region
      className="flex h-full w-full items-center gap-2 overflow-hidden bg-[#161618] pr-2 pl-4 whitespace-nowrap"
    >
      {terminando && (
        <Loader2 className="size-4 shrink-0 animate-spin text-neutral-300" aria-hidden="true" />
      )}
      <span
        data-tauri-drag-region
        role="status"
        className={`truncate text-[14px] leading-none font-semibold ${
          aviso ? "text-amber-300" : "text-white"
        }`}
      >
        {t(QUE_PASA[estado])}
      </span>
      {progreso.alto > 0 && (
        <span
          data-tauri-drag-region
          title={t("Lo que mide ya la imagen")}
          className="text-[13px] leading-none text-neutral-300 tabular-nums"
        >
          {new Intl.NumberFormat().format(progreso.alto)} px
        </span>
      )}

      <span className="ml-auto flex items-center gap-1">
        <button
          type="button"
          onClick={terminar}
          disabled={terminando}
          title={`${t("Terminar y abrir en el editor")} · Esc`}
          aria-label={t("Terminar")}
          className="flex h-9 shrink-0 items-center gap-1.5 rounded-lg bg-blue-500 px-3 text-sm font-medium text-white transition-colors duration-100 hover:bg-blue-400 disabled:pointer-events-none disabled:opacity-40"
        >
          <Check className="size-[18px] shrink-0" strokeWidth={2} />
          {t("Terminar")}
        </button>
        <button
          type="button"
          onClick={descartar}
          disabled={terminando}
          aria-label={armado ? t("¿Descartar?") : t("Descartar")}
          title={armado ? t("Otra vez para tirar la captura") : t("Descartar")}
          className={`flex h-9 shrink-0 items-center gap-1.5 rounded-lg px-2.5 text-sm font-medium transition-colors duration-100 disabled:pointer-events-none disabled:opacity-40 ${
            armado
              ? "bg-red-500 text-white hover:bg-red-400"
              : "text-red-400 hover:bg-red-500/15 hover:text-red-300"
          }`}
        >
          <X className="size-[18px] shrink-0" strokeWidth={2} />
          {armado && <span className="pr-0.5">{t("¿Descartar?")}</span>}
        </button>
      </span>
    </div>
  );
}
