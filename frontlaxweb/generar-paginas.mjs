/**
 * Escribe las páginas que contestan a una búsqueda: las tres por problema (textos en
 * paginas.mjs) y una «alternativa a» por cada programa de la comparativa, sacada entera de
 * comparativa.mjs, así que al cambiar una cifra cambian la portada y su página a la vez.
 *
 * La corre generar-en.mjs antes que nada, que es quien saca después la versión inglesa de los
 * data-en y pone las huellas: basta con correr `node frontlaxweb/generar-en.mjs`.
 *
 * También reparte estas páginas por donde tienen que estar: el pie de la portada, sitemap.xml,
 * llms.txt y los enlaces ingleses de la 404.
 */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { leerPortada, cromoPara, spritePara } from "./cromo.mjs";
import { HERRAMIENTAS, FILAS, TEXTOS, COMPROBADO } from "./comparativa.mjs";
import { el, dos, nombre, txt, minuscula, lista, escapar, reparto, frenteDos, medidas } from "./generar-comparativa.mjs";
import { PROBLEMAS, ALTERNATIVAS, TODAS, ACTUALIZADO, DESCARGA } from "./paginas.mjs";

const aqui = dirname(fileURLToPath(import.meta.url));
const BASE = "https://winshotx.com/";
const t = (es, en = es) => ({ es, en });
const herramienta = (id) => HERRAMIENTAS.find((h) => h.id === id);
const fila = (id) => FILAS.find((f) => f.id === id);
const alternativa = (id) => ALTERNATIVAS.find((a) => a.id === id);
const mayuscula = (s) => s.charAt(0).toUpperCase() + s.slice(1);
// «la Herramienta de Recortes», «the Snipping Tool», «ShareX».
const conArticulo = (h, L) => {
  const art = L === "es" ? h.art : h.artEn;
  return `${art ? `${art} ` : ""}${h[L]}`;
};

/* --------------------------------------------------------------------- las piezas */
const icono = (id) => `<svg aria-hidden="true"><use href="#${id}"/></svg>`;
const descarga = () =>
  `<p class="botones"><a class="btn uno" href="${DESCARGA}">${icono("i-download")}${el("span", "", t("Descargar para Windows", "Download for Windows"))}</a></p>`;

function foto({ img, ancho, alto, alt }) {
  return `<figure><img src="../img/${img}-es.webp" data-src-en="../img/${img}-en.webp" width="${ancho}" height="${alto}" loading="lazy" alt="${escapar(alt.es)}" data-alt-en="${escapar(alt.en)}"></figure>`;
}

// Una tabla sale de las filas de comparativa.mjs: aquí no se escribe ningún dato.
function tabla({ filas, rivales }) {
  const columnas = [HERRAMIENTAS[0], ...rivales.map(herramienta)];
  const cabeza = columnas.map((h) => el("th", 'scope="col"', dos((L) => nombre(h, L)))).join("");
  const cuerpo = filas.map((id) => {
    const f = fila(id);
    const celdas = columnas.map((h) => el("td", "", dos((L) => txt(f.v[h.id], L)))).join("");
    return `<tr>${el("th", 'scope="row"', dos((L) => f[L]))}${celdas}</tr>`;
  });
  return `<div class="tabla comparar"><table>
<thead><tr>${el("th", 'scope="col"', t("Qué", "What"))}${cabeza}</tr></thead>
<tbody>
${cuerpo.join("\n")}
</tbody>
</table></div>`;
}

// «¿Usas otro programa? Mira winshotx frente a OBS Studio, la Xbox Game Bar o ShareX.»
function otras(ids) {
  const enlaces = (L) => ids.map((id) => `<a href="../${alternativa(id).es}/">${conArticulo(herramienta(id), L)}</a>`);
  const unidas = (L) => {
    const e = enlaces(L);
    return `${e.slice(0, -1).join(", ")} ${L === "es" ? "o" : "or"} ${e.at(-1)}`;
  };
  return el("p", "", dos((L) => (L === "es" ? `¿Usas otro programa? Mira winshotx frente a ${unidas(L)}.` : `Using something else? See winshotx against ${unidas(L)}.`)));
}

function bloque(b) {
  if (b.p) return el("p", "", b.p);
  if (b.lista) return `<ul class="lista">\n${b.lista.map((x) => el("li", "", x)).join("\n")}\n</ul>`;
  if (b.pasos) return `<ol class="lista pasos">\n${b.pasos.map((x) => el("li", "", x)).join("\n")}\n</ol>`;
  if (b.aparte) return `<p class="aparte">${icono("i-info")}${el("span", "", b.aparte)}</p>`;
  if (b.tabla) return tabla(b.tabla);
  if (b.otras) return otras(b.otras);
  throw new Error(`bloque desconocido: ${Object.keys(b).join(", ")}`);
}

