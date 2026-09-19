interface Props {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}

/** Interruptor suelto, para usar dentro de una fila que ya tiene su texto. */
export function Switch({ checked, onChange, label, disabled = false }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative block h-[22px] w-[40px] shrink-0 rounded-full transition-colors duration-150 disabled:opacity-40 ${
        checked ? "bg-blue-500" : "bg-apagador"
      }`}
    >
      <span
        className={`absolute top-[2px] left-0 size-[18px] rounded-full bg-white shadow-sm transition-transform duration-150 ${
          checked ? "translate-x-[20px]" : "translate-x-[2px]"
        }`}
      />
    </button>
  );
}
