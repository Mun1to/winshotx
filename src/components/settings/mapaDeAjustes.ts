import {
  Camera,
  History,
  ImageDown,
  Images,
  Keyboard,
  LifeBuoy,
  SlidersHorizontal,
  Video,
  type LucideIcon,
} from "lucide-react";

/**
 * Los ocho apartados de los ajustes, en el orden del riel de la izquierda (S36, la
 * variación 9 que eligió Munito el 15-nov-2026).
 *
 * Antes eran cinco pestañas arriba con dos columnas de tarjetas cada una; ahora es un
 * apartado a la vez, con su título, su frase y sus filas en medio, y a la derecha el dibujo
 * de cómo va a salir. Por eso Capturar se parte en dos (lo que pasa al pulsar y lo que pasa
 * con la foto), Grabar en dos (grabar y los últimos segundos), y La app en dos (la app y la
 * ayuda): cada uno cabe entero sin rueda y su dibujo cuenta una sola cosa.
 */
export type ApartadoId =
  | "capturar"
  | "despues"
  | "grabar"
  | "ultimos"
  | "historial"
  | "teclas"
  | "app"
  | "ayuda";

export interface Apartado {
  id: ApartadoId;
  /** El nombre, que es también lo que dice el icono al dejar el ratón encima. */
  nombre: string;
  icono: LucideIcon;
  /** Una frase debajo del título. El historial no lleva: es una galería, no una lista de ajustes. */
  frase: string;
  /** Si lleva el dibujo de «Así va a salir» a la derecha. El historial y la ayuda, no. */
  dibujo: boolean;
}

export const APARTADOS: Apartado[] = [
  {
    id: "capturar",
    nombre: "Capturar",
    icono: Camera,
    frase: "Los atajos y lo que pasa al pulsarlos.",
    dibujo: true,
  },
  {
    id: "despues",
    nombre: "Después de capturar",
    icono: ImageDown,
    frase: "Qué se hace con la foto en cuanto la tienes.",
    dibujo: true,
  },
  {
    id: "grabar",
    nombre: "Grabar",
    icono: Video,
    frase: "El atajo, la fluidez, el sonido y la cámara.",
    dibujo: true,
  },
  {
    id: "ultimos",
    nombre: "Los últimos segundos",
    icono: History,
    frase: "Graba siempre en memoria y te quedas con lo último cuando pasa algo.",
    dibujo: true,
  },
  { id: "historial", nombre: "Historial", icono: Images, frase: "", dibujo: false },
  {
    id: "teclas",
    nombre: "Teclas de Windows",
    icono: Keyboard,
    frase: "Qué teclas del sistema abren winshotx en vez de Windows.",
    dibujo: true,
  },
  {
    id: "app",
    nombre: "La app",
    icono: SlidersHorizontal,
    frase: "Dónde guarda, cómo se ve y cuándo arranca.",
    dibujo: true,
  },
  {
    id: "ayuda",
    nombre: "Ayuda y winshotx",
    icono: LifeBuoy,
    frase: "La bienvenida, el tour y de dónde sale winshotx.",
    dibujo: false,
  },
];

export const apartado = (id: ApartadoId): Apartado => APARTADOS.find((a) => a.id === id) ?? APARTADOS[0];

/**
 * Cada ajuste que encuentra el buscador: su fila (`data-fila` en `Apartados.tsx`), su
 * apartado, su nombre (el de la fila, que se traduce con `t`) y las palabras con las que
 * alguien lo buscaría, en los tres idiomas.
 *
 * Las palabras van aquí y no en los diccionarios porque no se enseñan: son para encontrar.
 * Se busca en las tres a la vez, porque quien tiene la app en inglés puede escribir
 * «carpeta» y al revés, y el nombre de la fila entra solo, también en los tres.
 */
export interface AjusteBuscable {
  id: string;
  apartado: ApartadoId;
  nombre: string;
  /** S21 (Linux): el nombre de la fila allí, si no es el mismo. */
  nombreEnLinux?: string;
  palabras: string;
}

