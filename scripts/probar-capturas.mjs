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
 *  2. El historial: su apartado del riel, la rejilla con sus miniaturas y sus fechas, y las
 *     acciones al pasar el ratón. Desde S26, la caja de buscar por el texto de las fotos: que
 *     quepa en la fila de los filtros, que pida a Rust lo escrito, lo que sale sin resultados
 *     y que Escape la borre. El texto de cada foto lo pone aquí `TEXTOS`, y el Rust de
 *     mentira busca sin tildes ni mayúsculas, como `historial_texto.rs`.
 *  3. «Después de capturar», que desde la 0.5 (S36) es su propio apartado: que cabe sin rueda en
 *     900 x 760, que el color del fondo sale con fondo puesto, y que el dibujo de «Así va a
 *     salir» pierde el fondo al quitarlo. La ventana mide lo justo de cada apartado: eso lo
 *     comprueba `probar-medida-ajustes.mjs`.
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
// carta las mete enteras sin deformarlas. Y dos con texto escrito (S26), para la búsqueda.
const carpeta = join(tmpdir(), "winshotx-probar-capturas");
mkdirSync(carpeta, { recursive: true });
const ANCHA = join(carpeta, "ancha.png");
const ALTA = join(carpeta, "alta.png");
const FACTURA = join(carpeta, "factura.png");
const CAMION = join(carpeta, "camion.png");
const conTexto = (lineas) =>
  "drawbox=x=0:y=0:w=iw:h=96:color=0x2b5fd9:t=fill," +
  lineas
    .map(
      (linea, i) =>
        `drawtext=fontfile='C\\:/Windows/Fonts/segoeui.ttf':text='${linea.replaceAll(":", "\\:")}':fontsize=${i === 0 ? 64 : 44}:fontcolor=0x1f1f1f:x=70:y=${150 + i * 110}`,
    )
    .join(",");
for (const [ruta, entrada, filtro] of [
  [ANCHA, "testsrc2=size=1280x720:rate=1:duration=1", null],
  [ALTA, "testsrc2=size=600x900:rate=1:duration=1", null],
  [FACTURA, "color=c=white:s=1280x720:d=1", conTexto(["Factura de la luz", "Periodo octubre 2026", "Total 54.20 euros"])],
  [CAMION, "color=c=white:s=1280x720:d=1", conTexto(["El camión de reparto", "llega el martes a las 10:30"])],
]) {
  if (existsSync(ruta)) continue;
  const hecho = spawnSync("ffmpeg", [
    "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi",
    "-i", entrada, ...(filtro ? ["-vf", filtro] : []), "-frames:v", "1", ruta,
  ]);
  if (hecho.status !== 0) {
    console.error(`ffmpeg no ha podido hacer las imágenes de prueba: ${hecho.stderr}`);
    process.exit(1);
  }
}
const IMAGENES = { "/ancha.png": ANCHA, "/alta.png": ALTA, "/factura.png": FACTURA, "/camion.png": CAMION };

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
  ["winshotx-20261111-104210.png", "foto", 0.2, "/factura.png"],
  ["winshotx-20261111-093002.mp4", "video", 2, "/ancha.png"],
  ["winshotx-20261110-181500.png", "foto", 20, "/alta.png"],
  ["winshotx-20261103-090000.gif", "video", 200, null],
  ["winshotx-20261102-120000.png", "foto", 230, "/camion.png"],
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
// S26: lo que el lector de Windows habría leído en cada foto (las de las barras, nada).
const TEXTOS = {
  [HISTORIAL[0].entrada.ruta]: "Factura de la luz\nPeriodo octubre 2026\nTotal 54.20 euros",
  [HISTORIAL[4].entrada.ruta]: "El camión de reparto\nllega el martes a las 10:30",
};

