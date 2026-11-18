import { interpolate, useCurrentFrame } from "remotion";
import { COLOR, LETRA } from "../casa";

/**
 * Dos cifras medidas, una contra otra, contando a la vez: el número grande con su nombre al
 * lado y una barra fina de un solo color que crece con él (la de winshotx en azul y la otra en
 * el gris de «la otra herramienta» de la web). Sin cajas.
 *
 * `cuenta` es cuántos fotogramas tarda la cifra MÁS grande en llegar; la pequeña llega antes,
 * en proporción: es una carrera de verdad, no dos animaciones del mismo largo.
 */
export function Carrera({
  filas,
  unidad,
  entra = 0,
  cuenta,
  ancho = 1100,
}: {
  filas: { nombre: string; valor: number; nuestra?: boolean }[];
  unidad: string;
  entra?: number;
  cuenta: number;
  ancho?: number;
}) {
  const frame = useCurrentFrame() - entra;
  const max = Math.max(...filas.map((f) => f.valor));
  const corre = interpolate(frame, [0, cuenta], [0, max], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <div style={{ width: ancho, display: "flex", flexDirection: "column", gap: 56 }}>
      {filas.map((f, i) => {
        const v = Math.min(corre, f.valor);
        const llego = corre >= f.valor;
        const aparece = interpolate(frame, [i * 4, i * 4 + 10], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
        return (
          <div key={f.nombre} style={{ opacity: aparece }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", fontFamily: LETRA.titulo }}>
              <span style={{ fontSize: 54, fontWeight: 600, color: f.nuestra ? COLOR.texto : COLOR.suave }}>{f.nombre}</span>
              <span
                style={{
                  fontSize: 112,
                  fontWeight: 700,
                  letterSpacing: "-0.03em",
                  fontVariantNumeric: "tabular-nums",
                  color: f.nuestra ? (llego ? COLOR.azul : COLOR.texto) : COLOR.suave,
                }}
              >
                {Math.round(v)}
                <span style={{ fontSize: 56, fontWeight: 600, marginLeft: 10 }}>{unidad}</span>
              </span>
            </div>
            <div style={{ height: 8, marginTop: 10, borderRadius: 4, background: "rgba(255,255,255,0.06)" }}>
              <div
                style={{
                  height: 8,
                  borderRadius: 4,
                  width: `${(v / max) * 100}%`,
                  background: f.nuestra ? COLOR.azul : COLOR.otra,
                  boxShadow: f.nuestra && llego ? `0 0 24px ${COLOR.azul}` : undefined,
                }}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
