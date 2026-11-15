/**
 * Genera la página de novedades a partir de novedades/versiones.json.
 *
 *   node frontlaxweb/generar-novedades.mjs              escribe novedades/index.html
 *   node frontlaxweb/generar-novedades.mjs --comprobar  además, mira en GitHub que no falte ninguna versión
 *
 * Después hay que correr generar-en.mjs, que saca la inglesa (en/changelog/) de los data-en.
 *
 * De dónde sale versiones.json: de las notas de cada release de GitHub, del campo «novedades»
 * de la ficha de la Microsoft Store (packaging/store/ficha.json, en inglés y en español) y,
 * para las versiones que no traían notas, de sus commits. Cada versión nueva se añade arriba,
 * a mano, al publicarla: es una frase por cambio, y nadie la escribe mejor que quien la
 * acaba de hacer. `--comprobar` falla si en GitHub hay una versión que aquí no está.
 *
 * Una entrada puede llevar `"foto": "novedades/0.3.8.png"` (ruta dentro de frontlaxweb) y
 * `"foto_alt"` con su texto en los dos idiomas; hoy ninguna la lleva porque los releases no
 * traen capturas, y una captura que no es de esa versión sería mentir.
 */
import { readFile, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { leerPortada, cromoPara, spritePara } from "./cromo.mjs";

const aqui = dirname(fileURLToPath(import.meta.url));
const BASE = "https://winshotx.com/";
const { versiones } = JSON.parse(await readFile(join(aqui, "novedades", "versiones.json"), "utf8"));

// ---------------------------------------------------------------- comprobar contra GitHub
if (process.argv.includes("--comprobar")) {
  // En esta máquina hay variables que fuerzan los colores, y `gh` los mete dentro del JSON.
  const env = { ...process.env, CLICOLOR_FORCE: "0", FORCE_COLOR: "0", NO_COLOR: "1" };
  const salida = execFileSync("gh", ["api", "repos/Mun1to/winshotx/releases?per_page=100", "--paginate", "--jq", ".[] | select(.draft | not) | .tag_name"], { env, encoding: "utf8" });
  const publicadas = salida.split(/\r?\n/).filter(Boolean).map((t) => t.replace(/^v/, ""));
  const num = (v) => v.split(".").map(Number);
  const menor = (a, b) => { const [x, y] = [num(a), num(b)]; for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] < y[i]; return false; };
  const cubierta = (v) => versiones.some((e) => (e.desde ? !menor(v, e.desde) && !menor(e.version, v) : e.version === v));
  const faltan = publicadas.filter((v) => !cubierta(v));
  if (faltan.length) {
    console.error(`En GitHub hay versiones que la página de novedades no cuenta: ${faltan.join(", ")}`);
    console.error("Añade su entrada arriba del todo en frontlaxweb/novedades/versiones.json.");
    process.exit(1);
  }
  console.log(`Las ${publicadas.length} versiones publicadas en GitHub están en la página.`);
}

// ---------------------------------------------------------------- piezas
const escapar = (t) => t.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
// `**así**` es la palabra clave en negrita; lo demás se escapa.
const conNegrita = (t) => escapar(t).replace(/\*\*(.+?)\*\*/g, "<b>$1</b>");
const MESES_ES = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
const MESES_EN = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const fechaEs = (f) => { const [a, m, d] = f.split("-").map(Number); return `${d} de ${MESES_ES[m - 1]} de ${a}`; };
const fechaEn = (f) => { const [a, m, d] = f.split("-").map(Number); return `${d} ${MESES_EN[m - 1]} ${a}`; };

/** Un elemento con su español dentro y su inglés en data-en, que es lo que lee generar-en.mjs. */
const bilingue = (tag, es, en, atributos = "") => {
  const html = /<[a-z]/.test(es) || /<[a-z]/.test(en);
  return `<${tag}${atributos}${html ? " data-html" : ""} data-en="${escapar(en)}">${es}</${tag}>`;
};

