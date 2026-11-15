import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
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
