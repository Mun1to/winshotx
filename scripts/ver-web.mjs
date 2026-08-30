// Fotografia la web para poder mirarla, en escritorio y en un movil de verdad.
//
//   node scripts/ver-web.mjs                        la portada, en escritorio
//   node scripts/ver-web.mjs privacidad/            otra pagina
//   node scripts/ver-web.mjs privacidad/ --movil    con un iPhone 13 emulado
//   node scripts/ver-web.mjs --entera               la pagina completa, no solo lo visible
//
// Por que Playwright y no Chrome sin cabeza a secas: Chrome sin cabeza **no aplica el
// meta viewport**. Con `--window-size=390,640` pinta la pagina a ancho de escritorio y la
// recorta, asi que sale descuadrada y parece un fallo de la web cuando no lo es. Playwright
// emula el dispositivo entero (viewport, densidad de pixeles y user agent), que es lo unico
// que sirve para dar por buena una pagina en movil.

import { chromium, devices } from "playwright";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

const args = process.argv.slice(2);
const movil = args.includes("--movil");
const entera = args.includes("--entera");
const ruta = args.find((a) => !a.startsWith("--")) ?? "";

const raiz = resolve(process.cwd(), "frontlaxweb");
const destino = join(process.cwd(), ".fotos-web");
mkdirSync(destino, { recursive: true });

const navegador = await chromium.launch();
const ctx = await navegador.newContext(
  movil ? devices["iPhone 13"] : { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 },
);
const page = await ctx.newPage();

const url = `file:///${join(raiz, ruta, "index.html").replaceAll("\\", "/")}`;
await page.goto(url, { waitUntil: "networkidle" });
// El tema lo decide un script al cargar: se le da un momento para que no salga a medias.
await page.waitForTimeout(600);

const nombre = `${(ruta || "portada").replaceAll("/", "-").replace(/-$/, "")}${movil ? "-movil" : ""}.png`;
const archivo = join(destino, nombre);
await page.screenshot({ path: archivo, fullPage: entera });

const ancho = await page.evaluate(() => document.documentElement.scrollWidth);
const visible = await page.evaluate(() => document.documentElement.clientWidth);
console.log(`${nombre}  (${movil ? "iPhone 13" : "1440x900"})`);
console.log(`  ${archivo}`);
if (ancho > visible + 1) {
  console.log(`  AVISO: la pagina mide ${ancho} px de ancho y la pantalla ${visible}: se sale por un lado.`);
} else {
  console.log(`  ancho correcto: ${ancho} px, sin desbordar.`);
}

await navegador.close();
