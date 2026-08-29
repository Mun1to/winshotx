import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Camera,
  FolderOpen,
  History,
  Power,
  RefreshCw,
  Rewind,
  Settings as SettingsIcon,
  Square,
  Video,
} from "lucide-react";
import {
  getSettings,
  resizeTrayMenu,
  setSettings,
  trayMenuAction,
  trayMenuState,
} from "../../lib/ipc";
import { partesDeAtajo } from "../../lib/teclas";
import { aplicarIdioma, useT } from "../../lib/i18n";
import { aplicarTema } from "../../lib/tema";
import { EVENTS, type TrayMenuAction, type TrayMenuState } from "../../lib/types";

/**
 * El menú del botón derecho de la bandeja, dibujado por winshotx.
 *
 * Un menú del sistema es una lista de textos: no sabe enseñar un interruptor, ni el atajo
 * de cada cosa, ni decir qué versión hay puesta. Aquí el anillo de los últimos segundos es
 * lo que es, un interruptor, en vez de una entrada que enciende y otra que apaga.
 *
 * La ventana se reutiliza entre aperturas, así que el estado se vuelve a pedir cada vez
 * que Rust avisa de que se ha abierto: el anillo puede haberse encendido desde los ajustes
 * y una grabación puede estar en marcha desde el atajo.
 */
export function TrayMenu() {
  const t = useT();
  const [estado, setEstado] = useState<TrayMenuState | null>(null);
  const tarjeta = useRef<HTMLDivElement>(null);

  const cargar = useCallback(async () => {
    const [suyo, ajustes] = await Promise.all([trayMenuState(), getSettings()]);
    // El tema y el idioma se aplican aquí también: esta ventana nace aparte y no hereda
    // lo que haya pintado la de ajustes.
    aplicarTema(ajustes.theme);
    aplicarIdioma(ajustes.language);
    setEstado(suyo);
  }, []);

  useEffect(() => {
    void cargar();
    const etiqueta = getCurrentWindow().label;
    const unlisten = listen(EVENTS.trayMenuOpened, () => void cargar(), { target: etiqueta });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [cargar]);

  // La ventana se ajusta a lo que mide el menú: con el anillo apagado hay una entrada
  // menos, y un alto fijo dejaría un trozo vacío debajo.
  useEffect(() => {
    const caja = tarjeta.current;
    if (!caja) return;
    const medir = () => void resizeTrayMenu(Math.ceil(caja.getBoundingClientRect().height));
    medir();
    const observador = new ResizeObserver(medir);
    observador.observe(caja);
    return () => observador.disconnect();
  }, [estado]);

  // Escape cierra, como cualquier menú.
  useEffect(() => {
    const alPulsar = (e: KeyboardEvent) => {
      if (e.key === "Escape") void getCurrentWindow().hide();
    };
    window.addEventListener("keydown", alPulsar);
    return () => window.removeEventListener("keydown", alPulsar);
  }, []);

  const hacer = (accion: TrayMenuAction) => void trayMenuAction(accion);

  /** El anillo se enciende por el mismo camino que su interruptor de los ajustes. */
  const alternarAnillo = async () => {
    const ajustes = await getSettings();
    await setSettings({ ...ajustes, replayEnabled: !ajustes.replayEnabled });
    await cargar();
  };

  // Sin tarjeta de relleno mientras carga: un rectángulo opaco del tamaño de antes
  // parpadearía en la pantalla antes de que llegue el contenido de verdad.
  if (!estado) return <div ref={tarjeta} />;

  return (
    <div ref={tarjeta} className="flex flex-col bg-flotante">
      <header className="flex items-center gap-2 border-b border-linea px-3 py-2.5">
        <Camera className="size-4 shrink-0 text-marca" />
        <span className="flex-1 text-[13px] font-bold tracking-tight text-titulo">winshotx</span>
        <span className="text-[10.5px] font-semibold text-tenue">{estado.version}</span>
      </header>

      <div className="p-1">
        <Interruptor
          icono={<History className="size-4" />}
          texto={t("Grabar siempre lo último")}
          puesto={estado.replay}
          onClick={() => void alternarAnillo()}
        />

        <Separador />

        <Entrada
          icono={<Camera className="size-4" />}
          texto={t("Capturar región")}
          atajo={estado.captureShortcut}
          onClick={() => hacer("capture")}
        />
        <Entrada
          icono={estado.recording ? <Square className="size-4" /> : <Video className="size-4" />}
          texto={estado.recording ? t("Parar") : t("Grabar región")}
          atajo={estado.recordShortcut}
          resalta={estado.recording}
          onClick={() => hacer("record")}
        />
        {/* Solo con el anillo encendido: rescatar sin nada grabado no puede hacer nada. */}
        {estado.replay && (
          <Entrada
            icono={<Rewind className="size-4" />}
            texto={t("Quedarme con lo último")}
            atajo={estado.replayShortcut}
            onClick={() => hacer("replay")}
          />
        )}

        <Separador />

        <Entrada
          icono={<FolderOpen className="size-4" />}
          texto={t("Abrir la carpeta")}
          onClick={() => hacer("folder")}
        />
        <Entrada
          icono={<SettingsIcon className="size-4" />}
          texto={t("Ajustes")}
          onClick={() => hacer("settings")}
        />
        <Entrada
          icono={<RefreshCw className="size-4" />}
          texto={t("Buscar actualizaciones")}
          onClick={() => hacer("update")}
        />

        <Separador />

        <Entrada
          icono={<Power className="size-4" />}
          texto={t("Salir")}
          onClick={() => hacer("quit")}
        />
      </div>
    </div>
  );
}

function Separador() {
  return <div className="my-1 h-px bg-linea" />;
}

/** Una entrada normal: icono, nombre y, si lo tiene, su atajo a la derecha. */
function Entrada({
  icono,
  texto,
  atajo,
  resalta = false,
  onClick,
}: {
  icono: ReactNode;
  texto: string;
  atajo?: string;
  resalta?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-start transition-colors hover:bg-realce ${
        resalta ? "text-red-300" : "text-texto"
      }`}
    >
      <span className="shrink-0 text-apagado">{icono}</span>
      <span className="flex-1 truncate text-[12.5px] font-medium">{texto}</span>
      {atajo && (
        <span className="shrink-0 text-[10.5px] tracking-wide text-tenue">
          {partesDeAtajo(atajo).join("+")}
        </span>
      )}
    </button>
  );
}

/** Y una que se queda puesta: la misma fila con el interruptor de la app a la derecha. */
function Interruptor({
  icono,
  texto,
  puesto,
  onClick,
}: {
  icono: ReactNode;
  texto: string;
  puesto: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      role="switch"
      aria-checked={puesto}
      className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-start text-texto transition-colors hover:bg-realce"
    >
      <span className={`shrink-0 ${puesto ? "text-marca" : "text-apagado"}`}>{icono}</span>
      <span className="flex-1 truncate text-[12.5px] font-medium">{texto}</span>
      <span
        className={`relative h-4 w-7 shrink-0 rounded-full transition-colors ${
          puesto ? "bg-marca" : "bg-apagador"
        }`}
      >
        <span
          className={`absolute top-0.5 size-3 rounded-full bg-white transition-[left] ${
            puesto ? "left-3.5" : "left-0.5"
          }`}
        />
      </span>
    </button>
  );
}
