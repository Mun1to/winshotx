/**
 * Los datos de la comparativa de la portada («Frente a frente»): winshotx y otras diez.
 *
 * De aquí sale la sección entera con generar-comparativa.mjs, que la escribe en index.html
 * (y generar-en.mjs la pasa luego al inglés). Cambiar una cifra es cambiarla aquí y correr
 * `node frontlaxweb/generar-en.mjs`.
 *
 * Comprobado el 15 de noviembre de 2026 en la web, la ayuda y las notas de versión de cada
 * una (las fuentes, al pie de la sección). Lo que no sale en ninguna fuente oficial va como
 * «Sin dato oficial», nunca como «No», y una fila con ese valor no se enseña en esa pestaña.
 *
 * Caduca: los tamaños de los instaladores, las versiones y los idiomas cambian con cada
 * versión de cada una. El 2,97 MB es el instalador de la 0.4.7.
 *
 * Cada valor es [estado, español, inglés]; estado: si | no | medio | nv (sin dato oficial) |
 * num (una cifra o un texto).
 */

export const HERRAMIENTAS = [
  { id: "wx", es: "winshotx", en: "winshotx" },
  // `corto` es el nombre de la pestaña; `art`, el artículo de «Lo que gana la Herramienta de Recortes».
  { id: "recortes", es: "Herramienta de Recortes", en: "Snipping Tool", corto: "Recortes", cortoEn: "Snipping Tool", art: "la", artEn: "the" },
  { id: "gamebar", es: "Xbox Game Bar", en: "Xbox Game Bar", corto: "Game Bar", cortoEn: "Game Bar", art: "la" },
  { id: "sharex", es: "ShareX", en: "ShareX" },
  { id: "greenshot", es: "Greenshot", en: "Greenshot" },
  { id: "flameshot", es: "Flameshot", en: "Flameshot" },
  { id: "screentogif", es: "ScreenToGif", en: "ScreenToGif" },
  { id: "obs", es: "OBS Studio", en: "OBS Studio", corto: "OBS", cortoEn: "OBS" },
  { id: "lightshot", es: "Lightshot", en: "Lightshot" },
  { id: "picpick", es: "PicPick", en: "PicPick" },
  { id: "snagit", es: "Snagit", en: "Snagit" },
];

const S = (es = "Sí", en = "Yes") => ["si", es, en];
const N = (es = "No", en = "No") => ["no", es, en];
const M = (es, en) => ["medio", es, en];
const NV = ["nv", "Sin dato oficial", "No official info"];
const NUM = (es, en = es) => ["num", es, en];
const NOGRABA = N("No graba", "Does not record");

