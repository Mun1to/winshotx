import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AlertTriangle } from "lucide-react";
import { cancelCapture } from "../../lib/ipc";
import { useT } from "../../lib/i18n";

/** Segundos que el overlay puede estar sin fondo antes de quitarse de en medio. */
const RESCATE_MS = 8000;

/** Cierra el overlay por las dos vias: la limpia y, si falla, la de la propia ventana. */
async function salir() {
  try {
    await cancelCapture();
  } catch {
    await getCurrentWindow().close();
  }
}

/**
 * Lo que se ve mientras el fondo congelado todavia no esta, y lo que se ve si no
 * llega nunca. El overlay ocupa la pantalla entera y es opaco: dejarlo en negro
 * mudo equivale a secuestrar el escritorio, asi que aqui siempre hay un texto,
 * una salida a mano y un cierre automatico de rescate.
 */
export function BootScreen({ error }: { error: string | null }) {
  const t = useT();
  useEffect(() => {
    const rescate = window.setTimeout(() => void salir(), RESCATE_MS);
    return () => window.clearTimeout(rescate);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void salir();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center gap-3 bg-[#111113]">
      {error ? (
        <>
          <div className="flex max-w-xl items-start gap-2.5 rounded-xl border border-red-500/30 bg-red-950/60 px-4 py-3 text-sm text-red-200">
            <AlertTriangle className="mt-0.5 size-4 shrink-0" />
            <span>
              <span className="block font-medium">{t("No se ha podido preparar la captura")}</span>
              <span className="mt-1 block text-xs break-words text-red-300/90">{error}</span>
            </span>
          </div>
          <button
            type="button"
            onClick={() => void salir()}
            className="rounded-lg border border-white/15 px-4 py-1.5 text-xs text-neutral-300 transition-colors hover:bg-white/10 hover:text-white"
          >
            {t("Cerrar (Esc)")}
          </button>
        </>
      ) : (
        <>
          <span className="size-5 animate-spin rounded-full border-2 border-white/15 border-t-white/70" />
          <span className="text-xs text-neutral-400">{t("Preparando la captura… · Esc para salir")}</span>
        </>
      )}
    </div>
  );
}
