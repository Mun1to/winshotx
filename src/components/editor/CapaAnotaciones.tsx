import {
  createContext,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import {
  conEscala,
  controlDeCurva,
  desplazar,
  ESCALA_MAXIMA,
  ESCALA_MINIMA,
  escalaDe,
  FOCO_OSCURECE,
  simplificarTrazo,
  type Anotacion,
  type Herramienta,
  type Punto,
} from "../../lib/anotaciones";
import { useT } from "../../lib/i18n";

/**
 * La capa donde se dibuja encima de la vista previa.
 *
 * Se pinta con SVG y no con un `<canvas>` por una razón concreta: aquí solo hay que
 * ENSEÑAR las marcas mientras se colocan, y quien las dibuja de verdad sobre los píxeles
 * es Rust al exportar. Con SVG cada marca es un elemento que React sabe pintar, mover y
 * borrar solo; con un canvas habría que repintarlo entero a mano en cada movimiento del
 * ratón y llevar la cuenta de lo que hay debajo.
 *
 * **Las coordenadas se guardan de 0 a 1.** La vista previa casi nunca mide lo que va a
 * medir el archivo, así que guardarlas en píxeles obligaría a recalcularlas cada vez que
 * alguien toca «Dimensiones», y una sola que se olvidara dejaría la flecha señalando a
 * otro sitio.
 *
 * **Lo dibujado se coge y se mueve** (S23): pulsar encima de una marca la elige y
 * arrastrarla la mueve, con la herramienta que sea o sin ninguna; pulsar donde no hay nada
 * dibuja. Un doble clic en un texto lo abre para reescribirlo en el mismo campo donde se
 * escribió. Quien decide qué se ha pulsado es el navegador: cada marca lleva encima su
 * silueta invisible y algo más gruesa que su trazo, así que una línea fina se coge aunque
 * se falle por unos píxeles, y la de encima gana a la de debajo, como se ve.
 */
interface Props {
  herramienta: Herramienta | null;
  color: string;
  anotaciones: Anotacion[];
  onAnadir: (anotacion: Anotacion) => void;
  /**
   * Lo que mide de ancho la imagen de verdad, en píxeles. Al exportar, la letra del texto
   * sale de ahí (`encode::anotacion::texto`), y la vista previa la saca igual para que lo
   * que se escribe encima mida lo que va a medir en el archivo.
   */
  anchoImagen: number;
  /**
   * Lo que mide de alto la imagen de verdad. Con el ancho dice cuánto estira el SVG de 1000 x
   * 1000 cada eje, que es lo que hace falta para que la punta de una flecha abra lo mismo en
   * una captura apaisada que en el archivo. Sin él, se cuenta cuadrada.
   */
  altoImagen?: number;
  /** El sitio en `anotaciones` de la marca elegida, que se ve con su halo. */
  elegida?: number | null;
  onElegir?: (indice: number | null) => void;
  /** Una marca movida o un texto reescrito: la marca entera, como queda. */
  onCambiar?: (indice: number, anotacion: Anotacion) => void;
  /** Un texto que se ha dejado vacío al reescribirlo: se quita. */
  onBorrar?: (indice: number) => void;
  /**
   * Mientras otra capa necesita los clics de la imagen (colocar el punto de un zoom), lo
   * dibujado no se coge: quien pulsa encima de una flecha para acercar la cámara ahí quiere
   * acercarla, no llevarse la flecha.
   */
  quieta?: boolean;
  /**
   * El tamaño con el que salen el texto y el paso nuevos, contra el de siempre (issues 18 y
   * 19). Lo elige la barra de arriba; de serie, 1.
   */
  escala?: number;
}

/** Dónde se está escribiendo un texto, de 0 a 1, y lo que lleva escrito. */
interface Escribiendo {
  x: number;
  y: number;
  valor: string;
  color: string;
  /** Si se reescribe un texto que ya estaba, su sitio en la lista. */
  indice?: number;
}

/** Lo que mide el trazo de las marcas en la pantalla, en píxeles. */
const GROSOR = 6;
/** Lo que se puede fallar al pulsar una marca, en píxeles a cada lado de su trazo. */
const MARGEN = 6;
/** Lo que tiene que moverse el ratón para que un clic sea un arrastre, en píxeles. */
const UMBRAL = 3;
/** El tiempo entre dos clics para que sean un doble clic: el de Windows de serie. */
const DOBLE_CLIC_MS = 500;

/** Las marcas que se cogen por dentro, porque son una mancha y no un trazo. */
const LLENAS: ReadonlySet<Herramienta> = new Set(["highlight", "blur"]);

const nada = () => {};

export function CapaAnotaciones({
  herramienta,
  color,
  anotaciones,
  onAnadir,
  anchoImagen,
  altoImagen = anchoImagen,
  elegida = null,
  onElegir = nada,
  onCambiar = nada,
  onBorrar = nada,
  quieta = false,
  escala = 1,
}: Props) {
  const t = useT();
  // El de ahora, para el texto que se guarda desde un oyente o un efecto viejo.
  const escalaViva = useRef(escala);
  escalaViva.current = escala;
  const marco = useRef<SVGSVGElement>(null);
  const [dibujando, setDibujando] = useState<Anotacion | null>(null);
  /** La marca que se está arrastrando, ya movida, mientras no se suelta. */
  const [movida, setMovida] = useState<{ indice: number; marca: Anotacion } | null>(null);
  /** El último clic sobre una marca, para saber si el siguiente es un doble clic. */
  const ultimoClic = useRef<{ indice: number; cuando: number; x: number; y: number } | null>(null);
  /** Quita los oyentes del arrastre en curso, si lo hay. */
  const soltarArrastre = useRef<(() => void) | null>(null);
  useEffect(() => () => soltarArrastre.current?.(), []);
  // Lo de ahora, para lo que se guarda desde un oyente o un efecto viejo.
  const vivas = useRef(anotaciones);
  vivas.current = anotaciones;
  /**
   * El texto que se está escribiendo encima de la imagen. Va también en una ref porque se
   * guarda desde dos sitios que llegan juntos con el mismo clic (pulsar fuera en la imagen,
   * y el campo que pierde el foco por ese mismo clic): el primero lo guarda y lo vacía, y
   * el segundo ya no encuentra nada. Sin eso salían dos textos iguales, uno encima del otro.
   */
  const [escribiendo, setEscribiendo] = useState<Escribiendo | null>(null);
  const abierto = useRef<Escribiendo | null>(null);
  abierto.current = escribiendo;
  const campo = useRef<HTMLInputElement>(null);
  /**
   * Por dónde ha pasado el ratón, para el lápiz y la flecha curva. Va en una ref y no en
   * el estado porque cambia en cada movimiento y no se pinta tal cual: el lápiz pinta su
   * versión aclarada y la curva, la curva que sale de él.
   */
  const camino = useRef<Punto[]>([]);
  // Los `:` de useId no valen dentro de un `url(#...)` en todos los motores.
  const idDelVelo = `velo-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;

  /** El punto del ratón, en tanto por uno sobre la capa. */
  const punto = (e: React.PointerEvent): [number, number] => {
    const caja = marco.current?.getBoundingClientRect();
    if (!caja || caja.width === 0 || caja.height === 0) return [0, 0];
    return [
      Math.min(1, Math.max(0, (e.clientX - caja.left) / caja.width)),
      Math.min(1, Math.max(0, (e.clientY - caja.top) / caja.height)),
    ];
  };

  /**
   * Deja el texto escrito como una marca más. Sin nada escrito, no deja nada.
   *
   * Si era uno que ya estaba, se queda con lo nuevo en su mismo sitio; y vaciado del todo,
   * se quita, que es lo que quiere quien borra todas sus letras y pulsa Intro.
   */
  const guardarTexto = () => {
    const sitio = abierto.current;
    if (!sitio) return;
    abierto.current = null;
    setEscribiendo(null);
    const escrito = sitio.valor.trim();
    if (sitio.indice !== undefined) {
      const antes = vivas.current[sitio.indice];
      if (antes?.kind !== "text") return;
      if (!escrito) onBorrar(sitio.indice);
      else if (escrito !== antes.text) onCambiar(sitio.indice, { ...antes, text: escrito });
      return;
    }
    if (escrito) {
      onAnadir(
        conEscala(
          { kind: "text", x1: sitio.x, y1: sitio.y, x2: sitio.x, y2: sitio.y, color: sitio.color, text: escrito },
          escalaViva.current,
        ),
      );
    }
  };

  /**
   * Pulsar encima de una marca: la elige, y si se arrastra, la mueve.
   *
   * El arrastre se sigue desde la ventana y no desde la capa, porque sin herramienta la capa
   * no recibe el ratón (deja pasar los clics a la cámara y al zoom de debajo) y porque quien
   * arrastra hasta el borde se sale un poco: la marca se para en el borde y sigue cogida.
   * Mientras dura, la marca movida solo se pinta aquí; el cambio de verdad sale una vez, al
   * soltar, para que Ctrl+Z la devuelva de un golpe a donde estaba y no paso a paso.
   */
  const agarrar = (e: React.PointerEvent, indice: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    // Con un texto a medias, pulsar otra cosa lo guarda y nada más, como pulsar en vacío.
    if (abierto.current) {
      guardarTexto();
      return;
    }
    const marca = anotaciones[indice];
    const caja = marco.current?.getBoundingClientRect();
    if (!marca || !caja || caja.width === 0 || caja.height === 0) return;
    // Cancelar el clic (arriba) también le quita al navegador el mover el foco. Si se queda
    // en un campo del panel, Supr borraría una letra de ese campo en vez de la marca.
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    onElegir(indice);

    const ahora = performance.now();
    const antes = ultimoClic.current;
    ultimoClic.current = { indice, cuando: ahora, x: e.clientX, y: e.clientY };
    if (
      marca.kind === "text" &&
      antes?.indice === indice &&
      ahora - antes.cuando < DOBLE_CLIC_MS &&
      Math.hypot(e.clientX - antes.x, e.clientY - antes.y) < UMBRAL * 2
    ) {
      ultimoClic.current = null;
      setEscribiendo({ x: marca.x1, y: marca.y1, valor: marca.text, color: marca.color, indice });
      return;
    }

    seguirArrastre(e, indice, marca, (dx, dy) => {
      // Un arrastre no es la mitad de un doble clic.
      ultimoClic.current = null;
      return desplazar(marca, dx / caja.width, dy / caja.height);
    });
  };

  /**
   * Arrastrar el asa de un texto o de un paso elegido: lo agranda o lo encoge (issues 18 y
   * 19). Crece lo que se aleja el ratón del sitio fijo de la marca (el centro del paso, la
   * esquina de arriba a la izquierda del texto), así que da igual hacia dónde se tire.
   */
  const estirar = (e: React.PointerEvent, indice: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const marca = anotaciones[indice];
    const caja = (e.currentTarget as Element).closest("[data-agarre]")?.getBoundingClientRect();
    if (!marca || !caja) return;
    const [fx, fy] =
      marca.kind === "step" ? [caja.left + caja.width / 2, caja.top + caja.height / 2] : [caja.left, caja.top];
    const desde = Math.hypot(e.clientX - fx, e.clientY - fy);
    if (desde < 1) return;
    const antes = escalaDe(marca);
    seguirArrastre(e, indice, marca, (dx, dy) => {
      const lejos = Math.hypot(e.clientX + dx - fx, e.clientY + dy - fy);
      const escala = Math.min(ESCALA_MAXIMA, Math.max(ESCALA_MINIMA, (antes * lejos) / desde));
      // A centésimas: el asa no necesita más, y así «Guardar» no manda 1,4999999.
      return conEscala(marca, Math.round(escala * 100) / 100);
    });
  };

  /**
   * Sigue un arrastre desde la ventana hasta que se suelta. `cambio` dice cómo queda la marca
   * con el ratón movido tanto desde donde se pulsó.
   */
  const seguirArrastre = (
    e: React.PointerEvent,
    indice: number,
    marca: Anotacion,
    cambio: (dx: number, dy: number) => Anotacion,
  ) => {
    const [x0, y0] = [e.clientX, e.clientY];
    let movido = false;
    let ultima = marca;
    const seguir = (ev: PointerEvent) => {
      const [dx, dy] = [ev.clientX - x0, ev.clientY - y0];
      if (!movido && Math.hypot(dx, dy) < UMBRAL) return;
      movido = true;
      ultima = cambio(dx, dy);
      setMovida({ indice, marca: ultima });
    };
    const quitar = () => {
      window.removeEventListener("pointermove", seguir);
      window.removeEventListener("pointerup", soltarla);
      window.removeEventListener("pointercancel", cancelar);
      soltarArrastre.current = null;
    };
    const soltarla = () => {
      quitar();
      if (movido) onCambiar(indice, ultima);
      setMovida(null);
    };
    // Si el sistema se lleva el ratón (una ventana que sale encima), la marca vuelve.
    const cancelar = () => {
      quitar();
      setMovida(null);
    };
    soltarArrastre.current?.();
    window.addEventListener("pointermove", seguir);
    window.addEventListener("pointerup", soltarla);
    window.addEventListener("pointercancel", cancelar);
    soltarArrastre.current = quitar;
  };

  // Pulsar en cualquier otro sitio suelta lo elegido. Se mira en todo el documento porque,
  // sin herramienta, el clic en vacío no llega a la capa: se lo lleva lo de debajo. Los
  // botones y los campos no lo sueltan, para que Supr siga sabiendo qué borrar después de
  // tocar la barra.
  useEffect(() => {
    if (elegida === null) return;
    const fuera = (e: PointerEvent) => {
      const donde = e.target instanceof Element ? e.target : null;
      if (donde?.closest("[data-agarre], button, input, select, textarea, [role='slider']")) return;
      onElegir(null);
    };
    document.addEventListener("pointerdown", fuera, true);
    return () => document.removeEventListener("pointerdown", fuera, true);
  }, [elegida, onElegir]);

  const tirarTexto = () => {
    abierto.current = null;
    setEscribiendo(null);
  };

  // El foco va al campo en cuanto aparece, antes de que se pinte: con un fotograma de
  // retraso, lo que se tecleaba nada más pulsar se perdía (en Edge, «Hola» salía «la»). El
  // clic que lo abre no se lo quita, porque `empezar` cancela el `pointerdown` y con él el
  // `mousedown`, que es quien mueve el foco.
  const hayCampo = escribiendo !== null;
  useLayoutEffect(() => {
    if (hayCampo) campo.current?.focus();
  }, [hayCampo]);

  // Cambiar de herramienta con el texto a medias lo guarda, igual que pulsar fuera.
  useEffect(() => {
    if (herramienta !== "text") guardarTexto();
    // Solo al cambiar de herramienta; `guardarTexto` lee lo vivo de la ref.
  }, [herramienta]);

  const empezar = (e: React.PointerEvent) => {
    // Con un texto a medias, pulsar fuera de él lo guarda, y ese clic no hace nada más:
    // quien pulsa fuera para terminar no quiere empezar otro texto sin darse cuenta.
    if (abierto.current) {
      e.preventDefault();
      guardarTexto();
      return;
    }
    if (!herramienta) return;
    e.preventDefault();
    const [x, y] = punto(e);
    // El texto no se arrastra: se pulsa donde va, se escribe ahí mismo, y Intro o pulsar
    // fuera lo deja puesto.
    if (herramienta === "text") {
      setEscribiendo({ x, y, valor: "", color });
      return;
    }
    // Y el paso tampoco, y además se numera solo. Numerar a mano es justo lo que se olvida
    // cuando alguien mete un paso en medio, y entonces hay dos treses en la misma imagen.
    if (herramienta === "step") {
      const siguiente = anotaciones.filter((a) => a.kind === "step").length + 1;
      onAnadir(conEscala({ kind: "step", x1: x, y1: y, x2: x, y2: y, color, text: String(siguiente) }, escala));
      return;
    }
    camino.current = [[x, y]];
    setDibujando({ kind: herramienta, x1: x, y1: y, x2: x, y2: y, color, text: "" });
  };

  const mover = (e: React.PointerEvent) => {
    if (!dibujando) return;
    const [x, y] = punto(e);
    if (dibujando.kind === "pen" || dibujando.kind === "curve") {
      const ultimo = camino.current[camino.current.length - 1];
      // Un punto cada poco: el ratón manda muchos más de los que cambian el dibujo.
      if (!ultimo || Math.hypot(x - ultimo[0], y - ultimo[1]) > 0.002) {
        camino.current = [...camino.current, [x, y]];
      }
    }
    setDibujando(conCamino({ ...dibujando, x2: x, y2: y }, camino.current));
  };

  const soltar = () => {
    if (!dibujando) return;
    // Un clic sin arrastre no es una marca, es un clic: sin esto, cada vez que alguien
    // pulsa sin querer queda un rectángulo de cero píxeles imposible de ver y de borrar.
    // En el lápiz cuenta todo lo que se ha movido, no solo dónde acaba: un círculo a mano
    // termina donde empezó.
    const puntos = dibujando.kind === "pen" ? camino.current : [[dibujando.x1, dibujando.y1], [dibujando.x2, dibujando.y2]];
    const xs = puntos.map((p) => p[0]);
    const ys = puntos.map((p) => p[1]);
    const ancho = Math.max(...xs) - Math.min(...xs);
    const alto = Math.max(...ys) - Math.min(...ys);
    if (ancho > 0.01 || alto > 0.01) onAnadir(conCamino(dibujando, camino.current, true));
    camino.current = [];
    setDibujando(null);
  };

  // La que se arrastra se pinta ya movida, y la que se está dibujando va encima de todas.
  const puestas = movida
    ? anotaciones.map((a, i) => (i === movida.indice ? movida.marca : a))
    : anotaciones;
  const todas = dibujando ? [...puestas, dibujando] : puestas;
  // El foco va debajo de todo y todos juntos en un velo, igual que al exportar
  // (`encode::anotacion::pintar`): así no oscurece lo dibujado ni se tapan entre ellos.
  const focos = todas.filter((a) => a.kind === "focus");
  // Con su sitio en la lista, que es lo que dice cuál se coge. El que se está reescribiendo
  // no se pinta: el campo va justo encima, con la misma letra, y se verían dos.
  const textos = puestas
    .map((a, i) => [a, i] as const)
    .filter(([a, i]) => a.kind === "text" && i !== escribiendo?.indice);
  const pasos = puestas.map((a, i) => [a, i] as const).filter(([a]) => a.kind === "step");
  const coger = quieta ? "none" : undefined;
  /**
   * El asa de estirar, en la esquina de abajo a la derecha de lo elegido y solo cuando se
   * puede coger. Mide lo mismo a cualquier tamaño de la vista: es para el ratón, no es parte
   * del dibujo.
   */
  const asa = (i: number) =>
    i === elegida && !quieta ? (
      <span
        data-estirar=""
        onPointerDown={(e) => estirar(e, i)}
        title={t("Arrastra para cambiar el tamaño")}
        className="absolute size-3 rounded-full border-2 border-[#0a76c9] bg-white shadow-[0_1px_3px_rgba(0,0,0,0.5)]"
        style={{ pointerEvents: "auto", cursor: "nwse-resize", translate: "-50% -50%" }}
      />
    ) : null;

  return (
    <Proporcion.Provider value={Math.max(1, altoImagen) / Math.max(1, anchoImagen)}>
    <svg
      ref={marco}
      onPointerDown={empezar}
      onPointerMove={mover}
      onPointerUp={soltar}
      onPointerLeave={soltar}
      viewBox="0 0 1000 1000"
      preserveAspectRatio="none"
      className={`absolute inset-0 size-full ${
        herramienta ? "cursor-crosshair" : "pointer-events-none"
      }`}
    >
      {focos.length > 0 && (
        <g data-marca="focus">
          <mask id={idDelVelo}>
            <rect x={0} y={0} width={1000} height={1000} fill="#ffffff" />
            {focos.map((f, i) => (
              <rect
                key={i}
                data-zona-clara=""
                x={Math.min(f.x1, f.x2) * 1000}
                y={Math.min(f.y1, f.y2) * 1000}
                width={Math.abs(f.x2 - f.x1) * 1000}
                height={Math.abs(f.y2 - f.y1) * 1000}
                fill="#000000"
              />
            ))}
          </mask>
          <rect
            data-velo=""
            x={0}
            y={0}
            width={1000}
            height={1000}
            fill="#000000"
            opacity={FOCO_OSCURECE}
            mask={`url(#${idDelVelo})`}
            pointerEvents="none"
          />
        </g>
      )}
      {todas.map((a, i) =>
        // La que se está dibujando todavía no es de nadie: no se coge.
        i >= puestas.length ? (
          <Marca key={i} anotacion={a} />
        ) : a.kind === "text" || a.kind === "step" ? null : (
          <g key={i} data-agarre={i} onPointerDown={(e) => agarrar(e, i)}>
            {i === elegida && <Halo anotacion={a} />}
            <Marca anotacion={a} />
            {/*
              Lo que se pulsa: la misma silueta, invisible y con unos píxeles más de trazo,
              para coger una línea fina sin tener que acertarle. Las manchas (resaltado y
              tapado) se cogen además por dentro.
            */}
            <g
              data-silueta=""
              stroke="transparent"
              strokeWidth={GROSOR + 2 * MARGEN}
              strokeLinecap="round"
              strokeLinejoin="round"
              fill={LLENAS.has(a.kind) ? "transparent" : "none"}
              style={{ pointerEvents: coger ?? (LLENAS.has(a.kind) ? "all" : "stroke"), cursor: "move" }}
            >
              <Silueta anotacion={a} />
            </g>
          </g>
        ),
      )}
    </svg>
    {/*
      Los textos y los pasos van en HTML y no en el SVG: el SVG va estirado a 1000 x 1000, y
      ahí dentro un texto salía ensanchado y un paso, ovalado, en cuanto la captura no era
      cuadrada (la issue 19). Aquí miden lo que van a medir en el archivo, y el campo donde se
      escribe es la misma letra en el mismo sitio, así que al pulsar Intro no se mueve nada.
    */}
    {(textos.length > 0 || pasos.length > 0 || escribiendo) && (
      <div className="pointer-events-none absolute inset-0 overflow-hidden" style={{ containerType: "size" }}>
        {pasos.map(([a, i]) => (
          <span
            key={i}
            data-marca="step"
            data-agarre={i}
            onPointerDown={(e) => agarrar(e, i)}
            style={{
              ...estiloDelPaso(anchoImagen, a.x1, a.y1, escalaDe(a)),
              pointerEvents: coger ?? "auto",
              cursor: "move",
              userSelect: "none",
              ...(i === elegida ? { boxShadow: HALO_DEL_PASO } : {}),
            }}
          >
            <span style={estiloDelNumero(anchoImagen, a.color, escalaDe(a))}>{a.text}</span>
            {/* En el aro, a 45 grados: la esquina de su caja quedaría fuera del círculo. */}
            {asa(i) && <span className="absolute top-[85.4%] left-[85.4%]">{asa(i)}</span>}
          </span>
        ))}
        {textos.map(([a, i]) => (
          <span
            key={i}
            data-marca="text"
            data-agarre={i}
            onPointerDown={(e) => agarrar(e, i)}
            style={{
              ...letraDelTexto(anchoImagen, a.x1, a.y1, a.color, escalaDe(a)),
              pointerEvents: coger ?? "auto",
              cursor: "move",
              userSelect: "none",
              ...(i === elegida ? { filter: HALO_DEL_TEXTO } : {}),
            }}
          >
            {a.text}
            {asa(i) && <span className="absolute top-full left-full">{asa(i)}</span>}
          </span>
        ))}
        {escribiendo && (
          <input
            ref={campo}
            value={escribiendo.valor}
            onChange={(e) => {
              const valor = e.target.value;
              setEscribiendo((antes) => antes && { ...antes, valor });
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                guardarTexto();
              } else if (e.key === "Escape") {
                // Escape tira el texto a medias y nada más: que no le llegue al editor,
                // que con Escape suelta la herramienta o se cierra.
                e.preventDefault();
                e.stopPropagation();
                tirarTexto();
              }
            }}
            onBlur={guardarTexto}
            placeholder={t("Escribe aquí")}
            aria-label={t("Texto")}
            spellCheck={false}
            className="pointer-events-auto rounded-sm border-0 bg-transparent outline-[1.5px] outline-offset-2 outline-current outline-dashed placeholder:text-current placeholder:opacity-55"
            style={
              {
                ...letraDelTexto(
                  anchoImagen,
                  escribiendo.x,
                  escribiendo.y,
                  escribiendo.color,
                  // Uno que se reescribe, a su tamaño; uno nuevo, al elegido en la barra.
                  escribiendo.indice !== undefined && anotaciones[escribiendo.indice]
                    ? escalaDe(anotaciones[escribiendo.indice])
                    : escala,
                ),
                caretColor: escribiendo.color,
                // El campo crece con lo que se escribe, como el texto que va a quedar.
                fieldSizing: "content",
                minWidth: "1ch",
                margin: 0,
              } as CSSProperties
            }
          />
        )}
      </div>
    )}
    </Proporcion.Provider>
  );
}

