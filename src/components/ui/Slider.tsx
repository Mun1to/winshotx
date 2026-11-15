interface Props {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
  label?: string;
  /** El nombre para el lector de pantalla cuando no lleva rótulo encima (S31, la barra del play). */
  ariaLabel?: string;
  hint?: string;
  disabled?: boolean;
}

export function Slider({
  value,
  min,
  max,
  step = 1,
  onChange,
  label,
  ariaLabel,
  hint,
  disabled = false,
}: Props) {
  const percent = ((value - min) / (max - min)) * 100;
  return (
    <label className={`block ${disabled ? "opacity-40" : ""}`}>
      {label && (
        <div className="mb-2 flex items-baseline justify-between gap-3">
          <span className="text-[15px] leading-snug font-medium text-titulo">{label}</span>
          {hint && <span className="shrink-0 text-[13px] tabular-nums text-suave">{hint}</span>}
        </div>
      )}
      <input
        type="range"
        // La etiqueta esta al lado, dentro del mismo `label`, pero el lector de pantalla
        // llega antes al control que al texto: sin esto anuncia «control deslizante» y ya.
        aria-label={label ?? ariaLabel}
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{
          // Con los tokens del tema: la pista de antes era blanca transparente y en claro no se veía.
          background: `linear-gradient(to right, var(--color-marca) ${percent}%, var(--color-linea-fuerte) ${percent}%)`,
        }}
        className="h-1.5 w-full cursor-pointer appearance-none rounded-full outline-none [&::-webkit-slider-thumb]:size-4 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white [&::-webkit-slider-thumb]:shadow-[0_0_0_1px_rgb(0_0_0/0.16),0_1px_3px_rgb(0_0_0/0.35)]"
      />
    </label>
  );
}
