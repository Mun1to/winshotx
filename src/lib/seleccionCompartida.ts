/**
 * Una sola seleccion para todo el escritorio, aunque haya un overlay por pantalla.
 *
 * Cada monitor tiene su ventana de overlay y su propio React, y hasta noviembre de 2026 cada
 * uno tenia su recorte: no se podia empezar a arrastrar en una pantalla y seguir en la de al
 * lado. Ahora el recorte es UNO, y su verdad va en pixeles FISICOS enteros del escritorio
 * virtual. Cada overlay lo guarda en SUS pixeles CSS sin recortarlo a su ventana
 * (`aPantalla`), y el que lo cambia se lo cuenta a los demas con `EVENTO_SELECCION`.
 *
 * Aqui solo hay cuentas sin estado, con numeros, para poder probarlas con las pantallas de
 * verdad: una a la izquierda con x NEGATIVA y mas baja que la principal, y un portatil
 * debajo al 125 %. El plano entero esta en el plano de dos pantallas (S42).
 */
import type { MonitorInfo, Rect } from "./types";

/**
 * Lo que se cuentan los overlays sobre el recorte. Va SIEMPRE dirigido, con `emitTo` a la
 * ventana `overlay-<id>` y `listen` con su `target`: un `listen` sin destino oye cualquier
 * cosa, tambien el eco de lo que manda la propia ventana (la trampa del pool de overlays).
 */
export const EVENTO_SELECCION = "winshotx://overlay-seleccion";

/**
 * Cuantos pixeles fisicos hay que meterse en otra pantalla para que el recorte cruce.
 *
 * Quien arrastra hasta el borde de su pantalla suele pasarse unos pixeles con el raton, y
 * hasta ahora el recorte se quedaba en el borde. Con el cruce, ese pasarse meteria una tira
 * de 1 a 5 px de la pantalla de al lado (y una imagen de dos pantallas con un hueco). Por
 * debajo de esto el lado se queda pegado al borde, como siempre; a partir de aqui, cruza.
 */
export const IMAN = 24;

/** En que anda el que manda el recorte. `quieta` es que ya lo ha soltado. */
export type Fase = "dibujando" | "moviendo" | "estirando" | "quieta";

export interface Punto {
  x: number;
  y: number;
}

/** Lo que viaja en `EVENTO_SELECCION`. */
export interface SeleccionCompartida {
  /** `payload.generation`: lo de otra captura se ignora. */
  generacion: number;
  /** El `monitor.id` de quien lo manda: lo propio (el eco) se ignora. */
  emisor: number;
  /** En fisicos enteros del escritorio virtual; `null` es que no hay recorte. */
  rect: Rect | null;
  fase: Fase;
  /** El raton, en fisicos, para que la lupa salga en la pantalla donde esta. */
  cursor: Punto | null;
}

/** La etiqueta de la ventana de overlay de ese monitor (`windows_mgr::OVERLAY_PREFIX`). */
export function etiquetaDeOverlay(id: number): string {
  return `overlay-${id}`;
}

/**
 * Las pantallas de esta captura, con la propia dentro y tal como la ve este overlay.
 *
 * Un payload viejo no trae la lista: entonces es como hoy, una sola pantalla y sin cruzar.
 */
export function monitoresDe(propio: MonitorInfo, todos?: MonitorInfo[] | null): MonitorInfo[] {
  if (!todos || todos.length === 0) return [propio];
  if (!todos.some((m) => m.id === propio.id)) return [propio, ...todos];
  return todos.map((m) => (m.id === propio.id ? propio : m));
}

/** Lo que tienen en comun dos rectangulos, o `null` si no comparten ni un pixel. */
export function interseccion(a: Rect, b: Rect): Rect | null {
  const x = Math.max(a.x, b.x);
  const y = Math.max(a.y, b.y);
  const derecha = Math.min(a.x + a.width, b.x + b.width);
  const abajo = Math.min(a.y + a.height, b.y + b.height);
  if (derecha <= x || abajo <= y) return null;
  return { x, y, width: derecha - x, height: abajo - y };
}

export function interseca(a: Rect, b: Rect): boolean {
  return interseccion(a, b) !== null;
}

/** La caja que abarca todos, o `null` si no hay ninguno. */
export function cajaDe(rects: Rect[]): Rect | null {
  if (rects.length === 0) return null;
  const x = Math.min(...rects.map((r) => r.x));
  const y = Math.min(...rects.map((r) => r.y));
  const derecha = Math.max(...rects.map((r) => r.x + r.width));
  const abajo = Math.max(...rects.map((r) => r.y + r.height));
  return { x, y, width: derecha - x, height: abajo - y };
}

export function mismoRect(a: Rect | null, b: Rect | null): boolean {
  if (!a || !b) return a === b;
  return a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;
}

export function mismoPunto(a: Punto | null, b: Punto | null): boolean {
  if (!a || !b) return a === b;
  return a.x === b.x && a.y === b.y;
}

