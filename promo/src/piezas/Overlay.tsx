import { AbsoluteFill, Img, interpolate, useCurrentFrame } from "remotion";
import { MEDIDAS, foto, fotoDelOverlay } from "../app";
import { ALTO, ANCHO } from "../casa";
import { Cursor } from "./Cursor";

/**
 * La pantalla y el overlay de winshotx encima, sacados de la app de verdad.
 *
 * Hasta `aparece` se ve el escritorio (la guía de winshotx.com) con la flecha quieta donde
 * empieza el recorrido; desde ahí, el overlay fotograma a fotograma: la pantalla congelada y
 * oscurecida, la cruz, la lupa y el recorte. `desde` es por qué fotograma del recorrido se
 * entra, y `ritmo` lo frena o lo acelera (0,2 = cinco veces más lento).
 */
export function Overlay({ aparece = 0, desde = 0, ritmo = 1, hasta }: { aparece?: number; desde?: number; ritmo?: number; hasta?: number }) {
  const frame = useCurrentFrame();
  const { ratones } = MEDIDAS.overlay;
  if (frame < aparece) {
    const [x, y] = ratones[desde];
    return (
      <AbsoluteFill>
        <Img src={foto("escritorio.png")} style={{ width: ANCHO, height: ALTO }} />
        <Cursor x={x} y={y} forma="flecha" />
      </AbsoluteFill>
    );
  }
  const n = Math.floor(desde + (frame - aparece) * ritmo);
  const i = Math.min(hasta ?? MEDIDAS.overlay.fotogramas - 1, n);
  const [x, y] = ratones[Math.min(i, ratones.length - 1)];
  // La pantalla se congela de golpe, pero la cámara de la foto no: un destello muy corto
  // para que se note el momento en que winshotx coge la pantalla.
  const destello = interpolate(frame - aparece, [0, 2, 7], [0, 0.18, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <AbsoluteFill>
      <Img src={fotoDelOverlay(i)} style={{ width: ANCHO, height: ALTO }} />
      <Cursor x={x} y={y} forma={i >= 117 ? "flecha" : "cruz"} />
      <AbsoluteFill style={{ background: "#fff", opacity: destello }} />
    </AbsoluteFill>
  );
}
