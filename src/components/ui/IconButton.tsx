import type { LucideIcon } from "lucide-react";

interface Props {
  icon: LucideIcon;
  label: string;
  onClick?: () => void;
  active?: boolean;
  danger?: boolean;
  accent?: boolean;
  disabled?: boolean;
  /** Muestra el texto junto al icono en vez de solo el tooltip. */
  showLabel?: boolean;
  shortcut?: string;
}

export function IconButton({
  icon: Icon,
  label,
  onClick,
  active = false,
  danger = false,
  accent = false,
  disabled = false,
  showLabel = false,
  shortcut,
}: Props) {
  const tone = accent
    ? "bg-blue-500 text-white hover:bg-blue-400"
    : active
      ? "bg-white/15 text-white"
      : danger
        ? "text-red-400 hover:bg-red-500/15 hover:text-red-300"
        : "text-neutral-300 hover:bg-white/10 hover:text-white";

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={shortcut ? `${label} · ${shortcut}` : label}
      aria-label={label}
      className={`group flex h-9 shrink-0 items-center gap-1.5 rounded-lg px-2.5 text-sm font-medium whitespace-nowrap transition-colors duration-100 disabled:pointer-events-none disabled:opacity-40 ${tone}`}
    >
      <Icon className="size-[18px] shrink-0" strokeWidth={1.9} />
      {showLabel && <span className="pr-0.5">{label}</span>}
    </button>
  );
}
