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
import { EstadoActualizacion, useActualizacion } from "./Actualizacion";

const mirado = vi.fn();
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: () => {
    mirado();
    return Promise.resolve(null);
  },
}));

/** La barra de abajo, tal y como la monta la ventana de ajustes. */
function Barra() {
  const { fase, texto, alDia, instalar, mirar } = useActualizacion("0.2.11");
  return (
    <EstadoActualizacion
      fase={fase}
      texto={texto}
      alDia={alDia}
      instalar={instalar}
      mirar={() => void mirar(true)}
    />
  );
}

beforeEach(() => {
  mirado.mockClear();
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
  it("si pregunta a GitHub, y estando al dia deja el tick", async () => {
    responde("is_store_build", false);
    render(<Barra />);

    await waitFor(() => expect(mirado).toHaveBeenCalled());
    // Estando al dia la barra se queda en un solo boton, que es el tick de volver a mirar.
    await waitFor(() => expect(screen.getByRole("button")).toBeInTheDocument());
  });
});
