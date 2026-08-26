import type { ComponentType } from "react";
import { Camera, Keyboard, Settings2, Video } from "lucide-react";

/** Las cuatro secciones de los ajustes, en el orden en que se usan. */
export const SECCIONES = [
  { id: "capturar", rotulo: "Capturar", icono: Camera, subtitulo: "El atajo, la espera y qué entra en la foto." },
  { id: "grabar", rotulo: "Grabar", icono: Video, subtitulo: "El atajo, la fluidez y qué pasa al terminar." },
  {
    id: "teclas",
    rotulo: "Teclas de Windows",
    icono: Keyboard,
    subtitulo: "Quedarse con las teclas de captura que ya trae el sistema.",
  },
  { id: "app", rotulo: "La app", icono: Settings2, subtitulo: "Dónde caen los archivos y cómo se comporta winshotx." },
] as const satisfies readonly {
  id: string;
  rotulo: string;
  icono: ComponentType<{ className?: string }>;
  subtitulo: string;
}[];

export type SeccionId = (typeof SECCIONES)[number]["id"];

interface Props {
  activa: SeccionId;
  onCambiar: (id: SeccionId) => void;
}

/**
 * El menú de secciones, a la izquierda.
 *
 * Antes los cuatro bloques vivían a la vez en dos columnas, y con veinte filas dentro ya
 * no cabían: la última quedaba cortada por abajo y había que buscar con la rueda. Una
 * sección cada vez es lo que quita esa saturación sin quitar ni un ajuste.
 *
 * Lo elegido se marca con un tinte del azul de la marca y el texto en negrita, no con una
 * pastilla azul maciza: en una ventana pequeña la maciza era lo más llamativo de la
 * pantalla, más que los propios ajustes. Copiado de VoCript, que ya resolvió esto.
 */
export function SettingsNav({ activa, onCambiar }: Props) {
  return (
    <nav
      aria-label="Secciones de ajustes"
      className="flex w-[188px] shrink-0 flex-col gap-0.5 overflow-y-auto border-r border-white/8 p-2"
    >
      {SECCIONES.map((seccion) => {
        const Icono = seccion.icono;
        const seleccionada = seccion.id === activa;
        return (
          <button
            key={seccion.id}
            type="button"
            onClick={() => onCambiar(seccion.id)}
            aria-current={seleccionada ? "page" : undefined}
            // Deja que una prueba llegue a una seccion sin depender de su nombre visible.
            data-seccion={seccion.id}
            className={`flex w-full items-center gap-2.5 rounded-lg px-2.5 py-[7px] text-left text-[13.5px] transition-colors ${
              seleccionada
                ? "bg-[#0a9bff]/20 font-semibold text-white"
                : "text-neutral-400 hover:bg-white/[0.06] hover:text-neutral-200"
            }`}
          >
            <Icono
              className={`size-[17px] shrink-0 ${seleccionada ? "text-[#0a9bff]" : "opacity-85"}`}
            />
            <span className="truncate">{seccion.rotulo}</span>
          </button>
        );
      })}
    </nav>
  );
}
