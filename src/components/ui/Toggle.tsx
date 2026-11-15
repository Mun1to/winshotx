interface Props {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  hint?: string;
}

export function Toggle({ checked, onChange, label, hint }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className="flex w-full items-center justify-between gap-4 rounded-lg py-1 text-left"
    >
      {/* El nombre a 15 px y lo que explica a 13, como las filas de los ajustes de la 0.5. */}
      <span>
        <span className="block text-[15px] leading-snug font-medium text-titulo">{label}</span>
        {hint && <span className="mt-0.5 block text-[13px] leading-snug text-suave">{hint}</span>}
      </span>
      <span
        className={`relative h-[22px] w-[40px] shrink-0 rounded-full transition-colors duration-150 ${
          checked ? "bg-marca" : "bg-apagador"
        }`}
      >
        <span
          className={`absolute top-[2px] left-0 size-[18px] rounded-full bg-white shadow-sm transition-transform duration-150 ${
            checked ? "translate-x-[20px]" : "translate-x-[2px]"
          }`}
        />
      </span>
    </button>
  );
}
