// Saca las imágenes de la portada de la app DE VERDAD, sin abrir nada en el escritorio.
//
//   pnpm build                           (una vez: el banco de fotos usa el bundle de dist/)
//                                        (y ffmpeg en el PATH, para el vídeo del editor)
//   node frontlaxweb/hacer-imagenes.mjs
//
// Deja en frontlaxweb/img/, en español y en inglés:
//   escritorio-XX.webp  la pantalla congelada de la demo: nuestra guía a 1600 × 1000
//   capturar-XX.webp    la selección de winshotx encima de esa pantalla, con su barra
//   grabar-XX.webp      lo mismo con el atajo de grabar: el botón rojo de grabar
//   editor-XX.webp      el editor con ese recorte, el fondo «Atardecer», esquinas y sombra
//   bienvenida-XX.webp  la bienvenida, para la guía
//   ajustes-XX.webp     los ajustes en la pestaña Capturar, para la guía
//   app-XX.webp         los ajustes en la pestaña La app (carpeta, caché, código), para privacidad
//
// Y escribe en demo.js (entre «escritorio:desde» y «escritorio:hasta») dónde quedó cada cosa
// en la foto: el recorte que la demo trae de serie y el botón al que va el puntero. Se miden
// en la página con getBoundingClientRect, así que la demo nunca apunta a un sitio de la foto
// que ya no está ahí. Lo que se mide va marcado en la guía con data-demo="recorte" y
// data-demo="boton".
//
// Las fotos de la app salen de scripts/ver-ventana.mjs: el bundle real de dist/ con su CSS y
// su React, servido con un Tauri de mentira y fotografiado con un Chrome sin ventana.

import { chromium } from "playwright";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { readFile, writeFile, mkdtemp, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, extname, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const raiz = resolve(aqui, "..");
const banco = join(raiz, "scripts", "ver-ventana.mjs");

if (!existsSync(join(raiz, "dist", "index.html"))) {
  console.error("no hay dist/: corre antes `pnpm build` (el banco de fotos fotografía ese bundle)");
  process.exit(1);
}

const ESCRITORIO = { ancho: 1600, alto: 1000 };
// Lo que mide la foto de la selección: la misma que pide la portada en el <img>.
const TROZO = { ancho: 1140, alto: 720 };
const EDITOR = { ancho: 1280, alto: 800 };
// Calidad del WebP: con 0,86 el texto de la guía se lee igual que en el PNG y cada foto
// se queda en unos 60 KB.
const CALIDAD = 0.86;

const clamp = (v, min, max) => Math.min(max, Math.max(min, v));

/* --------------------------------------------- servir frontlaxweb como la web */
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
  const archivo = join(aqui, ruta);
  if (!archivo.startsWith(aqui)) return res.writeHead(403).end();
  try {
    const cuerpo = await readFile(archivo);
    res.writeHead(200, { "Content-Type": TIPOS[extname(archivo)] ?? "application/octet-stream" });
    res.end(cuerpo);
  } catch {
    res.writeHead(404).end("no está");
  }
});
await new Promise((ok) => servidor.listen(0, "127.0.0.1", ok));
const web = `http://127.0.0.1:${servidor.address().port}`;

const temporal = await mkdtemp(join(tmpdir(), "winshotx-imagenes-"));
const navegador = await chromium.launch();

/** Un lienzo en una página en blanco: recorta un PNG y lo pasa al formato que se pida. */
const taller = await (await navegador.newContext()).newPage();
async function convertir(rutaPng, trozo, tipo, calidad) {
  const datos = (await readFile(rutaPng)).toString("base64");
  const url = await taller.evaluate(
    async ({ datos, trozo, tipo, calidad }) => {
      const img = new Image();
      img.src = `data:image/png;base64,${datos}`;
      await img.decode();
      const r = trozo ?? { x: 0, y: 0, w: img.naturalWidth, h: img.naturalHeight };
      const lienzo = document.createElement("canvas");
      lienzo.width = r.w;
      lienzo.height = r.h;
      lienzo.getContext("2d").drawImage(img, r.x, r.y, r.w, r.h, 0, 0, r.w, r.h);
      return lienzo.toDataURL(tipo, calidad);
    },
    { datos, trozo, tipo, calidad },
  );
  if (!url.startsWith(`data:${tipo}`)) throw new Error(`este navegador no sabe escribir ${tipo}`);
  return Buffer.from(url.slice(url.indexOf(",") + 1), "base64");
}
const aWebp = async (rutaPng, destino) => {
  const webp = await convertir(rutaPng, null, "image/webp", CALIDAD);
  await writeFile(destino, webp);
  return webp.length;
};

/** Corre el banco de fotos y espera a que acabe. */
function fotografiar(args) {
  return new Promise((ok, mal) => {
    const hijo = spawn(process.execPath, [banco, ...args], { cwd: raiz, stdio: ["ignore", "pipe", "pipe"] });
    let salida = "";
    hijo.stdout.on("data", (d) => (salida += d));
    hijo.stderr.on("data", (d) => (salida += d));
    hijo.on("exit", (code) => (code === 0 ? ok() : mal(new Error(`ver-ventana salió con ${code}:\n${salida}`))));
  });
}

