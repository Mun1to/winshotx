import type { ReactNode } from "react";
import { AbsoluteFill, Easing, Sequence, interpolate, useCurrentFrame } from "remotion";
import cifras from "../cifras.json";
import { MEDIDAS, OVERLAY } from "./app";
import { COLOR, FPS, s } from "./casa";
import { fotosDeLinux } from "./linux";
import { Camara, type Toma } from "./piezas/Camara";
import { Cursor } from "./piezas/Cursor";
import { AJUSTES, Arriba, Centro, Cierre, EDITOR, EnElEditor, EnLinux, EnLosAjustes, cajaDeLinux, centroDelRecorte, RECORTE } from "./piezas/Escenas";
import { Logo } from "./piezas/Logo";
import { Overlay } from "./piezas/Overlay";
import { Palabras } from "./piezas/Palabras";
import { type Efecto, Efectos, efecto, ticsDeCuenta } from "./piezas/Sonidos";
import { Teclas } from "./piezas/Teclas";

/**
 * El vídeo de la 0.5: la dirección 3, «Lo hice yo», que eligió Munito (15-nov), con la
 * historia ENCIMA de la app moviéndose. Sin efectos de sonido al principio («no me gustan», de unos
 * clics genéricos); con efectos buenos y discretos desde las 22:30 (ver la cuarta vuelta).
 *
 * Segunda vuelta (15-nov, 19:05, «hazlo más rápido»): de 69 a 40,6 s (luego 40,5) con las mismas cinco
 * preguntas. Una idea nueva cada 2,2 s (sus tres preferidas de la carpeta de vídeos SaaS van
 * de 22 a 29 ideas por minuto; este, 28), cada pregunta sola 0,9 s en vez de 1,5, y ningún
 * texto menos tiempo del que pide leerlo (caracteres entre 17, la regla de la guía). Y el
 * atajo que se enseña es Win + Shift + S, el que él usa y el que winshotx puede quitarle a
 * Windows.
 *
 * La música la elige él entre las candidatas y se le pone después con ffmpeg (`musica.mjs`):
 * baja durante la historia y entra entera con el logo (`--sube`).
 */
/*
 * Tercera vuelta (15-nov, 21:28, «el zoom y grabar… es muy brusco»): rápido ENTRE ideas, suave
 * DENTRO de cada movimiento. La cámara pasa por sus puntos sin frenar en cada uno (`Camara`),
 * cada escena se va desenfocándose y la siguiente llega desenfocada (el corte cae entre dos
 * fotogramas borrosos, nunca entre dos nítidos de distinta escala), y el overlay se reproduce a
 * su ritmo entero (a 1,25 saltaba un fotograma de cada cuatro y el ratón daba tirones).
 *
 * Todos los cortes caen en un múltiplo de medio segundo, 15 fotogramas: a 120 golpes por minuto
 * cada corte cae en un golpe, si la pista empieza en su golpe (`node musica.mjs --desde`).
 */
/*
 * Cuarta vuelta (15-nov, 22:30): «efectos de sonido SÍ» y la sexta pregunta. Los cortes se
 * cuentan en golpes de la música y no en segundos, para sacar el mismo vídeo a compás de una
 * pista de otro tempo (Karma va a 127,5: `Promo-127`). A 120, un golpe es medio segundo.
 * Los efectos van en su propia pista, debajo de la música (`render.mjs` la saca aparte y
 * `musica.mjs` las mezcla), y la sexta pregunta, Win + Shift + S en Ajustes, va justo después
 * de la del atajo, que es la que la pide.
 */
