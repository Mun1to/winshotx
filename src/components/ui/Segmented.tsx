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
   * Cada botón tan ancho como su texto, en vez de todos iguales. Solo cambia la forma
   * `compacto`: en texto cada opción ya es tan ancha como su nombre.
   *
   * Lo usaba la cabecera de los ajustes: con cuatro secciones y "Teclas de Windows" entre
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
  /** En pastilla y más apretado, para la barra de abajo. */
  compacto?: boolean;
}

/**
 * Selector de pocas opciones: más claro que un slider cuando los valores son fijos.
 *
 * Desde la 0.5 son opciones en texto, con la elegida en negrita y una raya azul debajo, como
 * las pestañas del Historial y las opciones del editor: había tres formas de elegir entre
 * pocas cosas (esta en pastilla, y las otras dos en texto) y ahora las tres son texto con
 * su raya. En texto cada opción es tan ancha como su nombre, así que `ajustado` ya no
 * cambia nada. (`Opcion`, la del editor, sigue con el azul fijo del editor, que no tiene
 * tema claro: pasarla a esta pieza es del rediseño del editor.)
 *
 * La forma `compacto` se queda en pastilla: es la de la barra de abajo, con iconos y sin
 * sitio para una raya debajo.
 */
export function Segmented<T extends string | number>({
  value,
  options,
  onChange,
  ajustado = false,
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
  return (
    <div
      className="flex gap-1 rounded-lg bg-hueco p-0.5"
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
          className={`flex items-center justify-center rounded-md px-2 py-1 text-[11px] font-medium whitespace-nowrap transition-colors ${
            ajustado ? "" : "flex-1"
          } ${
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
