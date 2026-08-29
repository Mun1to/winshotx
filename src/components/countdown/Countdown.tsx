import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Camera } from "lucide-react";
import { EVENTS } from "../../lib/types";

/** Un número de la URL, que es como viaja el primero: la página aún no escucha eventos. */
function deLaUrl(clave: string): number {
  const crudo = new URLSearchParams(window.location.search).get(clave);
  const numero = Number(crudo);
  return Number.isFinite(numero) && numero > 0 ? Math.floor(numero) : 0;
}

/**
 * Los segundos que faltan para el disparo, en una ventanita que no coge el foco.
 *
 * Quien cuenta de verdad es Rust, que duerme los segundos enteros y luego congela la
 * pantalla; esto solo baja el número de uno en uno para que se vea. Si los dos relojes se
 * separan unas décimas no importa: lo que la ventana promete es "queda poco", no un
 * cronómetro. Rust manda un cero al acabar, y el cero se dibuja como la cámara, no como
 * un número: el instante entre que la cuenta llega a cero y la selección aparece dice
 * "ya voy" en vez de enseñar un número que ya no significa nada.
 */
export function Countdown() {
  const [segundos, setSegundos] = useState(() => deLaUrl("segundos"));
  /**
   * El número de una pantalla, que se enseña quieto.
   *
   * La misma ventana sirve para dos cosas porque son la misma ventana: un número grande,
   * centrado en una pantalla, que no coge el foco. La diferencia es que la cuenta atrás
   * baja sola y esto no: aquí el número no significa tiempo, significa «esta pantalla».
   */
  const [pantalla, setPantalla] = useState(() => deLaUrl("pantalla"));

  // La ventana se reutiliza entre capturas (se esconde, no se cierra), así que el número
  // nuevo llega por evento y no por un remontaje. `target` no es opcional aquí aunque el
  // tipo lo deje pasar: sin él el oyente se apunta a todo lo que se emita. Ver la trampa
  // 8 de docs/TRAMPAS.md, que costó una sesión entera de depuración en el overlay.
  useEffect(() => {
    const etiqueta = getCurrentWindow().label;
    const unlisten = listen<number>(
      EVENTS.countdown,
      (e) => {
        setPantalla(0);
        setSegundos(e.payload);
      },
      { target: etiqueta },
    );
    const unPantalla = listen<number>(
      EVENTS.screenNumber,
      (e) => {
        setSegundos(0);
        setPantalla(e.payload);
      },
      { target: etiqueta },
    );
    return () => {
      void unlisten.then((fn) => fn());
      void unPantalla.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    // El número de una pantalla no baja: se queda quieto hasta que Rust esconde la ventana.
    if (segundos <= 0 || pantalla > 0) return;
    const tic = window.setTimeout(() => setSegundos((n) => n - 1), 1000);
    return () => window.clearTimeout(tic);
  }, [segundos, pantalla]);

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-[#161618] text-white">
      {pantalla > 0 ? (
        <span className="text-7xl font-semibold tabular-nums">{pantalla}</span>
      ) : segundos > 0 ? (
        // `tabular-nums` para que el número no baile de sitio al pasar de 3 a 2.
        <span className="text-6xl font-light tabular-nums">{segundos}</span>
      ) : (
        <Camera className="size-12 text-neutral-400" />
      )}
    </div>
  );
}
