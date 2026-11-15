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
 * El vídeo de la 0.5: la dirección 3, «Lo hice yo», que eligió Munito (15-nov), con la
 * historia ENCIMA de la app moviéndose y SIN efectos de sonido («no me gustan»).
 *
 * Segunda vuelta (15-nov, 19:05, «hazlo más rápido»): de 69 a 40,6 s con las mismas cinco
 * preguntas. Una idea nueva cada 2,2 s (sus tres preferidas de la carpeta de vídeos SaaS van
 * de 22 a 29 ideas por minuto; este, 28), cada pregunta sola 0,9 s en vez de 1,5, y ningún
 * texto menos tiempo del que pide leerlo (caracteres entre 17, la regla de la guía). Y el
 * atajo que se enseña es Win + Shift + S, el que él usa y el que winshotx puede quitarle a
 * Windows.
 *
 * Este render va mudo: la música la elige él entre las candidatas y se le pone después con
 * ffmpeg (`musica.mjs`): baja durante la historia y entra entera con el logo (`--sube`).
 */
export const T = {
  lento: 0,
  harto: s(2.6),
  hice: s(4.1),
  nombre: s(5.6),
  rapido: s(7.4),
  graba: s(14.6),
  estudio: s(19.8),
  ultimos: s(24.6),
  peso: s(29.4),
  libre: s(33.8),
  cierre: s(36.4),
  fin: s(40.6),
};

/** Lo que dura sola cada pregunta, grande y en el centro, antes de que la conteste la app. */
const PREGUNTA = s(0.9);

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
function Pregunta({ texto, respuesta, dura, app, respuestaEn = PREGUNTA + 4 }: { texto: string; respuesta: string; dura: number; app: ReactNode; respuestaEn?: number }) {
  return (
    <AbsoluteFill>
      {app}
      <VeloVivo claves={[[0, 0.85, 16], [PREGUNTA - 5, 0.85, 16], [PREGUNTA + 6, 0, 0]]} />
      <Sequence durationInFrames={PREGUNTA}>
        <Centro>
          <Palabras texto={texto} tam={112} cada={3} sale={PREGUNTA - 6} />
        </Centro>
      </Sequence>
      <Sequence from={respuestaEn} durationInFrames={dura - respuestaEn}>
        <Arriba texto={respuesta} dura={dura - respuestaEn} tam={64} cada={3} />
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
          <Overlay aparece={T.hice + 6} desde={0} ritmo={0.7} hasta={OVERLAY.mira} />
        </Camara>
        <VeloVivo
          claves={[
            [0, 0.8, 14],
            [T.hice, 0.8, 14],
            [T.hice + 12, 0.62, 5],
            [T.nombre - 4, 0.62, 5],
            [T.nombre + 6, 0.88, 18],
          ]}
        />
        <Sequence from={T.lento} durationInFrames={T.harto - T.lento}>
          <Centro>
            <Palabras texto={"The Snipping Tool takes\n*920 ms* to open"} tam={100} entra={3} cada={3} sale={T.harto - T.lento - 7} />
          </Centro>
        </Sequence>
        <Sequence from={T.harto} durationInFrames={T.hice - T.harto}>
          <Centro>
            <Palabras texto="I got tired of *waiting*" tam={100} cada={3} sale={T.hice - T.harto - 7} />
          </Centro>
        </Sequence>
        <Sequence from={T.hice} durationInFrames={T.nombre - T.hice}>
          <Centro>
            <Palabras texto="So I built *my own*" tam={100} cada={3} sale={T.nombre - T.hice - 7} />
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
          respuestaEn={PREGUNTA + s(3.1)}
          respuesta="*114 ms*, with three screens"
          app={
            <Camara
              tomas={[
                { f: 0, x: 960, y: 540, z: 1 },
                { f: PREGUNTA + s(1.2), x: 960, y: 540, z: 1 },
                { f: PREGUNTA + s(2.8), x: centroDelRecorte.x, y: centroDelRecorte.y, z: 1.52 },
                { f: PREGUNTA + s(4.4), x: centroDelRecorte.x, y: centroDelRecorte.y + 40, z: 1.65 },
                { f: T.graba - T.rapido, x: centroDelRecorte.x, y: centroDelRecorte.y + 40, z: 1.72 },
              ]}
            >
              <Overlay aparece={PREGUNTA + s(1.1)} desde={OVERLAY.vaAlBoton} ritmo={1.25} />
            </Camara>
          }
        />
        {/* El atajo que se enseña es el suyo, Win + Shift + S (Munito, 15-nov: «el atajo
            principal es Win Shift S»); winshotx se lo puede quitar a Windows en Ajustes. */}
        <Sequence from={PREGUNTA} durationInFrames={s(1.3)}>
          <VeloVivo claves={[[0, 0.7, 8], [s(1.0), 0.7, 8], [s(1.3), 0, 0]]} />
          <Centro>
            <Teclas teclas={["Win", "Shift", "S"]} entra={2} cada={4} tam={96} />
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
              tomas={[
                { f: 0, x: 960, y: 560, z: 0.92 },
                { f: PREGUNTA + s(1.0), x: 960, y: 600, z: 1.02 },
                { f: PREGUNTA + s(2.8), x: EDITOR.x + 1150, y: EDITOR.y + 330, z: 1.35 },
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
              fondo={PREGUNTA + s(1.0)}
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
                { f: PREGUNTA + s(0.8), x: 960, y: 560, z: 1.12 },
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
            <Palabras texto="Is it heavy?" tam={112} cada={3} sale={PREGUNTA - 6} />
          </Centro>
        </Sequence>
        <Sequence from={PREGUNTA}>
          <Centro y={-50}>
            <Palabras texto="*3.00 MB* to install" tam={104} cada={3} />
          </Centro>
          <Centro y={80}>
            <Palabras texto="33 MB of memory while it runs" entra={s(0.7)} cada={3} tam={50} peso={500} color={COLOR.suave} />
          </Centro>
        </Sequence>
      </Sequence>

      {/* Lo que es, sobre la app, y el cierre. */}
      <Sequence from={T.libre} durationInFrames={T.cierre - T.libre}>
        <EnLosAjustes tomas={[{ f: 0, x: 960, y: 540, z: 0.9 }, { f: T.cierre - T.libre, x: 960, y: 540, z: 0.84 }]} img="ajustes-capturar.png" />
        <VeloVivo claves={[[0, 0.86, 16]]} />
        <Centro>
          <Palabras texto={"Free, open source,\n*no account*"} tam={100} cada={3} sale={T.cierre - T.libre - 7} />
        </Centro>
      </Sequence>
      <Sequence from={T.cierre}>
        <Cierre linea="For Windows 10 and 11" />
      </Sequence>
      {/* Un fundido a negro en el último segundo, para que el bucle de Reddit no salte en seco. */}
      <AbsoluteFill style={{ background: "#000", opacity: interpolate(frame, [T.fin - s(0.8), T.fin - 1], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }) }} />
    </AbsoluteFill>
  );
}