function seccion({ id, h2, foto: laFoto, html }) {
  const dentro = `${el("h2", "", h2)}\n${html}`;
  return laFoto
    ? `<section id="${id}" class="con-foto">\n<div>\n${dentro}\n</div>\n${foto(laFoto)}\n</section>`
    : `<section id="${id}">\n${dentro}\n</section>`;
}

const TITULO_PREGUNTAS = t("Preguntas", "Questions");
const preguntas = (lista_) =>
  seccion({
    id: "preguntas",
    h2: TITULO_PREGUNTAS,
    html: lista_.map(({ q, a }) => `${el("h3", "", q)}\n${el("p", "", a)}`).join("\n"),
  });

/* ------------------------------------------------- lo que dice cada «alternativa a» */
const INSTALADOR = fila("instalador");
const pesaWx = (L) => txt(INSTALADOR.v.wx, L);
const pesaOtra = (h, L) => txt(INSTALADOR.v[h.id], L);
const traeWindows = (h) => !/MB/.test(INSTALADOR.v[h.id][1]);

function entradillaAlternativa(h, L) {
  const { gana, pierde } = reparto(h);
  const x = conArticulo(h, L);
  const suyas = lista(gana.slice(0, 3).map((f) => minuscula(f.vende[L])), L);
  const deLaOtra = lista(pierde.map((f) => minuscula(f.vende[L])), L);
  if (L === "es") {
    const peso = traeWindows(h) ? `Su instalador pesa ${pesaWx(L)}, y ${x} viene con Windows.` : `Su instalador pesa ${pesaWx(L)}; el de ${h.es}, ${pesaOtra(h, L)}.`;
    return `winshotx es una alternativa a ${x} para Windows 10 y 11 y para Linux, gratis y sin cuenta. ${peso} A favor de winshotx: ${suyas}. A favor de ${x}: ${deLaOtra}.`;
  }
  const un = /^[AEIOUX]/.test(h.en) || h.en.startsWith("OBS") ? "an" : "a";
  const peso = traeWindows(h) ? `Its installer weighs ${pesaWx(L)}, and ${x} comes with Windows.` : `Its installer weighs ${pesaWx(L)}; ${h.en}'s, ${pesaOtra(h, L)}.`;
  return `winshotx is ${un} ${h.en} alternative for Windows 10 and 11 and for Linux, free and with no account. ${peso} Where winshotx wins: ${suyas}. Where ${x} wins: ${deLaOtra}.`;
}

// Cada respuesta se monta con los valores de comparativa.mjs, igual que las listas de la portada.
function preguntasAlternativa(h) {
  const { gana, pierde } = reparto(h);
  const precio = fila("precio").v[h.id];
  const idiomasWx = (L) => txt(fila("idiomas").v.wx, L);
  const mejor = (f, L) => {
    const valor = txt(f.v[h.id], L);
    if (f.id === "idiomas") return L === "es" ? `${f.vende.es}: ${minuscula(valor)}, frente a los ${idiomasWx(L)} de winshotx.` : `${f.vende.en}: ${minuscula(valor)}, against ${idiomasWx(L)} in winshotx.`;
    return /^(Sí|Yes)$/.test(valor) ? `${f.vende[L]}.` : `${f.vende[L]}: ${minuscula(valor)}.`;
  };
  return [
    {
      q: t("¿Se puede usar gratis para trabajar?", "Can it be used for free at work?"),
      a: dos((L) => (L === "es" ? "winshotx, sí: es gratis, también para trabajar." : "winshotx, yes: it is free, for work too.") + ` ${h[L]}: ${minuscula(txt(precio, L))}.`),
    },
    // Las que vienen con Windows no tienen instalador que comparar: ya lo dice la entradilla.
    ...(traeWindows(h) ? [] : [{
      q: t("¿Cuánto pesa cada una?", "How much does each one weigh?"),
      a: dos((L) => (L === "es" ? `El instalador de winshotx pesa ${pesaWx(L)}.` : `The winshotx installer weighs ${pesaWx(L)}.`) + ` ${h[L]}: ${minuscula(pesaOtra(h, L))}.`),
    }]),
    {
      q: dos((L) => (L === "es" ? `¿Qué hace mejor ${conArticulo(h, L)}?` : `What does ${conArticulo(h, L)} do better?`)),
      a: dos((L) => pierde.map((f) => mejor(f, L)).join(" ")),
    },
    {
      q: dos((L) => (L === "es" ? `¿En qué gana winshotx a ${conArticulo(h, L)}?` : `Where does winshotx beat ${conArticulo(h, L)}?`)),
      a: dos((L) => gana.slice(0, 3).map((f) => `${f.vende[L]} (${h[L]}: ${minuscula(txt(f.v[h.id], L))}).`).join(" ")),
    },
  ];
}

