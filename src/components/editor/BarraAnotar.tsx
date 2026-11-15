import {
  ArrowUpRight,
  ChevronDown,
  Circle,
  Crop,
  Focus,
  Highlighter,
  Pencil,
  ShieldCheck,
  Slash,
  Square,
  TextSelect,
  Type,
  Undo2,
  X,
} from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { COLORES, type Herramienta } from "../../lib/anotaciones";
import { useT } from "../../lib/i18n";
import { TECLA_COPIAR_TEXTO, TECLA_DE, TECLA_TAPAR_DATOS, conTecla } from "./teclas";

/**
 * Lo que se hace sobre la imagen: dibujar, encuadrar, tapar y leer su texto.
 *
 * Va ARRIBA de la vista previa, y el tiempo abajo: así cada cosa vive donde se busca, y el
 * vídeo queda pegado a su línea de tiempo. Antes iba entre los dos, y el encuadre flotaba
 * encima de la imagen en una pastilla con los mandos del tiempo.
 *
 * Las herramientas de dibujar van solo con su icono, que es como las reconoce cualquiera,
 * y su nombre y su tecla al pasar el ratón. Lo demás lleva su nombre escrito: «encuadrar»,
 * «copiar texto» o «tapar datos» no tienen un icono que se entienda solo.
 *
 * En UNA fila (S39, segunda vuelta). En la primera, para que cupiera, «Copiar texto» y «Tapar
 * datos» se quedaron en un icono suelto que no se adivinaba. Ahora lo que se encoge primero es
 * lo que se reconoce sin palabras: con la barra por debajo de 64rem (una ventana de menos de
 * 1.348 px, o sea casi siempre), los botones de dibujar pasan a 28 px de ancho y los cinco
 * colores se pliegan en uno que los abre. Solo en una ventana de portátil (la barra por debajo
 * de 50rem) «Encuadrar» y «Copiar texto» se quedan en su icono; «Tapar datos», nunca. Las
 * medidas son del hueco de dentro de la barra, sin sus 12 px de relleno a cada lado.
 */
interface Props {
  activa: Herramienta | null;
  onElegir: (herramienta: Herramienta | null) => void;
  color: string;
  onColor: (color: string) => void;
  cuantas: number;
  /**
   * Si hay algo que deshacer. No es lo mismo que haber marcas: con todas quitadas se puede
   * deshacer el quitarlas, y una marca movida se puede devolver a su sitio.
   */
  puedeDeshacer?: boolean;
  onDeshacer: () => void;
  onBorrarTodo: () => void;
  /** Tapar de una vez los datos personales que haya en la imagen. */
  onTaparDatos: () => void;
  /** Copiar el texto que hay escrito en la imagen. */
  onCopiarTexto: () => void;
  /** Mientras el lector de texto trabaja, que son unas decimas sobre una captura grande. */
  buscandoDatos: boolean;
  /** Si el marco de encuadrar está puesto para moverlo. */
  encuadrando: boolean;
  /** Lo que mide el trozo encuadrado («640 × 360»), o null si se exporta entera. */
  encuadre: string | null;
  onEncuadrar: () => void;
  onQuitarEncuadre: () => void;
}

/**
 * Las que señalan, en el orden de la barra: cada una al lado de su pariente (la flecha con
 * la curva y la línea, el rectángulo con la elipse) para que se encuentren sin leer. Sus
 * teclas, en `teclas.ts`.
 */
const HERRAMIENTAS: {
  id: Herramienta;
  icono: React.ComponentType<{ className?: string }>;
  etiqueta: string;
}[] = [
  { id: "arrow", icono: ArrowUpRight, etiqueta: "Flecha" },
  { id: "curve", icono: FlechaCurva, etiqueta: "Flecha curva" },
  { id: "line", icono: Slash, etiqueta: "Línea" },
  { id: "box", icono: Square, etiqueta: "Rectángulo" },
  { id: "ellipse", icono: Circle, etiqueta: "Elipse" },
  { id: "pen", icono: Pencil, etiqueta: "Lápiz" },
  { id: "text", icono: Type, etiqueta: "Texto" },
  { id: "highlight", icono: Highlighter, etiqueta: "Resaltar" },
  // El paso numerado va con los que señalan: es la marca de «primero esto, luego esto».
  { id: "step", icono: Paso, etiqueta: "Paso numerado" },
  // El foco también señala, aunque lo haga oscureciendo lo demás: «mira aquí».
  { id: "focus", icono: Focus, etiqueta: "Foco" },
];

