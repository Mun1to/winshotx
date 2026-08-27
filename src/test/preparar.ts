/**
 * Lo que hace falta antes de cada prueba del frontend.
 *
 * Los componentes de winshotx viven dentro de una ventana de Tauri y hablan con Rust por
 * `invoke`. Aqui no hay Rust, asi que se le pone delante un doble que responde lo que se
 * le diga. Es el mismo truco de `scripts/ver-ventana.mjs`, pero sin servidor ni Chrome:
 * ahi se fotografia una pantalla entera para mirarla, y aqui se comprueba una pieza sola
 * sin que nadie tenga que mirar nada.
 */
import "@testing-library/jest-dom/vitest";
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

/** Lo que contesta cada comando de Rust en esta prueba. Se rellena con `responde`. */
const respuestas = new Map<string, unknown>();

/** Los comandos que se han llamado, en orden, para comprobar que se llamo lo que tocaba. */
export const llamadas: { comando: string; args: unknown }[] = [];

/** Los comandos que van a fallar, con el mensaje que devuelven. Se rellena con `falla`. */
const fallos = new Map<string, string>();

/** Pone lo que devolvera un comando de Rust durante la prueba. */
export function responde(comando: string, valor: unknown) {
  respuestas.set(comando, valor);
}

/**
 * Hace que un comando de Rust falle con ese mensaje.
 *
 * Un `invoke` que falla llega al frontend como una promesa rechazada con el texto del
 * error, no con un `Error`, y sin esto no habia forma de probar lo que se le ensenna a
 * alguien cuando algo sale mal, que es justo lo que nadie mira hasta que pasa.
 */
export function falla(comando: string, mensaje: string) {
  fallos.set(comando, mensaje);
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (comando: string, args: unknown) => {
    llamadas.push({ comando, args });
    const fallo = fallos.get(comando);
    if (fallo !== undefined) return Promise.reject(fallo);
    return Promise.resolve(respuestas.get(comando) ?? null);
  },
  // Las miniaturas y los congelados se piden por este protocolo, no por `invoke`.
  convertFileSrc: (ruta: string) => `asset://${ruta}`,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
  emit: () => Promise.resolve(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: () => Promise.resolve(),
    hide: () => Promise.resolve(),
    show: () => Promise.resolve(),
    setFocus: () => Promise.resolve(),
    label: "prueba",
  }),
}));

afterEach(() => {
  cleanup();
  respuestas.clear();
  fallos.clear();
  llamadas.length = 0;
  localStorage.clear();
});
