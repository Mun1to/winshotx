import { spring, useCurrentFrame, useVideoConfig } from "remotion";
import { COLOR, LETRA } from "../casa";

/**
 * Un atajo dicho como se pulsa: cada tecla entra y se hunde un poco al pulsarse, una detrás
 * de otra. Suelto, sin cajitas (en la 0.5 el atajo en cajas fue «no me gusta el diseño»).
 */
export function Teclas({ teclas, entra = 0, cada = 5, tam = 64 }: { teclas: string[]; entra?: number; cada?: number; tam?: number }) {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return (
    <div style={{ display: "flex", gap: tam * 0.45, alignItems: "baseline", fontFamily: LETRA.titulo, fontWeight: 600, fontSize: tam, color: COLOR.texto }}>
      {teclas.map((t, i) => {
        const f = frame - entra - i * cada;
        const v = spring({ frame: f, fps, config: { damping: 200, mass: 0.4 } });
        // Se hunde al pulsarse y vuelve: un 8 % en tres fotogramas.
        const pulsa = f >= 0 && f < 6 ? 1 - 0.08 * Math.sin((f / 6) * Math.PI) : 1;
        return (
          <span key={i} style={{ display: "inline-flex", alignItems: "baseline", gap: tam * 0.45 }}>
            {i > 0 && <span style={{ color: COLOR.apagado, fontWeight: 400, opacity: v }}>+</span>}
            <span style={{ display: "inline-block", opacity: v, transform: `translateY(${(1 - v) * 18}px) scale(${pulsa})` }}>{t}</span>
          </span>
        );
      })}
    </div>
  );
}
