import type { ReactNode } from "react";

interface SectionProps {
  title: string;
  children: ReactNode;
}

/** Bloque de ajustes: rótulo pequeño arriba y una tarjeta con las filas dentro. */
export function Section({ title, children }: SectionProps) {
  return (
    <section>
      <h2 className="mb-1 px-1 text-[10px] font-semibold tracking-[0.09em] text-neutral-500 uppercase">
        {title}
      </h2>
      <div className="divide-y divide-white/6 overflow-hidden rounded-xl border border-white/8 bg-white/[0.03]">
        {children}
      </div>
    </section>
  );
}

interface RowProps {
  label: string;
  hint?: string;
  icon?: ReactNode;
  control: ReactNode;
  /** Pone el control debajo, para sliders o selectores anchos. */
  stacked?: boolean;
}

export function Row({ label, hint, icon, control, stacked = false }: RowProps) {
  if (stacked) {
    return (
      <div className="px-3 py-2">
        <div className="mb-2 flex items-baseline justify-between gap-3">
          <span className="text-[13px] text-neutral-200">{label}</span>
          {hint && <span className="text-[11px] tabular-nums text-neutral-500">{hint}</span>}
        </div>
        {control}
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between gap-3 px-3 py-2">
      <span className="flex min-w-0 items-center gap-2.5">
        {icon && <span className="shrink-0 text-neutral-500">{icon}</span>}
        <span className="min-w-0">
          <span className="block truncate text-[13px] text-neutral-200">{label}</span>
          {hint && <span className="block truncate text-[11px] text-neutral-500">{hint}</span>}
        </span>
      </span>
      <span className="shrink-0">{control}</span>
    </div>
  );
}
