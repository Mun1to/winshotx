/**
 * Lo de actualizar, y sobre todo lo de NO actualizar.
 *
 * Dentro de un paquete MSIX la carpeta de la app es de solo lectura, asi que un boton de
 * «Actualizar ahora» no podria instalar nada aunque encontrase una version nueva: daria un
 * error al final de la descarga, que es la peor forma de enterarse. Estas pruebas miran la
 * consecuencia, no el estado interno: que no se pinta ningun boton y que ni siquiera se
 * llega a preguntar a GitHub.
 */
import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { llamadas, responde } from "../../test/preparar";
import { AvisoActualizacion, BotonActualizar, useActualizacion } from "./Actualizacion";
import { aplicarIdioma } from "../../lib/i18n";

const mirado = vi.fn();
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: () => {
    mirado();
    return Promise.resolve(null);
  },
}));

/** El aviso de la cabecera y el botón de la fila de la versión, como los monta la ventana de ajustes. */
function Barra() {
  const { fase, texto, instalar, mirar } = useActualizacion("0.2.11");
  return (
    <>
      <AvisoActualizacion fase={fase} texto={texto} instalar={instalar} mirar={() => void mirar(true)} />
      <BotonActualizar fase={fase} instalar={instalar} mirar={() => void mirar(true)} />
    </>
  );
}

beforeEach(() => {
  mirado.mockClear();
  aplicarIdioma("es");
});

describe("viniendo de la Microsoft Store", () => {
  it("no pinta ningun control de actualizar", async () => {
    responde("is_store_build", true);
    const { container } = render(<Barra />);

    await waitFor(() => {
      expect(llamadas.some((l) => l.comando === "is_store_build")).toBe(true);
    });
    // Lo que ve el usuario: nada. Ni tick, ni boton, ni frase.
    await waitFor(() => expect(container.querySelector("button")).toBeNull());
    expect(screen.queryByText(/actualizar/i)).toBeNull();
  });

  it("no le pregunta a GitHub si hay version nueva", async () => {
    responde("is_store_build", true);
    render(<Barra />);

    await waitFor(() => {
      expect(llamadas.some((l) => l.comando === "is_store_build")).toBe(true);
    });
    // Y se le da margen de sobra por si la llamada fuese mas lenta que la comprobacion.
    await new Promise((r) => setTimeout(r, 50));
    expect(mirado).not.toHaveBeenCalled();
  });
});

describe("instalada desde el instalador de siempre", () => {
  it("si pregunta a GitHub, y estando al dia la cabecera no dice nada", async () => {
    responde("is_store_build", false);
    const { container } = render(<Barra />);

    await waitFor(() => expect(mirado).toHaveBeenCalled());
    // «Al día» con su tick era un «todo va bien» que no pedía nada: en la cabecera no sale
    // nada, y lo único que queda es el botón de buscar a mano, el de la fila de la versión.
    await waitFor(() => expect(screen.getByRole("button", { name: "Buscar actualizaciones" })).toBeInTheDocument());
    expect(container.querySelector("[data-aviso-actualizacion]")).toBeNull();
    expect(screen.queryByText(/al día/i)).toBeNull();
  });
});
