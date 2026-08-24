/**
 * Genera en/index.html a partir de index.html.
 *
 * Cada idioma necesita su propia URL: un botón que cambia los textos con JavaScript deja a
 * Google y a los rastreadores de las IA viendo solo la versión española. Los textos ingleses
 * ya viven en los atributos data-en de la página, así que aquí solo hay que aplicarlos.
 *
 *   node frontlaxweb/generar-en.mjs
 */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const BASE = "https://mun1to.github.io/winshotx/";

/** Los data-en llevan las marcas escapadas: aquí se devuelven a su forma. */
const desescapar = (t) =>
  t
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&quot;", '"')
    .replaceAll("&#39;", "'")
    .replaceAll("&amp;", "&");

let html = await readFile(join(aqui, "index.html"), "utf8");

// 1. el contenido de cada elemento traducido pasa a ser su data-en
const traducible = /<([a-z0-9]+)((?:[^>"]|"[^"]*")*?\sdata-en="([^"]*)"(?:[^>"]|"[^"]*")*)>([\s\S]*?)<\/\1>/g;
html = html.replace(traducible, (_todo, tag, atributos, ingles) => {
  const limpios = atributos
    .replace(/\sdata-en="(?:[^"]*)"/, "")
    .replace(/\sdata-html(?==|\s|$)/, "");
  return `<${tag}${limpios}>${desescapar(ingles)}</${tag}>`;
});

// 2. y los tooltips
html = html.replace(/\stitle="[^"]*"((?:[^>"]|"[^"]*")*?)\sdata-titulo-en="([^"]*)"/g,
  (_t, medio, ingles) => ` title="${ingles}"${medio}`);
html = html.replace(/\sdata-titulo-en="[^"]*"/g, "");

// 3. idioma de la página, rutas relativas y enlaces cruzados
html = html.replace('<html lang="es">', '<html lang="en">');
html = html.replace('href="logo.svg"', 'href="../logo.svg"');
html = html.replace('href="estilos.css"', 'href="../estilos.css"');
html = html.replace('src="demo.js"', 'src="../demo.js"');
html = html.replace(
  '<a class="idioma" id="idioma" href="en/" hreflang="en" lang="en">English</a>',
  '<a class="idioma" id="idioma" href="../" hreflang="es" lang="es">Español</a>',
);

// 4. lo que apunta a sí misma: canónica, idioma social y la ficha de datos
html = html.replace(`<link rel="canonical" href="${BASE}">`, `<link rel="canonical" href="${BASE}en/">`);
html = html.replace(`<meta property="og:url" content="${BASE}">`, `<meta property="og:url" content="${BASE}en/">`);
html = html.replace('<meta property="og:locale" content="es_ES">', '<meta property="og:locale" content="en_US">');
html = html.replace(
  '<meta property="og:locale:alternate" content="en_US">',
  '<meta property="og:locale:alternate" content="es_ES">',
);
html = html.replace(`"url": "${BASE}"`, `"url": "${BASE}en/"`);
html = html.replaceAll(`${BASE}social.png`, `${BASE}social-en.png`);

// 5. y las descripciones, que viven en atributos y no llevan data-en
const enIngles = {
  'content="Alternativa libre a la Herramienta de Recortes de Windows: captura de región, grabación en GIF y MP4 y editor fotograma a fotograma. Abre la selección en 28 ms y gasta 33 MB. Instalador de 2,2 MB, sin cuenta y sin nube."':
    'content="Free and open source alternative to the Windows Snipping Tool: region capture, GIF and MP4 recording and a frame by frame editor. Opens the selection in 28 ms and uses 33 MB. A 2.2 MB installer, no account and no cloud."',
  'content="winshotx · captura y grabación de pantalla para Windows"':
    'content="winshotx · screenshots and screen recording for Windows"',
  'content="La Herramienta de Recortes tarda 920 ms y gasta 253 MB. Esta tarde 28 ms y gasta 33 MB, graba GIF y trae editor. 2,2 MB, código abierto."':
    'content="The Snipping Tool takes 920 ms and 253 MB. This one takes 28 ms and 33 MB, records GIF and ships an editor. 2.2 MB, open source."',
  'content="La Herramienta de Recortes tarda 920 ms y gasta 253 MB. Esta tarde 28 ms y gasta 33 MB. 2,2 MB, código abierto."':
    'content="The Snipping Tool takes 920 ms and 253 MB. This one takes 28 ms and 33 MB. 2.2 MB, open source."',
  'content="winshotx: 28 ms contra 920 ms de la Herramienta de Recortes"':
    'content="winshotx: 28 ms against the Snipping Tool\'s 920 ms"',
};
for (const [es, en] of Object.entries(enIngles)) html = html.replace(es, en);

await mkdir(join(aqui, "en"), { recursive: true });
await writeFile(join(aqui, "en", "index.html"), html, "utf8");

const pendientes = (html.match(/data-en=/g) ?? []).length;
console.log(`en/index.html generado${pendientes ? ` (quedan ${pendientes} sin traducir)` : ""}`);
