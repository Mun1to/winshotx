import { invoke } from "@tauri-apps/api/core";
import type {
  CacheStats,
  CapturaReciente,
  CaptureFlow,
  ExportRequest,
  ExportResult,
  FrameMeta,
  MuestraCamara,
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
  StudioData,
} from "./types";
import type { Anotacion } from "./anotaciones";
import type { Recorte } from "./recorte";

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
 * Si las fotos van por la captura moderna de Windows o por la de reserva.
 *
 * En un Windows que no la tenga, las ventanas que compone la tarjeta grafica salen con
 * trozos en negro y no hay ajuste que lo cambie: lo unico que se puede hacer es decirlo,
 * para que quien lo vea sepa que es su Windows y no la app.
 */
export const capturaModerna = () => invoke<boolean>("captura_moderna");

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

/**
 * Vuelve enseguida: lo que tarda (miniaturas, editor o archivo) pasa por detrás en Rust, y
 * la barra se entera por el último tick, que llega con `saving`.
 */
export const stopRecording = () => invoke<void>("stop_recording");

export const pauseRecording = (paused: boolean) =>
  invoke<void>("pause_recording", { paused });

export const cancelRecording = () => invoke<void>("cancel_recording");

export const sessionInfo = (sessionId: string) =>
  invoke<SessionInfo>("session_info", { sessionId });

export const sessionFrames = (sessionId: string) =>
  invoke<FrameMeta[]>("session_frames", { sessionId });

/** Los clics, atajos y rastro del ratón anotados al grabar, para la vista previa. */
export const sessionStudio = (sessionId: string) =>
  invoke<StudioData>("session_studio", { sessionId });

/** La cámara del zoom en cada fotograma, con ese zoom y ese recorte. */
export const sessionCamera = (
  sessionId: string,
  zoom: number,
  crop: Recorte | null,
  zoomSpeed = 1,
) => invoke<MuestraCamara[]>("session_camera", { sessionId, zoom, crop, zoomSpeed });

/** Las últimas capturas guardadas, leídas de la carpeta, de la más nueva a la más vieja. */
export const ultimasCapturas = (cuantas: number) =>
  invoke<CapturaReciente[]>("ultimas_capturas", { cuantas });

/** Vuelve a dejar en el portapapeles una captura que ya estaba guardada. */
export const copiarCaptura = (path: string) => invoke<void>("copiar_captura", { path });

/**
 * Copia el texto que hay escrito en el fotograma, leído por el motor de Windows.
 *
 * Devuelve lo copiado para poder decir cuánto era: un aviso que dice «copiado» sin más no
 * distingue entre haber cogido un párrafo y haber cogido una palabra suelta mal leída.
 */
export const copiaTexto = (sessionId: string, index: number) =>
  invoke<string>("copiar_texto", { sessionId, index });

/**
 * Busca datos personales en el fotograma y devuelve las marcas que los taparían.
 *
 * No cambia nada por su cuenta: las marcas se añaden en el editor, donde se ven antes de
 * exportar y se quitan con Ctrl+Z. El mosaico borra los píxeles de verdad, así que taparlo
 * a la fuerza sería lo único de la aplicación que no se puede deshacer.
 */
export const tapaDatos = (sessionId: string, index: number) =>
  invoke<{ marcas: Anotacion[]; resumen: [string, number][] }>("tapar_datos", {
    sessionId,
    index,
  });

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

/** El diálogo de «Guardar como» de Windows: carpeta y nombre de una vez, o null si se cancela. */
export const pickSaveFile = (extension: string, directory?: string | null) =>
  invoke<string | null>("pick_save_file", { extension, directory: directory ?? null });

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

// Lo nuevo del plan de CleanShot X y Screen Studio, cada sesión debajo de su marca.
// S1 (línea de tiempo)
// S2 (grabación)
// S3 (fondos)
// S4 (capturas)
/** Una miniatura de la esquina, tal como la manda Rust: sin rutas que se puedan pedir de vuelta. */
export interface MiniaturaVista {
  id: number;
  /** La imagen pequeña que se enseña, en el temporal. */
  vista: string;
  /** Si ya está en la carpeta de capturas: entonces «Guardar» la enseña en el Explorador. */
  guardada: boolean;
}

/** La pila de miniaturas y cuántos segundos se queda cada una (0 = hasta cerrarla). */
export interface PilaMiniaturas {
  lista: MiniaturaVista[];
  segundos: number;
}

/** El aviso de Rust a la ventana de las miniaturas: la pila ha cambiado. */
export const EVENTO_MINIATURAS = "miniaturas-cambian";

export const miniaturasLista = () => invoke<PilaMiniaturas>("miniaturas_lista");
export const miniaturaCerrar = (id: number) => invoke<void>("miniatura_cerrar", { id });
export const miniaturaCopiar = (id: number) => invoke<void>("miniatura_copiar", { id });
/** La guarda en la carpeta de capturas, o la enseña en el Explorador si ya lo estaba. */
export const miniaturaGuardar = (id: number) => invoke<string>("miniatura_guardar", { id });
export const miniaturaEditar = (id: number) => invoke<void>("miniatura_editar", { id });
export const miniaturaAnclar = (id: number) => invoke<void>("miniatura_anclar", { id });
/** Empieza a arrastrar su archivo a otra app. Se llama con el botón del ratón pulsado. */
export const miniaturaArrastrar = (id: number) => invoke<void>("miniatura_arrastrar", { id });

/** Una foto o un vídeo de la carpeta de capturas. */
export interface EntradaHistorial {
  ruta: string;
  nombre: string;
  /** Un GIF es un vídeo: es una grabación. */
  tipo: "foto" | "video";
  bytes: number;
  /** Cuándo se guardó, en milisegundos desde 1970. */
  cuandoMs: number;
}

export interface PaginaHistorial {
  entradas: EntradaHistorial[];
  /** Cuántas hay con ese filtro, para saber si quedan más por cargar. */
  total: number;
}

export const historialLista = (filtro: "foto" | "video" | null, desde: number, cuantas: number) =>
  invoke<PaginaHistorial>("historial_lista", { filtro, desde, cuantas });
/** La miniatura del archivo, en el temporal: la da Windows, la misma del Explorador. */
export const historialMiniatura = (ruta: string) => invoke<string>("historial_miniatura", { ruta });
export const historialAbrir = (ruta: string) => invoke<void>("historial_abrir", { ruta });
export const historialMostrar = (ruta: string) => invoke<void>("historial_mostrar", { ruta });
export const historialCopiar = (ruta: string) => invoke<void>("historial_copiar", { ruta });
/** A la papelera de Windows, de donde se puede recuperar. */
export const historialBorrar = (ruta: string) => invoke<void>("historial_borrar", { ruta });
// S5 (scroll)
