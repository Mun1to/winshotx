import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Slider } from "../ui/Slider";
import { Toggle } from "../ui/Toggle";
import { CapaRecorte } from "./CapaRecorte";
import type { Edicion, MarcoCamara } from "../../lib/contrato";
import { camaraDeLaSesion } from "../../lib/ipc";
import {
  marcoDeFabrica,
  paraLosMandos,
  recorta,
  recorteLimpio,
  redondeo,
  REDONDEO_MAXIMO,
  SIN_RECORTE,
  TAM_MANDO_MAXIMO,
  TAM_MANDO_MINIMO,
  type CamaraDeLaSesion,
} from "../../lib/camara";
import { useT } from "../../lib/i18n";
import { FORMAS_DE_LA_CAMARA, SelectorDeForma } from "./AjustesDeLaPista";

interface Props {
  sessionId: string;
  edicion?: Edicion;
  onEdicion?: (cambio: Partial<Edicion>) => void;
}

/**
 * Los mandos de la webcam en el Estudio: enseñarla o no, su forma y sus esquinas, su tamaño,
 * el trozo de la cámara que se ve y el borde. Dónde va se decide arrastrándola en la vista
 * previa.
 *
 * Solo sale si la grabación trae cámara: un mando que no puede hacer nada no se enseña.
 */
export function AjustesCamara({ sessionId, edicion, onEdicion }: Props) {
  const t = useT();
  const [camara, setCamara] = useState<CamaraDeLaSesion | null>(null);
  /** La última que hubo puesta, para volver a ella al encenderla otra vez. */
  const ultimo = useRef<MarcoCamara | null>(null);

  useEffect(() => {
    let vigente = true;
    void camaraDeLaSesion(sessionId)
      .then((c) => {
        if (vigente) setCamara(c ?? null);
      })
      .catch(() => undefined);
    return () => {
      vigente = false;
    };
  }, [sessionId]);

  const url = useMemo(() => (camara ? convertFileSrc(camara.ruta) : null), [camara]);
  if (!camara || !url) return null;

  const marco = edicion?.camara;
  // Sin tocar todavía (`undefined`) sale puesta: la de fábrica la coloca la vista previa.
  const puesta = marco !== null;
  const m = paraLosMandos(marco ?? ultimo.current ?? marcoDeFabrica(16, 9));
  const cambiar = (cambio: Partial<MarcoCamara>) => onEdicion?.({ camara: { ...m, ...cambio } });
  const recortada = recorta(m.recorte);
  // S13 (redondeo): en tanto por ciento del lado corto, como lo enseña el mando.
  const esquinas = Math.round(redondeo(m) * 100);

  return (
    // Una fila por mando, con su línea fina entre medias, como el resto del panel (S39).
    <div data-ajustes-camara className="divide-y divide-linea-tenue [&>*]:py-2 [&>*:first-child]:pt-0">
      <Toggle
        checked={puesta}
        onChange={(si) => {
          if (si) {
            onEdicion?.({ camara: ultimo.current ?? marcoDeFabrica(16, 9) });
          } else {
            ultimo.current = marco ?? null;
            onEdicion?.({ camara: null });
          }
        }}
        label={t("La cámara")}
        hint={t("arrástrala en la vista previa para moverla")}
      />
      {puesta && (
        <SelectorDeForma
            forma={m.forma}
            onForma={(forma) => cambiar({ forma })}
            etiqueta={t("Forma de la cámara")}
            formas={FORMAS_DE_LA_CAMARA}
            enColumnas
          />
      )}
          {/* S13 (redondeo): el mismo mando que las esquinas del marco del fondo. El círculo
              no tiene esquinas. */}
          {puesta && m.forma !== "circulo" && (
            <Slider
              label={t("Esquinas redondeadas")}
              hint={esquinas === 0 ? t("rectas") : `${esquinas} %`}
              min={0}
              max={Math.round(REDONDEO_MAXIMO * 100)}
              step={1}
              value={esquinas}
              onChange={(v) => cambiar({ redondeo: v / 100 })}
            />
          )}
          {puesta && (
          <Slider
            label={t("Tamaño")}
            hint={`${Math.round(m.tam * 100)} %`}
            min={Math.round(TAM_MANDO_MINIMO * 100)}
            max={Math.round(TAM_MANDO_MAXIMO * 100)}
            step={1}
            value={Math.round(m.tam * 100)}
            onChange={(v) => cambiar({ tam: v / 100 })}
          />
          )}
          {puesta && (
          <div>
            <span className="mb-1.5 block text-[15px] font-medium text-titulo">{t("Recorte de la cámara")}</span>
            {/* La imagen de la cámara con el marco de recortar encima: el mismo que el de la
                captura, que se arrastra igual. */}
            {/* Como mucho 90 px de alto: a todo el ancho del panel se comía el sitio de los demás
                mandos y en una ventana de 700 de alto «Con borde» se quedaba debajo del borde. */}
            <div
              className="relative mx-auto overflow-hidden rounded-md bg-black"
              style={{
                aspectRatio: `${camara.ancho} / ${camara.alto}`,
                maxWidth: Math.round((90 * camara.ancho) / Math.max(1, camara.alto)),
              }}
            >
              <video
                src={url}
                muted
                playsInline
                preload="auto"
                disablePictureInPicture
                aria-hidden="true"
                onLoadedMetadata={(e) => {
                  // A un segundo: la primera imagen de una cámara suele salir oscura.
                  const v = e.currentTarget;
                  v.currentTime = Math.min(1, (Number.isFinite(v.duration) ? v.duration : 0) / 2);
                }}
                className="absolute inset-0 size-full object-fill"
              />
              <CapaRecorte
                activa
                recorte={recortada ? m.recorte : null}
                onRecorte={(r) => cambiar({ recorte: r ? recorteLimpio(r) : SIN_RECORTE })}
              />
            </div>
            <div className="mt-1.5 flex items-center justify-between gap-3">
              <span className="text-[13px] leading-snug text-suave">
                {t("Arrastra encima para recortarla")}
              </span>
              {recortada && (
                <button
                  type="button"
                  onClick={() => cambiar({ recorte: SIN_RECORTE })}
                  className="flex h-8 shrink-0 items-center rounded-lg bg-realce px-3 text-[13px] font-medium whitespace-nowrap text-titulo transition-colors hover:bg-activo hover:text-titulo"
                >
                  {t("Quitar el recorte")}
                </button>
              )}
            </div>
          </div>
          )}
          {puesta && <Toggle checked={m.borde} onChange={(borde) => cambiar({ borde })} label={t("Con borde")} />}
    </div>
  );
}
