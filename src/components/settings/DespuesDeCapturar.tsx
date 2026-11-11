import { PanelBottomOpen } from "lucide-react";
import { useT } from "../../lib/i18n";
import type { Settings } from "../../lib/types";
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

// S9 (presets y tapar): el fondo de las capturas vive en su archivo, con los estilos guardados.
export { FondoEnLasCapturas } from "./FondoEnLasCapturas";
