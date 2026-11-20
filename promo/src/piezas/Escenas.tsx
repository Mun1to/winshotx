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

/**
 * Los ajustes de la 0.5 (por defecto en el apartado de los últimos segundos), con la cámara por
 * dentro. Con `luego`, otra foto del mismo apartado entra encima en `en` (lo que cambia al
 * pulsar algo: las dos son del banco, antes y después); y lo de `children` va en el escenario,
 * dentro de la cámara (el puntero).
 */
export function EnLosAjustes({ tomas, img = "ajustes-ultimos.png", luego, children }: { tomas: Toma[]; img?: string; luego?: { img: string; en: number }; children?: ReactNode }) {
  const frame = useCurrentFrame();
  // Lo que tarda un interruptor de verdad en cambiar: unos 130 ms, con entrada y salida suaves.
  const cambio = luego ? interpolate(frame, [luego.en, luego.en + 4], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.inOut(Easing.cubic) }) : 0;
  return (
    <AbsoluteFill>
      <Fondo luz={0.6} />
      <Camara tomas={tomas}>
        <div style={{ position: "absolute", left: AJUSTES.x, top: AJUSTES.y, width: AJUSTES.w, height: AJUSTES.h, borderRadius: 10, overflow: "hidden", boxShadow: "0 30px 80px -20px #000d, 0 0 0 1px #ffffff14" }}>
          <Img src={foto(img)} style={{ width: AJUSTES.w, height: AJUSTES.h }} />
          {luego && <Img src={foto(luego.img)} style={{ position: "absolute", left: 0, top: 0, width: AJUSTES.w, height: AJUSTES.h, opacity: cambio }} />}
        </div>
        {children}
      </Camara>
    </AbsoluteFill>
  );
}

/**
 * Lo más que ocupa una pantalla de Linux en el escenario: queda fondo alrededor, para que se lea
 * como otra pantalla (la de Windows, en la primera pregunta, ocupa el cuadro entero). Va 50 px por
 * debajo del centro para que la respuesta de arriba no tape la barra de GNOME, que es lo que dice
 * «esto es Linux» de un vistazo.
 */
export const PANTALLA_LINUX = { w: 1440, h: 810, bajada: 50 };

/** Dónde queda una foto de `ancho` x `alto` dentro de `PANTALLA_LINUX`, centrada y entera. */
export function cajaDeLinux(ancho: number, alto: number) {
  const k = Math.min(PANTALLA_LINUX.w / ancho, PANTALLA_LINUX.h / alto);
  const w = Math.round(ancho * k);
  const h = Math.round(alto * k);
  return { x: (ANCHO - w) / 2, y: (ALTO - h) / 2 + PANTALLA_LINUX.bajada, w, h };
}

/**
 * winshotx en un escritorio de Linux (las fotos de `linux.ts`): la pantalla entera, con su barra
 * y sus ventanas, flotando sobre el fondo, y la cámara por dentro. Si hay una segunda foto (la de
 * X11), entra en `cambia`: la primera se va desenfocándose y la segunda llega desenfocada, como
 * los cortes entre escenas, pero en medio segundo y sin mover la cámara.
 */
export function EnLinux({ fotos, tomas, cambia, dura }: { fotos: { src: string; ancho: number; alto: number; tapar?: { x: number; y: number; w: number; h: number }[] }[]; tomas: Toma[]; cambia?: number; dura: number }) {
  const frame = useCurrentFrame();
  const o = { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.inOut(Easing.cubic) } as const;
  // De 0 (solo la primera) a 1 (solo la segunda), en 15 fotogramas centrados en `cambia`.
  const paso = cambia === undefined || fotos.length < 2 ? 0 : interpolate(frame, [cambia - 7, cambia + 8], [0, 1], o);
  // La pantalla entra y sale fundiéndose sobre el fondo oscuro: un escritorio claro (un fondo de
  // pantalla, un tema claro) entre dos escenas oscuras daba un salto de brillo en el corte (medido
  // con YDIF: 21 y 24 al entrar y al salir, frente a 10 en los demás cortes).
  // La salida empieza antes que el velo del corte, para que la bajada de brillo no se junte en sus 9 fotogramas.
  const entera = interpolate(frame, [0, 10, dura - 22, dura - 4], [0, 1, 1, 0], o);
  return (
    <AbsoluteFill>
      <Fondo luz={0.6} />
      <Camara tomas={tomas}>
        {fotos.slice(0, 2).map((f, i) => {
          const c = cajaDeLinux(f.ancho, f.alto);
          const v = (i === 0 ? 1 - paso : paso) * entera;
          if (v <= 0.001) return null;
          const borroso = Math.sin(Math.min(1, paso) * Math.PI) * 10;
          return (
            <div
              key={f.src}
              style={{
                position: "absolute",
                left: c.x,
                top: c.y,
                width: c.w,
                height: c.h,
                borderRadius: 12,
                overflow: "hidden",
                opacity: v,
                filter: borroso > 0.2 ? `blur(${borroso}px)` : undefined,
                boxShadow: "0 30px 80px -20px #000d, 0 0 0 1px #ffffff1f",
              }}
            >
              <Img src={f.src} style={{ width: c.w, height: c.h }} />
              {/* Lo que la foto trae y no es de winshotx (en GNOME, la pastilla naranja de «se está
                  grabando» que dejó el laboratorio), tapado con el negro de la barra. */}
              {(f.tapar ?? []).map((t, j) => {
                const k = c.w / f.ancho;
                return <div key={j} style={{ position: "absolute", left: t.x * k, top: t.y * k, width: t.w * k, height: t.h * k, background: "#000" }} />;
              })}
            </div>
          );
        })}
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
