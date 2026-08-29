/**
 * Fotografía la ventana principal sin abrir la app ni tocar el escritorio de nadie.
 *
 *   pnpm build
 *   node scripts/ver-ventana.mjs ajustes.png
 *   node scripts/ver-ventana.mjs bienvenida.png --bienvenida
 *   node scripts/ver-ventana.mjs estrecho.png --ancho=780
 *   node scripts/ver-ventana.mjs menu.png --menu --ancho=284 --escala=2
 *   node scripts/ver-ventana.mjs pendiente.png --tecla-pendiente
 *   node scripts/ver-ventana.mjs grabar.png --seccion=grabar --anillo
 *
 * Sirve el bundle DE VERDAD de `dist/` y le cuela un `__TAURI_INTERNALS__` de mentira en
 * el <head>, antes del script de la app, para que los `invoke` contesten datos fijos.
 * Después un Chrome sin ventana lo fotografía. Así se ve una pantalla igual que la ve el
 * usuario, con su CSS y su React reales, y no hay que lanzarle la app a nadie por encima
 * de lo que esté haciendo.
 *
 *   node scripts/ver-ventana.mjs anclada.png --anclada=recorte.png --ancho=520 --alto=340
 *   node scripts/ver-ventana.mjs editor.png --editor=fotograma.png --ancho=1180 --alto=760
 *   node scripts/ver-ventana.mjs editor.png --editor=x.png --recorte=200,120,420,300
 *   node scripts/ver-ventana.mjs overlay.png --overlay=escritorio.png
 *   node scripts/ver-ventana.mjs overlay.png --overlay=x.png --raton=300,260 --grabar
 *   node scripts/ver-ventana.mjs overlay.png --overlay=x.png --seleccion=200,180,600,380
 *   node scripts/ver-ventana.mjs overlay.png --overlay=x.png --tecla=p
 *
 * Cubre la ventana principal (bienvenida y ajustes) y, con --overlay, la de selección: ahí
 * hay que darle un PNG que haga de pantalla congelada, porque el overlay se dibuja encima
 * de una foto del escritorio. Y con --editor, la del editor, con una sesión de mentira
 * montada alrededor de ese único fotograma.
 */
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { existsSync, readFileSync } from "node:fs";
import { join, extname, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DIST = join(RAIZ, "dist");

const args = process.argv.slice(2);
const bandera = (nombre, pordefecto) => {
  const encontrada = args.find((a) => a.startsWith(`--${nombre}=`));
  return encontrada ? encontrada.split("=")[1] : pordefecto;
};

const salida = resolve(args.find((a) => !a.startsWith("--")) ?? "ventana.png");
const ancho = bandera("ancho", "840");
// El mismo alto que pide `tauri.conf.json`, para ver lo que de verdad entra sin rueda.
const alto = bandera("alto", "640");
const bienvenida = args.includes("--bienvenida");
// Win+Mayús+S pedida pero todavía no conseguida, que es cuando sale el botón de aplicar.
const teclaPendiente = args.includes("--tecla-pendiente");
// El anillo de los últimos segundos, encendido: con él apagado la sección no cuenta nada.
const anillo = args.includes("--anillo");
// Ruta de un PNG que hará de pantalla congelada; si viene, se fotografía el overlay.
const overlay = bandera("overlay", null);
// Dónde dejar el puntero, para ver lo que solo aparece al pasar por encima.
const raton = bandera("raton", null);
// Abre como si se hubiera pulsado el atajo de grabar, no el de capturar.
const grabar = args.includes("--grabar");
// Un recorte ya hecho, "x,y,ancho,alto", para ver lo que sale despues de soltar.
const seleccion = bandera("seleccion", null);
// Una tecla que pulsar al cargar, para llegar a lo que solo se ve tras pulsarla.
const tecla = bandera("tecla", null);
// Cuántas veces se avanza en la bienvenida antes de la foto, para mirar los pasos de
// dentro: son los que hay que recomprobar cada vez que se le añade algo a uno.
const paso = bandera("paso", null);
// En qué parada del tour guiado se hace la foto (0 = la primera). Arranca el tour desde
// su botón de "La app" y avanza con la flecha derecha, igual que una persona.
const tour = bandera("tour", null);
// Qué sección de los ajustes se abre: capturar, grabar, teclas o app.
const seccion = bandera("seccion", null);
// Como si la ventana se hubiera abierto sola despues de actualizar: abre en "La app" y
// la fila de Actualizaciones dice a que version se ha actualizado.
const actualizado = args.includes("--actualizado");
// Con que tema se pinta la ventana: sistema, claro u oscuro.
const tema = bandera("tema", "oscuro");
// Y en que idioma: sistema, es o en.
const idioma = bandera("idioma", "es");
// Los segundos de la cuenta atrás del temporizador; si viene, se fotografía esa ventanita.
const cuenta = bandera("cuenta", null);
// Un PNG que hará de captura anclada; si viene, se fotografía esa ventana flotante.
const anclada = bandera("anclada", null);
// Y otro que hará de fotograma del editor, con una sesión de mentira alrededor.
const editor = bandera("editor", null);
// El editor de una CAPTURA, que es un solo fotograma y no se reproduce: sale sin tira de
// miniaturas y sin nada que prometa un vídeo. Es otra pantalla y hay que poder mirarla.
const fija = args.includes("--fija");
// El menú de la bandeja, que es su propia ventana y no se puede fotografiar de otro modo
// sin abrirle la app encima a alguien.
const menu = args.includes("--menu");
// Un marco de recorte ya colocado dentro del editor: "x,y,ancho,alto" en píxeles de la foto.
const recorte = bandera("recorte", null);
// En vez de una foto, escupe el DOM ya montado. Para cuando lo que falla no se ve mirando:
// una medida que sale cero, una clase que no llegó, un estilo que el navegador no aplicó.
const dom = args.includes("--dom");
// Cuantos pixeles de pantalla por pixel de CSS. Existe porque **Chrome tiene un ancho
// minimo de ventana de unos 500 px**: al pedirle --window-size=284 pinta la pagina a 500 y
// recorta la foto a 284, asi que la mitad derecha del menu de bandeja parecia no existir.
// Con --escala=2 se le piden 568 fisicos, que si respeta, y la pagina se pinta a 284.
const escala = bandera("escala", null);
// Cuánto tiempo virtual corre antes de la foto. Sube para lo que tarda en aparecer y baja
// para lo que se mueve solo: la cuenta atrás llega a cero en tres segundos de reloj, así
// que con el valor de siempre se fotografía sola el final y nunca un número.
const tiempo = bandera("tiempo", cuenta !== null ? "800" : "4000");

if (!existsSync(join(DIST, "index.html"))) {
  console.error("no hay dist/: corre antes `pnpm build`");
  process.exit(1);
}

const AJUSTES = {
  captureShortcut: "CmdOrCtrl+Shift+KeyS",
  recordShortcut: "CmdOrCtrl+Shift+KeyA",
  replayShortcut: "CmdOrCtrl+Shift+Digit6",
  replayEnabled: anillo,
  replaySeconds: 30,
  replayScreen: null,
  replayFps: 15,
  replayHeight: 720,
  saveDirectory: "C:\\Users\\yo\\Pictures\\winshotx",
  copyAfterCapture: true,
  openEditorAfterRecording: true,
  captureCursor: true,
  recordAudio: false,
  fps: 30,
  playSound: false,
  showMagnifier: true,
  startWithWindows: false,
  captureDelaySeconds: 0,
  hideDesktopIcons: false,
  captureFlow: "toolbar",
  theme: tema,
  language: idioma,
  printScreenCapture: false,
  takeWinShiftS: teclaPendiente,
  // Lo único que decide si sale la bienvenida o los ajustes.
  onboarded: !bienvenida,
  snippingKeyRestore: null,
  disabledHotkeysRestore: null,
};

/**
 * Lo que mide un PNG, leído de su cabecera.
 *
 * Los ocho primeros bytes son la firma y los ocho siguientes la cabecera del trozo IHDR;
 * el ancho y el alto son los dos enteros de 32 bits que vienen justo detrás. Se lee a mano
 * para no meter una dependencia solo por esto.
 */
function medidaDelPng(ruta) {
  const bytes = readFileSync(resolve(ruta));
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
}

const REGION = editor ? medidaDelPng(editor) : { width: 1280, height: 800 };

// Una sesión de mentira para el editor: un solo fotograma repetido, que es todo lo que
// hace falta para mirar la pantalla. Sin `mp4Path` la vista previa es la imagen, que es
// justo lo que se quiere fotografiar.
const SESION = {
  id: "vista",
  region: { x: 0, y: 0, ...REGION },
  fps: 30,
  frameCount: fija ? 1 : 40,
  durationMs: fija ? 0 : 1333,
  hasAudio: true,
  hasClicks: true,
  cursorBaked: false,
  format: fija ? "still" : "video",
  mp4Path: null,
};

const FOTOGRAMAS = Array.from({ length: fija ? 1 : 40 }, (_, i) => ({
  index: i,
  timestampMs: Math.round((i * 1000) / 30),
  durationMs: 33,
  thumbPath: "/editor.png",
}));

const RESPUESTAS = {
  get_settings: AJUSTES,
  set_settings: AJUSTES,
  shortcut_status: { capture: true, record: true, replay: true, printScreen: false, winShiftS: false },
  // El anillo de los últimos segundos, encendido y con tres pantallas: es la única forma
  // de mirar esa sección entera, porque sus filas dependen de lo que conteste Rust.
  replay_status: {
    running: anillo,
    seconds: 30,
    screen: 2,
    screenLabel: "\\.\DISPLAY2",
    bytes: 214_000_000,
    bytesPerSecond: 3_400_000,
    width: 1280,
    height: 720,
    bufferedMs: 30_000,
  },
  list_screens: [
    { id: 0, label: "1", x: 0, y: 0, width: 1920, height: 1080, scale: 1, isPrimary: true },
    { id: 1, label: "2", x: 1920, y: 0, width: 1080, height: 1920, scale: 1, isPrimary: false },
    { id: 2, label: "3", x: -1920, y: 0, width: 1920, height: 1080, scale: 1, isPrimary: false },
  ],
  cache_stats: { bytes: 0, sessions: 0 },
  print_screen_state: { enabled: false, active: false, takenByWindows: true },
  just_updated: actualizado,
  tray_menu_state: {
    version: "0.2.6",
    recording: false,
    replay: anillo,
    captureShortcut: "Ctrl+Shift+2",
    recordShortcut: "Ctrl+Shift+5",
    replayShortcut: "Ctrl+Shift+6",
  },
  session_info: SESION,
  session_frames: FOTOGRAMAS,
  frame_image: "/editor.png",
  ffmpeg_available: false,
};

const OVERLAY = {
  monitor: { id: 0, label: "principal", x: 0, y: 0, width: 1280, height: 800, scale: 1, isPrimary: true },
  freezePath: "/freeze.png",
  windows: [
    { title: "Documento sin título - Bloc de notas", rect: { x: 90, y: 110, width: 520, height: 340 } },
    { title: "Explorador de archivos", rect: { x: 660, y: 260, width: 540, height: 400 } },
  ],
  settings: AJUSTES,
  intent: grabar ? "record" : "capture",
  // Se finge la pantalla 2 de 3, que es el caso que hay que poder mirar.
  screenNumber: 2,
  screenCount: 3,
};

/**
 * Sin transiciones de CSS.
 *
 * El navegador sin ventana avanza el reloj a saltos (`--virtual-time-budget`) y las
 * transiciones se quedan congeladas a medias: una pestaña recien pulsada sale con el
 * fondo de la anterior, y parece un fallo de la aplicacion que en el DOM no existe.
 * Costo un rato la primera vez. Las animaciones de framer-motion no se tocan: esas van
 * por JavaScript y se resuelven solas.
 */
const SIN_TRANSICIONES = `<style>*,*::before,*::after{transition:none !important}</style>`;

const MOCK = `<script>
window.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
  convertFileSrc: (p) => (${anclada !== null} ? "/anclada.png" : ${editor !== null} ? "/editor.png" : p),
  transformCallback: (cb) => { const id = Math.floor(Math.random() * 1e9); window["_" + id] = cb; return id; },
  // Los eventos de Tauri, de mentira pero funcionando: la app los usa para hablar entre
  // sus ventanas, y sin esto un botón que emite un evento no hace absolutamente nada aquí.
  _oyentes: {},
  invoke: function (cmd, args) {
    const tabla = ${JSON.stringify(RESPUESTAS)};
    if (cmd === "overlay_bootstrap") return Promise.resolve(${JSON.stringify(OVERLAY)});
    if (cmd in tabla) return Promise.resolve(tabla[cmd]);
    if (cmd === "plugin:event|listen") {
      (this._oyentes[args.event] ??= []).push(args.handler);
      return Promise.resolve(this._oyentes[args.event].length);
    }
    if (cmd === "plugin:event|emit" || cmd === "plugin:event|emit_to") {
      for (const id of this._oyentes[args.event] ?? []) {
        window["_" + id]?.({ event: args.event, id: 0, payload: args.payload });
      }
      return Promise.resolve(null);
    }
    if (cmd.startsWith("plugin:")) return Promise.resolve(null);
    return Promise.resolve(null);
  },
};
</script>`;

// Sin ratón no hay hover, y lo que solo se ve al pasar por encima no sale en la foto.
const RATON = raton
  ? `<script>
addEventListener("load", () => {
  const [x, y] = ${JSON.stringify(raton)}.split(",").map(Number);
  setTimeout(() => {
    const bajo = document.elementFromPoint(x, y);
    // El "entra" va antes del "se mueve" y NO burbujea, igual que en un raton de verdad:
    // se manda a cada antepasado por separado. Sin esto no se ven los controles que solo
    // aparecen al pasar por encima, como los de la captura anclada.
    for (let el = bajo; el; el = el.parentElement) {
      el.dispatchEvent(new PointerEvent("pointerenter", { clientX: x, clientY: y }));
      el.dispatchEvent(new PointerEvent("pointerover", { clientX: x, clientY: y, bubbles: true }));
    }
    bajo?.dispatchEvent(
      new PointerEvent("pointermove", { clientX: x, clientY: y, bubbles: true }),
    );
  }, 600);
});
</script>`
  : "";

// La sección se elige haciendo clic en su botón, como haría una persona: así se
// fotografía la pantalla de verdad y no un estado montado a mano que quizá no exista.
// Se busca por el texto porque las secciones son un `Segmented`, el mismo control que los
// selectores de dentro, y ese componente no lleva atributos propios de cada opción.
const ROTULOS = {
  capturar: "Capturar",
  grabar: "Grabar",
  teclas: "Teclas de Windows",
  app: "La app",
};
const SECCION = seccion
  ? `<script>
addEventListener("load", () => {
  setTimeout(() => {
    // Por POSICIÓN y no por el texto del botón: los rótulos cambian con el idioma, y
    // buscándolos por su nombre en español el guion dejaba de encontrarlos en inglés.
    const orden = ${JSON.stringify(Object.keys(ROTULOS))};
    const botones = document.querySelectorAll("header nav button");
    botones[orden.indexOf(${JSON.stringify(seccion)})]?.click();
  }, 500);
});
</script>`
  : "";

// Avanzar se hace pulsando el botón de la esquina, como una persona: es el último del
// pie, y así el guion no depende de cómo se llame en cada paso.
const PASO = paso
  ? `<script>
addEventListener("load", () => {
  let n = ${JSON.stringify(Number(paso))};
  const avanzar = () => {
    if (n-- <= 0) return;
    const botones = document.querySelectorAll("footer button");
    botones[botones.length - 1]?.click();
    setTimeout(avanzar, 320);
  };
  setTimeout(avanzar, 500);
});
</script>`
  : "";

const TOUR = tour !== null
  ? `<script>
addEventListener("load", () => {
  setTimeout(() => {
    // Por posición, no por texto: los rótulos cambian con el idioma. "La app" es la
    // cuarta sección, y dentro el botón del tour es el tercero de su bloque.
    document.querySelectorAll("header nav button")[3]?.click();
    setTimeout(() => {
      // Por el TÍTULO del bloque, no por su contenido: la carpeta de destino también
      // dice "winshotx" y así se colaba el bloque de Archivos.
      const bloque = [...document.querySelectorAll("section")].find(
        (s) => s.querySelector("h2")?.textContent.trim() === "winshotx",
      );
      const botones = bloque ? [...bloque.querySelectorAll("button")] : [];
      // Actualizaciones, Bienvenida, Tour: el tercero.
      botones[2]?.click();
      let n = ${JSON.stringify(Number(tour))};
      const avanzar = () => {
        if (n-- <= 0) return;
        dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
        setTimeout(avanzar, 260);
      };
      setTimeout(avanzar, 420);
    }, 420);
  }, 500);
});
</script>`
  : "";

const TECLA = tecla
  ? `<script>
addEventListener("load", () => {
  setTimeout(() => dispatchEvent(new KeyboardEvent("keydown", { key: ${JSON.stringify(tecla)}, bubbles: true })), 650);
});
</script>`
  : "";

// Arrastrar de verdad: sin soltar el ratón no hay recorte, y sin recorte no salen ni la
// barra de la selección ni las asas ni la medida.
const SELECCION = seleccion
  ? `<script>
addEventListener("load", () => {
  const [x, y, w, h] = ${JSON.stringify(seleccion)}.split(",").map(Number);
  const nuevo = (tipo, cx, cy) => new PointerEvent(tipo, { clientX: cx, clientY: cy, bubbles: true });
  setTimeout(() => {
    (document.querySelector("[class*=h-screen]") ?? document.body).dispatchEvent(nuevo("pointerdown", x, y));
    setTimeout(() => {
      dispatchEvent(nuevo("pointermove", x + w, y + h));
      setTimeout(() => dispatchEvent(nuevo("pointerup", x + w, y + h)), 60);
    }, 60);
  }, 700);
});
</script>`
  : "";

// El recorte del editor se coloca como lo haría una persona: se pulsa el botón y se
// arrastra sobre la vista previa. Así se fotografía el estado de verdad y no uno montado
// a mano que quizá el editor nunca llegue a tener.
const RECORTE = recorte
  ? `<script>
addEventListener("load", () => {
  const [x, y, w, h] = ${JSON.stringify(recorte)}.split(",").map(Number);
  const nuevo = (tipo, cx, cy) => new PointerEvent(tipo, { clientX: cx, clientY: cy, bubbles: true });
  setTimeout(() => {
    [...document.querySelectorAll("button")]
      .find((b) => (b.getAttribute("title") ?? "").startsWith("Recortar"))
      ?.click();
    setTimeout(() => {
      const capa = document.querySelector('svg[role="figure"]');
      capa?.dispatchEvent(nuevo("pointerdown", x, y));
      setTimeout(() => {
        capa?.dispatchEvent(nuevo("pointermove", x + w, y + h));
        setTimeout(() => capa?.dispatchEvent(nuevo("pointerup", x + w, y + h)), 60);
      }, 60);
    }, 120);
  }, 700);
});
</script>`
  : "";

const TIPOS = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".woff2": "font/woff2",
};

