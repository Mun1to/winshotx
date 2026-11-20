/**
 * «Así va a salir»: el dibujo en vivo de cada apartado de los ajustes (S36, la variación 9).
 *
 * Un dibujo pequeño de lo que va a pasar con lo que está elegido ahora mismo, y debajo lo
 * mismo dicho en frases. Se dibuja con los ajustes de verdad: si se apaga el cursor, el
 * cursor desaparece de la foto del dibujo y la frase cambia. Por eso las filas de al lado ya
 * no llevan su explicación debajo: la cuenta esto.
 *
 * Las frases van enteras, con sus `{marcadores}`, para que cada idioma las ordene a su
 * manera; lo que va entre `**` sale en negrita (la palabra que cambia con el ajuste).
 */
import type { ReactNode } from "react";

import type { ReplayStatus, Settings, SonidoDisponible, TeclasDelEscritorio } from "../../lib/types";
import { FONDOS } from "../../lib/fondos";
import { partesDeAtajo } from "../../lib/teclas";
import { useT } from "../../lib/i18n";
import type { ApartadoId } from "./mapaDeAjustes";

/** El fondo de escritorio del dibujo: azul y verde azulado apagados, como un fondo de Windows. */
const ESCRITORIO = "linear-gradient(140deg, #2b59c3 0%, #1f7a72 55%, #24647a 100%)";

const atajo = (a: string) => partesDeAtajo(a).join(" + ");

/**
 * S45 (Linux): con qué se dispara cada cosa en Wayland. Ahí no vale la tecla de los ajustes de
 * la app: la pone el escritorio, o se ata a mano a una orden donde no deja pedirla (Sway).
 */
interface TeclasEnWayland {
  /** Las que ha puesto el escritorio. `null` mientras no contesta. */
  delEscritorio: TeclasDelEscritorio | null;
  /** Que aquí las teclas se ponen a mano, en los atajos del escritorio. */
  aMano: boolean;
  /** Cómo se escribe winshotx aquí: `winshotx`, o la ruta del AppImage. */
  orden: string;
}

/**
 * Lo que va en negrita delante de «y eliges la zona»: la tecla de los ajustes. En Wayland, la
 * del escritorio (tal cual la escribe él, si no se sabe leer) o la orden que se ata a mano; y
 * `null` si el escritorio no ha dado ninguna, que es cuando el dibujo lo tiene que decir.
 */
function conQue(
  deLosAjustes: string,
  accion: "capture" | "record" | "replay",
  wayland: TeclasEnWayland | null,
): string | null {
  if (!wayland) return atajo(deLosAjustes);
  if (wayland.aMano) return `${wayland.orden || "winshotx"} --${accion}`;
  const tecla = wayland.delEscritorio?.[accion];
  if (!tecla) return null;
  return tecla.atajo ? atajo(tecla.atajo) : tecla.texto;
}

/** Parte una frase ya traducida por sus `**` y pone en negrita lo de dentro. */
export function conNegritas(frase: string): ReactNode[] {
  return frase.split(/\*\*(.+?)\*\*/).map((trozo, i) =>
    i % 2 === 1 ? (
      <strong key={i} className="font-semibold text-titulo">
        {trozo}
      </strong>
    ) : (
      trozo
    ),
  );
}

