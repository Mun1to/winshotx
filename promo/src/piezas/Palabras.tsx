import type { CSSProperties } from "react";
import { Easing, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { COLOR, LETRA } from "../casa";

/** Un renglón en palabras, y cada palabra en trozos: lo que va entre asteriscos, marcado. */
function palabrasDe(renglon: string) {
  let marca = false;
  return renglon
    .split(" ")
    .filter(Boolean)
    .map((cruda) => {
      const trozos: { texto: string; marca: boolean }[] = [];
      cruda.split("*").forEach((t, i) => {
        if (i > 0) marca = !marca;
        if (t) trozos.push({ texto: t, marca });
      });
      return trozos;
    });
}

/**
 * Una frase que entra palabra a palabra: cada una sale de un desenfoque y sube a su sitio,
 * una cada `cada` fotogramas desde `entra`, y se va entera en `sale` (si se dice). Lo que va
 * entre asteriscos sale en el azul de la marca; `\n` parte renglones.
 *
 * Es como entra todo lo que se lee: en las referencias de Munito el texto llega así, de dos
 * a cinco palabras, centrado, suelto (nunca en una caja) y con una sola palabra en color.
 */
export function Palabras({
  texto,
  entra = 0,
  cada = 4,
  sale,
  tam = 96,
  peso = 700,
  color = COLOR.texto,
  cuenta,
  style,
}: {
  texto: string;
  entra?: number;
  cada?: number;
  sale?: number;
  tam?: number;
  peso?: number;
  color?: string;
  /** Si se dice, las cifras («114», «3.00») cuentan desde cero en estos fotogramas al entrar. */
  cuenta?: number;
  style?: CSSProperties;
}) {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const fuera = sale === undefined ? 1 : interpolate(frame, [sale, sale + 8], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  let k = 0;
  return (
    <div
      style={{
        fontFamily: LETRA.titulo,
        fontSize: tam,
        fontWeight: peso,
        lineHeight: 1.08,
        letterSpacing: "-0.02em",
        color,
        textAlign: "center",
        opacity: fuera,
        filter: fuera < 0.99 ? `blur(${(1 - fuera) * 12}px)` : undefined,
        ...style,
      }}
    >
      {texto.split("\n").map((renglon, r) => (
        <div key={r}>
          {palabrasDe(renglon).map((trozos, i) => {
            const desde = frame - entra - k++ * cada;
            const v = spring({ frame: desde, fps, config: { damping: 200, mass: 0.45 } });
            // Una cifra que cuenta: sube deprisa y frena al llegar (curva de salida), con los
            // mismos decimales que la de verdad y cifras de ancho fijo para que no baile.
            const cifra = (t: string) => {
              const m = cuenta ? t.match(/^(\d+)(?:\.(\d+))?$/) : null;
              if (!m || !cuenta) return t;
              const p = interpolate(desde, [0, cuenta], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.out(Easing.cubic) });
              return (Number(t) * p).toFixed(m[2]?.length ?? 0);
            };
            return (
              <span key={i}>
                {i > 0 && " "}
                <span
                  style={{
                    display: "inline-block",
                    opacity: v,
                    transform: `translateY(${(1 - v) * 0.3}em)`,
                    filter: v < 0.98 ? `blur(${(1 - v) * 10}px)` : undefined,
                  }}
                >
                  {trozos.map((t, j) => (
                    <span key={j} style={{ ...(t.marca ? { color: COLOR.azul } : {}), ...(cuenta && /^\d/.test(t.texto) ? { fontVariantNumeric: "tabular-nums" } : {}) }}>
                      {cifra(t.texto)}
                    </span>
                  ))}
                </span>
              </span>
            );
          })}
        </div>
      ))}
    </div>
  );
}

/** Cuántos fotogramas tarda en estar entera una frase de `Palabras`. */
export const cuantoTarda = (texto: string, cada = 4) => texto.split(/\s+/).filter(Boolean).length * cada + 10;
