import { Check } from "lucide-react";
import { Marca } from "../ui/Marca";

/** Las cuatro secciones de los ajustes, en el orden en que se usan. */
export const SECCIONES = [
  { id: "capturar", rotulo: "Capturar", subtitulo: "El atajo, la espera y qué entra en la foto." },
  { id: "grabar", rotulo: "Grabar", subtitulo: "El atajo, la fluidez y qué pasa al terminar." },
  {
    id: "teclas",
    rotulo: "Teclas de Windows",
    subtitulo: "Quedarse con las teclas de captura que ya trae el sistema.",
  },
  { id: "app", rotulo: "La app", subtitulo: "Dónde caen los archivos y cómo se comporta winshotx." },
] as const satisfies readonly { id: string; rotulo: string; subtitulo: string }[];

export type SeccionId = (typeof SECCIONES)[number]["id"];

interface Props {
  activa: SeccionId;
  onCambiar: (id: SeccionId) => void;
  version: string;
  guardado: boolean;
  onSalir: () => void;
}

/**
 * La cabecera: marca a la izquierda, las cuatro secciones en el medio y salir a la derecha.
 *
 * Es una cabecera de pagina web y no un menu lateral por una razon de espacio, no de gusto:
 * una columna a la izquierda se lleva casi doscientos pixeles de ANCHO, que es justo lo que
 * hace falta para poner los ajustes en dos columnas y que una seccion entera quepa de una
 * vez. Arriba, la navegacion cuesta alto una sola vez y el ancho queda entero para lo que se
 * ha venido a ver. Aqui vive tambien el pie que habia antes: dos barras de adorno en una
 * ventana de 640 px de alto eran una de mas.
 *
 * Lo elegido se marca con una linea debajo, como los enlaces de una web, en vez de con una
 * pastilla: en horizontal la pastilla pesaba mas que el propio nombre de la seccion.
 */
export function SettingsHeader({ activa, onCambiar, version, guardado, onSalir }: Props) {
  return (
    <header className="flex h-12 shrink-0 items-center gap-5 border-b border-white/8 px-4">
      <span className="flex shrink-0 items-center gap-2">
        <Marca className="size-[18px]" />
        <span className="text-[13.5px] font-semibold text-neutral-100">winshotx</span>
        <span className="text-[11px] text-neutral-500">{version}</span>
      </span>

      <nav aria-label="Secciones de ajustes" className="flex min-w-0 flex-1 items-center gap-1">
        {SECCIONES.map((seccion) => {
          const seleccionada = seccion.id === activa;
          return (
            <button
              key={seccion.id}
              type="button"
              onClick={() => onCambiar(seccion.id)}
              aria-current={seleccionada ? "page" : undefined}
              // Deja que una prueba llegue a una seccion sin depender de su nombre visible.
              data-seccion={seccion.id}
              className={`relative h-12 shrink-0 px-3 text-[13px] whitespace-nowrap transition-colors ${
                seleccionada
                  ? "font-semibold text-white"
                  : "text-neutral-400 hover:text-neutral-100"
              }`}
            >
              {seccion.rotulo}
              {seleccionada && (
                <span className="absolute inset-x-2 -bottom-px h-[2px] rounded-full bg-[#0a9bff]" />
              )}
            </button>
          );
        })}
      </nav>

      <span className="flex shrink-0 items-center gap-3">
        {/* Ocupa sitio siempre, tenga texto o no: sin esto, guardar movia el boton de
            salir cada vez que se tocaba un interruptor. */}
        <span className="flex w-[74px] items-center justify-end gap-1.5 text-[11px] text-emerald-400">
          {guardado && (
            <>
              <Check className="size-3" />
              Guardado
            </>
          )}
        </span>
        <button
          type="button"
          onClick={onSalir}
          className="rounded-md px-2 py-1 text-[11.5px] text-neutral-400 transition-colors hover:bg-red-500/15 hover:text-red-300"
        >
          Salir
        </button>
      </span>
    </header>
  );
}
