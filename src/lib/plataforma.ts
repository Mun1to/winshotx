/**
 * Dónde corre winshotx, para no enseñar lo que ahí no existe.
 *
 * En Linux no hay Herramienta de Recortes ni teclas de Windows que quitarle, el arranque es
 * con la sesión y no «con Windows», y en Wayland las teclas no las pone winshotx: las elige
 * la persona en una ventana del escritorio. Todo eso sale de aquí.
 */
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { plataforma, teclasDelEscritorio } from "./ipc";
import { EVENTS } from "./types";
import type { Plataforma, TeclasDelEscritorio } from "./types";

/** Lo de siempre: si Rust no contesta (una versión vieja, las pruebas), se queda en Windows. */
export const EN_WINDOWS: Plataforma = { sistema: "windows", wayland: false };

/**
 * Dónde corre winshotx, o null mientras Rust contesta. Las pantallas esperan a saberlo antes
 * de pintarse: si no, en Linux se verían un instante las cosas de Windows.
 */
export function usePlataforma(): Plataforma | null {
  const [donde, setDonde] = useState<Plataforma | null>(null);
  useEffect(() => {
    let vivo = true;
    plataforma()
      .then((valor) => vivo && setDonde(valor ?? EN_WINDOWS))
      .catch(() => vivo && setDonde(EN_WINDOWS));
    return () => {
      vivo = false;
    };
  }, []);
  return donde;
}

/**
 * Las teclas que ha puesto el escritorio en Wayland, al día: el escritorio contesta cuando la
 * persona acepta o cambia las teclas en su ventana, y entonces llega `shortcutsChanged`.
 * null fuera de Wayland, donde mandan los ajustes.
 */
export function useTeclasDelEscritorio(enWayland: boolean): TeclasDelEscritorio | null {
  const [teclas, setTeclas] = useState<TeclasDelEscritorio | null>(null);
  useEffect(() => {
    if (!enWayland) return;
    let vivo = true;
    const leer = () =>
      void teclasDelEscritorio()
        .then((valor) => vivo && setTeclas(valor))
        .catch(() => undefined);
    leer();
    const cambios = listen(EVENTS.shortcutsChanged, leer);
    return () => {
      vivo = false;
      void cambios.then((quitar) => quitar());
    };
  }, [enWayland]);
  return teclas;
}