/** Las que no se pintan de un color: el mosaico tapa y el foco oscurece. */
const SIN_COLOR: (Herramienta | null)[] = ["blur", "focus"];

export function BarraAnotar({
  activa,
  onElegir,
  color,
  onColor,
  cuantas,
  puedeDeshacer = cuantas > 0,
  onDeshacer,
  onBorrarTodo,
  onTaparDatos,
  onCopiarTexto,
  buscandoDatos,
  encuadrando,
  encuadre,
  onEncuadrar,
  onQuitarEncuadre,
}: Props) {
  const t = useT();
  return (
    // El `flex-wrap` se queda solo para una ventana estrechísima, donde partirse es mejor que
    // salirse.
    <div
      data-barra-anotar
      className="@container flex shrink-0 flex-wrap items-center gap-0.5 border-b border-linea px-3 py-1.5"
    >
      {HERRAMIENTAS.map((h) => (
        <Boton
          key={h.id}
          icono={h.icono}
          etiqueta={t(h.etiqueta)}
          tecla={TECLA_DE[h.id]}
          dibujo
          activa={activa === h.id}
          onClick={() => onElegir(activa === h.id ? null : h.id)}
        />
      ))}

      {/* El color no sale con el mosaico ni con el foco: ahí no pinta nada. */}
      {!SIN_COLOR.includes(activa) && (
        <>
          <Raya />
          {/* Los cinco a la vista con la barra ancha; si no, plegados en uno. */}
          <span className="hidden gap-1.5 px-0.5 @[64rem]:flex">
            {COLORES.map((c) => (
              <PuntoDeColor key={c} color={c} puesto={color === c} onClick={() => onColor(c)} />
            ))}
          </span>
          <span className="@[64rem]:hidden">
            <ColoresPlegados color={color} onColor={onColor} />
          </span>
        </>
      )}

      {/*
        El texto se escribe encima de la imagen, donde se pulsa («CapaAnotaciones»). Antes se
        escribía aquí y se pegaba con un clic, y lo escrito se quedaba en la barra para el
        siguiente: no se veía dónde iba a caer ni cómo iba a quedar hasta soltarlo. El aviso
        solo cabe con la barra ancha; si no, partiría la barra en dos filas.
      */}
      {activa === "text" && (
        <span className="hidden px-1 text-[13px] whitespace-nowrap text-texto @[64rem]:inline">
          {t("Pulsa en la imagen y escribe")}
        </span>
      )}

      <Raya />

      {/*
        Encuadrar es quedarse con un trozo de la IMAGEN; recortar, en la línea de tiempo,
        con un trozo del TIEMPO. Antes los dos se llamaban «recortar», y en un vídeo no se
        sabía cuál de los dos iba a pasar.
      */}
      <BotonConNombre
        icono={Crop}
        nombre={encuadre ?? t("Encuadrar")}
        titulo={t("Quedarse con un trozo de la imagen (C)")}
        tecla="C"
        // Con el recorte puesto, su medida se ve siempre: es lo que va a salir.
        estrecho={encuadre === null}
        activa={encuadrando || encuadre !== null}
        onClick={onEncuadrar}
      />
      {encuadre !== null && (
        <Boton icono={X} etiqueta={t("Quitar el recorte")} activa={false} onClick={onQuitarEncuadre} />
      )}

      <span className="ml-auto flex items-center gap-0.5">
        {/*
          Tapar, a mano y solo, juntos: el mosaico (lo que se arrastra encima de un trozo) iba
          suelto entre dos rayas con un icono de puntitos que parecía una almohadilla, lejos de
          «Tapar datos», que es lo mismo hecho solo. Sigue lejos de las que señalan, que hacen lo
          contrario: mezclado con ellas se pulsaba por error.
        */}
        <Boton
          icono={Mosaico}
          etiqueta={t("Tapar con mosaico")}
          tecla={TECLA_DE.blur}
          activa={activa === "blur"}
          onClick={() => onElegir(activa === "blur" ? null : "blur")}
        />
        {/*
          Tapar datos es el mosaico hecho solo: encuentra los correos, los teléfonos, las
          tarjetas, las cuentas y los DNI de la imagen y deja un mosaico encima de cada uno. Lo
          que deja son marcas normales, así que se ven antes de exportar y se quitan con Ctrl+Z.
        */}
        <BotonConNombre
          icono={ShieldCheck}
          nombre={t("Tapar datos")}
          titulo={t("Tapar datos personales")}
          tecla={TECLA_TAPAR_DATOS}
          estrecho={false}
          activa={false}
          onClick={onTaparDatos}
          apagada={buscandoDatos}
        />
        {/*
          Leer el texto de la imagen ya existía en la captura, con la tecla T, y se quedaba
          fuera del editor: para copiar una frase de una captura que ya estabas editando
          había que salir y volver a capturar lo mismo. De un vídeo, directamente no había
          forma.
        */}
        <BotonConNombre
          icono={TextSelect}
          nombre={t("Copiar texto")}
          titulo={t("Copiar el texto de la imagen")}
          tecla={TECLA_COPIAR_TEXTO}
          estrecho
          activa={false}
          onClick={onCopiarTexto}
          apagada={buscandoDatos}
        />
        <Raya />
        <Boton
          icono={Undo2}
          etiqueta={t("Deshacer (Ctrl+Z)")}
          atajo="Control+Z"
          activa={false}
          onClick={onDeshacer}
          apagada={!puedeDeshacer}
        />
        <Boton
          icono={X}
          etiqueta={t("Quitar todas")}
          tecla={t("Mayús+Supr")}
          atajo="Shift+Delete"
          activa={false}
          onClick={onBorrarTodo}
          apagada={cuantas === 0}
        />
      </span>
    </div>
  );
}

