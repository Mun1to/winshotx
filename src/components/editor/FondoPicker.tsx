/**
 * El fondo del estudio: los de nombre de siempre y, debajo, los propios (B1, B2, B3 y B5):
 * una imagen tuya, tu fondo de escritorio, un vídeo o un GIF, y cualquier color.
 *
 * Los propios van en su propia fila y la paleta detrás de «Más colores»: la decisión de
 * pocos fondos de `fondos.ts` sigue en pie, y quien no busca nada raro ve lo de siempre.
 *
 * No guarda el fondo: lo dice hacia arriba. Uno de nombre con `onFondo` (como hasta ahora) y
 * uno propio con `onElegido`, que va a `edicion.fondo` y manda sobre el de nombre al
 * exportar. Lo engancha `ExportPanel`.
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { Film, ImagePlus, Monitor, Palette } from "lucide-react";
import { useT } from "../../lib/i18n";
import type { Background } from "../../lib/types";
import type { FondoElegido } from "../../lib/contrato";
import { cssDelFondo, FONDOS, recordarEscritorio, type Rgb } from "../../lib/fondos";
import {
  cancelarFondoAnimado,
  elegirArchivoDeFondo,
  EVENTO_PROGRESO_FONDO,
  FONDO_CANCELADO,
  fondoEscritorio,
  importarFondoAnimado,
  importarImagenDeFondo,
} from "../../lib/ipc";
import { PaletaDeColor } from "./PaletaDeColor";

interface Props {
  /** El fondo de nombre que hay puesto. */
  fondo: Background;
  /** Si hay marco. Sin aire alrededor no se ve ningún fondo: es «Sin fondo». */
  conMarco: boolean;
  /** El fondo propio, si hay uno: manda sobre `fondo`. */
  elegido: FondoElegido | null;
  /** Se eligió uno de nombre, o ninguno (`null` es «Sin fondo»). */
  onFondo: (fondo: Background | null) => void;
  /** Se eligió uno propio. */
  onElegido: (elegido: FondoElegido) => void;
}

/** El color con el que se abre la paleta si no hay ninguno puesto: el del «Atardecer». */
const COLOR_DE_PARTIDA: Rgb = [88, 116, 245];

