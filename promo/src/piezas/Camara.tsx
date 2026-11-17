import type { ReactNode } from "react";
import { AbsoluteFill, useCurrentFrame } from "remotion";
import { ALTO, ANCHO } from "../casa";

/** Un punto de la cámara: en el fotograma `f` mira al punto (x, y) del escenario con zoom `z`. */
export type Toma = { f: number; x: number; y: number; z: number };

/**
 * Una curva que pasa por todos los puntos sin frenar en los de en medio: arranca y acaba con
 * velocidad cero (entra y sale suave), y entre medias la velocidad cambia sin saltos (cúbica
 * monótona de Fritsch y Carlson, que además nunca se pasa de un punto y vuelve).
 *
 * Antes cada tramo llevaba su propia curva de entrada y salida, así que en cada punto de en medio
 * la cámara se paraba y volvía a arrancar, y en el centro de cada tramo iba al triple de su
 * velocidad media: «muy brusco» (Munito, 15-nov, 21:28).
 */
function suave(t: number, ts: number[], vs: number[]) {
  const n = ts.length;
  if (t <= ts[0]) return vs[0];
  if (t >= ts[n - 1]) return vs[n - 1];
  const h = ts.slice(1).map((v, i) => v - ts[i]);
  const d = h.map((hi, i) => (vs[i + 1] - vs[i]) / hi);
  const m = vs.map((_, i) => {
    if (i === 0 || i === n - 1) return 0;
    if (d[i - 1] * d[i] <= 0) return 0;
    const w1 = 2 * h[i] + h[i - 1];
    const w2 = h[i] + 2 * h[i - 1];
    return (w1 + w2) / (w1 / d[i - 1] + w2 / d[i]);
  });
  let i = 0;
  while (t > ts[i + 1]) i++;
  const u = (t - ts[i]) / h[i];
  const u2 = u * u;
  const u3 = u2 * u;
  return (2 * u3 - 3 * u2 + 1) * vs[i] + (u3 - 2 * u2 + u) * h[i] * m[i] + (-2 * u3 + 3 * u2) * vs[i + 1] + (u3 - u2) * h[i] * m[i + 1];
}

/**
 * Dónde está la cámara en un fotograma, entre sus tomas. El zoom va en escala logarítmica: pasar
 * de 1 a 2 se ve igual de rápido que de 2 a 4, y así no parece que acelere al acercarse.
 */
export function camaraEn(frame: number, tomas: Toma[]) {
  if (tomas.length === 1) return tomas[0];
  const fs = tomas.map((t) => t.f);
  return {
    f: frame,
    x: suave(frame, fs, tomas.map((t) => t.x)),
    y: suave(frame, fs, tomas.map((t) => t.y)),
    z: Math.exp(suave(frame, fs, tomas.map((t) => Math.log(t.z)))),
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
