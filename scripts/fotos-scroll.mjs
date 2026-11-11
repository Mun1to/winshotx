/**
 * Las fotos con las que se prueba el cosido de la captura con scroll.
 *
 *   node scripts/fotos-scroll.mjs            las rehace en src-tauri/tests/scroll/
 *   node scripts/fotos-scroll.mjs --en=<carpeta>
 *
 * Abre una página larga en Edge sin ventana (el mismo motor que pinta media pantalla de
 * Windows), la baja a distintas alturas y fotografía lo que se ve en cada una, como haría
 * winshotx con la zona elegida. Al lado deja la página entera de un tirón (`entera.png`),
 * que es lo que tiene que salir al coserlas. No abre nada en el escritorio de nadie ni
 * mueve el ratón.
 *
 * Tres escenas, cada una con lo suyo:
 *  - `sin-fijos`: la página sola. Pasos de todos los tamaños, uno de un píxel, una foto
 *    repetida (no se ha movido) y una hacia atrás (quien desplaza ha subido).
 *  - `cabecera-y-pie`: una cabecera fija arriba y una barra de cookies fija abajo. Tienen
 *    que salir UNA vez, arriba y abajo, y no en cada costura.
 *  - `escala-125`: la pantalla al 125 %, como el portátil de Munir, con una cabecera
 *    pegajosa y bajando de 100 en 100, que es lo que baja una muesca de la rueda.
 *
 * Lo que comprueba que salgan bien es `capture::scroll::pruebas::fotos_de_verdad`, en
 * `cargo test --lib`.
 */
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import sharp from "sharp";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const en = args.find((a) => a.startsWith("--en="))?.slice(5) ?? join(RAIZ, "src-tauri", "tests", "scroll");

/** Un generador de números con semilla: la página sale igual cada vez. */
function azar(semilla) {
  let s = semilla >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 2 ** 32;
  };
}

const PALABRAS = (
  "captura pantalla zona página baja rueda costura cabecera pie barra ventana texto foto " +
  "imagen larga cose mueve fija queda quieta fila columna alto ancho editor guardar copiar " +
  "navegador lista tabla ejemplo prueba marco desplaza sola mano tecla sale entera repite"
).split(" ");

/** Una página larga y variada: titulares, párrafos, una lista, una tabla y «fotos». */
function pagina({ cabecera = "", pie = "", semilla = 7 }) {
  const r = azar(semilla);
  const frase = (n) =>
    Array.from({ length: n }, () => PALABRAS[Math.floor(r() * PALABRAS.length)]).join(" ");
  const bloques = [];
  for (let i = 0; i < 5; i++) {
    bloques.push(`<h2>${i + 1}. ${frase(3)}</h2>`);
    bloques.push(`<p>${frase(28 + Math.floor(r() * 30))}.</p>`);
    if (i % 3 === 0) {
      const tono = Math.floor(r() * 360);
      bloques.push(
        `<div class="foto" style="background:linear-gradient(${Math.floor(r() * 180)}deg,hsl(${tono} 60% 55%),hsl(${(tono + 70) % 360} 55% 35%))">` +
          `<span style="left:${10 + Math.floor(r() * 60)}%;top:${10 + Math.floor(r() * 50)}%"></span>${frase(2)}</div>`,
      );
    }
    if (i % 3 === 1) {
      bloques.push(`<ul>${Array.from({ length: 4 }, () => `<li>${frase(5)}</li>`).join("")}</ul>`);
    }
    if (i % 3 === 2) {
      const filas = Array.from(
        { length: 4 },
        (_, f) => `<tr><td>${f + 1}</td><td>${frase(2)}</td><td>${Math.floor(r() * 9000)}</td></tr>`,
      ).join("");
      bloques.push(`<table><tr><th>N</th><th>Nombre</th><th>Valor</th></tr>${filas}</table>`);
    }
    bloques.push(`<p>${frase(20 + Math.floor(r() * 20))}.</p>`);
  }
  return `<!doctype html><html lang="es"><head><meta charset="utf-8"><style>
    html { scrollbar-width: none; }
    body { margin: 0; font: 15px/1.5 "Segoe UI", sans-serif; color: #1d1d22; background: #fbfbf8; }
    main { padding: ${cabecera ? 64 : 16}px 20px ${pie ? 60 : 24}px; }
    h2 { font-size: 21px; margin: 22px 0 8px; color: #23356b; }
    p { margin: 0 0 12px; }
    .foto { position: relative; height: 150px; border-radius: 10px; color: #fff; font-weight: 600; padding: 14px; margin: 12px 0; }
    .foto span { position: absolute; width: 46px; height: 46px; border-radius: 50%; background: #fff8; }
    table { border-collapse: collapse; width: 100%; margin: 8px 0 14px; }
    td, th { border: 1px solid #c9c9d6; padding: 4px 8px; text-align: left; }
    th { background: #e9ecf6; }
    .cabecera { top: 0; left: 0; right: 0; height: 52px; background: #23356b; color: #fff; display: flex;
      align-items: center; padding: 0 18px; font-weight: 600; z-index: 2; }
    .fija { position: fixed; }
    .pegajosa { position: sticky; }
    .pie { position: fixed; left: 0; right: 0; bottom: 0; height: 44px; background: #2b2b2b; color: #ffd84a;
      display: flex; align-items: center; justify-content: space-between; padding: 0 18px; z-index: 2; }
    .pie b { background: #ffd84a; color: #222; padding: 4px 10px; border-radius: 6px; }
  </style></head><body>${cabecera}<main>${bloques.join("\n")}</main>${pie}</body></html>`;
}

