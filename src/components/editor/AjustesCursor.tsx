import { Slider } from "../ui/Slider";
import { Toggle } from "../ui/Toggle";
import { useT } from "../../lib/i18n";
import type { Edicion, Suavizado } from "../../lib/contrato";
import { Opcion } from "../ui/Opcion";
import { Segmented } from "../ui/Segmented";
// S10 (sonidos)
import { useState } from "react";
import type { ClicElegido } from "../../lib/contrato";
import { elegirSonidoDeClic, importarSonidoDeClic } from "../../lib/ipc";
import { nombreDelClic, oirClic } from "../../lib/sonidoDeClic";

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
  /**
   * Qué trozo enseñar (S39): en el panel cada grupo plegable lleva lo suyo. El puntero
   * (suavizado, esconderlo al escribir y la estela), los clics (la onda y el sonido) o el
   * zoom (acercarse al escribir). Sin nada, todo junto.
   */
  parte?: "puntero" | "clics" | "zoom";
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
  parte,
}: Props) {
  const ve = (cual: NonNullable<Props["parte"]>) => !parte || parte === cual;
  const t = useT();
  const suavizado: Suavizado = edicion.suavizado ?? "poco";
  const estela = Math.round((edicion.estela ?? 0) * 100);
  const sonido = edicion.sonidoClic ?? 0;
  // S10 (sonidos): cuál suena, y lo que salió mal al traer uno propio.
  const elegido: ClicElegido = edicion.clic ?? { tipo: "suave" };
  const [fallo, setFallo] = useState<string | null>(null);
  const [trayendo, setTrayendo] = useState(false);
  const elegir = (clic: ClicElegido) => {
    setFallo(null);
    onEdicion({ clic });
    // Al elegirlo suena una vez, a su volumen: es la única forma de saber cómo es.
    void oirClic(clic, sonido).catch(() => undefined);
  };
  const traerPropio = async () => {
    setFallo(null);
    const origen = await elegirSonidoDeClic().catch(() => null);
    if (!origen) return;
    setTrayendo(true);
    try {
      elegir({ tipo: "propio", ruta: await importarSonidoDeClic(origen) });
    } catch (e) {
      setFallo(String(e));
    } finally {
      setTrayendo(false);
    }
  };

  return (
    // Un mando debajo de otro, con el mismo aire que el resto de su grupo del panel.
    <div className="space-y-4" data-ajustes-cursor>
      {ve("puntero") && conPuntero && (
        <div>
          <span className="mb-1 block text-[15px] font-medium text-titulo">{t("Movimiento suave")}</span>
          <Segmented
            value={suavizado}
            etiqueta={t("Movimiento suave")}
            options={[
              { value: "nada", label: t("Nada") },
              { value: "poco", label: t("Poco") },
              { value: "mucho", label: t("Mucho") },
            ]}
            onChange={(valor) => onEdicion({ suavizado: valor })}
          />
        </div>
      )}

      {ve("puntero") && conPuntero && conEscritura && (
        <Toggle
          checked={edicion.esconderAlEscribir ?? false}
          onChange={(v) => onEdicion({ esconderAlEscribir: v })}
          label={t("Esconderlo mientras escribes")}
          hint={t("vuelve al mover el ratón")}
        />
      )}

      {ve("puntero") && (conPuntero || conZoom) && (
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

      {ve("clics") && conClics && (
        <Toggle
          checked={edicion.ondaClic ?? false}
          onChange={(v) => onEdicion({ ondaClic: v })}
          label={t("Una onda en cada clic")}
        />
      )}

      {ve("clics") && conClics && (
        <div className="space-y-3">
          <Toggle
            checked={sonido > 0}
            onChange={(v) => onEdicion({ sonidoClic: v ? volumenRecordado() : 0 })}
            label={t("Sonido de clic")}
            hint={t("se oye en el vídeo, también si no grabaste sonido")}
          />
          {/* S10 (sonidos): los tres de serie y uno propio. */}
          {sonido > 0 && (
            <div data-sonido-de-clic>
              <div className="flex items-center gap-4">
                {(
                  [
                    ["suave", t("Suave")],
                    ["grave", t("Grave")],
                    ["toque", t("Toque")],
                  ] as const
                ).map(([tipo, nombre]) => (
                  <Opcion key={tipo} puesta={elegido.tipo === tipo} onClick={() => elegir({ tipo })}>
                    {nombre}
                  </Opcion>
                ))}
                <Opcion puesta={elegido.tipo === "propio"} onClick={() => void traerPropio()}>
                  {t("Propio…")}
                </Opcion>
              </div>
              {trayendo && <p className="pt-1 text-[13px] text-suave">{t("Preparando el sonido…")}</p>}
              {!trayendo && elegido.tipo === "propio" && (
                <p className="truncate pt-1 text-[13px] text-suave" title={nombreDelClic(elegido.ruta)}>
                  {nombreDelClic(elegido.ruta)}
                </p>
              )}
              {fallo && (
                <p className="pt-1 text-[13px] text-red-700 [[data-tema=oscuro]_&]:text-red-300" role="alert">
                  {fallo}
                </p>
              )}
            </div>
          )}
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

      {ve("zoom") && conEscritura && (
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
