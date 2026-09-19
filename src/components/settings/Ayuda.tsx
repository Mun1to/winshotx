import { useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useT } from "../../lib/i18n";

/** El hueco donde se dibuja, que en la app es la ventana entera. */
interface Ventana {
  ancho: number;
  alto: number;
}

/** Un rectángulo medido en pantalla, con lo que hace falta de un `DOMRect`. */
interface Caja {
  left: number;
  top: number;
  bottom: number;
  width: number;
  height: number;
}

/** El aire que se deja contra los bordes y entre el icono y el globo. */
const MARGEN = 8;

/**
 * Dónde se pone el globo, dado el icono, su propio tamaño y la ventana.
 *
 * Va aparte y sin tocar el DOM para poder probarla: colocar algo flotante se rompe
 * siempre en los mismos dos sitios, la última fila de abajo y la columna de la derecha, y
 * eso son cuentas, no pantallas. Es la misma razón por la que `sitio_de_la_barra` está
 * separada en Rust.
 */
export function sitioDelGlobo(icono: Caja, globo: Caja, ventana: Ventana) {
  // Empieza donde empieza el icono y crece hacia la derecha, no centrado: centrado se
  // salía de la tarjeta por la izquierda y tapaba la columna de al lado. Así el globo se
  // queda encima del bloque del que está hablando.
  const x = Math.max(MARGEN, Math.min(icono.left, ventana.ancho - globo.width - MARGEN));
  // Debajo si cabe, y si no, encima: las filas de abajo son media pantalla.
  const debajo = icono.bottom + MARGEN;
  const y =
    debajo + globo.height + MARGEN > ventana.alto
      ? Math.max(MARGEN, icono.top - globo.height - MARGEN)
      : debajo;
  return { x, y };
}

/**
 * El icono de un ajuste, convertido en la explicación larga de ese ajuste.
 *
 * Cada fila tiene un nombre corto y una línea de pista debajo, y ahí no cabe el porqué:
 * qué hace de verdad, qué cuesta y cuándo conviene. Eso se pone aquí, y se lee dejando el
 * ratón encima del icono, sin abrir ninguna pantalla ni empujar la fila hacia abajo.
 *
 * El globo se dibuja **fuera de la tarjeta**, pegado al `body`, y se coloca midiendo dónde
 * ha caído: si no cabe debajo se pone encima, y si se sale por un lado se arrima al borde.
 * Dentro de la tarjeta lo cortaría el marco de la ventana en las filas de abajo, que es
 * justo donde están los ajustes que más falta hace explicar.
 */
export function Ayuda({ texto, children }: { texto: string; children: ReactNode }) {
  const t = useT();
  const id = useId();
  const ancla = useRef<HTMLButtonElement>(null);
  const globo = useRef<HTMLDivElement>(null);
  const [abierto, setAbierto] = useState(false);

  useLayoutEffect(() => {
    if (!abierto || !globo.current || !ancla.current) return;
    const { x, y } = sitioDelGlobo(
      ancla.current.getBoundingClientRect(),
      globo.current.getBoundingClientRect(),
      { ancho: window.innerWidth, alto: window.innerHeight },
    );
    // Se coloca tocando el estilo y no el estado a propósito: guardar la posición
    // provocaría otro render, que volvería a medir, que volvería a guardar.
    globo.current.style.left = `${x}px`;
    globo.current.style.top = `${y}px`;
    globo.current.style.visibility = "visible";
  }, [abierto]);

  return (
    <>
      <button
        ref={ancla}
        type="button"
        // El icono no hace nada al pulsarlo, solo explica, así que lo dice el cursor y lo
        // dice el nombre accesible. En el teclado se lee igual: sale al recibir el foco.
        aria-label={t("Qué hace este ajuste")}
        aria-describedby={abierto ? id : undefined}
        // Punteros y no ratones: es lo que usa el resto de la app (el overlay entero se
        // maneja con eventos de puntero), y así vale igual para un lápiz.
        onPointerEnter={() => setAbierto(true)}
        onPointerLeave={() => setAbierto(false)}
        onFocus={() => setAbierto(true)}
        onBlur={() => setAbierto(false)}
        onKeyDown={(e) => {
          if (e.key === "Escape") setAbierto(false);
        }}
        className="flex w-4 shrink-0 cursor-help justify-center text-tenue transition-colors hover:text-marca focus-visible:text-marca"
      >
        {children}
      </button>
      {abierto &&
        createPortal(
          <div
            ref={globo}
            id={id}
            role="tooltip"
            // Nace invisible y en la esquina: se enseña cuando ya se ha medido, o se vería
            // un parpadeo en el sitio equivocado antes de saltar al bueno.
            style={{ position: "fixed", left: 0, top: 0, visibility: "hidden" }}
            className="pointer-events-none z-50 max-w-[19rem] rounded-lg border border-linea-fuerte bg-flotante px-3 py-2 text-[12.5px] leading-[1.45] text-suave shadow-2xl"
          >
            {texto}
          </div>,
          document.body,
        )}
    </>
  );
}
