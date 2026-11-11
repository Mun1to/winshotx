/**
 * Prueba en un navegador de verdad lo nuevo de las capturas (S4 del plan de CleanShot X):
 * la miniatura de la esquina, el historial y las filas de «Después de capturar».
 *
 *   pnpm build
 *   node scripts/probar-capturas.mjs
 *   node scripts/probar-capturas.mjs --fotos=<carpeta>   deja ahí la foto de cada paso
 *
 * Usa Edge sin ventana, que es el motor de las ventanas de la app (WebView2): no se abre
 * nada en el escritorio de nadie ni se le mueve el ratón. Sirve el `dist/` de verdad con un
 * Rust de mentira delante (`__TAURI_INTERNALS__`), y lo que la página le pide a Rust queda
 * apuntado en `window.__llamadas` para comprobarlo.
 *
 * Lo que comprueba:
 *  1. La miniatura: cada carta mide lo que dice `miniatura.rs` (la ventana no corta ni deja
 *     hueco), los botones salen al pasar el ratón, copiar pide su orden con su número, y
 *     arrastrar con el ratón de verdad pide a Rust el arrastre.
 *  2. El historial: la quinta pestaña, la rejilla con sus miniaturas y sus fechas, y las
 *     acciones al pasar el ratón.
 *  3. «Después de capturar» en Capturar, y que la pestaña sigue cabiendo sin rueda en la
 *     ventana de 840 x 620 que pide `tauri.conf.json`.
 *
 * Lo que NO puede probar: soltar la miniatura en WhatsApp. Eso es el arrastre de Windows
 * (OLE) con un ratón de verdad sobre otra app, y solo lo puede mirar Munir.
 *
 * Sale con 0 si todo cuadra y con 1 si algo no.
 */
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { existsSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DIST = join(RAIZ, "dist");
const args = process.argv.slice(2);
const fotos = args.find((a) => a.startsWith("--fotos="))?.slice(8) ?? null;
if (fotos) mkdirSync(fotos, { recursive: true });

if (!existsSync(join(DIST, "miniatura.html"))) {
  console.error("no hay dist/ con la miniatura: corre antes `pnpm build`");
  process.exit(1);
}

// Dos «capturas» de mentira hechas con ffmpeg: una apaisada y una alta, para ver que la
// carta las mete enteras sin deformarlas.
const carpeta = join(tmpdir(), "winshotx-probar-capturas");
mkdirSync(carpeta, { recursive: true });
const ANCHA = join(carpeta, "ancha.png");
const ALTA = join(carpeta, "alta.png");
for (const [ruta, medida] of [
  [ANCHA, "1280x720"],
  [ALTA, "600x900"],
]) {
  if (existsSync(ruta)) continue;
  const hecho = spawnSync("ffmpeg", [
    "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi",
    "-i", `testsrc2=size=${medida}:rate=1:duration=1`, "-frames:v", "1", ruta,
  ]);
  if (hecho.status !== 0) {
    console.error("ffmpeg no ha podido hacer las imágenes de prueba");
    process.exit(1);
  }
}

const AJUSTES = {
  captureShortcut: "CmdOrCtrl+Shift+KeyS",
  recordShortcut: "CmdOrCtrl+Shift+KeyA",
  replayShortcut: "CmdOrCtrl+Shift+Digit6",
  editShortcut: "Super+Shift+KeyL",
  replayEnabled: false,
  replaySeconds: 30,
  replayScreen: null,
  replayFps: 15,
  replayHeight: 720,
  saveDirectory: "C:\\Users\\yo\\Pictures\\winshotx",
  copyAfterCapture: true,
  openEditorAfterRecording: true,
  captureCursor: true,
  recordAudio: false,
  recordMicrophone: false,
  highlightClicks: false,
  highlightKeys: false,
  fps: 30,
  playSound: false,
  showMagnifier: true,
  startWithWindows: false,
  captureDelaySeconds: 0,
  hideDesktopIcons: false,
  captureFlow: "toolbar",
  theme: "oscuro",
  language: "es",
  printScreenCapture: false,
  takeWinShiftS: false,
  onboarded: true,
  snippingKeyRestore: null,
  disabledHotkeysRestore: null,
  marcoCapturas: { aire: 8, fondo: "atardecer" },
  miniatura: true,
  miniaturaSegundos: 10,
};

const AHORA = Date.now();
const HISTORIAL = [
  ["winshotx-20261111-104210.png", "foto", 0.2, "/ancha.png"],
  ["winshotx-20261111-093002.mp4", "video", 2, "/ancha.png"],
  ["winshotx-20261110-181500.png", "foto", 20, "/alta.png"],
  ["winshotx-20261103-090000.gif", "video", 200, null],
  ["winshotx-20261102-120000.png", "foto", 230, "/ancha.png"],
  ["winshotx-20261101-120000.png", "foto", 260, "/alta.png"],
].map(([nombre, tipo, horas, mini]) => ({
  entrada: {
    ruta: `C:\\Users\\yo\\Pictures\\winshotx\\${nombre}`,
    nombre,
    tipo,
    bytes: 1_800_000,
    cuandoMs: AHORA - horas * 3_600_000,
  },
  mini,
}));

const RESPUESTAS = {
  get_settings: AJUSTES,
  set_settings: AJUSTES,
  shortcut_status: { capture: true, edit: true, record: true, replay: true, printScreen: false, winShiftS: false },
  replay_status: { running: false, seconds: 0, screen: 0, screenLabel: "", bytes: 0, bytesPerSecond: 0, width: 0, height: 0, bufferedMs: 0 },
  list_screens: [{ id: 0, label: "1", x: 0, y: 0, width: 1920, height: 1080, scale: 1, isPrimary: true }],
  cache_stats: { bytes: 0, sessions: 0 },
  captura_moderna: true,
  print_screen_state: { enabled: false, active: false, takenByWindows: true },
  just_updated: false,
  miniaturas_lista: {
    lista: [
      { id: 7, vista: "/ancha.png", guardada: false },
      { id: 8, vista: "/alta.png", guardada: true },
    ],
    segundos: 0,
  },
  historial_lista: { entradas: HISTORIAL.map((h) => h.entrada), total: HISTORIAL.length },
};
const MINIATURAS_DEL_HISTORIAL = Object.fromEntries(HISTORIAL.map((h) => [h.entrada.ruta, h.mini]));

const MOCK = `<script>
window.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "miniaturas" }, currentWebview: { windowLabel: "miniaturas", label: "miniaturas" } },
  convertFileSrc: (p) => p,
  transformCallback: (cb) => { const id = Math.floor(Math.random() * 1e9); window["_" + id] = cb; return id; },
  invoke: function (cmd, args) {
    (window.__llamadas ??= []).push({ cmd, args });
    const tabla = ${JSON.stringify(RESPUESTAS)};
    const minis = ${JSON.stringify(MINIATURAS_DEL_HISTORIAL)};
    if (cmd === "historial_miniatura") {
      const mini = minis[args.ruta];
      return mini ? Promise.resolve(mini) : Promise.reject("Windows no da miniatura");
    }
    // El idioma y el tema los pone cada página en su almacén, como lo deja la app al cambiarlo.
    if (cmd === "get_settings" || cmd === "set_settings") {
      let idioma = "es";
      try { idioma = localStorage.getItem("winshotx.idioma") || "es"; } catch (e) {}
      let tema = "oscuro";
      try { tema = localStorage.getItem("winshotx.tema") || "oscuro"; } catch (e) {}
      return Promise.resolve({ ...tabla.get_settings, language: idioma, theme: tema });
    }
    if (cmd in tabla) return Promise.resolve(tabla[cmd]);
    return Promise.resolve(null);
  },
};
</script>
<style>*,*::before,*::after{transition:none !important}</style>`;

const TIPOS = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".png": "image/png", ".svg": "image/svg+xml", ".woff2": "font/woff2" };
const server = createServer(async (req, res) => {
  const ruta = decodeURIComponent(new URL(req.url, "http://x").pathname);
  if (ruta === "/ancha.png" || ruta === "/alta.png") {
    res.writeHead(200, { "Content-Type": "image/png" });
    res.end(await readFile(ruta === "/ancha.png" ? ANCHA : ALTA));
    return;
  }
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
await new Promise((listo) => server.listen(0, listo));
const base = `http://127.0.0.1:${server.address().port}`;

const fallos = [];
const comprueba = (bien, que) => {
  console.log(`${bien ? "  BIEN" : "  MAL "}  ${que}`);
  if (!bien) fallos.push(que);
};
const foto = async (pagina, nombre) => {
  if (fotos) await pagina.screenshot({ path: join(fotos, nombre) });
};
const navegador = await chromium.launch({ channel: "msedge", headless: true });

// ── 1. La miniatura de la esquina ──────────────────────────────────────────────────
{
  console.log("\nLa miniatura de la esquina, dos capturas apiladas");
  // Lo que mide la ventana con dos, según `miniatura::medida`: 256 x (150 * 2 + 8 + 16).
  const pagina = await navegador.newPage({ viewport: { width: 256, height: 324 } });
  pagina.on("pageerror", (e) => fallos.push(`error en la página: ${e.message}`));
  await pagina.goto(`${base}/miniatura.html`);
  await pagina.locator("[data-miniatura='8'] img").waitFor();
  await pagina.waitForFunction(() => [...document.images].every((i) => i.complete && i.naturalWidth > 0));
  const cajas = await pagina.locator("[data-miniatura]").evaluateAll((els) =>
    els.map((e) => {
      const r = e.getBoundingClientRect();
      return [r.left, r.top, r.width, r.height];
    }),
  );
  comprueba(
    JSON.stringify(cajas) === JSON.stringify([[8, 8, 240, 150], [8, 166, 240, 150]]),
    `las dos cartas caben justas en la ventana: ${JSON.stringify(cajas)}`,
  );
  const sobra = await pagina.evaluate(() => document.documentElement.scrollHeight - innerHeight);
  comprueba(sobra === 0, `sin nada cortado ni hueco debajo (${sobra} px de más)`);
  await foto(pagina, "1-miniaturas.png");

  const carta = pagina.locator("[data-miniatura='7']");
  await carta.hover();
  await pagina.waitForTimeout(100);
  const visibles = await pagina.evaluate(() =>
    [...document.querySelectorAll("[data-miniatura='7'] button")]
      .filter((b) => getComputedStyle(b.closest(".absolute")).opacity === "1")
      .map((b) => b.getAttribute("aria-label") ?? b.textContent),
  );
  comprueba(
    ["Cerrar", "Editar", "Anclar", "Copiar", "Guardar"].every((n) => visibles.includes(n)),
    `al pasar el ratón salen: ${visibles.join(", ")}`,
  );
  await foto(pagina, "2-miniatura-encima.png");
  await carta.getByText("Copiar").click();
  const copiar = await pagina.evaluate(() => window.__llamadas.find((l) => l.cmd === "miniatura_copiar"));
  comprueba(copiar?.args?.id === 7, `«Copiar» pide copiar la 7: ${JSON.stringify(copiar?.args)}`);

  // Arrastrar con un ratón de verdad: pulsar en la carta (no en un botón) y moverse.
  const caja = await pagina.locator("[data-miniatura='8']").boundingBox();
  await pagina.mouse.move(caja.x + 30, caja.y + caja.height - 20);
  await pagina.mouse.down();
  await pagina.mouse.move(caja.x + 60, caja.y + caja.height - 40, { steps: 6 });
  const arrastre = await pagina.evaluate(() => window.__llamadas.filter((l) => l.cmd === "miniatura_arrastrar"));
  comprueba(
    arrastre.length === 1 && arrastre[0].args.id === 8,
    `arrastrar la 8 pide el arrastre de Windows una vez: ${JSON.stringify(arrastre.map((l) => l.args))}`,
  );
  await foto(pagina, "3-arrastrando.png");
  await pagina.mouse.up();
  await pagina.close();
}

// ── 2. El historial ────────────────────────────────────────────────────────────────
for (const tema of ["oscuro", "claro"]) {
  console.log(`\nEl historial en la ventana de ajustes, tema ${tema}`);
  const pagina = await navegador.newPage({ viewport: { width: 840, height: 620 } });
  pagina.on("pageerror", (e) => fallos.push(`error en la página: ${e.message}`));
  await pagina.addInitScript((t) => localStorage.setItem("winshotx.tema", t), tema);
  await pagina.goto(`${base}/index.html`);
  await pagina.getByRole("button", { name: "Historial", exact: true }).click();
  await pagina.locator("[data-entrada]").first().waitFor();
  await pagina.waitForFunction(() => document.querySelectorAll("[data-entrada] img").length >= 5);
  await pagina.waitForTimeout(150);
  const cartas = await pagina.locator("[data-entrada]").count();
  comprueba(cartas === 6, `salen las ${cartas} de la carpeta`);
  const fechas = await pagina.locator("[data-entrada] p").allTextContents();
  comprueba(/^Hoy, \d\d:\d\d/.test(fechas[0]), `la primera dice «${fechas[0]}»`);
  const sinMini = await pagina.locator("[data-entrada='winshotx-20261103-090000.gif'] button[aria-label^='Abrir'] svg").count();
  comprueba(sinMini === 1, "la que no tiene miniatura enseña el dibujo de su tipo");
  await foto(pagina, `4-historial-${tema}.png`);
  if (tema === "oscuro") {
    await pagina.locator("[data-entrada]").nth(1).hover();
    await pagina.waitForTimeout(100);
    await foto(pagina, "5-historial-encima.png");
    await pagina.locator("[data-entrada]").nth(1).getByRole("button", { name: "Copiar" }).click();
    const copiar = await pagina.evaluate(() => window.__llamadas.find((l) => l.cmd === "historial_copiar"));
    comprueba(copiar?.args?.ruta?.endsWith("093002.mp4"), `copiar pide el vídeo: ${copiar?.args?.ruta}`);
    await pagina.getByRole("tab", { name: "Vídeos" }).click();
    await pagina.waitForTimeout(100);
    const filtro = await pagina.evaluate(() => window.__llamadas.findLast((l) => l.cmd === "historial_lista").args);
    comprueba(filtro.filtro === "video", `el filtro de vídeos pide solo vídeos: ${JSON.stringify(filtro)}`);
  }
  await pagina.close();
}

// ── 3. «Después de capturar», en Capturar ──────────────────────────────────────────
// En los dos idiomas: las frases inglesas miden distinto, y lo que cabe en uno puede pedir
// rueda en el otro.
for (const idioma of ["es", "en"]) {
  console.log(`\nDespués de capturar, en la pestaña de Capturar, en ${idioma}`);
  const pagina = await navegador.newPage({ viewport: { width: 840, height: 620 } });
  pagina.on("pageerror", (e) => fallos.push(`error en la página: ${e.message}`));
  await pagina.addInitScript((i) => localStorage.setItem("winshotx.idioma", i), idioma);
  await pagina.goto(`${base}/index.html`);
  await pagina.locator("[data-tour='despues-de-capturar']").waitFor();
  await pagina.waitForTimeout(150);
  const seccion = pagina.locator("[data-tour='despues-de-capturar']");
  const caja = await seccion.boundingBox();
  const atajo = await pagina.locator("[data-tour='al-pulsar']").boundingBox();
  comprueba(Math.abs(caja.x - atajo.x) < 1 && caja.y > atajo.y, "la miniatura va debajo de «Al pulsar el atajo», en la misma columna");
  const rueda = await pagina.evaluate(() => {
    const el = document.querySelector("[data-scroll-ajustes]");
    return el.scrollHeight - el.clientHeight;
  });
  comprueba(rueda <= 0, `con el fondo puesto, la pestaña cabe sin rueda en 840 x 620 (${rueda} px de más)`);
  comprueba(await pagina.getByRole("radiogroup").isVisible(), "con fondo puesto sale la fila del color");
  await foto(pagina, `6-despues-de-capturar-${idioma}.png`);
  if (idioma !== "es") {
    await pagina.close();
    continue;
  }
  await pagina.getByRole("button", { name: "Sin fondo" }).click();
  const guardado = await pagina.evaluate(() => window.__llamadas.findLast((l) => l.cmd === "set_settings")?.args);
  comprueba(guardado?.settings?.marcoCapturas?.aire === 0, `«Sin fondo» guarda el aire a 0: ${JSON.stringify(guardado?.settings?.marcoCapturas)}`);
  await pagina.close();
}

await navegador.close();
server.close();
console.log(fallos.length ? `\n${fallos.length} cosas no cuadran:\n- ${fallos.join("\n- ")}` : "\nTodo cuadra");
process.exit(fallos.length ? 1 : 0);
