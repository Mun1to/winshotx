import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { emit, listen } from "@tauri-apps/api/event";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import {
  cancelCapture,
  captureAllScreens,
  captureStill,
  copyColor,
  cronoMarca,
  freezeBytes,
  freezePng,
  openSettings,
  overlayBootstrap,
  overlayListo,
  scrollIniciar,
  setCaptureFlow,
  startRecording,
} from "../../lib/ipc";
import { EVENTO_SCROLL_MODO } from "../../lib/scroll";
import {
  EVENTO_PROPORCION,
  asaConProporcion,
  cambiarProporcion,
  crecerConProporcion,
  recorteConProporcion,
  siguienteProporcion,
  tieneLaProporcion,
  unidadDe,
  type Proporcion,
} from "../../lib/proporcion";
import { clamp } from "../../lib/format";
import { useT } from "../../lib/i18n";
import {
  aPantalla,
  aVirtual,
  esDeEstaPantalla,
  ventanaBajoElPunto,
  ventanasDeEstaPantalla,
  type VentanaDelSistema,
} from "../../lib/pantallas";
import {
  aFisico,
  apartarDeOtras,
  cajaDe,
  cursorDelGesto,
  dentroDe,
  enUnaPantalla,
  interseca,
  interseccion,
  monitoresDe,
  pantallaDeLaMedida,
  pantallaQueManda,
  pantallasQueToca,
  puntoACss,
  puntoAFisico,
  type Fase,
  type Punto,
} from "../../lib/seleccionCompartida";
import {
  EVENTS,
  type CaptureMode,
  type OverlayModeState,
  type OverlayPayload,
  type Rect,
  type StillAction,
} from "../../lib/types";
import { BootScreen } from "./BootScreen";
import { DimensionBadge } from "./DimensionBadge";
import { FloatingToolbar } from "./FloatingToolbar";
import { Magnifier } from "./Magnifier";
import { ModeBar, SELECTOR_BARRA } from "./ModeBar";
import { ScreenPicker } from "./ScreenPicker";
import { SelectionHandles, type HandleId } from "./SelectionHandles";
import { useSeleccionCompartida } from "./useSeleccionCompartida";

type Mode =
  | { kind: "idle" }
  | { kind: "drawing"; originX: number; originY: number; candidate: Rect | null }
  | { kind: "moving"; grabX: number; grabY: number; base: Rect }
  | { kind: "resizing"; handle: HandleId; base: Rect };

const MIN_DRAG = 4; // por debajo de esto, un arrastre cuenta como clic

/** Lo que se cuenta a las demas pantallas de cada gesto (`lib/seleccionCompartida.ts`). */
const FASE_DEL_GESTO: Record<Mode["kind"], Fase> = {
  idle: "quieta",
  drawing: "dibujando",
  moving: "moviendo",
  resizing: "estirando",
};

/** Lo que se ve de esta pantalla, en sus CSS. */
function vistaDeLaVentana(): Rect {
  return { x: 0, y: 0, width: window.innerWidth, height: window.innerHeight };
}

/** Lo que ocupa la lupa con su barra de abajo, y el aire que se le deja al cursor. */
const LUPA_ANCHO = 148;
const LUPA_ALTO = 192;
const LUPA_AIRE = 18;

/** Lo que ocupa la barra de acciones de alto, y el aire que se le deja al recorte. */
const BARRA_ALTO = 52;
const BARRA_AIRE = 10;

/**
 * Donde se pone la barra de acciones, y si va volteada.
 *
 * Iba debajo del recorte, y si ahi no cabia, encima. Cuando el recorte es la pantalla
 * entera no cabe en ninguno de los dos sitios, y la cuenta la dejaba en `top: -10`: la
 * barra existia, con sus botones, y no se veia **ni un pixel** de ella. Ahora, cuando no
 * cabe fuera, se mete DENTRO del recorte pegada a su borde de abajo, que es lo que hacen
 * las demas herramientas de captura.
 *
 * Con `volteada`, el `top` que se devuelve es el borde de ABAJO de la barra, porque el
 * componente se sube a si mismo con `-translate-y-full`.
 */
function sitioDeLaBarra(sel: Rect, alto: number) {
  if (sel.y + sel.height + BARRA_AIRE + BARRA_ALTO <= alto) {
    return { top: sel.y + sel.height + BARRA_AIRE, volteada: false };
  }
  if (sel.y - BARRA_AIRE - BARRA_ALTO >= 0) {
    return { top: sel.y - BARRA_AIRE, volteada: true };
  }
  return { top: Math.min(alto, sel.y + sel.height) - BARRA_AIRE, volteada: true };
}

/**
 * Donde se pone la lupa para que NO tape el pixel que se esta mirando.
 *
 * Iba siempre abajo y a la derecha del cursor, y cerca de un borde solo se recortaba para
 * no salirse de la pantalla. En la esquina de abajo a la derecha eso la dejaba **encima
 * del propio cursor**: se capturaba a ciegas justo donde mas falta hace ver, que es lo que
 * conto Munir el 9 de septiembre de 2026 («cuando capturas una esquina no ves lo que estas
 * capturando»). Ahora se cambia de lado, como ya hacia la barra flotante: si no cabe a la
 * derecha va a la izquierda, y si no cabe abajo va arriba.
 */
function sitioDeLaLupa(x: number, y: number, ancho: number, alto: number) {
  const cabeDerecha = x + LUPA_AIRE + LUPA_ANCHO <= ancho;
  const cabeAbajo = y + LUPA_AIRE + LUPA_ALTO <= alto;
  return {
    left: clamp(
      cabeDerecha ? x + LUPA_AIRE : x - LUPA_AIRE - LUPA_ANCHO,
      0,
      Math.max(0, ancho - LUPA_ANCHO),
    ),
    top: clamp(
      cabeAbajo ? y + LUPA_AIRE : y - LUPA_AIRE - LUPA_ALTO,
      0,
      Math.max(0, alto - LUPA_ALTO),
    ),
  };
}

function normalize(ax: number, ay: number, bx: number, by: number): Rect {
  return {
    x: Math.min(ax, bx),
    y: Math.min(ay, by),
    width: Math.abs(bx - ax),
    height: Math.abs(by - ay),
  };
}

function applyHandle(base: Rect, handle: HandleId, x: number, y: number): Rect {
  let left = base.x;
  let top = base.y;
  let right = base.x + base.width;
  let bottom = base.y + base.height;
  if (handle.includes("w")) left = x;
  if (handle.includes("e")) right = x;
  if (handle.startsWith("n")) top = y;
  if (handle.startsWith("s")) bottom = y;
  return normalize(left, top, right, bottom);
}

/** Si un gesto de puntero nacio dentro de la barra de arriba. */
function enLaBarra(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(SELECTOR_BARRA) !== null;
}

function contains(rect: Rect, x: number, y: number): boolean {
  return x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height;
}

/**
 * El fondo del overlay tapa la pantalla entera: si no se pinta, el usuario se
 * queda con un rectangulo negro encima de todo. Por eso hay dos vias.
 *
 * Las dos van por el IPC y salen de la memoria de Rust: ya no hay ningun archivo. La
 * normal trae la pantalla en PNG (dos o tres megabytes); llevarla en crudo, 8 MB por
 * pantalla, era el trozo mas gordo de todo el camino del atajo: 300-400 ms con tres
 * pantallas pidiendola a la vez. La de respaldo la trae sin comprimir, en BMP, por si el
 * PNG fallara.
 */
async function loadFreeze(monitorId: number): Promise<Blob> {
  try {
    const bytes = await freezePng(monitorId);
    if (!bytes || bytes.byteLength === 0) throw new Error("el PNG ha llegado vacio");
    return new Blob([bytes], { type: "image/png" });
  } catch (pngError) {
    console.warn("el PNG del congelado ha fallado, se pide sin comprimir", pngError);
    const bytes = await freezeBytes(monitorId);
    return new Blob([bytes], { type: "image/bmp" });
  }
}

