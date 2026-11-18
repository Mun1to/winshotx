import { useCallback, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { listen } from "@tauri-apps/api/event";
import {
  AppWindow,
  Camera,
  Clipboard,
  Keyboard,
  Mouse,
  MousePointerClick,
  Power,
  SquareDashedMousePointer,
  SquarePen,
  Video,
} from "lucide-react";
import {
  cambiarTeclasEnElEscritorio,
  getSettings,
  pedirPermisoDeCapturas,
  printScreenState,
  setSettings,
  shortcutStatus,
  usePrintScreen,
} from "../../lib/ipc";
import type {
  CaptureFlow,
  PrintScreenState,
  Settings,
  ShortcutStatus,
  TeclasDelEscritorio,
} from "../../lib/types";
import { EVENTS } from "../../lib/types";
import { partesDeAtajo } from "../../lib/teclas";
import { ShortcutField } from "../settings/ShortcutField";
// S21 (Linux): sin el paso de Impr Pant, y en Wayland las teclas las pone el escritorio.
import { RowButton } from "../settings/Section";
import { EjemploDeSway, OrdenDelAtajo, TeclaPuesta } from "../settings/TeclaDelEscritorio";
import { usePlataforma, useTeclasDelEscritorio } from "../../lib/plataforma";
import { Marca } from "../ui/Marca";
import { Segmented } from "../ui/Segmented";
import { Switch } from "../ui/Switch";
import { useT } from "../../lib/i18n";
import { DibujoAlSoltar, DibujoAtajos, DibujoBandeja, DibujoImprPant } from "./Dibujos";

/**
 * La bienvenida, una sola vez. Se abre sola tras instalar porque winshotx vive en la
 * bandeja: sin esto, la primera impresion de la app es que no ha pasado nada.
 *
 * Los cuatro pasos son las decisiones que no se pueden adivinar por el usuario: con que
 * teclas lo llama, que pasa al soltar el raton, si le cede la tecla Impr Pant a winshotx y
 * si quiere que arranque con Windows. Todo lo demas trae un valor por defecto que ya funciona.
 *
 * Desde la 0.5 (S38) habla el idioma de los ajustes nuevos: la misma cabecera con el nombre y
 * la version, los pasos como pestañas de texto con su raya, filas sueltas con una linea fina
 * entre ellas, y a la derecha el dibujo de lo que va a pasar.
 *
 * La ventana no cambia de tamaño al pasar de paso (S40): mide lo mismo que los ajustes
 * (`ventanaFija.ts`), y lo que sobra debajo de un paso corto lo llena el dibujo de la derecha,
 * que va de arriba abajo. De S38 a S40 media cada paso y se movia para dejar quieto el pie.
 */

const PASOS = ["Tus teclas", "Al soltar", "Impr Pant", "Listo"];
/** En Linux no hay Herramienta de Recortes a la que quitarle Impr Pant. */
const PASOS_EN_LINUX = PASOS.filter((nombre) => nombre !== "Impr Pant");

/**
 * Combinaciones recomendadas. Elegidas por no chocar con lo que ya usa Windows ni con lo
 * que usan a diario el navegador y el editor: `Ctrl+Shift+2` y `Ctrl+Shift+5` son las de
 * fabrica, `Ctrl+Alt+letra` casi nunca esta cogida, y `Alt+letra` es la mas corta de teclear.
 */
const SUGERIDOS_CAPTURA = ["CmdOrCtrl+Shift+Digit2", "CmdOrCtrl+Alt+KeyA", "Alt+KeyX"];
const SUGERIDOS_GRABACION = ["CmdOrCtrl+Shift+Digit5", "CmdOrCtrl+Alt+KeyR", "Alt+KeyV"];

const BOTON = "rounded-lg px-4 py-2 text-[14px] font-semibold whitespace-nowrap transition-colors";

export function WelcomeApp({ onDone }: { onDone: () => void }) {
  const t = useT();
  const [paso, setPaso] = useState(0);
  const [ajustes, setAjustes] = useState<Settings | null>(null);
  const [imprPant, setImprPant] = useState<PrintScreenState | null>(null);
  const [atajos, setAtajos] = useState<ShortcutStatus>({
    capture: true,
    edit: true,
    record: true,
    replay: false,
    printScreen: false,
    winShiftS: false,
  });
  const [ocupado, setOcupado] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const donde = usePlataforma();
  const enLinux = donde?.sistema === "linux";
  const enWayland = donde?.wayland ?? false;
  // Si «Cambiar» no abre nada (Hyprland, o un escritorio que winshotx no conoce), las teclas se
  // ponen a mano, igual que donde no hay portal.
  const [sinVentanaDeTeclas, setSinVentanaDeTeclas] = useState(false);
  const delEscritorio = useTeclasDelEscritorio(enWayland);
  const teclas =
    delEscritorio && sinVentanaDeTeclas ? { ...delEscritorio, aMano: true } : delEscritorio;
  const pasos = enLinux ? PASOS_EN_LINUX : PASOS;
  const nombre = pasos[paso];
  const cambiarTeclas = useCallback(() => {
    void cambiarTeclasEnElEscritorio().catch(() => setSinVentanaDeTeclas(true));
  }, []);

  useEffect(() => {
    void getSettings().then(setAjustes);
    void shortcutStatus().then(setAtajos);
    // Lo que hay hoy, para que la opcion elegida del paso 3 sea la de verdad. Si no se
    // puede saber, se queda como Windows la trae: la tecla abre la Herramienta de Recortes.
    void printScreenState()
      .then(setImprPant)
      .catch(() => undefined);
    // Ahora winshotx tiene el foco, y es el unico momento en que GNOME deja preguntar por el
    // permiso de capturas: desde un atajo no sale el dialogo. Fuera de Wayland no hace nada.
    let conPermiso = false;
    // El escritorio avisa de los atajos dos veces seguidas: una sola pregunta a la vez.
    let pidiendo = false;
    const pedirPermiso = () => {
      if (pidiendo) return;
      pidiendo = true;
      void pedirPermisoDeCapturas()
        .then((si) => {
          conPermiso = si;
        })
        .catch(() => {})
        .finally(() => {
          pidiendo = false;
        });
    };
    pedirPermiso();
    // En Wayland el escritorio pone los atajos cuando la persona acepta su dialogo, despues
    // de que esta pantalla ya haya preguntado. Y ese dialogo se lleva el foco: si sale a la
    // vez que el del permiso, GNOME no deja salir el del permiso (medido en el laboratorio de
    // S21), asi que al cerrarse se vuelve a pedir.
    const cambios = listen<ShortcutStatus>(EVENTS.shortcutsChanged, ({ payload }) => {
      setAtajos(payload);
      if (!conPermiso) pedirPermiso();
    });
    return () => {
      void cambios.then((quitar) => quitar());
    };
  }, []);

  /** Se guarda en cuanto se toca algo: si la ventana se cierra a medias, no se pierde. */
  const guardar = useCallback((parcial: Partial<Settings>) => {
    setAjustes((previo) => {
      if (!previo) return previo;
      const siguiente = { ...previo, ...parcial };
      void setSettings(siguiente)
        .then(() => shortcutStatus().then(setAtajos))
        .catch((e) => setError(String(e)));
      return siguiente;
    });
  }, []);

  const pedirTecla = useCallback(
    async (quiere: boolean) => {
      if (ocupado) return;
      setOcupado(true);
      setError(null);
      try {
        setImprPant(await usePrintScreen(quiere));
        // El comando guarda los ajustes en Rust por su cuenta: hay que releerlos o el
        // siguiente cambio de este mismo asistente reenviaria el valor viejo.
        setAjustes(await getSettings());
      } catch (e) {
        setError(String(e));
      } finally {
        setOcupado(false);
      }
    },
    [ocupado],
  );

  const terminar = useCallback(() => {
    setAjustes((previo) => {
      if (previo) void setSettings({ ...previo, onboarded: true }).catch(() => undefined);
      return previo;
    });
    onDone();
  }, [onDone]);

  // Hasta saber dónde corre no se pinta nada: en Linux se vería un instante el paso de Impr Pant.
  if (!ajustes || !donde) {
    return <div className="h-full bg-lienzo" />;
  }

  const ultimo = paso === pasos.length - 1;

  return (
    <div className="flex h-full flex-col bg-lienzo text-texto">
      <header className="flex shrink-0 items-stretch justify-between gap-6 border-b border-linea px-6">
        <span className="flex items-center gap-2.5 py-3.5">
          <Marca className="size-6 shrink-0 text-titulo" x="currentColor" />
          <span className="text-[16px] font-bold text-titulo">winshotx</span>
          <span className="text-[13px] text-apagado">{__VERSION__}</span>
        </span>
        <nav className="flex gap-5" aria-label={t("Pasos de la bienvenida")}>
          {pasos.map((cual, i) => (
            <button
              key={cual}
              type="button"
              onClick={() => setPaso(i)}
              aria-current={i === paso ? "step" : undefined}
              className={`-mb-px border-b-[3px] pt-[3px] text-[14px] font-semibold whitespace-nowrap transition-colors ${
                i === paso
                  ? "border-marca text-titulo"
                  : "border-transparent text-apagado hover:text-titulo"
              }`}
            >
              {t(cual)}
            </button>
          ))}
        </nav>
      </header>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto" data-scroll-bienvenida>
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={paso}
            data-paso={paso}
            className="flex flex-1 flex-col"
            initial={{ opacity: 0, x: 14 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -14 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
          >
            {nombre === "Tus teclas" && (
              <Atajos
                ajustes={ajustes}
                atajos={atajos}
                onCaptura={(v) => guardar({ captureShortcut: v })}
                onGrabacion={(v) => guardar({ recordShortcut: v })}
                enLinux={enLinux}
                teclas={enWayland ? teclas : undefined}
                onCambiarTeclas={cambiarTeclas}
              />
            )}
            {nombre === "Al soltar" && (
              <AlSoltar
                valor={ajustes.captureFlow}
                onChange={(v) => guardar({ captureFlow: v })}
              />
            )}
            {nombre === "Impr Pant" && (
              <TeclaImprPant estado={imprPant} onElegir={(quiere) => void pedirTecla(quiere)} />
            )}
            {nombre === "Listo" && (
              <Final
                ajustes={ajustes}
                imprPant={imprPant}
                onArranque={(v) => guardar({ startWithWindows: v })}
                enLinux={enLinux}
                teclas={enWayland ? teclas : undefined}
              />
            )}
          </motion.div>
        </AnimatePresence>
      </div>

      {error && (
        <p
          role="alert"
          className="shrink-0 px-7 pb-2 text-[13px] text-red-400 [[data-tema=claro]_&]:text-red-600"
        >
          {error}
        </p>
      )}

      <footer className="flex shrink-0 items-center justify-between gap-3 border-t border-linea px-6 py-3">
        {paso > 0 ? (
          <button
            type="button"
            onClick={() => setPaso((p) => p - 1)}
            className={`${BOTON} border border-linea bg-realce text-titulo hover:bg-activo`}
          >
            {t("Atrás")}
          </button>
        ) : (
          <span />
        )}
        <button
          type="button"
          onClick={ultimo ? terminar : () => setPaso((p) => p + 1)}
          className={`${BOTON} border border-transparent bg-blue-600 text-white hover:bg-blue-500`}
        >
          {paso === 0 ? t("Empezar") : ultimo ? t("Todo listo") : t("Siguiente")}
        </button>
      </footer>
    </div>
  );
}

/* ------------------------------------------------------------------ las piezas */

/**
 * Un paso: a la izquierda su titulo y sus filas, a la derecha el dibujo de lo que va a pasar.
 * Las dos columnas acaban a la misma altura: el dibujo va en su tarjeta, que se estira, y lo
 * de dentro se centra.
 */
function Paso({
  titulo,
  sub,
  derecha,
  children,
}: {
  titulo: string;
  sub: string;
  derecha: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-1 gap-7 px-7 pt-6 pb-7">
      <div className="min-w-0 flex-1">
        <h1 className="text-[22px] leading-tight font-bold text-pretty text-titulo">{titulo}</h1>
        <p className="mt-1.5 text-[14px] text-pretty text-apagado">{sub}</p>
        {children}
      </div>
      <div className="w-[280px] shrink-0">{derecha}</div>
    </div>
  );
}

/** El dibujo y, debajo, lo mismo dicho en frases cortas. */
function Escenario({
  titulo,
  dibujo,
  frases,
}: {
  titulo: string;
  dibujo: ReactNode;
  frases: ReactNode[];
}) {
  return (
    <figure className="flex h-full flex-col justify-center rounded-xl bg-tarjeta px-5 py-5">
      <figcaption className="mb-4 text-[15px] font-bold text-titulo">{titulo}</figcaption>
      {dibujo}
      <ul className="mt-4 space-y-2 text-[14px] leading-snug text-texto">
        {frases.map((frase, i) => (
          <li key={i} className="flex gap-2.5">
            <span aria-hidden="true" className="mt-[7px] size-[5px] shrink-0 rounded-full bg-marca" />
            <span className="text-pretty">{frase}</span>
          </li>
        ))}
      </ul>
    </figure>
  );
}

/**
 * Una frase traducida con lo que cambia en negrita. La frase va entera con sus `{marcas}`,
 * como manda `t`, y aqui solo se cambia cada marca por su valor resaltado.
 */
function ConNegrita({ frase, datos }: { frase: string; datos: Record<string, string> }) {
  return (
    <>
      {frase.split(/(\{[a-z]+\})/).map((trozo, i) => {
        const marca = /^\{([a-z]+)\}$/.exec(trozo);
        return marca && marca[1] in datos ? (
          <strong key={i} className="font-semibold text-titulo">
            {datos[marca[1]]}
          </strong>
        ) : (
          trozo
        );
      })}
    </>
  );
}

/**
 * Las filas sueltas de los ajustes: titulo suelto encima y las filas debajo, sin raya entre
 * ellas desde S40 (Munito, con la 0.5.0: «no me gusta el diseño de los separadores»). Las
 * separa el aire, como en los ajustes (`settings/Section.tsx`).
 */
function Bloque({ titulo, children }: { titulo?: string; children: ReactNode }) {
  return (
    <section className="mt-5">
      {titulo && <h2 className="mb-1.5 px-1 text-[15px] font-bold text-titulo">{titulo}</h2>}
      <div>{children}</div>
    </section>
  );
}

/** Lo que llevan todas las filas: el aire de los lados y el fondo suave bajo el raton. */
const FILA = "-mx-2 rounded-lg px-3 transition-colors duration-200 hover:bg-realce/60";

/**
 * Una fila: el icono suelto, el nombre a 15 px y debajo lo que explica a 13, sin cortar, y el
 * control a la derecha. Es la fila de los ajustes nuevos (variacion 9); cuando la de los
 * ajustes (`settings/Section.tsx`, P2) sea esa, esta puede pasar a usarla.
 */
function Fila({
  icono,
  nombre,
  pista,
  control,
  debajo = false,
}: {
  icono: ReactNode;
  nombre: string;
  pista?: ReactNode;
  control?: ReactNode;
  /**
   * El control debajo del texto y no a su derecha. Para opciones largas («Herramienta de
   * Recortes»): a la derecha le dejaban a la explicacion tres palabras por linea.
   */
  debajo?: boolean;
}) {
  const texto = (
    <span className="flex min-w-0 flex-1 items-center gap-3">
      <span className="flex w-4 shrink-0 justify-center text-tenue">{icono}</span>
      <span className="min-w-0">
        <span className="block text-[15px] font-medium text-pretty text-titulo">{nombre}</span>
        {pista && <span className="block text-[13px] text-pretty text-apagado">{pista}</span>}
      </span>
    </span>
  );
  if (debajo) {
    return (
      <div className={`${FILA} py-3`}>
        {texto}
        <div className="mt-2 pl-7">{control}</div>
      </div>
    );
  }
  return (
    // Al mismo alto que las de los ajustes (52), tengan lo que tengan a la derecha.
    <div className={`${FILA} flex min-h-[52px] flex-wrap items-center justify-between gap-x-4 gap-y-2 py-2`}>
      {texto}
      {control && <span className="shrink-0">{control}</span>}
    </div>
  );
}

function Tecla({ children }: { children: string }) {
  return (
    <kbd className="rounded-md border border-linea-fuerte bg-realce px-1.5 py-0.5 font-sans text-[12px] font-medium text-texto">
      {children}
    </kbd>
  );
}

function Atajo({ valor }: { valor: string }) {
  return (
    <span className="flex items-center gap-1">
      {partesDeAtajo(valor).map((parte, i) => (
        <Tecla key={parte + String(i)}>{parte}</Tecla>
      ))}
    </span>
  );
}

/** Un atajo para leer dentro de una frase: «Ctrl + Shift + S». */
function atajoEnTexto(valor: string): string {
  return partesDeAtajo(valor).join(" + ");
}

/**
 * Un atajo corto y de una pieza, «Ctrl Shift 2», como lo pinta el campo de al lado: con los
 * «+» las tres sugerencias no cabian en una linea y la ultima bajaba sola a la segunda.
 */
function atajoCorto(valor: string): string {
  return partesDeAtajo(valor).join(" ");
}

/**
 * Combinaciones recomendadas, para no obligar a nadie a inventarse una. En texto, no en
 * cajitas: se pulsan, y la que esta puesta sale en el azul de la marca.
 */
function Sugerencias({
  opciones,
  valor,
  onElegir,
}: {
  opciones: string[];
  valor: string;
  onElegir: (valor: string) => void;
}) {
  const t = useT();
  return (
    <span className="flex flex-wrap items-center gap-x-2">
      <span>{t("o prueba")}</span>
      {opciones.map((opcion, i) => {
        const puesta = opcion === valor;
        return (
          <span key={opcion} className="flex items-center gap-x-2">
            {i > 0 && <span aria-hidden="true">·</span>}
            <button
              type="button"
              onClick={() => onElegir(opcion)}
              aria-pressed={puesta}
              className={`font-semibold transition-colors ${
                puesta ? "text-marca" : "text-texto hover:text-marca"
              }`}
            >
              {atajoCorto(opcion)}
            </button>
          </span>
        );
      })}
    </span>
  );
}

/** Lo de debajo del nombre de un atajo: el aviso si otra app lo tiene, y las sugerencias. */
function PistaDeAtajo({
  libre,
  opciones,
  valor,
  onElegir,
}: {
  libre: boolean;
  opciones: string[];
  valor: string;
  onElegir: (valor: string) => void;
}) {
  const t = useT();
  return (
    <>
      {!libre && (
        <span aria-live="polite" className="block text-aviso">
          {t("Esta la tiene otra aplicación. Prueba con otra.")}
        </span>
      )}
      <Sugerencias opciones={opciones} valor={valor} onElegir={onElegir} />
    </>
  );
}

/* ------------------------------------------------------------------ los cuatro pasos */

function Atajos({
  ajustes,
  atajos,
  onCaptura,
  onGrabacion,
  enLinux,
  teclas,
  onCambiarTeclas,
}: {
  ajustes: Settings;
  atajos: ShortcutStatus;
  onCaptura: (valor: string) => void;
  onGrabacion: (valor: string) => void;
  enLinux: boolean;
  /** Solo en Wayland: las teclas que ha puesto el escritorio (null mientras contesta). */
  teclas?: TeclasDelEscritorio | null;
  onCambiarTeclas: () => void;
}) {
  const t = useT();
  if (teclas !== undefined) {
    return <AtajosEnWayland teclas={teclas} onCambiar={onCambiarTeclas} />;
  }
  return (
    <Paso
      titulo={t("winshotx ya está en marcha")}
      sub={
        enLinux
          ? t("No hay ventana que dejar abierta: se llama con una tecla, hace lo suyo y desaparece.")
          : t("Vive en la bandeja del sistema, junto al reloj. No hay ventana que dejar abierta: se llama con una tecla, hace lo suyo y desaparece.")
      }
      derecha={
        <Escenario
          titulo={t("Lo que vas a ver")}
          dibujo={<DibujoAtajos />}
          frases={[
            <ConNegrita
              key="c"
              frase={t("Pulsas {atajo} y arrastras la zona")}
              datos={{ atajo: atajoEnTexto(ajustes.captureShortcut) }}
            />,
            <ConNegrita
              key="g"
              frase={t("Pulsas {atajo} y eliges qué grabar")}
              datos={{ atajo: atajoEnTexto(ajustes.recordShortcut) }}
            />,
          ]}
        />
      }
    >
      <Bloque titulo={t("Las dos teclas que lo llaman")}>
        <Fila
          icono={<Camera className="size-4" />}
          nombre={t("Capturar una región")}
          pista={
            <PistaDeAtajo
              libre={atajos.capture}
              opciones={SUGERIDOS_CAPTURA}
              valor={ajustes.captureShortcut}
              onElegir={onCaptura}
            />
          }
          control={
            <ShortcutField
              value={ajustes.captureShortcut}
              active={atajos.capture}
              onChange={onCaptura}
            />
          }
        />
        <Fila
          icono={<Video className="size-4" />}
          nombre={t("Grabar en GIF o vídeo")}
          pista={
            <PistaDeAtajo
              libre={atajos.record}
              opciones={SUGERIDOS_GRABACION}
              valor={ajustes.recordShortcut}
              onElegir={onGrabacion}
            />
          }
          control={
            <ShortcutField
              value={ajustes.recordShortcut}
              active={atajos.record}
              onChange={onGrabacion}
            />
          }
        />
      </Bloque>
      <p className="mt-3 px-1 text-[13px] text-pretty text-apagado">
        {t("Haz clic en una y teclea la combinación que prefieras.")}
      </p>
      <p className="mt-2 px-1 text-[13px] text-pretty text-apagado">
        {t("Todo se queda en tu ordenador: sin cuenta, sin nube y sin nada que subir.")}
      </p>
    </Paso>
  );
}

/**
 * El primer paso en Wayland. Las dos teclas son las que se le piden al escritorio (capturar, y
 * capturar y abrir el editor), y no se escriben aquí: el escritorio enseña su ventana con las
 * que propone winshotx y la persona las acepta o las cambia ahí. Lo que se ve es lo que ha
 * quedado puesto, y «Cambiar» vuelve a abrir esa ventana.
 */
function AtajosEnWayland({
  teclas,
  onCambiar,
}: {
  teclas: TeclasDelEscritorio | null;
  onCambiar: () => void;
}) {
  const t = useT();
  const aMano = teclas?.aMano ?? false;
  const filas = [
    { id: "capture", icono: <Camera className="size-4" />, nombre: t("Capturar una región") },
    { id: "edit", icono: <SquarePen className="size-4" />, nombre: t("Capturar y abrir el editor") },
  ] as const;
  return (
    <Paso
      titulo={t("winshotx ya está en marcha")}
      sub={t("No hay ventana que dejar abierta: se llama con una tecla, hace lo suyo y desaparece.")}
      derecha={
        <Escenario
          titulo={t("Lo que vas a ver")}
          dibujo={<DibujoAtajos />}
          frases={[
            t("Pulsa el atajo cuando quieras"),
            t("Todo se queda en tu ordenador: sin cuenta, sin nube y sin nada que subir."),
          ]}
        />
      }
    >
      <p className="mt-4 px-1 text-[13px] text-pretty text-apagado">
        {aMano
          ? t("Este escritorio no deja que las apps pidan sus teclas. Ponlas tú en sus atajos de teclado, con estas dos órdenes:")
          : t("Estas son las dos teclas con las que se llama. En Wayland las elige el escritorio: acepta las que te propone en su ventana, o cámbialas ahí con «Cambiar».")}
      </p>
      <Bloque titulo={t("Las dos teclas que lo llaman")}>
        {filas.map((fila) => (
          <Fila
            key={fila.id}
            icono={fila.icono}
            nombre={fila.nombre}
            control={
              aMano ? (
                <OrdenDelAtajo orden={teclas?.orden ?? "winshotx"} accion={fila.id} />
              ) : (
                <span className="flex items-center gap-1.5">
                  <TeclaPuesta tecla={teclas?.[fila.id] ?? null} />
                  <RowButton onClick={onCambiar}>{t("Cambiar")}</RowButton>
                </span>
              )
            }
          />
        ))}
      </Bloque>
      {aMano && (
        <div className="mt-3 px-1">
          <EjemploDeSway orden={teclas?.orden ?? "winshotx"} />
        </div>
      )}
    </Paso>
  );
}

function AlSoltar({ valor, onChange }: { valor: CaptureFlow; onChange: (v: CaptureFlow) => void }) {
  const t = useT();
  const conBarra = valor !== "instant";
  return (
    <Paso
      titulo={t("¿Cómo prefieres capturar?")}
      sub={t("Las dos formas usan el mismo atajo y la misma selección. Lo que cambia es lo que pasa al soltar el ratón, y se puede cambiar cuando quieras desde los ajustes.")}
      derecha={
        <Escenario
          titulo={t("Lo que vas a ver")}
          dibujo={<DibujoAlSoltar conBarra={conBarra} />}
          frases={
            conBarra
              ? [
                  t("De esa barra salen el editor, el GIF y el vídeo"),
                  t("Grabar empieza al soltar, en las dos formas"),
                ]
              : [
                  t("Para pegar en un chat sin pensar"),
                  t("Grabar empieza al soltar, en las dos formas"),
                ]
          }
        />
      }
    >
      <Bloque>
        <Fila
          icono={<Mouse className="size-4" />}
          nombre={t("Al soltar el ratón")}
          pista={conBarra ? t("Seleccionas y eliges qué hacer.") : t("Seleccionas y se copia sola.")}
          control={
            <Segmented
              etiqueta={t("Al soltar el ratón")}
              value={conBarra ? "toolbar" : "instant"}
              options={[
                { value: "toolbar", label: t("Con barra") },
                { value: "instant", label: t("Al vuelo") },
              ]}
              onChange={(v) => onChange(v as CaptureFlow)}
            />
          }
        />
      </Bloque>
      <Bloque titulo={t("Cómo va")}>
        <Fila icono={<Keyboard className="size-4" />} nombre={t("Pulsas el atajo")} />
        <Fila
          icono={<SquareDashedMousePointer className="size-4" />}
          nombre={t("Arrastras la región")}
        />
        {conBarra ? (
          <Fila
            icono={<MousePointerClick className="size-4" />}
            nombre={t("Copiar, guardar, editar o grabar")}
          />
        ) : (
          <Fila icono={<Clipboard className="size-4" />} nombre={t("Ya está en el portapapeles")} />
        )}
      </Bloque>
    </Paso>
  );
}

function TeclaImprPant({
  estado,
  onElegir,
}: {
  estado: PrintScreenState | null;
  onElegir: (quiere: boolean) => void;
}) {
  const t = useT();
  const deWinshotx = estado?.enabled === true;
  const pista = !deWinshotx
    ? t("La Herramienta de Recortes se queda con Impr Pant y winshotx se llama con su atajo.")
    : estado?.active
      ? t("Si Windows sigue abriendo la Herramienta de Recortes con Impr Pant, cierra sesión y vuelve a entrar.")
      : null;
  return (
    <Paso
      titulo={t("¿Le quitamos la tecla a la Herramienta de Recortes?")}
      sub={t("En Windows, la tecla Impr Pant abre la Herramienta de Recortes. Si quieres, winshotx se queda con ella y responde a ese mismo dedo, sin aprender ningún atajo nuevo.")}
      derecha={
        <Escenario
          titulo={t("Lo que vas a ver")}
          dibujo={<DibujoImprPant deWinshotx={deWinshotx} />}
          frases={[t("Tus atajos de winshotx funcionan siempre, cojas esta tecla o no")]}
        />
      }
    >
      <Bloque>
        <Fila
          icono={<Keyboard className="size-4" />}
          nombre={t("Impr Pant abre")}
          debajo
          pista={
            <span aria-live="polite">
              {deWinshotx && !estado?.active ? (
                <span className="text-aviso">
                  {t("Impr Pant no ha caído: hay otro programa que la tiene cogida.")}
                </span>
              ) : (
                pista
              )}
            </span>
          }
          control={
            <Segmented
              etiqueta={t("Impr Pant abre")}
              value={deWinshotx ? "winshotx" : "recortes"}
              options={[
                { value: "winshotx", label: "winshotx" },
                { value: "recortes", label: t("Herramienta de Recortes") },
              ]}
              onChange={(v) => {
                if ((v === "winshotx") !== deWinshotx) onElegir(v === "winshotx");
              }}
            />
          }
        />
        <Fila
          icono={<AppWindow className="size-4" />}
          nombre={t("Win + Mayús + S")}
          pista={t("Se queda en Windows: quitársela apaga también Win + S, la búsqueda. Si la quieres, está en Ajustes, en Teclas de Windows.")}
        />
      </Bloque>
    </Paso>
  );
}

function Final({
  ajustes,
  imprPant,
  onArranque,
  enLinux,
  teclas,
}: {
  ajustes: Settings;
  imprPant: PrintScreenState | null;
  onArranque: (valor: boolean) => void;
  enLinux: boolean;
  /** Solo en Wayland: la tecla de captura es la que puso el escritorio. */
  teclas?: TeclasDelEscritorio | null;
}) {
  const t = useT();
  const arrancar = enLinux ? t("Arrancar al iniciar sesión") : t("Arrancar con Windows");
  return (
    <Paso
      titulo={t("Listo, ya puedes capturar")}
      sub={
        enLinux
          ? t("Esto es lo que queda configurado. Todo se cambia después en los ajustes, que se abren desde winshotx en el menú de aplicaciones.")
          : t("Esto es lo que queda configurado. Todo se cambia después desde el icono de la bandeja.")
      }
      derecha={
        enLinux ? (
          // El dibujo de la bandeja es la de Windows, y en GNOME ni siquiera hay bandeja.
          <Escenario
            titulo={t("Lo que vas a ver")}
            dibujo={<DibujoAtajos />}
            frases={[t("Pulsa el atajo cuando quieras. Los ajustes se abren desde winshotx en el menú de aplicaciones.")]}
          />
        ) : (
          <Escenario
            titulo={t("Dónde se queda")}
            dibujo={<DibujoBandeja />}
            frases={[
              t("Pulsa el atajo cuando quieras"),
              t("Con el botón derecho en el icono de la bandeja se abren los ajustes"),
            ]}
          />
        )
      }
    >
      <Bloque>
        <Fila
          icono={<Camera className="size-4" />}
          nombre={t("Capturar una región")}
          pista={imprPant?.enabled && imprPant.active ? t("También con Impr Pant") : undefined}
          control={
            teclas?.aMano ? (
              <code className="font-mono text-[12px] text-apagado">{`${teclas.orden || "winshotx"} --capture`}</code>
            ) : teclas !== undefined ? (
              <TeclaPuesta tecla={teclas?.capture ?? null} />
            ) : (
              <Atajo valor={ajustes.captureShortcut} />
            )
          }
        />
        {/* En Wayland grabar todavía no se le pide al escritorio (Fase 2): no se enseña una
            tecla que no va a hacer nada. */}
        {teclas === undefined && (
          <Fila
            icono={<Video className="size-4" />}
            nombre={t("Grabar en GIF o vídeo")}
            control={<Atajo valor={ajustes.recordShortcut} />}
          />
        )}
        <Fila
          icono={<Mouse className="size-4" />}
          nombre={t("Al soltar el ratón")}
          control={
            <span className="text-[14px] text-texto">
              {ajustes.captureFlow === "instant"
                ? t("se copia al portapapeles")
                : t("sale la barra para elegir")}
            </span>
          }
        />
        <Fila
          icono={<Power className="size-4" />}
          nombre={arrancar}
          pista={enLinux ? t("se queda esperando, sin ventana") : t("se abre en la bandeja, sin ventana")}
          control={
            <Switch
              checked={ajustes.startWithWindows}
              onChange={onArranque}
              label={arrancar}
            />
          }
        />
      </Bloque>
    </Paso>
  );
}
