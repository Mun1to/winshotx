import type { ReactNode } from "react";
import { AbsoluteFill, Easing, Img, interpolate, useCurrentFrame } from "remotion";
import { MEDIDAS, foto } from "../app";
import { ALTO, ANCHO, COLOR } from "../casa";
import { Camara, type Toma } from "./Camara";
import { Fondo } from "./Fondo";
import { Logo } from "./Logo";
import { Palabras } from "./Palabras";

/** Lo de detrás de un texto que va en el centro, encima de la app: se oscurece y se
    desenfoca para que la frase se lea sola (sin caja). */
export function Velo({ fuerza = 0.82, entra = 0, desenfoque = 14 }: { fuerza?: number; entra?: number; desenfoque?: number }) {
  const frame = useCurrentFrame();
  const v = interpolate(frame, [entra, entra + 8], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <AbsoluteFill
      style={{
        backdropFilter: `blur(${v * desenfoque}px)`,
        background: `radial-gradient(ellipse 70% 60% at 50% 50%, rgba(11,11,13,${v * fuerza}), rgba(11,11,13,${v * fuerza * 0.6}) 75%)`,
      }}
    />
  );
}

/**
 * Una frase arriba, encima de la app que se mueve debajo, con una sombra suave que baja desde
 * el borde y se va con ella. Nada de franja ni de caja: es una luz, no un rectángulo.
 */
export function Arriba({ texto, entra = 0, dura, tam = 62, cada = 4, cuenta }: { texto: string; entra?: number; dura: number; tam?: number; cada?: number; cuenta?: number }) {
  const frame = useCurrentFrame() - entra;
  const v = interpolate(frame, [0, 8, dura - 10, dura - 2], [0, 1, 1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  if (frame < 0 || frame > dura) return null;
  return (
    <AbsoluteFill>
      <AbsoluteFill style={{ opacity: v, background: "radial-gradient(ellipse 62% 34% at 50% 6%, rgba(11,11,13,0.94), rgba(11,11,13,0.6) 55%, transparent 100%)" }} />
      <Centro y={-410}>
        <Palabras texto={texto} entra={entra} cada={cada} tam={tam} sale={entra + dura - 10} cuenta={cuenta} />
      </Centro>
    </AbsoluteFill>
  );
}

/** Un texto centrado en el cuadro. */
export function Centro({ children, y = 0 }: { children: ReactNode; y?: number }) {
  return (
    <AbsoluteFill style={{ alignItems: "center", justifyContent: "center", transform: `translateY(${y}px)` }}>{children}</AbsoluteFill>
  );
}

/** Lo que mide el editor en el escenario: su ventana de 1440 x 900 centrada. */
export const EDITOR = { x: (ANCHO - 1440) / 2, y: (ALTO - 900) / 2, w: 1440, h: 900 };
/** Y los ajustes, 900 x 723. */
export const AJUSTES = { x: (ANCHO - 900) / 2, y: (ALTO - 723) / 2, w: 900, h: 723 };

/**
 * El editor en el escenario: sin fondo y, a partir de `fondo`, con el «Sunset» del Estudio
 * puesto (las dos fotos son del banco, antes y después del clic), y la cámara por dentro.
 */
export function EnElEditor({ tomas, fondo, gif }: { tomas: Toma[]; fondo?: number; gif?: number }) {
  const frame = useCurrentFrame();
  // El fondo entra en un tercio de segundo, con entrada y salida suaves (antes, 6 fotogramas en recta).
  const conFondo = fondo === undefined ? 0 : interpolate(frame, [fondo, fondo + 10], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.inOut(Easing.cubic) });
  const conGif = gif === undefined ? 0 : interpolate(frame, [gif, gif + 4], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const caja = { position: "absolute" as const, left: EDITOR.x, top: EDITOR.y, width: EDITOR.w, height: EDITOR.h };
  return (
    <AbsoluteFill>
      <Fondo luz={0.6} />
      <Camara tomas={tomas}>
        <div style={{ ...caja, borderRadius: 10, overflow: "hidden", boxShadow: "0 30px 80px -20px #000d, 0 0 0 1px #ffffff14" }}>
          <Img src={foto("editor-sin-fondo.png")} style={{ ...caja, left: 0, top: 0 }} />
          <Img src={foto("editor-atardecer.png")} style={{ ...caja, left: 0, top: 0, opacity: conFondo }} />
          <Img src={foto("editor-gif.png")} style={{ ...caja, left: 0, top: 0, opacity: conGif }} />
        </div>
      </Camara>
    </AbsoluteFill>
  );
}

/** Los ajustes de la 0.5 en el apartado de los últimos segundos, con la cámara por dentro. */
export function EnLosAjustes({ tomas, img = "ajustes-ultimos.png" }: { tomas: Toma[]; img?: string }) {
  return (
    <AbsoluteFill>
      <Fondo luz={0.6} />
      <Camara tomas={tomas}>
        <div style={{ position: "absolute", left: AJUSTES.x, top: AJUSTES.y, width: AJUSTES.w, height: AJUSTES.h, borderRadius: 10, overflow: "hidden", boxShadow: "0 30px 80px -20px #000d, 0 0 0 1px #ffffff14" }}>
          <Img src={foto(img)} style={{ width: AJUSTES.w, height: AJUSTES.h }} />
        </div>
      </Camara>
    </AbsoluteFill>
  );
}

/** El cierre: el logo, la web y lo que es, sin nada más (un final con varias cosas apiladas
    le resultó «confuso» en Adeorq). */
export function Cierre({ linea = "Free and open source, for Windows." }: { linea?: string }) {
  return (
    <AbsoluteFill>
      <Fondo />
      <Centro y={-60}>
        <Logo tam={170} />
      </Centro>
      <Centro y={120}>
        <Palabras texto="*winshotx.com*" entra={20} tam={64} peso={600} />
      </Centro>
      <Centro y={210}>
        <Palabras texto={linea} entra={32} cada={3} tam={38} peso={500} color={COLOR.suave} />
      </Centro>
    </AbsoluteFill>
  );
}

/** Dónde queda el recorte del overlay en el escenario, para que la cámara vaya a él. */
export const RECORTE = MEDIDAS.overlay.recorte;
export const centroDelRecorte = { x: RECORTE.x + RECORTE.w / 2, y: RECORTE.y + RECORTE.h / 2 };