const server = createServer(async (req, res) => {
  const ruta = req.url.split("?")[0];
  // El overlay pide su pantalla congelada por el protocolo asset, que aquí es este servidor.
  if (ruta === "/freeze.png" && overlay) {
    res.writeHead(200, { "Content-Type": "image/png" });
    res.end(await readFile(resolve(overlay)));
    return;
  }
  // Y la ventana anclada pide su imagen por el mismo camino.
  if (ruta === "/anclada.png" && anclada) {
    res.writeHead(200, { "Content-Type": "image/png" });
    res.end(await readFile(resolve(anclada)));
    return;
  }
  // El editor pide sus miniaturas y su fotograma grande, todos el mismo archivo.
  if (ruta === "/editor.png" && editor) {
    res.writeHead(200, { "Content-Type": "image/png" });
    res.end(await readFile(resolve(editor)));
    return;
  }
  const archivo = join(DIST, ruta === "/" ? "index.html" : ruta);
  try {
    let cuerpo = await readFile(archivo);
    if (extname(archivo) === ".html") cuerpo = String(cuerpo).replace("<head>", "<head>" + SIN_TRANSICIONES + MOCK + RATON + SELECCION + RECORTE + TECLA + SECCION + PASO + TOUR);
    res.writeHead(200, { "Content-Type": TIPOS[extname(archivo)] ?? "application/octet-stream" });
    res.end(cuerpo);
  } catch {
    res.writeHead(404).end("no está");
  }
});

