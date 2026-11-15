import type { ReactNode } from "react";
import { AbsoluteFill, Sequence, interpolate, useCurrentFrame } from "remotion";
import { OVERLAY } from "./app";
import { COLOR, s } from "./casa";
import { Camara, type Toma } from "./piezas/Camara";
import { AJUSTES, Arriba, Centro, Cierre, EDITOR, EnElEditor, EnLosAjustes, centroDelRecorte } from "./piezas/Escenas";
import { Logo } from "./piezas/Logo";
import { Overlay } from "./piezas/Overlay";
import { Palabras } from "./piezas/Palabras";
import { Teclas } from "./piezas/Teclas";

/**
 * El vídeo de la 0.5: la dirección 3, «Lo hice yo», que eligió Munito (15-nov), con lo que
 * pidió al elegirla: más larga (de 60 a 75 s), con aire en las cinco preguntas y la app a la
 * vista más tiempo, la historia ENCIMA de la app moviéndose (no sobre negro: en un muro mudo
 * nadie espera nueve segundos a ver la app) y SIN efectos de sonido («no me gustan»).
 *
 * Este render va mudo: la música la elige él entre las candidatas y se le pone después con
 * ffmpeg (`musica.mjs`), igualada a −16 LUFS y con su fundido al final.
 */
export const T = {
  lento: 0,
  harto: s(3.2),
  hice: s(6),
  nombre: s(8.8),
  rapido: s(12),
  graba: s(22.4),
  estudio: s(31.6),
  ultimos: s(40.8),
  peso: s(50),
  libre: s(57),
  cierre: s(61),
  fin: s(69),
};

/** Lo que dura sola cada pregunta, grande y en el centro, antes de que la conteste la app. */
const PREGUNTA = s(1.5);

/**
 * El velo de detrás de un texto, que cambia con el tiempo: cada clave es [fotograma, cuánto
 * oscurece, cuánto desenfoca]. Así la app se va viendo más nítida a medida que la historia
 * llega a ella.
 */
function VeloVivo({ claves }: { claves: [number, number, number][] }) {
  const frame = useCurrentFrame();
  const fs = claves.map((c) => c[0]);
  const o = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;
  const fuerza = interpolate(frame, fs, claves.map((c) => c[1]), o);
  const desenfoque = interpolate(frame, fs, claves.map((c) => c[2]), o);
  return (
    <AbsoluteFill
      style={{
        backdropFilter: desenfoque > 0.2 ? `blur(${desenfoque}px)` : undefined,
        background: `radial-gradient(ellipse 70% 60% at 50% 50%, rgba(11,11,13,${fuerza}), rgba(11,11,13,${fuerza * 0.6}) 75%)`,
      }}
    />
  );
}

/**
 * Una pregunta: primero sola en el centro con la app desenfocada detrás, y después la app
 * nítida contestándola, con la respuesta arriba.
 */
function Pregunta({ texto, respuesta, dura, app, respuestaEn = PREGUNTA + 6 }: { texto: string; respuesta: string; dura: number; app: ReactNode; respuestaEn?: number }) {
  return (
    <AbsoluteFill>
      {app}
      <VeloVivo claves={[[0, 0.85, 16], [PREGUNTA - 6, 0.85, 16], [PREGUNTA + 8, 0, 0]]} />
      <Sequence durationInFrames={PREGUNTA}>
        <Centro>
          <Palabras texto={texto} tam={112} sale={PREGUNTA - 8} />
        </Centro>
      </Sequence>
      <Sequence from={respuestaEn} durationInFrames={dura - respuestaEn}>
        <Arriba texto={respuesta} dura={dura - respuestaEn} tam={64} />
      </Sequence>
    </AbsoluteFill>
  );
}