export const BUSCABLES: AjusteBuscable[] = [
  { id: "capRegion", apartado: "capturar", nombre: "Capturar región", palabras: "atajo tecla zona recortar foto pantalla captura shortcut key area crop screenshot 快捷键 区域 截屏" },
  { id: "capEditor", apartado: "capturar", nombre: "Capturar y abrir el editor", palabras: "atajo editor editar flecha tapar shortcut edit arrow annotate 编辑器 编辑 标注" },
  { id: "capEsperar", apartado: "capturar", nombre: "Esperar antes de capturar", palabras: "espera retraso temporizador segundos cuenta atrás menú delay timer countdown seconds menu 延迟 倒计时 定时" },
  { id: "capIconos", apartado: "capturar", nombre: "Ocultar iconos del escritorio", palabras: "iconos escritorio esconder desktop icons hide 桌面 图标 隐藏" },
  { id: "capCursor", apartado: "capturar", nombre: "Incluir el cursor", palabras: "cursor ratón puntero mouse pointer 鼠标 指针 光标" },
  { id: "capLupa", apartado: "capturar", nombre: "Lupa de píxel", palabras: "lupa aumento píxel color hexadecimal zoom magnifier loupe pixel colour hex 放大镜 像素 颜色" },
  { id: "desSoltar", apartado: "despues", nombre: "Al soltar el ratón", palabras: "soltar barra copiar portapapeles release toolbar bar copy clipboard 工具栏 松开 剪贴板" },
  { id: "desCopiar", apartado: "despues", nombre: "Copiar al guardar", palabras: "copiar guardar portapapeles pegar copy save clipboard paste 复制 保存 剪贴板 粘贴" },
  { id: "desMiniatura", apartado: "despues", nombre: "Miniatura en la esquina", palabras: "miniatura esquina vista previa arrastrar thumbnail corner preview drag 缩略图 角落 预览 拖动" },
  { id: "desSonido", apartado: "despues", nombre: "Sonido de obturador", palabras: "sonido obturador ruido clic sound shutter click noise 声音 快门" },
  { id: "desFondo", apartado: "despues", nombre: "Fondo en las capturas", palabras: "fondo marco sombra esquinas ventana portátil estilo background frame shadow corners window laptop style 背景 边框 阴影 圆角 窗口 笔记本" },
  { id: "graRegion", apartado: "grabar", nombre: "Grabar región", palabras: "grabar vídeo atajo gif record video shortcut 录屏 视频 录制" },
  { id: "graZoom", apartado: "grabar", nombre: "Tecla de zoom", palabras: "zoom acercar tecla key closer 缩放 放大" },
  { id: "graFps", apartado: "grabar", nombre: "Fotogramas por segundo", palabras: "fotogramas fps fluidez frames smooth 帧率 流畅" },
  { id: "graEditor", apartado: "grabar", nombre: "Abrir el editor al terminar", palabras: "editor terminar abrir stop open 编辑器 结束" },
  { id: "graEditorExterno", apartado: "grabar", nombre: "Editor de vídeo externo", palabras: "editor externo ejecutable argumentos external video editor executable arguments 外部 视频 编辑器 参数" },
  { id: "graAudio", apartado: "grabar", nombre: "Audio del sistema", palabras: "audio sonido sistema pc altavoces sound system speakers 系统 声音 音频 扬声器" },
  { id: "graMicro", apartado: "grabar", nombre: "Micrófono", palabras: "micrófono micro voz microphone mic voice 麦克风 话筒" },
  { id: "graSonido", apartado: "grabar", nombre: "Sonido al empezar", palabras: "sonido empezar aviso marimba sound start beep 开始 提示音" },
  { id: "graCamara", apartado: "grabar", nombre: "Grabar también la cámara", palabras: "cámara webcam cara camera face 摄像头 相机" },
  { id: "ultSiempre", apartado: "ultimos", nombre: "Grabar siempre lo último", palabras: "últimos segundos siempre anillo repetición last seconds always replay 最近 回放 一直" },
  { id: "ultPantalla", apartado: "ultimos", nombre: "Qué pantalla", palabras: "pantalla monitor screen display 屏幕 显示器" },
  { id: "ultQuedarme", apartado: "ultimos", nombre: "Quedarme con lo último", palabras: "quedarme rescatar guardar atajo keep rescue save shortcut 保留 保存 快捷键" },
  { id: "ultCuanto", apartado: "ultimos", nombre: "Cuánto se guarda", palabras: "cuánto segundos duración how long seconds length 时长 秒" },
  { id: "ultCalidad", apartado: "ultimos", nombre: "Calidad", palabras: "calidad resolución tamaño disco quality resolution size disk 画质 分辨率 磁盘" },
  { id: "ultFluidez", apartado: "ultimos", nombre: "Fluidez", palabras: "fluidez fps fotogramas smoothness frames 流畅度 帧率" },
  { id: "historial", apartado: "historial", nombre: "Historial", palabras: "historial capturas grabaciones galería history captures recordings gallery 历史 截图 录屏" },
  { id: "tecRecortesWindows", apartado: "teclas", nombre: "Win+Mayús+S y Win+Mayús+R", palabras: "recortes ms-screenclip aplicaciones predeterminadas windows native hotkeys shortcuts startup 原生 快捷键 截图 录屏" },
  { id: "tecImpr", apartado: "teclas", nombre: "Impr Pant", palabras: "impr pant imprimir pantalla tecla print screen key prtsc 截屏键 按键" },
  { id: "tecWinS", apartado: "teclas", nombre: "Win + Mayús + S", palabras: "win mayús s tecla recortes shift key snipping 按键 截图工具" },
  { id: "tecRecortes", apartado: "teclas", nombre: "Herramienta de Recortes", palabras: "herramienta recortes quitar desinstalar snipping tool remove uninstall 截图工具 卸载 移除" },
  { id: "appCarpeta", apartado: "app", nombre: "Carpeta de destino", palabras: "carpeta destino dónde guarda archivos folder destination where save files 文件夹 保存 位置" },
  { id: "appCache", apartado: "app", nombre: "Caché de grabaciones", palabras: "caché espacio borrar vaciar cache space clear empty 缓存 空间 清空" },
  { id: "appArrancar", apartado: "app", nombre: "Arrancar con Windows", nombreEnLinux: "Arrancar al iniciar sesión", palabras: "arrancar inicio encender sesión start startup boot login 开机 启动 登录" },
  { id: "appTema", apartado: "app", nombre: "Tema", palabras: "tema claro oscuro color theme light dark colour 主题 浅色 深色" },
  { id: "appIdioma", apartado: "app", nombre: "Idioma", palabras: "idioma inglés español chino language english spanish chinese 语言 英语 西班牙语 中文" },
  { id: "appBienvenida", apartado: "ayuda", nombre: "Bienvenida", palabras: "bienvenida primer día welcome first day onboarding 欢迎" },
  { id: "appTour", apartado: "ayuda", nombre: "Tour de los ajustes", palabras: "tour recorrido ayuda guía guide help 导览 帮助 指南" },
  { id: "appVersion", apartado: "ayuda", nombre: "winshotx {version}", palabras: "versión actualizar actualización web version update website 版本 更新 网站" },
  { id: "appCafe", apartado: "ayuda", nombre: "Invítame a un café", palabras: "café apoyar donar coffee support donate 咖啡 支持 捐赠" },
  { id: "appCodigo", apartado: "ayuda", nombre: "El código y los fallos", palabras: "código fallos github licencia error code bugs license issue 代码 错误 许可" },
];