const MOCK = `<script>
// Soltar un oyente de eventos al cerrar un componente: la app de verdad lo hace con este
// objeto de Tauri, y sin él la página da un error que en la app no existe.
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
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
    // S26: la búsqueda como la hace Rust: sin tildes ni mayúsculas, solo en las fotos, y con
    // tres fotos todavía por leer mientras se busca.
    if (cmd === "historial_lista") {
      const textos = ${JSON.stringify(TEXTOS)};
      const plano = (t) => t.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
      const palabras = plano(args.buscar || "").split(/\s+/).filter(Boolean);
      const lista = tabla.historial_lista.entradas
        .filter((e) => !args.filtro || e.tipo === args.filtro)
        .filter((e) => palabras.every((p) => plano(textos[e.ruta] || "").includes(p)));
      return Promise.resolve({ entradas: lista, total: lista.length, buscable: true, porLeer: palabras.length ? 3 : 0 });
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
  if (ruta in IMAGENES) {
    res.writeHead(200, { "Content-Type": "image/png" });
    res.end(await readFile(IMAGENES[ruta]));
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
  const pagina = await navegador.newPage({ viewport: { width: 900, height: 760 } });
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

  // S26: la caja de buscar, en la fila de los filtros y sin partirla.
  const cajaBuscar = pagina.getByRole("searchbox");
  const enLaFila = await pagina.evaluate(() => {
    const caja = document.querySelector("[role=searchbox]").closest("label").getBoundingClientRect();
    const pestana = document.querySelector("[role=tab]").getBoundingClientRect();
    const carpeta = [...document.querySelectorAll("[data-historial] button")].find((b) => /carpeta/.test(b.textContent)).getBoundingClientRect();
    return { caja: [caja.top, caja.bottom, caja.width], pestana: [pestana.top, pestana.bottom], carpeta: [carpeta.top, carpeta.bottom] };
  });
  comprueba(
    enLaFila.caja[1] <= enLaFila.pestana[1] + 1 && enLaFila.caja[0] === enLaFila.carpeta[0] && enLaFila.caja[2] >= 240,
    `la caja va en la fila de los filtros, junto a «Abrir la carpeta», y mide ${Math.round(enLaFila.caja[2])} px`,
  );
  // Buscando, la pestaña no encoge (S24, cuando la ventana medía lo que su pestaña; desde S40
  // la ventana es fija y así la rueda no aparece y desaparece con cada letra).
  const altoDeLaPestana = () =>
    pagina.evaluate(() => Math.round(document.querySelector("[data-contenido-ajustes]").getBoundingClientRect().height));
  const altoSinBuscar = await altoDeLaPestana();
  await cajaBuscar.fill("FACTURA");
  await pagina.waitForTimeout(400);
  const altoBuscando = await altoDeLaPestana();
  comprueba(
    Math.abs(altoBuscando - altoSinBuscar) <= 1,
    `buscando, la pestaña no encoge: ${altoSinBuscar} px sin buscar, ${altoBuscando} buscando`,
  );
  const halladas = await pagina.locator("[data-entrada]").evaluateAll((cs) => cs.map((c) => c.getAttribute("data-entrada")));
  comprueba(
    halladas.length === 1 && halladas[0] === "winshotx-20261111-104210.png",
    `«FACTURA» encuentra la foto de la factura: ${JSON.stringify(halladas)}`,
  );
  const pedido = await pagina.evaluate(() => window.__llamadas.findLast((l) => l.cmd === "historial_lista").args);
  comprueba(pedido.buscar === "FACTURA", `pide a Rust lo escrito: ${JSON.stringify(pedido)}`);
  comprueba((await pagina.locator("[data-por-leer]").count()) === 0, "con resultados no dice cuántas quedan por leer");
  await pagina.waitForFunction(() => document.querySelectorAll("[data-entrada] img").length >= 1);
  await pagina.waitForTimeout(150);
  await foto(pagina, `4b-historial-buscando-${tema}.png`);
  await cajaBuscar.fill("camion");
  await pagina.waitForTimeout(400);
  const camion = await pagina.locator("[data-entrada]").evaluateAll((cs) => cs.map((c) => c.getAttribute("data-entrada")));
  comprueba(camion.join() === "winshotx-20261102-120000.png", `«camion» encuentra «camión»: ${JSON.stringify(camion)}`);
  await pagina.waitForTimeout(150);
  await foto(pagina, `4c-historial-camion-${tema}.png`);
  await cajaBuscar.fill("hipopotamo");
  await pagina.waitForTimeout(400);
  comprueba(
    await pagina.getByText("Ninguna foto lleva escrito «hipopotamo»").isVisible(),
    "sin resultados lo dice con lo buscado",
  );
  comprueba(
    (await pagina.locator("[data-sin-resultados] [data-por-leer]").textContent()) ===
      "Quedan 3 fotos por leer: las que lo lleven irán saliendo.",
    "y sin resultados dice cuántas quedan por leer",
  );
  await foto(pagina, `4d-historial-sin-resultados-${tema}.png`);
  const sinResultados = await altoDeLaPestana();
  comprueba(Math.abs(sinResultados - altoSinBuscar) <= 1, `sin resultados tampoco encoge: ${sinResultados} px`);
  await cajaBuscar.press("Escape");
  await pagina.waitForTimeout(400);
  comprueba((await pagina.locator("[data-entrada]").count()) === 6, "Escape la borra y vuelve el historial entero");
  const altoAlVolver = await altoDeLaPestana();
  comprueba(Math.abs(altoAlVolver - altoSinBuscar) <= 1, `al borrarla vuelve a medir lo de antes: ${altoAlVolver} px`);
  // Con una sola fila (los dos vídeos), lo de sin resultados tampoco puede hacerla crecer: en
  // la ventana de verdad, con cuatro fotos, pasó de 355 a 454 px.
  await pagina.getByRole("tab", { name: "Vídeos" }).click();
  await pagina.waitForTimeout(300);
  const unaFila = await altoDeLaPestana();
  await cajaBuscar.fill("hipopotamo");
  await pagina.waitForTimeout(400);
  const unaFilaSinNada = await altoDeLaPestana();
  comprueba(
    Math.abs(unaFilaSinNada - unaFila) <= 1,
    `con una sola fila, sin resultados no la hace crecer: ${unaFila} px, ${unaFilaSinNada} sin resultados`,
  );
  await foto(pagina, `4f-historial-una-fila-sin-resultados-${tema}.png`);
  await cajaBuscar.press("Escape");
  await pagina.getByRole("tab", { name: "Todo" }).click();
  await pagina.waitForTimeout(300);

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

// S26: la caja de buscar en inglés y en chino, que miden distinto: que siga cabiendo en la
// fila de los filtros y que lo de sin resultados salga traducido.
for (const idioma of ["en", "zh"]) {
  console.log(`\nLa caja de buscar del historial, en ${idioma}`);
  const pagina = await navegador.newPage({ viewport: { width: 900, height: 760 } });
  pagina.on("pageerror", (e) => fallos.push(`error en la página: ${e.message}`));
  await pagina.addInitScript((i) => localStorage.setItem("winshotx.idioma", i), idioma);
  await pagina.goto(`${base}/index.html`);
  await pagina.getByRole("button", { name: idioma === "en" ? "History" : "历史记录", exact: true }).click();
  await pagina.locator("[data-entrada]").first().waitFor();
  const fila = await pagina.evaluate(() => {
    const caja = document.querySelector("[role=searchbox]").closest("label").getBoundingClientRect();
    const pestana = document.querySelector("[role=tab]").getBoundingClientRect();
    return { abajo: caja.bottom, pestana: pestana.bottom, ancho: caja.width };
  });
  comprueba(fila.abajo <= fila.pestana + 1 && fila.ancho >= 240, `la caja cabe en la fila (${Math.round(fila.ancho)} px)`);
  await pagina.getByRole("searchbox").fill("hipopotamo");
  await pagina.waitForTimeout(400);
  const vacio = await pagina.locator("[data-sin-resultados]").textContent();
  comprueba(!/Ninguna|fotos|tu PC/.test(vacio), `sin resultados, traducido: «${vacio}»`);
  await foto(pagina, `4e-historial-sin-resultados-${idioma}.png`);
  await pagina.close();
}

// ── 3. «Después de capturar», su apartado ──────────────────────────────────────────
// En los dos idiomas: las frases inglesas miden distinto, y lo que cabe en uno puede pedir
// rueda en el otro.
for (const idioma of ["es", "en"]) {
  console.log(`