/** Desde `desde` hasta el final, con esos pasos uno detrás de otro, y el final siempre. */
function pasos(desde, fin, saltos) {
  const salen = [];
  for (let y = desde, i = 0; y < fin; y += saltos[i++ % saltos.length]) salen.push(y);
  salen.push(fin);
  return salen;
}

const ESCENAS = [
  {
    nombre: "sin-fijos",
    escala: 1,
    vista: { width: 480, height: 400 },
    html: pagina({ semilla: 11 }),
    // Un paso de un píxel, una foto repetida (520 dos veces) y una hacia atrás (430).
    posiciones: (fin) => [0, 90, 260, 261, 520, 520, 430, ...pasos(800, fin, [280, 190, 310])],
  },
  {
    nombre: "cabecera-y-pie",
    escala: 1,
    vista: { width: 480, height: 400 },
    html: pagina({
      semilla: 23,
      cabecera: '<div class="cabecera fija">winshotx · cabecera fija</div>',
      pie: '<div class="pie">Usamos cookies para nada <b>Vale</b></div>',
    }),
    posiciones: (fin) => pasos(0, fin, [150, 150, 110, 150, 140, 180, 120]),
  },
  {
    nombre: "escala-125",
    escala: 1.25,
    vista: { width: 400, height: 320 },
    html: pagina({
      semilla: 37,
      cabecera: '<div class="cabecera pegajosa">Cabecera pegajosa al 125 %</div>',
    }).replace("padding: 64px", "padding: 12px"),
    // Lo que bajan una y dos muescas de la rueda: 100 y 200 píxeles de la página, que al
    // 125 % son 125 y 250 de la pantalla.
    posiciones: (fin) => pasos(0, fin, [100, 200]),
  },
];

/** El PNG apretado del todo, sin perder nada: estas fotos viven en el repositorio. */
async function guardar(png, ruta) {
  await sharp(png).png({ compressionLevel: 9, adaptiveFiltering: true }).toFile(ruta);
}

const navegador = await chromium.launch({ channel: "msedge", headless: true });
let total = 0;
try {
  for (const escena of ESCENAS) {
    const carpeta = join(en, escena.nombre);
    rmSync(carpeta, { recursive: true, force: true });
    mkdirSync(carpeta, { recursive: true });
    const contexto = await navegador.newContext({
      viewport: escena.vista,
      deviceScaleFactor: escena.escala,
    });
    const hoja = await contexto.newPage();
    await hoja.setContent(escena.html, { waitUntil: "load" });
    const maximo = await hoja.evaluate(
      () => document.documentElement.scrollHeight - window.innerHeight,
    );
    const hechas = [];
    for (const [i, y] of escena.posiciones(maximo).entries()) {
      await hoja.evaluate((y) => {
        window.scrollTo({ top: y, behavior: "instant" });
        return new Promise((listo) => requestAnimationFrame(() => requestAnimationFrame(listo)));
      }, y);
      const real = await hoja.evaluate(() => window.scrollY);
      hechas.push(real);
      await guardar(await hoja.screenshot(), join(carpeta, `${String(i).padStart(2, "0")}.png`));
    }
    // La página entera, con una ventana tan alta como ella. La «página entera» de Edge
    // (`fullPage`) no vale aquí: deja lo fijo donde estaba en la primera vista, así que la
    // barra de cookies le sale a media página en vez de abajo del todo.
    const alto = await hoja.evaluate(() => document.documentElement.scrollHeight);
    await hoja.setViewportSize({ width: escena.vista.width, height: alto });
    await hoja.evaluate(() => window.scrollTo({ top: 0, behavior: "instant" }));
    await guardar(await hoja.screenshot(), join(carpeta, "entera.png"));
    writeFileSync(
      join(carpeta, "posiciones.json"),
      JSON.stringify({ escala: escena.escala, vista: escena.vista, posiciones: hechas }, null, 2) + "\n",
    );
    total += hechas.length;
    console.log(`${escena.nombre}: ${hechas.length} fotos, bajando hasta ${maximo} px`);
    await contexto.close();
  }
} finally {
  await navegador.close();
}
console.log(`listo: ${total} fotos en ${en}`);
