/**
 * La tecla de zoom al grabar (S2): saber si choca con otra tecla de winshotx.
 *
 * Mientras se graba, la tecla de zoom se lee a la vez que los atajos globales. Si es la
 * misma que la de grabar, pulsarla para acercar PARA la grabación; si es la de capturar,
 * abre una captura encima. Por eso la fila de Ajustes avisa antes de que pase.
 */
import type { Settings } from "./types";

/** Lo que significa un atajo, sin cómo se escribió: «CmdOrCtrl+Shift+9» y «Ctrl+Shift+Digit9» son el mismo. */
export function normalizarAtajo(atajo: string): string {
  const modificadores = new Set<string>();
  let tecla = "";
  for (const parte of atajo.split("+").map((p) => p.trim().toLowerCase())) {
    if (!parte) continue;
    if (["cmdorctrl", "commandorcontrol", "ctrl", "control"].includes(parte)) modificadores.add("ctrl");
    else if (["super", "win", "meta", "cmd", "command"].includes(parte)) modificadores.add("win");
    else if (["alt", "option"].includes(parte)) modificadores.add("alt");
    else if (parte === "shift") modificadores.add("shift");
    else tecla = parte.replace(/^(digit|key)(.)$/, "$2");
  }
  return [...[...modificadores].sort(), tecla].join("+");
}

/** Qué otra tecla de winshotx es la misma que la de zoom, o `null` si no choca con ninguna. */
export function chocaCon(
  ajustes: Pick<Settings, "zoomShortcut" | "captureShortcut" | "recordShortcut" | "replayShortcut" | "editShortcut">,
): "record" | "capture" | "replay" | "edit" | null {
  const zoom = normalizarAtajo(ajustes.zoomShortcut);
  const otras = [
    ["record", ajustes.recordShortcut],
    ["capture", ajustes.captureShortcut],
    ["replay", ajustes.replayShortcut],
    ["edit", ajustes.editShortcut],
  ] as const;
  return otras.find(([, atajo]) => normalizarAtajo(atajo) === zoom)?.[0] ?? null;
}