/** ffmpeg, el del PATH, como en scripts/probar-editor.mjs. */
function ffmpeg(args) {
  return new Promise((ok, mal) => {
    const hijo = spawn("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", ...args], { stdio: ["ignore", "ignore", "pipe"] });
    let error = "";
    hijo.stderr.on("data", (d) => (error += d));
    hijo.on("error", () => mal(new Error("no encuentro ffmpeg: hace falta para el vídeo del editor (winget install Gyan.FFmpeg)")));
    hijo.on("exit", (code) => (code === 0 ? ok() : mal(new Error(`ffmpeg salió con ${code}:\n${error}`))));
  });
}

/** Levanta el banco con --servir y devuelve la URL de la pantalla y cómo apagarlo. */
function servirBanco(args) {
  return new Promise((ok, mal) => {
    const hijo = spawn(process.execPath, [banco, ...args, "--servir"], { cwd: raiz, stdio: ["ignore", "pipe", "pipe"] });
    let salida = "";
    hijo.stdout.on("data", (d) => {
      salida += d;
      const url = salida.match(/http:\/\/127\.0\.0\.1:\d+\/\S+/);
      if (url) ok({ url: url[0], apagar: () => hijo.kill() });
    });
    hijo.stderr.on("data", (d) => (salida += d));
    hijo.on("exit", (code) => mal(new Error(`ver-ventana --servir salió con ${code}:\n${salida}`)));
  });
}

const medidas = {};
const pesos = [];

