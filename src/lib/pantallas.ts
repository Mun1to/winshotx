/**
 * Ir y venir entre las coordenadas de una pantalla y las del escritorio virtual.
 *
 * Windows coloca los monitores en un unico plano: el principal empieza en (0, 0) y los
 * demas se cuelgan alrededor. **El que esta a la izquierda o encima del principal tiene
 * coordenadas NEGATIVAS**, y esa es la trampa que ya ha mordido tres veces en este
 * proyecto: recortar a cero (`Math.max(0, x)`) no protege de nada, manda la region a la
 * pantalla equivocada.
 *
 * Vive aparte del componente, y no dentro de un `useMemo`, por una razon: dentro no se
 * puede probar sin montar media aplicacion. Aqui son cuatro funciones sin estado que se
 * comprueban con numeros, incluidos los negativos.
 *
 * Y hay dos unidades distintas en juego:
 * - **Fisicas**: las que usan Rust y el escritorio virtual. Son los pixeles de verdad.
 * - **CSS**: las que ve el navegador de dentro de la ventana, ya divididas por el zoom
 *   que Windows le aplica a esa pantalla.
 *
 * La `escala` es el puente: cuantos pixeles fisicos vale un pixel CSS.
 */
import type { MonitorInfo, Rect } from "./types";

/** El origen de un monitor. Se pide asi para poder probar sin inventarse un monitor entero. */
export type Origen = Pick<MonitorInfo, "x" | "y" | "width" | "height">;

/**
 * De coordenadas CSS de esta pantalla a fisicas del escritorio virtual.
 *
 * El ancho y el alto nunca bajan de dos pixeles: un recorte de un pixel no es una captura,
 * es un resbalon del raton, y aguas abajo hay codificadores que no aceptan un lado de cero.
 */
export function aVirtual(rect: Rect, monitor: Origen, escala: number): Rect {
  return {
    x: Math.round(monitor.x + rect.x * escala),
    y: Math.round(monitor.y + rect.y * escala),
    width: Math.max(2, Math.round(rect.width * escala)),
    height: Math.max(2, Math.round(rect.height * escala)),
  };
}

/** La vuelta: de fisicas del escritorio virtual a CSS de esta pantalla. */
export function aPantalla(rect: Rect, monitor: Origen, escala: number): Rect {
  return {
    x: (rect.x - monitor.x) / escala,
    y: (rect.y - monitor.y) / escala,
    width: rect.width / escala,
    height: rect.height / escala,
  };
}

/**
 * Si una region del escritorio virtual es de esta pantalla, mirando su CENTRO.
 *
 * Por el centro y no por la esquina porque una region puede pisar dos monitores, y
 * entonces las dos pantallas la reclamarian o ninguna la querria. Rust decide igual al
 * elegir de que pantalla recorta, y las dos mitades tienen que estar de acuerdo: si aqui
 * se decidiera de otra forma, el fantasma de la ultima captura saldria en una pantalla y
 * la foto se cogeria de otra.
 */
export function esDeEstaPantalla(rect: Rect, monitor: Origen): boolean {
  const cx = rect.x + Math.floor(rect.width / 2);
  const cy = rect.y + Math.floor(rect.height / 2);
  return (
    cx >= monitor.x &&
    cy >= monitor.y &&
    cx < monitor.x + monitor.width &&
    cy < monitor.y + monitor.height
  );
}

/** Una ventana del sistema, tal y como la manda Rust. */
export interface VentanaDelSistema {
  title: string;
  rect: Rect;
}

/**
 * Las ventanas del sistema que se ven en esta pantalla, ya en coordenadas CSS.
 *
 * Se quedan fuera las que no asoman por aqui y las mas pequennas que ocho pixeles: a ese
 * tamanno no hay nada que ajustar, y ademas Windows tiene ventanas invisibles de uno o
 * dos pixeles que ensuciarian el ajuste automatico.
 */
export function ventanasDeEstaPantalla(
  ventanas: VentanaDelSistema[],
  monitor: Origen,
  escala: number,
  anchoCss: number,
  altoCss: number,
): VentanaDelSistema[] {
  return ventanas
    .map((v) => ({ title: v.title, rect: aPantalla(v.rect, monitor, escala) }))
    .filter(
      (v) =>
        v.rect.width > 8 &&
        v.rect.height > 8 &&
        v.rect.x < anchoCss &&
        v.rect.y < altoCss &&
        v.rect.x + v.rect.width > 0 &&
        v.rect.y + v.rect.height > 0,
    );
}

/** La ventana mas pequenna que hay bajo un punto, que es la que esta encima de las demas. */
export function ventanaBajoElPunto(
  ventanas: VentanaDelSistema[],
  x: number,
  y: number,
): VentanaDelSistema | null {
  const dentro = ventanas.filter(
    (v) =>
      x >= v.rect.x && y >= v.rect.y && x < v.rect.x + v.rect.width && y < v.rect.y + v.rect.height,
  );
  if (dentro.length === 0) return null;
  return dentro.reduce((mejor, v) =>
    v.rect.width * v.rect.height < mejor.rect.width * mejor.rect.height ? v : mejor,
  );
}
