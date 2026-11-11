import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Frame } from "lucide-react";
import { FONDOS } from "../../lib/fondos";
import { useT } from "../../lib/i18n";
import { AVISO_PRESETS, type PresetGuardado } from "../../lib/ipc";
import { estiloDeLasCapturas, leerPresets, marcoCapturasDe, type MarcoCapturas } from "../../lib/presets";
import type { Background, Settings } from "../../lib/types";
import { Segmented } from "../ui/Segmented";
import { Row } from "./Section";

interface Props {
  settings: Settings;
  patch: (partial: Partial<Settings>) => void;
}

/** El aire del fondo, en tanto por ciento del lado corto: los mismos de `marco_captura::AIRES`. */
const AIRES: { value: number; label: string }[] = [
  { value: 0, label: "Sin fondo" },
  { value: 4, label: "Poco" },
  { value: 8, label: "Normal" },
  { value: 14, label: "Mucho" },
];

/**
 * El fondo para todas las capturas (E1 de S4), y desde S9 también con un estilo guardado.
 *
 * Es una fila de «La captura», porque dice cómo sale la foto, igual que el cursor de esa
 * misma lista. El color va dentro de la misma fila, debajo del aire, y solo con fondo puesto:
 * como fila aparte, la pestaña de Capturar dejaba de caber sin rueda.
 *
 * **Un estilo guardado en el editor se puede poner a todas las capturas** (el plan: «se
 * puede aplicar un preset a todas las capturas»). Se copian su fondo, también uno propio
 * como un color o una imagen, y su aire; la sombra y las esquinas de las capturas van
 * siempre, como las dejó S4. Solo sale si hay alguno guardado, y el que coincide con lo
 * puesto va subrayado, como cualquier opción de texto.
 */
export function FondoEnLasCapturas({ settings, patch }: Props) {
  const t = useT();
  const marco: MarcoCapturas = settings.marcoCapturas ?? { aire: 0, fondo: "atardecer" as Background };
  const [lista, setLista] = useState(() => leerPresets(settings.presets));

  // La lista la cambia el editor, que es donde se guardan los estilos: con la ventana de
  // ajustes abierta a la vez, se entera por el aviso en vez de enseñar la de cuando se abrió.
  useEffect(() => setLista(leerPresets(settings.presets)), [settings.presets]);
  useEffect(() => {
    const quitar = listen<PresetGuardado[]>(AVISO_PRESETS, (e) => setLista(leerPresets(e.payload)));
    return () => void quitar.then((fn) => fn());
  }, []);

  const conEstilo = estiloDeLasCapturas(lista, marco);
  const propio = Boolean(marco.elegido);
  /** El fondo de nombre, sin el propio que pudiera traer de un estilo. */
  const conNombre = (fondo: Background) => {
    const { elegido: _quitado, ...resto } = marco;
    return { ...resto, fondo };
  };

  return (
    <Row
      icon={<Frame className="size-4" />}
      label={t("Fondo en las capturas")}
      explicacion={t(
        "Cada foto que copias o guardas sale sobre un fondo, con aire alrededor, sombra y las esquinas redondeadas, lista para colgarla. Son los mismos fondos del Estudio del editor. No se pone al editar, al anclar ni al leer el texto.",
      )}
      hint={marco.aire === 0 ? t("las capturas salen tal cual") : t("se pone al copiar y al guardar")}
      stacked
      control={
        <div className="flex flex-col gap-2.5" data-tour="fondo-en-las-capturas">
          <Segmented
            value={marco.aire}
            options={AIRES.map((o) => ({ ...o, label: t(o.label) }))}
            onChange={(aire) => patch({ marcoCapturas: { ...marco, aire } })}
            etiqueta={t("Fondo en las capturas")}
          />
          {marco.aire > 0 && (
            <div role="radiogroup" aria-label={t("Color del fondo")} className="flex flex-wrap gap-2">
              {FONDOS.map((f) => (
                <button
                  key={f.id}
                  type="button"
                  role="radio"
                  aria-checked={!propio && marco.fondo === f.id}
                  aria-label={t(f.nombre)}
                  title={t(f.nombre)}
                  onClick={() => patch({ marcoCapturas: conNombre(f.id) })}
                  style={{ background: f.css }}
                  className={`size-7 rounded-md border-2 transition-colors ${
                    !propio && marco.fondo === f.id ? "border-marca" : "border-linea hover:border-suave"
                  }`}
                />
              ))}
            </div>
          )}
          {/* S9 (presets y tapar): un estilo guardado, para todas las capturas. */}
          {lista.length > 0 && (
            <div
              role="group"
              aria-label={t("Estilo guardado para todas las capturas")}
              className="flex flex-wrap items-center gap-x-4 gap-y-1"
            >
              <span className="text-[13px] text-texto">{t("O un estilo guardado:")}</span>
              {lista.map((p, i) => (
                <button
                  key={p.nombre}
                  type="button"
                  aria-pressed={conEstilo === i}
                  onClick={() => patch({ marcoCapturas: marcoCapturasDe(p.estilo) })}
                  className={`border-b-2 py-0.5 text-[13px] font-semibold transition-colors ${
                    conEstilo === i ? "border-marca text-texto" : "border-transparent text-suave hover:text-texto"
                  }`}
                >
                  {p.nombre}
                </button>
              ))}
            </div>
          )}
        </div>
      }
    />
  );
}
