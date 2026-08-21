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

/** Respaldo del fondo del overlay cuando el protocolo asset no responde. */
export const freezeBytes = (monitorId: number) =>
  invoke<ArrayBuffer>("freeze_bytes", { monitorId });

export const cancelCapture = () => invoke<void>("cancel_capture");

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

export const shortcutStatus = () => invoke<ShortcutStatus>("shortcut_status");

export const cacheStats = () => invoke<CacheStats>("cache_stats");

export const clearCache = () => invoke<CacheStats>("clear_cache");

export const openFolder = (path: string) => invoke<void>("open_folder", { path });

export const quitApp = () => invoke<void>("quit_app");