// `es`/`en`: la fila dicha como cosa («grabar GIF»), para la lista de lo que hacen las dos.
// `vende`: dicha como lo que hace winshotx, para su lista. `pierde`: donde gana la otra.
// `primero`: va arriba de la lista aunque la hagan muchas (el precio, frente a la de pago).
// `sinEmpate`: no se cuenta en «Las dos» (que las dos sean gratis no le dice nada a nadie).
export const FILAS = [
  {
    id: "precio", es: "Gratis, también para trabajar", en: "Free, for work too", primero: true, sinEmpate: true,
    vende: { es: "Es gratis, también para trabajar", en: "Free, for work too" },
    icono: "i-wallet",
    v: { wx: S(), recortes: S("Con Windows", "With Windows"), gamebar: S("Con Windows", "With Windows"), sharex: S(), greenshot: S(), flameshot: S(), screentogif: S(), obs: S(), lightshot: S(),
      picpick: M("Solo para uso personal", "Personal use only"), snagit: N("Suscripción anual", "Yearly subscription") },
  },
  {
    id: "estudio", es: "Un vídeo con zooms que siguen a los clics, y su fondo", en: "A video with zooms that follow your clicks, and a background",
    vende: { es: "Un vídeo con zooms que siguen a tus clics, y con fondo", en: "A video with zooms that follow your clicks, and a background" },
    icono: "i-zoom-in",
    v: { wx: S("Sí, al exportar", "Yes, on export"), recortes: N(), gamebar: N(), sharex: N(), greenshot: NOGRABA, flameshot: NOGRABA,
      screentogif: N(), obs: N(), lightshot: NOGRABA, picpick: N(), snagit: N() },
  },
  {
    id: "taparVideo", es: "Tapar datos personales en un vídeo", en: "Cover personal data in a video",
    vende: { es: "Tapa sola los datos personales de un vídeo", en: "Covers the personal data in a video by itself" },
    icono: "i-mosaico",
    v: { wx: S(), recortes: N(), gamebar: N(), sharex: N(), greenshot: NOGRABA, flameshot: NOGRABA, screentogif: N(), obs: N(),
      lightshot: NOGRABA, picpick: N(), snagit: N() },
  },
  {
    id: "mcp", es: "Servidor MCP para agentes de IA", en: "MCP server for AI agents",
    vende: { es: "La maneja un agente de IA", en: "An AI agent can drive it" },
    icono: "i-terminal",
    v: { wx: S("Sí, y una orden de terminal", "Yes, and a terminal command"), recortes: N(), gamebar: N(), sharex: N("No; tiene línea de órdenes", "No; it has a command line"),
      greenshot: M("Solo en sus versiones de prueba", "Only in its test builds"), flameshot: N("No; tiene línea de órdenes", "No; it has a command line"), screentogif: N(),
      obs: N("No; se maneja por WebSocket", "No; it has a WebSocket API"), lightshot: N(), picpick: N(), snagit: N() },
  },
  {
    id: "ultimos", es: "Los últimos segundos, sin haber dado a grabar", en: "The last seconds, without pressing record",
    vende: { es: "Guarda los últimos segundos sin haber dado a grabar", en: "Keeps the last seconds without pressing record" },
    icono: "i-rewind",
    v: { wx: S("15, 30 o 60 s", "15, 30 or 60 s"), recortes: N(), gamebar: S("30 s del juego", "30 s of the game"), sharex: N(), greenshot: NOGRABA, flameshot: NOGRABA,
      screentogif: N(), obs: S("De 5 s a 6 h", "5 s to 6 h"), lightshot: NOGRABA, picpick: N(), snagit: N() },
  },
  {
    id: "anclar", es: "Anclar una captura encima de todo", en: "Pin a shot on top of everything",
    vende: { es: "Ancla una captura encima de todo", en: "Pins a shot on top of everything" },
    icono: "i-pin",
    v: { wx: S(), recortes: N(), gamebar: N(), sharex: S(), greenshot: N(), flameshot: S(), screentogif: N(), obs: N(), lightshot: N(), picpick: N(), snagit: N() },
  },
  {
    id: "tapar", es: "Tapar datos personales en una imagen, solo", en: "Cover personal data in an image, by itself",
    vende: { es: "Tapa sola los datos personales de una imagen", en: "Covers the personal data in an image by itself" },
    icono: "i-shield-check",
    v: { wx: S("Sí, con un clic", "Yes, with one click"), recortes: M("Correos y teléfonos", "Emails and phone numbers"), gamebar: N(), sharex: N("No, a mano", "No, by hand"),
      greenshot: N("No, a mano", "No, by hand"), flameshot: N("No, a mano", "No, by hand"), screentogif: N("No, a mano", "No, by hand"), obs: N(), lightshot: N(),
      picpick: N("No, a mano", "No, by hand"), snagit: S("Sí, con un clic", "Yes, with one click") },
  },
  {
    id: "scroll", es: "Una página larga, bajando sola", en: "A long page, scrolling by itself",
    vende: { es: "Captura una página larga, bajando sola", en: "Captures a long page, scrolling by itself" },
    icono: "i-scroll",
    v: { wx: S(), recortes: N(), gamebar: N(), sharex: S(), greenshot: M("Solo en Internet Explorer", "Internet Explorer only"), flameshot: N(), screentogif: N(), obs: N(),
      lightshot: N(), picpick: S(), snagit: S() },
  },
  {
    id: "ocr", es: "Leer el texto de la imagen", en: "Read the text in the image",
    vende: { es: "Lee el texto de una imagen, y de un vídeo", en: "Reads the text in an image, and in a video" },
    icono: "i-text-select",
    v: { wx: S("Sí, también en un vídeo", "Yes, in a video too"), recortes: S(), gamebar: N(), sharex: S(), greenshot: S("Con su complemento", "With its plug-in"), flameshot: N(),
      screentogif: N(), obs: N(), lightshot: N(), picpick: M("Solo en su web aparte", "Only in its separate web app"), snagit: S() },
  },
  {
    id: "cuenta", es: "Sin cuenta", en: "No account", sinEmpate: true,
    vende: { es: "No pide cuenta", en: "Needs no account" },
    icono: "i-user-x",
    v: { wx: S(), recortes: NV, gamebar: M("Algunas funciones la piden", "Some features need one"), sharex: S(), greenshot: S(), flameshot: S(), screentogif: S(), obs: S(), lightshot: S(), picpick: S(),
      snagit: N("Pide cuenta", "Needs an account") },
  },
  {
    id: "gif", es: "Grabar GIF", en: "Record GIF",
    vende: { es: "Graba en GIF", en: "Records GIF" },
    icono: "i-gif",
    v: { wx: S(), recortes: M("Solo Insiders, 30 s", "Insiders only, 30 s"), gamebar: N(), sharex: S(), greenshot: NOGRABA, flameshot: NOGRABA, screentogif: S("Es lo suyo", "Its specialty"),
      obs: N(), lightshot: NOGRABA, picpick: S(), snagit: S("Al exportar", "On export") },
  },
  {
    id: "camara", es: "Tu cámara encima", en: "Your webcam on top",
    vende: { es: "Pone tu cámara encima del vídeo", en: "Puts your webcam on top of the video" },
    icono: "i-webcam",
    v: { wx: S("Redonda o con esquinas", "Round or with corners"), recortes: N(), gamebar: N(), sharex: S(), greenshot: NOGRABA, flameshot: NOGRABA,
      screentogif: N("Aparte, no encima", "Separately, not on top"), obs: S(), lightshot: NOGRABA, picpick: S(), snagit: S() },
  },
  {
    id: "mp4", es: "Grabar vídeo MP4", en: "Record MP4 video",
    vende: { es: "Graba vídeo MP4", en: "Records MP4 video" },
    icono: "i-video",
    v: { wx: S("A 15, 30 o 60 fps", "At 15, 30 or 60 fps"), recortes: S(), gamebar: S("Del juego o la app", "Of the game or app"), sharex: S(), greenshot: NOGRABA, flameshot: NOGRABA,
      screentogif: M("Con FFmpeg aparte", "With FFmpeg, separately"), obs: S(), lightshot: NOGRABA, picpick: S(), snagit: S() },
  },
  {
    id: "audio", es: "Sonido del PC y micrófono", en: "PC sound and microphone",
    vende: { es: "Graba el sonido del PC y el micrófono", en: "Records PC sound and the microphone" },
    icono: "i-mic",
    v: { wx: S("Cada uno en su pista", "Each on its own track"), recortes: S(), gamebar: S(), sharex: S(), greenshot: NOGRABA, flameshot: NOGRABA, screentogif: N(),
      obs: S("Hasta 6 pistas", "Up to 6 tracks"), lightshot: NOGRABA, picpick: S(), snagit: S() },
  },
  {
    id: "marcas", es: "Flechas, texto y pasos numerados", en: "Arrows, text and numbered steps",
    vende: { es: "Dibuja flechas, texto y pasos numerados", en: "Draws arrows, text and numbered steps" },
    icono: "i-pencil",
    v: { wx: S("Once marcas", "Eleven marks"), recortes: M("Sin texto ni pasos", "No text or steps"), gamebar: M("Solo texto", "Text only"), sharex: S(), greenshot: S(), flameshot: S(),
      screentogif: M("Sin pasos numerados", "No numbered steps"), obs: N(), lightshot: NV, picpick: S(), snagit: S() },
  },
  {
    id: "zona", es: "Capturar una zona", en: "Capture an area",
    vende: { es: "Captura una zona de la pantalla", en: "Captures an area of the screen" },
    icono: "i-crop",
    v: { wx: S("Ajustada a las ventanas", "Snapping to windows"), recortes: S(), gamebar: M("Solo el juego o la app", "Only the game or app"), sharex: S(), greenshot: S(), flameshot: S(),
      screentogif: M("Como fotograma", "As a frame"), obs: M("Una fuente o la escena", "A source or the scene"), lightshot: S(), picpick: S(), snagit: S() },
  },
  {
    // No va en las listas: son las dos cifras grandes de cada pestaña.
    id: "instalador", es: "Instalador", en: "Installer",
    v: { wx: NUM("2,97 MB", "2.97 MB"), recortes: NUM("Viene con Windows", "Comes with Windows"), gamebar: NUM("Viene con Windows", "Comes with Windows"), sharex: NUM("145 MB"), greenshot: NUM("4 MB"),
      flameshot: NUM("19 MB"), screentogif: NUM("75 MB"), obs: NUM("187 MB"), lightshot: NUM("2,4 MB", "2.4 MB"), picpick: NUM("69 MB"), snagit: NUM("441 MB") },
  },

  // ---------- donde gana la otra ----------
  {
    id: "puesta", pierde: true, es: "Viene puesta con Windows", en: "Comes with Windows",
    vende: { es: "Viene puesta con Windows", en: "Comes with Windows" },
    icono: "i-download",
    v: { wx: N(), recortes: S(), gamebar: S(), sharex: N(), greenshot: N(), flameshot: N(), screentogif: N(), obs: N(), lightshot: N(), picpick: N(), snagit: N() },
  },
  {
    id: "idiomas", pierde: true, es: "Idiomas", en: "Languages",
    vende: { es: "Habla más idiomas", en: "Speaks more languages" },
    icono: "i-languages",
    v: { wx: ["no", "3", "3"], recortes: S("64", "64"), gamebar: S("63", "63"), sharex: S("29", "29"), greenshot: S("39", "39"), flameshot: S("43", "43"), screentogif: S("29", "29"),
      obs: S("57", "57"), lightshot: NV, picpick: S("Más de 3", "More than 3"), snagit: S("6", "6") },
  },
  {
    id: "sistemas", pierde: true, es: "También en Mac o Linux", en: "Also on Mac or Linux",
    vende: { es: "Funciona también en Mac o en Linux", en: "Also runs on Mac or Linux" },
    icono: "i-monitor",
    v: { wx: N(), recortes: N(), gamebar: N(), sharex: N(), greenshot: N(), flameshot: S("Mac y Linux", "Mac and Linux"), screentogif: N(), obs: S("Mac y Linux", "Mac and Linux"),
      lightshot: S("Mac", "Mac"), picpick: N(), snagit: S("Mac", "Mac") },
  },
  {
    id: "compartir", pierde: true, es: "Subir y compartir con un enlace", en: "Upload and share a link",
    vende: { es: "Sube la captura y te da un enlace", en: "Uploads the shot and gives you a link" },
    icono: "i-link-2",
    v: { wx: N(), recortes: NV, gamebar: NV, sharex: S(), greenshot: S("Con su complemento", "With its plug-in"), flameshot: N("En Windows, no", "Not on Windows"), screentogif: S(), obs: N(),
      lightshot: S("Público en prnt.sc", "Public on prnt.sc"), picpick: S(), snagit: S() },
  },
  {
    id: "directo", pierde: true, es: "Retransmitir en directo", en: "Live streaming",
    vende: { es: "Retransmite en directo", en: "Streams live" },
    icono: "i-video",
    v: { wx: N(), recortes: N(), gamebar: N(), sharex: N(), greenshot: N(), flameshot: N(), screentogif: N(), obs: S(), lightshot: N(), picpick: N(), snagit: N() },
  },
  {
    id: "fotogramas", pierde: true, es: "Editar un GIF fotograma a fotograma", en: "Edit a GIF frame by frame",
    vende: { es: "Edita un GIF fotograma a fotograma", en: "Edits a GIF frame by frame" },
    icono: "i-film",
    v: { wx: N(), recortes: N(), gamebar: N(), sharex: N(), greenshot: N(), flameshot: N(), screentogif: S(), obs: N(), lightshot: N(), picpick: N(), snagit: N() },
  },
];

