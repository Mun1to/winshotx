import { AbsoluteFill, Img, Sequence, interpolate, useCurrentFrame, Easing } from "remotion";
import { MEDIDAS, OVERLAY, foto } from "../app";
import { ALTO, ANCHO, COLOR, s } from "../casa";
import { Camara, camaraEn, type Toma } from "../piezas/Camara";
import { AJUSTES, Arriba, Centro, Cierre, EDITOR, RECORTE, Velo } from "../piezas/Escenas";
import { Fondo } from "../piezas/Fondo";
import { Overlay } from "../piezas/Overlay";
import { Palabras } from "../piezas/Palabras";
import { Efectos, SONIDO, teclasEn } from "../piezas/Sonidos";
import { Teclas } from "../piezas/Teclas";

/**
 * Dirección 2, «Una tirada»: el camino entero de una captura, de la tecla al GIF, en una sola
 * toma y sin un corte. La cámara sigue al ratón por el overlay de verdad, entra por el recorte
 * y sale ya dentro del editor (el editor nace del hueco del recorte y crece), se pasea por él,
 * se aleja y aparecen los ajustes. El producto actúa desde el segundo cero; el texto, corto,
 * va diciendo qué acaba de pasar.
 */
export const D2 = {
  atajo: 0,
  overlay: s(1.6),
  /** El editor empieza a salir del recorte. */
  paso: s(8),
  editor: s(9.6),
  gif: s(15),
  ultimos: s(20),
  cifras: s(26),
  cierre: s(31),
  fin: s(38),
};

const RITMO = 1;
/** El fotograma del vídeo en que el recorrido del overlay llega a cada momento. */
const enVideo = (n: number) => D2.overlay + Math.round(n / RITMO);

/** La cámara por el overlay: sigue al ratón y se acerca al botón al elegir. */
const TOMAS_OVERLAY: Toma[] = [
  { f: 0, x: 960, y: 540, z: 1 },
  { f: enVideo(OVERLAY.vaAlBoton), x: 960, y: 520, z: 1.05 },
  { f: enVideo(OVERLAY.mira), x: MEDIDAS.boton.x + 60, y: MEDIDAS.boton.y + 10, z: 1.55 },
  { f: enVideo(OVERLAY.suelta), x: RECORTE.x + RECORTE.w / 2, y: RECORTE.y + RECORTE.h / 2 + 10, z: 1.8 },
  { f: enVideo(OVERLAY.barra), x: RECORTE.x + RECORTE.w / 2, y: RECORTE.y + RECORTE.h / 2 + 40, z: 1.75 },
  // Entra por el recorte: la cámara se mete hasta que el recorte llena el cuadro.
  { f: D2.editor, x: RECORTE.x + RECORTE.w / 2, y: RECORTE.y + RECORTE.h / 2, z: ANCHO / RECORTE.w },
];

/** El editor: se ve el recorte del Estudio, se pone el fondo, luego al panel de la derecha. */
const EN_EDITOR = (f0: number): Toma[] => [
  { f: f0, x: EDITOR.x + 560, y: EDITOR.y + 390, z: 1.3 },
  { f: f0 + s(2.4), x: EDITOR.x + 560, y: EDITOR.y + 380, z: 1.15 },
  { f: D2.gif, x: EDITOR.x + 900, y: EDITOR.y + 400, z: 1.1 },
  { f: D2.gif + s(3), x: EDITOR.x + 1260, y: EDITOR.y + 300, z: 1.7 },
  { f: D2.ultimos, x: 960, y: 540, z: 0.62 },
  { f: D2.ultimos + s(3.5), x: 1440, y: 560, z: 0.95 },
  { f: D2.cifras, x: 1200, y: 540, z: 0.62 },
];

