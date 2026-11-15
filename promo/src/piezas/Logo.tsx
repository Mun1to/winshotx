import { Easing, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { COLOR, LETRA } from "../casa";

/**
 * El logo de winshotx (`frontlaxweb/logo.svg`) que se monta solo: las cuatro esquinas de
 * recorte entran desde fuera y se cierran, y el aspa se dibuja dentro. Es lo que hace la app
 * (encuadrar algo), así que el logo se cuenta a sí mismo. Al lado, el nombre.
 */
export function Logo({ entra = 0, tam = 200, nombre = true }: { entra?: number; tam?: number; nombre?: boolean }) {
  const frame = useCurrentFrame() - entra;
  const { fps } = useVideoConfig();
  const cierra = spring({ frame, fps, config: { damping: 16, mass: 0.7 } });
  const lejos = (1 - cierra) * 18;
  const aspa = interpolate(frame, [8, 22], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.out(Easing.cubic) });
  const letras = spring({ frame: frame - 14, fps, config: { damping: 200 } });
  const esquinas = [
    { d: "M8 22v-8a6 6 0 0 1 6-6h8", dx: -lejos, dy: -lejos },
    { d: "M42 8h8a6 6 0 0 1 6 6v8", dx: lejos, dy: -lejos },
    { d: "M56 42v8a6 6 0 0 1-6 6h-8", dx: lejos, dy: lejos },
    { d: "M22 56h-8a6 6 0 0 1-6-6v-8", dx: -lejos, dy: lejos },
  ];
  return (
    <div style={{ display: "flex", alignItems: "center", gap: tam * 0.22 }}>
      <svg width={tam} height={tam} viewBox="0 0 64 64" style={{ overflow: "visible", opacity: Math.min(1, cierra * 2) }}>
        <g fill="none" stroke={COLOR.azul} strokeLinecap="round" strokeWidth={8}>
          {esquinas.map((e, i) => (
            <path key={i} d={e.d} transform={`translate(${e.dx} ${e.dy})`} />
          ))}
        </g>
        <g stroke="#fff" strokeWidth={8.5} strokeLinecap="round" fill="none">
          <path d="M24 24 40 40" pathLength={1} strokeDasharray={1} strokeDashoffset={aspa} />
          <path d="M40 24 24 40" pathLength={1} strokeDasharray={1} strokeDashoffset={aspa} />
        </g>
      </svg>
      {nombre && (
        <div
          style={{
            fontFamily: LETRA.titulo,
            fontWeight: 700,
            fontSize: tam * 0.62,
            letterSpacing: "-0.03em",
            color: COLOR.texto,
            opacity: letras,
            transform: `translateX(${(1 - letras) * -30}px)`,
          }}
        >
          winshotx
        </div>
      )}
    </div>
  );
}
