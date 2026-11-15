import { Html5Audio, Sequence, staticFile } from "remotion";

/**
 * Los efectos, PROVISIONALES: cinco clics de «Interface Sounds» de Kenney (CC0, licencia en
 * `public/sonido/kenney/License.txt`). Munito dijo de estos que suenan a plantilla («los
 * sonidos son muy genéricos que pones tú siempre», 7-nov) y la música la elige él de
 * `MarcaPersonal\identidad\sonidos\` (hoy vacía): están solo para que la prueba no vaya muda.
 */
const kenney = (n: string) => staticFile(`sonido/kenney/${n}.ogg`);
export const SONIDO = {
  tecla: ["click_002", "click_003", "click_004"].map(kenney),
  congela: kenney("drop_002"),
  frase: kenney("tick_002"),
} as const;

export type Efecto = { en: number; src: string; nivel?: number };

/** Cada efecto en su fotograma, uno detrás de otro. */
export function Efectos({ lista }: { lista: Efecto[] }) {
  return (
    <>
      {lista.map((e, i) => (
        <Sequence key={i} from={e.en} durationInFrames={45} layout="none">
          <Html5Audio src={e.src} volume={e.nivel ?? 0.5} />
        </Sequence>
      ))}
    </>
  );
}

/** Las teclas de un atajo, una cada `cada` fotogramas desde `en`. */
export const teclasEn = (en: number, cuantas: number, cada = 5): Efecto[] =>
  Array.from({ length: cuantas }, (_, i) => ({ en: en + i * cada, src: SONIDO.tecla[i % 3], nivel: 0.45 }));
