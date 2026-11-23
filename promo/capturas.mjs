/**
 * Saca de la app DE VERDAD (el bundle de `dist/` servido por el banco de fotos) todo lo que
 * enseña el vídeo, sin abrir nada en el escritorio de nadie.
 *
 *   (en la raíz del repo)  pnpm build                   una vez: el banco fotografía ese bundle
 *   (aquí, en promo/)      node capturas.mjs            todo
 *                          node capturas.mjs overlay    solo una parte (escritorio, overlay, editor, ajustes)
 *                          node capturas.mjs --raiz=<carpeta>   la app, el banco y la web de otro árbol
 *                                                       (el de la versión que sale, con su dist/ compilado)
 *                          node capturas.mjs --fondo=luz    la pantalla que se captura es pantallas/luz.html (una gráfica)
 *                                                       y no la guía de la web: el vídeo de Reddit
 *
 * El peso del instalador vive en UN sitio, `cifras.json`: el vídeo lo dice con ese número y la
 * guía de la web que se fotografía de fondo se pone a ese mismo número antes de la foto (la web
 * puede ir por detrás del instalador que se acaba de compilar). Si cambia, se rehacen las fotos
 * (`node capturas.mjs`), el vídeo (`node render.mjs Promo`) y la mezcla (`node musica.mjs`).
 *
 * Deja en `public/app/` (fuera de git: se rehace en un par de minutos):
 *   escritorio.png         la «pantalla» que se congela: la guía de winshotx.com en inglés
 *   overlay/NNNN.jpg       el overlay de selección fotograma a fotograma, con un ratón de verdad
 *   overlay.json           dónde estaba el ratón en cada fotograma y dónde quedó el recorte
 *   editor-*.png, ajustes-*.png   las ventanas que salen quietas
 *
 * Tres cosas que no se ven y hacen falta:
 * - El ratón lo mueve Playwright (eventos de entrada de Chrome), que es lo único que el overlay
 *   escucha: el `--raton` del banco no le llega (ver `scripts/ver-ventana.mjs`).
 * - El reloj de la página es de mentira (`page.clock`): entre foto y foto corre exactamente un
 *   fotograma, así que una animación sale a su velocidad aunque cada foto tarde lo que tarde.
 * - El banco finge dos ventanas abiertas («Bloc de notas», «Explorador») para probar el imán de
 *   ventanas; aquí no existen en la foto, así que se quitan antes de que el overlay las pida.
 */
import { chromium } from "playwright";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { readFile, writeFile, mkdir, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, extname, dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const raizPedida = process.argv.find((a) => a.startsWith("--raiz="))?.slice("--raiz=".length);
const raiz = raizPedida ? resolve(raizPedida) : resolve(aqui, "..");
const web = join(raiz, "frontlaxweb");
const banco = join(raiz, "scripts", "ver-ventana.mjs");
const salida = join(aqui, "public", "app");
const { pesoInstalador } = JSON.parse(await readFile(join(aqui, "cifras.json"), "utf8"));
if (!/^\d+\.\d\d$/.test(pesoInstalador)) {
  console.error(`cifras.json: pesoInstalador tiene que ser como "3.01" (MB, con dos decimales), y es "${pesoInstalador}"`);
  process.exit(1);
}

if (!existsSync(join(raiz, "dist", "index.html"))) {
  console.error(`no hay ${join(raiz, "dist")}: corre antes \`pnpm build\` en ese árbol (el banco fotografía ese bundle)`);
  process.exit(1);
}
const version = JSON.parse(await readFile(join(raiz, "package.json"), "utf8")).version;
console.log(`fotografío ${raiz} (versión ${version}), con el instalador a ${pesoInstalador} MB`);

/** La pantalla de mentira, en píxeles de CSS, y cuántos píxeles de verdad hay por cada uno:
    a 2 la cámara del vídeo puede acercarse al doble sin que se emborrone nada. */
const PANTALLA = { ancho: 1920, alto: 1080, escala: 2 };
const FPS = 30;
const partes = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const quiero = (n) => partes.length === 0 || partes.includes(n);
/** Con `--fondo=luz`, la pantalla que se congela es `pantallas/luz.html` (un artículo con una gráfica) y no la guía
    de la web: así el vídeo enseña a winshotx capturando algo de verdad y no su propia web. Lo que se selecciona es lo
    que esa página marca con `data-demo="recorte"`, con el mismo margen que alrededor de los botones de la guía. */
const fondo = process.argv.find((a) => a.startsWith("--fondo="))?.slice("--fondo=".length);
if (fondo && !/^[a-z0-9-]+$/.test(fondo)) {
  console.error(`--fondo=${fondo}: tiene que ser el nombre de una página de pantallas/, sin ruta`);
  process.exit(1);
}

/* ------------------------------------------------ la web, servida tal cual */
const TIPOS = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css",
  ".js": "text/javascript",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".webp": "image/webp",
  ".ico": "image/x-icon",
  ".json": "application/json",
};
const servidor = createServer(async (req, res) => {
  let ruta = decodeURIComponent(new URL(req.url, "http://x").pathname);
  if (ruta.endsWith("/")) ruta += "index.html";
  const archivo = join(web, ruta);
  if (!archivo.startsWith(web)) return res.writeHead(403).end();
  try {
    const cuerpo = await readFile(archivo);
    res.writeHead(200, { "Content-Type": TIPOS[extname(archivo)] ?? "application/octet-stream" });
    res.end(cuerpo);
  } catch {
    res.writeHead(404).end("no está");
  }
});
await new Promise((ok) => servidor.listen(0, "127.0.0.1", ok));
const urlWeb = `http://127.0.0.1:${servidor.address().port}`;

