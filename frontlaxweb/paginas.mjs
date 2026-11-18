/**
 * Las páginas que contestan a una búsqueda: tres por problema y una «alternativa a» por cada
 * programa de la comparativa de la portada.
 *
 * Aquí están las direcciones y los textos de las tres por problema. Las «alternativa a» no
 * llevan ningún dato escrito a mano: salen de comparativa.mjs en generar-paginas.mjs, igual
 * que la sección «Frente a frente» de la portada.
 *
 * Los atajos y los pasos están comprobados contra el código de la app (settings.rs y
 * SelectionCanvas.tsx) y contra docs/; lo que trae Windows, contra las páginas de ayuda de
 * Microsoft que cita comparativa.mjs. Las tablas de cada página no se escriben aquí: `filas`
 * dice qué filas de comparativa.mjs enseña y `rivales` frente a quién.
 */
import { HERRAMIENTAS, FILAS, TEXTOS } from "./comparativa.mjs";

/** Cuándo se tocaron estas páginas por última vez: es la fecha que lee el sitemap. */
export const ACTUALIZADO = "2026-11-18";

export const DESCARGA = "https://github.com/Mun1to/winshotx/releases/latest/download/winshotx-setup.exe";

const k = (...teclas) => teclas.map((t) => `<kbd>${t}</kbd>`).join("+");
const t = (es, en = es) => ({ es, en });

// El peso del instalador y la cifra medida salen de donde los saca la portada.
const pesa = FILAS.find((f) => f.id === "instalador").v.wx;
const [atajo] = TEXTOS.medidas;

/* ------------------------------------------------- lo que se repite entre las páginas */
const INSTALAR = t(
  `<b>Instala winshotx.</b> Pesa ${pesa[1]}, se instala solo para tu usuario y nunca sale la ventana de permisos de administrador.`,
  `<b>Install winshotx.</b> It weighs ${pesa[2]}, installs for your user only and never shows the administrator prompt.`,
);
const GRATIS = {
  q: t("¿Es gratis winshotx?", "Is winshotx free?"),
  a: t(
    "Sí, entero. Es código abierto bajo licencia MIT: el código completo está en GitHub, sin límite de tiempo ni funciones bloqueadas detrás de un pago.",
    "Yes, all of it. It's open source under the MIT licence: the full code is on GitHub, with no time limit and no feature locked behind a payment.",
  ),
};
const VERSIONES = {
  q: t("¿En qué versiones de Windows funciona?", "Which versions of Windows does it run on?"),
  a: t("Windows 10 (versión 1903 o superior) y Windows 11.", "Windows 10 (version 1903 or later) and Windows 11."),
};