/**
 * La letra de un texto en la vista previa, sacada como la saca Rust al exportar
 * (`encode::anotacion::texto`): Segoe UI seminegrita, con la celda a un cuarentaidosavo del
 * ancho de la imagen y entre 14 y 72 píxeles de ella, por su tamaño (`escala`), con dos de
 * aire, y metida dentro si se sale por la derecha o por abajo (`esquina_metida`).
 *
 * Va en unidades del contenedor para no medir la capa: 100 cqw es su ancho, que es el de
 * la imagen, así que un píxel de la imagen son `100 / anchoImagen` cqw a cualquier tamaño.
 */
export function letraDelTexto(
  anchoImagen: number,
  x: number,
  y: number,
  color: string,
  escala = 1,
): CSSProperties {
  const px = 100 / Math.max(1, anchoImagen);
  // El de siempre por su tamaño (issue 18), igual que `encode::anotacion::texto`.
  const celda = `clamp(${14 * px}cqw, ${100 / 42}cqw, ${72 * px}cqw)${escala === 1 ? "" : ` * ${escala}`}`;
  return {
    position: "absolute",
    left: `${x * 100}%`,
    top: `${y * 100}%`,
    translate: `min(0px, calc(${(1 - x) * 100}cqw - 100%)) min(0px, calc(${(1 - y) * 100}cqh - 100%))`,
    padding: `${2 * px}cqw`,
    fontFamily: '"Segoe UI", system-ui, sans-serif',
    fontWeight: 600,
    // GDI pide la letra por el alto de su celda, y CSS por el de la eme: en Segoe UI la
    // celda mide 1,33 emes.
    fontSize: `calc(${celda} / 1.33)`,
    lineHeight: 1.33,
    color,
    whiteSpace: "pre",
  };
}

