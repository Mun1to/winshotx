import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown } from "lucide-react";

interface Props<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  /** Cómo se llama el grupo entero, para el lector de pantalla. */
  etiqueta?: string;
}

/**
 * Un desplegable, para cuando las opciones no caben en una fila.
 *
 * El idioma es el caso que lo pide: con dieciocho idiomas, un grupo de botones como el de
 * la barra de abajo no cabe ni de ancho ni de legible. Aquí se ve el que hay elegido y el
 * resto se abre debajo, que es lo que cualquiera espera de un desplegable.
 *
 * Se cierra con Escape, con un clic fuera y al elegir, y el botón que lo abre recuerda
 * dónde estaba para que volver a él con el teclado devuelva el foco.
 */
export function Desplegable<T extends string>({
  value,
  options,
  onChange,
  etiqueta,
}: Props<T>) {
  const [abierto, setAbierto] = useState(false);
  const contenedor = useRef<HTMLDivElement>(null);
  const boton = useRef<HTMLButtonElement>(null);
  const elegido = options.find((o) => o.value === value);

  // Un clic fuera cierra, y con Escape también. Sin esto el desplegable se queda
  // abierto encima de la ventana y tapa lo que hay detrás.
  useEffect(() => {
    if (!abierto) return;
    const fuera = (e: MouseEvent) => {
      if (!contenedor.current?.contains(e.target as Node)) setAbierto(false);
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setAbierto(false);
        boton.current?.focus();
      }
    };
    document.addEventListener("mousedown", fuera);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("mousedown", fuera);
      document.removeEventListener("keydown", escape);
    };
  }, [abierto]);

  return (
    <div ref={contenedor} className="relative">
      <button
        ref={boton}
        type="button"
        onClick={() => setAbierto((v) => !v)}
        aria-haspopup="listbox"
        aria-expanded={abierto}
        aria-label={etiqueta}
        className="flex items-center gap-1 rounded-lg bg-hueco px-2 py-1 text-[11px] text-apagado transition-colors hover:text-titulo"
      >
        <span className="max-w-24 truncate">{elegido?.label ?? value}</span>
        <ChevronDown className="size-3" />
      </button>
      {abierto && (
        <ul
          role="listbox"
          aria-label={etiqueta}
          className="absolute bottom-full z-50 mb-1 max-h-72 min-w-40 overflow-y-auto rounded-lg border border-linea bg-panel p-1 shadow-lg"
        >
          {options.map((option) => (
            <li key={option.value}>
              <button
                type="button"
                role="option"
                aria-selected={option.value === value}
                onClick={() => {
                  onChange(option.value);
                  setAbierto(false);
                }}
                className="flex w-full items-center justify-between gap-2 rounded-md px-2 py-1 text-left text-[11px] text-apagado transition-colors hover:bg-realce hover:text-titulo"
              >
                <span className="truncate">{option.label}</span>
                {option.value === value && <Check className="size-3 shrink-0" />}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