export function FondoPicker({ fondo, conMarco, elegido, onFondo, onElegido }: Props) {
  const t = useT();
  const [paleta, setPaleta] = useState(elegido?.tipo === "color" || elegido?.tipo === "degradado");
  const [modo, setModo] = useState<"color" | "degradado">(elegido?.tipo === "color" ? "color" : "degradado");
  const [color, setColor] = useState<Rgb>(
    elegido?.tipo === "color" || elegido?.tipo === "degradado" ? elegido.rgb : COLOR_DE_PARTIDA,
  );
  /** Lo importado de un vídeo, de 0 a 1, mientras se importa. */
  const [importando, setImportando] = useState<number | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const importandoAhora = useRef(false);

  // Si se cierra el panel a mitad de importar, la importación se cancela: nadie va a
  // recoger ese fondo, y seguiría gastando máquina y disco.
  useEffect(
    () => () => {
      if (importandoAhora.current) void cancelarFondoAnimado();
    },
    [],
  );

  const propio = conMarco ? elegido : null;
  const pulsado = (f: Background) => conMarco && !elegido && fondo === f;

  const ponerColor = (rgb: Rgb, como = modo) => {
    setColor(rgb);
    onElegido({ tipo: como, rgb });
  };

  const traerImagen = async () => {
    setError(null);
    const origen = await elegirArchivoDeFondo("imagen");
    if (!origen) return;
    setOcupado(true);
    try {
      onElegido({ tipo: "imagen", ruta: await importarImagenDeFondo(origen) });
    } catch (e) {
      setError(String(e));
    } finally {
      setOcupado(false);
    }
  };

  const traerEscritorio = async () => {
    setError(null);
    setOcupado(true);
    try {
      const vista = await fondoEscritorio();
      recordarEscritorio(vista.tipo === "imagen" ? { imagen: vista.vista } : { rgb: vista.rgb });
      onElegido({ tipo: "escritorio" });
    } catch (e) {
      setError(String(e));
    } finally {
      setOcupado(false);
    }
  };

  const traerVideo = async () => {
    setError(null);
    const origen = await elegirArchivoDeFondo("video");
    if (!origen) return;
    setImportando(0);
    importandoAhora.current = true;
    const dejar = await listen<{ hecho: number }>(EVENTO_PROGRESO_FONDO, (e) => setImportando(e.payload.hecho));
    try {
      const hecho = await importarFondoAnimado(origen);
      onElegido({ tipo: "video", ruta: hecho.ruta });
    } catch (e) {
      // Cancelar no es un fallo: se queda el fondo que hubiera, sin decir nada.
      if (String(e) !== FONDO_CANCELADO) setError(String(e));
    } finally {
      importandoAhora.current = false;
      dejar();
      setImportando(null);
    }
  };

  return (
    <div className="space-y-1">
      <div className="grid grid-cols-9 gap-1">
        <button
          type="button"
          onClick={() => onFondo(null)}
          title={t("Sin fondo")}
          aria-label={t("Sin fondo")}
          aria-pressed={!conMarco}
          className={`flex aspect-square w-full items-center justify-center rounded-md border-2 text-neutral-400 transition-colors ${
            !conMarco ? "border-blue-500" : "border-white/15 hover:border-white/40"
          }`}
        >
          <svg viewBox="0 0 20 20" className="size-full p-1" aria-hidden="true">
            <line x1="4" y1="16" x2="16" y2="4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
          </svg>
        </button>
        {FONDOS.map((f) => (
          <button
            key={f.id}
            type="button"
            onClick={() => onFondo(f.id)}
            title={t(f.nombre)}
            aria-label={t(f.nombre)}
            aria-pressed={pulsado(f.id)}
            style={{ background: f.css }}
            className={`aspect-square w-full rounded-md border-2 transition-colors ${
              pulsado(f.id) ? "border-blue-500" : "border-white/15 hover:border-white/40"
            }`}
          />
        ))}
      </div>

      <div className="grid grid-cols-4 gap-1">
        <Propio
          puesto={propio?.tipo === "imagen"}
          vista={propio?.tipo === "imagen" ? cssDelFondo(fondo, propio) : null}
          icono={<ImagePlus className="size-[18px]" />}
          titulo={t("Poner una imagen tuya de fondo")}
          apagado={ocupado || importando !== null}
          onClick={() => void traerImagen()}
        >
          {t("Imagen")}
        </Propio>
        <Propio
          puesto={propio?.tipo === "escritorio"}
          vista={propio?.tipo === "escritorio" ? cssDelFondo(fondo, propio) : null}
          icono={<Monitor className="size-[18px]" />}
          titulo={t("Poner tu fondo de escritorio")}
          apagado={ocupado || importando !== null}
          onClick={() => void traerEscritorio()}
        >
          {t("Escritorio")}
        </Propio>
        <Propio
          puesto={propio?.tipo === "video"}
          vista={propio?.tipo === "video" ? cssDelFondo(fondo, propio) : null}
          icono={<Film className="size-[18px]" />}
          titulo={t("Poner un vídeo o un GIF tuyo de fondo, que se repite")}
          apagado={ocupado || importando !== null}
          onClick={() => void traerVideo()}
        >
          {t("Vídeo o GIF")}
        </Propio>
        <Propio
          puesto={propio?.tipo === "color" || propio?.tipo === "degradado"}
          vista={propio?.tipo === "color" || propio?.tipo === "degradado" ? cssDelFondo(fondo, propio) : null}
          icono={<Palette className="size-[18px]" />}
          titulo={t("Elegir cualquier color")}
          abierto={paleta}
          onClick={() => setPaleta((abierta) => !abierta)}
        >
          {t("Más colores")}
        </Propio>
      </div>

      {importando !== null && (
        <div className="space-y-1.5 pt-2" role="status">
          <div className="flex items-center justify-between gap-3">
            <span className="text-[13px] text-texto">
              {t("Importando el fondo")} <span className="tabular-nums">{Math.round(importando * 100)} %</span>
            </span>
            <button
              type="button"
              onClick={() => void cancelarFondoAnimado()}
              className="rounded-lg bg-white/10 px-3 py-1 text-[13px] font-medium text-neutral-100 transition-colors hover:bg-white/15 hover:text-white"
            >
              {t("Cancelar")}
            </button>
          </div>
          <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
            <div
              className="h-full bg-blue-500 transition-[width] duration-150"
              style={{ width: `${Math.round(importando * 100)}%` }}
            />
          </div>
        </div>
      )}

      {error && (
        <p className="pt-1 text-[13px] text-red-400" role="alert">
          {error}
        </p>
      )}

      {/* La paleta: crece desde arriba y empuja lo de debajo, como los desplegables del panel. */}
      <div
        className="grid transition-[grid-template-rows] duration-[240ms] ease-in-out"
        style={{ gridTemplateRows: paleta ? "1fr" : "0fr" }}
      >
        <div className="overflow-hidden">
          {paleta && (
            <div className="space-y-2.5 pt-2.5">
              <PaletaDeColor rgb={color} onCambiar={(rgb) => ponerColor(rgb)} />
              <div className="flex items-center gap-4">
                <Opcion
                  puesta={propio?.tipo === "color"}
                  onClick={() => {
                    setModo("color");
                    ponerColor(color, "color");
                  }}
                >
                  {t("Liso")}
                </Opcion>
                <Opcion
                  puesta={propio?.tipo === "degradado"}
                  onClick={() => {
                    setModo("degradado");
                    ponerColor(color, "degradado");
                  }}
                >
                  {t("Degradado")}
                </Opcion>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

/**
 * Un botón de los propios: su dibujo encima de la palabra. Con ese fondo puesto, el dibujo
 * se cambia por el propio fondo en pequeño, para que se vea cuál es.
 */
function Propio({
  puesto,
  vista,
  icono,
  titulo,
  apagado = false,
  abierto,
  onClick,
  children,
}: {
  puesto: boolean;
  vista: string | null;
  icono: ReactNode;
  titulo: string;
  apagado?: boolean;
  abierto?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={apagado}
      title={titulo}
      aria-pressed={puesto}
      aria-expanded={abierto}
      className={`flex flex-col items-center justify-center gap-1 rounded-md border-2 px-1 py-1.5 text-[12px] font-medium whitespace-nowrap transition-colors disabled:opacity-50 ${
        puesto ? "border-blue-500 text-white" : "border-white/15 text-neutral-200 hover:border-white/40 hover:text-white"
      }`}
    >
      {vista ? (
        <span className="block h-[18px] w-7 rounded-sm" style={{ background: vista }} aria-hidden="true" />
      ) : (
        <span className="flex h-[18px] items-center" aria-hidden="true">
          {icono}
        </span>
      )}
      {children}
    </button>
  );
}

/** Una opción en texto, con la elegida subrayada: la misma que usa el panel. */
function Opcion({ puesta, onClick, children }: { puesta: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={puesta}
      className={`border-b-2 py-1 text-[14px] font-semibold transition-colors ${
        puesta ? "border-blue-500 text-white" : "border-transparent text-neutral-400 hover:text-neutral-100"
      }`}
    >
      {children}
    </button>
  );
}