/**
 * Lo que guarda el lápiz y la flecha curva además de las dos esquinas.
 *
 * El lápiz se queda con su trazo (aclarado al soltar, que es cuando ya no cambia), y la
 * curva con el punto de control que sale del camino: quien dibuja una curva no quiere
 * colocar puntos de control, quiere dibujar la curva.
 */
function conCamino(marca: Anotacion, camino: Punto[], final = false): Anotacion {
  if (marca.kind === "pen") {
    return { ...marca, puntos: final ? simplificarTrazo(camino) : camino };
  }
  if (marca.kind === "curve") {
    const extremos: Punto[] = [[marca.x1, marca.y1], ...camino.slice(1, -1), [marca.x2, marca.y2]];
    return { ...marca, puntos: [controlDeCurva(extremos)] };
  }
  return marca;
}

/**
 * El contorno de una marca, sin color ni grosor: lo pinta quien lo usa.
 *
 * Sirve para dos cosas que tienen que caer justo encima de lo dibujado: la silueta invisible
 * que se pulsa para coger la marca y el halo de la elegida. El texto y el paso no están,
 * porque van en HTML y se cogen y se marcan ellos solos.
 */
function Silueta({ anotacion: a }: { anotacion: Anotacion }) {
  const proporcion = useContext(Proporcion);
  const [x1, y1, x2, y2] = [a.x1 * 1000, a.y1 * 1000, a.x2 * 1000, a.y2 * 1000];
  const [izq, arr] = [Math.min(x1, x2), Math.min(y1, y2)];
  const [ancho, alto] = [Math.abs(x2 - x1), Math.abs(y2 - y1)];
  const ve = "non-scaling-stroke";
  const puntas = (desdeX: number, desdeY: number) =>
    puntaDeFlecha(desdeX, desdeY, x2, y2, proporcion).map(([px, py], i) => (
      <line key={i} x1={x2} y1={y2} x2={px} y2={py} vectorEffect={ve} />
    ));
  switch (a.kind) {
    case "box":
    case "highlight":
    case "blur":
    case "focus":
      return <rect x={izq} y={arr} width={ancho} height={alto} vectorEffect={ve} />;
    case "ellipse":
      return <ellipse cx={izq + ancho / 2} cy={arr + alto / 2} rx={ancho / 2} ry={alto / 2} vectorEffect={ve} />;
    case "line":
      return <line x1={x1} y1={y1} x2={x2} y2={y2} vectorEffect={ve} />;
    case "arrow":
      return (
        <>
          <line x1={x1} y1={y1} x2={x2} y2={y2} vectorEffect={ve} />
          {puntas(x1, y1)}
        </>
      );
    case "curve": {
      const [cx, cy] = a.puntos?.[0] ? [a.puntos[0][0] * 1000, a.puntos[0][1] * 1000] : [(x1 + x2) / 2, (y1 + y2) / 2];
      const [dx, dy] = Math.hypot(x2 - cx, y2 - cy) > 0.5 ? [cx, cy] : [x1, y1];
      return (
        <>
          <path d={`M ${x1} ${y1} Q ${cx} ${cy} ${x2} ${y2}`} vectorEffect={ve} />
          {puntas(dx, dy)}
        </>
      );
    }
    case "pen":
      return (
        <polyline
          points={(a.puntos?.length ? a.puntos : [[a.x1, a.y1], [a.x2, a.y2]])
            .map(([px, py]) => `${px * 1000},${py * 1000}`)
            .join(" ")}
          vectorEffect={ve}
        />
      );
    default:
      return null;
  }
}

