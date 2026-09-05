import { invoke } from "@tauri-apps/api/core";
import type {
  CacheStats,
  CaptureFlow,
  ExportRequest,
  ExportResult,
  FrameMeta,
  OverlayPayload,
  Rect,
  RecordOptions,
  ReplayStatus,
  Screen,
  SessionInfo,
  PrintScreenState,
  Settings,
  ShortcutStatus,
  TrayMenuAction,
  TrayMenuState,
  StillAction,
  StillResult,
} from "./types";

/** Unica puerta hacia Rust: si un comando cambia de nombre, se cambia aqui. */

export const overlayBootstrap = (monitorId: number) =>
  invoke<OverlayPayload>("overlay_bootstrap", { monitorId });

export const captureStill = (region: Rect, action: StillAction) =>
  invoke<StillResult>("capture_still", { region, action });

/** Todas las pantallas en una sola imagen, cada una en su sitio real. */
export const captureAllScreens = (action: StillAction) =>
  invoke<StillResult>("capture_all_screens", { action });

/** Respaldo del fondo del overlay cuando el protocolo asset no responde. */
/** La pantalla congelada de este monitor, en PNG. Es el camino normal. */
export const freezePng = (monitorId: number) =>
  invoke<ArrayBuffer>("freeze_png", { monitorId });

/** Y sin comprimir, en BMP, si el PNG fallara. */
export const freezeBytes = (monitorId: number) =>
  invoke<ArrayBuffer>("freeze_bytes", { monitorId });

/**
 * Este overlay ya tiene la imagen pintada: que Rust lo ensenne. Hasta entonces la ventana
 * sigue aparcada fuera de las pantallas y nadie ve una pantalla de carga.
 */
export const overlayListo = (monitorId: number, generation: number) =>
  invoke<void>("overlay_listo", { monitorId, generation }).catch(() => undefined);

export const cancelCapture = () => invoke<void>("cancel_capture");

/**
 * Una marca del cronometro del camino del atajo. Sin `--crono` en Rust no hace nada, y si
 * el puente falla tampoco: medir no puede romper lo que se mide.
 */
export const cronoMarca = (etapa: string) =>
  invoke<void>("crono_marca", { etapa }).catch(() => undefined);

/** Copia al portapapeles el color que hay bajo el cursor, en `#rrggbb`. */
export const copyColor = (color: string) => invoke<void>("copy_color", { color });

/** Copia al portapapeles la imagen de una captura anclada. */
export const copyPinned = (path: string) => invoke<void>("copy_pinned", { path });

/** Guarda en la carpeta de capturas una que estaba anclada. Devuelve dónde la ha dejado. */
export const savePinned = (path: string) => invoke<string>("save_pinned", { path });

/** Copia al portapapeles el texto que haya dentro de una captura anclada. */
export const pinnedText = (path: string) => invoke<void>("pinned_text", { path });

export const startRecording = (region: Rect, options: RecordOptions) =>
  invoke<SessionInfo>("start_recording", { region, options });

export const stopRecording = () => invoke<SessionInfo>("stop_recording");

export const pauseRecording = (paused: boolean) =>
  invoke<void>("pause_recording", { paused });

export const cancelRecording = () => invoke<void>("cancel_recording");

export const sessionInfo = (sessionId: string) =>
  invoke<SessionInfo>("session_info", { sessionId });

export const sessionFrames = (sessionId: string) =>
  invoke<FrameMeta[]>("session_frames", { sessionId });

/** Extrae un frame concreto como PNG y devuelve su ruta absoluta. */
export const frameImage = (sessionId: string, index: number) =>
  invoke<string>("frame_image", { sessionId, index });

export const exportMedia = (request: ExportRequest) =>
  invoke<ExportResult>("export_media", { request });

export const ffmpegAvailable = () => invoke<boolean>("ffmpeg_available");

export const getSettings = () => invoke<Settings>("get_settings");

