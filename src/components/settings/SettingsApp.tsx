import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  AppWindow,
  BookOpen,
  Camera,
  Compass,
  Clipboard,
  Crop,
  EyeOff,
  FolderOpen,
  Gauge,
  Monitor,
  HardDrive,
  History,
  Keyboard,
  Languages,
  Mic,
  MousePointer2,
  Palette,
  Power,
  Scissors,
  SquarePen,
  Timer,
  Video,
  Volume2,
  ZoomIn,
} from "lucide-react";
import {
  cacheStats,
  clearCache,
  getSettings,
  justUpdated,
  openFolder,
  openWindowsApps,
  pickDirectory,
  removeSnippingTool,
  useWinShiftS,
  listScreens,
  printScreenState,
  quitApp,
  replayStatus,
  restartShell,
  setSettings,
  shortcutStatus,
  usePrintScreen,
} from "../../lib/ipc";
import { formatBytes } from "../../lib/format";
import { aplicarTema } from "../../lib/tema";
import { aplicarIdioma, useT } from "../../lib/i18n";
import {
  EVENTS,
  type CacheStats,
  type CaptureFlow,
  type PrintScreenState,
  type ReplayStatus,
  type Screen,
  type Settings,
  type Theme,
  type Language,
  type ShortcutStatus,
} from "../../lib/types";
import { Segmented } from "../ui/Segmented";
import { Switch } from "../ui/Switch";
import { Row, RowButton, Section } from "./Section";
import { SettingsHeader, type SeccionId } from "./SettingsHeader";
import { GuidedTour } from "./GuidedTour";
import { UpdateRow } from "./UpdateRow";
import { ShortcutField } from "./ShortcutField";

/** La inyecta Vite desde package.json: escribirla a mano acababa en cuatro copias. */
const VERSION = __VERSION__;

const FPS_OPTIONS = [
  { value: 15, label: "15 fps" },
  { value: 30, label: "30 fps" },
  { value: 60, label: "60 fps" },
];

/**
 * Cuanto guarda hacia atras el anillo.
 *
 * Tres valores y no una casilla donde escribir un numero: aqui no se elige un numero, se
 * elige entre «lo que acaba de pasar», «la jugada entera» y «el minuto malo».
 */
const SEGUNDOS_ATRAS = [
  { value: 15, label: "15 s" },
  { value: 30, label: "30 s" },
  { value: 60, label: "60 s" },
];

/**
 * A cuántos fotogramas graba el anillo.
 *
 * Es el ajuste que de verdad decide lo que cuesta tenerlo puesto, porque aquí se graba todo
 * el rato: lo que vale un fotograma se paga toda la tarde. Por eso la fila enseña al lado
 * cuánto disco puede llegar a ocupar, en vez de dejar que se descubra solo.
 */
const FPS_ANILLO = [
  { value: 15, label: "15 fps" },
  { value: 30, label: "30 fps" },
  { value: 60, label: "60 fps" },
];

/**
 * Lo que puede llegar a ocupar el anillo, en bytes.
 *
 * Sale de lo mismo que el tope de Rust (`buffer::bytes_max`): 2,3 MB por fotograma, que es
 * lo que mide uno de pantalla completa cuando la pantalla cambia entera, medido sobre una
 * partida a 1920 × 1080. Un escritorio de trabajo gasta una décima parte, así que este es
 * el techo y no lo normal.
 */
const PEOR_FOTOGRAMA = 2_300_000;
const TECHO = 4 * 1024 * 1024 * 1024;
const loQuePuedeOcupar = (segundos: number, fps: number) =>
  Math.min(segundos * fps * PEOR_FOTOGRAMA, TECHO);

const FLUJOS: { value: CaptureFlow; label: string }[] = [
  { value: "toolbar", label: "Sale la barra" },
  { value: "instant", label: "Se copia sola" },
];

