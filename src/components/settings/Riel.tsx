import { useT } from "../../lib/i18n";
import { APARTADOS, type ApartadoId } from "./mapaDeAjustes";

/**
 * Lo que mide el riel de alto con sus ocho iconos: 16 de aire arriba y abajo, 44 por icono
 * y 4 entre uno y otro. La página nunca mide menos, o la ventana a la medida (que mide la
 * página) cortaría el último icono en los apartados cortos.
 */
export const ALTO_RIEL = 16 * 2 + APARTADOS.length * 44 + (APARTADOS.length - 1) * 4;

interface Props {
  activo: ApartadoId;
  onIr: (id: ApartadoId) => void;
}

/**
 * El riel de la izquierda: un icono por apartado, sin texto (la variación 9).
 *
 * El nombre va en el `title`, para quien deja el ratón encima, y en el `aria-label`, para
 * quien lo oye. El elegido lleva el icono en el azul de la marca y una raya de 2 px a la
 * izquierda, sin fondo: un recuadro quieto alrededor del icono sería una cárcel.
 *
 * Con flechas arriba y abajo se pasa de uno a otro, como en una lista.
 */
export function Riel({ activo, onIr }: Props) {
  const t = useT();
  const mover = (paso: number) => {
    const i = APARTADOS.findIndex((a) => a.id === activo);
    const siguiente = APARTADOS[(i + paso + APARTADOS.length) % APARTADOS.length];
    onIr(siguiente.id);
    // El foco sigue al apartado, para poder seguir con las flechas.
    requestAnimationFrame(() =>
      document.querySelector<HTMLElement>(`[data-riel] [data-vista="${siguiente.id}"]`)?.focus(),
    );
  };
  return (
    <nav
      aria-label={t("Apartados de los ajustes")}
      data-riel
      data-tour="riel"
      className="flex w-16 shrink-0 flex-col items-center gap-1 border-r border-linea py-4"
      onKeyDown={(e) => {
        if (e.key === "ArrowDown") {
          e.preventDefault();
          mover(1);
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          mover(-1);
        }
      }}
    >
      {APARTADOS.map((a) => {
        const elegido = a.id === activo;
        return (
          <button
            key={a.id}
            type="button"
            data-vista={a.id}
            title={t(a.nombre)}
            aria-label={t(a.nombre)}
            aria-current={elegido ? "page" : undefined}
            onClick={() => onIr(a.id)}
            className={`relative flex size-11 shrink-0 items-center justify-center rounded-md transition-colors hover:bg-realce ${
              elegido ? "text-marca" : "text-apagado hover:text-titulo"
            }`}
          >
            {elegido && <span aria-hidden="true" className="absolute inset-y-2 -left-[10px] w-[2px] rounded-full bg-marca" />}
            <a.icono className="size-5" strokeWidth={1.9} />
          </button>
        );
      })}
    </nav>
  );
}
