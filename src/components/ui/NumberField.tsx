interface Props {
  value: number;
  onChange: (value: number) => void;
  label: string;
  min?: number;
  max?: number;
  suffix?: string;
  disabled?: boolean;
}

export function NumberField({
  value,
  onChange,
  label,
  min = 1,
  max = 99999,
  suffix,
  disabled = false,
}: Props) {
  return (
    <label className={`block ${disabled ? "opacity-40" : ""}`}>
      <span className="mb-1.5 block text-xs font-medium text-neutral-300">{label}</span>
      <span className="flex items-center rounded-lg border border-white/10 bg-black/30 focus-within:border-blue-500/60">
        <input
          type="number"
          value={Math.round(value)}
          min={min}
          max={max}
          disabled={disabled}
          onChange={(e) => {
            const next = Number(e.target.value);
            if (Number.isFinite(next)) onChange(Math.min(max, Math.max(min, next)));
          }}
          className="w-full bg-transparent px-2.5 py-1.5 text-sm tabular-nums text-white outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none"
        />
        {suffix && <span className="pr-2.5 text-xs text-neutral-500">{suffix}</span>}
      </span>
    </label>
  );
}