/**
 * Cómo se ve la marca elegida: con un borde blanco pegado a su forma y una sombra corta por
 * fuera, como una pegatina levantada del papel. Sin recuadro alrededor ni asas en las
 * esquinas: las asas dirían que se puede estirar, y estas solo se mueven. El texto y el paso
 * sí se estiran (issues 18 y 19), y llevan su asa. El blanco se ve sobre lo oscuro y la
 * sombra sobre lo claro, así que se ve elegida sobre cualquier captura y con cualquiera de
 * los cinco colores.
 */
function Halo({ anotacion: a }: { anotacion: Anotacion }) {
  // Las manchas no tienen trazo: el halo va por su borde.
  const trazo = LLENAS.has(a.kind) ? 0 : GROSOR;
  return (
    <g data-halo="" fill="none" strokeLinecap="round" strokeLinejoin="round" pointerEvents="none">
      <g stroke="rgba(0, 0, 0, 0.45)" strokeWidth={trazo + 10}>
        <Silueta anotacion={a} />
      </g>
      <g stroke="#ffffff" strokeWidth={trazo + 6}>
        <Silueta anotacion={a} />
      </g>
    </g>
  );
}

/** El halo del texto elegido: el mismo borde blanco y la misma sombra, siguiendo las letras. */
const HALO_DEL_TEXTO =
  "drop-shadow(0 0 1px #ffffff) drop-shadow(0 0 1px #ffffff) drop-shadow(0 0 1.5px #ffffff) drop-shadow(0 1px 2px rgba(0, 0, 0, 0.55))";