/*
 * La 0.6.0 (20-nov): lo grande es que winshotx ya funciona en Linux (X11 y GNOME con Wayland), y
 * entra como séptima pregunta, «On Linux?», justo después de las dos del atajo: Win + Shift + S es
 * de Windows, y la siguiente pregunta es la que se haría alguien de Linux al verlo. Las seis de la
 * 0.5 se quedan (las eligió él) y el vídeo no pasa de unos 45 s (45,5): la de Linux dura 4 s, como
 * las cortas, y para hacerle sitio la primera pierde medio segundo (de 7 a 6,5 s, sin tocar el
 * ratón del overlay), y la de grabar, la del Estudio, la de los últimos segundos y el cierre, entre
 * medio y un segundo cada una. Los cortes siguen cayendo en golpes de la música, y el logo, en el
 * mismo golpe de Be Happy (`musica.mjs --desde` corre lo mismo que se corre el logo: 1 s antes).
 * El cierre dice «For Windows and Linux», y el peso del instalador sale de `cifras.json`.
 */
/*
 * Para Reddit (23-nov): «¿no crees que puedes mostrar mejor winshotx?». Lo que no vendía: capturaba y grababa su propia
 * web (los botones de descarga), la app salía pequeña para un móvil, y la velocidad no se veía al lado de la otra. Sin
 * tocar lo que eligió él (Be Happy, las siete preguntas, los cortes en golpes, 45,5 s): la pantalla que se captura es
 * una gráfica de un artículo (`capturas.mjs --fondo=luz`), la cámara llega más cerca a lo que se nombra en cada
 * pregunta, y las dos cifras dicen también las de la Herramienta de Recortes (920 ms y 253 MB, medidas en su PC).
 */
const GOLPES = {
  lento: 0,
  harto: 5,
  hice: 8,
  nombre: 11,
  rapido: 15,
  atajo: 28,
  linux: 37,
  graba: 45,
  estudio: 54,
  ultimos: 62,
  peso: 70,
  libre: 79,
  cierre: 84,
  fin: 91,
};
export type Tiempos = Record<keyof typeof GOLPES, number>;
/** Los cortes en fotogramas para una música de `bpm` golpes por minuto. */
export const tiempos = (bpm: number): Tiempos =>
  Object.fromEntries(Object.entries(GOLPES).map(([k, g]) => [k, Math.round((g * 60 * FPS) / bpm)])) as Tiempos;
/**
 * `whoosh: false` deja solo los efectos de lo que se pulsa (teclas, clics, obturador, interruptor,
 * tics): es como suenan los seis vídeos de lanzamiento de fuera medidos el 15-nov (en 41 cortes,
 * solo 2 llevan un golpe; los clics, sí, en su fotograma). Con `true`, además un whoosh en cada
 * corte, en el zoom y con el logo, que es lo que pide su gusto de «cada transición con su sonido».
 */
export type PropsPromo = { bpm: number; whoosh?: boolean };

/** Lo que dura sola cada pregunta, grande y en el centro, antes de que la conteste la app. */
const PREGUNTA = s(0.9);
/** Lo que tarda una escena en desenfocarse antes del corte (y la siguiente en enfocarse). */
const PASO = 9;

/**
 * El velo de detrás de un texto, que cambia con el tiempo: cada clave es [fotograma, cuánto
 * oscurece, cuánto desenfoca]. Así la app se va viendo más nítida a medida que la historia
 * llega a ella. Entre clave y clave, con entrada y salida suaves.
 */
function VeloVivo({ claves }: { claves: [number, number, number][] }) {
  const frame = useCurrentFrame();
  const fs = claves.map((c) => c[0]);
  const o = { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.inOut(Easing.cubic) } as const;
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
function Pregunta({ texto, respuesta, dura, app, respuestaEn = PREGUNTA + 4, cuenta, sola = PREGUNTA, tam = 112 }: { texto: string; respuesta: string; dura: number; app: ReactNode; respuestaEn?: number; cuenta?: number; sola?: number; tam?: number }) {
  return (
    <AbsoluteFill>
      {app}
      {/* Llega desenfocada, se enfoca al contestar, y se vuelve a desenfocar antes del corte. */}
      <VeloVivo claves={[[0, 0.85, 16], [sola - 5, 0.85, 16], [sola + PASO, 0, 0], [dura - PASO, 0, 0], [dura, 0.85, 16]]} />
      <Sequence durationInFrames={sola}>
        <Centro>
          <Palabras texto={texto} tam={tam} cada={3} sale={sola - 6} />
        </Centro>
      </Sequence>
      <Sequence from={respuestaEn} durationInFrames={dura - respuestaEn}>
        <Arriba texto={respuesta} dura={dura - respuestaEn} tam={64} cada={3} cuenta={cuenta} />
      </Sequence>
    </AbsoluteFill>
  );
}

/** Lo de dentro se va desenfocándose en los últimos `PASO` fotogramas de su escena. */
function SeVa({ dura, children }: { dura: number; children: ReactNode }) {
  const frame = useCurrentFrame();
  const v = interpolate(frame, [dura - PASO, dura], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.in(Easing.cubic) });
  return <AbsoluteFill style={{ opacity: v, filter: v < 0.99 ? `blur(${(1 - v) * 14}px)` : undefined }}>{children}</AbsoluteFill>;
}

