import { describe, expect, it } from "vitest";
import { chocaCon, normalizarAtajo } from "./teclaZoom";

const DE_FABRICA = {
  zoomShortcut: "CmdOrCtrl+Shift+9",
  captureShortcut: "CmdOrCtrl+Shift+2",
  recordShortcut: "CmdOrCtrl+Shift+5",
  replayShortcut: "CmdOrCtrl+Shift+6",
  editShortcut: "Super+Shift+KeyL",
};

describe("la tecla de zoom", () => {
  it("se escribe de dos formas y es la misma tecla", () => {
    // La de fábrica viene como «9»; la que guarda el campo de atajos, como «Digit9».
    expect(normalizarAtajo("CmdOrCtrl+Shift+9")).toBe(normalizarAtajo("Ctrl+Shift+Digit9"));
    expect(normalizarAtajo("Shift+CmdOrCtrl+KeyZ")).toBe(normalizarAtajo("CmdOrCtrl+Shift+Z"));
    expect(normalizarAtajo("CmdOrCtrl+Shift+9")).not.toBe(normalizarAtajo("CmdOrCtrl+Alt+9"));
  });

  it("la de fábrica no choca con ninguna de las de winshotx", () => {
    expect(chocaCon(DE_FABRICA)).toBeNull();
  });

  it("avisa si es la de grabar, que pulsarla pararía la grabación", () => {
    expect(chocaCon({ ...DE_FABRICA, zoomShortcut: "CmdOrCtrl+Shift+Digit5" })).toBe("record");
  });

  it("y si es la de capturar o la de los últimos segundos", () => {
    expect(chocaCon({ ...DE_FABRICA, zoomShortcut: "Ctrl+Shift+2" })).toBe("capture");
    expect(chocaCon({ ...DE_FABRICA, zoomShortcut: "CmdOrCtrl+Shift+Digit6" })).toBe("replay");
    expect(chocaCon({ ...DE_FABRICA, zoomShortcut: "Super+Shift+L" })).toBe("edit");
  });
});
