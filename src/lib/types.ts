/** Contrato compartido con Rust. Todo en pixeles FISICOS del escritorio virtual. */

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface MonitorInfo {
  id: number;
  label: string;
  /** Origen del monitor dentro del escritorio virtual, en pixeles fisicos. */
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  isPrimary: boolean;
}

export interface WindowRect {
  title: string;
  rect: Rect;
}

/** Que pasa al soltar el raton sobre la region. Son los dos perfiles de trabajo. */
export type CaptureFlow = "toolbar" | "instant";

/** Con que se abrio el overlay: el mismo sirve para capturar y para grabar. */
export type OverlayIntent = "capture" | "record";

export interface OverlayPayload {
  monitor: MonitorInfo;
  /** Ruta absoluta del PNG congelado de este monitor. */
  freezePath: string;
  windows: WindowRect[];
  settings: Settings;
  intent: OverlayIntent;
}

export type StillAction = "copy" | "save" | "edit";

export interface StillResult {
  path: string | null;
  copied: boolean;
  width: number;
  height: number;
}

export type RecordFormat = "gif" | "video";

/** Lo que puede traer una sesion: una grabacion o una captura fija llevada al editor. */
export type SessionFormat = RecordFormat | "still";

export interface RecordOptions {
  format: RecordFormat;
  fps: number;
  captureCursor: boolean;
  audio: boolean;
}

export interface SessionInfo {
  id: string;
  region: Rect;
  fps: number;
  frameCount: number;
  durationMs: number;
  hasAudio: boolean;
  format: SessionFormat;
  /** MP4 escrito en streaming durante la grabacion, si lo hubo. */
  mp4Path: string | null;
}

export interface FrameMeta {
  index: number;
  timestampMs: number;
  durationMs: number;
  /** Ruta absoluta de la miniatura; el frontend la pasa por convertFileSrc. */
  thumbPath: string;
}

export interface RecordingTick {
  elapsedMs: number;
  frames: number;
  bytes: number;
  paused: boolean;
}

export type ExportFormat = "gif" | "mp4" | "png";
export type ExportEngine = "native" | "ffmpeg";

export interface ExportRequest {
  sessionId: string;
  format: ExportFormat;
  engine: ExportEngine;
  /** Rango de frames incluido, ambos extremos. */
  from: number;
  to: number;
  width: number;
  height: number;
  fps: number;
  /** 0..100; en GIF controla dithering y colores, en MP4 el bitrate. */
  quality: number;
  audio: boolean;
  loop: boolean;
  /** null = carpeta por defecto de los ajustes. */
  destination: string | null;
  copyToClipboard: boolean;
}

export interface ExportResult {
  path: string;
  bytes: number;
  copied: boolean;
  elapsedMs: number;
}

export interface ExportProgress {
  stage: "reading" | "palette" | "encoding" | "writing" | "done";
  done: number;
  total: number;
}

export interface Settings {
  captureShortcut: string;
  recordShortcut: string;
  saveDirectory: string;
  copyAfterCapture: boolean;
  openEditorAfterRecording: boolean;
  captureCursor: boolean;
  recordAudio: boolean;
  fps: number;
  playSound: boolean;
  showMagnifier: boolean;
  startWithWindows: boolean;
  captureFlow: CaptureFlow;
  printScreenCapture: boolean;
  onboarded: boolean;
  /** Interno: lo que valía el ajuste de Windows antes de quitarle la tecla. */
  snippingKeyRestore: number | null;
}

export interface ShortcutStatus {
  capture: boolean;
  record: boolean;
  printScreen: boolean;
  winShiftS: boolean;
}

export interface PrintScreenState {
  /** winshotx ha pedido la tecla. */
  enabled: boolean;
  /** Y Windows se la ha dado de verdad. */
  active: boolean;
  /** La Herramienta de Recortes la sigue teniendo asignada. */
  takenByWindows: boolean;
  /** Win+Mayús+S también ha caído del lado de winshotx. */
  winShiftS: boolean;
}

export interface CacheStats {
  bytes: number;
  sessions: number;
}

export const EVENTS = {
  overlayShow: "winshotx://overlay-show",
  overlayHide: "winshotx://overlay-hide",
  recordingTick: "winshotx://recording-tick",
  recordingStopped: "winshotx://recording-stopped",
  sessionReady: "winshotx://session-ready",
  exportProgress: "winshotx://export-progress",
  settingsChanged: "winshotx://settings-changed",
  /** Lo manda la bandeja cuando se pide mirar si hay version nueva. */
  checkUpdate: "winshotx://check-update",
  /** La ventana de ajustes vuelve a estar a la vista: toca refrescar lo de dentro. */
  settingsShown: "winshotx://settings-shown",
} as const;
