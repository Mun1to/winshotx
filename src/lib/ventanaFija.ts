import { useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import {
  PhysicalPosition,
  PhysicalSize,
  currentMonitor,
  getCurrentWindow,
} from "@tauri-apps/api/window";
import { guardarVentanaAjustes } from "./ipc";
import { corriendoEnLinux } from "./teclas";

/**
 * La ventana de ajustes (y la bienvenida, que vive en la misma) tiene siempre el mismo tamaño
 * (S40, 15-nov-2026). Lo dijo Munito con la 0.5.0 delante y el historial lleno: «no me gusta
 * que de repente se agrande tanto la interfaz que haya que scrollear; el tamaño siempre el
 * mismo, a no ser que lo modifiques tú».
 *
 * De S24 a la 0.5.0 la ventana medía lo que medía cada apartado: crecía y encogía al pulsar
 * el riel, y el historial la estiraba hasta 760. Ahora:
 *
 * - abre a `ANCHO_VENTANA` x `ALTO_VENTANA`, el alto que hace caber sin rueda el apartado más
 *   alto de los fijos (Grabar), y no se toca al cambiar de apartado ni de paso;
 * - lo que no cabe (el historial lleno, una pantalla baja) baja con la rueda DENTRO, con la
 *   cabecera y el riel quietos;
 * - si alguien la estira a mano, su tamaño se guarda (`ventanaAjustes` en los ajustes) y la
 *   próxima vez abre así;
 * - y nunca se sale de la zona útil de su pantalla (la de encima de la barra de tareas): en un
 *   portátil de 1366 x 768 al 125 % cabe lo que cabe y el resto baja con la rueda.
 *
 * La ventana no se destruye al cerrarla, solo se esconde, así que esto pasa una vez por
 * arranque de la app: dentro de una misma sesión se queda con el tamaño que tenga.
 */

/** El ancho de serie, en px lógicos: el riel, las filas y el dibujo de «Así va a salir». */
export const ANCHO_VENTANA = 900;
/**
 * El alto de serie por dentro, en px lógicos: el de Grabar a 900 de ancho (cabecera, título,
 * sus dos grupos de filas, el dibujo y la barra de abajo), medido en Edge con
 * `scripts/probar-medida-ajustes.mjs`, que falla si algún apartado fijo deja de caber. Era 681
 * hasta que volvió la barra de abajo (S40), que se lleva 44. Es también el de
 * `tauri.conf.json`, para que la ventana nazca así y no cambie al cargar (lo vigila
 * `ventanaFija.test.ts`).
 */
export const ALTO_VENTANA = 723;

/** Un tamaño, en px lógicos. */
export interface Tamanno {
  ancho: number;
  alto: number;
}

/** Dónde está la ventana y dónde puede estar, todo en píxeles físicos (los de la pantalla). */
export interface Sitio {
  /** Lo que se quiere por dentro, en px LÓGICOS. */
  quiere: Tamanno;
  escala: number;
  /** Lo que mide la ventana por fuera menos lo de dentro: barra de título y bordes. */
  marco: { ancho: number; alto: number };
  /** Lo que ocupa hoy por fuera. */
  fuera: { x: number; y: number; ancho: number; alto: number };
  /** La zona útil de su pantalla, sin la barra de tareas. Null si no se sabe. */
  zona: { x: number; y: number; ancho: number; alto: number } | null;
}

/** Cómo se queda: el tamaño de dentro y el borde de arriba a la izquierda de fuera, en físicos. */
export interface Colocacion {
  ancho: number;
  alto: number;
  x: number;
  y: number;
}

/**
 * Cuánto mide la ventana y dónde va. Va aparte, sin Tauri, para probar las pantallas bajas y
 * las escalas sin tener esas pantallas.
 *
 * Se conserva el centro (nace centrada en su pantalla) y, si no cabe, se encoge a la zona útil
 * y se mete dentro. Se redondea hacia arriba en físicos: a 125 %, 723 lógicos son 903,75, y
 * redondeando hacia abajo el hueco se queda medio píxel corto y sale la rueda por nada.
 */
export function colocar(s: Sitio): Colocacion {
  let ancho = Math.ceil(s.quiere.ancho * s.escala);
  let alto = Math.ceil(s.quiere.alto * s.escala);
  if (s.zona) {
    ancho = Math.min(ancho, s.zona.ancho - s.marco.ancho);
    alto = Math.min(alto, s.zona.alto - s.marco.alto);
  }
  const anchoFuera = ancho + s.marco.ancho;
  const altoFuera = alto + s.marco.alto;
  let x = Math.round(s.fuera.x + (s.fuera.ancho - anchoFuera) / 2);
  let y = Math.round(s.fuera.y + (s.fuera.alto - altoFuera) / 2);
  if (s.zona) {
    // Primero que no se salga por abajo y por la derecha, y después por arriba y por la
    // izquierda: si hubiera que elegir, que se vea la barra de título, que es por donde se mueve.
    x = Math.max(Math.min(x, s.zona.x + s.zona.ancho - anchoFuera), s.zona.x);
    y = Math.max(Math.min(y, s.zona.y + s.zona.alto - altoFuera), s.zona.y);
  }
  return { ancho, alto, x, y };
}

/**
 * Si un tamaño de la ventana lo ha puesto alguien a mano o es el que puso la app. Dos píxeles
 * de margen: al pasar la ventana a una pantalla con otra escala, Windows la reescala y los
 * lógicos bailan por el redondeo, y eso no es que nadie la haya estirado.
 */
export function esDeLaApp(lo: Tamanno, propio: Tamanno): boolean {
  return Math.abs(lo.ancho - propio.ancho) <= 2 && Math.abs(lo.alto - propio.alto) <= 2;
}

/**
 * Lo que mide la ventana por dentro según la página, en px lógicos, o `null` si todavía no mide
 * nada (arrancada con la sesión está escondida).
 *
 * S45 (Linux): en GNOME Wayland la barra de título la dibuja GTK dentro de la propia ventana,
 * con su sombra alrededor, y el tamaño «de dentro» que da Tauri, y el de su aviso de tamaño
 * nuevo, los cuentan: una ventana de 900 x 723 dice medir 952 x 822. Con eso, solo enseñarla
 * ya parecía que alguien la había estirado: se guardaba 952 x 822 y el arranque siguiente la
 * abría así de grande. La página sabe lo que mide en cualquier escritorio, así que en Linux
 * se le pregunta a ella. En Windows se sigue preguntando a Tauri, como siempre.
 */
function dentroSegunLaPagina(): Tamanno | null {
  const ancho = window.innerWidth;
  const alto = window.innerHeight;
  return ancho > 0 && alto > 0 ? { ancho, alto } : null;
}

/** Lo que se espera desde el último tirón antes de guardar: estirar una ventana son decenas. */
const ESPERA_GUARDAR = 500;

/**
 * Pone la ventana a su tamaño al abrir (el guardado o el de serie) y, desde ahí, apunta el que
 * le dé quien la estire a mano. Va en `App.tsx`, que es quien tiene la ventana: la bienvenida
 * y los ajustes se turnan dentro y ninguno de los dos la toca.
 */
export function useVentanaFija(listo: boolean, guardada: Tamanno | null | undefined) {
  // Una vez por arranque, aunque React monte dos veces (StrictMode en desarrollo).
  const colocada = useRef(false);
  // El tamaño que ha puesto la app, en lógicos: lo que se mida igual no es de nadie.
  const propio = useRef<Tamanno>(guardada ?? { ancho: ANCHO_VENTANA, alto: ALTO_VENTANA });

  useEffect(() => {
    if (!listo) return;
    const ventana = getCurrentWindow();
    let soltar: (() => void) | null = null;
    let espera: number | undefined;
    let vivo = true;

    if (!colocada.current) {
      colocada.current = true;
      if (guardada) propio.current = guardada;
      void colocarAlAbrir(propio.current, (puesto) => {
        propio.current = puesto;
      });
    }

    // Desde aquí, lo que cambie de tamaño lo ha cambiado quien la usa.
    void ventana
      .onResized(({ payload }) => {
        window.clearTimeout(espera);
        // Minimizada mide cero.
        if (!(payload.width > 0) || !(payload.height > 0)) return;
        espera = window.setTimeout(() => {
          void (async () => {
            const [escala, maximizada] = await Promise.all([
              ventana.scaleFactor(),
              ventana.isMaximized().catch(() => false),
            ]);
            // Maximizada no es «su tamaño»: al volver, vuelve al de antes.
            if (maximizada || !(escala > 0)) return;
            const lo = corriendoEnLinux()
              ? dentroSegunLaPagina()
              : {
                  ancho: Math.round(payload.width / escala),
                  alto: Math.round(payload.height / escala),
                };
            // En Linux, con la página sin medir nada todavía, no hay tamaño que apuntar.
            if (!lo || esDeLaApp(lo, propio.current)) return;
            propio.current = lo;
            await guardarVentanaAjustes(lo.ancho, lo.alto);
          })().catch(() => undefined);
        }, ESPERA_GUARDAR);
      })
      .then((quitar) => {
        if (vivo) soltar = quitar;
        else quitar();
      })
      // Sin eventos de ventana no se recuerda el tamaño, pero la ventana sigue funcionando.
      .catch(() => undefined);

    return () => {
      vivo = false;
      window.clearTimeout(espera);
      soltar?.();
    };
    // `guardada` solo cuenta la primera vez: después manda lo que haga quien la usa.
  }, [listo]);
}

/**
 * Pone la ventana al tamaño `quiere` (en lógicos), centrada donde estaba y dentro de la zona
 * útil de su pantalla. `puesto` recibe lo que se va a poner, en lógicos, ANTES de pedirlo: el
 * aviso de tamaño nuevo llega mientras se cambia, y tiene que verse como de la app.
 */
async function colocarAlAbrir(quiere: Tamanno, puesto: (t: Tamanno) => void): Promise<void> {
  const ventana = getCurrentWindow();
  try {
    const [escala, deTauri, fuera, posicion, pantalla] = await Promise.all([
      ventana.scaleFactor(),
      ventana.innerSize(),
      ventana.outerSize(),
      ventana.outerPosition(),
      currentMonitor().catch(() => null),
    ]);
    // Un Tauri de mentira que contesta null a todo no puede decir cuánto mide nada: mejor no
    // tocar la ventana que pedirle tamaño cero.
    if (!(escala > 0) || !(deTauri?.height > 0) || !(fuera?.height > 0)) return;
    // En Linux lo de dentro lo dice la página (`dentroSegunLaPagina`); si aún no mide nada,
    // vale lo de Tauri, que con la ventana sin enseñar es el tamaño con el que nació.
    const pagina = corriendoEnLinux() ? dentroSegunLaPagina() : null;
    const dentro = pagina
      ? { width: Math.round(pagina.ancho * escala), height: Math.round(pagina.alto * escala) }
      : deTauri;
    const sitio = colocar({
      quiere,
      escala,
      marco: { ancho: fuera.width - dentro.width, alto: fuera.height - dentro.height },
      fuera: { x: posicion.x, y: posicion.y, ancho: fuera.width, alto: fuera.height },
      zona: pantalla
        ? {
            x: pantalla.workArea.position.x,
            y: pantalla.workArea.position.y,
            ancho: pantalla.workArea.size.width,
            alto: pantalla.workArea.size.height,
          }
        : null,
    });
    puesto({ ancho: sitio.ancho / escala, alto: sitio.alto / escala });
    if (sitio.ancho !== dentro.width || sitio.alto !== dentro.height) {
      await ventana.setSize(new PhysicalSize(sitio.ancho, sitio.alto));
    }
    if (sitio.x !== posicion.x || sitio.y !== posicion.y) {
      await ventana.setPosition(new PhysicalPosition(sitio.x, sitio.y));
    }
  } catch {
    // Sin Tauri delante (las pruebas) o con la ventana cerrándose: se queda como está.
  }
}

/**
 * Si lo de dentro no cabe en su hueco con rueda: entonces el desvanecido de abajo dice «esto
 * sigue». Cabiendo, sobra: tapaba el borde de abajo de las filas sin que hubiera nada que bajar.
 */
export function useSobra(
  hueco: RefObject<HTMLElement | null>,
  contenido: RefObject<HTMLElement | null>,
  listo: boolean,
) {
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
  return sobra;
}
