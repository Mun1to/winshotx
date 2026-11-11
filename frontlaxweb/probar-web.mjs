// Mira la web como la vería alguien, sin abrir nada en el escritorio: Chromium sin ventana.
//
//   node frontlaxweb/probar-web.mjs                      fotos de la portada en claro y oscuro
//   node frontlaxweb/probar-web.mjs novedades/ docs/     otras páginas (la portada siempre va)
//   node frontlaxweb/probar-web.mjs --entera             la página completa, no solo lo de arriba
//   node frontlaxweb/probar-web.mjs --cruceta            además, mide la cruceta de la portada
//
// Cada página sale en oscuro y en claro, a 1920 (la pantalla de Munir), a 1440 y en un
// iPhone 13, y en las tres se apuntan los errores de consola y si algo se sale por un lado.
// Las fotos van a `.fotos-web\` en la raíz del repo, que git no ve.
//
// La cruceta se mide con la CPU cuatro veces más lenta (lo que pide la casa para dar algo por
// fluido): el ratón de Playwright cruza la portada durante dos segundos y se cuentan los
// fotogramas que pinta su lienzo. También comprueba que el cursor del sistema sigue a la vista
// y que con «?motion=off» se queda quieta.

import { chromium, devices } from "playwright";
import { mkdirSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const entera = args.includes("--entera");
const medirCruceta = args.includes("--cruceta");
const paginas = ["", ...args.filter((a) => !a.startsWith("--"))];

const destino = resolve(aqui, "..", ".fotos-web");
mkdirSync(destino, { recursive: true });

const url = (ruta, extra = "") => pathToFileURL(join(aqui, ruta, "index.html")).href + extra;

const TAMANOS = [
  { nombre: "1920", contexto: { viewport: { width: 1920, height: 1080 } } },
  { nombre: "1440", contexto: { viewport: { width: 1440, height: 900 } } },
  { nombre: "movil", contexto: devices["iPhone 13"] },
];

const navegador = await chromium.launch();
let fallos = 0;

for (const ruta of paginas) {
  for (const tema of ["dark", "light"]) {
    for (const tam of TAMANOS) {
      const ctx = await navegador.newContext({ ...tam.contexto, colorScheme: tema });
      const page = await ctx.newPage();
      const errores = [];
      page.on("console", (m) => { if (m.type() === "error") errores.push(m.text()); });
      page.on("pageerror", (e) => errores.push(String(e)));
      await page.goto(url(ruta), { waitUntil: "load" });
      await page.waitForTimeout(500);

      const nombre = `${(ruta || "portada").replaceAll("/", "-").replace(/-$/, "")}-${tema === "dark" ? "oscuro" : "claro"}-${tam.nombre}.png`;
      await page.screenshot({ path: join(destino, nombre), fullPage: entera });

      const { ancho, visible } = await page.evaluate(() => ({
        ancho: document.documentElement.scrollWidth,
        visible: document.documentElement.clientWidth,
      }));
      const desborda = ancho > visible + 1;
      // Los errores de red de file:// (el contador de GitHub, las fuentes) no son de la web.
      const propios = errores.filter((e) => !/net::ERR|Failed to load resource|api\.github\.com/i.test(e));
      if (desborda || propios.length) fallos++;
      console.log(
        `${nombre.padEnd(34)} ${desborda ? `SE SALE (${ancho} > ${visible})` : "ancho bien"}` +
          `${propios.length ? `  ERRORES: ${propios.join(" | ")}` : ""}`,
      );
      await ctx.close();
    }
  }
}

if (medirCruceta) {
  const medir = async (extra, cpu) => {
    const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    const errores = [];
    page.on("pageerror", (e) => errores.push(String(e)));
    page.on("console", (m) => { if (m.type() === "error") errores.push(m.text()); });
    await page.goto(url("", extra), { waitUntil: "load" });
    const cdp = await ctx.newCDPSession(page);
    if (cpu > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate: cpu });
    await page.mouse.move(200, 300);
    await page.waitForTimeout(400);

    // Cada `rAF` en el que el lienzo cambia de verdad cuenta como un fotograma pintado.
    await page.evaluate(() => {
      window.__cuadros = [];
      const lienzo = document.querySelector("canvas.cruceta");
      if (!lienzo) return;
      const contar = (t) => { window.__cuadros.push(t); if (!window.__parar) requestAnimationFrame(contar); };
      requestAnimationFrame(contar);
    });
    const t0 = Date.now();
    let i = 0;
    while (Date.now() - t0 < 2000) {
      const x = 200 + ((i * 23) % 1000);
      const y = 250 + Math.round(Math.sin(i / 6) * 120);
      await page.mouse.move(x, y);
      i++;
    }
    const datos = await page.evaluate(() => {
      window.__parar = true;
      const c = window.__cuadros;
      const huecos = c.slice(1).map((t, k) => t - c[k]);
      const lienzo = document.querySelector("canvas.cruceta");
      const cursor = getComputedStyle(document.body).cursor;
      const html = getComputedStyle(document.documentElement).cursor;
      return {
        hay: !!lienzo,
        visible: lienzo ? getComputedStyle(lienzo).display !== "none" && getComputedStyle(lienzo).opacity !== "0" : false,
        cuadros: c.length,
        ms: c.length > 1 ? c[c.length - 1] - c[0] : 0,
        peor: huecos.length ? Math.max(...huecos) : 0,
        lentos: huecos.filter((h) => h > 20).length,
        cursorTapado: cursor === "none" || html === "none",
        pointerEvents: lienzo ? getComputedStyle(lienzo).pointerEvents : "",
        quieta: lienzo ? lienzo.dataset.quieta === "1" : false,
      };
    });
    await ctx.close();
    return { ...datos, errores };
  };

  for (const [extra, cpu, que] of [["", 1, "normal"], ["", 4, "CPU 4x"], ["?motion=off", 4, "motion=off, CPU 4x"]]) {
    const d = await medir(extra, cpu);
    const fps = d.ms ? (d.cuadros - 1) / (d.ms / 1000) : 0;
    console.log(
      `cruceta ${que.padEnd(20)} ${d.hay ? "" : "NO HAY LIENZO  "}${fps.toFixed(1)} fps, peor hueco ${d.peor.toFixed(1)} ms, ` +
        `${d.lentos} huecos de más de 20 ms; cursor ${d.cursorTapado ? "TAPADO" : "a la vista"}; ` +
        `clics ${d.pointerEvents === "none" ? "pasan a través" : "LOS COGE EL LIENZO"}` +
        `${extra ? `; ${d.quieta ? "se queda quieta" : "SE MUEVE"}` : ""}` +
        `${d.errores.length ? `  ERRORES: ${d.errores.join(" | ")}` : ""}`,
    );
    if (!d.hay || d.cursorTapado || d.pointerEvents !== "none" || d.errores.length) fallos++;
  }
}

await navegador.close();
console.log(fallos ? `\n${fallos} con algo que mirar. Fotos en ${destino}` : `\nTodo en orden. Fotos en ${destino}`);
process.exit(fallos ? 1 : 0);
