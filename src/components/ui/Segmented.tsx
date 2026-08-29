interface Props<T extends string | number> {
  value: T;
  options: { value: T; label: string }[];
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
}

/** Selector de pocas opciones: más claro que un slider cuando los valores son fijos. */
export function Segmented<T extends string | number>({
  value,
  options,
  onChange,
  ajustado = false,
  etiqueta,
}: Props<T>) {
  return (
    <div
      className="flex gap-1 rounded-lg bg-hueco p-1"
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
          className={`rounded-md px-3 py-1.5 text-xs font-medium whitespace-nowrap transition-colors ${
            ajustado ? "" : "flex-1"
          } ${
            option.value === value
              ? "bg-pastilla text-titulo shadow-sm"
              : "text-apagado hover:text-titulo"
          }`}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