/**
 * El halo del paso elegido: una raya oscura pegada a su aro blanco (si no, se fundirían), una
 * banda blanca y otra raya oscura por fuera, como el de las demás.
 */
const HALO_DEL_PASO = "0 0 0 2px rgba(0, 0, 0, 0.45), 0 0 0 5px #ffffff, 0 0 0 7px rgba(0, 0, 0, 0.45)";

/**
 * Lo que mide un paso en píxeles de la imagen, como lo pinta Rust (`encode::anotacion::paso`):
 * el disco a un veintiseisavo del ancho y entre 15 y 60 px, el aro blanco un 60 % del trazo
 * por fuera, y la celda del número 1,15 veces el radio. Todo por su tamaño (`escala`).
 *
 * Con las cuentas en `f32`, como Rust (`Math.fround` es redondear a eso): en doble precisión,
 * en una de 1300 de ancho la celda salía de 57 px, y en el archivo, de 58.
 */
export function medidaDelPaso(anchoImagen: number, escala = 1): { radio: number; aro: number; letra: number } {
  const f = Math.fround;
  const radio = f(Math.min(60, Math.max(15, f(anchoImagen / 26))) * f(escala));
  const grosor = Math.min(9, Math.max(3, Math.round(f(anchoImagen / 400))));
  const aro = f(radio + f(f(grosor * f(0.6)) * f(escala)));
  return { radio, aro, letra: Math.round(f(radio * f(1.15))) };
}

