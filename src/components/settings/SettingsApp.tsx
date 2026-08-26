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
  HardDrive,
  Keyboard,
  Mic,
  MousePointer2,
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
  openFolder,
  openWindowsApps,
  pickDirectory,
  removeSnippingTool,
  useWinShiftS,
  printScreenState,
  quitApp,
  restartShell,
  setSettings,
  shortcutStatus,
  usePrintScreen,
} from "../../lib/ipc";
import { formatBytes } from "../../lib/format";
import {
  EVENTS,
  type CacheStats,
  type CaptureFlow,
  type PrintScreenState,
  type Settings,
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

const FLUJOS: { value: CaptureFlow; label: string }[] = [
  { value: "toolbar", label: "Sale la barra" },
  { value: "instant", label: "Se copia sola" },
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
  // Que seccion se esta viendo. Es estado de la pantalla y no un ajuste: no tiene sentido
  // guardarlo en disco ni que sobreviva a cerrar la ventana.
  const [seccion, setSeccion] = useState<SeccionId>("capturar");
  const [tour, setTour] = useState(arrancarTour);
  const [settings, setLocal] = useState<Settings | null>(null);
  const [shortcuts, setShortcuts] = useState<ShortcutStatus>({
    capture: true,
    record: true,
    printScreen: false,
    winShiftS: false,
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
  }, []);

  // Cerrar los ajustes solo esconde la ventana, nunca la destruye, asi que esto se
  // monta una vez por sesion. Sin volver a preguntar al reaparecer, el tamanno de la
  // cache se quedaba en el que tenia al arrancar y el boton de vaciar, muerto.
  useEffect(() => {
    refrescar();
    setError(null);
    const unlisten = listen(EVENTS.settingsShown, () => {
      refrescar();
      setError(null);
    });
    return () => {
      void unlisten.then((fn) => fn());
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
      .catch((e) => setError(String(e)));
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
      <div className="flex h-full items-center justify-center bg-[#161618] text-sm text-neutral-500">
        Cargando…
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-[#161618]">
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
                <Section title="Al pulsar el atajo" tour="al-pulsar">
                  <Row
                    icon={<Camera className="size-4" />}
                    label="Capturar región"
                    hint={shortcuts.capture ? undefined : "esa combinación está ocupada"}
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
                    label="Esperar antes de capturar"
                    hint={
                      settings.captureDelaySeconds === 0
                        ? "la pantalla se congela al pulsar el atajo"
                        : "da tiempo a abrir el menú que quieres fotografiar"
                    }
                    stacked
                    control={
                      <Segmented
                        value={settings.captureDelaySeconds}
                        options={ESPERAS}
                        onChange={(v) => patch({ captureDelaySeconds: v })}
                      />
                    }
                  />
                  <Row
                    icon={<EyeOff className="size-4" />}
                    label="Ocultar iconos del escritorio"
                    hint="solo mientras dura el disparo"
                    control={
                      <Switch
                        checked={settings.hideDesktopIcons}
                        onChange={(v) => patch({ hideDesktopIcons: v })}
                        label="Ocultar iconos del escritorio"
                      />
                    }
                  />

                </Section>

                <Section title="La captura" tour="la-captura">
                  <Row
                    icon={<MousePointer2 className="size-4" />}
                    label="Incluir el cursor"
                    control={
                      <Switch
                        checked={settings.captureCursor}
                        onChange={(v) => patch({ captureCursor: v })}
                        label="Incluir el cursor"
                      />
                    }
                  />
                  <Row
                    icon={<ZoomIn className="size-4" />}
                    label="Lupa de píxel"
                    control={
                      <Switch
                        checked={settings.showMagnifier}
                        onChange={(v) => patch({ showMagnifier: v })}
                        label="Lupa de píxel"
                      />
                    }
                  />
                  <Row
                    icon={<Crop className="size-4" />}
                    label="Al soltar el ratón"
                    hint={
                      settings.captureFlow === "instant"
                        ? "va directa al portapapeles"
                        : "sale la barra para copiar, guardar o editar"
                    }
                    stacked
                    control={
                      <Segmented
                        value={settings.captureFlow}
                        options={FLUJOS}
                        onChange={(v) => patch({ captureFlow: v })}
                      />
                    }
                  />
                  <Row
                    icon={<Clipboard className="size-4" />}
                    label="Copiar al guardar"
                    control={
                      <Switch
                        checked={settings.copyAfterCapture}
                        onChange={(v) => patch({ copyAfterCapture: v })}
                        label="Copiar al guardar"
                      />
                    }
                  />
                  <Row
                    icon={<Volume2 className="size-4" />}
                    label="Sonido de obturador"
                    control={
                      <Switch
                        checked={settings.playSound}
                        onChange={(v) => patch({ playSound: v })}
                        label="Sonido de obturador"
                      />
                    }
                  />
                </Section>
              </>
            )}

            {seccion === "grabar" && (
              <>
                <Section title="Cómo se graba" tour="como-se-graba">
                  <Row
                    icon={<Video className="size-4" />}
                    label="Grabar región"
                    hint={
                      shortcuts.record ? "el mismo atajo la termina" : "esa combinación está ocupada"
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
                    label="Fotogramas por segundo"
                    stacked
                    control={
                      <Segmented
                        value={settings.fps}
                        options={FPS_OPTIONS}
                        onChange={(v) => patch({ fps: v })}
                      />
                    }
                  />
                  <Row
                    icon={<Mic className="size-4" />}
                    label="Audio del sistema"
                    hint="todavía no disponible"
                    control={
                      <Switch
                        checked={false}
                        disabled
                        onChange={() => undefined}
                        label="Audio del sistema"
                      />
                    }
                  />
                </Section>
                <Section title="Al terminar">
                  <Row
                    icon={<SquarePen className="size-4" />}
                    label="Abrir el editor al terminar"
                    control={
                      <Switch
                        checked={settings.openEditorAfterRecording}
                        onChange={(v) => patch({ openEditorAfterRecording: v })}
                        label="Abrir el editor al terminar"
                      />
                    }
                  />
                </Section>
              </>
            )}

            {seccion === "teclas" && (
              <>
                <Section title="Las teclas de captura de Windows" tour="las-teclas">
                  <Row
                    icon={<Keyboard className="size-4" />}
                    label="Impr Pant"
                    hint={
                      imprPant?.enabled
                        ? imprPant.active
                          ? "ya es de winshotx"
                          : "cierra sesión para que Windows la suelte"
                        : "hoy abre la Herramienta de Recortes"
                    }
                    tone={imprPant?.enabled ? (imprPant.active ? "ok" : "warn") : "normal"}
                    control={
                      <Switch
                        checked={imprPant?.enabled ?? false}
                        onChange={(v) => void cambiarImprPant(v)}
                        label="Usar también Impr Pant"
                      />
                    }
                  />
                  <Row
                    icon={<AppWindow className="size-4" />}
                    label="Win + Mayús + S"
                    hint={
                      !settings.takeWinShiftS
                        ? "cuesta Win+S, la búsqueda"
                        : aplicando
                          ? "reiniciando el Explorador…"
                          : shortcuts.winShiftS
                            ? "de winshotx, o pulsa Aplicar si Windows la sigue abriendo"
                            : "el escritorio todavía la tiene"
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
                            title="Reinicia el Explorador para que Windows suelte la tecla. La barra de tareas parpadea un segundo y no se cierra nada más."
                          >
                            {aplicando ? "Un momento…" : "Aplicar"}
                          </RowButton>
                        )}
                        <Switch
                          checked={settings.takeWinShiftS}
                          onChange={(v) => void cambiarWinShiftS(v)}
                          label="Quedarme con Win+Mayús+S"
                        />
                      </span>
                    }
                  />
                </Section>
                <Section title="La Herramienta de Recortes">
                  <Row
                    icon={<Scissors className="size-4" />}
                    label="Herramienta de Recortes"
                    hint={
                      recortes === "confirmar"
                        ? "vuelve desde la Microsoft Store"
                        : recortes === "quitando"
                          ? "quitándola…"
                          : recortes === "quitada"
                            ? "quitada, ya no abre nada"
                            : recortes === "ya no estaba"
                              ? "no estaba instalada"
                              : "quitarla es lo único que la calla del todo"
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
                          <RowButton onClick={() => void openWindowsApps()}>Ver cómo</RowButton>
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
                            ? "Sí, quitarla"
                            : recortes === "quitada" || recortes === "ya no estaba"
                              ? "Hecho"
                              : "Quitar"}
                        </RowButton>
                      </span>
                    }
                  />
                </Section>
              </>
            )}

            {seccion === "app" && (
              <>
                <Section title="Archivos" tour="archivos">
                  <Row
                    icon={<FolderOpen className="size-4" />}
                    label="Carpeta de destino"
                    hint={settings.saveDirectory}
                    control={
                      <span className="flex gap-1.5">
                        <RowButton onClick={() => void openFolder(settings.saveDirectory)}>
                          Abrir
                        </RowButton>
                        <RowButton
                          onClick={() =>
                            void pickDirectory().then((dir) => dir && patch({ saveDirectory: dir }))
                          }
                        >
                          Cambiar
                        </RowButton>
                      </span>
                    }
                  />
                  <Row
                    icon={<HardDrive className="size-4" />}
                    label="Caché de grabaciones"
                    hint={
                      cache.sessions === 0
                        ? "vacía"
                        : `${formatBytes(cache.bytes)} en ${cache.sessions} ${
                            cache.sessions === 1 ? "sesión" : "sesiones"
                          }`
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
                        Vaciar
                      </RowButton>
                    }
                  />
                </Section>
                <Section title="winshotx">
                  <UpdateRow version={VERSION} />
                  <Row
                    icon={<BookOpen className="size-4" />}
                    label="Bienvenida"
                    control={<RowButton onClick={onVerBienvenida}>Ver otra vez</RowButton>}
                  />
                  <Row
                    icon={<Compass className="size-4" />}
                    label="Tour de los ajustes"
                    hint="seis paradas, una por sección"
                    control={<RowButton onClick={() => setTour(true)}>Empezar</RowButton>}
                  />
                  <Row
                    icon={<Power className="size-4" />}
                    label="Arrancar con Windows"
                    control={
                      <Switch
                        checked={settings.startWithWindows}
                        onChange={(v) => patch({ startWithWindows: v })}
                        label="Arrancar con Windows"
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
