import { AbsoluteFill, Sequence, interpolate, useCurrentFrame } from "remotion";
import { OVERLAY } from "../app";
import { COLOR, s } from "../casa";
import { Camara } from "../piezas/Camara";
import { Carrera } from "../piezas/Carrera";
import { Arriba, Centro, Cierre, EnElEditor, EnLosAjustes, EDITOR, AJUSTES, Velo, centroDelRecorte } from "../piezas/Escenas";
import { Fondo } from "../piezas/Fondo";
import { Logo } from "../piezas/Logo";
import { Overlay } from "../piezas/Overlay";
import { Palabras } from "../piezas/Palabras";
import { Efectos, SONIDO, teclasEn } from "../piezas/Sonidos";
import { Teclas } from "../piezas/Teclas";

/**
 * Dirección 1, «La carrera»: primero la prueba. El atajo, la carrera de los dos tiempos
 * medidos (a cámara lenta, cinco veces) con el overlay de verdad apareciendo cuando llega el
 * de winshotx, la memoria, el nombre, y después cuatro cosas que hace, cada una con su
 * resultado. Las cifras son las de la web: 114 ms y 920 ms, 33 MB y 253 MB, 3,01 MB.
 */
export const D1 = {
  atajo: 0,
  carrera: s(1.4),
  memoria: s(6.4),
  nombre: s(9.6),
  captura: s(11.6),
  estudio: s(15.6),
  gif: s(19.6),
  ultimos: s(23.6),
  peso: s(28.6),
  cierre: s(33),
  fin: s(40),
};

/** La carrera, frenada cinco veces: 920 ms tardan 4,6 s en llegar y 114 ms, 0,57 s. */
const LENTO = 5;
const CUENTA = s((0.92 * LENTO));
const LLEGA_WINSHOTX = Math.round((CUENTA * 114) / 920);

