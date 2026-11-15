import type { ReactNode } from "react";

/** Una fila de opciones en texto, con la elegida subrayada: nada de pastillas. */
export function Opciones({ children }: { children: ReactNode }) {
  return <div className="flex items-center gap-4">{children}</div>;
}

/**
 * Una opción en texto, con la elegida subrayada: la MISMA forma que `Segmented` (14 px
 * seminegrita, la elegida con su raya de 3 px en el azul de la marca y en tinta entera, las
 * demás en `apagado`). Desde la 0.5 sigue al tema, como el resto del editor.
 *
 * Se queda como pieza aparte solo para lo que `Segmented` no hace: una opción con su dibujo
 * dentro (la forma de la cámara o de la lupa) o que hace algo más al pulsarla (traer un
 * sonido propio). Para elegir entre unos nombres, `Segmented`.
 */
export function Opcion({
  puesta,
  onClick,
  children,
  etiqueta,
}: {
  puesta: boolean;
  onClick: () => void;
  children: ReactNode;
  /** El nombre entero, si lo que se ve es más corto (o lleva un dibujo delante). */
  etiqueta?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={puesta}
      aria-label={etiqueta}
      className={`border-b-[3px] py-1 text-[14px] font-semibold whitespace-nowrap tabular-nums transition-colors ${
        puesta ? "border-marca text-titulo" : "border-transparent text-apagado hover:text-titulo"
      }`}
    >
      {children}
    </button>
  );
}
