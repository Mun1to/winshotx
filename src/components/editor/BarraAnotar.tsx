import {
  ArrowUpRight,
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
import { COLORES, type Herramienta } from "../../lib/anotaciones";
import { useT } from "../../lib/i18n";

/**
 * Lo que se hace sobre la imagen: dibujar, encuadrar, leer su texto y tapar datos.
 *
 * Va ARRIBA de la vista previa, y el tiempo abajo: así cada cosa vive donde se busca, y el
 * vídeo queda pegado a su línea de tiempo. Antes iba entre los dos, y el encuadre flotaba
 * encima de la imagen en una pastilla con los mandos del tiempo.
 *
 * El difuminado lleva su propio icono y su propio sitio, separado de los demás por una
 * raya: los otros cuatro señalan algo y este **tapa** algo, que es lo contrario. Mezclarlo
 * en la fila hacía que se pulsara por error.
 *
 * Las herramientas de dibujar van solo con su icono, que es como las reconoce cualquiera,
 * y su nombre al pasar el ratón. Lo demás lleva su nombre escrito: «encuadrar», «copiar
 * texto» o «tapar datos» no tienen un icono que se entienda solo.
 */
interface Props {
  activa: Herramienta | null;
  onElegir: (herramienta: Herramienta | null) => void;
  color: string;
  onColor: (color: string) => void;
  texto: string;
  onTexto: (texto: string) => void;
  cuantas: number;
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
 * la curva y la línea, el rectángulo con la elipse) para que se encuentren sin leer.
 *
 * Las teclas del 1 al 6 son las de siempre y las pone el editor (`EditorApp.tsx`); las
 * marcas que entraron el 11 de noviembre no tienen tecla todavía.
 */
const HERRAMIENTAS: {
  id: Herramienta;
  icono: React.ComponentType<{ className?: string }>;
  etiqueta: string;
  tecla?: string;
}[] = [
  { id: "arrow", icono: ArrowUpRight, etiqueta: "Flecha", tecla: "1" },
  { id: "curve", icono: FlechaCurva, etiqueta: "Flecha curva" },
  { id: "line", icono: Slash, etiqueta: "Línea" },
  { id: "box", icono: Square, etiqueta: "Rectángulo", tecla: "2" },
  { id: "ellipse", icono: Circle, etiqueta: "Elipse" },
  { id: "pen", icono: Pencil, etiqueta: "Lápiz" },
  { id: "text", icono: Type, etiqueta: "Texto", tecla: "3" },
  { id: "highlight", icono: Highlighter, etiqueta: "Resaltar", tecla: "4" },
  // El paso numerado va con los que señalan: es la marca de «primero esto, luego esto».
  { id: "step", icono: Paso, etiqueta: "Paso numerado", tecla: "5" },
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
  texto,
  onTexto,
  cuantas,
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
    <div className="@container flex shrink-0 flex-wrap items-center gap-1 border-b border-white/8 bg-black/25 px-3 py-2">
      {HERRAMIENTAS.map((h) => (
        <Boton
          key={h.id}
          icono={h.icono}
          etiqueta={h.tecla ? `${t(h.etiqueta)} · ${h.tecla}` : t(h.etiqueta)}
          activa={activa === h.id}
          onClick={() => onElegir(activa === h.id ? null : h.id)}
        />
      ))}

      <Raya />

      {/* Tapar va aparte: los otros cuatro señalan, este esconde. */}
      <Boton
        icono={Difuminar}
        etiqueta={`${t("Mosaico")} · 6`}
        activa={activa === "blur"}
        onClick={() => onElegir(activa === "blur" ? null : "blur")}
      />

      {/* El color no sale con el mosaico ni con el foco: ahí no pinta nada. */}
      {!SIN_COLOR.includes(activa) && (
        <>
          <Raya />
          <span className="flex gap-1.5 px-1">
            {COLORES.map((c) => (
              <button
                key={c}
                type="button"
                onClick={() => onColor(c)}
                title={c}
                aria-label={c}
                aria-pressed={color === c}
                style={{ background: c }}
                className={`size-6 rounded-full border-2 transition-colors ${
                  color === c ? "border-white" : "border-white/20 hover:border-white/50"
                }`}
              />
            ))}
          </span>
        </>
      )}

      {/* El campo solo aparece con el texto elegido: es lo único que se escribe. */}
      {activa === "text" && (
        <input
          value={texto}
          onChange={(e) => onTexto(e.target.value)}
          placeholder={t("Escribe y pulsa en la imagen")}
          className="h-[34px] min-w-[200px] flex-1 rounded-md border border-white/10 bg-black/40 px-2.5 text-[13px] text-neutral-100 placeholder:text-neutral-500 focus:border-blue-500/70 focus:outline-none"
        />
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
        activa={encuadrando || encuadre !== null}
        onClick={onEncuadrar}
      />
      {encuadre !== null && (
        <Boton icono={X} etiqueta={t("Quitar el recorte")} activa={false} onClick={onQuitarEncuadre} />
      )}

      <span className="ml-auto flex items-center gap-1">
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
          estrecho
          activa={false}
          onClick={onCopiarTexto}
          apagada={buscandoDatos}
        />
        {/*
          Tapar datos es el difuminado hecho solo: encuentra los correos, los teléfonos,
          las tarjetas, las cuentas y los DNI de la imagen y deja un mosaico encima de cada
          uno. Lo que deja son marcas normales, así que se ven antes de exportar y se
          quitan con Ctrl+Z.
        */}
        <BotonConNombre
          icono={ShieldCheck}
          nombre={t("Tapar datos")}
          titulo={t("Tapar datos personales")}
          estrecho
          activa={false}
          onClick={onTaparDatos}
          apagada={buscandoDatos}
        />
        <Raya />
        <Boton
          icono={Undo2}
          etiqueta={t("Deshacer la última (Ctrl+Z)")}
          activa={false}
          onClick={onDeshacer}
          apagada={cuantas === 0}
        />
        <Boton
          icono={X}
          etiqueta={t("Quitar todas")}
          activa={false}
          onClick={onBorrarTodo}
          apagada={cuantas === 0}
        />
      </span>
    </div>
  );
}

function Raya() {
  return <span className="mx-1 h-5 w-px bg-white/10" />;
}

function Boton({
  icono: Icono,
  etiqueta,
  activa,
  onClick,
  apagada = false,
}: {
  /** Uno de lucide o el que se dibuja aquí abajo: los dos pintan un SVG y ya está. */
  icono: React.ComponentType<{ className?: string }>;
  etiqueta: string;
  activa: boolean;
  onClick: () => void;
  apagada?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={apagada}
      title={etiqueta}
      aria-label={etiqueta}
      aria-pressed={activa}
      className={`flex size-[34px] items-center justify-center rounded-md transition-colors disabled:opacity-30 ${
        activa ? "bg-blue-500 text-white" : "text-neutral-300 hover:bg-white/10 hover:text-white"
      }`}
    >
      <Icono className="size-[18px]" />
    </button>
  );
}

function BotonConNombre({
  icono: Icono,
  nombre,
  titulo,
  activa,
  onClick,
  apagada = false,
  estrecho = false,
}: {
  icono: React.ComponentType<{ className?: string }>;
  nombre: string;
  titulo: string;
  /** Si en una ventana estrecha se queda solo con el icono. */
  estrecho?: boolean;
  activa: boolean;
  onClick: () => void;
  apagada?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={apagada}
      title={titulo}
      aria-label={nombre}
      aria-pressed={activa}
      className={`flex h-[34px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium whitespace-nowrap tabular-nums transition-colors disabled:opacity-30 ${
        activa ? "bg-blue-500 text-white" : "text-neutral-200 hover:bg-white/10 hover:text-white"
      }`}
    >
      <Icono className="size-[18px]" />
      {/* En una ventana estrecha se queda el icono, con su nombre al pasar el ratón: mejor
          eso que la barra partida en dos filas comiéndose la imagen. */}
      <span className={estrecho ? "hidden @[52rem]:inline" : undefined}>{nombre}</span>
    </button>
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
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <circle cx="12" cy="12" r="9" fill="currentColor" />
      <path
        d="M10.5 9.5L12.5 8v8"
        fill="none"
        stroke="var(--color-neutral-900, #171717)"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/**
 * El icono de tapar datos: un cuadrado hecho de cuadraditos.
 *
 * Dibujado a mano porque ninguno de los de lucide dice «mosaico»: el de gota dice
 * «desenfoque», y esto no desenfoca, tira la información.
 */
function Difuminar({ className = "size-4" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="currentColor" aria-hidden="true">
      {[4, 9, 14, 19].map((y) =>
        [4, 9, 14, 19].map((x) => (
          <rect
            key={`${x}-${y}`}
            x={x - 1.6}
            y={y - 1.6}
            width={3.2}
            height={3.2}
            rx={0.6}
            opacity={(x + y) % 10 === 3 ? 0.35 : (x * y) % 7 === 0 ? 0.9 : 0.6}
          />
        )),
      )}
    </svg>
  );
}
