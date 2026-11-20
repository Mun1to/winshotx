/**
 * La dirección de un vídeo o de un sonido del disco, para un `<video>` o un `<audio>`.
 *
 * En Windows es la de siempre: el protocolo `asset:` de Tauri. En Linux ese protocolo sirve
 * para las imágenes pero no para el vídeo ni el sonido, porque en WebKitGTK los reproduce
 * GStreamer, que no lo conoce: ahí se piden a un servidor que winshotx abre en 127.0.0.1
 * (`src-tauri/src/medios.rs`). Rust deja su dirección en la ventana antes de que cargue la
 * página, así que esto no tiene que esperar a nadie.
 *
 * Las imágenes siguen con `convertFileSrc`: esto es solo para lo que se reproduce.
 */
import { convertFileSrc } from "@tauri-apps/api/core";

declare global {
  interface Window {
    /** `http://127.0.0.1:PUERTO/FICHA`, solo en Linux. */
    __WINSHOTX_MEDIOS__?: string;
  }
}

export function urlDeMedio(ruta: string): string {
  const base = window.__WINSHOTX_MEDIOS__;
  // La ruta va entera en un solo tramo, barras incluidas: el servidor la decodifica él.
  return typeof base === "string" && base !== "" ? `${base}/${encodeURIComponent(ruta)}` : convertFileSrc(ruta);
}