/** La pantalla de mentira: el fondo del escritorio, sus iconos y una ventana con texto. */
function Pantallita({ children, iconos = true }: { children?: ReactNode; iconos?: boolean }) {
  return (
    <div className="relative aspect-[16/10] w-full overflow-hidden rounded-lg" style={{ background: ESCRITORIO }}>
      {iconos && (
        <div data-dibujo="iconos" className="absolute left-[3%] top-[6%] flex flex-col gap-[9%]" style={{ height: "60%" }}>
          {[0, 1, 2].map((i) => (
            <div key={i} className="flex flex-col items-center gap-[3px]">
              <div className="size-[18px] rounded-[4px] bg-white/85" />
              <div className="h-[3px] w-[22px] rounded-full bg-white/60" />
            </div>
          ))}
        </div>
      )}
      <div className="absolute left-[20%] top-[12%] h-[66%] w-[64%] overflow-hidden rounded-md bg-[#f4f4f5] shadow-[0_6px_20px_rgba(0,0,0,0.35)]">
        <div className="flex h-[13%] items-center justify-end gap-[6%] bg-[#e4e4e7] px-[4%]">
          <span className="h-[2px] w-[7px] bg-zinc-500" />
          <span className="size-[7px] border border-zinc-500" />
          {/* La X de cerrar, dibujada con dos rayas: un glifo de texto sale distinto en cada letra. */}
          <span className="relative size-[7px]">
            <span className="absolute left-1/2 top-0 h-full w-px -translate-x-1/2 rotate-45 bg-zinc-500" />
            <span className="absolute left-1/2 top-0 h-full w-px -translate-x-1/2 -rotate-45 bg-zinc-500" />
          </span>
        </div>
        <div className="space-y-[5%] p-[6%]">
          <div className="h-[7px] w-[55%] rounded-full bg-zinc-400" />
          <div className="h-[5px] w-[90%] rounded-full bg-zinc-300" />
          <div className="h-[5px] w-[80%] rounded-full bg-zinc-300" />
          <div className="h-[5px] w-[86%] rounded-full bg-zinc-300" />
          <div className="mt-[8%] h-[26px] w-[40%] rounded bg-sky-200" />
        </div>
      </div>
      {children}
    </div>
  );
}

function Cursor({ className = "" }: { className?: string }) {
  return (
    <svg data-dibujo="cursor" viewBox="0 0 14 20" className={`absolute h-[20px] w-[14px] drop-shadow ${className}`}>
      <path d="M1 1 L1 16 L5 12.5 L8 19 L10.5 18 L7.6 11.6 L13 11.6 Z" fill="#fff" stroke="#111" strokeWidth="1.2" />
    </svg>
  );
}

/**
 * Lo mismo que el dibujo, en frases, con un punto de la marca delante de cada una.
 *
 * `partible` es para cuando una frase lleva una orden (S45, donde las teclas se ponen a mano):
 * la de un AppImage es su ruta entera, sin un espacio, y sin poder partirse por cualquier letra
 * se saldría de la tarjeta.
 */
function Frases({ lineas, partible = false }: { lineas: (string | false | null | undefined)[]; partible?: boolean }) {
  return (
    <ul data-frases className="mt-4 space-y-2.5 text-[14px] leading-snug text-texto">
      {lineas.filter((l): l is string => Boolean(l)).map((l) => (
        <li key={l} className="flex gap-2.5">
          <span aria-hidden="true" className="mt-[7px] size-[5px] shrink-0 rounded-full bg-marca" />
          <span className={partible ? "text-pretty [overflow-wrap:anywhere]" : "text-pretty"}>{conNegritas(l)}</span>
        </li>
      ))}
    </ul>
  );
}

function Capturar({ s, wayland }: { s: Settings; wayland: TeclasEnWayland | null }) {
  const t = useT();
  const con = conQue(s.captureShortcut, "capture", wayland);
  return (
    <>
      <Pantallita iconos={!s.hideDesktopIcons}>
        <div
          className="absolute rounded-[2px] border-2 border-dashed border-[#0a9bff]"
          style={{ left: "30%", top: "30%", width: "42%", height: "40%", boxShadow: "0 0 0 999px rgba(0,0,0,0.45)" }}
        />
        <Cursor className="left-[71%] top-[69%]" />
        {s.showMagnifier && (
          <div
            data-dibujo="lupa"
            className="absolute left-[76%] top-[38%] size-[64px] overflow-hidden rounded-full border-2 border-white shadow-lg"
            style={{
              background:
                "repeating-linear-gradient(0deg, transparent 0 7px, rgba(0,0,0,.18) 7px 8px), repeating-linear-gradient(90deg, transparent 0 7px, rgba(0,0,0,.18) 7px 8px), linear-gradient(90deg, #f4f4f5 50%, #bae6fd 50%)",
            }}
          >
            <div className="absolute left-1/2 top-1/2 size-[9px] -translate-x-1/2 -translate-y-1/2 border-2 border-[#0a9bff]" />
          </div>
        )}
        {s.captureDelaySeconds > 0 && (
          <div data-dibujo="cuenta" className="absolute inset-0 flex items-center justify-center">
            <span className="text-[44px] font-bold text-white drop-shadow-[0_2px_8px_rgba(0,0,0,0.6)]">
              {s.captureDelaySeconds}
            </span>
          </div>
        )}
      </Pantallita>
      <Frases
        partible={wayland?.aMano}
        lineas={[
          con === null
            ? t("**Todavía sin tecla**: pídela con «Cambiar»")
            : t("**{atajo}** y eliges la zona con el ratón", { atajo: con }),
          s.captureDelaySeconds > 0
            ? t("Cuenta **{n} s** antes de hacer la foto", { n: s.captureDelaySeconds })
            : t("La foto sale en cuanto sueltas"),
          s.hideDesktopIcons
            ? t("Los iconos del escritorio **no salen** en la foto")
            : t("Los iconos del escritorio salen en la foto"),
          s.captureCursor ? t("El cursor **sale** en la foto") : t("El cursor **no sale** en la foto"),
          s.showMagnifier && t("Una lupa al lado del ratón para afinar al píxel"),
        ]}
      />
    </>
  );
}