function pasosAlternativa(h) {
  const k = (...teclas) => teclas.map((x) => `<kbd>${x}</kbd>`).join("+");
  const ultimo = h.id === "recortes"
    ? t(
        `<b>Si quieres, dale la tecla ${k("Impr Pant")}.</b> En <b>Ajustes → Teclas de Windows</b> hay un interruptor que se la quita a la Herramienta de Recortes y se la da a winshotx.`,
        `<b>If you like, give it the ${k("Print Screen")} key.</b> In <b>Settings → Windows keys</b> a switch takes that key from the Snipping Tool and gives it to winshotx.`,
      )
    : t(
        "<b>Cambia los atajos si quieres.</b> En <b>Ajustes</b>, pulsa el campo y teclea la combinación que quieras.",
        "<b>Change the shortcuts if you like.</b> In <b>Settings</b>, click the field and press the combination you want.",
      );
  return [
    dos((L) => (L === "es"
      ? `<b>Instala winshotx.</b> Pesa ${pesaWx(L)}, se instala solo para tu usuario y nunca sale la ventana de permisos de administrador.`
      : `<b>Install winshotx.</b> It weighs ${pesaWx(L)}, installs for your user only and never shows the administrator prompt.`)),
    t(`<b>Captura con ${k("Ctrl", "Shift", "2")}.</b> Arrastra sobre lo que quieras.`, `<b>Capture with ${k("Ctrl", "Shift", "2")}.</b> Drag over what you want.`),
    t(`<b>Graba con ${k("Ctrl", "Shift", "5")}.</b> Arrastra sobre la zona, y pulsa el mismo atajo otra vez para parar.`, `<b>Record with ${k("Ctrl", "Shift", "5")}.</b> Drag over the area, and press the same shortcut again to stop.`),
    ultimo,
  ];
}

const pasoAPaso = () =>
  el("p", "", dos((L) => {
    const enlaces = PROBLEMAS.map((p) => `<a href="../${p.es}/">${minuscula(p.pie[L])}</a>`);
    return `${L === "es" ? "Paso a paso" : "Step by step"}: ${enlaces.slice(0, -1).join(", ")} ${L === "es" ? "y" : "and"} ${enlaces.at(-1)}.`;
  }));

/* ------------------------------------------------------------------- cada página */
function contenidoProblema(p) {
  const secciones = p.secciones.map((s) => seccion({ id: s.id, h2: s.h2, foto: s.foto, html: s.cuerpo.map(bloque).join("\n") }));
  return {
    ...p,
    miga: p.pie,
    indice: [...p.secciones.map((s) => ({ id: s.id, nombre: s.indice })), { id: "preguntas", nombre: TITULO_PREGUNTAS }],
    html: [...secciones, preguntas(p.preguntas)].join("\n\n"),
  };
}

function contenidoAlternativa(a) {
  const h = a.herramienta;
  const titulo = dos((L) => (L === "es" ? `Alternativa a ${conArticulo(h, L)}: winshotx, frente a frente` : `${h.en} alternative: winshotx, head to head`));
  const miga = dos((L) => (L === "es" ? `Alternativa a ${conArticulo(h, L)}` : `${h.en} alternative`));
  const descripcion = dos((L) => (L === "es"
    ? `winshotx frente a ${conArticulo(h, L)}: lo que gana cada una y lo que pesa cada instalador, con las fuentes oficiales comprobadas el ${COMPROBADO.es}.`
    : `winshotx against ${conArticulo(h, L)}: where each one wins and what each installer weighs, from official sources checked on ${COMPROBADO.en}.`));
  const secciones = [
    seccion({ id: "frente", h2: TEXTOS.titulo, html: `${h.id === "recortes" ? `${medidas(false)}\n` : ""}${frenteDos(h)}` }),
    seccion({
      id: "empezar",
      h2: t("Cómo se empieza", "How to get started"),
      foto: {
        img: "editor",
        ancho: 1280,
        alto: 800,
        alt: t("El editor de winshotx con un fondo del Estudio, esquinas redondeadas y sombra", "The winshotx editor with a Studio background, rounded corners and a shadow"),
      },
      html: `${bloque({ pasos: pasosAlternativa(h) })}\n${pasoAPaso()}`,
    }),
    preguntas(preguntasAlternativa(h)),
    seccion({ id: "fuentes", h2: t("De dónde salen los datos", "Where the data comes from"), html: el("p", 'class="fuentes"', TEXTOS.fuentes) }),
  ];
  return {
    ...a,
    titulo,
    miga,
    descripcion,
    h1: dos((L) => (L === "es" ? `Alternativa a ${conArticulo(h, L)} en Windows` : `${h.en} alternative for Windows`)),
    entradilla: dos((L) => entradillaAlternativa(h, L)),
    indice: [
      { id: "frente", nombre: TEXTOS.titulo },
      { id: "empezar", nombre: t("Empezar", "Get started") },
      { id: "preguntas", nombre: TITULO_PREGUNTAS },
      { id: "fuentes", nombre: t("Fuentes", "Sources") },
    ],
    html: secciones.join("\n\n"),
  };
}