/**
 * De CSS de esta pantalla a fisicos enteros del escritorio, redondeando igual que
 * `aVirtual` (que es lo que se manda a Rust) pero sin su minimo de dos pixeles: un recorte
 * que empieza mide cero, y hay que poder decirlo.
 */
export function aFisico(rect: Rect, monitor: Pick<MonitorInfo, "x" | "y">, escala: number): Rect {
  return {
    x: Math.round(monitor.x + rect.x * escala),
    y: Math.round(monitor.y + rect.y * escala),
    width: Math.round(rect.width * escala),
    height: Math.round(rect.height * escala),
  };
}

/**
 * Un punto del raton de CSS de esta pantalla a fisicos, con la escala de ESTA pantalla:
 * `monitor.x + clientX * escala`. No vale `screenX`, que Chromium da con la escala del
 * monitor de debajo, y con el portatil al 125 % no cuadraria con nada.
 */
export function puntoAFisico(p: Punto, monitor: Pick<MonitorInfo, "x" | "y">, escala: number): Punto {
  return { x: monitor.x + p.x * escala, y: monitor.y + p.y * escala };
}

export function puntoACss(p: Punto, monitor: Pick<MonitorInfo, "x" | "y">, escala: number): Punto {
  return { x: (p.x - monitor.x) / escala, y: (p.y - monitor.y) / escala };
}

/** Las pantallas de las que el recorte coge al menos un pixel. */
export function pantallasQueToca(rect: Rect, monitores: MonitorInfo[]): MonitorInfo[] {
  return monitores.filter((m) => interseca(rect, m));
}

/** La pantalla donde cae un punto, o `null` si cae en un hueco o fuera del escritorio. */
export function pantallaDelPunto(p: Punto, monitores: MonitorInfo[]): MonitorInfo | null {
  return (
    monitores.find(
      (m) => p.x >= m.x && p.y >= m.y && p.x < m.x + m.width && p.y < m.y + m.height,
    ) ?? null
  );
}

/**
 * La pantalla que manda sobre un recorte: la que se lleva mas trozo. Ella pinta la barra de
 * acciones, y ella lo recorta a lo suyo si se pasa a video o a una proporcion (un solo
 * escritor: si lo hicieran todas, cada una lo dejaria en su pantalla).
 *
 * Empate de area: la que tiene el centro (mirado como `esDeEstaPantalla`); y si sigue, la
 * de id menor, para que todas elijan la misma sin hablar entre ellas.
 */
export function pantallaQueManda(rect: Rect, monitores: MonitorInfo[]): MonitorInfo | null {
  let mejores: MonitorInfo[] = [];
  let mayor = 0;
  for (const m of monitores) {
    const i = interseccion(rect, m);
    const area = i ? i.width * i.height : 0;
    if (area <= 0) continue;
    if (area > mayor) {
      mayor = area;
      mejores = [m];
    } else if (area === mayor) {
      mejores.push(m);
    }
  }
  if (mejores.length === 0) return null;
  const centro = {
    x: rect.x + Math.floor(rect.width / 2),
    y: rect.y + Math.floor(rect.height / 2),
  };
  const conElCentro = pantallaDelPunto(centro, mejores);
  if (conElCentro) return conElCentro;
  return mejores.reduce((a, b) => (b.id < a.id ? b : a));
}

/**
 * La pantalla que ensenna la medida: la de la esquina de arriba a la izquierda, que es
 * donde se pone la etiqueta. Si esa esquina cae en un hueco entre pantallas, la que manda.
 */
export function pantallaDeLaMedida(rect: Rect, monitores: MonitorInfo[]): MonitorInfo | null {
  return pantallaDelPunto({ x: rect.x, y: rect.y }, monitores) ?? pantallaQueManda(rect, monitores);
}

/** Si el recorte o el raton estan en alguna pantalla que no es la propia. */
export function tocaOtraPantalla(
  rect: Rect | null,
  cursor: Punto | null,
  propia: number,
  monitores: MonitorInfo[],
): boolean {
  const otras = monitores.filter((m) => m.id !== propia);
  if (rect && otras.some((m) => interseca(rect, m))) return true;
  return cursor !== null && pantallaDelPunto(cursor, otras) !== null;
}

function entre(v: number, min: number, max: number): number {
  return Math.min(Math.max(v, min), max);
}

/** Un punto metido dentro de una caja, bordes incluidos. */
export function dentroDe(p: Punto, caja: Rect): Punto {
  return { x: entre(p.x, caja.x, caja.x + caja.width), y: entre(p.y, caja.y, caja.y + caja.height) };
}

/**
 * El iman del borde (ver `IMAN`), eje por eje y en fisicos: si el raton se ha salido de la
 * caja de las pantallas donde empezo el gesto por menos de `umbral`, ese eje se queda en el
 * borde; si se ha salido mas, cruza. Vale igual para un hueco entre pantallas que para otra
 * pantalla, porque lo que importa es no llevarse una tira de lo de fuera sin querer.
 */