function Despues({ s }: { s: Settings }) {
  const t = useT();
  const marco = s.marcoCapturas ?? { aire: 0, fondo: "atardecer" as const };
  const fondo = FONDOS.find((f) => f.id === marco.fondo);
  const conFondo = marco.aire > 0;
  const segundos = s.miniaturaSegundos ?? 10;
  return (
    <>
      <div className="relative flex aspect-[16/10] w-full items-center justify-center overflow-hidden rounded-lg">
        <div
          data-dibujo={conFondo ? "fondo" : undefined}
          className={`flex items-center justify-center ${conFondo ? "rounded-md" : ""}`}
          style={conFondo ? { background: fondo?.css ?? FONDOS[0].css, padding: `${Math.max(10, marco.aire * 2.2)}px` } : undefined}
        >
          <div
            className={`relative h-[118px] w-[170px] overflow-hidden bg-[#f4f4f5] ${
              conFondo ? "rounded-md shadow-[0_8px_22px_rgba(0,0,0,0.35)]" : "border border-linea-fuerte"
            }`}
          >
            <div className="space-y-2 p-3">
              <div className="h-[7px] w-[60%] rounded-full bg-zinc-400" />
              <div className="h-[5px] w-[92%] rounded-full bg-zinc-300" />
              <div className="h-[5px] w-[78%] rounded-full bg-zinc-300" />
              <div className="mt-3 h-[28px] w-[46%] rounded bg-sky-200" />
            </div>
            {s.captureCursor && <Cursor className="left-[62%] top-[58%]" />}
          </div>
        </div>
        {s.miniatura !== false && (
          <div
            data-dibujo="miniatura"
            className="absolute bottom-[7%] right-[5%] h-[34px] w-[52px] rounded-[4px] border-2 border-white bg-[#f4f4f5] shadow-lg"
          />
        )}
      </div>
      <Frases
        lineas={[
          s.captureFlow === "instant"
            ? t("Al soltar, la foto **se copia sola**")
            : t("Al soltar, sale **la barra** para elegir qué hacer"),
          s.copyAfterCapture && t("Al guardarla, también se copia"),
          s.miniatura === false
            ? t("Sin miniatura en la esquina")
            : segundos > 0
              ? t("Se queda una miniatura en la esquina **{n} s**", { n: segundos })
              : t("Se queda una miniatura en la esquina hasta que la cierres"),
          conFondo
            ? t("Sale sobre un fondo **{fondo}**, con sombra y esquinas redondas", { fondo: t(fondo?.nombre ?? "") })
            : t("Sale tal cual, sin fondo"),
          s.playSound ? t("Suena el obturador") : t("Sin sonido de obturador"),
        ]}
      />
    </>
  );
}

