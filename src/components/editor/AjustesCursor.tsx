import type { ReactNode } from "react";
import { Slider } from "../ui/Slider";
import { Toggle } from "../ui/Toggle";
import { useT } from "../../lib/i18n";
import type { Edicion, Suavizado } from "../../lib/contrato";

/**
 * Los mandos del cursor y los clics del Estudio (S7): el suavizado de tres opciones, esconder
 * el puntero al escribir, la estela, la onda y el sonido de cada clic, y acercarse al escribir.
 *
 * Van en el grupo «Puntero» del panel. El suavizado **sustituye al interruptor «Movimiento
 * suave»**: «Poco» es exactamente lo que hacía ese interruptor encendido (y lo que sale si no se
 * toca nada), «Nada» lo que hacía apagado.
 *
 * Todo viaja en `edicion`, apagado de serie: sin tocar nada, el vídeo sale como antes.
 */
interface Props {
  edicion: Edicion;
  onEdicion: (cambio: Partial<Edicion>) => void;
  /** Hay puntero dibujado (el tamaño no es 0): sin él no hay nada que suavizar ni esconder. */
  conPuntero: boolean;
  /** La grabación tiene clics: sin ellos la onda y el sonido no harían nada. */
  conClics: boolean;
  /** La grabación tiene ratos de escritura. Las de antes no los traen. */
  conEscritura?: boolean;
  /** Hay zoom: la estela también vale para la cámara aunque no haya puntero. */
  conZoom?: boolean;
}

/** El volumen con el que nace el sonido de clic: bajo, nunca a tope. */
export const VOLUMEN_CLIC_DE_SERIE = 0.3;
const CLAVE_VOLUMEN = "winshotx.volumenClic";

/** El último volumen del clic que se eligió, para que vuelva el mismo al encenderlo. */
function volumenRecordado(): number {
  try {
    const v = Number(localStorage.getItem(CLAVE_VOLUMEN));
    return v > 0 && v <= 1 ? v : VOLUMEN_CLIC_DE_SERIE;
  } catch {
    return VOLUMEN_CLIC_DE_SERIE;
  }
}

function recordarVolumen(v: number) {
  try {
    localStorage.setItem(CLAVE_VOLUMEN, String(v));
  } catch {
    // Sin almacenamiento, se olvida: vuelve a nacer bajo.
  }
}

export function AjustesCursor({
  edicion,
  onEdicion,
  conPuntero,
  conClics,
  conEscritura = true,
  conZoom = false,
}: Props) {
  const t = useT();
  const suavizado: Suavizado = edicion.suavizado ?? "poco";
  const estela = Math.round((edicion.estela ?? 0) * 100);
  const sonido = edicion.sonidoClic ?? 0;

  return (
    <div className="space-y-3" data-ajustes-cursor>
      {conPuntero && (
        <div>
          <span className="block text-[13px] font-medium text-texto">{t("Movimiento suave")}</span>
          <div className="flex items-center gap-4">
            {(
              [
                ["nada", t("Nada")],
                ["poco", t("Poco")],
                ["mucho", t("Mucho")],
              ] as const
            ).map(([valor, nombre]) => (
              <Opcion key={valor} puesta={suavizado === valor} onClick={() => onEdicion({ suavizado: valor })}>
                {nombre}
              </Opcion>
            ))}
          </div>
        </div>
      )}

      {conPuntero && conEscritura && (
        <Toggle
          checked={edicion.esconderAlEscribir ?? false}
          onChange={(v) => onEdicion({ esconderAlEscribir: v })}
          label={t("Esconderlo mientras escribes")}
          hint={t("vuelve al mover el ratón")}
        />
      )}

      {(conPuntero || conZoom) && (
        <Slider
          label={t("Estela al moverse")}
          hint={estela === 0 ? t("sin estela") : `${estela} %`}
          min={0}
          max={100}
          step={5}
          value={estela}
          onChange={(v) => onEdicion({ estela: v / 100 })}
        />
      )}

      {conClics && (
        <Toggle
          checked={edicion.ondaClic ?? false}
          onChange={(v) => onEdicion({ ondaClic: v })}
          label={t("Una onda en cada clic")}
        />
      )}

      {conClics && (
        <div className="space-y-2">
          <Toggle
            checked={sonido > 0}
            onChange={(v) => onEdicion({ sonidoClic: v ? volumenRecordado() : 0 })}
            label={t("Sonido de clic")}
            hint={t("se oye en el vídeo, también si no grabaste sonido")}
          />
          {sonido > 0 && (
            <Slider
              label={t("Volumen del clic")}
              hint={`${Math.round(sonido * 100)} %`}
              min={5}
              max={100}
              step={5}
              value={Math.round(sonido * 100)}
              onChange={(v) => {
                recordarVolumen(v / 100);
                onEdicion({ sonidoClic: v / 100 });
              }}
            />
          )}
        </div>
      )}

      {conEscritura && (
        <Toggle
          checked={edicion.zoomAlEscribir ?? false}
          onChange={(v) => onEdicion({ zoomAlEscribir: v })}
          label={t("Acercarse mientras escribes")}
          hint={t("al cursor de texto, o al ratón si no se ve")}
        />
      )}
    </div>
  );
}

/** Una opción en texto, con la elegida subrayada: nada de pastillas. La misma del panel. */
function Opcion({ puesta, onClick, children }: { puesta: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={puesta}
      className={`border-b-2 py-1 text-[14px] font-semibold whitespace-nowrap transition-colors ${
        puesta ? "border-blue-500 text-white" : "border-transparent text-neutral-400 hover:text-neutral-100"
      }`}
    >
      {children}
    </button>
  );
}