const enlace = (href, texto) => `<a href="${href}" rel="noopener">${texto}</a>`;
const fuentes = (idioma, recortes) => [
  enlace(RECORTES.replace("{idioma}", idioma), recortes),
  enlace("https://support.microsoft.com/en-us/accessibility/windows/use-a-screen-reader-to-record-your-screen-with-xbox-game-bar", "Xbox Game Bar"),
  enlace("https://getsharex.com/changelog", "ShareX 22.4"),
  enlace("https://getgreenshot.org/help/", "Greenshot 1.3"),
  enlace("https://flameshot.org/", "Flameshot 15"),
  enlace("https://github.com/NickeManarin/ScreenToGif/releases/tag/2.43.2", "ScreenToGif 2.43"),
  enlace("https://obsproject.com/kb/", "OBS Studio 33"),
  enlace("https://app.prntscr.com/en/learnmore.html", "Lightshot"),
  enlace("https://picpick.app/en/features/", "PicPick 7.6"),
  enlace("https://www.techsmith.com/snagit/features/", "Snagit 2027"),
].join(", ");
const RECORTES = "https://support.microsoft.com/{idioma}/windows/use-snipping-tool-to-capture-screenshots-00246869-1843-655f-f220-97299b865f6b";
const AVISAR = "https://github.com/Mun1to/winshotx/issues";

