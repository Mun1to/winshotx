import type { ReactNode } from "react";

interface Props<T extends string | number> {
  value: T;
  /**
   * `icono` dibuja el boton con esa figura y deja el texto como nombre accesible.
   *
   * Lo usa la barra de abajo, donde no caben tres palabras por opcion pero el lector de
   * pantalla y el globo del raton tienen que seguir diciendo cual es cual.
   */
  options: { value: T; label: string; icono?: ReactNode }[];
  onChange: (value: T) => void;
  /**
   * Cada botón tan ancho como su texto, en vez de todos iguales.
   *
   * Lo usa la cabecera de los ajustes: con cuatro secciones y "Teclas de Windows" entre
   * ellas, repartir a partes iguales hacía los cuatro tan anchos como el más largo y el
   * control se comía media barra.
   */
  ajustado?: boolean;
  /**
   * Cómo se llama el grupo entero.
   *
   * Un puñado de botones sueltos no dice qué se está eligiendo: quien lo oye con un lector
   * de pantalla escucha «15 fps, 30 fps, 60 fps» sin saber de qué. Y hay dos grupos de fps
   * en la misma pantalla, los de grabar y los del anillo, que solo se distinguen así.
   */
  etiqueta?: string;
  /** Mas apretado, para la barra de abajo. */
  compacto?: boolean;
}

/** Selector de pocas opciones: más claro que un slider cuando los valores son fijos. */
export function Segmented<T extends string | number>({
  value,
  options,
  onChange,
  ajustado = false,
  etiqueta,
  compacto = false,
}: Props<T>) {
  return (
    <div
      className={`flex gap-1 rounded-lg bg-hueco ${compacto ? "p-0.5" : "p-1"}`}
      role={etiqueta ? "group" : undefined}
      aria-label={etiqueta}
    >
      {options.map((option) => (
        <button
          key={String(option.value)}
          type="button"
          onClick={() => onChange(option.value)}
          // Cuál está elegido se veía solo por el color. Un lector de pantalla no ve
          // colores, y una prueba tampoco: esto lo dice en voz alta.
          aria-pressed={option.value === value}
          // Con icono, el texto no se ve pero sigue estando: es el nombre para quien lo
          // oye y el globo para quien deja el raton encima.
          aria-label={option.icono ? option.label : undefined}
          title={option.icono ? option.label : undefined}
          className={`flex items-center justify-center rounded-md font-medium whitespace-nowrap transition-colors ${
            compacto ? "px-2 py-1 text-[11px]" : "px-3 py-1.5 text-xs"
          } ${ajustado ? "" : "flex-1"} ${
            option.value === value
              ? "bg-pastilla text-titulo shadow-sm"
              : "text-apagado hover:text-titulo"
          }`}
        >
          {option.icono ?? option.label}
        </button>
      ))}
    </div>
  );
}
