interface Props {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
  label?: string;
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
  hint,
  disabled = false,
}: Props) {
  const percent = ((value - min) / (max - min)) * 100;
  return (
    <label className={`block ${disabled ? "opacity-40" : ""}`}>
      {label && (
        <div className="mb-1.5 flex items-baseline justify-between">
          <span className="text-xs font-medium text-suave">{label}</span>
          {hint && <span className="text-xs tabular-nums text-tenue">{hint}</span>}
        </div>
      )}
      <input
        type="range"
        // La etiqueta esta al lado, dentro del mismo `label`, pero el lector de pantalla
        // llega antes al control que al texto: sin esto anuncia «control deslizante» y ya.
        aria-label={label}
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{
          background: `linear-gradient(to right, rgb(59 130 246) ${percent}%, rgb(255 255 255 / 0.12) ${percent}%)`,
        }}
        className="h-1.5 w-full cursor-pointer appearance-none rounded-full outline-none [&::-webkit-slider-thumb]:size-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white [&::-webkit-slider-thumb]:shadow-md"
      />
    </label>
  );
}
