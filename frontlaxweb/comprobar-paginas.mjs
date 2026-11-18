/**
 * Mira, en el HTML ya generado, que cada página nueva cumpla lo que le pide un buscador.
 *
 *   node frontlaxweb/comprobar-paginas.mjs
 *
 * También la corre generar-en.mjs al acabar, y entonces el despliegue se para si algo falla:
 * entre 300 y 600 palabras, un solo h1 distinto del de la portada, title, descripción y
 * canonical propios, hreflang de ida y vuelta, JSON-LD que se lee con su FAQPage y su
 * BreadcrumbList, la dirección en el sitemap, y ninguna raya larga en lo que se lee.
 */
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { TODAS } from "./paginas.mjs";

const BASE = "https://winshotx.com/";
const MIN_PALABRAS = 300;
const MAX_PALABRAS = 600;
const MAX_DESCRIPCION = 165;

const uno = (html, patron) => html.match(patron)?.[1] ?? "";
const texto = (html) =>
  html
    .replace(/<script[\s\S]*?<\/script>/g, " ")
    .replace(/<svg[\s\S]*?<\/svg>/g, " ")
    .replace(/<[^>]+>/g, " ")
    .replaceAll("&nbsp;", " ")
    .replace(/&[a-z]+;/g, " ");
const palabras = (html) => texto(html).split(/\s+/).filter((p) => /[\p{L}\p{N}]/u.test(p)).length;

export async function comprobarPaginas(aqui) {
  const fallos = [];
  const sitemap = await readFile(join(aqui, "sitemap.xml"), "utf8");
  const h1Portada = (L) => readFile(join(aqui, ...(L === "en" ? ["en"] : []), "index.html"), "utf8").then((h) => uno(h, /<h1[^>]*>([\s\S]*?)<\/h1>/));
  const portadas = { es: await h1Portada("es"), en: await h1Portada("en") };
  const vistos = { title: new Map(), description: new Map(), h1: new Map() };
  const resumen = [];

  for (const p of TODAS) {
    for (const L of ["es", "en"]) {
      const ruta = L === "es" ? p.es : `en/${p.en}`;
      const url = `${BASE}${ruta}/`;
      const mal = (que) => fallos.push(`/${ruta}/: ${que}`);
      const html = await readFile(join(aqui, ...ruta.split("/"), "index.html"), "utf8");
      const main = uno(html, /<main[^>]*>([\s\S]*?)<\/main>/);

      const n = palabras(main);
      resumen.push([ruta, n]);
      if (n < MIN_PALABRAS || n > MAX_PALABRAS) mal(`tiene ${n} palabras (entre ${MIN_PALABRAS} y ${MAX_PALABRAS})`);

      const h1s = [...html.matchAll(/<h1[^>]*>([\s\S]*?)<\/h1>/g)].map((m) => m[1]);
      if (h1s.length !== 1) mal(`tiene ${h1s.length} h1`);
      if (h1s[0] === portadas[L]) mal("su h1 es el de la portada");

      const propios = {
        title: uno(html, /<title[^>]*>([\s\S]*?)<\/title>/),
        description: uno(html, /<meta name="description" content="([^"]*)"/),
        h1: h1s[0] ?? "",
      };
      for (const [que, valor] of Object.entries(propios)) {
        if (!valor.trim()) mal(`no tiene ${que}`);
        const otra = vistos[que].get(valor);
        if (otra) mal(`su ${que} es el mismo que el de /${otra}/`);
        vistos[que].set(valor, ruta);
      }
      if (propios.description.length > MAX_DESCRIPCION) mal(`la descripción mide ${propios.description.length} caracteres (${MAX_DESCRIPCION} como mucho)`);

      if (uno(html, /<link rel="canonical" href="([^"]*)"/) !== url) mal("el canonical no apunta a sí misma");
      if (uno(html, /<html lang="([^"]*)"/) !== L) mal("el idioma de <html> no es el suyo");
      const hreflang = (h) => uno(html, new RegExp(`<link rel="alternate" hreflang="${h}" href="([^"]*)"`));
      if (hreflang("es") !== `${BASE}${p.es}/`) mal("el hreflang es no va a la página española");
      if (hreflang("en") !== `${BASE}en/${p.en}/`) mal("el hreflang en no va a la página inglesa");
      if (hreflang("x-default") !== `${BASE}${p.es}/`) mal("el hreflang x-default no va a la página española");

      const tipos = [];
      for (const [, json] of html.matchAll(/<script type="application\/ld\+json"[^>]*>([\s\S]*?)<\/script>/g)) {
        try {
          tipos.push(JSON.parse(json)["@type"]);
        } catch (e) {
          mal(`un JSON-LD no se lee: ${e.message}`);
        }
      }
      for (const tipo of ["BreadcrumbList", "FAQPage"]) if (!tipos.includes(tipo)) mal(`le falta el JSON-LD ${tipo}`);

      if (!sitemap.includes(`<loc>${url}</loc>`)) mal("no está en sitemap.xml");
      if (/[—–]/.test(texto(main))) mal("lleva una raya larga en el texto");
      if (L === "en" && /\sdata-(en|html|aria-en|src-en|alt-en)\b/.test(html)) mal("le quedan marcas sin traducir");
    }
  }
  return { fallos, resumen };
}

// Suelta, desde la terminal: enseña las palabras de cada página.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const { fallos, resumen } = await comprobarPaginas(dirname(fileURLToPath(import.meta.url)));
  for (const [ruta, n] of resumen) console.log(`${String(n).padStart(4)} palabras  /${ruta}/`);
  for (const f of fallos) console.error(`MAL ${f}`);
  console.log(fallos.length ? `\n${fallos.length} fallos` : `\n${resumen.length} páginas en orden`);
  process.exit(fallos.length ? 1 : 0);
}