/**
 * La caja de un paso en la vista previa: el aro blanco, redondo de verdad porque se mide en
 * unidades del ancho del contenedor en las dos direcciones (100 cqw es el ancho de la imagen).
 *
 * Su centro se mete lo justo para que quepa entero, como al exportar
 * (`encode::anotacion::centro_metido`) y por la misma razón: un disco partido por el borde
 * no se lee, porque el número se va fuera y lo que queda es una mancha.
 */
export function estiloDelPaso(anchoImagen: number, x: number, y: number, escala = 1): CSSProperties {
  const px = 100 / Math.max(1, anchoImagen);
  const aro = medidaDelPaso(anchoImagen, escala).aro * px;
  return {
    position: "absolute",
    left: `clamp(${aro}cqw, ${x * 100}cqw, ${100 - aro}cqw)`,
    top: `clamp(${aro}cqw, ${y * 100}cqh, calc(100cqh - ${aro}cqw))`,
    width: `${aro * 2}cqw`,
    height: `${aro * 2}cqw`,
    translate: "-50% -50%",
    borderRadius: "50%",
    background: "#ffffff",
    display: "grid",
    placeItems: "center",
  };
}

/** El disco de color de dentro del aro, con su número en blanco: Segoe UI seminegrita. */
function estiloDelNumero(anchoImagen: number, color: string, escala: number): CSSProperties {
  const px = 100 / Math.max(1, anchoImagen);
  const { radio, letra } = medidaDelPaso(anchoImagen, escala);
  return {
    width: `${radio * 2 * px}cqw`,
    height: `${radio * 2 * px}cqw`,
    borderRadius: "50%",
    background: color,
    color: "#ffffff",
    display: "grid",
    placeItems: "center",
    fontFamily: '"Segoe UI", system-ui, sans-serif',
    fontWeight: 600,
    // La celda de GDI son 1,33 emes, como en el texto.
    fontSize: `${(letra / 1.33) * px}cqw`,
    lineHeight: 1,
  };
}

