import {
  Check,
  Copy,
  Download,
  Film,
  Mic,
  MicOff,
  Pencil,
  Sparkles,
  X,
} from "lucide-react";
import { GlassPanel } from "../ui/GlassPanel";
import { IconButton } from "../ui/IconButton";

interface Props {
  left: number;
  top: number;
  /** La barra se pega arriba de la seleccion cuando abajo no cabe. */
  flipped: boolean;
  audio: boolean;
  busy: boolean;
  onCopy: () => void;
  onSave: () => void;
  onEdit: () => void;
  onRecordGif: () => void;
  onRecordVideo: () => void;
  onToggleAudio: () => void;
  onCancel: () => void;
}

export function FloatingToolbar({
  left,
  top,
  flipped,
  audio,
  busy,
  onCopy,
  onSave,
  onEdit,
  onRecordGif,
  onRecordVideo,
  onToggleAudio,
  onCancel,
}: Props) {
  return (
    <div
      style={{ left, top }}
      className={`absolute z-40 -translate-x-1/2 ${flipped ? "-translate-y-full" : ""}`}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <GlassPanel className="flex items-center gap-0.5 rounded-xl p-1">
        <IconButton icon={Copy} label="Copiar" shortcut="Enter" onClick={onCopy} disabled={busy} />
        <IconButton
          icon={Download}
          label="Guardar"
          shortcut="Ctrl+S"
          onClick={onSave}
          disabled={busy}
        />
        <IconButton icon={Pencil} label="Editar" shortcut="E" onClick={onEdit} disabled={busy} />
        <span className="mx-0.5 h-5 w-px bg-white/10" />
        <IconButton
          icon={Sparkles}
          label="Grabar GIF"
          shortcut="G"
          onClick={onRecordGif}
          disabled={busy}
        />
        <IconButton
          icon={Film}
          label="Grabar vídeo"
          shortcut="V"
          onClick={onRecordVideo}
          disabled={busy}
        />
        <IconButton
          icon={audio ? Mic : MicOff}
          label={audio ? "Audio activado" : "Audio silenciado"}
          shortcut="M"
          onClick={onToggleAudio}
          active={audio}
        />
        <span className="mx-0.5 h-5 w-px bg-white/10" />
        <IconButton icon={X} label="Cancelar" shortcut="Esc" onClick={onCancel} danger />
      </GlassPanel>
      {busy && (
        <div className="mt-1.5 flex items-center gap-1.5 rounded-lg bg-neutral-900/90 px-2 py-1 text-[11px] text-neutral-300 shadow-lg">
          <Check className="size-3 text-emerald-400" /> Procesando…
        </div>
      )}
    </div>
  );
}
