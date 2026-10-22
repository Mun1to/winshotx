import { Plus } from "lucide-react";
import { useT } from "../../lib/i18n";

/** Los de un clic. El primero es el de fabrica. */
export const COLORES_DEL_RESALTE = [
  "#f97316",
  "#ef4444",
  "#ec4899",
  "#8b5cf6",
  "#3b82f6",
  "#06b6d4",
  "#22c55e",
  "#eab308",
] as const;

interface Props {
  value: string;
  onChange: (color: string) => void;
}

/**
 * El color del resalte de la ventana bajo el cursor: ocho muestras y, al final, el
 * selector de Windows para cualquier otro. Si el elegido no es ninguna muestra, el hueco
 * del selector se pinta de ese color, para que se vea cual esta puesto.
 */
export function ColorDelResalte({ value, onChange }: Props) {
  const t = useT();
  const actual = value.toLowerCase();
  const propio = !COLORES_DEL_RESALTE.some((c) => c === actual);

  return (
    <div className="flex flex-wrap items-center gap-2" role="radiogroup" aria-label={t("Color del resalte")}>
      {COLORES_DEL_RESALTE.map((color) => (
        <button
          key={color}
          type="button"
          role="radio"
          aria-checked={actual === color}
          aria-label={color}
          title={color}
          onClick={() => onChange(color)}
          className={`size-6 rounded-full transition-transform duration-100 hover:scale-110 ${
            actual === color ? "ring-2 ring-titulo ring-offset-2 ring-offset-lienzo" : ""
          }`}
          style={{ backgroundColor: color }}
        />
      ))}
      <label
        title={t("Otro color")}
        className={`relative flex size-6 cursor-pointer items-center justify-center rounded-full border border-dashed border-linea-viva text-tenue transition-transform duration-100 hover:scale-110 ${
          propio ? "border-solid ring-2 ring-titulo ring-offset-2 ring-offset-lienzo" : ""
        }`}
        style={propio ? { backgroundColor: actual } : undefined}
      >
        {!propio && <Plus className="size-3.5" />}
        <input
          type="color"
          aria-label={t("Otro color")}
          value={/^#[0-9a-f]{6}$/.test(actual) ? actual : COLORES_DEL_RESALTE[0]}
          onChange={(e) => onChange(e.target.value)}
          className="absolute inset-0 cursor-pointer opacity-0"
        />
      </label>
    </div>
  );
}