const NAVEGADORES = [
  "C:\\Program Files\\BraveSoftware\\Brave-Browser\\Application\\brave.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
];
const navegador = NAVEGADORES.find(existsSync);
if (!navegador) {
  console.error("no encuentro Brave, Chrome ni Edge");
  process.exit(1);
}

server.listen(0, () => {
  let pagina = overlay ? "overlay.html" : "index.html";
  // La cuenta atrás es una ventanita cuadrada de 132 px, pero el navegador sin ventana no
  // baja de unos 500: por debajo de eso deja el lienzo a 500 igual y la foto sale recortada
  // por la esquina, en negro, porque lo centrado se queda fuera. Así que se fotografía a
  // 500 y sale con más aire del que tiene de verdad. Para verla a su tamaño real hace
  // falta un navegador de verdad con el viewport a 132.
  let [w, h] = [ancho, alto];
  if (cuenta !== null) {
    pagina = `cuenta.html?segundos=${cuenta}`;
    [w, h] = ["500", "500"];
  }
  if (anclada !== null) {
    // La ventana anclada sale del tamaño del recorte, así que aquí manda --ancho/--alto.
    pagina = `pin.html?imagen=${encodeURIComponent(resolve(anclada))}`;
  }
  if (menu) {
    pagina = "tray-menu.html";
  } else if (editor !== null) {
    pagina = "editor.html?session=vista";
  }
  const url = `http://127.0.0.1:${server.address().port}/${pagina}`;
  const hijo = spawn(navegador, [
    "--headless=new",
    "--disable-gpu",
    "--hide-scrollbars",
    `--window-size=${escala ? Math.round(Number(w) * Number(escala)) : w},${
      escala ? Math.round(Number(h) * Number(escala)) : h
    }`,
    ...(escala ? [`--force-device-scale-factor=${escala}`] : []),
    ...(dom ? ["--dump-dom"] : [`--screenshot=${salida}`]),
    // Sin esto sale la pantalla antes de que React pinte nada.
    `--virtual-time-budget=${tiempo}`,
    // Y así se fotografía lo que ve quien tiene puesto "reducir movimiento" en Windows,
    // que es como la app se comporta desde que respeta esa preferencia.
    "--force-prefers-reduced-motion",
    url,
  ], dom ? { stdio: "inherit" } : undefined);
  hijo.on("exit", (code) => {
    server.close();
    if (!dom) console.log(code === 0 ? `hecha: ${salida}` : `el navegador salió con ${code}`);
    process.exit(code ?? 1);
  });
});