/**
 * El puntero de Windows yendo de `de` a `a` (en el escenario) entre `sale` y `llega`, con
 * entrada y salida suaves, y hundiéndose un poco al hacer clic en `clic`.
 */
function Raton({ de, a, sale, llega, clic }: { de: { x: number; y: number }; a: { x: number; y: number }; sale: number; llega: number; clic: number }) {
  const frame = useCurrentFrame();
  const o = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;
  const t = interpolate(frame, [sale, llega], [0, 1], { ...o, easing: Easing.inOut(Easing.cubic) });
  const x = de.x + (a.x - de.x) * t;
  const y = de.y + (a.y - de.y) * t;
  const pulsa = frame >= clic && frame < clic + 6 ? 1 - 0.12 * Math.sin(((frame - clic) / 6) * Math.PI) : 1;
  return (
    <AbsoluteFill style={{ opacity: interpolate(frame, [sale - 6, sale], [0, 1], o), transform: `scale(${pulsa})`, transformOrigin: `${x}px ${y}px` }}>
      <Cursor x={x} y={y} forma="flecha" />
    </AbsoluteFill>
  );
}

/**
 * El overlay de la primera pregunta: cuándo aparece y por qué fotograma de su recorrido entra. En
 * la 0.6.0 la escena dura medio segundo menos: aparece 3 fotogramas antes y entra 3 más adelante
 * en su camino al botón (a su ritmo entero, como antes), y así la barra de debajo del recorte se
 * sigue viendo entera antes del corte.
 */
const Q1 = { aparece: PREGUNTA + s(1.0), desde: OVERLAY.vaAlBoton + 13 };

/**
 * Cuánto se acerca la cámara al recorte de la primera pregunta. Con la gráfica (`capturas.mjs --fondo=luz`, Reddit,
 * 23-nov) el recorte mide 896 x 499 y no 440 x 94 como los botones de la guía: a 1,72 se salía del cuadro. Lo más
 * cerca que cabe con la barra de debajo y la respuesta de arriba: la cámara mira 15 px por ENCIMA del centro, para
 * que el borde de arriba del recorte quede debajo de la respuesta.
 */
const ZOOM_RECORTE = RECORTE.w > 600 ? { cerca: 1.3, dentro: 1.42, fin: 1.48, baja: -15 } : { cerca: 1.52, dentro: 1.65, fin: 1.72, baja: 40 };

/** La sexta pregunta: lo que dura sola (lleva más letra que las otras) y cuándo se hace clic. */
const ATAJO = { sola: s(1.4), llega: s(1.4) + 20, clic: s(1.4) + 22 };
/**
 * La cámara de la pregunta de Linux acaba con este zoom, y mirando a la altura que deja el borde de
 * arriba de la pantalla en el mismo sitio que al empezar (y = 540 + (borde − y) · z = borde).
 */
const ZOOM_LINUX = 1.45;
/**
 * Dónde mira la cámara para que el borde de arriba de la ventana de winshotx (en las dos fotos, unos 175 px por debajo
 * del borde de la pantalla de 1080) quede a 200 px del borde del cuadro, debajo de la respuesta. A 1,45 la foto sale a
 * 1,09 de su tamaño: todavía nítida, y la ventana ya se lee (a 1,08 ocupaba un tercio del ancho).
 */