/* ------------------------------------------------------------ las tres por problema */
export const PROBLEMAS = [
  {
    id: "grabar",
    es: "grabar-pantalla",
    en: "screen-recorder",
    pie: t("Grabar la pantalla", "Record your screen"),
    titulo: t("Cómo grabar la pantalla en Windows 10 y 11, con sonido", "How to record your screen on Windows 10 and 11, with sound"),
    descripcion: t(
      "Cómo grabar la pantalla en Windows con lo que ya trae el sistema, y qué añade winshotx: tu cámara encima, zooms a cada clic y datos personales tapados. Gratis.",
      "How to record your screen with what Windows already includes, and what winshotx adds: your webcam on top, zooms on every click and personal data covered.",
    ),
    h1: t("Cómo grabar la pantalla en Windows", "How to record your screen on Windows"),
    entradilla: t(
      `En Windows 11, pulsa ${k("Win", "Mayús", "R")} y la Herramienta de Recortes graba la zona que elijas, sin instalar nada. En un juego o una app también vale ${k("Win", "Alt", "R")}, de la Xbox Game Bar. Si quieres tu cámara encima, zooms a cada clic o tapar datos personales, winshotx lo hace gratis y sin cuenta.`,
      `On Windows 11, press ${k("Win", "Shift", "R")} and the Snipping Tool records the area you pick, with nothing to install. In a game or an app, ${k("Win", "Alt", "R")} from Xbox Game Bar works too. If you want your webcam on top, zooms on every click or personal data covered up, winshotx does it for free and with no account.`,
    ),
    secciones: [
      {
        id: "windows",
        indice: t("Con Windows", "With Windows"),
        h2: t("Lo que ya trae Windows", "What Windows already includes"),
        cuerpo: [
          { p: t("Para un vídeo corto de vez en cuando, con esto te basta:", "For a short video every now and then, this is enough:") },
          {
            lista: [
              t(
                `<b>Herramienta de Recortes, en Windows 11.</b> Pulsa ${k("Win", "Mayús", "R")}, elige la zona y dale a Iniciar. Para acabar, dale a Detener y guarda el vídeo.`,
                `<b>Snipping Tool, on Windows 11.</b> Press ${k("Win", "Shift", "R")}, pick the area and select Start. To finish, select Stop and save the video.`,
              ),
              t(
                `<b>Xbox Game Bar, en Windows 10 y 11.</b> Abre el juego o la app y pulsa ${k("Win", "Alt", "R")} para empezar, y otra vez para parar. Graba solo ese juego o esa app.`,
                `<b>Xbox Game Bar, on Windows 10 and 11.</b> Open the game or app and press ${k("Win", "Alt", "R")} to start, and again to stop. It records only that game or app.`,
              ),
            ],
          },
        ],
      },
      {
        id: "pasos",
        indice: t("Con winshotx", "With winshotx"),
        h2: t("Cómo se graba con winshotx", "How to record with winshotx"),
        foto: {
          img: "grabar",
          ancho: 1140,
          alto: 720,
          alt: t("Una zona elegida para grabar en vídeo, con su botón rojo de grabar", "An area picked to record as video, with its red record button"),
        },
        cuerpo: [
          {
            pasos: [
              INSTALAR,
              t(
                `<b>Pulsa ${k("Ctrl", "Shift", "5")} y arrastra sobre la zona.</b> Al soltar el ratón, empieza a grabar. Para grabar solo una ventana, deja el ratón encima y pulsa ${k("W")}.`,
                `<b>Press ${k("Ctrl", "Shift", "5")} and drag over the area.</b> When you let go of the mouse, it starts recording. To record just one window, leave the mouse over it and press ${k("W")}.`,
              ),
              t(
                `<b>Pulsa ${k("Ctrl", "Shift", "5")} otra vez para parar.</b> Se abre el editor con la grabación entera.`,
                `<b>Press ${k("Ctrl", "Shift", "5")} again to stop.</b> The editor opens with the whole recording.`,
              ),
              t(
                `<b>Recorta y guarda.</b> Arrastra los bordes azules de la línea de tiempo y pulsa ${k("Ctrl", "S")} para sacar el MP4.`,
                `<b>Trim and save.</b> Drag the blue edges of the timeline and press ${k("Ctrl", "S")} to export the MP4.`,
              ),
            ],
          },
          {
            aparte: t(
              `<b>¿Se te ha pasado?</b> Con el anillo encendido, ${k("Ctrl", "Shift", "6")} se queda con los últimos 15, 30 o 60 segundos sin haber dado a grabar antes.`,
              `<b>Missed it?</b> With the ring on, ${k("Ctrl", "Shift", "6")} keeps the last 15, 30 or 60 seconds without having pressed record beforehand.`,
            ),
          },
        ],
      },
      {
        id: "comparar",
        indice: t("Comparar", "Compare"),
        h2: t("Lo que añade winshotx", "What winshotx adds"),
        cuerpo: [
          { p: t("Las tres, fila a fila, y también lo que gana Windows:", "The three of them, row by row, including where Windows wins:") },
          { tabla: { filas: ["mp4", "audio", "camara", "estudio", "taparVideo", "ultimos", "puesta"], rivales: ["recortes", "gamebar"] } },
          { otras: ["obs", "gamebar", "sharex"] },
        ],
      },
    ],
    preguntas: [
      {
        q: t("¿Cómo grabo la pantalla en Windows 11 sin instalar nada?", "How do I record my screen on Windows 11 without installing anything?"),
        a: t(
          `Con la Herramienta de Recortes: pulsa ${k("Win", "Mayús", "R")}, elige la zona y dale a Iniciar. Para acabar, dale a Detener y guarda el vídeo.`,
          `With the Snipping Tool: press ${k("Win", "Shift", "R")}, pick the area and select Start. To finish, select Stop and save the video.`,
        ),
      },
      {
        q: t("¿Graba el sonido del PC y el micrófono?", "Does it record PC sound and the microphone?"),
        a: t(
          "Sí: el audio del sistema (lo que suene por los altavoces) y tu micrófono, cada uno grabado en su propia pista y con su volumen en el editor. Cada uno tiene su interruptor en Ajustes y cualquiera de los dos puede ir solo.",
          "Yes: the system audio (whatever the speakers are playing) and your microphone, each recorded on its own track and with its own volume in the editor. Each has its own switch in Settings and either can go alone.",
        ),
      },
      GRATIS,
      VERSIONES,
    ],
  },

  {
    id: "captura",
    es: "captura-de-pantalla",
    en: "screenshot",
    pie: t("Hacer una captura", "Take a screenshot"),
    titulo: t("Cómo hacer una captura de pantalla en Windows 10 y 11", "How to take a screenshot on Windows 10 and 11"),
    descripcion: t(
      "Los atajos de Windows para hacer una captura de pantalla, dónde se guarda, y qué añade winshotx: anclarla, tapar datos personales y capturar una página larga.",
      "The Windows shortcuts to take a screenshot, where it is saved, and what winshotx adds: pinning it, covering personal data and capturing a long page.",
    ),
    h1: t("Cómo hacer una captura de pantalla en Windows", "How to take a screenshot on Windows"),
    entradilla: t(
      `Pulsa ${k("Win", "Mayús", "S")}, elige la zona y Windows guarda la captura en la carpeta Capturas de pantalla. Para una captura suelta no necesitas nada más. Si capturas a menudo, winshotx enseña la selección ya pintada en ${atajo.num}, tapa datos personales con un clic y baja sola por una página larga.`,
      `Press ${k("Win", "Shift", "S")}, pick the area and Windows saves the shot in the Screenshots folder. For a one-off screenshot you need nothing else. If you capture often, winshotx shows the selection already painted in ${atajo.num}, covers personal data with one click and scrolls down a long page by itself.`,
    ),
    secciones: [
      {
        id: "windows",
        indice: t("Con Windows", "With Windows"),
        h2: t("Lo que ya trae Windows", "What Windows already includes"),
        cuerpo: [
          { p: t("Tres teclas, y cada una hace algo distinto:", "Three keys, and each one does something different:") },
          {
            lista: [
              t(
                `<b>${k("Win", "Mayús", "S")}.</b> Abre la Herramienta de Recortes para elegir un rectángulo, una ventana, la pantalla entera o una forma libre. La captura se guarda sola en <b>Imágenes › Capturas de pantalla</b>.`,
                `<b>${k("Win", "Shift", "S")}.</b> Opens the Snipping Tool to pick a rectangle, a window, the full screen or a freeform shape. The shot is saved by itself in <b>Pictures › Screenshots</b>.`,
              ),
              t(
                `<b>${k("Win", "Impr Pant")}.</b> Guarda la pantalla entera en esa misma carpeta.`,
                `<b>${k("Win", "Print Screen")}.</b> Saves the whole screen in that same folder.`,
              ),
              t(
                `<b>${k("Impr Pant")}.</b> Según cómo tengas Windows, copia la pantalla entera al portapapeles o abre la Herramienta de Recortes.`,
                `<b>${k("Print Screen")}.</b> Depending on how your Windows is set up, it copies the whole screen to the clipboard or opens the Snipping Tool.`,
              ),
            ],
          },
        ],
      },
      {
        id: "pasos",
        indice: t("Con winshotx", "With winshotx"),
        h2: t("Cómo se captura con winshotx", "How to capture with winshotx"),
        foto: {
          img: "capturar",
          ancho: 1140,
          alto: 720,
          alt: t("La selección de winshotx sobre una pantalla congelada, con su barra para copiar, guardar o editar", "The winshotx selection over a frozen screen, with its bar to copy, save or edit"),
        },
        cuerpo: [
          {
            pasos: [
              INSTALAR,
              t(
                `<b>Pulsa ${k("Ctrl", "Shift", "2")} y arrastra sobre lo que quieras.</b> La pantalla se congela mientras eliges, y un clic sobre una ventana la coge entera.`,
                `<b>Press ${k("Ctrl", "Shift", "2")} and drag over what you want.</b> The screen freezes while you choose, and one click on a window takes it whole.`,
              ),
              t(
                `<b>Elige qué hacer con ella.</b> En la barra que sale junto a la selección, ${k("Enter")} copia, ${k("Ctrl", "S")} guarda, ${k("E")} abre el editor, ${k("A")} la ancla encima de todo y ${k("T")} copia su texto.`,
                `<b>Choose what to do with it.</b> On the bar next to the selection, ${k("Enter")} copies, ${k("Ctrl", "S")} saves, ${k("E")} opens the editor, ${k("A")} pins it on top of everything and ${k("T")} copies its text.`,
              ),
            ],
          },
          {
            aparte: t(
              `<b>${atajo.num}</b> ${atajo.es}; la Herramienta de Recortes, ${atajo.otra}. ${TEXTOS.medido.es}`,
              `<b>${atajo.num}</b> ${atajo.en}; the Snipping Tool, ${atajo.otra}. ${TEXTOS.medido.en}`,
            ),
          },
        ],
      },
      {
        id: "comparar",
        indice: t("Comparar", "Compare"),
        h2: t("Lo que añade winshotx", "What winshotx adds"),
        cuerpo: [
          { p: t("Fila a fila, y también lo que gana la de Windows:", "Row by row, including where the Windows one wins:") },
          { tabla: { filas: ["zona", "marcas", "anclar", "tapar", "scroll", "ocr", "puesta"], rivales: ["recortes", "sharex"] } },
          { otras: ["recortes", "sharex", "greenshot", "lightshot"] },
        ],
      },
    ],
    preguntas: [
      {
        q: t("¿Dónde se guardan las capturas?", "Where are the screenshots saved?"),
        a: t(
          "Con Windows, en <b>Imágenes › Capturas de pantalla</b>. Con winshotx, en la carpeta que elijas en <b>Ajustes → La app</b>; de fábrica, <code>Imágenes\\winshotx</code>.",
          "With Windows, in <b>Pictures › Screenshots</b>. With winshotx, in the folder you choose in <b>Settings → The app</b>; by default, <code>Pictures\\winshotx</code>.",
        ),
      },
      {
        q: t("¿Puedo capturar con la tecla Impr Pant?", "Can I capture with the Print Screen key?"),
        a: t(
          `Sí. En <b>Ajustes → Teclas de Windows</b> hay un interruptor que se la quita a la Herramienta de Recortes y se la da a winshotx, y la app lo pregunta la primera vez que la abres.`,
          `Yes. In <b>Settings → Windows keys</b> a switch takes that key from the Snipping Tool and gives it to winshotx, and the app asks the first time you open it.`,
        ),
      },
      {
        q: t("¿Cómo capturo una página larga entera?", "How do I capture a whole long page?"),
        a: t(
          `Pulsa ${k("Ctrl", "Shift", "2")}, después ${k("S")} y elige la zona: baja la página sola y la junta en una imagen.`,
          `Press ${k("Ctrl", "Shift", "2")}, then ${k("S")} and pick the area: it scrolls the page down by itself and stitches it into one image.`,
        ),
      },
      {
        q: t("¿Sube mis capturas a algún sitio?", "Does it upload my screenshots anywhere?"),
        a: t(
          "No. Ni cuenta, ni telemetría, ni subidas. La única llamada a la red es mirar si hay versión nueva en GitHub, y la de la Microsoft Store ni eso.",
          "No. No account, no telemetry, no uploads. The only network call is checking GitHub for a new version, and the Microsoft Store version does not even do that.",
        ),
      },
    ],
  },

  {
    id: "gif",
    es: "grabar-gif",
    en: "screen-to-gif",
    pie: t("Grabar un GIF", "Record a GIF"),
    titulo: t("Cómo grabar un GIF de la pantalla en Windows", "Screen to GIF: how to record a GIF of your screen on Windows"),
    descripcion: t(
      "Cómo grabar un GIF de la pantalla en Windows sin instalar FFmpeg: eliges la zona, paras, recortas y lo pegas en Slack o Discord. Gratis y sin cuenta.",
      "How to record a GIF of your screen on Windows without installing FFmpeg: pick the area, stop, trim and paste it into Slack or Discord. Free and with no account.",
    ),
    h1: t("Cómo grabar un GIF de la pantalla en Windows", "How to record a GIF of your screen on Windows"),
    entradilla: t(
      `Windows todavía no graba GIF para todo el mundo: en la Herramienta de Recortes solo está en Insiders, y hasta 30 segundos. Con winshotx pulsas ${k("Ctrl", "Shift", "5")}, después ${k("G")}, arrastras sobre la zona y sale un GIF. No hay que instalar FFmpeg.`,
      `Windows does not record GIF for everyone yet: in the Snipping Tool it is Insiders only, and up to 30 seconds. With winshotx you press ${k("Ctrl", "Shift", "5")}, then ${k("G")}, drag over the area and get a GIF. There is no FFmpeg to install.`,
    ),
    secciones: [
      {
        id: "pasos",
        indice: t("Paso a paso", "Step by step"),
        h2: t("Cómo se graba un GIF con winshotx", "How to record a GIF with winshotx"),
        foto: {
          img: "editor",
          ancho: 1280,
          alto: 800,
          alt: t("El editor de winshotx, con la línea de tiempo abajo y los formatos MP4, GIF, PNG y JPG a la derecha", "The winshotx editor, with the timeline at the bottom and the MP4, GIF, PNG and JPG formats on the right"),
        },
        cuerpo: [
          {
            pasos: [
              INSTALAR,
              t(
                `<b>Pulsa ${k("Ctrl", "Shift", "5")} y después ${k("G")}.</b> La ${k("G")} cambia de vídeo a GIF antes de recortar.`,
                `<b>Press ${k("Ctrl", "Shift", "5")} and then ${k("G")}.</b> The ${k("G")} key switches from video to GIF before you crop.`,
              ),
              t(
                "<b>Arrastra sobre la zona.</b> Al soltar el ratón, empieza a grabar.",
                "<b>Drag over the area.</b> When you let go of the mouse, it starts recording.",
              ),
              t(
                `<b>Pulsa ${k("Ctrl", "Shift", "5")} otra vez para parar.</b> Se abre el editor, ya en GIF.`,
                `<b>Press ${k("Ctrl", "Shift", "5")} again to stop.</b> The editor opens, already on GIF.`,
              ),
              t(
                `<b>Recorta y guarda.</b> Arrastra los bordes azules de la línea de tiempo y pulsa ${k("Ctrl", "S")}. El panel enseña el tamaño estimado antes de escribir nada.`,
                `<b>Trim and save.</b> Drag the blue edges of the timeline and press ${k("Ctrl", "S")}. The panel shows the estimated size before writing anything.`,
              ),
            ],
          },
        ],
      },
      {
        id: "formato",
        indice: t("GIF o MP4", "GIF or MP4"),
        h2: t("Cuándo usar GIF y cuándo MP4", "When to use GIF and when MP4"),
        cuerpo: [
          {
            lista: [
              t(
                "<b>GIF.</b> Clips cortos de interfaz. Hace bucle en todas partes sin reproductor.",
                "<b>GIF.</b> Short interface clips. It loops everywhere with no player.",
              ),
              t(
                "<b>MP4.</b> Todo lo que pase de unos segundos, o con mucho movimiento.",
                "<b>MP4.</b> Anything longer than a few seconds, or with a lot of movement.",
              ),
            ],
          },
        ],
      },
      {
        id: "comparar",
        indice: t("Comparar", "Compare"),
        h2: t("Frente a otros programas de GIF", "Compared with other GIF tools"),
        cuerpo: [
          {
            p: t(
              "Si lo tuyo es editar un GIF fotograma a fotograma, ScreenToGif lo hace y winshotx no:",
              "If what you need is editing a GIF frame by frame, ScreenToGif does it and winshotx does not:",
            ),
          },
          { tabla: { filas: ["gif", "fotogramas", "marcas", "mp4", "instalador"], rivales: ["screentogif", "sharex"] } },
          { otras: ["screentogif", "sharex"] },
        ],
      },
    ],
    preguntas: [
      {
        q: t("¿Cómo grabo un GIF de la pantalla en Windows?", "How do I record a GIF of my screen on Windows?"),
        a: t(
          `Pulsa ${k("Ctrl", "Shift", "5")}, arrastra sobre la zona y púlsalo otra vez para parar. Se abre el editor con todos los fotogramas y el panel escribe el GIF. No hay que instalar FFmpeg.`,
          `Press ${k("Ctrl", "Shift", "5")}, drag over the area, and press it again to stop. The editor opens with every frame and the export panel writes the GIF. There is no FFmpeg to install.`,
        ),
      },
      {
        q: t("¿Cómo pego un GIF en Slack o en Discord?", "How do I paste a GIF into Slack or Discord?"),
        a: t(
          "Usa <b>Copiar</b> en el panel de exportación. Un PNG va al portapapeles como imagen, y un JPG, un GIF o un MP4 van a la vez como archivo y como su ruta escrita: se pegan directamente en Slack, Discord o el Explorador, y en una caja de texto sale la ruta.",
          "Use <b>Copy</b> in the export panel. A PNG goes to the clipboard as an image, and a JPG, a GIF or an MP4 go as a file and as their written path at the same time: they paste straight into Slack, Discord or File Explorer, and into any text box the path comes out instead.",
        ),
      },
      {
        q: t("¿El GIF lleva sonido?", "Does the GIF have sound?"),
        a: t(
          "No, el formato GIF no guarda sonido. Si lo necesitas, graba en MP4: el sonido del PC y tu micrófono van cada uno en su pista.",
          "No, the GIF format does not store sound. If you need it, record in MP4: the PC sound and your microphone each go on their own track.",
        ),
      },
      GRATIS,
    ],
  },
];

/* --------------------------------------------------- una «alternativa a» por programa */
const enRuta = (nombre) => nombre.toLowerCase().replaceAll(" ", "-");

export const ALTERNATIVAS = HERRAMIENTAS.filter((h) => h.id !== "wx").map((h) => ({
  id: h.id,
  es: `alternativa-a-${enRuta(h.es)}`,
  en: `${enRuta(h.en)}-alternative`,
  herramienta: h,
  pie: t(h.corto ?? h.es, h.cortoEn ?? h.en),
}));

/** Todas las páginas nuevas, para repartirlas por el sitemap, el pie y el inglés. */
export const TODAS = [...PROBLEMAS, ...ALTERNATIVAS];
