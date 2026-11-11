import type { ReactNode } from "react";

/** Una fila de opciones en texto, con la elegida subrayada: nada de pastillas. */
export function Opciones({ children }: { children: ReactNode }) {
  return <div className="flex items-center gap-4">{children}</div>;
}

/**
 * Una opción en texto, con la elegida subrayada y en blanco. Es la misma en todo el editor
 * (la velocidad, el suavizado del puntero, la forma de la lupa, el tipo de fondo propio): si
 * cada pieza lleva su copia, acaban pareciéndose pero no siendo iguales.
 */
export function Opcion({ puesta, onClick, children }: { puesta: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={puesta}
      className={`border-b-2 py-1 text-[14px] font-semibold whitespace-nowrap tabular-nums transition-colors ${
        puesta ? "border-blue-500 text-white" : "border-transparent text-neutral-400 hover:text-neutral-100"
      }`}
    >
      {children}
    </button>
  );
}
