import type { ReactNode } from "react";
import { AbsoluteFill, Easing, interpolate, useCurrentFrame } from "remotion";
import { ALTO, ANCHO } from "../casa";

/** Un punto de la cámara: en el fotograma `f` mira al punto (x, y) del escenario con zoom `z`. */
export type Toma = { f: number; x: number; y: number; z: number };

/** Dónde está la cámara en un fotograma, entre sus tomas, con la curva de quien la mueve a mano. */
export function camaraEn(frame: number, tomas: Toma[]) {
  if (tomas.length === 1) return tomas[0];
  const fs = tomas.map((t) => t.f);
  const o = { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.bezier(0.65, 0, 0.35, 1) } as const;
  return {
    f: frame,
    x: interpolate(frame, fs, tomas.map((t) => t.x), o),
    y: interpolate(frame, fs, tomas.map((t) => t.y), o),
    z: interpolate(frame, fs, tomas.map((t) => t.z), o),
  };
}

/**
 * Lo de dentro es un escenario de 1920 x 1080 (los píxeles de CSS de la pantalla de mentira
 * del banco), y la cámara se acerca y se mueve por él. Es la transición que usan las
 * referencias: no hay fundidos, la cámara entra hasta lo que importa y sale.
 */
export function Camara({ tomas, children }: { tomas: Toma[]; children: ReactNode }) {
  const frame = useCurrentFrame();
  const c = camaraEn(frame, tomas);
  return (
    <AbsoluteFill style={{ overflow: "hidden" }}>
      <div
        style={{
          position: "absolute",
          width: ANCHO,
          height: ALTO,
          transformOrigin: "0 0",
          transform: `translate(${ANCHO / 2 - c.x * c.z}px, ${ALTO / 2 - c.y * c.z}px) scale(${c.z})`,
        }}
      >
        {children}
      </div>
    </AbsoluteFill>
  );
}