for (const idioma of ["es", "en"]) {
  /* ---------------- 0. la bienvenida y los ajustes: van en la guía, así que antes que su foto */
  // Cada una a la altura de lo que lleva, como la ventana de verdad desde que se ajusta a su
  // contenido: a 760 fijos, media foto era ventana vacía y en la guía se veía como un hueco.
  const ventanas = [
    ["bienvenida", ["--bienvenida", "--alto=470"]],
    ["ajustes", ["--alto=600"]],
    ["app", ["--seccion=app", "--alto=560"]],
  ];
  for (const [nombre, extra] of ventanas) {
    const png = join(temporal, `${nombre}-${idioma}.png`);
    await fotografiar([png, `--idioma=${idioma}`, ...extra]);
    pesos.push([`${nombre}-${idioma}.webp`, await aWebp(png, join(aqui, "img", `${nombre}-${idioma}.webp`))]);
  }

  /* ---------------- 1. la pantalla congelada: nuestra guía, en oscuro y quieta */
  const ctx = await navegador.newContext({
    viewport: { width: ESCRITORIO.ancho, height: ESCRITORIO.alto },
    deviceScaleFactor: 1,
    colorScheme: "dark",
  });
  const guia = await ctx.newPage();
  // ?motion=off: nada a medio entrar en la foto.
  await guia.goto(`${web}/${idioma === "en" ? "en/" : ""}docs/?motion=off`, { waitUntil: "load" });
  await guia.evaluate(() => document.fonts.ready);
  await guia.waitForTimeout(400);
  const caja = await guia.evaluate(() => {
    const medir = (cual) => {
      const el = document.querySelector(`[data-demo="${cual}"]`);
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) };
    };
    return { recorte: medir("recorte"), boton: medir("boton") };
  });
  if (!caja.recorte || !caja.boton) {
    throw new Error(`la guía (${idioma}) no marca data-demo="recorte" y data-demo="boton"`);
  }
  const pngEscritorio = join(temporal, `escritorio-${idioma}.png`);
  await guia.screenshot({ path: pngEscritorio });
  await ctx.close();

  // El recorte de serie, con un poco de aire alrededor y sin salirse de la foto.
  const AIRE = 22;
  const x = clamp(caja.recorte.x - AIRE, 0, ESCRITORIO.ancho);
  const y = clamp(caja.recorte.y - AIRE, 0, ESCRITORIO.alto);
  const recorte = {
    x,
    y,
    w: clamp(caja.recorte.w + AIRE * 2, 40, ESCRITORIO.ancho - x),
    h: clamp(caja.recorte.h + AIRE * 2, 40, ESCRITORIO.alto - y),
  };
  const boton = caja.boton;
  if (boton.x < recorte.x || boton.y < recorte.y || boton.x + boton.w > recorte.x + recorte.w || boton.y + boton.h > recorte.y + recorte.h) {
    throw new Error(`(${idioma}) el botón al que va el puntero se sale del recorte: ${JSON.stringify({ recorte, boton })}`);
  }
  medidas[idioma] = { ancho: ESCRITORIO.ancho, alto: ESCRITORIO.alto, recorte, boton };
  pesos.push([`escritorio-${idioma}.webp`, await aWebp(pngEscritorio, join(aqui, "img", `escritorio-${idioma}.webp`))]);

  /* ---------------- 2. el trozo de pantalla que ve el overlay, con el recorte en medio */
  const t = {
    x: clamp(Math.round(recorte.x + recorte.w / 2 - TROZO.ancho / 2), 0, ESCRITORIO.ancho - TROZO.ancho),
    y: clamp(Math.round(recorte.y + recorte.h / 2 - TROZO.alto / 2 + 30), 0, ESCRITORIO.alto - TROZO.alto),
    w: TROZO.ancho,
    h: TROZO.alto,
  };
  const pngTrozo = join(temporal, `trozo-${idioma}.png`);
  await writeFile(pngTrozo, await convertir(pngEscritorio, t, "image/png"));
  const seleccion = [recorte.x - t.x, recorte.y - t.y, recorte.w, recorte.h].join(",");
  const comunes = [`--overlay=${pngTrozo}`, `--seleccion=${seleccion}`, `--ancho=${TROZO.ancho}`, `--alto=${TROZO.alto}`, `--idioma=${idioma}`];

  const pngCapturar = join(temporal, `capturar-${idioma}.png`);
  await fotografiar([pngCapturar, ...comunes]);
  pesos.push([`capturar-${idioma}.webp`, await aWebp(pngCapturar, join(aqui, "img", `capturar-${idioma}.webp`))]);

  const pngGrabar = join(temporal, `grabar-${idioma}.png`);
  await fotografiar([pngGrabar, ...comunes, "--grabar"]);
  pesos.push([`grabar-${idioma}.webp`, await aWebp(pngGrabar, join(aqui, "img", `grabar-${idioma}.webp`))]);

  /* ---------------- 3. el editor con ese recorte y el fondo «Atardecer» del Estudio */
  const pngRecorte = join(temporal, `recorte-${idioma}.png`);
  await writeFile(pngRecorte, await convertir(pngEscritorio, recorte, "image/png"));
  // Sin MP4, el editor se queda diciendo «Preparando el vídeo…» abajo a la izquierda, que es
  // lo que ve el usuario los primeros segundos y no lo que se quiere enseñar. Doce segundos
  // de ese mismo recorte quieto bastan para que salga el botón de reproducir.
  const mp4Recorte = join(temporal, `recorte-${idioma}.mp4`);
  await ffmpeg(["-loop", "1", "-i", pngRecorte, "-t", "12", "-r", "30", "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2", "-c:v", "libx264", "-pix_fmt", "yuv420p", mp4Recorte]);
  const { url, apagar } = await servirBanco([`--editor=${pngRecorte}`, `--video=${mp4Recorte}`, "--duracion=12", `--idioma=${idioma}`, `--ancho=${EDITOR.ancho}`, `--alto=${EDITOR.alto}`]);
  try {
    const ctxEditor = await navegador.newContext({
      viewport: { width: EDITOR.ancho, height: EDITOR.alto },
      deviceScaleFactor: 1,
      colorScheme: "dark",
      reducedMotion: "reduce",
    });
    const editor = await ctxEditor.newPage();
    await editor.goto(url, { waitUntil: "load" });
    const atardecer = editor.getByRole("button", { name: idioma === "en" ? "Sunset" : "Atardecer", exact: true }).first();
    await atardecer.waitFor({ timeout: 15000 });
    await atardecer.click();
    // Que el ratón no se quede encima de nada: el hover no es lo que se quiere enseñar.
    await editor.mouse.move(2, EDITOR.alto - 2);
    await editor.waitForTimeout(1200);
    const pngEditor = join(temporal, `editor-${idioma}.png`);
    await editor.screenshot({ path: pngEditor });
    await ctxEditor.close();
    pesos.push([`editor-${idioma}.webp`, await aWebp(pngEditor, join(aqui, "img", `editor-${idioma}.webp`))]);
  } finally {
    apagar();
  }
}

/* ---------------- 4. lo medido, a demo.js */
const rutaDemo = join(aqui, "demo.js");
const demo = await readFile(rutaDemo, "utf8");
const bloque = `/* escritorio:desde */
const ESCRITORIO = {
  es: ${JSON.stringify(medidas.es)},
  en: ${JSON.stringify(medidas.en)},
}[idioma];
/* escritorio:hasta */`;
const nuevo = demo.replace(/\/\* escritorio:desde \*\/[\s\S]*?\/\* escritorio:hasta \*\//, bloque);
if (nuevo === demo && !demo.includes(bloque)) throw new Error("demo.js no tiene las marcas escritorio:desde y escritorio:hasta");
if (nuevo !== demo) await writeFile(rutaDemo, nuevo, "utf8");

await navegador.close();
servidor.close();
await rm(temporal, { recursive: true, force: true });

for (const [nombre, bytes] of pesos) console.log(`${nombre.padEnd(22)} ${(bytes / 1024).toFixed(0)} KB`);
console.log(`demo.js: ${nuevo === demo ? "las medidas ya estaban" : "medidas nuevas escritas"}`);
console.log("Ahora: node frontlaxweb/generar-en.mjs (las huellas de demo.js cambian)");
