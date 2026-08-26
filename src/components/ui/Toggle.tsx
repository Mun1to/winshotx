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
      className="flex w-full items-center justify-between gap-4 rounded-lg py-1.5 text-left"
    >
      <span>
        <span className="block text-xs font-medium text-texto">{label}</span>
        {hint && <span className="block text-[11px] text-tenue">{hint}</span>}
      </span>
      <span
        className={`relative h-[22px] w-[40px] shrink-0 rounded-full transition-colors duration-150 ${
          checked ? "bg-blue-500" : "bg-apagador"
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
