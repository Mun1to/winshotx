/**
 * La pantalla de carga del overlay y su cierre de rescate.
 *
 * El rescate existe para que un overlay opaco y sin fondo no se quede tapando el escritorio.
 * Lo que se comprueba aquí es que solo cierra la captura que ESTA ventana tiene entre manos:
 * las ventanas del overlay se crean al arrancar la app, sin ninguna captura, y su rescate
 * saltaba a los 8 segundos de nacer. Si la primera captura se estaba preparando justo
 * entonces, la anulaba y no se abría nada, sin ningún error (medido en X11 el 19 de
 * noviembre de 2026: 1 de cada 5 primeras capturas pedidas a los 8 segundos de arrancar).
 */
import { act, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BootScreen } from "./BootScreen";
import { aplicarIdioma } from "../../lib/i18n";
import { llamadas } from "../../test/preparar";

const SIN_CAPTURA = "el monitor 0 no tiene captura congelada";
const cierres = () => llamadas.filter((l) => l.comando === "cancel_capture").length;
const pasan = (ms: number) => act(() => vi.advanceTimersByTime(ms));

beforeEach(() => {
  aplicarIdioma("es");
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

describe("el cierre de rescate", () => {
  it("una ventana sin captura entre manos no cierra nada, pase lo que pase", () => {
    render(<BootScreen error={SIN_CAPTURA} generacion={null} />);
    pasan(60_000);
    expect(cierres()).toBe(0);
  });

  it("una captura que no llega a pintarse se cierra sola a los 8 segundos", () => {
    render(<BootScreen error={null} generacion={3} />);
    pasan(7900);
    expect(cierres()).toBe(0);
    pasan(200);
    expect(cierres()).toBe(1);
  });

  it("el plazo cuenta desde que llega la captura, no desde que la ventana existe", () => {
    // La ventana lleva 7,9 segundos creada y sin nada que hacer cuando llega la primera
    // captura. Antes, a los 8 segundos de NACER la cerraba: 100 ms después de pedirla.
    const vista = render(<BootScreen error={SIN_CAPTURA} generacion={null} />);
    pasan(7900);
    vista.rerender(<BootScreen error={null} generacion={1} />);
    pasan(200);
    expect(cierres()).toBe(0);
    pasan(7700);
    expect(cierres()).toBe(0);
    pasan(200);
    expect(cierres()).toBe(1);
  });

  it("una captura nueva vuelve a empezar la cuenta", () => {
    const vista = render(<BootScreen error={null} generacion={1} />);
    pasan(7000);
    vista.rerender(<BootScreen error={null} generacion={2} />);
    pasan(7000);
    expect(cierres()).toBe(0);
    pasan(1100);
    expect(cierres()).toBe(1);
  });

  it("Escape cierra siempre, con captura o sin ella", () => {
    render(<BootScreen error={SIN_CAPTURA} generacion={null} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(cierres()).toBe(1);
  });
});
