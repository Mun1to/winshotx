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

/**
 * La 0.5 (S37): la barra de abajo iba a 10 px y la tecla a 9. A 12 px las coordenadas y el
 * color no caben en una línea de 132 px, así que van en dos, y la barra no ensancha la lupa.
 */
describe("la barra de abajo", () => {
  it("a 12 px, en dos líneas y del ancho de la imagen", () => {
    pintar({ px: 1919, py: 1079 });
    const coordenadas = screen.getByText("1919, 1079");
    const barra = coordenadas.parentElement!;
    expect(barra.className).toContain("w-[132px]");
    expect(barra.className).toContain("flex-col");
    expect(barra.className).toContain("text-[12px]");
    expect(screen.getByText("C").className).toContain("text-[12px]");
  });
});
