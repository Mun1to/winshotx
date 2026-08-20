import { useEffect, useRef, useState } from "react";

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

/** Del código físico de la tecla al nombre que entiende Tauri y al que lee el usuario. */
function labelFor(code: string): string {
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Numpad")) return `Num ${code.slice(6)}`;
  const bonitos: Record<string, string> = {
    CmdOrCtrl: "Ctrl",
    CommandOrControl: "Ctrl",
    Control: "Ctrl",
    Super: "Win",
    Meta: "Win",
    Escape: "Esc",
    Space: "Espacio",
    PrintScreen: "Impr Pant",
    ArrowUp: "↑",
    ArrowDown: "↓",
    ArrowLeft: "←",
    ArrowRight: "→",
    Backslash: "\\",
    Slash: "/",
    Comma: ",",
    Period: ".",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
  };
  return bonitos[code] ?? code;
}

function toParts(shortcut: string): string[] {
  return shortcut.split("+").filter(Boolean).map(labelFor);
}

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

      parts.push(e.code);
      onChange(parts.join("+"));
      setRecording(false);
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

  const parts = toParts(value);

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
