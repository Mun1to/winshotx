import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, X } from "lucide-react";

interface Props {
  title: string;
  subtitle: string;
  onClose: () => void;
}

export function TitleBar({ title, subtitle, onClose }: Props) {
  return (
    <header
      data-tauri-drag-region
      className="flex h-11 shrink-0 items-center justify-between border-b border-white/8 px-3"
    >
      <span data-tauri-drag-region className="flex items-baseline gap-2 pl-1">
        <span data-tauri-drag-region className="text-[13px] font-semibold text-white">
          {title}
        </span>
        <span data-tauri-drag-region className="text-[11px] tabular-nums text-neutral-500">
          {subtitle}
        </span>
      </span>
      <span className="flex items-center gap-1">
        <button
          type="button"
          aria-label="Minimizar"
          onClick={() => void getCurrentWindow().minimize()}
          className="flex size-7 items-center justify-center rounded-md text-neutral-400 transition-colors hover:bg-white/10 hover:text-white"
        >
          <Minus className="size-4" />
        </button>
        <button
          type="button"
          aria-label="Cerrar"
          onClick={onClose}
          className="flex size-7 items-center justify-center rounded-md text-neutral-400 transition-colors hover:bg-red-500 hover:text-white"
        >
          <X className="size-4" />
        </button>
      </span>
    </header>
  );
}