export function Promo() {
  const frame = useCurrentFrame();
  const camaraHistoria: Toma[] = [
    { f: 0, x: 960, y: 540, z: 1 },
    { f: T.nombre, x: 900, y: 560, z: 1.12 },
    { f: T.rapido, x: 880, y: 570, z: 1.16 },
  ];
  return (
    <AbsoluteFill style={{ background: COLOR.fondo }}>
      {/* La historia, encima de tu pantalla: primero quieta y, al «so I built my own», se
          congela debajo y aparece winshotx. */}
      <Sequence durationInFrames={T.rapido}>
        <Camara tomas={camaraHistoria}>
          <Overlay aparece={T.hice + 10} desde={0} ritmo={0.55} hasta={OVERLAY.mira} />
        </Camara>
        <VeloVivo
          claves={[
            [0, 0.8, 14],
            [T.hice, 0.8, 14],
            [T.hice + 16, 0.62, 5],
            [T.nombre - 4, 0.62, 5],
            [T.nombre + 8, 0.88, 18],
          ]}
        />
        <Sequence from={T.lento} durationInFrames={T.harto - T.lento}>
          <Centro>
            <Palabras texto={"The Snipping Tool takes\n*920 ms* to open"} tam={100} entra={8} sale={T.harto - T.lento - 10} />
          </Centro>
        </Sequence>
        <Sequence from={T.harto} durationInFrames={T.hice - T.harto}>
          <Centro>
            <Palabras texto="I got tired of *waiting*" tam={100} sale={T.hice - T.harto - 10} />
          </Centro>
        </Sequence>
        <Sequence from={T.hice} durationInFrames={T.nombre - T.hice}>
          <Centro>
            <Palabras texto="So I built *my own*" tam={100} sale={T.nombre - T.hice - 10} />
          </Centro>
        </Sequence>
        <Sequence from={T.nombre} durationInFrames={T.rapido - T.nombre}>
          <Centro>
            <Logo tam={200} entra={4} />
          </Centro>
        </Sequence>
      </Sequence>

      {/* 1. ¿Rápido? El atajo y el overlay de verdad, de la flecha a la barra. */}
      <Sequence from={T.rapido} durationInFrames={T.graba - T.rapido}>
        <Pregunta
          texto="How fast?"
          dura={T.graba - T.rapido}
          respuestaEn={PREGUNTA + s(3.4)}
          respuesta="*114 ms*, with three screens"
          app={
            <Camara
              tomas={[
                { f: 0, x: 960, y: 540, z: 1 },
                { f: PREGUNTA + s(1.6), x: 960, y: 540, z: 1 },
                { f: PREGUNTA + s(4.4), x: centroDelRecorte.x, y: centroDelRecorte.y, z: 1.45 },
                { f: PREGUNTA + s(6.6), x: centroDelRecorte.x, y: centroDelRecorte.y + 40, z: 1.65 },
                { f: T.graba - T.rapido, x: centroDelRecorte.x, y: centroDelRecorte.y + 40, z: 1.72 },
              ]}
            >
              <Overlay aparece={PREGUNTA + s(1.4)} desde={OVERLAY.vaAlBoton} ritmo={1} />
            </Camara>
          }
        />
        <Sequence from={PREGUNTA + 2} durationInFrames={s(1.5)}>
          <VeloVivo claves={[[0, 0.7, 8], [s(1.2), 0.7, 8], [s(1.5), 0, 0]]} />
          <Centro>
            <Teclas teclas={["Ctrl", "Shift", "2"]} entra={4} cada={5} tam={96} />
          </Centro>
        </Sequence>
      </Sequence>

      {/* 2. ¿Graba? El editor entero, su línea de tiempo y la pestaña GIF. */}
      <Sequence from={T.graba} durationInFrames={T.estudio - T.graba}>
        <Pregunta
          texto="Does it record?"
          dura={T.estudio - T.graba}
          respuesta="*GIF* or MP4, trimmed to the frame"
          app={
            <EnElEditor
              gif={PREGUNTA + s(3.6)}
              tomas={[
                { f: 0, x: 960, y: 560, z: 0.92 },
                { f: PREGUNTA + s(2.6), x: 960, y: 600, z: 1.02 },
                { f: PREGUNTA + s(5.4), x: EDITOR.x + 1150, y: EDITOR.y + 330, z: 1.35 },
                { f: T.estudio - T.graba, x: EDITOR.x + 1150, y: EDITOR.y + 320, z: 1.42 },
              ]}
            />
          }
        />
      </Sequence>

      {/* 3. ¿Soso? El fondo del Estudio. */}
      <Sequence from={T.estudio} durationInFrames={T.ultimos - T.estudio}>
        <Pregunta
          texto="Too plain?"
          dura={T.ultimos - T.estudio}
          respuesta="Studio look in *one click*"
          app={
            <EnElEditor
              fondo={PREGUNTA + s(2.2)}
              tomas={[
                { f: 0, x: EDITOR.x + 560, y: EDITOR.y + 390, z: 1.08 },
                { f: T.ultimos - T.estudio, x: EDITOR.x + 560, y: EDITOR.y + 380, z: 1.32 },
              ]}
            />
          }
        />
      </Sequence>

      {/* 4. ¿Llegaste tarde? Los últimos segundos, en los Ajustes nuevos. */}
      <Sequence from={T.ultimos} durationInFrames={T.peso - T.ultimos}>
        <Pregunta
          texto="Missed the moment?"
          dura={T.peso - T.ultimos}
          respuesta="Keep the *last 30 seconds*"
          app={
            <EnLosAjustes
              tomas={[
                { f: 0, x: 960, y: 540, z: 1.05 },
                { f: PREGUNTA + s(2.5), x: 960, y: 560, z: 1.12 },
                { f: T.peso - T.ultimos, x: AJUSTES.x + 290, y: AJUSTES.y + 230, z: 1.6 },
              ]}
            />
          }
        />
      </Sequence>

      {/* 5. ¿Pesa? Las cifras, con la app quieta y desenfocada detrás. */}
      <Sequence from={T.peso} durationInFrames={T.libre - T.peso}>
        <EnElEditor fondo={-10} tomas={[{ f: 0, x: 960, y: 540, z: 1 }, { f: T.libre - T.peso, x: 960, y: 540, z: 0.94 }]} />
        <VeloVivo claves={[[0, 0.86, 16]]} />
        <Sequence durationInFrames={PREGUNTA}>
          <Centro>
            <Palabras texto="Is it heavy?" tam={112} sale={PREGUNTA - 8} />
          </Centro>
        </Sequence>
        <Sequence from={PREGUNTA}>
          <Centro y={-50}>
            <Palabras texto="*3.00 MB* to install" tam={104} />
          </Centro>
          <Centro y={80}>
            <Palabras texto="33 MB of memory while it runs" entra={s(1.2)} cada={3} tam={50} peso={500} color={COLOR.suave} />
          </Centro>
        </Sequence>
      </Sequence>

      {/* Lo que es, sobre la app, y el cierre. */}
      <Sequence from={T.libre} durationInFrames={T.cierre - T.libre}>
        <EnLosAjustes tomas={[{ f: 0, x: 960, y: 540, z: 0.9 }, { f: T.cierre - T.libre, x: 960, y: 540, z: 0.84 }]} img="ajustes-capturar.png" />
        <VeloVivo claves={[[0, 0.86, 16]]} />
        <Centro>
          <Palabras texto={"Free, open source,\n*no account*"} tam={100} sale={T.cierre - T.libre - 10} />
        </Centro>
      </Sequence>
      <Sequence from={T.cierre}>
        <Cierre linea="For Windows 10 and 11" />
      </Sequence>
      {/* Un fundido a negro en el último segundo, para que el bucle de Reddit no salte en seco. */}
      <AbsoluteFill style={{ background: "#000", opacity: interpolate(frame, [T.fin - s(1), T.fin - 1], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }) }} />
    </AbsoluteFill>
  );
}