/** Una marca dibujada, en el sistema de 1000 x 1000 del SVG. */
function Marca({ anotacion: a }: { anotacion: Anotacion }) {
  const proporcion = useContext(Proporcion);
  const [x1, y1, x2, y2] = [a.x1 * 1000, a.y1 * 1000, a.x2 * 1000, a.y2 * 1000];
  const [izq, arr] = [Math.min(x1, x2), Math.min(y1, y2)];
  const [ancho, alto] = [Math.abs(x2 - x1), Math.abs(y2 - y1)];

  switch (a.kind) {
    case "box":
      return (
        <rect
          x={izq}
          y={arr}
          width={ancho}
          height={alto}
          fill="none"
          stroke={a.color}
          strokeWidth={6}
          vectorEffect="non-scaling-stroke"
        />
      );
    case "highlight":
      return <rect x={izq} y={arr} width={ancho} height={alto} fill={a.color} opacity={0.32} />;
    case "blur":
      // En la vista previa el difuminado se enseña como una zona tramada: el mosaico de
      // verdad lo hace Rust sobre los píxeles, y fingirlo aquí con un filtro daría una
      // idea equivocada de lo que va a salir.
      return (
        <g>
          <rect x={izq} y={arr} width={ancho} height={alto} fill="#0f0f12" opacity={0.82} />
          <rect
            x={izq}
            y={arr}
            width={ancho}
            height={alto}
            fill="none"
            stroke="#ffffff"
            strokeWidth={2}
            strokeDasharray="8 6"
            vectorEffect="non-scaling-stroke"
          />
        </g>
      );
    case "arrow":
      return (
        <g stroke={a.color} strokeWidth={6} fill="none" vectorEffect="non-scaling-stroke">
          <line x1={x1} y1={y1} x2={x2} y2={y2} vectorEffect="non-scaling-stroke" />
          {puntaDeFlecha(x1, y1, x2, y2, proporcion).map(([px, py], i) => (
            <line
              key={i}
              x1={x2}
              y1={y2}
              x2={px}
              y2={py}
              vectorEffect="non-scaling-stroke"
              strokeLinecap="round"
            />
          ))}
        </g>
      );
    case "curve": {
      // La misma cuadrática que pinta Rust (`punto_de_curva`): la `Q` del SVG es esa curva.
      // La punta mira desde el punto de control, que es hacia donde llega la curva.
      const [cx, cy] = a.puntos?.[0] ? [a.puntos[0][0] * 1000, a.puntos[0][1] * 1000] : [(x1 + x2) / 2, (y1 + y2) / 2];
      const [dx, dy] = Math.hypot(x2 - cx, y2 - cy) > 0.5 ? [cx, cy] : [x1, y1];
      return (
        <g data-marca="curve" stroke={a.color} strokeWidth={6} fill="none" strokeLinecap="round">
          <path d={`M ${x1} ${y1} Q ${cx} ${cy} ${x2} ${y2}`} vectorEffect="non-scaling-stroke" />
          {puntaDeFlecha(dx, dy, x2, y2, proporcion).map(([px, py], i) => (
            <line key={i} x1={x2} y1={y2} x2={px} y2={py} vectorEffect="non-scaling-stroke" />
          ))}
        </g>
      );
    }
    case "line":
      return (
        <line
          data-marca="line"
          x1={x1}
          y1={y1}
          x2={x2}
          y2={y2}
          stroke={a.color}
          strokeWidth={6}
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
      );
    case "ellipse":
      return (
        <ellipse
          data-marca="ellipse"
          cx={izq + ancho / 2}
          cy={arr + alto / 2}
          rx={ancho / 2}
          ry={alto / 2}
          fill="none"
          stroke={a.color}
          strokeWidth={6}
          vectorEffect="non-scaling-stroke"
        />
      );
    case "pen":
      return (
        <polyline
          data-marca="pen"
          points={(a.puntos?.length ? a.puntos : [[a.x1, a.y1], [a.x2, a.y2]])
            .map(([px, py]) => `${px * 1000},${py * 1000}`)
            .join(" ")}
          fill="none"
          stroke={a.color}
          strokeWidth={6}
          strokeLinecap="round"
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
        />
      );
    case "focus":
      // Lo pinta el velo de arriba, todos los focos juntos y por debajo de las demás.
      return null;
    case "text":
      // Lo pinta la capa de HTML de encima, a su tamaño de verdad.
      return null;
    case "step":
      // Lo pinta la capa de HTML de encima, redondo y a su tamaño de verdad (issue 19).
      return null;
    default:
      return null;
  }
}

