import type { ReactNode } from "react";
import { Slider } from "../ui/Slider";
import { Toggle } from "../ui/Toggle";
import type { Forma, TramoZoom } from "../../lib/contrato";
import { LUPA_NUEVA, NIVEL_MAXIMO } from "../../lib/zooms";
import { VELOCIDADES_DE_TROZO, type Pieza } from "../../lib/tramo";
import { useT } from "../../lib/i18n";
import { Opcion } from "../ui/Opcion";

/**
 * Los mandos de lo que está elegido en la línea de tiempo: un bloque de la pista de zooms
 * o el trozo que hay bajo el cabezal.
 *
 * Van debajo de la línea de tiempo, en el sitio de la frase de ayuda, y no en una ventana
 * encima: así no tapan la imagen, que es donde se coloca el punto de un zoom fijo.
 */

/** Las cuatro formas de la lupa, en el orden en que se ofrecen. */
export const FORMAS: { forma: Forma; nombre: string; corto: string }[] = [
  { forma: "circulo", nombre: "Círculo", corto: "Círculo" },
  { forma: "cuadrado", nombre: "Cuadrado", corto: "Cuadrado" },
  { forma: "rectangulo", nombre: "Rectángulo", corto: "Rectángulo" },
  { forma: "redondeado", nombre: "Rectángulo redondeado", corto: "Redondeado" },
];

/**
 * Elegir la forma (de la lupa o de la cámara): cada una con su dibujo Y su nombre, y la
 * elegida subrayada como el resto de opciones del editor. Con solo los cuatro dibujos
 * finos, sin nombre, no se veía que eran botones: Munito, 12-nov, «¿solo está la opción de
 * círculo para la cámara?».
 */
export function SelectorDeForma({
  forma,
  onForma,
  etiqueta,
  enDosFilas = false,
}: {
  forma: Forma;
  onForma: (forma: Forma) => void;
  etiqueta: string;
  /** En el panel, que es estrecho, dos por fila; debajo de la línea de tiempo, las cuatro seguidas. */
  enDosFilas?: boolean;
}) {
  const t = useT();
  return (
    <div
      role="group"
      aria-label={etiqueta}
      className={enDosFilas ? "grid grid-cols-2 justify-items-start gap-x-4 gap-y-1" : "flex items-center gap-4"}
    >
      {FORMAS.map((f) => (
        <Opcion key={f.forma} puesta={forma === f.forma} onClick={() => onForma(f.forma)} etiqueta={t(f.nombre)}>
          <span className="flex items-center gap-1.5">
            <DibujoDeForma forma={f.forma} puesta={forma === f.forma} />
            {t(f.corto)}
          </span>
        </Opcion>
      ))}
    </div>
  );
}

export function AjustesDelZoom({
  zoom,
  onCambio,
  onQuitar,
  onListo,
}: {
  zoom: TramoZoom;
  onCambio: (cambio: Partial<TramoZoom>) => void;
  onQuitar: () => void;
  onListo: () => void;
}) {
  const t = useT();
  const lupa = zoom.lupa ?? null;
  return (
    <div data-ajustes-zoom className="flex flex-wrap items-center gap-x-6 gap-y-2">
      <div role="group" aria-label={t("Qué hace el bloque")} className="flex items-center gap-4">
        <Opcion puesta={!lupa} onClick={() => onCambio({ lupa: null })}>
          {t("Zoom")}
        </Opcion>
        <Opcion puesta={lupa !== null} onClick={() => onCambio({ lupa: lupa ?? LUPA_NUEVA })}>
          {t("Lupa")}
        </Opcion>
      </div>

      {lupa ? (
        <>
          <SelectorDeForma
            forma={lupa.forma}
            onForma={(forma) => onCambio({ lupa: { ...lupa, forma } })}
            etiqueta={t("Forma de la lupa")}
          />
          <div className="w-[120px]">
            <Slider
              label={t("Tamaño")}
              hint={`${Math.round(lupa.tam * 100)} %`}
              min={10}
              max={60}
              step={1}
              value={Math.round(lupa.tam * 100)}
              onChange={(v) => onCambio({ lupa: { ...lupa, tam: v / 100 } })}
            />
          </div>
          <div className="w-[120px]">
            <Slider
              label={t("Aumento")}
              hint={`${lupa.aumento.toFixed(1)}×`}
              min={1.5}
              max={5}
              step={0.1}
              value={lupa.aumento}
              onChange={(v) => onCambio({ lupa: { ...lupa, aumento: v } })}
            />
          </div>
          <div className="w-[136px]">
            <Toggle
              checked={lupa.marco}
              onChange={(marco) => onCambio({ lupa: { ...lupa, marco } })}
              label={t("Con marco")}
            />
          </div>
        </>
      ) : (
        <div className="w-[180px]">
          <Slider
            label={t("Cuánto se acerca")}
            hint={`${zoom.nivel.toFixed(1)}×`}
            min={1.2}
            max={NIVEL_MAXIMO}
            step={0.1}
            value={zoom.nivel}
            onChange={(nivel) => onCambio({ nivel })}
          />
        </div>
      )}

      <div role="group" aria-label={t("Dónde mira")} className="flex items-center gap-4">
        <Opcion puesta={zoom.modo === "sigue"} onClick={() => onCambio({ modo: "sigue" })}>
          {t("Sigue al ratón")}
        </Opcion>
        <Opcion puesta={zoom.modo === "fijo"} onClick={() => onCambio({ modo: "fijo" })}>
          {t("Fijo")}
        </Opcion>
      </div>
      <div className="w-[160px]">
        <Toggle
          checked={zoom.instantaneo}
          onChange={(instantaneo) => onCambio({ instantaneo })}
          label={t("Entra de golpe")}
        />
      </div>

      <div className="ml-auto flex items-center gap-2">
        <Boton onClick={onQuitar}>{t("Quitar el zoom")}</Boton>
        <Boton onClick={onListo}>{t("Listo")}</Boton>
      </div>
    </div>
  );
}