const ARRIBA_LINUX = (() => {
  const c = cajaDeLinux(1920, 1080);
  return c.y + (175 * c.h) / 1080 + (540 - 200) / ZOOM_LINUX;
})();
/** El interruptor de «Take Win+Shift+S», apagado, en el escenario (su centro). */
const INTERRUPTOR = (() => {
  const r = MEDIDAS.teclas.no.interruptor;
  return { x: AJUSTES.x + r.x + r.w / 2, y: AJUSTES.y + r.y + r.h / 2 };
})();

export function Promo({ bpm, whoosh: conWhoosh = true }: PropsPromo) {
  const frame = useCurrentFrame();
  const T = tiempos(bpm);
  // Cada corte, un whoosh con su pico justo en el corte, y nunca el mismo dos veces seguidas.
  const cortes = [T.rapido, T.atajo, T.linux, T.graba, T.estudio, T.ultimos, T.peso, T.libre];
  const linux = fotosDeLinux();
  const whoosh = ["whoosh-1", "whoosh-2", "whoosh-3", "whoosh-4"] as const;
  // Los niveles, medidos con el mismo script que los vídeos de fuera (`musica.mjs` sube la pista
  // entera 4 dB): las teclas, los clics, el obturador y el interruptor quedan a la altura del
  // nivel normal de la música, como los clics de Raycast y Linear; los tics y los whoosh, un poco
  // por debajo.
  const aire: Efecto[] = conWhoosh
    ? [
        efecto("sube", T.nombre + 6, 0.22, true),
        ...cortes.map((f, i) => efecto(whoosh[i % whoosh.length], f, 0.25, true)),
        efecto("zoom", T.cierre, 0.25, true),
        efecto("zoom", T.rapido + PREGUNTA + s(2.0), 0.2, true),
      ]
    : [];
  const sonidos: Efecto[] = [
    ...aire,
    // La historia va con la música a −8 dB: sus efectos, también bajos. «920» es la quinta
    // palabra de su frase (entra en el 3, una cada 3).
    ...ticsDeCuenta(T.lento + 3 + 4 * 3, 15, 0.11),
    // 1. Ctrl, Shift y 2, cada una en el fotograma en que se hunde (`Teclas`: entra 2, cada 4);
    // el aire del zoom al recorte, con el pico donde la cámara va más deprisa; el clic al
    // apretar el ratón y el obturador al soltarlo, que es cuando la captura está hecha.
    ...(["tecla-1", "tecla-2", "tecla-3"] as const).map((n, i) => efecto(n, T.rapido + PREGUNTA + 2 + i * 4, 0.43)),
    efecto("clic", T.rapido + Q1.aparece + OVERLAY.arrastra - Q1.desde, 0.86),
    efecto("obturador", T.rapido + Q1.aparece + OVERLAY.suelta - Q1.desde, 0.6),
    ...ticsDeCuenta(T.rapido + PREGUNTA + s(3.1), 15, 0.2),
    // 6. El clic en el interruptor.
    efecto("interruptor", T.atajo + ATAJO.clic, 0.32),
    // 3. Un clic y entra el fondo del Estudio.
    efecto("clic", T.estudio + PREGUNTA + s(1.0), 0.86),
    // 5. Las dos cifras; la de la memoria, más baja, que es la segunda línea.
    ...ticsDeCuenta(T.peso + PREGUNTA, 15, 0.2),
    ...ticsDeCuenta(T.peso + PREGUNTA + s(0.7), 15, 0.13),
  ];
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
          <Overlay aparece={T.hice + 6} desde={0} ritmo={0.5} hasta={OVERLAY.mira} />
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
            <Palabras texto={"The Snipping Tool takes\n*920 ms* to open"} tam={100} entra={3} cada={3} sale={T.harto - T.lento - 7} cuenta={15} />
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
          <SeVa dura={T.rapido - T.nombre}>
            <Centro>
              <Logo tam={200} entra={4} />
            </Centro>
          </SeVa>
        </Sequence>
      </Sequence>

      {/* 1. ¿Rápido? El atajo y el overlay de verdad, de la flecha a la barra. */}
      <Sequence from={T.rapido} durationInFrames={T.atajo - T.rapido}>
        <Pregunta
          texto="How fast?"
          dura={T.atajo - T.rapido}
          respuestaEn={PREGUNTA + s(3.1)}
          respuesta="*114 ms*. The Snipping Tool: 920 ms"
          cuenta={15}
          app={
            <Camara
              tomas={[
                { f: 0, x: 960, y: 540, z: 1 },
                { f: PREGUNTA + s(1.2), x: 960, y: 540, z: 1 },
                { f: PREGUNTA + s(2.8), x: centroDelRecorte.x, y: centroDelRecorte.y, z: ZOOM_RECORTE.cerca },
                { f: PREGUNTA + s(4.4), x: centroDelRecorte.x, y: centroDelRecorte.y + ZOOM_RECORTE.baja, z: ZOOM_RECORTE.dentro },
                { f: T.atajo - T.rapido, x: centroDelRecorte.x, y: centroDelRecorte.y + ZOOM_RECORTE.baja, z: ZOOM_RECORTE.fin },
              ]}
            >
              {/* A su ritmo entero: a 1,25 se saltaba un fotograma de cada cuatro y el ratón daba
                  tirones. Para que quepa, entra 10 fotogramas más adelante en su camino al botón. */}
              <Overlay aparece={Q1.aparece} desde={Q1.desde} ritmo={1} />
            </Camara>
          }
        />
        {/* El atajo propio de winshotx, Ctrl + Shift + 2 (el que trae de serie). Win + Shift + S
            se guarda para la sexta pregunta: Munito, 17-nov, «en vez de el win pon un shortcut de
            antes porque luego dices lo del win». */}
        <Sequence from={PREGUNTA} durationInFrames={s(1.3)}>
          <VeloVivo claves={[[0, 0.7, 8], [s(1.0), 0.7, 8], [s(1.3), 0, 0]]} />
          <Centro>
            <Teclas teclas={["Ctrl", "Shift", "2"]} entra={2} cada={4} tam={96} />
          </Centro>
        </Sequence>
      </Sequence>

      {/* 6. ¿Te va Win + Shift + S? Quédatelo: en Ajustes, Teclas de Windows, el interruptor se
          enciende y el dibujo de la derecha pasa a decir que abre winshotx. Va aquí, justo
          después de enseñar el atajo (Munito, 15-nov: «el atajo principal es Win Shift S»). */}
      <Sequence from={T.atajo} durationInFrames={T.graba - T.atajo}>
        <Pregunta
          texto="Used to *Win + Shift + S*?"
          tam={104}
          sola={ATAJO.sola}
          dura={T.graba - T.atajo}
          respuestaEn={ATAJO.clic + 6}
          respuesta="Keep it. It opens *winshotx*"
          app={
            <EnLosAjustes
              img="ajustes-teclas.png"
              luego={{ img: "ajustes-teclas-si.png", en: ATAJO.clic }}
              tomas={[
                { f: 0, x: 960, y: 540, z: 1 },
                { f: ATAJO.llega, x: 975, y: 485, z: 1.6 },
                { f: T.graba - T.atajo, x: 978, y: 480, z: 1.8 },
              ]}
            >
              <Raton de={{ x: INTERRUPTOR.x + 210, y: INTERRUPTOR.y + 260 }} a={{ x: INTERRUPTOR.x + 2, y: INTERRUPTOR.y + 2 }} sale={ATAJO.sola + 2} llega={ATAJO.llega} clic={ATAJO.clic} />
            </EnLosAjustes>
          }
        />
      </Sequence>

      {/* 7. ¿Y en Linux? La novedad de la 0.6.0: la app de verdad en GNOME con Wayland y, si hay
          foto, en X11 después. Lo que dice es lo que hay: GNOME con Wayland, no «Wayland» a secas
          (en KDE con Wayland no funciona todavía). */}
      <Sequence from={T.linux} durationInFrames={T.graba - T.linux}>
        <Pregunta
          texto="On Linux?"
          dura={T.graba - T.linux}
          respuesta="Yes. X11 and *GNOME on Wayland*"
          app={
            <EnLinux
              fotos={linux}
              dura={T.graba - T.linux}
              cambia={Math.round((PREGUNTA + PASO + (T.graba - T.linux - PASO)) / 2)}
              tomas={[
                { f: 0, x: 960, y: 540, z: 1 },
                // A la ventana de winshotx, que es lo que se nombra (Reddit, 23-nov: a 1,08 no se leía), y
                // ya cerca cuando sale la respuesta: la barra de GNOME se queda fuera, y lo que dice «Linux»
                // es la respuesta.
                { f: PREGUNTA + PASO, x: 960, y: ARRIBA_LINUX + 12, z: ZOOM_LINUX - 0.12 },
                { f: T.graba - T.linux, x: 960, y: ARRIBA_LINUX, z: ZOOM_LINUX },
              ]}
            />
          }
        />
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
                // Un solo viaje del centro a la exportación, repartido por toda la escena: antes
                // cruzaba 430 px en 1,8 s y en 17,3 s daba el golpe que vio Munito.
                { f: 0, x: 960, y: 560, z: 0.95 },
                { f: PREGUNTA + s(1.4), x: 1080, y: 540, z: 1.08 },
                { f: T.estudio - T.graba, x: EDITOR.x + 1150, y: EDITOR.y + 325, z: 1.38 },
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
                { f: 0, x: EDITOR.x + 580, y: EDITOR.y + 390, z: 1.15 },
                { f: T.ultimos - T.estudio, x: EDITOR.x + 570, y: EDITOR.y + 380, z: 1.45 },
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
                { f: PREGUNTA + s(0.8), x: 975, y: 500, z: 1.5 },
                { f: T.peso - T.ultimos, x: AJUSTES.x + 475, y: AJUSTES.y + 305, z: 1.75 },
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
            <Palabras texto={`*${cifras.pesoInstalador} MB* to install`} tam={104} cada={3} cuenta={15} sale={T.libre - T.peso - PREGUNTA - 8} />
          </Centro>
          <Centro y={80}>
            <Palabras texto="33 MB of memory. The Snipping Tool: 253 MB" entra={s(0.7)} cada={3} tam={50} peso={500} color={COLOR.suave} cuenta={15} sale={T.libre - T.peso - PREGUNTA - 8} />
          </Centro>
        </Sequence>
      </Sequence>

      {/* Lo que es, sobre la app, y el cierre. */}
      <Sequence from={T.libre} durationInFrames={T.cierre - T.libre}>
        <EnLosAjustes tomas={[{ f: 0, x: 960, y: 540, z: 0.9 }, { f: T.cierre - T.libre, x: 960, y: 540, z: 0.84 }]} img="ajustes-capturar.png" />
        {/* Se oscurece casi a negro antes del cierre, que va sobre el fondo oscuro. */}
        <VeloVivo claves={[[0, 0.86, 16], [T.cierre - T.libre - PASO, 0.86, 16], [T.cierre - T.libre, 0.97, 24]]} />
        <Centro>
          <Palabras texto={"Free, open source,\n*no account*"} tam={100} cada={3} sale={T.cierre - T.libre - 7} />
        </Centro>
      </Sequence>
      <Sequence from={T.cierre}>
        <Cierre linea="For Windows and Linux" />
      </Sequence>
      <Efectos lista={sonidos} />
      {/* Un fundido a negro en el último segundo, para que el bucle de Reddit no salte en seco. */}
      <AbsoluteFill style={{ background: "#000", opacity: interpolate(frame, [T.fin - s(0.8), T.fin - 1], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }) }} />
    </AbsoluteFill>
  );
}
