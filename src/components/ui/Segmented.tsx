interface Props<T extends string | number> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}

/** Selector de pocas opciones: más claro que un slider cuando los valores son fijos. */
export function Segmented<T extends string | number>({ value, options, onChange }: Props<T>) {
  return (
    <div className="flex gap-1 rounded-lg bg-black/40 p-1">
      {options.map((option) => (
        <button
          key={String(option.value)}
          type="button"
          onClick={() => onChange(option.value)}
          className={`flex-1 rounded-md px-3 py-1.5 text-xs font-medium whitespace-nowrap transition-colors ${
            option.value === value
              ? "bg-white/15 text-white shadow-sm"
              : "text-neutral-400 hover:text-white"
          }`}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
