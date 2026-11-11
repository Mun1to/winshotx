/**
 * El fondo animado moviéndose en la vista previa: el vídeo importado, en bucle, debajo de
 * la grabación. Sin él la vista previa enseña su primer fotograma quieto (`cssDelFondo`), y
 * un fondo que se mueve en el archivo y no en la pantalla parece roto.
 *
 * Mudo y sin controles: el MP4 importado no lleva sonido, y esto es decorado, no un vídeo
 * que se mira. Va encima del fondo de CSS y debajo de todo lo demás.
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import type { FondoElegido } from "../../lib/contrato";
import { vistaDelVideo } from "../../lib/fondos";

export function FondoVivo({ elegido }: { elegido?: FondoElegido | null }) {
  if (elegido?.tipo !== "video") return null;
  return (
    <video
      key={elegido.ruta}
      src={convertFileSrc(elegido.ruta)}
      poster={convertFileSrc(vistaDelVideo(elegido.ruta))}
      autoPlay
      loop
      muted
      playsInline
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 -z-10 size-full object-cover"
    />
  );
}