export function DireccionTirada() {
  const frame = useCurrentFrame();
  // El paso de máscara: el editor entero cabe primero DENTRO del recorte, en pequeño, y crece
  // a la vez que la cámara entra en él; el overlay se va por los bordes.
  const crece = interpolate(frame, [D2.paso, D2.editor], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.bezier(0.65, 0, 0.35, 1) });
  const conFondo = interpolate(frame, [D2.editor + s(1.6), D2.editor + s(1.8)], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const conGif = interpolate(frame, [D2.gif + s(0.9), D2.gif + s(1.05)], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const ajustes = interpolate(frame, [D2.ultimos + s(0.6), D2.ultimos + s(1.4)], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.out(Easing.cubic) });
  // Dónde está el recorte en pantalla mientras la cámara entra en él: ahí nace el editor.
  const camO = camaraEn(frame, TOMAS_OVERLAY);
  const hueco = {
    x: ANCHO / 2 + (RECORTE.x - camO.x) * camO.z,
    y: ALTO / 2 + (RECORTE.y - camO.y) * camO.z,
    w: RECORTE.w * camO.z,
    h: RECORTE.h * camO.z,
  };
  // El editor entero cabe en el hueco al principio y llena el cuadro al final.
  const escala = interpolate(crece, [0, 1], [Math.min(hueco.w / ANCHO, hueco.h / ALTO), 1]);
  const caja = {
    x: interpolate(crece, [0, 1], [hueco.x + (hueco.w - ANCHO * escala) / 2, 0]),
    y: interpolate(crece, [0, 1], [hueco.y + (hueco.h - ALTO * escala) / 2, 0]),
  };
  const editorCaja = { position: "absolute" as const, left: EDITOR.x, top: EDITOR.y, width: EDITOR.w, height: EDITOR.h };
  const enEditor = frame >= D2.editor;
  return (
    <AbsoluteFill style={{ background: COLOR.fondo }}>
      {/* El overlay, con la cámara siguiendo al ratón. */}
      {!enEditor && (
        <AbsoluteFill style={{ opacity: interpolate(crece, [0, 1], [1, 0.25]) }}>
          <Camara tomas={TOMAS_OVERLAY}>
            <Overlay aparece={D2.overlay} desde={0} ritmo={RITMO} />
          </Camara>
        </AbsoluteFill>
      )}
      {/* El editor, que nace del hueco del recorte y luego es el escenario. */}
      {frame >= D2.paso && (
        <AbsoluteFill
          style={
            enEditor
              ? undefined
              : {
                  transformOrigin: "0 0",
                  transform: `translate(${caja.x}px, ${caja.y}px) scale(${escala})`,
                  opacity: Math.min(1, crece * 5),
                  borderRadius: 14 / Math.max(escala, 0.05),
                  overflow: "hidden",
                }
          }
        >
          <Fondo luz={0.6} />
          <Camara tomas={EN_EDITOR(D2.editor)}>
            <div style={{ ...editorCaja, borderRadius: 10, overflow: "hidden", boxShadow: "0 30px 80px -20px #000d, 0 0 0 1px #ffffff14" }}>
              <Img src={foto("editor-sin-fondo.png")} style={{ ...editorCaja, left: 0, top: 0 }} />
              <Img src={foto("editor-atardecer.png")} style={{ ...editorCaja, left: 0, top: 0, opacity: conFondo }} />
              <Img src={foto("editor-gif.png")} style={{ ...editorCaja, left: 0, top: 0, opacity: conGif }} />
            </div>
            <div
              style={{
                position: "absolute",
                left: EDITOR.x + EDITOR.w + 120 - (1 - ajustes) * 80,
                top: AJUSTES.y,
                width: AJUSTES.w,
                height: AJUSTES.h,
                opacity: ajustes,
                borderRadius: 10,
                overflow: "hidden",
                boxShadow: "0 30px 80px -20px #000d, 0 0 0 1px #ffffff14",
              }}
            >
              <Img src={foto("ajustes-ultimos.png")} style={{ width: AJUSTES.w, height: AJUSTES.h }} />
            </div>
          </Camara>
        </AbsoluteFill>
      )}
      {/* Lo que se lee, corto, diciendo lo que acaba de pasar. */}
      <Sequence durationInFrames={D2.overlay + 6}>
        <Velo fuerza={0.7} desenfoque={8} />
        <Centro>
          <Teclas teclas={["Ctrl", "Shift", "2"]} entra={8} tam={96} />
        </Centro>
      </Sequence>
      <Sequence from={D2.overlay + 4} durationInFrames={s(2.4)}>
        <Arriba texto="The screen *freezes*" tam={64} dura={s(2.4)} />
      </Sequence>
      <Sequence from={enVideo(OVERLAY.mira) - 4} durationInFrames={s(2.6)}>
        <Arriba texto="Pick it to the *pixel*" tam={64} dura={s(2.6)} />
      </Sequence>
      <Sequence from={enVideo(OVERLAY.barra) - 10} durationInFrames={D2.paso - enVideo(OVERLAY.barra) + 14}>
        <Arriba texto="*114 ms* from key to selection" tam={64} cada={3} dura={D2.paso - enVideo(OVERLAY.barra) + 14} />
      </Sequence>
      <Sequence from={D2.editor + 6} durationInFrames={s(4.4)}>
        <Arriba texto="Studio look in *one click*" tam={60} dura={s(4.4)} />
      </Sequence>
      <Sequence from={D2.gif + 4} durationInFrames={s(4.6)}>
        <Arriba texto="*GIF* or MP4, ready to paste" tam={60} dura={s(4.6)} />
      </Sequence>
      <Sequence from={D2.ultimos + 10} durationInFrames={s(5.4)}>
        <Arriba texto="Missed it? Keep the *last 30 s*" tam={60} dura={s(5.4)} />
      </Sequence>
      <Sequence from={D2.cifras} durationInFrames={D2.cierre - D2.cifras}>
        <Velo fuerza={0.8} />
        <Centro y={-50}>
          <Palabras texto="*33 MB* while it runs" tam={92} />
        </Centro>
        <Centro y={70}>
          <Palabras texto="*3.00 MB* to install" entra={s(1.2)} tam={92} />
        </Centro>
      </Sequence>
      <Sequence from={D2.cierre}>
        <Cierre />
      </Sequence>
      <Efectos
        lista={[
          ...teclasEn(8, 3),
          { en: D2.overlay, src: SONIDO.congela, nivel: 0.6 },
          { en: enVideo(OVERLAY.arrastra), src: SONIDO.tecla[0], nivel: 0.4 },
          { en: enVideo(OVERLAY.suelta), src: SONIDO.tecla[1], nivel: 0.4 },
          { en: D2.paso, src: SONIDO.frase, nivel: 0.4 },
          { en: D2.editor + s(1.6), src: SONIDO.tecla[2], nivel: 0.4 },
        ]}
      />
    </AbsoluteFill>
  );
}
