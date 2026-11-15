import type { ReactNode } from "react";
import { LogOut, Search } from "lucide-react";
import { useT } from "../../lib/i18n";
import { Marca } from "../ui/Marca";

interface Props {
  version: string;
  /** Abre la paleta del buscador. */
  onBuscar: () => void;
  /** El aviso de actualización: solo cuando hay algo que decir. */
  aviso: ReactNode;
  onSalir: () => void;
}

/**
 * La cabecera de los ajustes (S36, la variación 9): a la izquierda el logo, el nombre y la
 * versión; en medio el buscador; a la derecha el aviso de actualización, si lo hay, y Salir.
 *
 * El nombre vuelve a estar dentro: con el riel de iconos a la izquierda ya no hay pestañas
 * con nombre que digan dónde se está, y la versión es lo primero que se mira al contar un
 * fallo.
 *
 * El buscador parece un campo pero es un botón: al pulsarlo (o con Ctrl + F) se abre la
 * paleta encima, que es donde se escribe. Así la cabecera no cambia de alto ni de sitio
 * mientras se busca.
 */
export function Cabecera({ version, onBuscar, aviso, onSalir }: Props) {
  const t = useT();
  return (
    <header className="grid shrink-0 grid-cols-[1fr_minmax(0,400px)_1fr] items-center gap-6 border-b border-linea px-5 py-3">
      <div className="flex min-w-0 items-center gap-2.5 justify-self-start">
        <Marca className="size-6 shrink-0" x="currentColor" />
        <span className="text-[16px] font-semibold text-titulo">winshotx</span>
        <span className="text-[13px] text-apagado">{version}</span>
      </div>

      <button
        type="button"
        data-abrir-buscar
        data-tour="buscar"
        onClick={onBuscar}
        className="group flex w-full min-w-0 items-center gap-2.5 border-b-2 border-linea pb-1.5 pt-1.5 text-left transition-colors hover:border-linea-viva"
      >
        <Search className="size-4 shrink-0 text-apagado" strokeWidth={1.9} />
        <span className="min-w-0 flex-1 truncate text-[14.5px] text-apagado group-hover:text-suave">
          {t("Buscar un ajuste")}
        </span>
        <span className="shrink-0 text-[12px] text-tenue">Ctrl+F</span>
      </button>

      <div className="flex min-w-0 items-center gap-3 justify-self-end">
        {aviso}
        <button
          type="button"
          onClick={onSalir}
          className="flex shrink-0 items-center gap-2 rounded-md px-2 py-1.5 text-[13.5px] font-medium text-apagado transition-colors hover:bg-realce hover:text-titulo"
        >
          <LogOut className="size-4" strokeWidth={1.9} />
          {t("Salir")}
        </button>
      </div>
    </header>
  );
}