export const NUEVAS = [...PROBLEMAS.map(contenidoProblema), ...ALTERNATIVAS.map(contenidoAlternativa)];

/* ------------------------------------------------------- el pie de la portada */
const rutaPortada = join(aqui, "index.html");
const enlacePie = (p) => `          <li>${el("a", `href="${p.es}/"`, p.pie)}</li>`;
const bloquePie = `<!-- paginas:desde (lo escribe generar-paginas.mjs a partir de paginas.mjs) -->
    <div class="pie-rejilla pie-paginas">
      <div>
        ${el("h2", 'class="pie-titulo"', t("Cómo se hace", "How to"))}
        <ul>
${PROBLEMAS.map(enlacePie).join("\n")}
        </ul>
      </div>
      <div class="pie-alternativas">
        ${el("h2", 'class="pie-titulo"', t("Alternativa a", "Alternative to"))}
        <ul>
${ALTERNATIVAS.map(enlacePie).join("\n")}
        </ul>
      </div>
    </div>
    <!-- paginas:hasta -->`;
const portadaHtml = await readFile(rutaPortada, "utf8");
const marcas = /<!-- paginas:desde[\s\S]*?<!-- paginas:hasta -->/;
if (!marcas.test(portadaHtml)) throw new Error("el pie de la portada no tiene las marcas paginas:desde y paginas:hasta");
const nuevaPortada = portadaHtml.replace(marcas, () => bloquePie);
if (nuevaPortada !== portadaHtml) await writeFile(rutaPortada, nuevaPortada, "utf8");

/* --------------------------------------------------------------- escribirlas */
const portada = await leerPortada(aqui);

function documento(p) {
  const { barra, pie } = cromoPara(portada, p.es);
  const url = `${BASE}${p.es}/`;
  const urlEn = `${BASE}en/${p.en}/`;
  const titulo = escapar(p.titulo.es);
  const descripcion = escapar(p.descripcion.es);
  const html = `<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
${el("title", "", p.titulo)}
<meta name="description" content="${descripcion}">
<link rel="canonical" href="${url}">
<link rel="alternate" hreflang="es" href="${url}">
<link rel="alternate" hreflang="en" href="${urlEn}">
<link rel="alternate" hreflang="x-default" href="${url}">
<meta name="color-scheme" content="light dark">
<meta name="theme-color" content="#0b0b0d" media="(prefers-color-scheme: dark)">
<meta name="theme-color" content="#f7f7f9" media="(prefers-color-scheme: light)">
<meta property="og:type" content="article">
<meta property="og:site_name" content="winshotx">
<meta property="og:locale" content="es_ES">
<meta property="og:locale:alternate" content="en_US">
<meta property="og:url" content="${url}">
<meta property="og:title" content="${titulo}">
<meta property="og:description" content="${descripcion}">
<meta property="og:image" content="${BASE}social.png?v=4">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="winshotx, «Crop the screen before you blink»: 114 ms, instalador de 3,01 MB, gratis y de código abierto, junto a su ventana de ajustes">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="${titulo}">
<meta name="twitter:description" content="${descripcion}">
<meta name="twitter:image" content="${BASE}social.png?v=4">
<link rel="icon" href="../favicon.ico?v=2" sizes="any">
<link rel="icon" href="../logo.svg?v=2" type="image/svg+xml">
<link rel="apple-touch-icon" href="../apple-touch-icon.png?v=2">
<script>
// El dial de movimiento de la casa, igual que en la portada (ver index.html).
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
<link rel="stylesheet" href="../estilos.css">
<script type="application/ld+json">
{
  "@context": "https://schema.org",
  "@type": "BreadcrumbList",
  "itemListElement": [
    { "@type": "ListItem", "position": 1, "name": "winshotx", "item": "${BASE}" },
    { "@type": "ListItem", "position": 2, "name": ${JSON.stringify(p.miga.es)}, "item": "${url}" }
  ]
}
</script>
<script type="application/ld+json" data-faq>
{}
</script>
</head>
<body>

<!-- Esta página la escribe generar-paginas.mjs: no se edita a mano, que se pisa la próxima vez. -->
<svg style="display:none" xmlns="http://www.w3.org/2000/svg"></svg>

${barra}

<main class="leer">
  <div class="env">
    <header class="leer-cabeza">
      ${el("h1", "", p.h1)}
      ${el("p", 'class="entradilla"', p.entradilla)}
      ${descarga()}
    </header>

    <div class="leer-rejilla">
      <nav class="leer-indice" aria-label="En esta página" data-aria-en="On this page">
${p.indice.map((i) => `        ${el("a", `href="#${i.id}"`, i.nombre)}`).join("\n")}
      </nav>

      <div class="texto">
${p.html}
      </div>
    </div>
  </div>
</main>

${pie}

<script src="../leer.js"></script>
</body>
</html>
`;
  return html.replace(/<svg style="display:none"[^>]*><\/svg>/, () => spritePara(html, portada.simbolos, p.es));
}