const entrada = (v) => {
  const id = `v${v.version.replaceAll(".", "-")}`;
  const enlace = `https://github.com/Mun1to/winshotx/releases/tag/v${v.version}`;
  const nombreEs = v.desde ? `Versiones ${v.desde} a ${v.version}` : `Versión ${v.version}`;
  const nombreEn = v.desde ? `Versions ${v.desde} to ${v.version}` : `Version ${v.version}`;
  const lineaEs = `<a href="${enlace}" rel="noopener">${nombreEs}</a> · ${fechaEs(v.fecha)}`;
  const lineaEn = `<a href="${enlace}" rel="noopener">${nombreEn}</a> · ${fechaEn(v.fecha)}`;
  const puntos = v.es.puntos.map((p, i) => `    ${bilingue("li", conNegrita(p), conNegrita(v.en.puntos[i]))}`).join("\n");
  const foto = v.foto
    ? `\n  <img class="foto-version" src="../${v.foto}" alt="${escapar(v.foto_alt?.es ?? "")}" loading="lazy">`
    : "";
  return `<section id="${id}">
  ${bilingue("h2", escapar(v.es.titulo), escapar(v.en.titulo))}
  ${bilingue("p", lineaEs, lineaEn, ' class="version"')}${foto}
  <ul class="lista">
${puntos}
  </ul>
</section>`;
};

// ---------------------------------------------------------------- la página
// La barra, el pie y los dibujos salen de la portada (cromo.mjs), como en las demás páginas.
const portada = await leerPortada(aqui);
const { barra: menu, pie } = cromoPara(portada, "novedades");
const privacidad = await readFile(join(aqui, "privacidad", "index.html"), "utf8");
const css = privacidad.match(/href="\.\.\/estilos\.css\?v=[a-f0-9]+"/)[0];
const leer = privacidad.match(/src="\.\.\/leer\.js\?v=[a-f0-9]+"/)[0];

const ultima = versiones[0];
const descripcion = "Lo que trae cada versión de winshotx, de la más nueva a la primera, con el enlace a su publicación en GitHub.";
const idDe = (v) => `v${v.version.replaceAll(".", "-")}`;
const indice = versiones
  .map((v) => {
    const nombre = v.desde ? `${v.desde} a ${v.version}` : v.version;
    const nombreEn = v.desde ? `${v.desde} to ${v.version}` : v.version;
    return `        <a href="#${idDe(v)}"${nombre === nombreEn ? "" : ` data-en="${nombreEn}"`}>${nombre}</a>`;
  })
  .join("\n");