Después de capturar, en su apartado, en ${idioma}`);
  const pagina = await navegador.newPage({ viewport: { width: 900, height: 760 } });
  pagina.on("pageerror", (e) => fallos.push(`error en la página: ${e.message}`));
  await pagina.addInitScript((i) => localStorage.setItem("winshotx.idioma", i), idioma);
  await pagina.goto(`${base}/index.html`);
  await pagina.locator('[data-riel] [data-vista="despues"]').click();
  await pagina.locator("[data-tour='despues-de-capturar']").waitFor();
  await pagina.waitForTimeout(150);
  const miniatura = await pagina.locator('[data-fila="desMiniatura"]').count();
  comprueba(miniatura === 1, "la miniatura de la esquina tiene su fila en «Después de capturar»");
  const rueda = await pagina.evaluate(() => {
    const el = document.querySelector("[data-scroll-ajustes]");
    return el.scrollHeight - el.clientHeight;
  });
  comprueba(rueda <= 0, `con el fondo puesto, el apartado cabe sin rueda en 900 x 760 (${rueda} px de más)`);
  // Por su nombre: el marco y el color son dos grupos de opciones, y solo el color va en muestras.
  const color = pagina.getByRole("radiogroup", { name: idioma === "es" ? "Color del fondo" : "Background color" });
  comprueba(await color.isVisible(), "con fondo puesto sale la fila del color");
  const conFondo = await pagina.locator('[data-dibujo-de] [data-dibujo="fondo"]').count();
  comprueba(conFondo === 1, "y el dibujo de «Así va a salir» enseña la foto sobre su fondo");
  await foto(pagina, `6-despues-de-capturar-${idioma}.png`);
  if (idioma !== "es") {
    await pagina.close();
    continue;
  }
  await pagina.getByRole("button", { name: "Sin fondo" }).click();
  const guardado = await pagina.evaluate(() => window.__llamadas.findLast((l) => l.cmd === "set_settings")?.args);
  comprueba(guardado?.settings?.marcoCapturas?.aire === 0, `«Sin fondo» guarda el aire a 0: ${JSON.stringify(guardado?.settings?.marcoCapturas)}`);
  await pagina.waitForTimeout(150);
  const sinFondo = await pagina.locator('[data-dibujo-de] [data-dibujo="fondo"]').count();
  comprueba(sinFondo === 0, "y al quitarlo, el dibujo lo pierde también");
  await pagina.close();
}

await navegador.close();
server.close();
console.log(fallos.length ? `\n${fallos.length} cosas no cuadran:\n- ${fallos.join("\n- ")}` : "\nTodo cuadra");
process.exit(fallos.length ? 1 : 0);