export function DireccionCarrera() {
  const frame = useCurrentFrame();
  const fueraDelEscritorio = interpolate(frame, [D1.memoria - 6, D1.memoria + 6], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <AbsoluteFill style={{ background: COLOR.fondo }}>
      {/* El escritorio y, al llegar los 114 ms, el overlay de verdad encima. */}
      <Sequence durationInFrames={D1.memoria + 8}>
        <AbsoluteFill style={{ opacity: fueraDelEscritorio }}>
          <Overlay aparece={D1.carrera + LLEGA_WINSHOTX} desde={0} ritmo={0.3} hasta={OVERLAY.vaAlBoton} />
          <Velo fuerza={0.8} entra={0} desenfoque={10} />
        </AbsoluteFill>
      </Sequence>
      <Sequence durationInFrames={D1.carrera + 4}>
        <Centro y={-40}>
          <Palabras texto="Press the shortcut" tam={58} peso={600} color={COLOR.suave} />
        </Centro>
        <Centro y={60}>
          <Teclas teclas={["Ctrl", "Shift", "2"]} entra={14} tam={84} />
        </Centro>
      </Sequence>
      <Sequence from={D1.carrera} durationInFrames={D1.memoria - D1.carrera}>
        <Centro y={-330}>
          <Palabras texto="Slowed down 5 times" tam={36} peso={500} color={COLOR.suave} />
        </Centro>
        <Centro y={10}>
          <Carrera unidad="ms" cuenta={CUENTA} filas={[{ nombre: "winshotx", valor: 114, nuestra: true }, { nombre: "Snipping Tool", valor: 920 }]} />
        </Centro>
        <Centro y={330}>
          <Palabras texto="From the key to the selection on screen" entra={CUENTA - 10} cada={2} tam={34} peso={500} color={COLOR.suave} />
        </Centro>
      </Sequence>
      <Sequence from={D1.memoria} durationInFrames={D1.nombre - D1.memoria}>
        <Fondo />
        <Centro y={-250}>
          <Palabras texto="Memory while capturing" tam={44} peso={600} color={COLOR.suave} />
        </Centro>
        <Centro y={30}>
          <Carrera unidad="MB" cuenta={s(1.4)} entra={6} filas={[{ nombre: "winshotx", valor: 33, nuestra: true }, { nombre: "Snipping Tool", valor: 253 }]} />
        </Centro>
      </Sequence>
      <Sequence from={D1.nombre} durationInFrames={D1.captura - D1.nombre}>
        <Fondo />
        <Centro y={-30}>
          <Logo tam={150} />
        </Centro>
        <Centro y={130}>
          <Palabras texto="A free capture app for *Windows*" entra={16} cada={3} tam={44} peso={500} color={COLOR.suave} />
        </Centro>
      </Sequence>
      <Sequence from={D1.captura} durationInFrames={D1.estudio - D1.captura}>
        <Camara tomas={[{ f: 0, x: 960, y: 540, z: 1 }, { f: s(1.4), x: centroDelRecorte.x, y: centroDelRecorte.y, z: 1.7 }, { f: s(4), x: centroDelRecorte.x, y: centroDelRecorte.y + 40, z: 1.6 }]}>
          <Overlay aparece={0} desde={OVERLAY.vaAlBoton} ritmo={1.25} />
        </Camara>
        <Arriba texto="Frozen screen, *pixel* exact" tam={60} entra={4} dura={s(3.6) + 6} />
      </Sequence>
      <Sequence from={D1.estudio} durationInFrames={D1.gif - D1.estudio}>
        <EnElEditor fondo={s(1.2)} tomas={[{ f: 0, x: EDITOR.x + 560, y: EDITOR.y + 390, z: 1.05 }, { f: s(3.6), x: EDITOR.x + 560, y: EDITOR.y + 380, z: 1.25 }]} />
        <Arriba texto="Studio look in *one click*" tam={60} entra={4} dura={s(3.6) + 6} />
      </Sequence>
      <Sequence from={D1.gif} durationInFrames={D1.ultimos - D1.gif}>
        <EnElEditor fondo={-10} gif={s(0.8)} tomas={[{ f: 0, x: EDITOR.x + 900, y: EDITOR.y + 420, z: 1.15 }, { f: s(3.4), x: EDITOR.x + 1260, y: EDITOR.y + 300, z: 1.7 }]} />
        <Arriba texto="Save it as a *GIF*" tam={60} entra={4} dura={s(3.6) + 6} />
      </Sequence>
      <Sequence from={D1.ultimos} durationInFrames={D1.peso - D1.ultimos}>
        <EnLosAjustes tomas={[{ f: 0, x: 960, y: 540, z: 1.1 }, { f: s(4), x: AJUSTES.x + 290, y: AJUSTES.y + 220, z: 1.7 }]} />
        <Arriba texto="Missed it? Keep the *last 30 s*" tam={60} entra={4} dura={s(4.6) + 6} />
      </Sequence>
      <Sequence from={D1.peso} durationInFrames={D1.cierre - D1.peso}>
        <Fondo />
        <Centro y={-40}>
          <Palabras texto="*3.01 MB* installer" tam={110} />
        </Centro>
        <Centro y={90}>
          <Palabras texto="No account, no cloud, no telemetry" entra={14} cada={3} tam={46} peso={500} color={COLOR.suave} />
        </Centro>
      </Sequence>
      <Sequence from={D1.cierre}>
        <Cierre />
      </Sequence>
      <Efectos
        lista={[
          ...teclasEn(14, 3),
          { en: D1.carrera + LLEGA_WINSHOTX, src: SONIDO.congela, nivel: 0.6 },
          { en: D1.carrera + CUENTA, src: SONIDO.frase, nivel: 0.4 },
          { en: D1.memoria + 6, src: SONIDO.frase, nivel: 0.35 },
          { en: D1.nombre, src: SONIDO.congela, nivel: 0.5 },
        ]}
      />
    </AbsoluteFill>
  );
}
