import type { ReactNode } from "react";

interface SectionProps {
  title: string;
  /** Una línea bajo el rótulo, para cuando las filas de dentro comparten un porqué. */
  note?: string;
  children: ReactNode;
}

/** Bloque de ajustes: rótulo pequeño arriba y una tarjeta con las filas dentro. */
export function Section({ title, note, children }: SectionProps) {
  return (
    <section>
      <h2 className="px-1 text-[10px] font-semibold tracking-[0.09em] text-neutral-500 uppercase">
        {title}
      </h2>
      {note && <p className="mt-1 px-1 text-[11px] leading-snug text-neutral-500">{note}</p>}
      <div className="mt-1.5 divide-y divide-white/6 overflow-hidden rounded-xl border border-white/8 bg-white/[0.03]">
        {children}
      </div>
    </section>
  );
}

/** Qué dice la línea de debajo: un dato, un aviso que hay que leer, o algo ya resuelto. */
export type RowTone = "normal" | "warn" | "ok" | "error";

const TONO: Record<RowTone, string> = {
  normal: "text-neutral-500",
  warn: "text-amber-400/90",
  ok: "text-emerald-400/90",
  error: "text-red-400/90",
};

interface RowProps {
  label: string;
  hint?: ReactNode;
  icon?: ReactNode;
  control: ReactNode;
  /** Pone el control debajo, a lo ancho, para sliders o selectores. */
  stacked?: boolean;
  tone?: RowTone;
}

export function Row({ label, hint, icon, control, stacked = false, tone = "normal" }: RowProps) {
  // El hueco del icono se reserva siempre, tenga icono o no: sin eso, una fila sin
  // icono dejaba su texto cuatro pixeles a la izquierda y la columna salía torcida.
  const texto = (
    <span className="flex min-w-0 items-center gap-2.5">
      <span className="flex w-4 shrink-0 justify-center text-neutral-500">{icon}</span>
      <span className="min-w-0">
        <span className="block truncate text-[13px] text-neutral-200">{label}</span>
        {hint && <span className={`block truncate text-[11px] ${TONO[tone]}`}>{hint}</span>}
      </span>
    </span>
  );

  if (stacked) {
    return (
      <div className="px-3 py-2.5">
        <div className="mb-2.5">{texto}</div>
        {control}
      </div>
    );
  }

  // Altura fija para todas: con `py` a secas, las filas con explicación quedaban más
  // altas que las de una línea y la tarjeta parecía un montón de cajas sueltas.
  return (
    <div className="flex min-h-[46px] items-center justify-between gap-3 px-3 py-2">
      {texto}
      <span className="shrink-0">{control}</span>
    </div>
  );
}

interface RowButtonProps {
  onClick: () => void;
  children: ReactNode;
  disabled?: boolean;
  /** Para el que hace algo que no se deshace solo. */
  danger?: boolean;
}

/** El botón secundario de una fila. Estaba copiado siete veces con las mismas clases. */
export function RowButton({ onClick, children, disabled = false, danger = false }: RowButtonProps) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`rounded-md border px-2.5 py-1 text-[11px] whitespace-nowrap transition-colors disabled:opacity-40 ${
        danger
          ? "border-red-500/50 bg-red-500/15 text-red-300 hover:bg-red-500/25"
          : "border-white/10 text-neutral-300 hover:bg-white/10 hover:text-white"
      }`}
    >
      {children}
    </button>
  );
}