export function SelectionCanvas({ monitorId }: { monitorId: number }) {
  const t = useT();
  const [payload, setPayload] = useState<OverlayPayload | null>(null);
  const [selection, setSelection] = useState<Rect | null>(null);
  const [mode, setMode] = useState<Mode>({ kind: "idle" });
  const modeRef = useRef<Mode>(mode);
  modeRef.current = mode;
  /**
   * La captura ya se ha cerrado (Escape, o Rust al aparcar los overlays) y lo que llegue del
   * ratón hasta la próxima vez que se enseñe no hace nada.
   *
   * El overlay no se esconde al cerrar: se aparca fuera de las pantallas, y Chromium le sigue
   * mandando el ratón del arrastre que estaba en marcha. Con Escape a medio arrastre, al
   * soltar el botón se copiaba el recorte o empezaba a grabar (Munito, 16-nov: «haz que
   * cuando le des al Esc no se capture nada»). Se limpia en `boot`, al enseñarse otra vez.
   */
  const cerrada = useRef(false);
  const soltarElGesto = useCallback(() => {
    cerrada.current = true;
    setMode({ kind: "idle" });
  }, []);
  /** Si la pulsación que está llegando ya trajo su `pointerdown`: ver `onMouseDownCapture`. */
  const pointerdownVisto = useRef(false);
  const [cursor, setCursor] = useState({ x: 0, y: 0 });
  const cursorRef = useRef(cursor);
  cursorRef.current = cursor;
  /**
   * Si el raton esta en ESTA pantalla. Hay un overlay por monitor y todos pintaban la lupa,
   * la cruceta, el resalte y la barra de modos: en el que no tenia el raton salian donde
   * se quedo el cursor (o en la esquina, en (0, 0)). Solo los pinta el que lo tiene; el
   * resto se queda con el velo.
   */
  const [cursorAqui, setCursorAqui] = useState(false);
  const [hex, setHex] = useState("#000000");
  // El manejador de teclas se registra una vez, asi que lee el color por referencia: con
  // el valor del cierre copiaria siempre el negro con el que arranca.
  const hexRef = useRef(hex);
  hexRef.current = hex;
  /** El ultimo color copiado, para poder decirlo. Se borra solo. */
  const [colorCopiado, setColorCopiado] = useState<string | null>(null);
  useEffect(() => {
    if (!colorCopiado) return;
    const id = setTimeout(() => setColorCopiado(null), 1600);
    return () => clearTimeout(id);
  }, [colorCopiado]);
  /** Qué se hará con el recorte. Lo elige la barra de arriba, antes de recortar. */
  const [modo, setModo] = useState<CaptureMode>("still");
  /** Coger la pantalla entera de un clic, sin arrastrar. */
  const [pantallaEntera, setPantallaEntera] = useState(false);
  /**
   * Si al soltar el recorte sale la barra para elegir que hacer con el.
   *
   * Es el ajuste `captureFlow` puesto donde se usa. Nace de los ajustes guardados y, al
   * tocarlo aqui, se guarda alli: quien lo apaga una vez lo tiene apagado mannana, sin
   * tener que acordarse de ir a la ventana de ajustes a cambiarlo de vuelta.
   */
  const [conBarra, setConBarra] = useState(true);
  const modoRef = useRef<CaptureMode>("still");
  modoRef.current = modo;
  /** Hay una grabación en marcha: este overlay solo hace fotos. */
  const grabandoRef = useRef(false);
  const pantallaRef = useRef(false);
  pantallaRef.current = pantallaEntera;
  const conBarraRef = useRef(true);
  conBarraRef.current = conBarra;

  /**
   * Cambia el estado de la barra en TODAS las pantallas.
   *
   * Hay un overlay por monitor y cada uno tiene su propio React, asi que tocar el boton
   * aqui solo cambiaba esta pantalla. El evento vuelve tambien a quien lo manda, de modo
   * que aplicar el cambio es cosa del listener y no hay dos caminos que mantener.
   */
  const difundir = useCallback((cambio: Partial<OverlayModeState>) => {
    void emit(EVENTS.overlayMode, {
      mode: cambio.mode ?? modoRef.current,
      fullScreen: cambio.fullScreen ?? pantallaRef.current,
      withToolbar: cambio.withToolbar ?? conBarraRef.current,
    } satisfies OverlayModeState);
  }, []);

  /**
   * El interruptor de la barra de acciones: se enciende en todas las pantallas y se
   * guarda en los ajustes.
   *
   * Se guarda con `setCaptureFlow` y no con `setSettings`: aquel reengancha los tres
   * atajos globales y mira si hay que reiniciar el anillo cada vez que se llama, y esto
   * se pulsa con la captura abierta delante.
   */
  const cambiarBarra = useCallback(
    (valor: boolean) => {
      difundir({ withToolbar: valor });
      void setCaptureFlow(valor ? "toolbar" : "instant").catch((e) => {
        // Que no se haya podido guardar no deshace lo que se acaba de encender: la
        // captura de ahora sigue el interruptor igual, y lo unico que se pierde es que
        // la proxima vez vuelva a nacer asi.
        console.warn("no se ha podido guardar el perfil de captura", e);
      });
    },
    [difundir],
  );
  /**
   * La captura con scroll: al soltar el recorte no se hace una foto, se baja la página y
   * se cose (`capture::scroll`). Se enciende en todas las pantallas a la vez, como lo
   * demás de la barra de arriba, y se apaga sola en cada captura nueva: quien la usa una
   * vez no quiere encontrársela puesta en la siguiente foto de todos los días.
   */
  const [conScroll, setConScroll] = useState(false);
  const conScrollRef = useRef(false);
  conScrollRef.current = conScroll;
  const cambiarScroll = useCallback(
    (valor: boolean) => {
      // Aquí mismo y a las demás pantallas: el evento vuelve también a esta, y poner
      // dos veces el mismo valor no hace nada.
      setConScroll(valor);
      if (valor) setPantallaEntera(false);
      void emit(EVENTO_SCROLL_MODO, valor);
      if (valor) difundir({ mode: "still", fullScreen: false });
    },
    [difundir],
  );
  /**
   * La proporción fija del recorte (`lib/proporcion.ts`): se cambia con la X, también a
   * mitad de arrastrar, o con su botón de la barra de arriba. Como el scroll, vuelve a
   * «libre» en cada captura nueva: es de este recorte, no un ajuste, y encontrársela puesta
   * días después haría pensar que el recorte va mal.
   */
  const [proporcion, setProporcion] = useState<Proporcion>("libre");
  const proporcionRef = useRef<Proporcion>("libre");
  proporcionRef.current = proporcion;
  /** En todas las pantallas a la vez; el evento vuelve también a esta. */
  const elegirProporcion = useCallback((valor: Proporcion) => {
    setProporcion(valor);
    void emit(EVENTO_PROPORCION, valor);
  }, []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);
  const [source, setSource] = useState<HTMLCanvasElement | null>(null);
  const selectionRef = useRef<Rect | null>(null);
  selectionRef.current = selection;
  /** El numero de ESTA pantalla, para saber si una orden por numero va con nosotros. */
  const numeroPantalla = useRef(0);

  /**
   * Donde se apreto el boton encima de la barra de arriba, mientras no se sepa que es.
   *
   * La barra ocupa una franja del centro de arriba y ahi no se podia empezar a recortar:
   * se quedaba ella el `pointerdown`. Ahora el gesto se decide al moverse, que es la
   * unica forma de que las dos cosas quepan en el mismo sitio: si el puntero se va, se
   * recorta por debajo de la barra; si se suelta sin moverse, el clic es del boton.
   */
  const origenBarra = useRef<{ x: number; y: number } | null>(null);

  /**
   * Lo que mide la ventana, al dia. En Windows el overlay ya tiene su tamanno cuando llega la
   * captura (espera aparcado), pero en Linux con Wayland se crea escondido y no sabe lo que
   * mide hasta que el escritorio lo pone a pantalla completa: con la medida de antes, la
   * primera captura salia con escala 1600 y un recorte de «558400 × 832000».
   */
  const [anchoVentana, setAnchoVentana] = useState(() => window.innerWidth);
  useEffect(() => {
    const medir = () => setAnchoVentana(window.innerWidth);
    window.addEventListener("resize", medir);
    medir();
    return () => window.removeEventListener("resize", medir);
  }, []);

  /** Pixeles fisicos por pixel CSS: el freeze manda, el webview puede estar escalado por DPI. */
  const scale = useMemo(() => {
    if (!source) return 1;
    return source.width / Math.max(1, anchoVentana);
  }, [source, anchoVentana]);

  /**
   * Lo que hace falta para las cuentas de la proporción: su escalón (el doble grabando, para
   * que los lados salgan pares) y la pantalla en píxeles físicos. Null si va libre.
   */
  const geometria = useMemo(() => {
    // La captura con scroll sale tan larga como la página: ahí no hay proporción que
    // guardar, y su botón ni se ve.
    const unidad = conScroll ? null : unidadDe(proporcion, modo !== "still");
    if (!unidad || !source) return null;
    return { unidad, limite: { ancho: source.width, alto: source.height }, escala: scale };
  }, [proporcion, modo, conScroll, source, scale]);
  const geometriaRef = useRef(geometria);
  geometriaRef.current = geometria;

  /**
   * Todas las pantallas de esta captura (la propia con lo que dice su payload), y la caja
   * del escritorio entero. Un payload viejo sin la lista es una sola pantalla.
   */
  const monitores = useMemo(
    () => (payload ? monitoresDe(payload.monitor, payload.monitores) : []),
    [payload],
  );
  const variasPantallas = monitores.length > 1;
  const escritorio = useMemo(() => cajaDe(monitores), [monitores]);

  /**
   * Si el recorte puede seguir en la pantalla de al lado. Solo en foto libre: el video y el
   * GIF graban un monitor por sesion, la proporcion se cuenta contra una pantalla, el scroll
   * baja una ventana y la pantalla entera es una. Sin cruzar, el raton se queda dentro de
   * esta ventana, que de paso evita grabar un trozo dibujado mas alla del borde.
   */
  const puedeCruzar =
    variasPantallas && modo === "still" && !conScroll && proporcion === "libre" && !pantallaEntera;

  /**
   * Las pantallas que tocaba el recorte al empezar el gesto, en fisicos: el iman del borde
   * cuenta desde ellas y mover no se sale de su caja (`lib/seleccionCompartida.ts`).
   */
  const partidaRef = useRef<{ caja: Rect; ids: number[] } | null>(null);
  const empezarGesto = useCallback(
    (base: Rect | null) => {
      if (!payload) return;
      const yo = payload.monitor;
      const tocadas =
        base && base.width > 0 && base.height > 0
          ? pantallasQueToca(aFisico(base, yo, scale), monitores)
          : [];
      const partida = tocadas.length > 0 ? tocadas : [yo];
      partidaRef.current = { caja: cajaDe(partida)!, ids: partida.map((m) => m.id) };
    },
    [payload, scale, monitores],
  );

  /**
   * Donde se queda el raton de un gesto, en CSS de esta pantalla: dentro de la ventana si
   * no se puede cruzar; y si se puede, con el iman del borde y dentro del escritorio.
   */
  const limitarCursor = useCallback(
    (x: number, y: number): Punto => {
      if (!puedeCruzar || !payload) {
        return { x: clamp(x, 0, window.innerWidth), y: clamp(y, 0, window.innerHeight) };
      }
      const yo = payload.monitor;
      const p = cursorDelGesto(
        puntoAFisico({ x, y }, yo, scale),
        partidaRef.current?.caja ?? yo,
        escritorio ?? yo,
      );
      return puntoACss(p, yo, scale);
    },
    [puedeCruzar, payload, scale, escritorio],
  );

  /** El recorte que se dibuja de un punto a otro, libre o con su proporción. */
  const dibujar = useCallback(
    (ax: number, ay: number, bx: number, by: number): Rect => {
      if (!geometria) {
        const b = limitarCursor(bx, by);
        return normalize(ax, ay, b.x, b.y);
      }
      const { unidad, limite, escala } = geometria;
      return recorteConProporcion({ x: ax, y: ay }, { x: bx, y: by }, unidad, limite, escala);
    },
    [geometria, limitarCursor],
  );

  /**
   * Mover el recorte entero sin salirse: de la ventana si no se puede cruzar, y si se puede,
   * de la caja de las pantallas que tocaba al empezar, con el iman si asoma a otra.
   */
  const moverA = useCallback(
    (x: number, y: number, base: Rect): Rect => {
      const partida = partidaRef.current;
      if (!puedeCruzar || !payload || !partida) {
        return {
          x: clamp(x, 0, window.innerWidth - base.width),
          y: clamp(y, 0, window.innerHeight - base.height),
          width: base.width,
          height: base.height,
        };
      }
      const yo = payload.monitor;
      const caja = aPantalla(partida.caja, yo, scale);
      const movido = {
        x: clamp(x, caja.x, caja.x + caja.width - base.width),
        y: clamp(y, caja.y, caja.y + caja.height - base.height),
        width: base.width,
        height: base.height,
      };
      const fisico = aFisico(movido, yo, scale);
      const apartado = apartarDeOtras(fisico, partida.ids, monitores);
      return apartado === fisico ? movido : aPantalla(apartado, yo, scale);
    },
    [puedeCruzar, payload, scale, monitores],
  );

  /**
   * Al cambiar la proporción (o de foto a grabar, que cambia el escalón), el recorte que ya
   * hay se pone a la nueva en ese mismo momento, sin esperar a que se mueva el ratón: a
   * mitad de arrastrar, desde donde se empezó hasta el cursor; tirando de un asa, igual que
   * si se acabara de mover; y quieto, en el mismo sitio y de tamaño parecido.
   */
  //
  // Con varias pantallas hay ademas otra cosa que hacer a la vez: un recorte que cruza y
  // pasa a video, a GIF, a una proporcion o a scroll se queda en el trozo de una sola
  // pantalla. Y lo hace UNA (plano dos pantallas, 2.5): la del gesto si hay uno en marcha,
  // y si no, la que manda; las demas se enteran por `EVENTO_SELECCION`. Si lo hicieran
  // todas, cada una lo dejaria en la suya. Va en el mismo efecto que la proporcion porque
  // las dos cosas cambian el mismo recorte, y dos efectos leerian los dos el de antes.
  const reglasAntes = useRef({ clave: "", cruza: false });
  /** Si este overlay es el que escribe el recorte cuando no hay gesto aqui. Ver abajo. */
  const escritorRef = useRef(true);
  useEffect(() => {
    const clave = geometria ? `${proporcion}|${modo !== "still"}` : "";
    const antes = reglasAntes.current;
    reglasAntes.current = { clave, cruza: puedeCruzar };
    const nuevaGeometria = geometria !== null && clave !== antes.clave;
    const dejaDeCruzar = antes.cruza && !puedeCruzar;
    if (!nuevaGeometria && !dejaDeCruzar) return;
    const m = modeRef.current;
    if (m.kind === "idle" && !escritorRef.current) return;
    const vista = vistaDeLaVentana();
    /** Lo que cruza, en el trozo de esta pantalla; lo que no, tal cual. */
    const aSuTrozo = (r: Rect): Rect => (dejaDeCruzar ? (interseccion(r, vista) ?? r) : r);
    const c = cursorRef.current;
    if (m.kind === "drawing") {
      setSelection(dibujar(m.originX, m.originY, c.x, c.y));
      return;
    }
    if (m.kind === "resizing") {
      const base = aSuTrozo(m.base);
      if (base !== m.base) setMode({ ...m, base });
      if (geometria) {
        const { unidad, limite, escala } = geometria;
        setSelection(asaConProporcion(base, m.handle, c, unidad, limite, escala));
      } else {
        const b = limitarCursor(c.x, c.y);
        setSelection(applyHandle(base, m.handle, b.x, b.y));
      }
      return;
    }
    const actual = selectionRef.current;
    if (!actual) return;
    let nuevo = aSuTrozo(actual);
    if (nuevaGeometria && geometria) {
      if (nuevo.width < MIN_DRAG || nuevo.height < MIN_DRAG) return;
      const { unidad, limite, escala } = geometria;
      nuevo = cambiarProporcion(nuevo, unidad, limite, escala);
    }
    if (nuevo === actual) return;
    setSelection(nuevo);
    // Arrastrándolo, el tamaño sale de `base`: sin esto, el siguiente movimiento le
    // devolvería el de antes.
    if (m.kind === "moving") setMode({ ...m, base: nuevo });
  }, [geometria, proporcion, modo, puedeCruzar, dibujar, limitarCursor]);

  /** Si el fondo ya esta dibujado en `source`. Hasta entonces, la pantalla de arranque. */
  const [fondoListo, setFondoListo] = useState(false);
  /** Donde se cuelga el lienzo con la pantalla congelada, que es el fondo que se ve. */
  const fondoRef = useRef<HTMLDivElement>(null);

  /** El raton del gesto de aqui, en fisicos y dentro del escritorio, para la lupa de al lado. */
  const cursorFis = useMemo(() => {
    if (!payload || mode.kind === "idle") return null;
    const p = puntoAFisico(cursor, payload.monitor, scale);
    const dentro = escritorio ? dentroDe(p, escritorio) : p;
    return { x: Math.round(dentro.x), y: Math.round(dentro.y) };
  }, [payload, mode.kind, cursor, scale, escritorio]);

  /**
   * El recorte es uno para todas las pantallas: lo de aqui se cuenta a las demas, y lo que
   * hacen ellas llega aqui. `faseRemota` dice si hay un gesto en marcha en otra pantalla.
   */
  const { faseRemota, cursorRemoto } = useSeleccionCompartida({
    payload,
    monitores,
    escala: scale,
    listo: fondoListo && source !== null,
    selection,
    fase: FASE_DEL_GESTO[mode.kind],
    cursorFis,
    ponerSeleccion: setSelection,
  });

  /**
   * Si el recorte asoma por ESTA pantalla. Con varias, cada una guarda el recorte entero,
   * tambien cuando esta en otra: para lo que se pinta aqui cuenta el trozo que se ve.
   */
  const seleccionAqui =
    selection !== null && (!variasPantallas || interseca(selection, vistaDeLaVentana()));

  /** Ventanas del sistema recortadas a este monitor, ya en coordenadas CSS locales. */
  const snapTargets = useMemo(
    () =>
      payload
        ? ventanasDeEstaPantalla(
            payload.windows,
            payload.monitor,
            scale,
            window.innerWidth,
            window.innerHeight,
          )
        : [],
    [payload, scale],
  );

  /** La de delante bajo el punto, en orden Z (ver `ventanaBajoElPunto`). */
  const windowAt = useCallback(
    (x: number, y: number) => ventanaBajoElPunto(snapTargets, x, y),
    [snapTargets],
  );

  /** La ventana resaltada, en una referencia: la lee el oyente del teclado. */
  const hoveredRef = useRef<VentanaDelSistema | null>(null);

  const hovered = useMemo(() => {
    if (!cursorAqui || seleccionAqui || mode.kind !== "idle" || pantallaEntera) return null;
    return windowAt(cursor.x, cursor.y);
  }, [windowAt, cursor, cursorAqui, seleccionAqui, mode.kind, pantallaEntera]);
  hoveredRef.current = hovered;

  /**
   * La ventana overlay se reutiliza entre capturas (ver `windows_mgr::open_overlays`): no
   * se desmonta ni se remonta, asi que este arranque no puede depender solo del montaje
   * inicial. `bootIdRef` distingue la invocacion vigente de una anterior que aun estuviera
   * a medias, para que una llegada tardia de la vieja no pise el estado de la nueva.
   */
  const bootIdRef = useRef(0);

  const boot = useCallback(async (payloadListo?: OverlayPayload) => {
    const miId = ++bootIdRef.current;
    const vigente = () => bootIdRef.current === miId;
    const quien = payloadListo?.monitor.id ?? monitorId;
    void cronoMarca(`js-evento-${quien}`);

    // Se limpia lo de la vez anterior ANTES de pedir nada: si esta ventana se reutiliza,
    // sin esto se veria un instante la captura vieja antes de que llegue la nueva.
    setPayload(null);
    setFondoListo(false);
    setSource(null);
    setSelection(null);
    setMode({ kind: "idle" });
    cerrada.current = false;
    pointerdownVisto.current = false;
    setPantallaEntera(false);
    setConScroll(false);
    setProporcion("libre");
    setBusy(false);
    // `conBarra` NO se reinicia aqui: lo pone el payload unas lineas mas abajo con lo que
    // digan los ajustes, y ponerlo a un valor de fabrica antes seria ensennar la barra
    // encendida un instante a quien la tiene apagada.
    setError(null);
    setBootError(null);
    setCursorAqui(false);

    try {
      // Cuando se reutiliza una ventana, el backend manda el payload ya hecho en el
      // propio evento (ver EVENTS.overlayShow): pedirlo aparte por invoke seria una
      // vuelta de IPC completa que no hace falta. Solo en el primer montaje, cuando
      // nadie nos lo ha dado, se pide.
      const data = payloadListo ?? (await overlayBootstrap(monitorId));
      if (!vigente()) return;
      setPayload(data);
      numeroPantalla.current = data.screenNumber;
      // Donde esta el raton al abrir: si se queda quieto no llega ningun `pointermove`, y
      // sin esto ninguna pantalla sabria que lo tiene hasta que se moviera.
      void cursorPosition()
        .then((p) => {
          const m = data.monitor;
          const fuera =
            p.x < m.x || p.y < m.y || p.x >= m.x + m.width || p.y >= m.y + m.height;
          if (!vigente() || fuera) return;
          const escala = window.devicePixelRatio || 1;
          setCursor({ x: (p.x - m.x) / escala, y: (p.y - m.y) / escala });
          setCursorAqui(true);
        })
        .catch(() => {});
      // Quien pulsa el atajo de grabar quiere grabar: la barra abre en vídeo y ya.
      grabandoRef.current = data.recording ?? false;
      setModo(data.intent === "record" && !data.recording ? "video" : "still");
      setConBarra(data.settings.captureFlow === "toolbar");
      void getCurrentWindow().setFocus();

      // Aqui va el id de `data.monitor.id` (el de ESTA captura, siempre correcto),
      // NUNCA el prop `monitorId` de la URL: esta ventana se reutiliza entre capturas
      // (ver windows_mgr::open_overlays) y ese prop se fijo la primera vez que se creo,
      // que puede no coincidir con el id actual si el orden de los monitores del sistema
      // cambio entre medias. Pedir el id equivocado es ensennar el freeze de otro monitor.
      const blob = await loadFreeze(data.monitor.id);
      if (!vigente()) return;
      void cronoMarca(`js-bytes-${quien}`);

      // Un solo lienzo hace de fondo y de fuente para la lupa: decodificar una vez, no dos
      // (antes el fondo era un <img> con su propia decodificacion ademas de esta).
      const bitmap = await createImageBitmap(blob);
      if (!vigente()) return;
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      canvas.className = "pointer-events-none absolute inset-0 h-full w-full";
      canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
      bitmap.close?.();
      setSource(canvas);
      setFondoListo(true);
      void cronoMarca(`js-canvas-${quien}`);
    } catch (e) {
      if (vigente()) setBootError(String(e));
    }
  }, [monitorId]);

  useEffect(() => {
    void boot();
  }, [boot]);

  // El lienzo con la pantalla congelada se cuelga en el DOM a mano: React no lo redibuja,
  // y asi el mismo elemento que lee la lupa es el que se ve.
  useLayoutEffect(() => {
    const hueco = fondoRef.current;
    if (!hueco || !source || !fondoListo) return;
    hueco.replaceChildren(source);
    return () => {
      hueco.replaceChildren();
    };
  }, [source, fondoListo]);

  /**
   * Con el fondo colgado, se le dice a Rust que ya puede ensennar esta ventana: hasta
   * entonces esta aparcada fuera de las pantallas y nadie ve la pantalla de carga.
   *
   * Se espera al siguiente cuadro (`requestAnimationFrame`), que es el que lleva el lienzo
   * recien colgado: para cuando Rust reciba el aviso y mueva la ventana, ese cuadro ya
   * esta pintado. Con dos cuadros se esperaba de mas (medido: 16 ms). Y un navegador
   * aparcado fuera de las pantallas puede no pintar cuadros: si en 30 ms no ha llegado
   * ninguno, se avisa igual.
   */
  useEffect(() => {
    if (!fondoListo || !payload) return;
    const { id } = payload.monitor;
    const generacion = payload.generation;
    let avisado = false;
    const avisar = () => {
      if (avisado) return;
      avisado = true;
      void cronoMarca(`js-pintado-${id}`);
      void overlayListo(id, generacion);
    };
    const uno = requestAnimationFrame(avisar);
    const plazo = window.setTimeout(avisar, 30);
    return () => {
      cancelAnimationFrame(uno);
      window.clearTimeout(plazo);
    };
  }, [fondoListo, payload]);

  // La ventana ya estaba montada de una captura anterior: no llega un remontaje que
  // dispare el arranque solo, asi que el backend lo pide explicitamente por evento, con
  // el payload de la captura nueva ya dentro.
  //
  // `target` NO es opcional aqui, aunque el tipo lo deje pasar. Sin el, `listen` se
  // registra como `{ kind: 'Any' }` y eso significa recibir TODO lo que se emita, incluso
  // lo que iba dirigido a otra ventana (@tauri-apps/api/event.d.ts: "defaults to
  // { kind: 'Any' }"). Con un overlay por monitor, cada ventana recibia tambien los
  // payloads de las otras dos y se quedaba con el ultimo del bucle de `open_overlays`:
  // las tres acababan pintando y recortando la misma pantalla. Arreglarlo en Rust con
  // `emit_to` era necesario y no bastaba, porque el que se apuntaba a todo era este lado.
  // Se pasa la etiqueta como cadena a proposito: asi los dos lados usan `AnyLabel` con la
  // misma etiqueta y coinciden sin depender de que tipo de destino sea cada uno.
  useEffect(() => {
    const etiqueta = getCurrentWindow().label;
    const unlisten = listen<OverlayPayload>(EVENTS.overlayShow, (e) => void boot(e.payload), {
      target: etiqueta,
    });
    // Cerrada desde Rust (el Escape de otra pantalla, Guardar como...): si el arrastre era
    // de esta, se suelta aquí también.
    const cierre = listen(EVENTS.overlayHide, () => soltarElGesto(), { target: etiqueta });
    return () => {
      void unlisten.then((fn) => fn());
      void cierre.then((fn) => fn());
    };
  }, [boot, soltarElGesto]);

  const toPhysical = useCallback(
    (rect: Rect): Rect => aVirtual(rect, payload!.monitor, scale),
    [payload, scale],
  );

  /**
   * Lo que se graba o se baja con scroll, de una sola pantalla: ni el video ni el scroll
   * cruzan. Lo normal es que la pantalla que manda ya lo haya recortado al cambiar de modo;
   * esto es por si se graba antes de que llegue su aviso.
   */
  const aUnaPantalla = useCallback(
    (rect: Rect): Rect => enUnaPantalla(toPhysical(rect), monitores),
    [toPhysical, monitores],
  );

  const capturarRegion = useCallback(
    async (rect: Rect, action: StillAction) => {
      if (!payload || busy) return;
      setBusy(true);
      setError(null);
      try {
        await captureStill(toPhysical(rect), action);
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [payload, busy, toPhysical],
  );

  /**
   * La ultima region capturada, traida a coordenadas CSS de ESTA pantalla, o null si fue
   * en otra. Es la vuelta de `toPhysical`: el payload la trae en coordenadas del
   * escritorio virtual justamente porque puede no ser de aqui.
   *
   * Se mira por el centro, igual que hace Rust al decidir de que pantalla recorta, para
   * que las dos mitades esten de acuerdo sobre de quien es una region.
   */
  const ultimaRegion = useMemo(() => {
    const r = payload?.lastRegion;
    if (!r || !payload) return null;
    if (!esDeEstaPantalla(r, payload.monitor)) return null;
    return aPantalla(r, payload.monitor, scale);
  }, [payload, scale]);

  const ultimaRegionRef = useRef<Rect | null>(null);
  ultimaRegionRef.current = ultimaRegion;

  /** La captura con scroll de esa zona: Rust cierra el overlay y se pone a bajar. */
  const empezarScroll = useCallback(
    async (rect: Rect) => {
      if (!payload || busy) return;
      setBusy(true);
      setError(null);
      try {
        await scrollIniciar(aUnaPantalla(rect));
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [payload, busy, aUnaPantalla],
  );

  const capturarTodasLasPantallas = useCallback(
    async (action: StillAction) => {
      if (busy) return;
      setBusy(true);
      setError(null);
      try {
        await captureAllScreens(action);
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [busy],
  );

  const runStill = useCallback(
    async (action: StillAction) => {
      if (!selection) return;
      await capturarRegion(selection, action);
    },
    [selection, capturarRegion],
  );

  /**
   * Perfil al vuelo: se suelta el raton y ya esta, sin barra y sin un solo clic mas. En
   * foto eso es la imagen en el portapapeles; en video o GIF, la grabacion arrancando.
   * Con el otro perfil sale la barra, que ademas deja ajustar el recuadro antes: grabando
   * eso importa mas que en una foto, porque lo que salga mal se ve minutos despues.
   */
  const alVuelo = !conBarra;

  /**
   * Si esta captura se abrio con la tecla del editor.
   *
   * Sale del payload y no de un ajuste porque es del DISPARO, no de la configuracion: la
   * misma persona usa las dos teclas el mismo dia, una para la captura de todos los dias
   * y otra para la que hay que trabajar.
   */
  const alEditor = payload?.intent === "edit";
  const alEditorRef = useRef(false);
  alEditorRef.current = alEditor;

  /**
   * Graba LA VENTANA de debajo del cursor, no el trozo de pantalla donde está.
   *
   * La diferencia se ve en cuanto algo pasa por delante: grabando la región, un aviso que
   * salta encima acaba dentro del vídeo; pidiéndole a Windows esa ventana, no.
   */
  const grabarVentana = useCallback(
    async (ventana: VentanaDelSistema, format: "gif" | "video") => {
      if (!payload || busy || !ventana.hwnd) return;
      setBusy(true);
      setError(null);
      try {
        await startRecording(toPhysical(ventana.rect), {
          format,
          fps: payload.settings.fps,
          audio: payload.settings.recordAudio && format === "video",
          microphone: payload.settings.recordMicrophone && format === "video",
          highlightClicks: payload.settings.highlightClicks,
          highlightKeys: payload.settings.highlightKeys,
          window: ventana.hwnd,
        });
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [payload, busy, toPhysical],
  );

  const grabarRegion = useCallback(
    async (rect: Rect, format: "gif" | "video") => {
      if (!payload || busy) return;
      setBusy(true);
      setError(null);
      try {
        await startRecording(aUnaPantalla(rect), {
          format,
          fps: payload.settings.fps,
          // El interruptor de audio del overlay se fue con la barra vieja: mientras el
          // ajuste diga "todavia no disponible", esto es siempre false.
          audio: payload.settings.recordAudio && format === "video",
          // El GIF no lleva sonido, asi que el microfono tampoco tiene sentido ahi.
          microphone: payload.settings.recordMicrophone && format === "video",
          // Los aros si valen igual en GIF: el fotograma es el mismo.
          highlightClicks: payload.settings.highlightClicks,
          highlightKeys: payload.settings.highlightKeys,
        });
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [payload, busy, aUnaPantalla],
  );

  const nudge = useCallback((dx: number, dy: number, resize: boolean) => {
    setSelection((prev) => {
      if (!prev) return prev;
      const g = geometriaRef.current;
      if (resize && g) {
        // Con la proporción fija, las dos medidas crecen juntas, de escalón en escalón.
        return crecerConProporcion(prev, dx + dy, g.unidad, g.limite, g.escala);
      }
      if (resize) {
        return {
          ...prev,
          width: Math.max(2, prev.width + dx),
          height: Math.max(2, prev.height + dy),
        };
      }
      return { ...prev, x: prev.x + dx, y: prev.y + dy };
    });
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const step = e.shiftKey ? 10 : 1;
      switch (e.key) {
        case "Escape":
          e.preventDefault();
          // Un solo Escape se sale, haya recorte hecho o no. Antes el primero borraba el
          // recorte y hacia falta otro para cerrar: quien pulsa Escape encima de una
          // captura quiere irse, y el recorte se rehace arrastrando otra vez, sin tener
          // que borrarlo antes. Y a medio arrastre, el gesto se suelta aquí: al soltar el
          // botón no se captura ni se graba nada.
          soltarElGesto();
          void cancelCapture();
          return;
        case "Enter":
          e.preventDefault();
          // Enter hace lo que diga la barra de arriba: copiar la foto, empezar a grabar o
          // empezar a bajar la página.
          if (modo === "still" && conScrollRef.current) {
            if (selectionRef.current) void empezarScroll(selectionRef.current);
          } else if (modo !== "still") {
            if (selectionRef.current) void grabarRegion(selectionRef.current, modo);
          } else {
            void runStill("copy");
          }
          return;
        case "ArrowLeft":
          e.preventDefault();
          nudge(-step, 0, e.altKey);
          return;
        case "ArrowRight":
          e.preventDefault();
          nudge(step, 0, e.altKey);
          return;
        case "ArrowUp":
          e.preventDefault();
          nudge(0, -step, e.altKey);
          return;
        case "ArrowDown":
          e.preventDefault();
          nudge(0, step, e.altKey);
          return;
      }
      const key = e.key.toLowerCase();
      if (key === "s" && e.ctrlKey && e.shiftKey) {
        e.preventDefault();
        void runStill("save_as");
      } else if (key === "s" && e.ctrlKey) {
        e.preventDefault();
        void runStill("save");
      } else if (key === "s") {
        // La captura con scroll. Solo en foto, como su botón.
        if (modoRef.current === "still") cambiarScroll(!conScrollRef.current);
      } else if (key === "a" && e.ctrlKey) {
        e.preventDefault();
        const todo = { x: 0, y: 0, width: window.innerWidth, height: window.innerHeight };
        if (!alVuelo) setSelection(todo);
        else if (modo !== "still") void grabarRegion(todo, modo);
        else void capturarRegion(todo, "copy");
      } else if (key === "e") {
        void runStill("edit");
      } else if (key === "a") {
        // Anclar. `Ctrl+A` de aqui arriba es otra cosa (toda la pantalla), y no chocan:
        // la letra sola actua sobre el recorte y con Ctrl cambia lo que se recorta.
        void runStill("pin");
      } else if (key === "t") {
        // El texto de la captura al portapapeles, leido por el motor de Windows.
        void runStill("text");
      } else if (key === "c") {
        // El color que hay debajo del cursor. La lupa ya lo ensennaba y no habia forma de
        // llevarselo; asi el cuentagotas no es una herramienta mas, es una tecla.
        //
        // No cierra la seleccion: quien esta cogiendo un color suele coger varios, y
        // cerrar despues del primero obligaria a volver a lanzar el atajo cada vez.
        void copyColor(hexRef.current)
          .then(() => setColorCopiado(hexRef.current))
          .catch((err) => setError(String(err)));
      } else if (key === "x" && !e.ctrlKey && !e.altKey && !e.metaKey) {
        // La proporción del recorte: libre, 16:9, 1:1, 9:16 y 4:3, y vuelta a empezar; con
        // Mayús, hacia atrás. Se pulsa a mitad de arrastrar y el recuadro cambia ya. La X
        // no la usaba nada aquí, y sola no choca con `Alt+X`, que es uno de los atajos
        // globales que propone la bienvenida. Con scroll no hace nada, como su botón.
        if (conScrollRef.current) return;
        e.preventDefault();
        elegirProporcion(siguienteProporcion(proporcionRef.current, e.shiftKey));
      } else if (key === "p") {
        difundir({ fullScreen: !pantallaRef.current });
      } else if (key === "b") {
        // La barra de acciones, encendida o apagada. Con `Alt` no: es una letra sola como
        // las demas de aqui, y el `b` de "barra" no lo usa nada mas.
        cambiarBarra(!conBarraRef.current);
      } else if (key === "f") {
        difundir({ mode: "still" });
      } else if (key === "g") {
        if (!grabandoRef.current) difundir({ mode: "gif" });
      } else if (key === "v") {
        if (!grabandoRef.current) difundir({ mode: "video" });
      } else if (key === "w") {
        if (grabandoRef.current) return;
        // Grabar SOLO la ventana de debajo, se tape o no se tape. Es una tecla y no un
        // boton porque solo tiene sentido mientras hay una ventana resaltada debajo del
        // raton, que es justo cuando no se esta mirando ninguna barra.
        const debajo = hoveredRef.current;
        if (!debajo?.hwnd) return;
        e.preventDefault();
        void grabarVentana(debajo, modoRef.current === "gif" ? "gif" : "video");
      } else if (key === "r") {
        // La misma zona otra vez, sin volver a arrastrar. Solo responde la pantalla donde
        // cayo aquella region: en las demas no hay nada que repetir, asi que no hacen nada
        // en vez de capturar algo que no es.
        const previa = ultimaRegionRef.current;
        if (!previa) return;
        e.preventDefault();
        // Mismo reparto que `Ctrl+A`: con la barra se deja puesta para retocarla, y con el
        // perfil "se copia sola" se hace y ya, que es lo que ese perfil promete.
        if (!alVuelo) setSelection(previa);
        else if (modoRef.current !== "still") void grabarRegion(previa, modoRef.current);
        else void capturarRegion(previa, "copy");
      } else if (key === "0") {
        // Todas las pantallas de golpe, en una sola imagen. El `0` va con los numeros de
        // pantalla y significa "ninguna en concreto: todas".
        //
        // Esta no necesita ir por evento como las otras: no hay que decidir a que ventana
        // le toca, porque Rust las junta todas leyendo las capturas congeladas. La que
        // tiene el foco la pide y ya.
        //
        // Solo tiene sentido en foto: grabar es de una region de una pantalla, asi que en
        // video o GIF esta tecla no hace nada en vez de hacer algo raro.
        if (modoRef.current !== "still") return;
        e.preventDefault();
        void capturarTodasLasPantallas("copy");
      } else if (key >= "1" && key <= "9") {
        // El numero que se ve en cada pantalla es la tecla que se la lleva. La orden va
        // por evento porque la tecla solo llega a la pantalla que tiene el foco, y casi
        // nunca es la que se quiere capturar.
        e.preventDefault();
        void emit(EVENTS.overlayTakeScreen, Number(key));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [
    runStill,
    nudge,
    alVuelo,
    modo,
    capturarRegion,
    capturarTodasLasPantallas,
    grabarRegion,
    difundir,
    cambiarBarra,
    cambiarScroll,
    empezarScroll,
    elegirProporcion,
    soltarElGesto,
  ]);

  const readHex = useCallback(
    (cssX: number, cssY: number) => {
      if (!source) return;
      const ctx = source.getContext("2d", { willReadFrequently: true });
      if (!ctx) return;
      const px = clamp(Math.floor(cssX * scale), 0, source.width - 1);
      const py = clamp(Math.floor(cssY * scale), 0, source.height - 1);
      const data = ctx.getImageData(px, py, 1, 1).data;
      const toHex = (c: number) => c.toString(16).padStart(2, "0");
      setHex("#" + toHex(data[0]) + toHex(data[1]) + toHex(data[2]));
    },
    [source, scale],
  );

  // El raton de un gesto de otra pantalla que pasa por esta: la lupa sale aqui, con el
  // color de aqui.
  useEffect(() => {
    if (cursorRemoto) readHex(cursorRemoto.x, cursorRemoto.y);
  }, [cursorRemoto, readHex]);

  // Durante el gesto los eventos se escuchan en window, no en el div: asi el arrastre
  // no se pierde aunque el webview no conceda la captura de puntero. Y con el boton
  // apretado, Chromium sigue mandando aqui el raton aunque salga de la ventana: asi el
  // recorte sigue en la pantalla de al lado (`clientX` mas alla del ancho, o negativo).
  useEffect(() => {
    if (mode.kind === "idle") return;

    const onMove = (e: PointerEvent) => {
      const x = e.clientX;
      const y = e.clientY;
      setCursor({ x, y });
      if (mode.kind === "drawing") {
        // La lupa sigue en pantalla mientras se dibuja, asi que el color tiene
        // que seguir al cursor en vez de quedarse en el del primer clic.
        readHex(x, y);
        setSelection(dibujar(mode.originX, mode.originY, x, y));
      } else if (mode.kind === "moving") {
        setSelection(moverA(x - mode.grabX, y - mode.grabY, mode.base));
      } else if (mode.kind === "resizing") {
        const g = geometriaRef.current;
        if (g) {
          setSelection(asaConProporcion(mode.base, mode.handle, { x, y }, g.unidad, g.limite, g.escala));
        } else {
          const b = limitarCursor(x, y);
          setSelection(applyHandle(mode.base, mode.handle, b.x, b.y));
        }
      }
    };

    const onUp = () => {
      if (cerrada.current) {
        setMode({ kind: "idle" });
        return;
      }
      if (mode.kind === "drawing") {
        const drawn = selectionRef.current;
        const dibujado = drawn && drawn.width >= MIN_DRAG && drawn.height >= MIN_DRAG;
        // Clic seco: si hay una ventana debajo, se selecciona entera (estilo ShareX).
        if (!dibujado) setSelection(mode.candidate);
        const elegido = dibujado ? drawn : mode.candidate;
        // Al soltar manda el modo elegido arriba: grabar empieza aqui mismo, y la foto
        // solo se copia sola si ese es el perfil. Si no, sale la barra de la seleccion.
        if (elegido) {
          // Con scroll, al soltar empieza a bajar: no hay nada que elegir antes.
          if (modo === "still" && conScrollRef.current) void empezarScroll(elegido);
          else if (modo !== "still") void grabarRegion(elegido, modo);
          // La tecla del editor no pregunta: quien la pulsa ya sabe que va a anotar, y
          // pararse en la barra a elegir «editar» seria un clic de mas en el unico sitio
          // donde no hace falta decidir nada.
          else if (alEditor) void capturarRegion(elegido, "edit");
          else if (alVuelo) void capturarRegion(elegido, "copy");
        }
      }
      setMode({ kind: "idle" });
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }, [
    mode,
    readHex,
    dibujar,
    moverA,
    limitarCursor,
    alVuelo,
    alEditor,
    modo,
    capturarRegion,
    grabarRegion,
    empezarScroll,
  ]);

  /** Se lleva esta pantalla entera con lo que diga la barra: foto, video o GIF. */
  const llevarsePantalla = useCallback(async () => {
    const todo = {
      x: 0,
      y: 0,
      width: window.innerWidth,
      height: window.innerHeight,
    };
    difundir({ fullScreen: false });
    // Con la barra puesta, la pantalla entera es una SELECCION, no una captura. Quien
    // elige ese perfil quiere decidir despues: copiar, guardar, editar o anclar. Antes se
    // la llevaba al portapapeles y cerraba, y la barra no aparecia nunca; `Ctrl+A`, que
    // hace exactamente lo mismo, si la ensennaba. Dos caminos al mismo sitio no pueden
    // acabar en dos sitios distintos. Lo conto Munir el 9 de septiembre de 2026.
    if (alEditorRef.current) {
      await capturarRegion(todo, "edit");
      return;
    }
    if (conBarraRef.current) {
      setSelection(todo);
      return;
    }
    if (modoRef.current !== "still") await grabarRegion(todo, modoRef.current);
    else await capturarRegion(todo, "copy");
  }, [difundir, capturarRegion, grabarRegion]);

  // Las dos ordenes que llegan de las otras pantallas: que cambie la barra, y que esta
  // pantalla en concreto se capture entera porque alguien pulso su numero.
  useEffect(() => {
    const modos = listen<OverlayModeState>(EVENTS.overlayMode, ({ payload: p }) => {
      setModo(grabandoRef.current ? "still" : p.mode);
      setPantallaEntera(p.fullScreen);
      setConBarra(p.withToolbar);
      if (p.fullScreen) setSelection(null);
      // El scroll es de foto y de un recorte: grabar o coger la pantalla entera lo apaga.
      if (p.mode !== "still" || p.fullScreen) setConScroll(false);
    });
    const scroll = listen<boolean>(EVENTO_SCROLL_MODO, ({ payload: valor }) => {
      setConScroll(valor);
      if (valor) setPantallaEntera(false);
    });
    const numeros = listen<number>(EVENTS.overlayTakeScreen, ({ payload: n }) => {
      if (n === numeroPantalla.current) void llevarsePantalla();
    });
    const proporciones = listen<Proporcion>(EVENTO_PROPORCION, ({ payload: valor }) => {
      setProporcion(valor);
    });
    return () => {
      void modos.then((fn) => fn());
      void numeros.then((fn) => fn());
      void scroll.then((fn) => fn());
      void proporciones.then((fn) => fn());
    };
  }, [llevarsePantalla]);

  const onPointerDown = (e: React.MouseEvent) => {
    if (busy) return;
    // Lo que nace encima de la barra todavia no es nada: se apunta donde empezo y se
    // espera a ver si el puntero se mueve (recorte por debajo) o se suelta (clic del
    // boton). Ver `origenBarra`.
    if (enLaBarra(e.target)) {
      origenBarra.current = { x: e.clientX, y: e.clientY };
      return;
    }
    // Con "pantalla entera" puesto no hay nada que arrastrar: donde caiga el clic, esa
    // pantalla se lleva, y se lleva YA. Un clic es un clic, tambien con el perfil de la
    // barra: quien pide la pantalla entera ya ha dicho lo que quiere.
    if (pantallaEntera) {
      void llevarsePantalla();
      return;
    }
    const x = e.clientX;
    const y = e.clientY;
    // Donde empieza el gesto: lo primero que se cuenta a las demas pantallas.
    setCursor({ x, y });
    const current = selectionRef.current;
    if (current && contains(current, x, y)) {
      empezarGesto(current);
      setMode({ kind: "moving", grabX: x - current.x, grabY: y - current.y, base: current });
      return;
    }
    empezarGesto(null);
    setMode({ kind: "drawing", originX: x, originY: y, candidate: windowAt(x, y)?.rect ?? null });
    setSelection({ x, y, width: 0, height: 0 });
  };

  /**
   * En Linux el overlay se esconde de verdad, y con Escape a medio arrastre WebKitGTK no ve
   * soltar el botón: cree que sigue pulsado y la pulsación siguiente no le da `pointerdown`, solo
   * `mousedown`. Se le manda el que falta al elemento pulsado, para que cada cosa responda como
   * siempre: el fondo empieza el recorte, las asas estiran y la barra de acciones se queda su clic.
   */
  const onMouseDownCapture = (e: React.MouseEvent) => {
    const visto = pointerdownVisto.current;
    pointerdownVisto.current = false;
    if (visto || e.button !== 0 || e.buttons !== 1) return;
    e.target.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        cancelable: true,
        button: 0,
        buttons: 1,
        clientX: e.clientX,
        clientY: e.clientY,
        pointerType: "mouse",
        isPrimary: true,
      }),
    );
    pointerdownVisto.current = false;
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const desdeLaBarra = origenBarra.current;
    if (desdeLaBarra) {
      // `buttons` a cero es que ya se solto: aquello era un clic de la barra y este
      // movimiento no arrastra nada. Sin esta linea, mover el raton despues de pulsar un
      // boton se ponia a dibujar un recorte solo.
      if (e.buttons === 0 || pantallaEntera || busy) {
        origenBarra.current = null;
      } else if (
        Math.abs(e.clientX - desdeLaBarra.x) >= MIN_DRAG ||
        Math.abs(e.clientY - desdeLaBarra.y) >= MIN_DRAG
      ) {
        origenBarra.current = null;
        empezarGesto(null);
        // Sin candidata: quien arrastra desde la barra quiere SU recorte, no la ventana
        // que haya debajo.
        setMode({
          kind: "drawing",
          originX: desdeLaBarra.x,
          originY: desdeLaBarra.y,
          candidate: null,
        });
        setSelection(dibujar(desdeLaBarra.x, desdeLaBarra.y, e.clientX, e.clientY));
        return;
      }
    }
    setCursorAqui(true);
    if (mode.kind !== "idle") return;
    setCursor({ x: e.clientX, y: e.clientY });
    readHex(e.clientX, e.clientY);
  };

  if (!payload || !fondoListo) {
    return <BootScreen error={bootError} generacion={payload?.generation ?? null} />;
  }

  const active =
    selection !== null && selection.width >= MIN_DRAG && selection.height >= MIN_DRAG
      ? selection
      : null;
  const highlight = !active && hovered ? hovered : null;
  /**
   * Con varias pantallas el recorte puede estar entero en otra, o asomar solo un trozo por
   * esta. `visible` dice si asoma; `trozo`, lo que se ve aqui, que es donde van la barra de
   * acciones y la medida. Con una sola pantalla, los dos son el recorte, como siempre.
   */
  const vista = vistaDeLaVentana();
  const visible = active !== null && (!variasPantallas || interseca(active, vista));
  const trozo = active && variasPantallas ? (interseccion(active, vista) ?? active) : active;
  const rectFisico = active && variasPantallas ? aFisico(active, payload.monitor, scale) : null;
  /** La barra de acciones va en una sola pantalla: la que se lleva mas trozo. */
  const manda =
    !variasPantallas ||
    (rectFisico !== null && pantallaQueManda(rectFisico, monitores)?.id === payload.monitor.id);
  /** Y la medida, en la de la esquina de arriba a la izquierda, que es donde se pone. */
  const conLaMedida =
    !variasPantallas ||
    (rectFisico !== null && pantallaDeLaMedida(rectFisico, monitores)?.id === payload.monitor.id);
  // Un solo escritor: sin gesto aqui, el recorte lo reajusta la que manda, y solo si no hay
  // un gesto en marcha en otra pantalla (ver el efecto de la proporcion).
  escritorRef.current = !variasPantallas || (manda && faseRemota === "quieta");
  /** Mientras otra pantalla arrastra, aqui no se ofrece nada: se mira. */
  const gestoFuera = faseRemota !== "quieta";
  const sitioBarra = trozo ? sitioDeLaBarra(trozo, window.innerHeight) : null;
  const conBarraDeAcciones =
    active !== null &&
    visible &&
    manda &&
    !gestoFuera &&
    mode.kind === "idle" &&
    !alVuelo &&
    !alEditor &&
    !conScroll;
  /**
   * Si la barra de acciones se ha ido ENCIMA del recorte (debajo no cabía): ahí es donde va
   * la etiqueta de la medida, y la tapaba entera. Pasa con todo recorte alto pegado abajo,
   * y con 9:16 pasa a menudo, justo cuando la etiqueta dice la proporción. Visto en una
   * ventana de verdad, en la pantalla de abajo al 125 % (S27).
   */
  const barraEncima =
    conBarraDeAcciones && sitioBarra !== null && sitioBarra.volteada && sitioBarra.top <= trozo!.y;
  /**
   * Donde va la lupa: en la pantalla donde esta el raton. Si el gesto es de otra pantalla,
   * lo dice su aviso; si es de esta, el raton puede haberse ido a la de al lado, y entonces
   * aqui no se pinta.
   */
  const cursorDentro =
    cursor.x >= 0 && cursor.y >= 0 && cursor.x < vista.width && cursor.y < vista.height;
  const lupaEn = gestoFuera
    ? faseRemota === "dibujando"
      ? cursorRemoto
      : null
    : cursorAqui && cursorDentro && (!active || mode.kind === "drawing")
      ? cursor
      : null;
  const magnifierVisible = lupaEn !== null && payload.settings.showMagnifier && !pantallaEntera;

  return (
    <div
      // En la fase de captura: las asas y la barra paran su `pointerdown` antes de que suba.
      onPointerDownCapture={() => {
        pointerdownVisto.current = true;
      }}
      onPointerDown={onPointerDown}
      onMouseDownCapture={onMouseDownCapture}
      onPointerMove={onPointerMove}
      onPointerEnter={() => setCursorAqui(true)}
      onPointerLeave={() => setCursorAqui(false)}
      className="relative h-screen w-screen overflow-hidden"
      style={{ cursor: pantallaEntera ? "pointer" : active ? "default" : "crosshair" }}
    >
      {/* El fondo: el lienzo con la pantalla congelada, colgado aqui por el efecto de
          arriba. Un lienzo y no un <img>, para decodificar la imagen una sola vez. */}
      <div ref={fondoRef} className="pointer-events-none absolute inset-0" />

      {/* Sin seleccion: velo uniforme. Con seleccion: el velo lo dibuja la sombra del recuadro. */}
      {/* Y si el recorte esta en otra pantalla, aqui el velo uniforme: la sombra de un recuadro
          lejano no llega a taparla entera. */}
      {!visible && <div className="pointer-events-none absolute inset-0 bg-black/45" />}

      {/* La ventana de debajo del cursor, con su nombre: el marco solo, y encima azul
          fuerte, no decia de que era ni que se le podia hacer clic. */}
      {highlight && (
        <div
          style={{
            left: highlight.rect.x,
            top: highlight.rect.y,
            width: highlight.rect.width,
            height: highlight.rect.height,
          }}
          className="pointer-events-none absolute rounded-[3px] border border-white/45 bg-white/[0.06]"
        >
          {highlight.title && (
            <span
              // Fuera del marco cuando cabe: dentro se pone justo encima de la barra de
              // titulo de la ventana y se leen dos titulos pisados uno sobre otro.
              className={`absolute left-0 flex items-center gap-2 rounded-md bg-neutral-900/90 px-2 py-1 text-[12px] whitespace-nowrap text-neutral-200 shadow-lg backdrop-blur-sm ${
                modo !== "still" && highlight.hwnd > 0 ? "max-w-[420px]" : "max-w-[min(360px,100%)]"
              } ${highlight.rect.y > 28 ? "-top-7" : "top-1.5 left-1.5"}`}
            >
              {/* Si no cabe, se corta el título, que es largo y se reconoce por el principio. */}
              <span className="min-w-0 truncate">{highlight.title}</span>
              {/*
                La pista de la tecla, solo grabando y solo si Windows nos da esa ventana.
                Va pegada al titulo porque es ahi donde se esta mirando: una leyenda en una
                esquina de la pantalla no la lee nadie con el raton al otro lado. Suelta, sin
                cajita dentro de la etiqueta, y entera siempre: es lo que hay que leer.
              */}
              {modo !== "still" && highlight.hwnd > 0 && (
                <span className="shrink-0 font-semibold text-white">{t("W: grabar solo esta ventana")}</span>
              )}
            </span>
          )}
        </div>
      )}

      {/* El recuadro va entero, sin recortar a la ventana: asi la junta entre pantallas no
          lleva raya, y cada pantalla pinta las asas que le caen. */}
      {active && visible && (
        <>
          <div
            style={{
              left: active.x,
              top: active.y,
              width: active.width,
              height: active.height,
              boxShadow: "0 0 0 100vmax rgba(0,0,0,0.45)",
            }}
            className="pointer-events-none absolute border border-blue-500/90"
          />
          <SelectionHandles
            rect={active}
            onGrab={(handle) => {
              empezarGesto(active);
              setMode({ kind: "resizing", handle, base: active });
            }}
          />
          {conLaMedida && trozo && (
            <DimensionBadge
              width={active.width * scale}
              height={active.height * scale}
              proporcion={
                tieneLaProporcion(active.width * scale, active.height * scale, proporcion)
                  ? proporcion
                  : "libre"
              }
              left={trozo.x + 2}
              top={!barraEncima && trozo.y > 30 ? trozo.y - 30 : trozo.y + 6}
            />
          )}
        </>
      )}

      {/*
        La cruceta: dos guias que cruzan la pantalla por donde esta el cursor. Alinear el
        borde de una seleccion con algo que esta al otro lado de la pantalla se hacia a
        ojo; con la guia se ve si estan en la misma linea antes de soltar.
        Se pinta cuando se pinta la lupa, que es cuando hace falta precision, y desaparece
        con la seleccion hecha para no ensuciar lo que se esta mirando.
      */}
      {magnifierVisible && lupaEn && (
        <div className="pointer-events-none absolute inset-0 z-30">
          <div
            className="absolute inset-x-0 h-px bg-white/25 mix-blend-difference"
            style={{ top: lupaEn.y }}
          />
          <div
            className="absolute inset-y-0 w-px bg-white/25 mix-blend-difference"
            style={{ left: lupaEn.x }}
          />
        </div>
      )}

      {magnifierVisible && lupaEn && source && (
        <Magnifier
          source={source}
          px={lupaEn.x * scale}
          py={lupaEn.y * scale}
          {...sitioDeLaLupa(lupaEn.x, lupaEn.y, window.innerWidth, window.innerHeight)}
          hex={hex}
        />
      )}

      <AnimatePresence>
        {conBarraDeAcciones && trozo && (
          <FloatingToolbar
            key="toolbar"
            left={clamp(trozo.x + trozo.width / 2, 190, window.innerWidth - 190)}
            top={sitioBarra!.top}
            flipped={sitioBarra!.volteada}
            busy={busy}
            modo={modo}
            onCopy={() => void runStill("copy")}
            onSave={() => void runStill("save")}
            onSaveAs={() => void runStill("save_as")}
            onEdit={() => void runStill("edit")}
            onPin={() => void runStill("pin")}
            onText={() => void runStill("text")}
            onQr={() => void runStill("qr")}
            onRecord={() => {
              if (selection && modo !== "still") void grabarRegion(selection, modo);
            }}
            onCancel={() => void cancelCapture()}
          />
        )}
      </AnimatePresence>

      {/*
        El color copiado. Va donde los errores y con la misma forma, porque es lo mismo:
        algo que acaba de pasar y que hay que contar sin tapar la pantalla.
      */}
      {colorCopiado && !error && (
        <div className="pointer-events-none absolute inset-x-0 top-8 flex justify-center">
          <div className="flex items-center gap-2 rounded-xl border border-white/15 bg-neutral-900/90 px-4 py-2.5 text-xs text-neutral-200 shadow-2xl backdrop-blur-md">
            <span
              className="size-3 rounded-[4px] border border-white/25"
              style={{ backgroundColor: colorCopiado }}
            />
            <span className="font-mono uppercase">{colorCopiado}</span>
            <span className="text-neutral-400">{t("copiado")}</span>
          </div>
        </div>
      )}

      {error && (
        <div className="pointer-events-none absolute inset-x-0 top-8 flex justify-center">
          <div className="max-w-xl rounded-xl border border-red-500/30 bg-red-950/90 px-4 py-2.5 text-xs text-red-200 shadow-2xl backdrop-blur-md">
            {/* Los errores llegan de Rust escritos en espannol. Como la clave del
                diccionario ES la frase espannola, pasarlos por `t` traduce los que
                alguien ha traducido y deja el resto tal cual, que es mejor que verlos
                todos en castellano con la aplicacion en ingles. */}
            {t(error)}
          </div>
        </div>
      )}

      {/*
        El fantasma de la ultima captura, donde estuvo, con su tecla encima. Una tecla que
        no se ve no la usa nadie, y esto se explica solo: ahi tenias lo de antes, pulsa R y
        vuelve. Se va en cuanto se empieza a arrastrar, para no estorbar.
      */}
      {cursorAqui &&
        ultimaRegion &&
        !seleccionAqui &&
        mode.kind === "idle" &&
        !pantallaEntera &&
        !hovered && (
        <div
          className="pointer-events-none absolute rounded-[3px] border border-dashed border-white/35"
          style={{
            left: ultimaRegion.x,
            top: ultimaRegion.y,
            width: ultimaRegion.width,
            height: ultimaRegion.height,
          }}
        >
          <span className="absolute -top-7 left-0 flex items-center gap-1.5 rounded-full border border-white/10 bg-neutral-900/85 px-2.5 py-1 text-[12px] whitespace-nowrap text-neutral-300 shadow-lg backdrop-blur-md">
            <kbd className="rounded-[4px] border border-white/15 bg-white/10 px-1.5 py-0.5 text-[12px] leading-none font-medium text-neutral-200">
              R
            </kbd>
            {t("la de antes")}
          </span>
        </div>
      )}

      {/* Lo que hace la captura con scroll, mientras se elige la zona: sin esto, el botón
          encendido no dice qué va a pasar al soltar. */}
      {conScroll && cursorAqui && !active && mode.kind === "idle" && !error && (
        <div className="pointer-events-none absolute inset-x-0 top-[84px] flex justify-center">
          <div className="rounded-xl border border-white/10 bg-neutral-900/90 px-4 py-2.5 text-[14px] text-neutral-100 shadow-2xl backdrop-blur-md">
            {t("Recorta lo que se desplaza y winshotx lo bajará solo")}
          </div>
        </div>
      )}

      {pantallaEntera && !active && (
        <ScreenPicker
          numero={payload.screenNumber}
          total={payload.screenCount}
          modo={modo}
          ancho={payload.monitor.width}
          alto={payload.monitor.height}
        />
      )}

      {/* La barra de modos, solo en la pantalla del raton: dos barras iguales, una en cada
          monitor, parecen dos capturas abiertas. Al elegir pantalla entera se ve en todas,
          que es justo cuando se elige entre ellas. */}
      {(cursorAqui || pantallaEntera) && (
        <ModeBar
          value={modo}
          onChange={(m) => difundir({ mode: m })}
          pantallaEntera={pantallaEntera}
          onPantallaEntera={(v) => difundir({ fullScreen: v })}
          conBarra={conBarra}
          onConBarra={cambiarBarra}
          conScroll={conScroll}
          onConScroll={cambiarScroll}
          proporcion={proporcion}
          onProporcion={elegirProporcion}
          soloFoto={payload?.recording ?? false}
          onAjustes={() => void openSettings()}
          onCancel={() => void cancelCapture()}
          dimmed={mode.kind !== "idle" || gestoFuera}
        />
      )}
    </div>
  );
}
