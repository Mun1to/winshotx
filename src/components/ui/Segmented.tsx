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
   * Cómo se llama el grupo entero.
   *
   * Un puñado de botones sueltos no dice qué se está eligiendo: quien lo oye con un lector
   * de pantalla escucha «15 fps, 30 fps, 60 fps» sin saber de qué. Y hay dos grupos de fps
   * en la misma pantalla, los de grabar y los del anillo, que solo se distinguen así.
   */
  etiqueta?: string;
  /** Más apretado y a 13 px, para la barra de abajo. También en texto con su raya. */
  compacto?: boolean;
}

/**
 * Selector de pocas opciones: más claro que un slider cuando los valores son fijos.
 *
 * Desde la 0.5 son opciones en texto, con la elegida en negrita y una raya azul debajo, como
 * las pestañas del Historial y las opciones del editor: había tres formas de elegir entre
 * pocas cosas (esta en pastilla, y las otras dos en texto) y ahora las tres son texto con
 * su raya, cada opción tan ancha como su nombre. (`Opcion`, la del editor, sigue con el azul fijo del editor, que no tiene
 * tema claro: pasarla a esta pieza es del rediseño del editor.)
 *
 * La forma `compacto` es la de la barra de abajo: la misma raya, más apretada y a 13 px (desde
 * S40; antes era una pastilla a 11).
 */
export function Segmented<T extends string | number>({
  value,
  options,
  onChange,
  etiqueta,
  compacto = false,
}: Props<T>) {
  if (!compacto) {
    return (
      <div
        className="flex items-center gap-4"
        role={etiqueta ? "group" : undefined}
        aria-label={etiqueta}
      >
        {options.map((option) => {
          const elegida = option.value === value;
          return (
            <button
              key={String(option.value)}
              type="button"
              onClick={() => onChange(option.value)}
              aria-pressed={elegida}
              aria-label={option.icono ? option.label : undefined}
              title={option.icono ? option.label : undefined}
              className={`border-b-[3px] py-1 text-[14px] font-semibold whitespace-nowrap tabular-nums transition-colors ${
                elegida ? "border-marca text-titulo" : "border-transparent text-apagado hover:text-titulo"
              }`}
            >
              {option.icono ?? option.label}
            </button>
          );
        })}
      </div>
    );
  }
  // La forma de la barra de abajo (S40): también en texto con su raya, más apretada y a 13 px.
  // Hasta S36 era una pastilla a 11 px, que es una caja y una letra por debajo de la casa.
  return (
    <div
      className="flex items-center gap-3"
      role={etiqueta ? "group" : undefined}
      aria-label={etiqueta}
    >
      {options.map((option) => {
        const elegida = option.value === value;
        return (
          <button
            key={String(option.value)}
            type="button"
            onClick={() => onChange(option.value)}
            // Cuál está elegido se veía solo por el color. Un lector de pantalla no ve
            // colores, y una prueba tampoco: esto lo dice en voz alta.
            aria-pressed={elegida}
            aria-label={option.icono ? option.label : undefined}
            title={option.icono ? option.label : undefined}
            className={`border-b-2 py-0.5 text-[13px] font-semibold whitespace-nowrap transition-colors ${
              elegida ? "border-marca text-titulo" : "border-transparent text-apagado hover:text-titulo"
            }`}
          >
            {option.icono ?? option.label}
          </button>
        );
      })}
    </div>
  );
}