/** Lo que se hace con el trozo que hay bajo el cabezal: quitarlo, o cambiarle la velocidad. */
export function AjustesDelTrozo({
  pieza,
  numero,
  cuantas,
  puedeQuitarse,
  onCambio,
}: {
  pieza: Pieza;
  /** Cuál es, contando desde 1, y cuántas hay. */
  numero: number;
  cuantas: number;
  /** El último trozo que se queda no se puede quitar: el vídeo se quedaría vacío. */
  puedeQuitarse: boolean;
  onCambio: (cambio: Partial<Pieza>) => void;
}) {
  const t = useT();
  return (
    <div data-ajustes-trozo className="flex flex-wrap items-center gap-x-5 gap-y-2">
      <span className="text-[13px] font-medium whitespace-nowrap text-neutral-100">
        {t("Trozo {n} de {total}", { n: numero, total: cuantas })}
      </span>
      <Boton onClick={() => onCambio({ cortada: !pieza.cortada })} apagado={!pieza.cortada && !puedeQuitarse}>
        {pieza.cortada ? t("Volver a ponerlo") : t("Quitar este trozo")}
      </Boton>
      {!pieza.cortada && (
        <div role="group" aria-label={t("Velocidad del trozo")} className="flex items-center gap-4">
          {VELOCIDADES_DE_TROZO.map((v) => (
            <Opcion key={v} puesta={pieza.velocidad === v} onClick={() => onCambio({ velocidad: v })}>
              {v}×
            </Opcion>
          ))}
        </div>
      )}
    </div>
  );
}

function Boton({ onClick, apagado = false, children }: { onClick: () => void; apagado?: boolean; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={apagado}
      className="flex h-8 items-center gap-1.5 rounded-lg bg-white/10 px-3 text-[13px] font-medium whitespace-nowrap text-neutral-100 transition-colors hover:bg-white/15 hover:text-white disabled:opacity-40"
    >
      {children}
    </button>
  );
}

/** La forma dibujada a trazo, como los iconos de la barra de anotar. */
export function DibujoDeForma({ forma, puesta }: { forma: Forma; puesta: boolean }) {
  const trazo = { fill: "none", stroke: "currentColor", strokeWidth: puesta ? 2.2 : 1.9 };
  return (
    <svg viewBox="0 0 24 24" className="size-5" aria-hidden="true">
      {forma === "circulo" && <circle cx="12" cy="12" r="8" {...trazo} />}
      {forma === "cuadrado" && <rect x="4" y="4" width="16" height="16" {...trazo} />}
      {forma === "rectangulo" && <rect x="2.5" y="7" width="19" height="10.5" {...trazo} />}
      {forma === "redondeado" && <rect x="2.5" y="7" width="19" height="10.5" rx="3.5" {...trazo} />}
    </svg>
  );
}
