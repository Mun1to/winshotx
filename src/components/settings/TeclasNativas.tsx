import { useT } from "../../lib/i18n";
import { CAPTURA_WINDOWS, GRABACION_WINDOWS, partesDeAtajo } from "../../lib/teclas";
import type { PrintScreenState } from "../../lib/types";

export function TeclasNativas({ estado, legado, mostrarImprPant = true }: {
  estado: PrintScreenState;
  legado: boolean;
  mostrarImprPant?: boolean;
}) {
  const t = useT();
  const filas = [
    [CAPTURA_WINDOWS, legado ? t("Desactiva la opción antigua de Win+Mayús+S y vuelve a iniciar sesión.") : t("Capturar una región")],
    [GRABACION_WINDOWS, t("Grabar vídeo")],
  ];
  if (mostrarImprPant) {
    filas.push([t("Impr Pant"), estado.takenByWindows || estado.active
      ? t("abre winshotx")
      : t("La captura con Impr Pant está desactivada en Windows.")]);
  }
  return (
    <dl className="mt-3 divide-y divide-linea">
      {filas.map(([tecla, accion]) => (
        <div key={tecla} className="flex items-center justify-between gap-3 py-3">
          <dt className="whitespace-nowrap font-semibold text-titulo">{partesDeAtajo(tecla).join(" + ")}</dt>
          <dd className="text-right text-[14px] text-texto">{accion}</dd>
        </div>
      ))}
    </dl>
  );
}
