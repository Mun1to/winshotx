import { useEffect, useRef, useState } from "react";
import { partesDeAtajo } from "../../lib/teclas";

/** Teclas que solo acompañan: por sí solas no forman un atajo. */
const MODIFIERS = new Set([
  "Control",
  "Shift",
  "Alt",
  "Meta",
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

interface Props {
  value: string;
  onChange: (value: string) => void;
  /** false cuando el sistema no ha dejado registrarlo. */
  active: boolean;
}

export function ShortcutField({ value, onChange, active }: Props) {
  const [recording, setRecording] = useState(false);
  const boxRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!recording) return;

    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        setRecording(false);
        return;
      }
      if (MODIFIERS.has(e.code) || MODIFIERS.has(e.key)) return;

      const parts: string[] = [];
      if (e.ctrlKey || e.metaKey) parts.push("CmdOrCtrl");
      if (e.shiftKey) parts.push("Shift");
      if (e.altKey) parts.push("Alt");
      // Un atajo global sin modificadores secuestraría la tecla en todo el sistema.
      if (parts.length === 0) return;
      // Sin código físico, el atajo saldría como "CmdOrCtrl+Shift+" y Windows no puede
      // registrar eso: pasa con el teclado en pantalla y con las teclas automatizadas.
      if (!e.code) return;

      parts.push(e.code);
      // Cerrar ANTES de guardar. Si el guardado se va por el desagüe, el campo se
      // quedaba pidiendo teclas para siempre y parecía que no se había enterado.
      setRecording(false);
      onChange(parts.join("+"));
    };

    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, onChange]);

  useEffect(() => {
    if (!recording) return;
    const onClickOutside = (e: MouseEvent) => {
      if (!boxRef.current?.contains(e.target as Node)) setRecording(false);
    };
    window.addEventListener("mousedown", onClickOutside);
    return () => window.removeEventListener("mousedown", onClickOutside);
  }, [recording]);

  const parts = partesDeAtajo(value);

  return (
    <button
      ref={boxRef}
      type="button"
      onClick={() => setRecording((r) => !r)}
      title={recording ? "Pulsa la combinación" : "Clic para cambiar el atajo"}
      className={`flex h-8 min-w-[132px] items-center justify-end gap-1 rounded-lg border px-2 transition-colors ${
        recording
          ? "animate-pulse border-blue-500 bg-blue-500/10"
          : active
            ? "border-white/10 bg-black/40 hover:border-white/25"
            : "border-red-500/40 bg-red-500/10 hover:border-red-400"
      }`}
    >
      {recording ? (
        <span className="w-full text-center text-[11px] text-blue-300">Pulsa las teclas…</span>
      ) : (
        parts.map((part, index) => (
          <kbd
            key={`${part}-${index}`}
            className="rounded-[5px] border border-white/10 bg-white/10 px-1.5 py-0.5 text-[11px] leading-none font-medium text-neutral-200"
          >
            {part}
          </kbd>
        ))
      )}
    </button>
  );
}