/** Corre el banco y espera a que acabe (para las ventanas que salen quietas). */
function fotografiar(args) {
  return new Promise((ok, mal) => {
    const hijo = spawn(process.execPath, [banco, ...args], { cwd: raiz, stdio: ["ignore", "pipe", "pipe"] });
    let texto = "";
    hijo.stdout.on("data", (d) => (texto += d));
    hijo.stderr.on("data", (d) => (texto += d));
    hijo.on("exit", (code) => (code === 0 ? ok() : mal(new Error(`ver-ventana salió con ${code}:\n${texto}`))));
  });
}

/** Levanta el banco con --servir y devuelve la URL de la pantalla y cómo apagarlo. */
function servirBanco(args) {
  return new Promise((ok, mal) => {
    const hijo = spawn(process.execPath, [banco, ...args, "--servir"], { cwd: raiz, stdio: ["ignore", "pipe", "pipe"] });
    let texto = "";
    hijo.stdout.on("data", (d) => {
      texto += d;
      const url = texto.match(/http:\/\/127\.0\.0\.1:\d+\/\S+/);
      if (url) ok({ url: url[0], apagar: () => hijo.kill() });
    });
    hijo.stderr.on("data", (d) => (texto += d));
    hijo.on("exit", (code) => mal(new Error(`ver-ventana --servir salió con ${code}:\n${texto}`)));
  });
}

/** ffmpeg, el del PATH. */
function ffmpeg(args) {
  return new Promise((ok, mal) => {
    const hijo = spawn("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", ...args], { stdio: ["ignore", "ignore", "pipe"] });
    let error = "";
    hijo.stderr.on("data", (d) => (error += d));
    hijo.on("error", () => mal(new Error("no encuentro ffmpeg (winget install Gyan.FFmpeg)")));
    hijo.on("exit", (code) => (code === 0 ? ok() : mal(new Error(`ffmpeg salió con ${code}:\n${error}`))));
  });
}

/** Una curva suave de 0 a 1, la de un ratón que arranca y frena. */
const suave = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);

await mkdir(salida, { recursive: true });
const navegador = await chromium.launch({ channel: "msedge", headless: true });
const apagar = [];
const medidas = {};