for (const p of NUEVAS) {
  await mkdir(join(aqui, p.es), { recursive: true });
  await writeFile(join(aqui, p.es, "index.html"), documento(p), "utf8");
}

/* -------------------------------------------------- el sitemap, llms.txt y la 404 */
const entradaSitemap = (loc, p) => `  <url>
    <loc>${BASE}${loc}/</loc>
    <lastmod>${p.actualizado ?? ACTUALIZADO}</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.7</priority>
    <xhtml:link rel="alternate" hreflang="es" href="${BASE}${p.es}/"/>
    <xhtml:link rel="alternate" hreflang="en" href="${BASE}en/${p.en}/"/>
    <xhtml:link rel="alternate" hreflang="x-default" href="${BASE}${p.es}/"/>
  </url>`;
const rutaSitemap = join(aqui, "sitemap.xml");
const sitemap = (await readFile(rutaSitemap, "utf8")).replace(/\s*<!-- paginas:desde[\s\S]*?<!-- paginas:hasta -->/, "");
const bloqueSitemap = TODAS.flatMap((p) => [entradaSitemap(p.es, p), entradaSitemap(`en/${p.en}`, p)]).join("\n");
await writeFile(rutaSitemap, sitemap.replace("</urlset>", () => `  <!-- paginas:desde (lo escribe generar-paginas.mjs) -->\n${bloqueSitemap}\n  <!-- paginas:hasta -->\n</urlset>`), "utf8");

const rutaLlms = join(aqui, "llms.txt");
const llms = (await readFile(rutaLlms, "utf8")).replace(/## How-to guides\n[\s\S]*?(?=## Documentation\n)/, "");
const lineaLlms = (p) => `- [${p.titulo.en}](${BASE}en/${p.en}/) ([Spanish](${BASE}${p.es}/))`;
const bloqueLlms = `## How-to guides\n\n${NUEVAS.filter((p) => !p.herramienta).map(lineaLlms).join("\n")}\n\n## Comparisons\n\nEach page says where winshotx wins and where the other tool wins, with sources.\n\n${NUEVAS.filter((p) => p.herramienta).map(lineaLlms).join("\n")}\n\n`;
if (!llms.includes("## Documentation\n")) throw new Error("llms.txt no tiene la sección Documentation");
await writeFile(rutaLlms, llms.replace("## Documentation\n", () => `${bloqueLlms}## Documentation\n`), "utf8");

// Quien se pierde dentro de /en/ lee la 404 en inglés, y sus enlaces van a la página inglesa.
const ruta404 = join(aqui, "404.html");
const pagina404 = await readFile(ruta404, "utf8");
const ingles = { "/novedades/": "/en/changelog/", "/privacidad/": "/en/privacy/", "/soporte/": "/en/support/", ...Object.fromEntries(TODAS.map((p) => [`/${p.es}/`, `/en/${p.en}/`])) };
const nueva404 = pagina404.replace(/var ingles = \{[^}]*\};/, () => `var ingles = ${JSON.stringify(ingles)};`);
if (nueva404 === pagina404 && !pagina404.includes(JSON.stringify(ingles))) throw new Error("la 404 no tiene su tabla de enlaces ingleses");
if (nueva404 !== pagina404) await writeFile(ruta404, nueva404, "utf8");

console.log(`paginas: ${PROBLEMAS.length} por problema y ${ALTERNATIVAS.length} alternativas`);