function Raya() {
  return <span className="mx-1.5 h-5 w-px bg-linea-fuerte @max-[64rem]:mx-1" />;
}

function Boton({
  icono: Icono,
  etiqueta,
  tecla,
  atajo,
  dibujo = false,
  activa,
  onClick,
  apagada = false,
}: {
  /** Uno de lucide o el que se dibuja aquí abajo: los dos pintan un SVG y ya está. */
  icono: React.ComponentType<{ className?: string }>;
  /** El nombre, sin la tecla: es como lo encuentra un lector de pantalla. */
  etiqueta: string;
  /** La tecla, que se enseña al pasar el ratón junto al nombre: «Flecha (1)». */
  tecla?: string;
  /** La tecla como la escribe la web (`aria-keyshortcuts`), si no es la misma que `tecla`. */
  atajo?: string;
  /** Las de dibujar, que son las que estrechan primero con la barra justa. */
  dibujo?: boolean;
  activa: boolean;
  onClick: () => void;
  apagada?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={apagada}
      title={conTecla(etiqueta, tecla)}
      aria-label={etiqueta}
      aria-keyshortcuts={atajo ?? tecla}
      aria-pressed={activa}
      className={`flex size-8 shrink-0 items-center justify-center rounded-md transition-colors disabled:opacity-30 ${
        dibujo ? "@max-[64rem]:w-7" : ""
      } ${activa ? "bg-[#0a76c9] text-white" : "text-suave hover:bg-realce hover:text-titulo"}`}
    >
      <Icono className="size-[18px]" />
    </button>
  );
}

function BotonConNombre({
  icono: Icono,
  nombre,
  titulo,
  tecla,
  activa,
  onClick,
  apagada = false,
  estrecho,
}: {
  icono: React.ComponentType<{ className?: string }>;
  nombre: string;
  titulo: string;
  tecla: string;
  /**
   * Si se queda en su icono con la barra por debajo de 50rem de hueco (una ventana de menos de
   * 1.124 px con el panel, como la de 1.080 de un portátil de 1.366). Su nombre sigue saliendo
   * al pasar el ratón.
   */
  estrecho: boolean;
  activa: boolean;
  onClick: () => void;
  apagada?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={apagada}
      // El título ya lleva su tecla cuando se escribe a mano («(C)»), que es lo que traduce.
      title={titulo.endsWith(")") ? titulo : conTecla(titulo, tecla)}
      aria-label={nombre}
      aria-keyshortcuts={tecla}
      aria-pressed={activa}
      className={`flex h-8 shrink-0 items-center gap-1.5 rounded-md px-2 text-[13px] font-medium whitespace-nowrap tabular-nums transition-colors disabled:opacity-30 @max-[64rem]:px-1.5 ${
        activa ? "bg-[#0a76c9] text-white" : "text-texto hover:bg-realce hover:text-titulo"
      }`}
    >
      <Icono className="size-[18px]" />
      <span className={estrecho ? "@max-[50rem]:hidden" : undefined}>{nombre}</span>
    </button>
  );
}

