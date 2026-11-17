import { AbsoluteFill } from "remotion";
import { COLOR } from "../casa";

/** El suelo de la pieza: el negro de la portada con su luz azul detrás del texto. */
export function Fondo({ luz = 1 }: { luz?: number }) {
  return (
    <AbsoluteFill style={{ background: COLOR.fondo }}>
      <AbsoluteFill
        style={{
          opacity: luz,
          background: `radial-gradient(ellipse 55% 45% at 50% 46%, ${COLOR.halo}, transparent 70%)`,
        }}
      />
    </AbsoluteFill>
  );
}
