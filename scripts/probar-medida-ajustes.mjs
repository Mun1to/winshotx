/**
 * Comprueba que la ventana de ajustes mide lo que mide su pestaña (S24, 14-nov-2026).
 *
 *   pnpm build
 *   node scripts/probar-medida-ajustes.mjs
 *   node scripts/probar-medida-ajustes.mjs --fotos=<carpeta>   deja la foto de cada pestaña
 *
 * Usa Edge sin ventana (el motor de WebView2) con reloj de verdad, que hace falta: la ventana
 * se reajusta cuando el contenido cambia de tamaño, y eso lo avisa el navegador al pintar.
 * Levanta `ver-ventana.mjs --servir`, cuyo Tauri de mentira apunta lo que pide la página
 * (`setSize`), y en cada caso:
 *
 *  1. pulsa cada pestaña, en orden y luego al revés (crecer y encoger);
 *  2. lee el alto que ha pedido la ventana y pone el navegador a ese alto, como haría Windows;
 *  3. mira que no le sobra hueco debajo de las tarjetas (menos de 2 px) y que no le hace
 *     falta rueda; y que, puesta a su medida, no pide otro alto (si no, bailaría).
 *
 * Casos: 100, 125 y 150 % de escala; español, inglés y chino (las frases miden distinto); el
 * anillo encendido (Grabar tiene más filas); y una pantalla baja, un portátil de 1366 x 768 al
 * 125 % (566 px lógicos útiles), donde la ventana no puede salirse: se queda en la zona útil,
 * con la rueda dentro y el desvanecido de abajo diciendo que sigue.
 *
 * El historial es la excepción a propósito: crece hasta 760 y después baja con la rueda.
 *
 * Sale con 0 si todo cuadra y con 1 si algo no.
 */
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const fotos = process.argv.find((a) => a.startsWith("--fotos="))?.slice(8) ?? null;
if (fotos) mkdirSync(fotos, { recursive: true });

/** Lo que mide la barra de título de mentira de `ver-ventana.mjs`. */
const MARCO = 32;
const NOMBRES = ["capturar", "grabar", "historial", "teclas", "app"];

const CASOS = [
  { nombre: "100 %", escala: 1, zona: 1032, idioma: "es" },
  { nombre: "125 %", escala: 1.25, zona: 816, idioma: "es" },
  { nombre: "150 %", escala: 1.5, zona: 672, idioma: "es" },
  { nombre: "inglés", escala: 1, zona: 1032, idioma: "en" },
  { nombre: "chino", escala: 1, zona: 1032, idioma: "zh" },
  { nombre: "anillo encendido", escala: 1, zona: 1032, idioma: "es", anillo: true },
  { nombre: "portátil 1366 x 768 al 125 %", escala: 1.25, zona: 566, idioma: "es" },
];

let mal = 0;
const navegador = await chromium.launch({ channel: "msedge", headless: true });

for (const caso of CASOS) {
  console.log(`\n${caso.nombre}`);
  const servidor = spawn(
    "node",
    [
      join(RAIZ, "scripts", "ver-ventana.mjs"),
      "--servir",
      `--zona=${caso.zona}`,
      `--idioma=${caso.idioma}`,
      ...(caso.anillo ? ["--anillo"] : []),
    ],
    { cwd: RAIZ },
  );
  const url = await new Promise((listo) => servidor.stdout.on("data", (d) => listo(String(d).trim())));
  const contexto = await navegador.newContext({
    viewport: { width: 840, height: 760 },
    deviceScaleFactor: caso.escala,
    reducedMotion: "reduce",
  });
  const pagina = await contexto.newPage();
  const errores = [];
  pagina.on("pageerror", (e) => errores.push(String(e)));
  await pagina.goto(url);
  await pagina.waitForSelector("[data-contenido-ajustes]");
  await pagina.waitForTimeout(400);

  const pedido = () =>
    pagina.evaluate(() => {
      const m = document.querySelector('meta[name="winshotx-medida"]');
      if (!m) return null;
      const [, alto, tipo, dpr] = m.content.split(",");
      return tipo === "Physical" ? Number(alto) / Number(dpr) : Number(alto);
    });
  const medir = () =>
    pagina.evaluate(() => {
      const h = document.querySelector("[data-scroll-ajustes]");
      const c = document.querySelector("[data-contenido-ajustes]");
      return {
        rueda: Math.round((h.scrollHeight - h.clientHeight) * 10) / 10,
        hueco: Math.round((h.clientHeight - c.getBoundingClientRect().height) * 10) / 10,
        desvanecido: !!document.querySelector(".bg-gradient-to-t.from-lienzo"),
      };
    });

  // Lo más que cabe por dentro en esa pantalla, en px lógicos.
  const tope = Math.floor((caso.zona * caso.escala - Math.round(MARCO * caso.escala)) / caso.escala);
  for (const i of [0, 1, 2, 3, 4, 3, 1, 0]) {
    const nombre = NOMBRES[i];
    await pagina.locator("header nav button").nth(i).click();
    await pagina.waitForTimeout(250);
    const pide = await pedido();
    const alto = Math.ceil(pide - 0.01);
    await pagina.setViewportSize({ width: 840, height: alto });
    await pagina.waitForTimeout(250);
    const m = await medir();
    const estable = Math.abs((await pedido()) - pide) < 0.5;
    const apretada = alto >= tope - 1;
    const enHistorial = nombre === "historial";
    let bien = estable && pide > 0;
    let nota = "";
    if (apretada) {
      // No cabe: tiene que llevar su rueda y decirlo con el desvanecido.
      bien &&= m.rueda > 0 ? m.desvanecido : !m.desvanecido;
      nota = "llega al tope de la pantalla";
    } else if (!enHistorial) {
      // Hasta 2 px de hueco: la ventana pide en píxeles físicos redondeando hacia arriba, y
      // aquí el navegador solo se puede poner en px de CSS enteros. A 125 % eso suma 1,3 px.
      bien &&= m.rueda <= 0 && m.hueco < 2 && !m.desvanecido;
    }
    if (!bien) mal += 1;
    console.log(
      `  ${bien ? "BIEN" : "MAL "} ${nombre.padEnd(9)} ${String(Math.round(pide)).padStart(4)} px  rueda ${m.rueda}  hueco ${m.hueco}${m.desvanecido ? "  desvanecido" : ""}${estable ? "" : "  BAILA"}${nota ? `  (${nota})` : ""}`,
    );
    if (fotos && caso.idioma === "es" && !caso.anillo) {
      await pagina.screenshot({ path: join(fotos, `${caso.nombre.replace(/[^\w]+/g, "-")}-${nombre}.png`) });
    }
  }
  if (errores.length) {
    mal += 1;
    console.log("  MAL  errores de la página:", errores);
  }
  await contexto.close();
  servidor.kill();
}

await navegador.close();
console.log(mal ? `\n${mal} MAL` : "\ntodo BIEN");
process.exit(mal ? 1 : 0);
