import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Camera,
  Check,
  Clipboard,
  FolderOpen,
  HardDrive,
  MousePointer2,
  Power,
  Search,
  Video,
  Volume2,
} from "lucide-react";
import {
  cacheStats,
  clearCache,
  getSettings,
  openFolder,
  pickDirectory,
  quitApp,
  setSettings,
  shortcutStatus,
} from "../../lib/ipc";
import { formatBytes } from "../../lib/format";
import { EVENTS, type CacheStats, type Settings, type ShortcutStatus } from "../../lib/types";
import { Segmented } from "../ui/Segmented";
import { Switch } from "../ui/Switch";
import { Row, Section } from "./Section";
import { UpdateRow } from "./UpdateRow";
import { ShortcutField } from "./ShortcutField";

/** La version se escribe una vez aqui y se usa en el pie y en el actualizador. */
const VERSION = "0.1.5";

const FPS_OPTIONS = [
  { value: 15, label: "15 fps" },
  { value: 30, label: "30 fps" },
  { value: 60, label: "60 fps" },
];

export function SettingsApp() {
  const [settings, setLocal] = useState<Settings | null>(null);
  const [shortcuts, setShortcuts] = useState<ShortcutStatus>({
    capture: true,
    record: true,
  });
  const [cache, setCache] = useState<CacheStats>({ bytes: 0, sessions: 0 });
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refrescar = useCallback(() => {
    void getSettings().then(setLocal);
    void shortcutStatus().then(setShortcuts);
    void cacheStats().then(setCache);
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

  const patch = useCallback((partial: Partial<Settings>) => {
    setError(null);
    setLocal((prev) => {
      if (!prev) return prev;
      const next = { ...prev, ...partial };
      void setSettings(next)
        .then(() => {
          setSaved(true);
          window.setTimeout(() => setSaved(false), 1200);
          return shortcutStatus().then(setShortcuts);
        })
        .catch((e) => setError(String(e)));
      return next;
    });
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
      <div className="grid flex-1 grid-cols-2 content-start items-start gap-x-4 gap-y-3 overflow-y-auto p-4">
        <div className="space-y-3">
          <Section title="Atajos globales">
            <Row
              icon={<Camera className="size-4" />}
              label="Capturar región"
              hint={
                shortcuts.capture
                  ? undefined
                  : "ocupado, púlsalo para cambiarlo"
              }
              control={
                <ShortcutField
                  value={settings.captureShortcut}
                  active={shortcuts.capture}
                  onChange={(v) => patch({ captureShortcut: v })}
                />
              }
            />
            <Row
              icon={<Video className="size-4" />}
              label="Grabar región"
              hint={
                shortcuts.record
                  ? "púlsalo otra vez para terminar"
                  : "ocupado, púlsalo para cambiarlo"
              }
              control={
                <ShortcutField
                  value={settings.recordShortcut}
                  active={shortcuts.record}
                  onChange={(v) => patch({ recordShortcut: v })}
                />
              }
            />
          </Section>

          <Section title="Captura">
            <Row
              icon={<Clipboard className="size-4" />}
              label="Copiar al portapapeles"
              hint="al guardar, deja además la imagen lista para pegar"
              control={
                <Switch
                  checked={settings.copyAfterCapture}
                  onChange={(v) => patch({ copyAfterCapture: v })}
                  label="Copiar al portapapeles"
                />
              }
            />
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
              icon={<Search className="size-4" />}
              label="Lupa de píxel"
              hint="zoom 6× con el color exacto bajo el cursor"
              control={
                <Switch
                  checked={settings.showMagnifier}
                  onChange={(v) => patch({ showMagnifier: v })}
                  label="Lupa de píxel"
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
        </div>

        <div className="space-y-3">
          <Section title="Grabación">
            <Row
              label="Fotogramas por segundo"
              hint={settings.fps >= 60 ? "más fluido, más disco" : undefined}
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
            <Row
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

          <Section title="Archivos">
            <Row
              icon={<FolderOpen className="size-4" />}
              label="Carpeta de destino"
              hint={settings.saveDirectory}
              control={
                <span className="flex gap-1">
                  <button
                    type="button"
                    onClick={() => void openFolder(settings.saveDirectory)}
                    className="rounded-md border border-white/10 px-2 py-1 text-[11px] text-neutral-300 transition-colors hover:bg-white/10 hover:text-white"
                  >
                    Abrir
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      void pickDirectory().then(
                        (dir) => dir && patch({ saveDirectory: dir }),
                      )
                    }
                    className="rounded-md border border-white/10 px-2 py-1 text-[11px] text-neutral-300 transition-colors hover:bg-white/10 hover:text-white"
                  >
                    Cambiar
                  </button>
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
                <button
                  type="button"
                  disabled={cache.sessions === 0}
                  onClick={() =>
                    void clearCache()
                      .then(setCache)
                      .catch((e) => setError(String(e)))
                  }
                  className="rounded-md border border-white/10 px-2 py-1 text-[11px] text-neutral-300 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-40"
                >
                  Vaciar
                </button>
              }
            />
          </Section>


          {error && (
            <p className="rounded-lg bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
              {error}
            </p>
          )}
        </div>

        <div className="col-span-2">
          <Section title="Sistema">
            <UpdateRow version={VERSION} />
            <Row
              icon={<Power className="size-4" />}
              label="Arrancar con Windows"
              hint="se abre en la bandeja, sin ventana"
              control={
                <Switch
                  checked={settings.startWithWindows}
                  onChange={(v) => patch({ startWithWindows: v })}
                  label="Arrancar con Windows"
                />
              }
            />
          </Section>
        </div>
      </div>

      <footer className="flex shrink-0 items-center justify-between border-t border-white/8 px-4 py-2">
        <span className="flex items-center gap-1.5 text-[11px] text-neutral-500">
          {saved ? (
            <>
              <Check className="size-3 text-emerald-400" />
              <span className="text-emerald-400">Guardado</span>
            </>
          ) : (
            `winshotx ${VERSION} · MIT`
          )}
        </span>
        <button
          type="button"
          onClick={() => void quitApp()}
          className="rounded-md px-2 py-1 text-[11px] text-neutral-400 transition-colors hover:bg-red-500/15 hover:text-red-300"
        >
          Salir de winshotx
        </button>
      </footer>
    </div>
  );
}
