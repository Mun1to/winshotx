import { Camera, Video, X } from "lucide-react";
import type { CaptureMode } from "../../lib/types";

interface Props {
  value: CaptureMode;
  onChange: (mode: CaptureMode) => void;
  onCancel: () => void;
  /** Se aparta mientras se arrastra, para no tapar lo que se está recortando. */
  dimmed: boolean;
}

/**
 * Qué se va a hacer con el recorte, elegido ANTES de recortar.
 *
 * Va arriba y centrada, donde Windows pone la suya, porque es el sitio donde la gente
 * ya la busca. Y hace falta: en el perfil "se copia sola" no salía ninguna barra, así
 * que desde ese perfil no había forma de grabar nada.
 */
export function ModeBar({ value, onChange, onCancel, dimmed }: Props) {
  return (
    <div
      onPointerDown={(e) => e.stopPropagation()}
      className={`pointer-events-auto absolute top-6 left-1/2 z-50 -translate-x-1/2 transition-opacity duration-150 ${
        dimmed ? "opacity-25" : "opacity-100"
      }`}
    >
      <div className="flex items-center gap-1 rounded-2xl border border-white/10 bg-neutral-900/90 p-1.5 shadow-2xl backdrop-blur-xl">
        <Boton
          activo={value === "still"}
          onClick={() => onChange("still")}
          tecla="F"
          titulo="Una imagen del recorte"
        >
          <Camera className="size-4" />
          Foto
        </Boton>

        <Boton
          activo={value === "video"}
          onClick={() => onChange("video")}
          tecla="V"
          titulo="Graba el recorte en MP4"
        >
          <Video className="size-4" />
          Vídeo
        </Boton>

        {/* Las letras, no un icono: no hay dibujo que diga "GIF" y el que había, unas
            chispas, no lo decía. Tres letras que todo el mundo reconoce. */}
        <Boton
          activo={value === "gif"}
          onClick={() => onChange("gif")}
          tecla="G"
          titulo="Graba el recorte en GIF, para pegarlo en cualquier sitio"
        >
          <span className="font-mono text-[11px] font-bold tracking-[0.06em]">GIF</span>
        </Boton>

        <span className="mx-1 h-6 w-px bg-white/10" />

        <button
          type="button"
          onClick={onCancel}
          title="Salir sin capturar (Esc)"
          className="flex size-8 items-center justify-center rounded-lg text-neutral-400 transition-colors hover:bg-red-500/20 hover:text-red-300"
        >
          <X className="size-4" />
        </button>
      </div>
    </div>
  );
}

function Boton({
  activo,
  onClick,
  tecla,
  titulo,
  children,
}: {
  activo: boolean;
  onClick: () => void;
  tecla: string;
  titulo: string;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={`${titulo} · ${tecla}`}
      aria-pressed={activo}
      className={`flex h-8 items-center gap-1.5 rounded-lg px-3 text-[13px] font-medium transition-colors ${
        activo
          ? "bg-blue-500 text-white shadow-sm"
          : "text-neutral-300 hover:bg-white/10 hover:text-white"
      }`}
    >
      {children}
    </button>
  );
}
