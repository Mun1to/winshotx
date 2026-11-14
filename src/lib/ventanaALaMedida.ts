import { useCallback, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import {
  PhysicalPosition,
  PhysicalSize,
  currentMonitor,
  getCurrentWindow,
} from "@tauri-apps/api/window";

/**
 * La ventana de ajustes mide lo que mide lo que enseña (S24, 14-nov-2026).
 *
 * Antes la ventana abría a 840 x 760 para todas las pestañas: el alto de la más larga
 * (Grabar). En Capturar quedaban 160 px vacíos debajo de las tarjetas, y en Teclas de Windows
 * media ventana. Munito la achicaba a mano cada vez. Ahora cada pestaña pide su alto al
 * abrirse y al cambiar lo de dentro (encender el anillo añade filas, el inglés parte otras
 * frases), y la regla de siempre se cumple sola: ninguna pestaña de ajustes lleva rueda.
 *
 * Lo único que la para es la pantalla: la ventana no se sale nunca de la zona útil (la de
 * encima de la barra de tareas). En una pantalla baja, lo que no cabe se queda dentro con
 * su rueda, que es mejor que una ventana con el pie por debajo de la barra de tareas.
 */

/** Lo que se mide dentro de la página, en px CSS (que en Tauri son los lógicos). */
export interface Medidas {
  /** Lo que mide por dentro la ventana entera ahora mismo. */
  altoVentana: number;
  /** Lo que mide el hueco con rueda donde va la pestaña. */
  altoHueco: number;
  /** Lo que mide de verdad la pestaña, con sus márgenes. */
  altoContenido: number;
}

/**
 * El alto de dentro que pide la ventana: lo de fuera del hueco (cabecera, barra de abajo, un
 * aviso de error si lo hay) se queda como está, y el hueco pasa a medir su contenido.
 */
export function altoPedido(m: Medidas): number {
  return m.altoVentana - m.altoHueco + m.altoContenido;
}

/** Dónde está la ventana y dónde puede estar, en píxeles físicos (los de la pantalla). */
export interface Sitio {
  /** El alto que se pide por dentro, en px lógicos. */
  pedido: number;
  /** Lo que se deja como mucho, en px lógicos (el historial no crece con cada foto). */
  maximo?: number;
  escala: number;
  /** Lo que mide la ventana de alto por fuera menos lo de dentro: barra de título y bordes. */
  marco: number;
  /** El borde de arriba de la ventana, por fuera. */
  y: number;
  /** Lo que mide hoy de alto por fuera. */
  altoFuera: number;
  /** La zona útil de su pantalla, sin la barra de tareas. Null si no se sabe. */
  zona: { y: number; alto: number } | null;
  /**
   * La primera vez se conserva el centro (la ventana nace centrada con el alto de
   * `tauri.conf.json`); después, el borde de arriba, para que las pestañas no se muevan de
   * debajo del ratón al pulsarlas.
   */
  conservarCentro: boolean;
}

export interface Encaje {
  /** El alto de dentro, en píxeles físicos. */
  alto: number;
  /** El nuevo borde de arriba, por fuera; null si no hace falta moverla. */
  y: number | null;
}

/**
 * Cuánto mide la ventana y dónde va. Va aparte, sin Tauri, para probar las pantallas bajas y
 * las escalas sin tener esas pantallas.
 *
 * Se redondea hacia arriba en físicos: a 125 %, 600 px lógicos son 750 físicos justos, pero
 * 600,4 redondeado hacia abajo deja el hueco medio píxel corto y sale la rueda por nada.
 */
export function encajar(s: Sitio): Encaje {
  let alto = Math.ceil(Math.min(s.pedido, s.maximo ?? Infinity) * s.escala);
  if (s.zona) alto = Math.min(alto, s.zona.alto - s.marco);
  const fuera = alto + s.marco;
  let y = s.conservarCentro ? Math.round(s.y + (s.altoFuera - fuera) / 2) : s.y;
  if (s.zona) {
    // Primero que no se salga por abajo, y después por arriba: si hubiera que elegir, que
    // se vea la barra de título, que es por donde se mueve.
    y = Math.min(y, s.zona.y + s.zona.alto - fuera);
    y = Math.max(y, s.zona.y);
  }
  return { alto, y: y === s.y ? null : y };
}

/**
 * Ajusta la ventana a lo que hay dentro de `contenido`, que vive dentro de `hueco` (el
 * contenedor con rueda). Se vuelve a ajustar cuando el contenido cambia de tamaño (otra
 * pestaña, otras filas, otro idioma, otro ancho) y cuando cambia `otraVez`.
 *
 * Lo que NO hace es pelearse con quien la estira a mano: estirar el alto no cambia lo que
 * mide el contenido, así que no se dispara. Estirar el ancho sí, porque las frases se
 * parten distinto y el alto justo es otro.
 */
export function useVentanaALaMedida(
  hueco: RefObject<HTMLElement | null>,
  contenido: RefObject<HTMLElement | null>,
  opciones: { listo: boolean; maximo?: number; otraVez?: unknown },
) {
  const { listo, maximo, otraVez } = opciones;
  const primera = useRef(true);
  // Una sola a la vez: dos ajustes cruzados (el del cambio de pestaña y el del contenido que
  // termina de pintarse) podían acabar con el alto del primero puesto encima del segundo.
  const enCurso = useRef(false);
  const pendiente = useRef(false);
  const maximoRef = useRef(maximo);
  maximoRef.current = maximo;

  const ajustar = useCallback(async () => {
    if (enCurso.current) {
      pendiente.current = true;
      return;
    }
    enCurso.current = true;
    try {
      do {
        pendiente.current = false;
        const h = hueco.current;
        const c = contenido.current;
        if (!h || !c) return;
        const pedido = altoPedido({
          altoVentana: document.documentElement.clientHeight,
          altoHueco: h.clientHeight,
          altoContenido: c.getBoundingClientRect().height,
        });
        const ventana = getCurrentWindow();
        const [escala, dentro, fuera, posicion, pantalla] = await Promise.all([
          ventana.scaleFactor(),
          ventana.innerSize(),
          ventana.outerSize(),
          ventana.outerPosition(),
          currentMonitor().catch(() => null),
        ]);
        // Un Tauri de mentira que contesta null a todo (los bancos de `scripts/`) no puede
        // decir cuánto mide nada: mejor no tocar la ventana que pedirle alto cero.
        if (!(escala > 0) || !(dentro?.height > 0) || !(fuera?.height > 0)) return;
        const encaje = encajar({
          pedido,
          maximo: maximoRef.current,
          escala,
          marco: fuera.height - dentro.height,
          y: posicion.y,
          altoFuera: fuera.height,
          zona: pantalla
            ? { y: pantalla.workArea.position.y, alto: pantalla.workArea.size.height }
            : null,
          conservarCentro: primera.current,
        });
        primera.current = false;
        if (encaje.alto !== dentro.height) {
          await ventana.setSize(new PhysicalSize(dentro.width, encaje.alto));
        }
        if (encaje.y !== null) {
          await ventana.setPosition(new PhysicalPosition(posicion.x, encaje.y));
        }
      } while (pendiente.current);
    } catch {
      // Sin Tauri delante (las pruebas) o con la ventana cerrándose: se queda como está,
      // que es una ventana que se puede estirar a mano.
    } finally {
      enCurso.current = false;
    }
  }, [hueco, contenido]);

  useEffect(() => {
    if (!listo) return;
    const c = contenido.current;
    void ajustar();
    if (!c || typeof ResizeObserver === "undefined") return;
    const observador = new ResizeObserver(() => void ajustar());
    observador.observe(c);
    return () => observador.disconnect();
  }, [listo, ajustar, contenido]);

  useEffect(() => {
    if (listo) void ajustar();
  }, [listo, otraVez, maximo, ajustar]);

  // Si al final no cabe (una pantalla baja, el historial lleno, o la ventana encogida a mano),
  // el desvanecido de abajo dice «esto sigue». Cabiendo, sobra: tapaba el borde de abajo de
  // las tarjetas sin que hubiera nada que bajar.
  const [sobra, setSobra] = useState(false);
  useEffect(() => {
    if (!listo) return;
    const h = hueco.current;
    const c = contenido.current;
    if (!h || !c || typeof ResizeObserver === "undefined") return;
    const mirar = () => setSobra(c.getBoundingClientRect().height > h.clientHeight + 1);
    mirar();
    const observador = new ResizeObserver(mirar);
    observador.observe(h);
    observador.observe(c);
    return () => observador.disconnect();
  }, [listo, hueco, contenido]);
  return { sobra };
}
