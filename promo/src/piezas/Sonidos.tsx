import { Html5Audio, Sequence, staticFile } from "remotion";
import mixkit from "../../public/sonido/mixkit/efectos.json";
import { FPS } from "../casa";

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

export type Efecto = { en: number; src: string; nivel?: number; dura?: number };

/** Cada efecto en su fotograma, uno detrás de otro. */
export function Efectos({ lista }: { lista: Efecto[] }) {
  return (
    <>
      {lista.map((e, i) => (
        <Sequence key={i} from={e.en} durationInFrames={e.dura ?? 45} layout="none">
          <Html5Audio src={e.src} volume={e.nivel ?? 0.5} />
        </Sequence>
      ))}
    </>
  );
}

/** Las teclas de un atajo, una cada `cada` fotogramas desde `en`. */
export const teclasEn = (en: number, cuantas: number, cada = 5): Efecto[] =>
  Array.from({ length: cuantas }, (_, i) => ({ en: en + i * cada, src: SONIDO.tecla[i % 3], nivel: 0.45 }));

/**
 * Los efectos del vídeo de la 0.5, de Mixkit (`node sonidos.mjs` los baja y los prepara; la
 * licencia y el porqué de cada uno, allí). Munito, 15-nov a las 22:30: «tienes que poner efectos
 * de sonido y hacerlo mejor»: discretos, cada uno en su fotograma y por debajo de la música.
 */
const MIXKIT = mixkit as Record<string, { ms: number; picoMs: number }>;
export type NombreEfecto = keyof typeof mixkit;

/**
 * Un efecto en el fotograma `en`. Con `pico`, lo que cae en `en` es su pico y no su arranque:
 * así el whoosh de un corte suena más fuerte justo en el corte, no después.
 */
export function efecto(nombre: NombreEfecto, en: number, nivel: number, pico = false): Efecto {
  const m = MIXKIT[nombre];
  return {
    en: Math.max(0, pico ? en - Math.round((m.picoMs / 1000) * FPS) : en),
    src: staticFile(`sonido/mixkit/${nombre}.wav`),
    nivel,
    dura: Math.ceil((m.ms / 1000) * FPS) + 2,
  };
}

/**
 * Los tics de una cifra que cuenta desde `en` durante `cuenta` fotogramas con la curva de
 * `Palabras` (sale deprisa y frena): uno cada vez que la cifra pasa una quinta parte, así que
 * se juntan al principio y se separan al llegar, y el último cae cuando la cifra se para. Dos
 * tics distintos, alternos, para que no suene la «ametralladora» del mismo sonido.
 */
export function ticsDeCuenta(en: number, cuenta: number, nivel: number): Efecto[] {
  const pasos = 5;
  return Array.from({ length: pasos }, (_, i) => {
    const k = (i + 1) / pasos;
    const t = 1 - Math.cbrt(1 - k);
    const ultimo = i === pasos - 1;
    return efecto(i % 2 ? "tic-2" : "tic-1", en + Math.max(1, Math.round(t * cuenta)), ultimo ? nivel * 1.3 : nivel);
  });
}
