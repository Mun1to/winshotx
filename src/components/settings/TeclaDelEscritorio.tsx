import { partesDeAtajo } from "../../lib/teclas";
import { useT } from "../../lib/i18n";
import type { TeclaDelEscritorio } from "../../lib/types";

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