export function conIman(p: Punto, partida: Rect, umbral = IMAN): Punto {
  const eje = (v: number, desde: number, largo: number) => {
    const hasta = desde + largo;
    const fuera = v < desde ? desde - v : v > hasta ? v - hasta : 0;
    return fuera > 0 && fuera < umbral ? entre(v, desde, hasta) : v;
  };
  return { x: eje(p.x, partida.x, partida.width), y: eje(p.y, partida.y, partida.height) };
}

/**
 * Donde se queda el raton de un gesto que puede cruzar: con su iman, y nunca fuera de la
 * caja del escritorio (lo que no es de ninguna pantalla no se puede capturar).
 */
export function cursorDelGesto(
  p: Punto,
  partida: Rect,
  escritorio: Rect,
  umbral = IMAN,
): Punto {
  return dentroDe(conIman(p, partida, umbral), escritorio);
}

/**
 * El iman al mover el recorte entero: si al moverlo asoma menos de `umbral` a una pantalla
 * que no tocaba al empezar, se aparta lo justo para no asomar. Devuelve el MISMO objeto si
 * no hay que apartarlo, para que quien llama sepa que no ha cambiado nada.
 */
export function apartarDeOtras(
  rect: Rect,
  partida: number[],
  monitores: MonitorInfo[],
  umbral = IMAN,
): Rect {
  let r = rect;
  for (const m of monitores) {
    if (partida.includes(m.id)) continue;
    const i = interseccion(r, m);
    if (!i) continue;
    const opciones: Punto[] = [];
    // Solo se puede apartar por un eje si el borde de la otra pantalla cruza el recorte.
    if (r.x < m.x && r.x + r.width > m.x) opciones.push({ x: -i.width, y: 0 });
    if (r.x < m.x + m.width && r.x + r.width > m.x + m.width) opciones.push({ x: i.width, y: 0 });
    if (r.y < m.y && r.y + r.height > m.y) opciones.push({ x: 0, y: -i.height });
    if (r.y < m.y + m.height && r.y + r.height > m.y + m.height) opciones.push({ x: 0, y: i.height });
    const corta = opciones
      .filter((o) => Math.abs(o.x + o.y) < umbral)
      .sort((a, b) => Math.abs(a.x + a.y) - Math.abs(b.x + b.y))[0];
    if (corta) r = { ...r, x: r.x + corta.x, y: r.y + corta.y };
  }
  return r;
}

/**
 * Lo que se graba, de una sola pantalla: el video y el GIF no cruzan en esta version (cada
 * sesion de grabacion de Windows es de un monitor). Si el recorte cruza, se queda con el
 * trozo de la pantalla que manda. Es la red de abajo: lo normal es que la que manda ya lo
 * haya recortado al pasar a video.
 */
export function enUnaPantalla(rect: Rect, monitores: MonitorInfo[]): Rect {
  if (pantallasQueToca(rect, monitores).length <= 1) return rect;
  const manda = pantallaQueManda(rect, monitores);
  return (manda && interseccion(rect, manda)) || rect;
}

/** Lo que este overlay sabe que saben los demas, para no repetirse. */
export interface Aviso {
  rect: Rect | null;
  fase: Fase;
  cursor: Punto | null;
  /** Si lo ultimo que se mando tocaba otra pantalla. */
  tocaba: boolean;
}

export const SIN_AVISO: Aviso = { rect: null, fase: "quieta", cursor: null, tocaba: false };

/**
 * Si hay que avisar a las demas pantallas, y cuando.
 *
 * - `ya`: al empezar un gesto, al acabarlo y con cualquier cambio sin gesto (el teclado,
 *   Ctrl+A, la R, recortar al pasar a video). Son pocos y no pueden perderse.
 * - `luego`: en medio de un gesto, como mucho uno por cuadro, y solo si el recorte o el
 *   raton tocan otra pantalla (o la tocaban en el ultimo aviso, para que se entere de que ya
 *   no). Dentro de una sola pantalla un gesto entero son dos avisos.
 * - `nada`: lo demas, y lo que ya se sabe (el eco de lo que acaba de llegar de otra).
 */
export function queAvisar(
  ahora: Omit<Aviso, "tocaba">,
  antes: Aviso,
  tocaAhora: boolean,
): "ya" | "luego" | "nada" {
  if (ahora.fase !== antes.fase) return "ya";
  if (ahora.fase === "quieta") return mismoRect(ahora.rect, antes.rect) ? "nada" : "ya";
  if (mismoRect(ahora.rect, antes.rect) && mismoPunto(ahora.cursor, antes.cursor)) return "nada";
  return tocaAhora || antes.tocaba ? "luego" : "nada";
}
