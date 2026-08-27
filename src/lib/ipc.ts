import { invoke } from "@tauri-apps/api/core";
import type {
  CacheStats,
  ExportRequest,
  ExportResult,
  FrameMeta,
  OverlayPayload,
  Rect,
  RecordOptions,
  SessionInfo,
  PrintScreenState,
  Settings,
  ShortcutStatus,
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
export const freezeBytes = (monitorId: number) =>
  invoke<ArrayBuffer>("freeze_bytes", { monitorId });

export const cancelCapture = () => invoke<void>("cancel_capture");

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

export const pickDirectory = () => invoke<string | null>("pick_directory");

export const revealInExplorer = (path: string) =>
  invoke<void>("reveal_in_explorer", { path });

export const discardSession = (sessionId: string) =>
  invoke<void>("discard_session", { sessionId });

/** Cierto una sola vez, si este arranque viene de actualizar. Se consume al leerlo. */
export const justUpdated = () => invoke<boolean>("just_updated");

export const shortcutStatus = () => invoke<ShortcutStatus>("shortcut_status");

export const printScreenState = () => invoke<PrintScreenState>("print_screen_state");

/** Le quita la tecla Impr Pant a la Herramienta de Recortes, o se la devuelve. */
export const usePrintScreen = (enabled: boolean) =>
  invoke<PrintScreenState>("use_print_screen", { enabled });

export const cacheStats = () => invoke<CacheStats>("cache_stats");

export const clearCache = () => invoke<CacheStats>("clear_cache");

export const openFolder = (path: string) => invoke<void>("open_folder", { path });

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
