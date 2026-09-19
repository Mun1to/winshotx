/**
 * Del nombre que entiende Tauri al que lee una persona.
 *
 * Un atajo se guarda con el codigo fisico de la tecla (`CmdOrCtrl+Shift+KeyS`) porque es lo
 * unico que no cambia con la distribucion del teclado. Eso no se le puede enseñar a nadie:
 * en pantalla tiene que poner `Ctrl Shift S`.
 */
const BONITOS: Record<string, string> = {
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

export function nombreDeTecla(code: string): string {
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Numpad")) return `Num ${code.slice(6)}`;
  return BONITOS[code] ?? code;
}

export function partesDeAtajo(atajo: string): string[] {
  return atajo.split("+").filter(Boolean).map(nombreDeTecla);
}
