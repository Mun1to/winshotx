import { AbsoluteFill, Sequence, interpolate, useCurrentFrame } from "remotion";
import { OVERLAY } from "../app";
import { COLOR, s } from "../casa";
import { Camara } from "../piezas/Camara";
import { AJUSTES, Arriba, Centro, Cierre, EDITOR, EnElEditor, EnLosAjustes, Velo, centroDelRecorte } from "../piezas/Escenas";
import { Fondo } from "../piezas/Fondo";
import { Logo } from "../piezas/Logo";
import { Overlay } from "../piezas/Overlay";
import { Palabras } from "../piezas/Palabras";
import { Efectos, SONIDO, teclasEn } from "../piezas/Sonidos";
import { Teclas } from "../piezas/Teclas";

/**
 * Dirección 3, «Lo hice yo»: el arranque solo con texto y en primera persona (lo que Munito
 * se llevó de su referencia de morphy), el nombre, y luego cinco preguntas que haría quien lo
 * ve, cada una contestada por la app haciéndolo. Es la que más se parece a un post de Reddit:
 * alguien que cuenta por qué lo hizo.
 */
export const D3 = {
  lento: 0,
  yo: s(2.4),
  hice: s(4.4),
  nombre: s(6.4),
  rapido: s(8.6),
  graba: s(13.6),
  estudio: s(18.6),
  ultimos: s(23.6),
  peso: s(28.6),
  cierre: s(33.6),
  fin: s(42),
};

/** Una pregunta grande en el centro, sola, antes de que la app la conteste. */
function Pregunta({ texto, dura = s(1.1) }: { texto: string; dura?: number }) {
  return (
    <Sequence durationInFrames={dura}>
      <Fondo />
      <Centro>
        <Palabras texto={texto} tam={110} sale={dura - 8} />
      </Centro>
    </Sequence>
  );
}

/** La respuesta, arriba y suelta, encima de la app. */
function Respuesta({ texto, entra, dura }: { texto: string; entra: number; dura: number }) {
  return (
    <Sequence from={entra} durationInFrames={dura}>
      <Arriba texto={texto} dura={dura} />
    </Sequence>
  );
}

