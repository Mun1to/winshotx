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
}

/** Selector de pocas opciones: más claro que un slider cuando los valores son fijos. */
export function Segmented<T extends string | number>({
  value,
  options,
  onChange,
  ajustado = false,
}: Props<T>) {
  return (
    <div className="flex gap-1 rounded-lg bg-hueco p-1">
      {options.map((option) => (
        <button
          key={String(option.value)}
          type="button"
          onClick={() => onChange(option.value)}
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