function Grabar({
  s,
  hay,
  conBarra,
  wayland,
}: {
  s: Settings;
  hay: SonidoDisponible | null;
  conBarra: boolean;
  wayland: TeclasEnWayland | null;
}) {
  const t = useT();
  const con = conQue(s.recordShortcut, "record", wayland);
  // Lo que se va a grabar de verdad: lo encendido en los ajustes que además se pueda grabar
  // aquí. Donde el sistema no da el sonido (o no hay micrófono) el interruptor sale apagado
  // aunque el ajuste siga encendido, y esta frase tiene que decir lo mismo que él.
  const conPc = s.recordAudio && (hay === null || hay.altavoz);
  const conMicro = s.recordMicrophone && (hay === null || hay.microfono);
  const sonido =
    conPc && conMicro
      ? t("Con el sonido del PC y tu micrófono")
      : conPc
        ? t("Con el sonido del PC, sin micrófono")
        : conMicro
          ? t("Con tu micrófono, sin el sonido del PC")
          : t("Sin sonido");
  return (
    <>
      <Pantallita>
        <div
          data-dibujo="marco"
          className="absolute rounded-[3px] border-2 border-red-500"
          style={{ left: "16%", top: "8%", width: "72%", height: "76%" }}
        >
          {/* La barra de grabar, con su tiempo. En Wayland no la hay (una ventana no se puede
              dejar fuera del vídeo ni elegir dónde se pone), así que allí no se dibuja. */}
          {conBarra && (
            <div
              data-dibujo="barra"
              className="absolute -top-[11px] left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-full bg-zinc-900/90 px-2 py-[2px] text-[10px] font-semibold tabular-nums text-white"
            >
              <span className="size-[6px] rounded-full bg-red-500" />
              0:12
            </div>
          )}
          {s.recordCamera && (
            <div
              data-dibujo="camara"
              className="absolute bottom-[6%] left-[4%] size-[46px] overflow-hidden rounded-full border-2 border-white bg-slate-500 shadow-lg"
            >
              <div className="absolute left-1/2 top-[22%] size-[16px] -translate-x-1/2 rounded-full bg-slate-200" />
              <div className="absolute left-1/2 top-[62%] h-[26px] w-[34px] -translate-x-1/2 rounded-t-full bg-slate-200" />
            </div>
          )}
        </div>
        <Cursor className="left-[58%] top-[56%]" />
      </Pantallita>
      <Frases
        partible={wayland?.aMano}
        lineas={[
          con === null
            ? t("**Todavía sin tecla**: pídela con «Cambiar»")
            : t("**{atajo}** y eliges qué grabar", { atajo: con }),
          t("A **{fps} fps**", { fps: s.fps }),
          sonido,
          s.recordCamera ? t("Con tu cámara en una esquina") : t("Sin cámara"),
          s.openEditorAfterRecording ? t("Al terminar, se abre el editor") : t("Al terminar, se guarda sin abrir nada"),
        ]}
      />
    </>
  );
}

function Ultimos({
  s,
  wayland,
  encendido,
}: {
  s: Settings;
  wayland: TeclasEnWayland | null;
  /** Que el anillo está grabando: en Wayland su tecla solo se le pide al escritorio entonces. */
  encendido: boolean;
}) {
  const t = useT();
  const con = conQue(s.replayShortcut, "replay", wayland);
  const n = 12;
  const guardados = Math.max(2, Math.round((s.replaySeconds / 60) * n));
  const desde = ((n - guardados) / n) * 100;
  return (
    <>
      <div className="pt-2">
        <div className={`relative flex items-end gap-[3px] ${s.replayEnabled ? "" : "opacity-40"}`}>
          {Array.from({ length: n }, (_, i) => (
            <div
              key={i}
              data-dibujo={i >= n - guardados ? "guardado" : undefined}
              className={`aspect-[16/10] flex-1 rounded-[2px] ${i >= n - guardados ? "" : "opacity-25"}`}
              style={{ background: ESCRITORIO }}
            />
          ))}
        </div>
        <div className="relative mt-2 h-[30px]">
          <div
            className="absolute top-0 h-[8px] rounded-b-[3px] border-x-2 border-b-2 border-marca"
            style={{ left: `${desde}%`, right: 0 }}
          />
          {/* Los dos textos van juntos y pegados a la derecha: a lo ancho de la llave si caben, y
              si no crecen hacia la izquierda. Sueltos, a 15 s (la llave es un cuarto del dibujo)
              «hace 15 s» empezaba en el borde de la llave y se metía debajo de «ahora». */}
          <div
            className="absolute right-0 top-[12px] flex justify-between gap-2 whitespace-nowrap text-[12px]"
            style={{ minWidth: `${100 - desde}%` }}
          >
            <span className="text-apagado">{t("hace {n} s", { n: s.replaySeconds })}</span>
            <span className="font-semibold text-titulo">{t("ahora")}</span>
          </div>
        </div>
      </div>
      <Frases
        partible={wayland?.aMano}
        lineas={[
          s.replayEnabled
            ? t("**Grabando siempre**, en memoria, sin guardar nada")
            : t("**Apagado**: no graba nada hasta que lo enciendas"),
          con !== null
            ? t("**{atajo}** y te quedas con los últimos **{n} s**", { atajo: con, n: s.replaySeconds })
            : encendido
              ? t("**Todavía sin tecla**: pídela con «Cambiar»")
              : // Apagado, «Cambiar» no se puede pulsar: lo que va a pasar es que el escritorio
                // pregunta por la tecla al encenderlo.
                t("Al encenderlo eliges la tecla que se queda con los últimos **{n} s**", { n: s.replaySeconds }),
          s.replayHeight === 0
            ? t("A tamaño real, a {fps} fps", { fps: s.replayFps })
            : t("A {alto}p, a {fps} fps", { alto: s.replayHeight, fps: s.replayFps }),
          t("Lo que no guardas se borra solo"),
        ]}
      />
    </>
  );
}