try {
  /* ---------------- 1. la pantalla que se congela: la guía de la web, en inglés y en oscuro */
  const pngEscritorio = join(salida, "escritorio.png");
  {
    const ctx = await navegador.newContext({
      viewport: { width: PANTALLA.ancho, height: PANTALLA.alto },
      deviceScaleFactor: PANTALLA.escala,
      colorScheme: "dark",
    });
    const p = await ctx.newPage();
    medidas.fondo = fondo ?? null;
    if (fondo) {
      await p.goto(pathToFileURL(join(aqui, "pantallas", `${fondo}.html`)).href, { waitUntil: "load" });
      await p.evaluate(() => document.fonts.ready);
      await p.waitForTimeout(300);
      medidas.boton = await p.evaluate(() => {
        const r = document.querySelector('[data-demo="recorte"]')?.getBoundingClientRect();
        return r ? { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) } : null;
      });
      if (!medidas.boton) throw new Error(`pantallas/${fondo}.html no marca data-demo="recorte"`);
      medidas.fondo = fondo;
      await p.screenshot({ path: pngEscritorio });
      await ctx.close();
      console.log(`la pantalla: pantallas/${fondo}.html, y se selecciona ${JSON.stringify(medidas.boton)}`);
    } else {
    // ?motion=off: nada a medio entrar en la foto.
    await p.goto(`${urlWeb}/en/docs/?motion=off`, { waitUntil: "load" });
    await p.evaluate(() => document.fonts.ready);
    // El peso que dice la guía, al de `cifras.json` (el mismo que dice el vídeo).
    const pesos = await p.evaluate((peso) => {
      const vistos = [];
      const paseo = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      for (let n = paseo.nextNode(); n; n = paseo.nextNode()) {
        n.nodeValue = n.nodeValue.replace(/\b\d+\.\d\d MB\b/g, (viejo) => (vistos.push(viejo), `${peso} MB`));
      }
      return vistos;
    }, pesoInstalador);
    if (pesos.length === 0) throw new Error("la guía ya no dice el peso del instalador («N.NN MB»): mira qué enseña ahora");
    console.log(`la guía decía ${[...new Set(pesos)].join(", ")}; en la foto, ${pesoInstalador} MB`);
    await p.waitForTimeout(500);
    medidas.boton = await p.evaluate(() => {
      const el = document.querySelector('[data-demo="boton"]');
      const r = el?.getBoundingClientRect();
      return r ? { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) } : null;
    });
    if (!medidas.boton) throw new Error('la guía no marca data-demo="boton"');
    await p.screenshot({ path: pngEscritorio });
    await ctx.close();
    }
  }

  /** Lo que se selecciona: de la esquina de arriba a la izquierda a la de abajo a la derecha, con aire. En la guía, los
      dos botones (440 x 94 desde 18 px a la izquierda y 20 por encima del primero); en otra pantalla, lo marcado. */
  const objetivo = (() => {
    const b = medidas.boton;
    const esquina = { x: b.x - 18, y: b.y - 20 };
    const fin = fondo ? { x: b.x + b.w + 18, y: b.y + b.h + 20 } : { x: b.x + 422, y: b.y + 74 };
    return { esquina, fin, recorte: { x: esquina.x, y: esquina.y, w: fin.x - esquina.x, h: fin.y - esquina.y } };
  })();

  /* ---------------- 2. el overlay con un ratón de verdad, fotograma a fotograma */
  if (quiero("overlay")) {
    const { url, apagar: parar } = await servirBanco([
      `--overlay=${pngEscritorio}`,
      `--ancho=${PANTALLA.ancho}`,
      `--alto=${PANTALLA.alto}`,
      "--idioma=en",
    ]);
    apagar.push(parar);
    const ctx = await navegador.newContext({
      viewport: { width: PANTALLA.ancho, height: PANTALLA.alto },
      deviceScaleFactor: PANTALLA.escala,
      colorScheme: "dark",
    });
    const p = await ctx.newPage();
    // Las dos ventanas que el banco finge para probar el imán no están en esta foto.
    await p.route("**/overlay.html", async (ruta) => {
      const r = await ruta.fetch();
      const cuerpo = (await r.text()).replace(/"windows":\[[^\]]*\]/, '"windows":[]');
      await ruta.fulfill({ response: r, body: cuerpo });
    });
    await p.clock.install();
    await p.goto(url, { waitUntil: "load" });
    // Que dé tiempo a decodificar el congelado (eso va por fuera del reloj) y a montarse.
    await p.waitForTimeout(800);
    await p.clock.runFor(1200);
    await p.waitForTimeout(300);

    const dir = join(salida, "overlay");
    await rm(dir, { recursive: true, force: true });
    await mkdir(dir, { recursive: true });

    // El recorrido: entra por la derecha, va a la esquina de arriba del botón, se para a
    // mirar la lupa, arrastra hasta cubrir los dos botones, suelta y deja ver la barra.
    const desde = { x: 1240, y: 300 };
    const { esquina, fin } = objetivo;
    const tramos = [
      { s: 0.5, de: desde, a: desde },
      { s: 1.4, de: desde, a: esquina },
      { s: 0.7, de: esquina, a: esquina },
      { s: 1.3, de: esquina, a: fin, apretado: true, empieza: "down" },
      { s: 1.8, de: fin, a: fin, empieza: "up" },
    ];
    const ratones = [];
    let n = 0;
    await p.mouse.move(desde.x, desde.y);
    for (const tramo of tramos) {
      if (tramo.empieza === "down") await p.mouse.down();
      if (tramo.empieza === "up") await p.mouse.up();
      const cuantos = Math.round(tramo.s * FPS);
      for (let i = 0; i < cuantos; i++) {
        const t = suave((i + 1) / cuantos);
        const x = tramo.de.x + (tramo.a.x - tramo.de.x) * t;
        const y = tramo.de.y + (tramo.a.y - tramo.de.y) * t;
        await p.mouse.move(x, y);
        await p.clock.runFor(1000 / FPS);
        await p.screenshot({ path: join(dir, `${String(n).padStart(4, "0")}.jpg`), type: "jpeg", quality: 90 });
        ratones.push([Math.round(x * 10) / 10, Math.round(y * 10) / 10, tramo.apretado ? 1 : 0]);
        n++;
      }
    }
    medidas.overlay = { fotogramas: n, fps: FPS, ratones, recorte: { x: esquina.x, y: esquina.y, w: fin.x - esquina.x, h: fin.y - esquina.y } };
    await ctx.close();
    parar();
    console.log(`overlay: ${n} fotogramas`);
  }

  /* ---------------- 3. el editor con ese recorte, del tamaño que se abre en una pantalla 1920 */
  if (quiero("editor")) {
    const { recorte } = objetivo;
    // El recorte, sacado del escritorio a su tamaño de verdad (a la escala de la foto).
    const e = PANTALLA.escala;
    const pngRecorte = join(salida, "recorte.png");
    await ffmpeg(["-i", pngEscritorio, "-vf", `crop=${recorte.w * e}:${recorte.h * e}:${recorte.x * e}:${recorte.y * e}`, pngRecorte]);
    // Doce segundos de ese recorte quieto, para que el editor tenga su vídeo y su botón de play.
    const mp4Recorte = join(salida, "recorte.mp4");
    await ffmpeg(["-loop", "1", "-i", pngRecorte, "-t", "12", "-r", "30", "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2", "-c:v", "libx264", "-pix_fmt", "yuv420p", mp4Recorte]);
    const EDITOR = { ancho: 1440, alto: 900 };
    const { url, apagar: parar } = await servirBanco([`--editor=${pngRecorte}`, `--video=${mp4Recorte}`, "--duracion=12", "--idioma=en", `--ancho=${EDITOR.ancho}`, `--alto=${EDITOR.alto}`]);
    apagar.push(parar);
    const ctx = await navegador.newContext({
      viewport: { width: EDITOR.ancho, height: EDITOR.alto },
      deviceScaleFactor: 2,
      colorScheme: "dark",
      reducedMotion: "reduce",
    });
    const p = await ctx.newPage();
    await p.goto(url, { waitUntil: "load" });
    const atardecer = p.getByRole("button", { name: "Sunset", exact: true }).first();
    await atardecer.waitFor({ timeout: 15000 });
    await p.mouse.move(2, EDITOR.alto - 2);
    await p.waitForTimeout(900);
    await p.screenshot({ path: join(salida, "editor-sin-fondo.png") });
    await atardecer.click();
    await p.mouse.move(2, EDITOR.alto - 2);
    await p.waitForTimeout(1200);
    await p.screenshot({ path: join(salida, "editor-atardecer.png") });
    const gif = p.getByRole("tab", { name: "GIF", exact: true }).or(p.getByRole("button", { name: "GIF", exact: true })).first();
    if (await gif.count()) {
      await gif.click();
      await p.mouse.move(2, EDITOR.alto - 2);
      await p.waitForTimeout(900);
      await p.screenshot({ path: join(salida, "editor-gif.png") });
    }
    medidas.editor = { ...EDITOR, escala: 2 };
    await ctx.close();
    parar();
    console.log("editor: hecho");
  }

  /* ---------------- 4. los ajustes de la 0.5: el riel, el buscador y lo de los últimos segundos */
  if (quiero("ajustes")) {
    await fotografiar([join(salida, "ajustes-capturar.png"), "--idioma=en", "--tema=oscuro", "--escala=2"]);
    await fotografiar([join(salida, "ajustes-ultimos.png"), "--idioma=en", "--tema=oscuro", "--escala=2", "--seccion=ultimos", "--anillo"]);
    medidas.ajustes = { ancho: 900, alto: 723, escala: 2 };
    console.log("ajustes: hecho");
  }

  /* ---------------- 5. las teclas de Windows: «Take Win+Shift+S» apagado y encendido */
  if (quiero("teclas")) {
    // Las dos con el banco levantado y Playwright, para que salgan iguales y para medir dónde
    // queda el interruptor. Encendido es `--tecla-pendiente` (pedida) y además conseguida: el
    // banco no tiene bandera para eso, así que se cambia su respuesta de `shortcut_status`.
    const fotos = [
      { nombre: "ajustes-teclas.png", clave: "no", args: [] },
      { nombre: "ajustes-teclas-si.png", clave: "si", args: ["--tecla-pendiente"], conseguida: true },
    ];
    for (const f of fotos) {
      const { url, apagar: parar } = await servirBanco(["--idioma=en", "--tema=oscuro", "--seccion=teclas", ...f.args]);
      apagar.push(parar);
      const ctx = await navegador.newContext({ viewport: { width: 900, height: 723 }, deviceScaleFactor: 2, colorScheme: "dark", reducedMotion: "reduce" });
      const p = await ctx.newPage();
      if (f.conseguida) {
        await p.route("**/index.html", async (ruta) => {
          const r = await ruta.fetch();
          const cuerpo = (await r.text()).replace('"winShiftS":false', '"winShiftS":true');
          await ruta.fulfill({ response: r, body: cuerpo });
        });
      }
      await p.goto(url, { waitUntil: "load" });
      const interruptor = p.locator('[data-fila="tecWinS"] [role="switch"]');
      await interruptor.waitFor({ timeout: 15000 });
      await p.evaluate(() => document.fonts.ready);
      await p.waitForTimeout(1200);
      const caja = async (sel) => {
        const r = await p.locator(sel).first().boundingBox();
        return r && { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) };
      };
      // Con el interruptor encendido la fila crece (sale «Apply» y la pista ocupa dos líneas), así
      // que se mide en las dos fotos.
      (medidas.teclas ??= {})[f.clave] = {
        interruptor: await caja('[data-fila="tecWinS"] [role="switch"]'),
        fila: await caja('[data-fila="tecWinS"]'),
        dibujo: await caja('[data-dibujo="win-mayus-s"]'),
      };
      await p.screenshot({ path: join(salida, f.nombre) });
      await ctx.close();
      parar();
    }
    console.log("teclas: hecho");
  }

  const rutaMedidas = join(salida, "medidas.json");
  const antes = existsSync(rutaMedidas) ? JSON.parse(await readFile(rutaMedidas, "utf8")) : {};
  await writeFile(rutaMedidas, JSON.stringify({ ...antes, ...medidas, pantalla: PANTALLA }, null, 1));
} finally {
  for (const parar of apagar) parar();
  await navegador.close();
  servidor.close();
}
