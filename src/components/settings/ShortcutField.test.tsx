import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { aplicarIdioma } from "../../lib/i18n";
import { ShortcutField } from "./ShortcutField";

/**
 * El campo del atajo desde S40: el atajo como texto («Ctrl + Shift + S»), sin el recuadro de
 * fuera ni las teclas en cajitas, que Munito no quería. La raya azul de «se cambia aquí» es de
 * CSS (al pasar el ratón y con el foco) y se mira en foto; aquí, lo que se lee y lo que hace.
 */
describe("el campo del atajo", () => {
  beforeEach(() => aplicarIdioma("es"));

  it("se lee como texto, con un + entre tecla y tecla, y sin cajitas", () => {
    render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={() => undefined} />);
    const campo = screen.getByRole("button", { name: "Ctrl Shift S" });
    expect(campo.textContent).toBe("Ctrl+Shift+S");
    expect(campo.querySelector("kbd")).toBeNull();
    // Las teclas en la tinta de los títulos; el + más callado.
    const [ctrl, mas] = [...campo.querySelectorAll("span span")];
    expect(ctrl.className).toContain("text-titulo");
    expect(mas.className).toContain("text-apagado");
  });

  it("si Windows no se lo ha dado, las teclas van en rojo", () => {
    render(<ShortcutField value="Alt+KeyX" active={false} onChange={() => undefined} />);
    const alt = screen.getByText("Alt");
    expect(alt.className).toContain("light-dark(#b91c1c,#f87171)");
  });

  it("al pulsarlo escucha las teclas, y guarda la combinación que se pulse", () => {
    const onChange = vi.fn();
    render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={onChange} />);
    fireEvent.click(screen.getByRole("button"));
    expect(screen.getByText("Pulsa las teclas…")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "r", code: "KeyR", ctrlKey: true, altKey: true });
    expect(onChange).toHaveBeenCalledWith("CmdOrCtrl+Alt+KeyR");
    expect(screen.queryByText("Pulsa las teclas…")).not.toBeInTheDocument();
  });

  it("Esc deja de escuchar sin cambiar nada", () => {
    const onChange = vi.fn();
    render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={onChange} />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(window, { key: "Escape", code: "Escape" });
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Ctrl Shift S" })).toBeInTheDocument();
  });
});

/**
 * Los eventos son los medidos en WebKitGTK con la tecla de verdad, en X11 y en GNOME Wayland:
 * la tecla del logo se llama `Super`, su código es `OSLeft` y `metaKey` no se enciende nunca.
 */
describe("la tecla Super en WebKitGTK", () => {
  beforeEach(() => aplicarIdioma("es"));

  const SUPER = { key: "Super", code: "OSLeft" };
  const SHIFT = { key: "Shift", code: "ShiftLeft", shiftKey: true };

  function escuchando() {
    const onChange = vi.fn();
    render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={onChange} />);
    fireEvent.click(screen.getByRole("button"));
    return onChange;
  }

  it("Super, Mayús y K se guardan como Super+Shift+KeyK", () => {
    const onChange = escuchando();
    fireEvent.keyDown(window, SUPER);
    fireEvent.keyDown(window, SHIFT);
    fireEvent.keyDown(window, { key: "K", code: "KeyK", shiftKey: true });
    expect(onChange).toHaveBeenCalledWith("Super+Shift+KeyK");
  });

  it("apretadas al revés, Mayús primero, es el mismo atajo", () => {
    const onChange = escuchando();
    fireEvent.keyDown(window, SHIFT);
    fireEvent.keyDown(window, { ...SUPER, shiftKey: true });
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: "K", code: "KeyK", shiftKey: true });
    expect(onChange).toHaveBeenCalledWith("Super+Shift+KeyK");
  });

  it("Super y una letra, sin nada más, ya es un atajo", () => {
    const onChange = escuchando();
    fireEvent.keyDown(window, SUPER);
    fireEvent.keyDown(window, { key: "k", code: "KeyK" });
    expect(onChange).toHaveBeenCalledWith("Super+KeyK");
  });

  it("la Super de la derecha cuenta igual", () => {
    const onChange = escuchando();
    fireEvent.keyDown(window, { key: "Super", code: "OSRight" });
    fireEvent.keyDown(window, { key: "k", code: "KeyK" });
    expect(onChange).toHaveBeenCalledWith("Super+KeyK");
  });

  it("una vez suelta, deja de contar", () => {
    const onChange = escuchando();
    fireEvent.keyDown(window, SUPER);
    fireEvent.keyUp(window, SUPER);
    fireEvent.keyDown(window, { key: "k", code: "KeyK", ctrlKey: true });
    expect(onChange).toHaveBeenCalledWith("CmdOrCtrl+KeyK");
  });

  it("si el escritorio se lleva el foco, no se queda pegada", () => {
    // Super sola abre las actividades de GNOME: llega el keydown, luego blur y nunca el keyup.
    const onChange = escuchando();
    fireEvent.keyDown(window, SUPER);
    fireEvent.blur(window);
    fireEvent.keyDown(window, { key: "k", code: "KeyK", ctrlKey: true });
    expect(onChange).toHaveBeenCalledWith("CmdOrCtrl+KeyK");
  });

  it("en Windows la tecla del logo sigue guardándose igual", () => {
    const onChange = escuchando();
    fireEvent.keyDown(window, { key: "Meta", code: "MetaLeft", metaKey: true });
    fireEvent.keyDown(window, { key: "S", code: "KeyS", metaKey: true, shiftKey: true });
    expect(onChange).toHaveBeenCalledWith("Super+Shift+KeyS");
  });
});

