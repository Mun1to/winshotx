import { ArrowUpRight, Circle, Highlighter, Square, Type, Undo2, X } from "lucide-react";
import { COLORES, type Herramienta } from "../../lib/anotaciones";
import { useT } from "../../lib/i18n";

/**
 * Las seis herramientas de anotar, el color y deshacer.
 *
 * Va debajo de la vista previa, junto a los controles de reproducción, y no en una columna
 * aparte: una barra lateral más se lleva doscientos píxeles de ancho, que es justo lo que
 * hace falta para ver la captura.
 *
 * El difuminado lleva su propio icono y su propio sitio, separado de los demás por una
 * raya: los otros cuatro señalan algo y este **tapa** algo, que es lo contrario. Mezclarlo
 * en la fila hacía que se pulsara por error.
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
}

const HERRAMIENTAS: {
  id: Herramienta;
  icono: React.ComponentType<{ className?: string }>;
  etiqueta: string;
  tecla: string;
}[] = [
  { id: "arrow", icono: ArrowUpRight, etiqueta: "Flecha", tecla: "1" },
  { id: "box", icono: Square, etiqueta: "Rectángulo", tecla: "2" },
  { id: "text", icono: Type, etiqueta: "Texto", tecla: "3" },
  { id: "highlight", icono: Highlighter, etiqueta: "Resaltar", tecla: "4" },
  // El paso numerado va con los que señalan: es la marca de «primero esto, luego esto».
  { id: "step", icono: Circle, etiqueta: "Paso numerado", tecla: "5" },
];

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
}: Props) {
  const t = useT();
  return (
    <div className="flex flex-wrap items-center gap-1.5 border-t border-white/8 bg-black/25 px-3 py-2">
      {HERRAMIENTAS.map((h) => (
        <Boton
          key={h.id}
          icono={h.icono}
          etiqueta={`${t(h.etiqueta)} · ${h.tecla}`}
          activa={activa === h.id}
          onClick={() => onElegir(activa === h.id ? null : h.id)}
        />
      ))}

      <span className="mx-0.5 h-5 w-px bg-white/10" />

      {/* Tapar datos va aparte: los otros cuatro señalan, este esconde. */}
      <Boton
        icono={Difuminar}
        etiqueta={`${t("Tapar datos")} · 6`}
        activa={activa === "blur"}
        onClick={() => onElegir(activa === "blur" ? null : "blur")}
      />

      <span className="mx-0.5 h-5 w-px bg-white/10" />

      {/* El color no sale con el difuminado: ahí no pinta nada. */}
      {activa !== "blur" && (
        <span className="flex gap-1">
          {COLORES.map((c) => (
            <button
              key={c}
              type="button"
              onClick={() => onColor(c)}
              title={c}
              aria-label={c}
              aria-pressed={color === c}
              style={{ background: c }}
              className={`size-5 rounded-full border-2 transition-colors ${
                color === c ? "border-white" : "border-white/20 hover:border-white/50"
              }`}
            />
          ))}
        </span>
      )}

      {/* El campo solo aparece con el texto elegido: es lo único que se escribe. */}
      {activa === "text" && (
        <input
          value={texto}
          onChange={(e) => onTexto(e.target.value)}
          placeholder={t("Escribe y pulsa en la imagen")}
          className="h-7 min-w-[190px] flex-1 rounded-md border border-white/10 bg-black/40 px-2 text-[12px] text-neutral-200 placeholder:text-neutral-600 focus:border-blue-500/70 focus:outline-none"
        />
      )}

      <span className="ml-auto flex items-center gap-1">
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
      className={`flex size-7 items-center justify-center rounded-md transition-colors disabled:opacity-30 ${
        activa ? "bg-blue-500 text-white" : "text-neutral-400 hover:bg-white/10 hover:text-white"
      }`}
    >
      <Icono className="size-4" />
    </button>
  );
}

/**
 * El icono de tapar datos: un cuadrado hecho de cuadraditos.
 *
 * Dibujado a mano porque ninguno de los de lucide dice «mosaico»: el de gota dice
 * «desenfoque», y esto no desenfoca, tira la información.
 */
function Difuminar() {
  return (
    <svg viewBox="0 0 24 24" className="size-4" fill="currentColor" aria-hidden="true">
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
