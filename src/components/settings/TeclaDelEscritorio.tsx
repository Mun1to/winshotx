import { useEffect, useState } from "react";
import { Check, Copy } from "lucide-react";
import { copiarOrdenDelAtajo } from "../../lib/ipc";
import { partesDeAtajo } from "../../lib/teclas";
import { useT } from "../../lib/i18n";
import type { TeclaDelEscritorio } from "../../lib/types";

/**
 * La orden que hay que poner a mano en los atajos del escritorio, donde no se le pueden pedir
 * (Sway y los demás de wlroots no tienen el portal), con un botón para copiarla. Va en el
 * sitio de la tecla y con su misma altura. La copia la hace Rust, que monta la orden él.
 */
export function OrdenDelAtajo({ orden, accion }: { orden: string; accion: "capture" | "edit" }) {
  const t = useT();
  const [copiada, setCopiada] = useState(false);
  useEffect(() => {
    if (!copiada) return;
    const reloj = setTimeout(() => setCopiada(false), 2000);
    return () => clearTimeout(reloj);
  }, [copiada]);
  const texto = `${orden || "winshotx"} --${accion}`;
  const copiar = () =>
    void copiarOrdenDelAtajo(accion)
      .then(() => setCopiada(true))
      .catch(() => undefined);
  return (
    <span className="flex h-8 min-w-[132px] max-w-[260px] items-center gap-1 rounded-lg border border-linea-fuerte bg-hueco pr-1 pl-2">
      <code className="min-w-0 flex-1 truncate font-mono text-[11px] text-texto" title={texto}>
        {texto}
      </code>
      <button
        type="button"
        onClick={copiar}
        aria-label={copiada ? t("Copiada") : t("Copiar la orden")}
        title={copiada ? t("Copiada") : t("Copiar la orden")}
        className="grid size-6 shrink-0 place-items-center rounded-md text-apagado hover:bg-realce hover:text-texto"
      >
        {copiada ? <Check className="size-3.5 text-emerald-400" /> : <Copy className="size-3.5" />}
      </button>
    </span>
  );
}

/**
 * Cómo se ponen las dos teclas a mano en Sway, que es el escritorio más común sin el portal:
 * en su configuración, con la primera línea para que winshotx arranque con la sesión (Sway no
 * lee las entradas de arranque de los demás escritorios, y sin winshotx abierto la tecla solo
 * lo abriría).
 */
export function EjemploDeSway({ orden }: { orden: string }) {
  const t = useT();
  const app = orden || "winshotx";
  return (
    <div className="text-[11px] text-tenue">
      <p>{t("En Sway, por ejemplo, van en ~/.config/sway/config:")}</p>
      <pre className="mt-1.5 overflow-x-auto rounded-lg border border-linea bg-hueco px-3 py-2 font-mono text-[11px] leading-relaxed text-suave">
        {`exec ${app} --autostart\nbindsym Print exec ${app} --capture\nbindsym Shift+Print exec ${app} --edit`}
      </pre>
    </div>
  );
}

/**
 * La tecla que ha puesto el escritorio en Wayland, en el sitio del campo de los atajos y con
 * su misma cara, pero sin poder pulsarse: ahí la tecla no la decide winshotx, la elige la
 * persona en una ventana del escritorio (el botón «Cambiar» de al lado).
 *
 * Si el escritorio la escribe de una forma que no se sabe leer, sale su texto tal cual; y si
 * no le ha dado ninguna (se canceló su ventana), sale en rojo, como un atajo ocupado.
 */
export function TeclaPuesta({ tecla }: { tecla: TeclaDelEscritorio | null }) {
  const t = useT();
  const partes = tecla?.atajo ? partesDeAtajo(tecla.atajo) : null;
  return (
    <span
      className={`flex h-8 min-w-[132px] items-center justify-end gap-1 rounded-lg border px-2 ${
        tecla ? "border-linea-fuerte bg-hueco" : "border-red-500/40 bg-red-500/10"
      }`}
    >
      {partes ? (
        partes.map((parte, i) => (
          <kbd
            key={`${parte}-${i}`}
            className="rounded-[5px] border border-linea-fuerte bg-realce px-1.5 py-0.5 text-[11px] leading-none font-medium text-texto"
          >
            {parte}
          </kbd>
        ))
      ) : (
        <span className="w-full text-center text-[11px] text-texto">
          {tecla ? tecla.texto : t("Sin tecla")}
        </span>
      )}
    </span>
  );
}
