/**
 * Cambia el dominio de la web en todos los sitios donde esta escrito, de una vez.
 *
 *   node frontlaxweb/cambiar-dominio.mjs winshotx.com
 *   node frontlaxweb/cambiar-dominio.mjs mun1to.github.io/winshotx   (para volver atras)
 *
 * La URL vive en la canonica, en los hreflang, en Open Graph, en el JSON-LD, en robots.txt,
 * en el sitemap, en llms.txt, en los dos README y en el generador del ingles. Cambiarla a mano
 * en nueve archivos es como se olvida uno y Google acaba indexando dos sitios distintos.
 *
 * Despues hay que hacer dos cosas mas, que no se pueden desde aqui:
 *   1. En Cloudflare, un CNAME de winshotx.com a mun1to.github.io (y otro de www).
 *   2. En Ajustes del repo, Pages, poner el dominio y esperar al certificado.
 * El archivo CNAME que necesita GitHub Pages si lo escribe este script.
 */
import { readFile, writeFile, readdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const raiz = join(aqui, "..");

const nuevo = process.argv[2]?.replace(/^https?:\/\//, "").replace(/\/$/, "");
if (!nuevo) {
  console.error("Falta el dominio. Ejemplo: node frontlaxweb/cambiar-dominio.mjs winshotx.com");
  process.exit(1);
}

const BASE_NUEVA = `https://${nuevo}/`;
const CUALQUIER_BASE = /https:\/\/(?:mun1to\.github\.io\/winshotx|winshotx\.com|www\.winshotx\.com)\//g;

const archivos = [
  "frontlaxweb/index.html",
  "frontlaxweb/docs/index.html",
  "frontlaxweb/generar-en.mjs",
  "frontlaxweb/robots.txt",
  "frontlaxweb/sitemap.xml",
  "frontlaxweb/llms.txt",
  "README.md",
  "README.es.md",
];

let tocados = 0;
for (const relativo of archivos) {
  const ruta = join(raiz, relativo);
  const antes = await readFile(ruta, "utf8");
  const despues = antes.replace(CUALQUIER_BASE, BASE_NUEVA);
  if (antes !== despues) {
    await writeFile(ruta, despues, "utf8");
    const veces = (antes.match(CUALQUIER_BASE) ?? []).length;
    console.log(`  ${relativo}  (${veces})`);
    tocados++;
  }
}

// El dominio propio necesita este archivo en la raiz del sitio publicado.
const cname = join(aqui, "CNAME");
if (nuevo.includes(".github.io")) {
  try {
    const { unlink } = await import("node:fs/promises");
    await unlink(cname);
    console.log("  frontlaxweb/CNAME borrado");
  } catch {
    // no habia, perfecto
  }
} else {
  await writeFile(cname, `${nuevo}\n`, "utf8");
  console.log(`  frontlaxweb/CNAME  ->  ${nuevo}`);
}

console.log(`\n${tocados} archivos apuntando a ${BASE_NUEVA}`);
console.log("Ahora: node frontlaxweb/generar-en.mjs, y revisa el sitemap antes de commitear.");

// Aviso de lo que queda fuera del alcance de este script.
const winget = join(raiz, "packaging", "winget");
try {
  const versiones = await readdir(winget);
  console.log(
    `\nOjo: los manifiestos de winget (${versiones.filter((v) => /^\d/.test(v)).join(", ")}) ` +
      "llevan la URL antigua. No se tocan: los enviados ya estan en winget-pkgs y solo se " +
      "cambian creando la carpeta de la version siguiente.",
  );
} catch {
  // sin manifiestos todavia
}