function Tecla({ children, viva, ancha }: { children: ReactNode; viva?: boolean; ancha?: boolean }) {
  return (
    <span
      data-viva={viva ? "" : undefined}
      className={`flex h-[38px] items-center justify-center whitespace-nowrap rounded-md border-2 px-2 text-[12.5px] font-semibold transition-colors ${
        ancha ? "min-w-[64px]" : "min-w-[38px]"
      } ${viva ? "border-marca text-titulo" : "border-linea-fuerte text-apagado"}`}
    >
      {children}
    </span>
  );
}

function Teclas({ s }: { s: Settings }) {
  const t = useT();
  return (
    <>
      <div className="space-y-5">
        <div data-dibujo="impr-pant">
          {/* Tres teclas, las de esa esquina del teclado: con cuatro no cabían en una fila y el
              dibujo crecía sin avisar. */}
          <div className="flex gap-1.5">
            <Tecla>F12</Tecla>
            <Tecla viva={s.printScreenCapture} ancha>
              {t("Impr Pant")}
            </Tecla>
            <Tecla ancha>{t("Bloq Despl")}</Tecla>
          </div>
          <p className="mt-2 text-[13px] text-apagado">
            {s.printScreenCapture ? t("abre winshotx") : t("abre la captura de Windows")}
          </p>
        </div>
        <div data-dibujo="win-mayus-s">
          <div className="flex items-center gap-1.5 text-apagado">
            <Tecla viva={s.takeWinShiftS}>Win</Tecla>+
            <Tecla viva={s.takeWinShiftS} ancha>
              {t("Mayús")}
            </Tecla>
            +<Tecla viva={s.takeWinShiftS}>S</Tecla>
          </div>
          <p className="mt-2 text-[13px] text-apagado">
            {s.takeWinShiftS ? t("abre winshotx") : t("abre la Herramienta de Recortes")}
          </p>
        </div>
      </div>
      <Frases
        lineas={[
          s.printScreenCapture ? t("**Impr Pant** abre winshotx") : t("**Impr Pant** sigue siendo de Windows"),
          s.takeWinShiftS ? t("**Win + Mayús + S** abre winshotx") : t("**Win + Mayús + S** sigue siendo de Windows"),
          t("Tus atajos de winshotx funcionan siempre, cojas estas o no"),
        ]}
      />
    </>
  );
}

function Mini({ claro, elegido }: { claro: boolean; elegido: boolean }) {
  const t = useT();
  return (
    <div className="flex-1" data-dibujo={elegido ? (claro ? "tema-claro" : "tema-oscuro") : undefined}>
      <div
        className={`rounded-md border-2 p-2 transition-colors ${elegido ? "border-marca" : "border-linea"}`}
        style={{ background: claro ? "#f6f6f7" : "#161618" }}
      >
        <div className="mb-2 flex gap-1.5">
          <div className={`h-[5px] w-[30%] rounded-full ${claro ? "bg-zinc-800" : "bg-zinc-100"}`} />
          <div className={`h-[5px] w-[20%] rounded-full ${claro ? "bg-zinc-400" : "bg-zinc-600"}`} />
        </div>
        {[0, 1, 2].map((i) => (
          <div key={i} className={`flex items-center justify-between border-t py-1.5 ${claro ? "border-black/10" : "border-white/10"}`}>
            <div className={`h-[4px] w-[45%] rounded-full ${claro ? "bg-zinc-400" : "bg-zinc-500"}`} />
            <div className={`h-[8px] w-[15px] rounded-full ${i === 1 ? "bg-[#0a9bff]" : claro ? "bg-zinc-300" : "bg-zinc-700"}`} />
          </div>
        ))}
      </div>
      <p className={`mt-1.5 text-center text-[13px] ${elegido ? "font-semibold text-titulo" : "text-apagado"}`}>
        {claro ? t("Claro") : t("Oscuro")}
      </p>
    </div>
  );
}