/**
 * Lo que mide de alto la imagen por cada unidad de ancho (alto / ancho): cuánto estira el SVG
 * de 1000 x 1000 el eje vertical contra el horizontal.
 */
const Proporcion = createContext(1);

/**
 * Los dos extremos de la punta, a 0,52 rad (unos 30 grados) a cada lado de la línea, como los
 * pinta Rust (`encode::anotacion::punta`).
 *
 * Las cuentas van en píxeles de la imagen y no en el sistema de 1000 x 1000, que va estirado
 * (`preserveAspectRatio="none"`): ahí, en una captura apaisada una flecha horizontal salía con
 * la punta más cerrada y una vertical más abierta que en el archivo (lo avisó ProNotion en la
 * #19, «otras herramientas tienen fallos parecidos»). `proporcion` es alto / ancho de la imagen.
 * El largo es el de siempre en horizontal, un 4,8 % del ancho, a juego con el trazo de la vista
 * previa, que es más gordo que el del archivo para que se vea.
 */
function puntaDeFlecha(x1: number, y1: number, x2: number, y2: number, proporcion = 1): [number, number][] {
  // Con el eje vertical en las mismas unidades que el horizontal, los ángulos son los de verdad.
  const angulo = Math.atan2((y2 - y1) * proporcion, x2 - x1);
  const largo = 48;
  return [-0.52, 0.52].map((lado) => {
    const a = angulo + Math.PI + lado;
    return [x2 + Math.cos(a) * largo, y2 + (Math.sin(a) * largo) / proporcion] as [number, number];
  });
}