/** En minúsculas y sin tildes: «cámara» y «camara» tienen que encontrar lo mismo. */
export const normalizar = (x: string) =>
  x
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase();

/**
 * Lo que encuentra el buscador para lo escrito, en el orden del riel, y primero lo que lo
 * lleva en el nombre (quien escribe «sonido» busca antes «Sonido de obturador» que el
 * micrófono). Cada palabra escrita tiene que estar, en cualquier orden.
 *
 * `nombres` da el nombre de cada fila en los tres idiomas (el de la pantalla, el primero),
 * y `hay` deja fuera las filas que hoy no salen (elegir pantalla con una sola).
 */
export function buscarAjustes(
  consulta: string,
  nombres: (b: AjusteBuscable) => string[],
  hay: (id: string) => boolean = () => true,
): AjusteBuscable[] {
  const palabras = normalizar(consulta).split(/\s+/).filter(Boolean);
  if (palabras.length === 0) return [];
  const encontrados = BUSCABLES.filter((b) => hay(b.id)).flatMap((b) => {
    const enNombre = normalizar(nombres(b).join(" "));
    const todo = `${enNombre} ${normalizar(b.palabras)}`;
    if (!palabras.every((p) => todo.includes(p))) return [];
    return [{ b, primero: palabras.every((p) => enNombre.includes(p)) ? 0 : 1 }];
  });
  return encontrados.sort((x, y) => x.primero - y.primero).map((x) => x.b);
}
