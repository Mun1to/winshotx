/**
 * Fotografía la ventana principal sin abrir la app ni tocar el escritorio de nadie.
 *
 *   pnpm build
 *   node scripts/ver-ventana.mjs ajustes.png
 *   node scripts/ver-ventana.mjs bienvenida.png --bienvenida
 *   node scripts/ver-ventana.mjs estrecho.png --ancho=780
 *   node scripts/ver-ventana.mjs pendiente.png --tecla-pendiente
 *
 * Sirve el bundle DE VERDAD de `dist/` y le cuela un `__TAURI_INTERNALS__` de mentira en
 * el <head>, antes del script de la app, para que los `invoke` contesten datos fijos.
 * Después un Chrome sin ventana lo fotografía. Así se ve una pantalla igual que la ve el
 * usuario, con su CSS y su React reales, y no hay que lanzarle la app a nadie por encima
 * de lo que esté haciendo.
 *
 *   node scripts/ver-ventana.mjs overlay.png --overlay=escritorio.png
 *   node scripts/ver-ventana.mjs overlay.png --overlay=x.png --raton=300,260 --grabar
 *   node scripts/ver-ventana.mjs overlay.png --overlay=x.png --seleccion=200,180,600,380
 *
 * Cubre la ventana principal (bienvenida y ajustes) y, con --overlay, la de selección: ahí
 * hay que darle un PNG que haga de pantalla congelada, porque el overlay se dibuja encima
 * de una foto del escritorio. El editor necesita una sesión con frames y no entra aquí.
 */
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { existsSync } from "node:fs";
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
// Ruta de un PNG que hará de pantalla congelada; si viene, se fotografía el overlay.
const overlay = bandera("overlay", null);
// Dónde dejar el puntero, para ver lo que solo aparece al pasar por encima.
const raton = bandera("raton", null);
// Abre como si se hubiera pulsado el atajo de grabar, no el de capturar.
const grabar = args.includes("--grabar");
// Un recorte ya hecho, "x,y,ancho,alto", para ver lo que sale despues de soltar.
const seleccion = bandera("seleccion", null);

if (!existsSync(join(DIST, "index.html"))) {
  console.error("no hay dist/: corre antes `pnpm build`");
  process.exit(1);
}

const AJUSTES = {
  captureShortcut: "CmdOrCtrl+Shift+KeyS",
  recordShortcut: "CmdOrCtrl+Shift+KeyA",
  saveDirectory: "C:\\Users\\yo\\Pictures\\winshotx",
  copyAfterCapture: true,
  openEditorAfterRecording: true,
  captureCursor: true,
  recordAudio: false,
  fps: 30,
  playSound: false,
  showMagnifier: true,
  startWithWindows: false,
  captureFlow: "toolbar",
  printScreenCapture: false,
  takeWinShiftS: teclaPendiente,
  // Lo único que decide si sale la bienvenida o los ajustes.
  onboarded: !bienvenida,
  snippingKeyRestore: null,
  disabledHotkeysRestore: null,
};

const RESPUESTAS = {
  get_settings: AJUSTES,
  set_settings: AJUSTES,
  shortcut_status: { capture: true, record: true, printScreen: false, winShiftS: false },
  cache_stats: { bytes: 0, sessions: 0 },
  print_screen_state: { enabled: false, active: false, takenByWindows: true },
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
};

const MOCK = `<script>
window.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
  convertFileSrc: (p) => p,
  transformCallback: (cb) => { const id = Math.floor(Math.random() * 1e9); window["_" + id] = cb; return id; },
  invoke: (cmd) => {
    const tabla = ${JSON.stringify(RESPUESTAS)};
    if (cmd === "overlay_bootstrap") return Promise.resolve(${JSON.stringify(OVERLAY)});
    if (cmd in tabla) return Promise.resolve(tabla[cmd]);
    // Un listen devuelve el número con el que se cancela; el updater sin endpoint, null.
    if (cmd.startsWith("plugin:")) return Promise.resolve(cmd.endsWith("|listen") ? 0 : null);
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
    document.elementFromPoint(x, y)?.dispatchEvent(
      new PointerEvent("pointermove", { clientX: x, clientY: y, bubbles: true }),
    );
  }, 600);
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
  const archivo = join(DIST, ruta === "/" ? "index.html" : ruta);
  try {
    let cuerpo = await readFile(archivo);
    if (extname(archivo) === ".html") cuerpo = String(cuerpo).replace("<head>", "<head>" + MOCK + RATON + SELECCION);
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
  const pagina = overlay ? "overlay.html" : "index.html";
  const url = `http://127.0.0.1:${server.address().port}/${pagina}`;
  const hijo = spawn(navegador, [
    "--headless=new",
    "--disable-gpu",
    "--hide-scrollbars",
    `--window-size=${ancho},${alto}`,
    `--screenshot=${salida}`,
    // Sin esto sale la pantalla antes de que React pinte nada.
    "--virtual-time-budget=4000",
    // Y así se fotografía lo que ve quien tiene puesto "reducir movimiento" en Windows,
    // que es como la app se comporta desde que respeta esa preferencia.
    "--force-prefers-reduced-motion",
    url,
  ]);
  hijo.on("exit", (code) => {
    server.close();
    console.log(code === 0 ? `hecha: ${salida}` : `el navegador salió con ${code}`);
    process.exit(code ?? 1);
  });
});
