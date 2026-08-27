/**
 * La lupa de la selección.
 *
 * Enseña el píxel exacto que hay bajo el cursor, sus coordenadas y su color. El color se
 * puede copiar con la tecla C, y una tecla que no se ve no la usa nadie: que esa letra
 * esté escrita al lado es parte de la función, no un adorno.
 */
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { Magnifier } from "./Magnifier";
import { aplicarIdioma } from "../../lib/i18n";

function pintar(props: Partial<React.ComponentProps<typeof Magnifier>> = {}) {
  return render(
    <Magnifier source={null} px={1204} py={733} left={100} top={200} hex="#0a9bff" {...props} />,
  );
}

beforeEach(() => aplicarIdioma("es"));

describe("lo que dice la lupa", () => {
  it("las coordenadas del píxel, redondeadas", () => {
    pintar({ px: 1204.7, py: 733.2 });
    expect(screen.getByText("1205, 733")).toBeInTheDocument();
  });

  it("el color que hay debajo", () => {
    pintar();
    expect(screen.getByText("#0a9bff")).toBeInTheDocument();
  });

  it("y la tecla con la que se copia", () => {
    // Sin esto, el color estaba a la vista y no había forma de saber que se podía coger.
    pintar();
    expect(screen.getByText("C")).toBeInTheDocument();
  });

  it("la muestra del color va pintada de ese color", () => {
    const { container } = pintar({ hex: "#22c55e" });
    const muestra = [...container.querySelectorAll("span")].find(
      (s) => s.style.backgroundColor !== "",
    );
    expect(muestra).toBeDefined();
    expect(muestra!.style.backgroundColor).toBe("#22c55e");
  });
});

describe("dónde se coloca", () => {
  it("donde le digan, y sin comerse los clics", () => {
    // La lupa va encima de la pantalla congelada: si atrapara el ratón, arrastrar por
    // debajo de ella dejaría de seleccionar.
    const { container } = pintar({ left: 340, top: 120 });
    const caja = container.firstElementChild as HTMLElement;
    expect(caja.style.left).toBe("340px");
    expect(caja.style.top).toBe("120px");
    expect(caja.className).toContain("pointer-events-none");
  });
});