function App({ s, enLinux }: { s: Settings; enLinux: boolean }) {
  const t = useT();
  const partes = s.saveDirectory.split(/[\\/]/).filter(Boolean);
  const carpeta = partes[partes.length - 1] ?? s.saveDirectory;
  return (
    <>
      <div className="flex gap-3">
        <Mini claro elegido={s.theme === "claro" || s.theme === "sistema"} />
        <Mini claro={false} elegido={s.theme === "oscuro" || s.theme === "sistema"} />
      </div>
      <Frases
        lineas={[
          t("Guarda en **{carpeta}**", { carpeta }),
          s.theme === "sistema"
            ? enLinux
              ? t("Claro u oscuro, como el sistema")
              : t("Claro u oscuro, como Windows")
            : s.theme === "claro"
              ? t("Siempre en claro")
              : t("Siempre en oscuro"),
          enLinux
            ? s.startWithWindows
              ? t("Arranca al iniciar sesión")
              : t("No arranca sola al iniciar sesión")
            : s.startWithWindows
              ? t("Arranca con Windows, en la bandeja")
              : t("No arranca sola con Windows"),
          // «En español» se traduce por el nombre del idioma de cada diccionario («In English»,
          // «中文界面»): el diccionario que se usa es justo el del idioma elegido.
          s.language === "sistema"
            ? enLinux
              ? t("En el idioma del sistema")
              : t("En el idioma de Windows")
            : t("En español"),
        ]}
      />
    </>
  );
}

/**
 * El dibujo de un apartado. Va dentro del panel de «Así va a salir» (fondo de tarjeta y
 * esquinas redondas), que lo pone `SettingsApp` a toda la altura de la columna.
 */
export function VistaPrevia({
  apartado,
  settings,
  enLinux = false,
  enWayland = false,
  sonido = null,
  replay,
  teclasDelEscritorio = null,
  teclasAMano = false,
  ordenDeLaApp = "winshotx",
}: {
  apartado: ApartadoId;
  settings: Settings;
  /** S21 (Linux): las frases que hablan de Windows hablan allí del sistema. */
  enLinux?: boolean;
  /** S44 (Linux): en Wayland no hay barra de grabar, y el dibujo de Grabar no la pinta. */
  enWayland?: boolean;
  /** S44 (Linux): qué sonido se puede grabar aquí. null donde no hay nada que avisar (Windows). */
  sonido?: SonidoDisponible | null;
  /** Cómo va el anillo. En Wayland dice si su tecla se le ha pedido ya al escritorio (S45). */
  replay?: ReplayStatus;
  /** S45 (Linux): en Wayland, las teclas que ha puesto el escritorio. */
  teclasDelEscritorio?: TeclasDelEscritorio | null;
  /** S45 (Linux): que en este escritorio las teclas se ponen a mano, con la orden. */
  teclasAMano?: boolean;
  /** S45 (Linux): cómo se escribe winshotx aquí, para la orden. */
  ordenDeLaApp?: string;
}) {
  const wayland: TeclasEnWayland | null = enWayland
    ? { delEscritorio: teclasDelEscritorio, aMano: teclasAMano, orden: ordenDeLaApp }
    : null;
  switch (apartado) {
    case "capturar":
      return <Capturar s={settings} wayland={wayland} />;
    case "despues":
      return <Despues s={settings} />;
    case "grabar":
      return <Grabar s={settings} hay={sonido} conBarra={!enWayland} wayland={wayland} />;
    case "ultimos":
      return <Ultimos s={settings} wayland={wayland} encendido={replay?.running ?? settings.replayEnabled} />;
    case "teclas":
      return <Teclas s={settings} />;
    case "app":
      return <App s={settings} enLinux={enLinux} />;
    default:
      return null;
  }
}