let html = `<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title data-en="What's new in winshotx, version by version">Novedades de winshotx, versión a versión</title>
<meta name="description" content="${descripcion}">
<link rel="canonical" href="${BASE}novedades/">
<link rel="alternate" hreflang="es" href="${BASE}novedades/">
<link rel="alternate" hreflang="en" href="${BASE}en/changelog/">
<link rel="alternate" hreflang="x-default" href="${BASE}novedades/">
<meta name="color-scheme" content="light dark">
<meta name="theme-color" content="#0b0b0d" media="(prefers-color-scheme: dark)">
<meta name="theme-color" content="#f7f7f9" media="(prefers-color-scheme: light)">
<meta property="og:type" content="article">
<meta property="og:site_name" content="winshotx">
<meta property="og:locale" content="es_ES">
<meta property="og:locale:alternate" content="en_US">
<meta property="og:url" content="${BASE}novedades/">
<meta property="og:title" content="Novedades de winshotx">
<meta property="og:description" content="${descripcion}">
<meta property="og:image" content="${BASE}social.png?v=4">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="winshotx, «Crop the screen before you blink»: 114 ms, instalador de 3,00 MB, gratis y de código abierto, junto a su ventana de ajustes">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="Novedades de winshotx">
<meta name="twitter:description" content="${descripcion}">
<meta name="twitter:image" content="${BASE}social.png?v=4">
<link rel="icon" href="../favicon.ico?v=2" sizes="any">
<link rel="icon" href="../logo.svg?v=2" type="image/svg+xml">
<link rel="apple-touch-icon" href="../apple-touch-icon.png?v=2">
<script>
// El dial de movimiento de la casa, igual que en la portada (ver index.html): sin él, el
// enlace «Menos animaciones» del pie no hacía nada en esta página.
(function () {
  var h = document.documentElement;
  var q = new URLSearchParams(location.search).get("motion");
  var reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  var gain = q === "off" ? 0 : q === "full" ? 1 : reduce ? 0.25 : 1;
  h.style.setProperty("--motion-gain", String(gain));
  h.classList.toggle("motion", gain > 0);
  h.classList.toggle("quieto", gain === 0);
})();
</script>
<link rel="stylesheet" ${css}>
<script type="application/ld+json">
{
  "@context": "https://schema.org",
  "@type": "BreadcrumbList",
  "itemListElement": [
    { "@type": "ListItem", "position": 1, "name": "winshotx", "item": "${BASE}" },
    { "@type": "ListItem", "position": 2, "name": "Novedades", "item": "${BASE}novedades/" }
  ]
}
</script>
</head>
<body>

<!-- Esta página la escribe generar-novedades.mjs a partir de novedades/versiones.json: no se
     edita a mano, que se pisa la próxima vez. -->
<svg style="display:none" xmlns="http://www.w3.org/2000/svg"></svg>

${menu}

<main class="leer novedades">
  <div class="env">
    <header class="leer-cabeza">
      ${bilingue("h1", "Novedades", "What's new")}
      ${bilingue(
        "p",
        `Lo que trae cada versión, de la más nueva a la primera. La app se actualiza sola desde <b>Ajustes</b>, y la última es la <b>${ultima.version}</b>.`,
        `What each version brings, newest first. The app updates itself from <b>Settings</b>, and the latest is <b>${ultima.version}</b>.`,
        ' class="entradilla"',
      )}
    </header>

    <div class="leer-rejilla con-lado">
      <nav class="leer-indice" aria-label="Versiones" data-aria-en="Versions">
${indice}
      </nav>

      <div class="texto">
${versiones.map(entrada).join("\n\n")}
      </div>

      <figure class="leer-lado">
        <img src="../img/editor-es.webp" data-src-en="../img/editor-en.webp" width="1280" height="800" loading="lazy" alt="El editor de winshotx con una captura sobre el fondo Océano del Estudio" data-alt-en="The winshotx editor with a screenshot on the Studio's Ocean background">
        ${bilingue(
          "figcaption",
          "El editor, con una captura sobre el fondo «Océano» del Estudio. Cada versión llega sola: cuando sale una nueva, <b>Ajustes</b> dice «Actualizar ahora».",
          "The editor, with a screenshot on the Studio's «Ocean» background. Each version arrives by itself: when a new one is out, <b>Settings</b> says «Update now».",
        )}
      </figure>
    </div>
  </div>
</main>

${pie}

<script ${leer}></script>
</body>
</html>
`;
html = html.replace(/<svg style="display:none"[^>]*><\/svg>/, () => spritePara(html, portada.simbolos, "novedades"));
await writeFile(join(aqui, "novedades", "index.html"), html, "utf8");
console.log(`novedades/index.html: ${versiones.length} entradas, la última la ${ultima.version}`);

// ---------------------------------------------------------------- las tres últimas, en la portada
// La portada cuenta las tres últimas versiones entre dos marcas; cada una lleva a su sitio
// en la página de novedades.
const tarjeta = (v) => {
  const nombreEs = v.desde ? `Versiones ${v.desde} a ${v.version}` : `Versión ${v.version}`;
  const nombreEn = v.desde ? `Versions ${v.desde} to ${v.version}` : `Version ${v.version}`;
  return `      <article class="novedad">
        ${bilingue("p", `${nombreEs} · ${fechaEs(v.fecha)}`, `${nombreEn} · ${fechaEn(v.fecha)}`, ' class="cuando"')}
        <h3><a href="novedades/#${idDe(v)}"${` data-en="${escapar(v.en.titulo)}"`}>${escapar(v.es.titulo)}</a></h3>
        ${bilingue("p", conNegrita(v.es.puntos[0]), conNegrita(v.en.puntos[0]))}
      </article>`;
};
const rutaPortada = join(aqui, "index.html");
const portadaHtml = await readFile(rutaPortada, "utf8");
const bloque = `<!-- novedades:desde (lo escribe generar-novedades.mjs a partir de versiones.json) -->
    <div class="novedades-portada">
${versiones.slice(0, 3).map(tarjeta).join("\n")}
    </div>
    <!-- novedades:hasta -->`;
const nuevaPortada = portadaHtml.replace(/<!-- novedades:desde[\s\S]*?<!-- novedades:hasta -->/, () => bloque);
if (nuevaPortada === portadaHtml && !portadaHtml.includes(bloque)) throw new Error("la portada no tiene las marcas novedades:desde y novedades:hasta");
if (nuevaPortada !== portadaHtml) {
  await writeFile(rutaPortada, nuevaPortada, "utf8");
  console.log("index.html: las tres últimas versiones");
}