export const TEXTOS = {
  titulo: { es: "Frente a frente", en: "Head to head" },
  entradilla: {
    es: "Lo que hace winshotx y no hacen las otras diez, y donde gana alguna de ellas. El tiempo y la memoria, medidos; lo demás, de la web de cada una.",
    en: "What winshotx does and the other ten do not, and where one of them wins. Time and memory, measured; the rest, from each one's own website.",
  },
  fan: { es: "<b>8 veces más rápida</b> que la que trae Windows. Ahí lo dejo.", en: "<b>8 times faster</b> than the one Windows ships. Just saying." },
  pregunta: { es: "¿Cuál usas ahora?", en: "Which one do you use now?" },
  instalador: { es: "Lo que pesa el instalador.", en: "What the installer weighs." },
  gana: { es: "Lo que gana winshotx", en: "Where winshotx wins" },
  ganaOtra: { es: "Lo que gana {x}", en: "Where {x} wins" },
  nada: { es: "Nada de esta lista.", en: "Nothing on this list." },
  lasDos: { es: "Las dos:", en: "Both:" },
  ademas: { es: "Y además:", en: "And also:" },
  idiomas: { es: "idiomas; winshotx, 3", en: "languages; winshotx, 3" },
  // Las tres cifras medidas, solo en la pestaña de la Herramienta de Recortes, que es contra la que se midieron.
  medido: {
    es: "Medido en un mismo equipo con Windows 11, tres veces seguidas y las dos arrancando desde cero. El cronómetro para cuando la selección ya se ve en pantalla.",
    en: "Measured on the same Windows 11 machine, three runs each, both starting from cold. The clock stops when the selection is already on screen.",
  },
  medidas: [
    // `dura`: la barra se llena a la velocidad de verdad (114 ms la de winshotx, 920 la de Windows).
    { num: "114 ms", otra: "920 ms", largo: 12.4, dura: ["114ms", "920ms"], es: "del atajo a la selección ya pintada, con tres pantallas", en: "from the shortcut to the selection, already painted, with three screens" },
    { num: "33 MB", otra: "253 MB", largo: 13, es: "de memoria mientras capturas", en: "of memory while you capture" },
    { num: "31 MB", otra: "98 MB", largo: 31.6, es: "esperando en la bandeja", en: "waiting in the tray" },
  ],
  fuentes: {
    es: `Comprobado el 15 de noviembre de 2026 en la web, la ayuda y las notas de versión de cada una: ${fuentes("es-es", "Herramienta de Recortes")}. «No» quiere decir que no sale en ninguna de sus fuentes oficiales; ScreenToGif es la versión que su autor ya llama antigua. Si algo ha cambiado, ${enlace(AVISAR, "avísame")} y se corrige.`,
    en: `Checked on 15 November 2026 on each one's website, help pages and release notes: ${fuentes("en-us", "Snipping Tool")}. "No" means it is not in any of its official sources; ScreenToGif is the version its author now calls the old one. If something has changed, ${enlace(AVISAR, "tell me")} and it gets fixed.`,
  },
};
