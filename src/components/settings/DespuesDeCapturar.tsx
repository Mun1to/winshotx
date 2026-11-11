import { Frame, PanelBottomOpen } from "lucide-react";
import { FONDOS } from "../../lib/fondos";
import { useT } from "../../lib/i18n";
import type { Background, Settings } from "../../lib/types";
import { Segmented } from "../ui/Segmented";
import { Row, Section } from "./Section";

/**
 * Lo que pasa con una foto después de capturarla (S4 del plan de CleanShot X): la miniatura
 * de la esquina (E2) en su bloque, y el fondo para todas las capturas (E1) como fila de
 * «La captura».
 *
 * Los valores por defecto son los de Rust (`settings.rs`): miniatura a los 10 segundos y
 * sin fondo. Aquí se repiten para unos ajustes viejos que no los traen.
 */
interface Props {
  settings: Settings;
  patch: (partial: Partial<Settings>) => void;
}

/** Cuánto se queda la miniatura. «No» la apaga; «Hasta cerrarla» es sin reloj. */
type Duracion = "no" | "5" | "10" | "30" | "siempre";

const DURACIONES: { value: Duracion; label: string }[] = [
  { value: "no", label: "No" },
  { value: "5", label: "5 s" },
  { value: "10", label: "10 s" },
  { value: "30", label: "30 s" },
  { value: "siempre", label: "Hasta cerrarla" },
];

/** El aire del fondo, en tanto por ciento del lado corto: los mismos de `marco_captura::AIRES`. */
const AIRES: { value: number; label: string }[] = [
  { value: 0, label: "Sin fondo" },
  { value: 4, label: "Poco" },
  { value: 8, label: "Normal" },
  { value: 14, label: "Mucho" },
];

export function duracionDe(settings: Settings): Duracion {
  if (settings.miniatura === false) return "no";
  const segundos = settings.miniaturaSegundos ?? 10;
  if (segundos === 0) return "siempre";
  return String(segundos) as Duracion;
}

export function ajustesDeDuracion(d: Duracion): Partial<Settings> {
  if (d === "no") return { miniatura: false };
  return { miniatura: true, miniaturaSegundos: d === "siempre" ? 0 : Number(d) };
}

export function DespuesDeCapturar({ settings, patch }: Props) {
  const t = useT();
  const duracion = duracionDe(settings);

  return (
    <Section title={t("Después de capturar")} tour="despues-de-capturar">
      <Row
        icon={<PanelBottomOpen className="size-4" />}
        label={t("Miniatura en la esquina")}
        explicacion={t(
          "Después de copiar o guardar una foto sale abajo a la derecha. Arrástrala a un chat, a un correo o a una carpeta y llega como archivo; al pasar el ratón la copias, la guardas, la editas o la anclas. Se va sola, salvo mientras tienes el ratón encima.",
        )}
        // Sin línea de ayuda debajo: con ella la pestaña de Capturar dejaba de caber sin
        // rueda. Lo que hace va en la explicación del icono.
        stacked
        control={
          <Segmented
            value={duracion}
            options={DURACIONES.map((o) => ({ ...o, label: t(o.label) }))}
            onChange={(d) => patch(ajustesDeDuracion(d))}
            etiqueta={t("Miniatura en la esquina")}
          />
        }
      />
    </Section>
  );
}

/**
 * El fondo para todas las capturas (E1): una fila de «La captura», porque dice cómo sale
 * la foto, igual que el cursor de esa misma lista.
 *
 * El color va dentro de la misma fila, debajo del aire, y solo con fondo puesto. Como fila
 * aparte con su nombre, la pestaña de Capturar dejaba de caber en la ventana y pedía rueda,
 * y los ajustes se leen enteros sin bajar.
 */
export function FondoEnLasCapturas({ settings, patch }: Props) {
  const t = useT();
  const marco = settings.marcoCapturas ?? { aire: 0, fondo: "atardecer" as Background };
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
                  aria-checked={marco.fondo === f.id}
                  aria-label={t(f.nombre)}
                  title={t(f.nombre)}
                  onClick={() => patch({ marcoCapturas: { ...marco, fondo: f.id } })}
                  style={{ background: f.css }}
                  className={`size-7 rounded-md border-2 transition-colors ${
                    marco.fondo === f.id ? "border-marca" : "border-linea hover:border-suave"
                  }`}
                />
              ))}
            </div>
          )}
        </div>
      }
    />
  );
}
