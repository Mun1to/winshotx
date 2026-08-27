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

/** De que color se pinta la app. "sistema" es seguir a Windows y cambiar con el. */
export type Theme = "sistema" | "claro" | "oscuro";

/** En que idioma habla la app. "sistema" es el de Windows, si winshotx lo habla. */
export type Language = "sistema" | "es" | "en";

/** Con que se abrio el overlay: el mismo sirve para capturar y para grabar. */
export type OverlayIntent = "capture" | "record";

/** Que se hara con el recorte. Se elige en la barra de arriba, antes de recortar. */
export type CaptureMode = "still" | "video" | "gif";

export interface OverlayPayload {
  monitor: MonitorInfo;
  /** Ruta absoluta del PNG congelado de este monitor. */
  freezePath: string;
  windows: WindowRect[];
  settings: Settings;
  intent: OverlayIntent;
  /** Que numero de pantalla es esta, empezando por 1. */
  screenNumber: number;
  /** Cuantas pantallas hay en total. Con una sola no hace falta numerarlas. */
  screenCount: number;
  /**
   * La ultima region capturada, en coordenadas del escritorio virtual, o `null` si
   * todavia no se ha capturado nada desde que arranco la app. Viene en coordenadas
   * globales porque puede ser de otro monitor: cada overlay mira si es suya.
   */
  lastRegion: Rect | null;
}

export type StillAction = "copy" | "save" | "edit" | "pin" | "text";

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
  /** La voz por el micrófono. Con el audio del sistema puesto, van mezclados. */
  microphone: boolean;
  /** Marcar cada clic con un aro dentro del vídeo. */
  highlightClicks: boolean;
  /** Enseñar los atajos que se pulsan. Solo atajos, nunca una tecla suelta. */
  highlightKeys: boolean;
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
  /** Píxeles de aire alrededor de la captura. 0 es sin marco. */
  margin: number;
  /** El fondo de ese aire: blanco, negro, gris, atardecer o menta. */
  background: Background;
  /** Si la captura lleva sombra sobre el fondo. */
  shadow: boolean;
  /** null = carpeta por defecto de los ajustes. */
  destination: string | null;
  copyToClipboard: boolean;
}

/** Los fondos del marco. Unos pocos elegidos, no una rueda de color entera. */
export type Background = "blanco" | "negro" | "gris" | "atardecer" | "menta";

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
  /** Claro, oscuro, o lo que diga Windows. */
  theme: Theme;
  /** Español, inglés, o el idioma de Windows. */
  language: Language;
  recordShortcut: string;
  saveDirectory: string;
  copyAfterCapture: boolean;
  openEditorAfterRecording: boolean;
  captureCursor: boolean;
  recordAudio: boolean;
  /** Grabar también la voz por el micrófono. */
  recordMicrophone: boolean;
  /** Marcar cada clic con un aro dentro del vídeo. */
  highlightClicks: boolean;
  /** Enseñar los atajos que se pulsan. Solo atajos, nunca una tecla suelta. */
  highlightKeys: boolean;
  fps: number;
  playSound: boolean;
  showMagnifier: boolean;
  startWithWindows: boolean;
  /** Segundos de espera antes de congelar la pantalla. 0 es sin espera. */
  captureDelaySeconds: number;
  /** Esconder los iconos del escritorio mientras se congela, y devolverlos después. */
  hideDesktopIcons: boolean;
  captureFlow: CaptureFlow;
  printScreenCapture: boolean;
  takeWinShiftS: boolean;
  onboarded: boolean;
  /** Interno: lo que valía el ajuste de Windows antes de quitarle la tecla. */
  snippingKeyRestore: number | null;
  /** Interno: y las letras que el shell tenía apagadas antes de tocar nada. */
  disabledHotkeysRestore: string | null;
}

export interface ShortcutStatus {
  capture: boolean;
  record: boolean;
  printScreen: boolean;
  /** Si el escritorio ha soltado ya Win+Mayús+S. Se puede pedir y no conseguir. */
  winShiftS: boolean;
}

export interface PrintScreenState {
  /** winshotx ha pedido la tecla. */
  enabled: boolean;
  /** Y Windows se la ha dado de verdad. */
  active: boolean;
  /** La Herramienta de Recortes la sigue teniendo asignada. */
  takenByWindows: boolean;
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
  /**
   * Lo que hay elegido en la barra del overlay.
   *
   * Hay un overlay por monitor, cada uno con su propio React y su propia barra, y las
   * teclas solo llegan al que tiene el foco. Sin esto, pulsar "pantalla entera" numeraba
   * una sola pantalla y las otras dos se quedaban como si nada.
   */
  overlayMode: "winshotx://overlay-mode",
  /** "Que la pantalla numero N se capture entera", venga la orden de donde venga. */
  overlayTakeScreen: "winshotx://overlay-take-screen",
  /**
   * Segundos que le quedan al temporizador. Rust manda el numero de partida al
   * ensennar la ventanita y un cero al acabar; entre medias cuenta la propia pagina.
   */
  countdown: "winshotx://countdown",
} as const;

/** Lo que viaja en `overlayMode`: el estado de la barra, igual en todas las pantallas. */
export interface OverlayModeState {
  mode: CaptureMode;
  fullScreen: boolean;
}
