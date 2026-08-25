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
 * Cubre la ventana principal, que es la bienvenida y los ajustes. El overlay y el editor
 * necesitan sus propios datos (una captura congelada, una sesión con frames) y no valen
 * con esta tabla.
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

const MOCK = `<script>
window.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
  convertFileSrc: (p) => p,
  transformCallback: (cb) => { const id = Math.floor(Math.random() * 1e9); window["_" + id] = cb; return id; },
  invoke: (cmd) => {
    const tabla = ${JSON.stringify(RESPUESTAS)};
    if (cmd in tabla) return Promise.resolve(tabla[cmd]);
    // Un listen devuelve el número con el que se cancela; el updater sin endpoint, null.
    if (cmd.startsWith("plugin:")) return Promise.resolve(cmd.endsWith("|listen") ? 0 : null);
    return Promise.resolve(null);
  },
};
</script>`;

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
  const archivo = join(DIST, ruta === "/" ? "index.html" : ruta);
  try {
    let cuerpo = await readFile(archivo);
    if (extname(archivo) === ".html") cuerpo = String(cuerpo).replace("<head>", "<head>" + MOCK);
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
  const url = `http://127.0.0.1:${server.address().port}/index.html`;
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