export function DireccionLoHiceYo() {
  const frame = useCurrentFrame();
  const escritorio = interpolate(frame, [D3.rapido + s(1.1), D3.rapido + s(1.3)], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const P = s(1.1);
  return (
    <AbsoluteFill style={{ background: COLOR.fondo }}>
      <Fondo />
      {/* La historia, solo texto, en primera persona. */}
      <Sequence from={D3.lento} durationInFrames={D3.yo - D3.lento}>
        <Centro>
          <Palabras texto={"The Snipping Tool takes\n*920 ms* to open"} tam={96} sale={D3.yo - D3.lento - 10} />
        </Centro>
      </Sequence>
      <Sequence from={D3.yo} durationInFrames={D3.hice - D3.yo}>
        <Centro>
          <Palabras texto="I got tired of *waiting*" tam={96} sale={D3.hice - D3.yo - 10} />
        </Centro>
      </Sequence>
      <Sequence from={D3.hice} durationInFrames={D3.nombre - D3.hice}>
        <Centro>
          <Palabras texto="So I built *my own*" tam={96} sale={D3.nombre - D3.hice - 8} />
        </Centro>
      </Sequence>
      <Sequence from={D3.nombre} durationInFrames={D3.rapido - D3.nombre}>
        <Centro>
          <Logo tam={190} />
        </Centro>
      </Sequence>

      {/* ¿Rápido? El atajo y el overlay de verdad. */}
      <Sequence from={D3.rapido} durationInFrames={D3.graba - D3.rapido}>
        <Pregunta texto="How fast?" dura={P} />
        <Sequence from={P}>
          <AbsoluteFill style={{ opacity: escritorio }}>
            <Camara tomas={[{ f: 0, x: 960, y: 540, z: 1 }, { f: s(1.6), x: 960, y: 540, z: 1 }, { f: s(3.9), x: centroDelRecorte.x, y: centroDelRecorte.y, z: 1.5 }]}>
              <Overlay aparece={s(0.9)} desde={OVERLAY.vaAlBoton} ritmo={1.4} />
            </Camara>
          </AbsoluteFill>
          <Sequence durationInFrames={s(1)}>
            <Velo fuerza={0.7} desenfoque={8} />
            <Centro>
              <Teclas teclas={["Ctrl", "Shift", "2"]} entra={2} cada={4} tam={88} />
            </Centro>
          </Sequence>
        </Sequence>
        <Respuesta texto="*114 ms*, with three screens" entra={P + s(1.1)} dura={s(2.8)} />
      </Sequence>

      {/* ¿Graba? El editor con su línea de tiempo y la exportación. */}
      <Sequence from={D3.graba} durationInFrames={D3.estudio - D3.graba}>
        <Pregunta texto="Does it record?" dura={P} />
        <Sequence from={P}>
          <EnElEditor gif={s(1.6)} tomas={[{ f: 0, x: 960, y: 560, z: 0.92 }, { f: s(1.4), x: 960, y: 560, z: 0.96 }, { f: s(3.4), x: EDITOR.x + 1150, y: EDITOR.y + 330, z: 1.35 }]} />
        </Sequence>
        <Respuesta texto="*GIF* or MP4, trimmed to the frame" entra={P + 4} dura={s(3.7)} />
      </Sequence>

      {/* ¿Feo? El Estudio. */}
      <Sequence from={D3.estudio} durationInFrames={D3.ultimos - D3.estudio}>
        <Pregunta texto="Too plain?" dura={P} />
        <Sequence from={P}>
          <EnElEditor fondo={s(1)} tomas={[{ f: 0, x: EDITOR.x + 560, y: EDITOR.y + 390, z: 1.1 }, { f: s(3.6), x: EDITOR.x + 560, y: EDITOR.y + 380, z: 1.3 }]} />
        </Sequence>
        <Respuesta texto="Studio look in *one click*" entra={P + 4} dura={s(3.7)} />
      </Sequence>

      {/* ¿Llegaste tarde? Los últimos segundos. */}
      <Sequence from={D3.ultimos} durationInFrames={D3.peso - D3.ultimos}>
        <Pregunta texto="Missed the moment?" dura={P} />
        <Sequence from={P}>
          <EnLosAjustes tomas={[{ f: 0, x: 960, y: 540, z: 1.1 }, { f: s(3.6), x: AJUSTES.x + 290, y: AJUSTES.y + 220, z: 1.7 }]} />
        </Sequence>
        <Respuesta texto="Keep the *last 30 seconds*" entra={P + 4} dura={s(3.7)} />
      </Sequence>

      {/* ¿Pesa? */}
      <Sequence from={D3.peso} durationInFrames={D3.cierre - D3.peso}>
        <Pregunta texto="Is it heavy?" dura={P} />
        <Sequence from={P}>
          <Fondo />
          <Centro y={-50}>
            <Palabras texto="*3.00 MB* to install" tam={100} />
          </Centro>
          <Centro y={80}>
            <Palabras texto="33 MB of memory while it runs" entra={s(0.9)} cada={3} tam={48} peso={500} color={COLOR.suave} />
          </Centro>
        </Sequence>
      </Sequence>

      <Sequence from={D3.cierre}>
        <Cierre linea="Free, open source, no account" />
      </Sequence>
      <Efectos
        lista={[
          { en: D3.lento + 2, src: SONIDO.frase, nivel: 0.35 },
          { en: D3.yo + 2, src: SONIDO.frase, nivel: 0.35 },
          { en: D3.hice + 2, src: SONIDO.frase, nivel: 0.35 },
          { en: D3.nombre, src: SONIDO.congela, nivel: 0.55 },
          { en: D3.rapido, src: SONIDO.frase, nivel: 0.35 },
          ...teclasEn(D3.rapido + P + 2, 3, 4),
          { en: D3.rapido + P + s(0.9), src: SONIDO.congela, nivel: 0.6 },
        ]}
      />
    </AbsoluteFill>
  );
}
