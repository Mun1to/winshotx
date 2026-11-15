import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Frame } from "lucide-react";
import { FONDOS } from "../../lib/fondos";
// S25 (marco)
import { MARCOS, type CarcasaElegida } from "../../lib/carcasa";
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

/** «Ninguno» del marco, que en los ajustes es `null`: el selector necesita un valor que se pueda comparar. */
const SIN_MARCO = "ninguno";

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
      id="desFondo"
      icon={<Frame className="size-4" />}
      label={t("Fondo en las capturas")}
      explicacion={t(
        "Cada foto que copias o guardas sale sobre un fondo, con aire alrededor, sombra y las esquinas redondeadas, lista para colgarla. Con marco, va dentro de una ventana de Windows 11 (clara u oscura, como las de winshotx) o de un portátil. Son los mismos fondos del Estudio del editor. No se pone al editar, al anclar ni al leer el texto.",
      )}
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
          {/* S25 (marco): la ventana o el portátil, solo con fondo, como en el Estudio. Con la
              pieza común de elegir entre pocas cosas (S36): era una copia suya, con su raya de 2 px
              y su letra de 13, y se quedaba distinta de las de al lado. */}
          {marco.aire > 0 && (
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
              <span className="text-[14px] text-texto">{t("Marco:")}</span>
              <Segmented
                etiqueta={t("Marco")}
                value={marco.carcasa ?? SIN_MARCO}
                options={MARCOS.map((m) => ({ value: m.id ?? SIN_MARCO, label: t(m.nombre) }))}
                onChange={(v) => patch({ marcoCapturas: { ...marco, carcasa: v === SIN_MARCO ? null : (v as CarcasaElegida) } })}
              />
            </div>
          )}
          {/* S9 (presets y tapar): un estilo guardado, para todas las capturas. Ninguno va
              elegido mientras lo puesto no coincida con uno. */}
          {lista.length > 0 && (
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
              <span className="text-[14px] text-texto">{t("O un estilo guardado:")}</span>
              <Segmented
                etiqueta={t("Estilo guardado para todas las capturas")}
                value={conEstilo}
                options={lista.map((p, i) => ({ value: i, label: p.nombre }))}
                onChange={(i) => patch({ marcoCapturas: marcoCapturasDe(lista[i].estilo) })}
              />
            </div>
          )}
        </div>
      }
    />
  );
}