export const setSettings = (settings: Settings) =>
  invoke<Settings>("set_settings", { settings });

/**
 * Solo el interruptor de la barra de acciones, guardado desde el overlay.
 *
 * No es `setSettings` con un campo cambiado a proposito: aquel reengancha los atajos
 * globales y el anillo en cada llamada, y esto se pulsa con la captura abierta.
 */
export const setCaptureFlow = (flow: CaptureFlow) =>
  invoke<void>("set_capture_flow", { flow });

/** Cierra la captura y abre la ventana de ajustes. */
export const openSettings = () => invoke<void>("open_settings");

export const pickDirectory = () => invoke<string | null>("pick_directory");

export const revealInExplorer = (path: string) =>
  invoke<void>("reveal_in_explorer", { path });

export const discardSession = (sessionId: string) =>
  invoke<void>("discard_session", { sessionId });

/** Cierto una sola vez, si este arranque viene de actualizar. Se consume al leerlo. */
export const justUpdated = () => invoke<boolean>("just_updated");
/** Cierto si winshotx viene de la Microsoft Store, que se actualiza sola. */
export const isStoreBuild = () => invoke<boolean>("is_store_build");

export const shortcutStatus = () => invoke<ShortcutStatus>("shortcut_status");

export const printScreenState = () => invoke<PrintScreenState>("print_screen_state");

/** Las pantallas que hay, para elegir cuál vigila el anillo. No captura nada. */
export const listScreens = () => invoke<Screen[]>("list_screens");

/** Enseña el número de esa pantalla, en esa pantalla, un par de segundos. */
export const showScreenNumber = (screen: number) =>
  invoke<void>("show_screen_number", { screen });

/** Cómo va el anillo de los últimos segundos. */
export const replayStatus = () => invoke<ReplayStatus>("replay_status");

/** Quedarse con lo último que pasó. Lo mismo que hace la tecla. */
export const replaySave = () => invoke<void>("replay_save");

/** Le quita la tecla Impr Pant a la Herramienta de Recortes, o se la devuelve. */
export const usePrintScreen = (enabled: boolean) =>
  invoke<PrintScreenState>("use_print_screen", { enabled });

export const cacheStats = () => invoke<CacheStats>("cache_stats");

export const clearCache = () => invoke<CacheStats>("clear_cache");

export const openFolder = (path: string) => invoke<void>("open_folder", { path });

/** Abre en el navegador uno de los enlaces de `lib/enlaces.ts`. Rust rechaza los demás. */
export const openUrl = (url: string) => invoke<void>("open_url", { url });

/** Lleva a la lista de aplicaciones de Windows, donde se quita la Herramienta de Recortes. */
export const openWindowsApps = () => invoke<void>("open_windows_apps");

/** Quita la Herramienta de Recortes de este usuario. Devuelve si había algo que quitar. */
export const removeSnippingTool = () => invoke<boolean>("remove_snipping_tool");

/** Le quita Win+Mayús+S al escritorio, a cambio de perder Win+S. */
export const useWinShiftS = (enabled: boolean) =>
  invoke<boolean>("use_win_shift_s", { enabled });

/** Hace que el escritorio relea la lista de teclas apagadas, sin cerrar sesión. */
export const restartShell = () => invoke<ShortcutStatus>("restart_shell");

export const quitApp = () => invoke<void>("quit_app");

/** Lo que el menú de la bandeja necesita para pintarse: versión, atajos y qué corre. */
export const trayMenuState = () => invoke<TrayMenuState>("tray_menu_state");

/** Una entrada del menú, pulsada. Rust esconde el menú y hace lo que toque. */
export const trayMenuAction = (action: TrayMenuAction) =>
  invoke<void>("tray_menu_action", { action });

/** El alto que ha medido el menú, para que su ventana se ajuste al contenido. */
export const resizeTrayMenu = (height: number) =>
  invoke<void>("resize_tray_menu", { height });
