/**
 * El nombre que se le enseña a una persona de cada tecla de un atajo.
 *
 * La tecla del logo se guarda siempre como `Super`, pero se llama distinto en cada sistema.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { nombreDeTecla, partesDeAtajo } from "./teclas";

/** Lo que dicen de sí mismos WebView2 en Windows y WebKitGTK en Linux, también en Wayland. */
const EN_WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/142.0 Edg/142.0";
const EN_LINUX = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";

const correEn = (agente: string) => vi.spyOn(navigator, "userAgent", "get").mockReturnValue(agente);

afterEach(() => {
  vi.restoreAllMocks();
});

describe("la tecla del logo", () => {
  it("en Linux se llama Super", () => {
    correEn(EN_LINUX);
    expect(nombreDeTecla("Super")).toBe("Super");
    expect(nombreDeTecla("Meta")).toBe("Super");
    expect(partesDeAtajo("Super+Shift+KeyL")).toEqual(["Super", "Shift", "L"]);
  });

  it("en Windows se sigue llamando Win", () => {
    correEn(EN_WINDOWS);
    expect(nombreDeTecla("Super")).toBe("Win");
    expect(nombreDeTecla("Meta")).toBe("Win");
    expect(partesDeAtajo("Super+Shift+KeyS")).toEqual(["Win", "Shift", "S"]);
  });
});

describe("las demás teclas", () => {
  it("se llaman igual en los dos sistemas", () => {
    for (const agente of [EN_WINDOWS, EN_LINUX]) {
      correEn(agente);
      expect(partesDeAtajo("CmdOrCtrl+Alt+Digit2")).toEqual(["Ctrl", "Alt", "2"]);
    }
  });
});
