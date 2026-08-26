import type { ReactNode } from "react";

interface SectionProps {
  title?: string;
  children: ReactNode;
}

/**
 * Un bloque de ajustes: una tarjeta con su nombre arriba, en el azul de la marca, y una
 * linea fina separando cada fila de la siguiente.
 *
 * El rotulo va DENTRO de la tarjeta, no flotando encima: como titulo suelto en mayusculas
 * gritaba un encabezado que el propio marco del bloque ya anuncia, y con cuatro bloques
 * seguidos la pantalla se leia como una lista de cajas sueltas en vez de como una pagina.
 * Es el mismo patron que usa VoCript, que es de donde viene esta forma.
 */
export function Section({ title, children }: SectionProps) {
  return (
    <section className="mb-3 w-full rounded-xl border border-white/8 bg-white/[0.03]">
      {title && (
        <h2 className="border-b border-white/8 px-4 py-3 text-[14.5px] font-semibold text-[#0a9bff]">
          {title}
        </h2>
      )}
      {/* La linea va entre filas y nunca encima de la primera. Se pone sobre los hijos
          directos y no sobre `.fila + .fila`: alguna fila llega envuelta en su propio
          contenedor, y una regla de hermanos adyacentes se saltaba justo esas. */}
      <div className="[&>*+*]:border-t [&>*+*]:border-white/8">{children}</div>
    </section>
  );
}

/** Qué dice la línea de debajo: un dato, un aviso que hay que leer, o algo ya resuelto. */
export type RowTone = "normal" | "warn" | "ok" | "error";

const TONO: Record<RowTone, string> = {
  normal: "text-neutral-400",
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
    <span className="flex min-w-0 items-center gap-3">
      <span className="flex w-4 shrink-0 justify-center text-neutral-500">{icon}</span>
      <span className="min-w-0">
        <span className="block truncate text-[14px] font-medium text-neutral-100">{label}</span>
        {hint && <span className={`block truncate text-[12.5px] ${TONO[tone]}`}>{hint}</span>}
      </span>
    </span>
  );

  if (stacked) {
    return (
      <div className="px-4 py-3">
        <div className="mb-2.5">{texto}</div>
        {control}
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between gap-4 px-4 py-2.5">
      {texto}
      <span className="shrink-0">{control}</span>
    </div>
  );
}

interface RowButtonProps {
  onClick: () => void;
  children: ReactNode;
  disabled?: boolean;
  /** Lo que sale al dejar el ratón encima, cuando el botón solo no se explica. */
  title?: string;
  /** Para el que hace algo que no se deshace solo. */
  danger?: boolean;
}

/** El botón secundario de una fila. Estaba copiado siete veces con las mismas clases. */
export function RowButton({
  onClick,
  children,
  disabled = false,
  danger = false,
  title,
}: RowButtonProps) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`rounded-md border px-2.5 py-1 text-[11.5px] whitespace-nowrap transition-colors disabled:opacity-40 ${
        danger
          ? "border-red-500/50 bg-red-500/15 text-red-300 hover:bg-red-500/25"
          : "border-white/10 text-neutral-300 hover:bg-white/10 hover:text-white"
      }`}
    >
      {children}
    </button>
  );
}