function PuntoDeColor({ color, puesto, onClick }: { color: string; puesto: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={color}
      aria-label={color}
      aria-pressed={puesto}
      style={{ background: color }}
      className={`size-[22px] shrink-0 rounded-full border-2 transition-colors ${
        puesto ? "border-titulo" : "border-linea-fuerte hover:border-linea-viva"
      }`}
    />
  );
}

/**
 * Los cinco colores plegados en uno, para la barra justa: el puesto, con su flecha, y al
 * pulsarlo los cinco debajo. Elegir uno lo cierra, y también pulsar fuera o Escape (que no
 * llega al editor: ahí cerraría la ventana).
 */
function ColoresPlegados({ color, onColor }: { color: string; onColor: (color: string) => void }) {
  const t = useT();
  const [abierto, setAbierto] = useState(false);
  const caja = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!abierto) return;
    const fuera = (e: PointerEvent) => {
      if (!caja.current?.contains(e.target as Node)) setAbierto(false);
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopImmediatePropagation();
      setAbierto(false);
    };
    document.addEventListener("pointerdown", fuera);
    window.addEventListener("keydown", escape, true);
    return () => {
      document.removeEventListener("pointerdown", fuera);
      window.removeEventListener("keydown", escape, true);
    };
  }, [abierto]);
  return (
    <span ref={caja} className="relative flex">
      <button
        type="button"
        onClick={() => setAbierto((a) => !a)}
        title={t("Color")}
        aria-label={t("Color")}
        aria-haspopup="true"
        aria-expanded={abierto}
        className="flex h-8 items-center gap-0.5 rounded-md px-1 text-suave transition-colors hover:bg-realce hover:text-titulo"
      >
        <span style={{ background: color }} className="size-5 rounded-full border-2 border-linea-fuerte" />
        <ChevronDown className={`size-3.5 transition-transform duration-150 ${abierto ? "rotate-180" : ""}`} />
      </button>
      {abierto && (
        <span
          role="group"
          aria-label={t("Color")}
          className="absolute top-full left-0 z-30 mt-1.5 flex gap-1.5 rounded-lg border border-linea bg-flotante p-2 shadow-xl shadow-black/40"
        >
          {COLORES.map((c) => (
            <PuntoDeColor
              key={c}
              color={c}
              puesto={color === c}
              onClick={() => {
                onColor(c);
                setAbierto(false);
              }}
            />
          ))}
        </span>
      )}
    </span>
  );
}

/**
 * El icono de la flecha curva: un arco con su punta. El `Spline` de lucide son dos puntos
 * de control unidos, que dice «curva de Bézier» a quien sabe qué es eso y nada a los demás.
 */
function FlechaCurva({ className = "size-4" }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      className={className}
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M5 19C5 10 10 6 18 6" />
      <path d="M14 3l4 3-3 4" />
    </svg>
  );
}

/**
 * El icono del paso numerado: un disco con un 1. Antes era el círculo de lucide, pero ese
 * es ahora el de la elipse, y dos botones con el mismo dibujo no se distinguen.
 */
function Paso({ className = "size-4" }: { className?: string }) {
  // El 1 va RECORTADO del disco, no pintado encima: pintado en negro fijo, en el tema claro
  // (disco oscuro) no se veía, y en blanco no se vería con la herramienta puesta (icono blanco).
  const id = `paso${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <mask id={id}>
        <rect width="24" height="24" fill="white" />
        <path
          d="M10.5 9.5L12.5 8v8"
          fill="none"
          stroke="black"
          strokeWidth={2}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </mask>
      <circle cx="12" cy="12" r="9" fill="currentColor" mask={`url(#${id})`} />
    </svg>
  );
}

/**
 * El icono del mosaico: un cuadro pixelado, a cuadros, como se tapa una cara en la tele.
 *
 * Dibujado a mano porque ninguno de los de lucide dice «mosaico»: el de gota dice
 * «desenfoque», y esto no desenfoca, tira la información. El de antes era una rejilla de 4 por
 * 4 puntitos sueltos que, a 18 px, se leía como una almohadilla («el #»).
 */
function Mosaico({ className = "size-4" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <rect x="3" y="3" width="18" height="18" rx="2" fill="none" stroke="currentColor" strokeWidth={2} />
      <path
        fill="currentColor"
        d="M4 4h5.33v5.33H4zM14.67 4H20v5.33h-5.33zM9.33 9.33h5.34v5.34H9.33zM4 14.67h5.33V20H4zM14.67 14.67H20V20h-5.33z"
      />
    </svg>
  );
}