/** El automatico va primero porque es el de fabrica y el que acierta casi siempre. */
const TEMAS: { value: Theme; label: string }[] = [
  { value: "sistema", label: "Automático" },
  { value: "claro", label: "Claro" },
  { value: "oscuro", label: "Oscuro" },
];

/** Los dos idiomas que habla hoy. Anadir uno mas es una linea aqui y un archivo mas. */
const IDIOMAS: { value: Language; label: string }[] = [
  { value: "sistema", label: "Automático" },
  { value: "es", label: "Español" },
  { value: "en", label: "Inglés" },
];

/** Tres opciones y ninguna más: un campo de números aquí solo sirve para escribir 47. */
const ESPERAS = [
  { value: 0, label: "Sin espera" },
  { value: 3, label: "3 segundos" },
  { value: 5, label: "5 segundos" },
];

interface SettingsAppProps {
  onVerBienvenida: () => void;
  /** Arranca el tour nada mas montarse: es lo que pasa al terminar la bienvenida. */
  arrancarTour?: boolean;
}

export function SettingsApp({ onVerBienvenida, arrancarTour = false }: SettingsAppProps) {
  const t = useT();
  // Las tablas de opciones se guardan en espannol y se traducen al pintarlas: el `value`
  // es lo que va al disco y ese no puede cambiar de idioma nunca.
  const flujos = FLUJOS.map((o) => ({ ...o, label: t(o.label) }));
  const temas = TEMAS.map((o) => ({ ...o, label: t(o.label) }));
  const esperas = ESPERAS.map((o) => ({ ...o, label: t(o.label) }));
  const idiomas = IDIOMAS.map((o) => ({ ...o, label: t(o.label) }));
  // Que seccion se esta viendo. Es estado de la pantalla y no un ajuste: no tiene sentido
  // guardarlo en disco ni que sobreviva a cerrar la ventana.
  const [seccion, setSeccion] = useState<SeccionId>("capturar");
  // Si esta ventana se ha abierto sola porque se acaba de actualizar, hay que llevarle
  // a donde esta la noticia. Abrir en Capturar dejaria el aviso en otra pestanna, y la
  // ventana se leeria como que ha aparecido porque si.
  const [recienActualizado, setRecienActualizado] = useState(false);
  const [tour, setTour] = useState(arrancarTour);
  const [settings, setLocal] = useState<Settings | null>(null);
  const [shortcuts, setShortcuts] = useState<ShortcutStatus>({
    capture: true,
    record: true,
    replay: false,
    printScreen: false,
    winShiftS: false,
  });
  const [pantallas, setPantallas] = useState<Screen[]>([]);
  const [replay, setReplay] = useState<ReplayStatus>({
    running: false,
    seconds: 0,
    screen: 0,
    screenLabel: "",
    bytes: 0,
    bufferedMs: 0,
  });
  const [cache, setCache] = useState<CacheStats>({ bytes: 0, sessions: 0 });
  const [imprPant, setImprPant] = useState<PrintScreenState | null>(null);
  /** null = sin tocar · "confirmar" = esperando el segundo clic · el resto, el resultado. */
  const [recortes, setRecortes] = useState<string | null>(null);
  /** Mientras se reinicia el Explorador, que son un par de segundos largos. */
  const [aplicando, setAplicando] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refrescar = useCallback(() => {
    void getSettings().then(setLocal);
    void shortcutStatus().then(setShortcuts);
    void cacheStats().then(setCache);
    void printScreenState().then(setImprPant);
    void replayStatus()
      .then((estado) => estado && setReplay(estado))
      .catch(() => {
        // Una compilacion sin este comando: la fila sale como si estuviera apagado.
      });
    // Las pantallas se preguntan al abrir los ajustes y no al arrancar la app: enchufar o
    // quitar un monitor es justo lo que pasa entre una vez y otra.
    void listScreens()
      .then((lista) => lista && setPantallas(lista))
      .catch(() => setPantallas([]));
  }, []);

  // Cerrar los ajustes solo esconde la ventana, nunca la destruye, asi que esto se
  // monta una vez por sesion. Sin volver a preguntar al reaparecer, el tamanno de la
  // cache se quedaba en el que tenia al arrancar y el boton de vaciar, muerto.
  useEffect(() => {
    refrescar();
    setError(null);
    // Se pregunta una sola vez y en un solo sitio: en Rust la respuesta se consume al
    // leerla, asi que dos preguntas dejarian a una de las dos sin enterarse.
    void justUpdated()
      .then((si) => {
        if (!si) return;
        setRecienActualizado(true);
        setSeccion("app");
      })
      .catch(() => {
        // Una compilacion sin este comando: se abre donde siempre.
      });
    const unlisten = listen(EVENTS.settingsShown, () => {
      refrescar();
      setError(null);
    });
    // El anillo cambia por su cuenta: se enciende al arrancar la app, va llenandose y
    // guarda cuando alguien pulsa la tecla. Preguntando solo al abrir la ventana, la
    // fila se quedaria contando lo de hace un rato.
    const unReplay = listen<ReplayStatus>(EVENTS.replay, (evento) => setReplay(evento.payload));
    return () => {
      void unlisten.then((fn) => fn());
      void unReplay.then((fn) => fn());
    };
  }, [refrescar]);

  // El ultimo valor conocido, para poder guardar fuera del updater de useState.
  const ultimo = useRef<Settings | null>(null);
  ultimo.current = settings;

  const patch = useCallback((partial: Partial<Settings>) => {
    const prev = ultimo.current;
    if (!prev) return;
    const next = { ...prev, ...partial };
    // Guardar dentro del updater parecia mas corto, pero React lo llama dos veces y
    // salian dos escrituras y dos re-registros de atajos por cada cambio.
    ultimo.current = next;
    setError(null);
    setLocal(next);
    void setSettings(next)
      .then(() => {
        setSaved(true);
        window.setTimeout(() => setSaved(false), 1200);
        return shortcutStatus().then(setShortcuts);
      })
      .catch((e) => {
        setError(String(e));
        // Rust puede haber rechazado parte del cambio (encender el anillo y no poder).
        // Sin releer, el interruptor se quedaria en «si» sobre algo que no esta pasando.
        void getSettings().then((frescos) => {
          ultimo.current = frescos;
          setLocal(frescos);
        });
        void replayStatus()
          .then((estado) => estado && setReplay(estado))
          .catch(() => {});
      });
  }, []);

  const cambiarImprPant = useCallback(async (quiere: boolean) => {
    setError(null);
    try {
      setImprPant(await usePrintScreen(quiere));
      // usePrintScreen escribe los ajustes en Rust. Sin volver a leerlos, el proximo
      // cambio de cualquier otra fila reenviaria el valor viejo y desharia esto.
      const frescos = await getSettings();
      ultimo.current = frescos;
      setLocal(frescos);
      setShortcuts(await shortcutStatus());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  // Desinstalar algo no puede pasar de un clic despistado: el primero pregunta y el
  // segundo lo hace. Y a los pocos segundos se olvida, para no dejar la trampa armada.
  const quitarRecortes = useCallback(async () => {
    if (recortes !== "confirmar") {
      setRecortes("confirmar");
      window.setTimeout(() => setRecortes((r) => (r === "confirmar" ? null : r)), 5000);
      return;
    }
    setRecortes("quitando");
    try {
      const habia = await removeSnippingTool();
      setRecortes(habia ? "quitada" : "ya no estaba");
    } catch (e) {
      setRecortes(null);
      setError(String(e));
    }
  }, [recortes]);

  // Va por su cuenta porque es lo unico que le quita algo al usuario: la lista de atajos
  // de Windows va por letra, asi que apagar Win+Mayus+S apaga tambien Win+S.
  const cambiarWinShiftS = useCallback(async (quiere: boolean) => {
    setError(null);
    try {
      await useWinShiftS(quiere);
      const frescos = await getSettings();
      ultimo.current = frescos;
      setLocal(frescos);
      setShortcuts(await shortcutStatus());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  // Reiniciar el Explorador es lo que hace que Windows relea la lista de teclas apagadas.
  // Antes esto era "cierra sesión y vuelve a entrar", que nadie hace por un atajo, y hasta
  // entonces la tecla no era de nadie: ni la abría Windows ni la cogía winshotx.
  const aplicarTecla = useCallback(async () => {
    setError(null);
    setAplicando(true);
    try {
      setShortcuts(await restartShell());
    } catch (e) {
      setError(String(e));
    } finally {
      setAplicando(false);
    }
  }, []);

  if (!settings) {
    return (
      <div className="flex h-full items-center justify-center bg-lienzo text-sm text-tenue">
        Cargando…
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-lienzo">
      <SettingsHeader
        activa={seccion}
        onCambiar={setSeccion}
        version={VERSION}
        guardado={saved}
        onSalir={() => void quitApp()}
      />

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
        {/* Dos columnas, que es para lo que se subio la navegacion arriba: cada seccion
            tiene dos bloques y asi la mas cargada cabe entera sin rueda. */}
        <div className="grid grid-cols-2 items-start gap-4">

            {seccion === "capturar" && (
              <>
                <Section title={t("Al pulsar el atajo")} tour="al-pulsar">
                  <Row
                    icon={<Camera className="size-4" />}
                    label={t("Capturar región")}
                    hint={shortcuts.capture ? undefined : t("esa combinación está ocupada")}
                    tone="warn"
                    control={
                      <ShortcutField
                        value={settings.captureShortcut}
                        active={shortcuts.capture}
                        onChange={(v) => patch({ captureShortcut: v })}
                      />
                    }
                  />
                  <Row
                    icon={<Timer className="size-4" />}
                    label={t("Esperar antes de capturar")}
                    hint={
                      settings.captureDelaySeconds === 0
                        ? t("la pantalla se congela al pulsar el atajo")
                        : t("da tiempo a abrir el menú que quieres fotografiar")
                    }
                    stacked
                    control={
                      <Segmented
                        value={settings.captureDelaySeconds}
                        options={esperas}
                        onChange={(v) => patch({ captureDelaySeconds: v })}
                      />
                    }
                  />
                  <Row
                    icon={<EyeOff className="size-4" />}
                    label={t("Ocultar iconos del escritorio")}
                    hint={t("solo mientras dura el disparo")}
                    control={
                      <Switch
                        checked={settings.hideDesktopIcons}
                        onChange={(v) => patch({ hideDesktopIcons: v })}
                        label={t("Ocultar iconos del escritorio")}
                      />
                    }
                  />

                </Section>

                <Section title={t("La captura")} tour="la-captura">
                  <Row
                    icon={<MousePointer2 className="size-4" />}
                    label={t("Incluir el cursor")}
                    control={
                      <Switch
                        checked={settings.captureCursor}
                        onChange={(v) => patch({ captureCursor: v })}
                        label={t("Incluir el cursor")}
                      />
                    }
                  />
                  <Row
                    icon={<ZoomIn className="size-4" />}
                    label={t("Lupa de píxel")}
                    control={
                      <Switch
                        checked={settings.showMagnifier}
                        onChange={(v) => patch({ showMagnifier: v })}
                        label={t("Lupa de píxel")}
                      />
                    }
                  />
                  <Row
                    icon={<Crop className="size-4" />}
                    label={t("Al soltar el ratón")}
                    hint={
                      settings.captureFlow === "instant"
                        ? t("va directa al portapapeles")
                        : t("sale la barra para copiar, guardar o editar")
                    }
                    stacked
                    control={
                      <Segmented
                        value={settings.captureFlow}
                        options={flujos}
                        onChange={(v) => patch({ captureFlow: v })}
                      />
                    }
                  />
                  <Row
                    icon={<Clipboard className="size-4" />}
                    label={t("Copiar al guardar")}
                    control={
                      <Switch
                        checked={settings.copyAfterCapture}
                        onChange={(v) => patch({ copyAfterCapture: v })}
                        label={t("Copiar al guardar")}
                      />
                    }
                  />
                  <Row
                    icon={<Volume2 className="size-4" />}
                    label={t("Sonido de obturador")}
                    control={
                      <Switch
                        checked={settings.playSound}
                        onChange={(v) => patch({ playSound: v })}
                        label={t("Sonido de obturador")}
                      />
                    }
                  />
                </Section>
              </>
            )}

            {seccion === "grabar" && (
              <>
                <Section title={t("Cómo se graba")} tour="como-se-graba">
                  <Row
                    icon={<Video className="size-4" />}
                    label={t("Grabar región")}
                    hint={
                      shortcuts.record ? t("el mismo atajo la termina") : t("esa combinación está ocupada")
                    }
                    tone={shortcuts.record ? "normal" : "warn"}
                    control={
                      <ShortcutField
                        value={settings.recordShortcut}
                        active={shortcuts.record}
                        onChange={(v) => patch({ recordShortcut: v })}
                      />
                    }
                  />
                  <Row
                    icon={<Gauge className="size-4" />}
                    label={t("Fotogramas por segundo")}
                    stacked
                    control={
                      <Segmented
                        value={settings.fps}
                        options={FPS_OPTIONS}
                        onChange={(v) => patch({ fps: v })}
                      />
                    }
                  />
                </Section>

                {/* El sonido, en su bloque. Estaba dentro de «Cómo se graba» y con el
                    micrófono al lado eran seis filas en una columna, que ya no cabían: la
                    sección salía con rueda y la columna de al lado, vacía. */}
                <Section title={t("El sonido")}>
                  <Row
                    icon={<Volume2 className="size-4" />}
                    label={t("Audio del sistema")}
                    hint={t("lo que suene por los altavoces, dentro del vídeo")}
                    control={
                      <Switch
                        checked={settings.recordAudio}
                        onChange={(v) => patch({ recordAudio: v })}
                        label={t("Audio del sistema")}
                      />
                    }
                  />
                  <Row
                    icon={<Mic className="size-4" />}
                    label={t("Micrófono")}
                    hint={
                      settings.recordAudio
                        ? t("tu voz, mezclada con el sonido del sistema")
                        : t("tu voz, para narrar lo que se está grabando")
                    }
                    control={
                      <Switch
                        checked={settings.recordMicrophone}
                        onChange={(v) => patch({ recordMicrophone: v })}
                        label={t("Micrófono")}
                      />
                    }
                  />
                </Section>


                {/* Los ultimos segundos. Es la unica funcion de winshotx que trabaja sin
                    que nadie se lo haya pedido en ese momento, asi que la fila dice
                    siempre que esta haciendo, que pantalla mira y cuanto ocupa. Un
                    interruptor que solo dijera «si» no seria honesto. */}
                <Section title={t("Los últimos segundos")}>
                  <Row
                    icon={<History className="size-4" />}
                    label={t("Grabar siempre lo último")}
                    hint={
                      replay.running
                        ? // El número y no el nombre del monitor: Windows los llama
                          // «\.\DISPLAY3», que no le dice nada a nadie. El número es el
                          // mismo que sale en la pantalla al elegirla para capturar.
                          t("vigilando la pantalla {n} · {tamaño} en disco", {
                            n: replay.screen,
                            "tamaño": formatBytes(replay.bytes),
                          })
                        : t("graba sin parar y tira lo viejo, para poder rescatar lo último")
                    }
                    tone={replay.running ? "ok" : "normal"}
                    control={
                      <Switch
                        checked={settings.replayEnabled}
                        onChange={(v) => patch({ replayEnabled: v })}
                        label={t("Grabar siempre lo último")}
                      />
                    }
                  />
                  <Row
                    icon={<Timer className="size-4" />}
                    label={t("Cuánto se guarda")}
                    hint={t("hacia atrás, desde que pulses la tecla")}
                    stacked
                    control={
                      <Segmented
                        value={settings.replaySeconds}
                        options={SEGUNDOS_ATRAS}
                        onChange={(v) => patch({ replaySeconds: v })}
                      />
                    }
                  />
                  {/* Solo con más de una pantalla: elegir entre una es una fila de ruido. */}
                  {pantallas.length > 1 && (
                    <Row
                      icon={<Monitor className="size-4" />}
                      label={t("Qué pantalla")}
                      hint={t("no puede cambiar de pantalla sin tirar lo que lleva grabado")}
                      stacked
                      control={
                        <Segmented
                          etiqueta={t("Qué pantalla")}
                          value={settings.replayScreen ?? -1}
                          options={[
                            { value: -1, label: t("La del ratón") },
                            ...pantallas.map((p) => ({
                              value: p.id,
                              label: `${p.id + 1}${p.isPrimary ? " ★" : ""}`,
                            })),
                          ]}
                          onChange={(v) => patch({ replayScreen: v === -1 ? null : v })}
                        />
                      }
                    />
                  )}
                  <Row
                    icon={<Gauge className="size-4" />}
                    label={t("Calidad")}
                    hint={t("hasta {tamaño} en disco si la pantalla cambia entera", {
                      "tamaño": formatBytes(
                        loQuePuedeOcupar(settings.replaySeconds, settings.replayFps),
                      ),
                    })}
                    stacked
                    control={
                      <Segmented
                        value={settings.replayFps}
                        options={FPS_ANILLO}
                        onChange={(v) => patch({ replayFps: v })}
                        etiqueta={t("Calidad")}
                      />
                    }
                  />
                  <Row
                    icon={<Video className="size-4" />}
                    label={t("Quedarme con lo último")}
                    hint={
                      !replay.running
                        ? t("primero hay que encenderlo aquí arriba")
                        : replay.bufferedMs < settings.replaySeconds * 1000
                          ? t("ahora mismo hay {n} s guardados", {
                              n: Math.floor(replay.bufferedMs / 1000),
                            })
                          : shortcuts.replay
                            ? t("abre el editor con lo último, sin dejar de grabar")
                            : t("esa combinación está ocupada")
                    }
                    tone={replay.running && !shortcuts.replay ? "warn" : "normal"}
                    control={
                      <ShortcutField
                        value={settings.replayShortcut}
                        // Con el anillo apagado la tecla NO se pide al sistema, así que
                        // pintarla en rojo diría que está ocupada cuando solo está sin
                        // usar. El rojo tiene que significar una sola cosa.
                        active={shortcuts.replay || !replay.running}
                        onChange={(v) => patch({ replayShortcut: v })}
                      />
                    }
                  />
                </Section>

                {/* Debajo del sonido y no en la otra columna: con tres bloques, la
                    rejilla mandaria este a la izquierda y dejaria la derecha a medias. */}
                <Section title={t("Al terminar")} className="col-start-2">
                  <Row
                    icon={<SquarePen className="size-4" />}
                    label={t("Abrir el editor al terminar")}
                    control={
                      <Switch
                        checked={settings.openEditorAfterRecording}
                        onChange={(v) => patch({ openEditorAfterRecording: v })}
                        label={t("Abrir el editor al terminar")}
                      />
                    }
                  />
                </Section>
              </>
            )}

            {seccion === "teclas" && (
              <>
                <Section title={t("Las teclas de captura de Windows")} tour="las-teclas">
                  <Row
                    icon={<Keyboard className="size-4" />}
                    label={t("Impr Pant")}
                    hint={
                      imprPant?.enabled
                        ? imprPant.active
                          ? t("ya es de winshotx")
                          : t("cierra sesión para que Windows la suelte")
                        : t("hoy abre la Herramienta de Recortes")
                    }
                    tone={imprPant?.enabled ? (imprPant.active ? "ok" : "warn") : "normal"}
                    control={
                      <Switch
                        checked={imprPant?.enabled ?? false}
                        onChange={(v) => void cambiarImprPant(v)}
                        label={t("Usar también Impr Pant")}
                      />
                    }
                  />
                  <Row
                    icon={<AppWindow className="size-4" />}
                    label={t("Win + Mayús + S")}
                    hint={
                      !settings.takeWinShiftS
                        ? t("cuesta Win+S, la búsqueda")
                        : aplicando
                          ? t("reiniciando el Explorador…")
                          : shortcuts.winShiftS
                            ? t("de winshotx, o pulsa Aplicar si Windows la sigue abriendo")
                            : t("el escritorio todavía la tiene")
                    }
                    tone={
                      !settings.takeWinShiftS ? "normal" : shortcuts.winShiftS ? "ok" : "warn"
                    }
                    control={
                      <span className="flex items-center gap-1.5">
                        {/*
                          Cuando el atajo de captura del usuario ya es Win+Mayus+S, el backend lo
                          marca como conseguido en cuanto RegisterHotKey tiene exito, sin poder
                          saber si el Explorador (que intercepta esta combinacion antes que
                          cualquier programa) ya releyo la lista de teclas apagadas o sigue con la
                          de antes de activar el interruptor. El boton se deja siempre visible en
                          vez de solo cuando el backend dice que falta: reiniciar el Explorador no
                          hace dano de mas, y es la unica forma real de confirmarlo.
                        */}
                        {settings.takeWinShiftS && (
                          <RowButton
                            disabled={aplicando}
                            onClick={() => void aplicarTecla()}
                            title={t("Reinicia el Explorador para que Windows suelte la tecla. La barra de tareas parpadea un segundo y no se cierra nada más.")}
                          >
                            {aplicando ? t("Un momento…") : t("Aplicar")}
                          </RowButton>
                        )}
                        <Switch
                          checked={settings.takeWinShiftS}
                          onChange={(v) => void cambiarWinShiftS(v)}
                          label={t("Quedarme con Win+Mayús+S")}
                        />
                      </span>
                    }
                  />
                </Section>
                <Section title={t("La Herramienta de Recortes")}>
                  <Row
                    icon={<Scissors className="size-4" />}
                    label={t("Herramienta de Recortes")}
                    hint={
                      recortes === "confirmar"
                        ? t("vuelve desde la Microsoft Store")
                        : recortes === "quitando"
                          ? t("quitándola…")
                          : recortes === "quitada"
                            ? t("quitada, ya no abre nada")
                            : recortes === "ya no estaba"
                              ? t("no estaba instalada")
                              : t("quitarla es lo único que la calla del todo")
                    }
                    tone={
                      recortes === "confirmar"
                        ? "warn"
                        : recortes === "quitada" || recortes === "ya no estaba"
                          ? "ok"
                          : "normal"
                    }
                    control={
                      <span className="flex gap-1.5">
                        {!recortes && (
                          <RowButton onClick={() => void openWindowsApps()}>{t("Ver cómo")}</RowButton>
                        )}
                        <RowButton
                          danger={recortes === "confirmar"}
                          disabled={
                            recortes === "quitando" ||
                            recortes === "quitada" ||
                            recortes === "ya no estaba"
                          }
                          onClick={() => void quitarRecortes()}
                        >
                          {recortes === "confirmar"
                            ? t("Sí, quitarla")
                            : recortes === "quitada" || recortes === "ya no estaba"
                              ? t("Hecho")
                              : t("Quitar")}
                        </RowButton>
                      </span>
                    }
                  />
                </Section>
              </>
            )}

            {seccion === "app" && (
              <>
                {/* Esta seccion tiene tres bloques y las demas dos. Sin envolver los dos
                    de la izquierda, el tercero abre una fila nueva por debajo del bloque
                    mas alto y la seccion deja de caber sin rueda. */}
                <div className="flex flex-col">
                <Section title={t("Archivos")} tour="archivos">
                  <Row
                    icon={<FolderOpen className="size-4" />}
                    label={t("Carpeta de destino")}
                    hint={settings.saveDirectory}
                    control={
                      <span className="flex gap-1.5">
                        <RowButton onClick={() => void openFolder(settings.saveDirectory)}>
                          {t("Abrir")}
                        </RowButton>
                        <RowButton
                          onClick={() =>
                            void pickDirectory().then((dir) => dir && patch({ saveDirectory: dir }))
                          }
                        >
                          {t("Cambiar")}
                        </RowButton>
                      </span>
                    }
                  />
                  <Row
                    icon={<HardDrive className="size-4" />}
                    label={t("Caché de grabaciones")}
                    hint={
                      cache.sessions === 0
                        ? t("vacía")
                        : t("{tam} en {n} {palabra}", {
                            tam: formatBytes(cache.bytes),
                            n: cache.sessions,
                            palabra: cache.sessions === 1 ? t("sesión") : t("sesiones"),
                          })
                    }
                    control={
                      <RowButton
                        disabled={cache.sessions === 0}
                        onClick={() =>
                          void clearCache()
                            .then(setCache)
                            .catch((e) => setError(String(e)))
                        }
                      >
                        {t("Vaciar")}
                      </RowButton>
                    }
                  />
                </Section>
                <Section title={t("Aspecto")}>
                  <Row
                    icon={<Palette className="size-4" />}
                    label={t("Tema")}
                    hint={settings.theme === "sistema" ? t("sigue a Windows") : undefined}
                    control={
                      <Segmented
                        ajustado
                        value={settings.theme}
                        options={temas}
                        onChange={(v) => {
                          // Se pinta en el acto y se guarda despues: esperar a que Rust
                          // conteste para cambiar de color hace que el boton parezca roto.
                          aplicarTema(v);
                          patch({ theme: v });
                        }}
                      />
                    }
                  />
                  <Row
                    icon={<Languages className="size-4" />}
                    label={t("Idioma")}
                    hint={settings.language === "sistema" ? t("el de Windows") : undefined}
                    control={
                      <Segmented
                        ajustado
                        value={settings.language}
                        options={idiomas}
                        onChange={(v) => {
                          // Igual que el tema: se cambia el idioma en el acto y se guarda
                          // despues. Los textos se repintan solos, sin recargar la ventana
                          // ni perder por donde iba el usuario.
                          aplicarIdioma(v);
                          patch({ language: v });
                        }}
                      />
                    }
                  />
                </Section>
                </div>
                <Section title={t("winshotx")}>
                  <UpdateRow version={VERSION} recienActualizado={recienActualizado} />
                  <Row
                    icon={<BookOpen className="size-4" />}
                    label={t("Bienvenida")}
                    control={<RowButton onClick={onVerBienvenida}>{t("Ver otra vez")}</RowButton>}
                  />
                  <Row
                    icon={<Compass className="size-4" />}
                    label={t("Tour de los ajustes")}
                    hint={t("seis paradas, una por sección")}
                    control={<RowButton onClick={() => setTour(true)}>{t("Empezar")}</RowButton>}
                  />
                  <Row
                    icon={<Power className="size-4" />}
                    label={t("Arrancar con Windows")}
                    control={
                      <Switch
                        checked={settings.startWithWindows}
                        onChange={(v) => patch({ startWithWindows: v })}
                        label={t("Arrancar con Windows")}
                      />
                    }
                  />
                </Section>
              </>
            )}
        </div>
      </div>

      {tour && <GuidedTour onNavegar={setSeccion} onCerrar={() => setTour(false)} />}

      {/* Fuera de la rejilla: un fallo escondido al final de una columna no se ve. */}
      {error && (
        <p className="mx-4 mb-3 shrink-0 rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      )}

    </div>
  );
}