describe("revisión del campo de atajos con Super en WebKitGTK", () => {
  beforeEach(() => aplicarIdioma("es"));

  const SUPER = { key: "Super", code: "OSLeft" };
  const LETRA = { key: "K", code: "KeyK", shiftKey: true };

  it("control: Super y la letra, sin que el padre se repinte", () => {
    const guardado = vi.fn();
    render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={(v) => guardado(v)} />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(window, SUPER);
    fireEvent.keyDown(window, LETRA);
    expect(guardado).toHaveBeenCalledWith("Super+Shift+KeyK");
  });

  it("el padre se repinta entre Super y la letra (onChange es una flecha nueva)", () => {
    const guardado = vi.fn();
    const { rerender } = render(
      <ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={(v) => guardado(v)} />,
    );
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(window, SUPER);
    rerender(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={(v) => guardado(v)} />);
    fireEvent.keyDown(window, LETRA);
    expect(guardado).toHaveBeenCalledWith("Super+Shift+KeyK");
  });

  it("tras repintarse, guarda con el onChange más reciente y no con el viejo", () => {
    const viejo = vi.fn();
    const nuevo = vi.fn();
    const { rerender } = render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={viejo} />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(window, SUPER);
    rerender(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={nuevo} />);
    fireEvent.keyDown(window, LETRA);
    expect(viejo).not.toHaveBeenCalled();
    expect(nuevo).toHaveBeenCalledWith("Super+Shift+KeyK");
  });

  it("soltar Super antes de la letra no deja Super pegada", () => {
    const guardado = vi.fn();
    render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={(v) => guardado(v)} />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(window, SUPER);
    fireEvent.keyUp(window, SUPER);
    fireEvent.keyDown(window, LETRA);
    expect(guardado).toHaveBeenCalledWith("Shift+KeyK");
  });

  it("perder el foco con Super pulsada la olvida", () => {
    const guardado = vi.fn();
    render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={(v) => guardado(v)} />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(window, SUPER);
    fireEvent.blur(window);
    fireEvent.keyDown(window, LETRA);
    expect(guardado).toHaveBeenCalledWith("Shift+KeyK");
  });

  it("con las dos Super, soltar una deja la otra contando", () => {
    const guardado = vi.fn();
    render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={(v) => guardado(v)} />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(window, SUPER);
    fireEvent.keyDown(window, { key: "Super", code: "OSRight" });
    fireEvent.keyUp(window, SUPER);
    fireEvent.keyDown(window, LETRA);
    expect(guardado).toHaveBeenCalledWith("Super+Shift+KeyK");
  });
});

describe("la pista mientras escucha las teclas", () => {
  const EN_LINUX = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
  const EN_WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/142.0 Edg/142.0";

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function pista(agente: string, idioma: "es" | "en" | "zh") {
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue(agente);
    aplicarIdioma(idioma);
    const { unmount } = render(<ShortcutField value="CmdOrCtrl+Shift+KeyS" active onChange={() => undefined} />);
    fireEvent.click(screen.getByRole("button"));
    const titulo = screen.getByRole("button").getAttribute("title");
    unmount();
    return titulo;
  }

  it("en Linux habla del escritorio, en los tres idiomas", () => {
    expect(pista(EN_LINUX, "es")).toBe("Pulsa la combinación · las que el escritorio se reserva no llegan hasta aquí");
    expect(pista(EN_LINUX, "en")).toBe("Press the combination · the ones the desktop keeps never reach here");
    expect(pista(EN_LINUX, "zh")).toBe("请按下组合键 · 桌面保留的组合键不会传到这里");
  });

  it("en Windows sigue hablando de Windows", () => {
    expect(pista(EN_WINDOWS, "es")).toBe("Pulsa la combinación · las que Windows se reserva no llegan hasta aquí");
    expect(pista(EN_WINDOWS, "en")).toBe("Press the combination · the ones Windows keeps never reach here");
    expect(pista(EN_WINDOWS, "zh")).toBe("请按下组合键 · Windows 保留的组合键不会传到这里");
  });
});
