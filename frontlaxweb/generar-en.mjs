/**
 * Genera la version inglesa del sitio a partir de la espanola.
 *
 *   node frontlaxweb/generar-en.mjs
 *
 * Cada idioma necesita su propia URL: un boton que cambia los textos con JavaScript deja a
 * Google y a los rastreadores de las IA viendo solo la version espanola. Los textos ingleses
 * ya viven en los atributos data-en de cada pagina, asi que aqui solo hay que aplicarlos.
 *
 * Paginas: index.html -> en/index.html, y docs/index.html -> en/docs/index.html.
 */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const BASE = "https://winshotx.com/";

/** Los data-en llevan las marcas escapadas: aqui se devuelven a su forma. */
const desescapar = (t) =>
  t
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&quot;", '"')
    .replaceAll("&#39;", "'")
    .replaceAll("&amp;", "&");

/** Lo que es igual en todas las paginas: aplicar los data-en y los tooltips. */
function traducir(html) {
  const traducible = /<([a-z0-9]+)((?:[^>"]|"[^"]*")*?\sdata-en="([^"]*)"(?:[^>"]|"[^"]*")*)>([\s\S]*?)<\/\1>/g;
  html = html.replace(traducible, (_todo, tag, atributos, ingles) => {
    const limpios = atributos
      .replace(/\sdata-en="(?:[^"]*)"/, "")
      .replace(/\sdata-html(?==|\s|$)/, "");
    return `<${tag}${limpios}>${desescapar(ingles)}</${tag}>`;
  });

  html = html.replace(/\stitle="[^"]*"((?:[^>"]|"[^"]*")*?)\sdata-titulo-en="([^"]*)"/g,
    (_t, medio, ingles) => ` title="${ingles}"${medio}`);
  html = html.replace(/\sdata-titulo-en="[^"]*"/g, "");

  // Los aria-label son invisibles para quien revisa la pagina, asi que se quedaban en
  // espanol sin que nadie lo notara. Mismo apano que los tooltips.
  html = html.replace(/\saria-label="[^"]*"((?:[^>"]|"[^"]*")*?)\sdata-aria-en="([^"]*)"/g,
    (_t, medio, ingles) => ` aria-label="${ingles}"${medio}`);
  html = html.replace(/\sdata-aria-en="([^"]*)"((?:[^>"]|"[^"]*")*?)\saria-label="[^"]*"/g,
    (_t, ingles, medio) => `${medio} aria-label="${ingles}"`);
  html = html.replace(/\sdata-aria-en="[^"]*"/g, "");

  html = html.replace('<html lang="es">', '<html lang="en">');
  html = html.replace('<meta property="og:locale" content="es_ES">', '<meta property="og:locale" content="en_US">');
  html = html.replace(
    '<meta property="og:locale:alternate" content="en_US">',
    '<meta property="og:locale:alternate" content="es_ES">',
  );
  html = html.replaceAll(`${BASE}social.png`, `${BASE}social-en.png`);
  return html;
}

/**
 * Pone la version en la URL del CSS y del JS.
 *
 * GitHub Pages sirve el HTML con 10 minutos de cache y los recursos con 4 horas, asi que
 * al publicar un cambio de estilos hay una ventana larga en la que el visitante que ya
 * habia entrado se lleva la pagina nueva con el CSS viejo: los iconos salen a tamano
 * natural y la barra se descoloca. Con la huella del archivo en la URL, un cambio de
 * contenido es una URL distinta y el navegador no puede servir la de antes.
 */
function versionar(html, huellas) {
  return html.replace(
    /((?:\.\.\/)*)(estilos\.css|demo\.js)(\?v=[a-f0-9]+)?/g,
    (_todo, subir, archivo) => `${subir}${archivo}?v=${huellas[archivo]}`,
  );
}

const huellas = Object.fromEntries(
  await Promise.all(
    ["estilos.css", "demo.js"].map(async (archivo) => [
      archivo,
      // El texto se normaliza a saltos de linea de Unix antes de la huella: en Windows el
      // archivo esta en disco con CRLF y en el servidor con LF, y sin esto cada uno
      // calcularia una huella distinta y la comprobacion del despliegue no cuadraria nunca.
      createHash("sha256")
        .update((await readFile(join(aqui, archivo), "utf8")).replaceAll("\r\n", "\n"))
        .digest("hex")
        .slice(0, 8),
    ]),
  ),
);

/**
 * Construye el bloque FAQPage a partir de las preguntas que la pagina ENSENA.
 *
 * Escribirlo a mano se desincroniza en cuanto alguien anade o quita una pregunta, y la
 * politica de Google descalifica el bloque entero si marca algo que no esta visible. Al
 * sacarlo del propio HTML eso no puede pasar, y cada idioma se queda con el suyo.
 */
function construirFaq(html, idioma) {
  if (!html.includes("data-faq")) return html;

  const seccion = html.match(/<section id="preguntas">([\s\S]*?)<\/section>/);
  if (!seccion) throw new Error("no aparece la seccion de preguntas");

  const soloTexto = (s) =>
    desescapar(s.replace(/<[^>]+>/g, " ")).replace(/\s+/g, " ").trim();

  // docs/ enseña el FAQ como h3+p sueltos; la portada lo mete en un acordeón animado
  // con el chevron y el <div class="faq-cajon"> de por medio. Cualquier cosa entre el
  // </h3> y el <p> de la respuesta vale, con tal de que sea el <p> que le corresponde.
  const pares = [
    ...seccion[1].matchAll(/<h3[^>]*>([\s\S]*?)<\/h3>[\s\S]*?<p[^>]*>([\s\S]*?)<\/p>/g),
  ];
  if (!pares.length) throw new Error("la seccion de preguntas no tiene ningun par h3 + p");

  const bloque = {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    inLanguage: idioma,
    mainEntity: pares.map(([, pregunta, respuesta]) => ({
      "@type": "Question",
      name: soloTexto(pregunta),
      acceptedAnswer: { "@type": "Answer", text: soloTexto(respuesta) },
    })),
  };

  return html.replace(
    /<script type="application\/ld\+json" data-faq>[\s\S]*?<\/script>/,
    `<script type="application/ld+json" data-faq>
${JSON.stringify(bloque, null, 2)}
</script>`,
  );
}

/**
 * Las descripciones viven en atributos y no pueden llevar data-en, asi que aqui va la
 * frase espannola ENTERA y su traduccion.
 *
 * Eso las hace fragiles: cambiar una coma en el HTML deja la clave sin casar y la pagina
 * inglesa se queda con la frase en castellano, sin que nada falle. Paso de verdad, con el
 * tamanno del instalador: la meta description inglesa estuvo en espannol desde que el
 * numero cambio de 2,2 a 2,49 MB. Por eso, al final del archivo, se comprueba que TODAS
 * las claves se hayan usado en alguna pagina.
 */
const ATRIBUTOS = {
  'content="Alternativa libre a la Herramienta de Recortes de Windows: captura de región, grabación en GIF y MP4 y editor fotograma a fotograma. Enseña la selección ya pintada 114 ms después del atajo, con tres pantallas, y gasta 33 MB. Instalador de 2,52 MB, sin cuenta y sin nube."':
    'content="Free and open source alternative to the Windows Snipping Tool: region capture, GIF and MP4 recording and a frame by frame editor. Shows the selection already painted 114 ms after the shortcut, with three screens, and uses 33 MB. A 2.52 MB installer, no account and no cloud."',
  'content="winshotx · captura y grabación de pantalla para Windows"':
    'content="winshotx · screenshots and screen recording for Windows"',
  'content="La Herramienta de Recortes tarda 920 ms y gasta 253 MB. Esta la enseña ya pintada en 114 ms con tres pantallas y gasta 33 MB, graba GIF y trae editor. 2,52 MB, código abierto."':
    'content="The Snipping Tool takes 920 ms and 253 MB. This one shows it already painted in 114 ms with three screens and uses 33 MB, records GIF and ships an editor. 2.52 MB, open source."',
  'content="La Herramienta de Recortes tarda 920 ms y gasta 253 MB. Esta la enseña ya pintada en 114 ms con tres pantallas y gasta 33 MB. 2,52 MB, código abierto."':
    'content="The Snipping Tool takes 920 ms and 253 MB. This one shows it already painted in 114 ms with three screens and uses 33 MB. 2.52 MB, open source."',
  'content="winshotx: 114 ms contra 920 ms de la Herramienta de Recortes"':
    'content="winshotx: 114 ms against the Snipping Tool\'s 920 ms"',
  'content="winshotx no recoge nada: ni cuentas, ni telemetría, ni analítica. Aquí están, una por una, las únicas conexiones que hace la aplicación y para qué usa cada permiso de Windows."':
    'content="winshotx collects nothing: no accounts, no telemetry, no analytics. Here, one by one, are the only connections the app makes and what it uses each Windows permission for."',
  'content="No recoge nada. Las únicas conexiones que hace la aplicación, una por una."':
    'content="It collects nothing. The only connections the app makes, one by one."',
  'content="Privacidad de winshotx"': 'content="winshotx privacy"',
  'content="Cómo se usa winshotx: instalación, atajos de teclado, captura de región con lupa, grabación en GIF y MP4, editor fotograma a fotograma, exportación y ajustes."':
    'content="How winshotx works: install, keyboard shortcuts, region capture with a magnifier, GIF and MP4 recording, the frame by frame editor, export and settings."',
  'content="Guía de winshotx"': 'content="winshotx documentation"',
  'content="Instalación, atajos, captura de región, grabación en GIF y MP4, editor fotograma a fotograma y exportación."':
    'content="Install, shortcuts, region capture, GIF and MP4 recording, the frame by frame editor and export."',
  'content="Instalación, atajos, captura de región, grabación en GIF y MP4, editor y exportación."':
    'content="Install, shortcuts, region capture, GIF and MP4 recording, the editor and export."',
};

/**
 * Cada pagina tiene sus propios enlaces que apuntan a si misma o al otro idioma, y estan
 * escritos a mano porque son cuatro y equivocarse en uno rompe el hreflang entero.
 */
const PAGINAS = [
  {
    origen: "index.html",
    destino: ["en", "index.html"],
    propios: {
      [`<link rel="canonical" href="${BASE}">`]: `<link rel="canonical" href="${BASE}en/">`,
      [`<meta property="og:url" content="${BASE}">`]: `<meta property="og:url" content="${BASE}en/">`,
      [`"url": "${BASE}"`]: `"url": "${BASE}en/"`,
      'href="logo.svg': 'href="../logo.svg',
      'href="favicon.ico': 'href="../favicon.ico',
      'href="apple-touch-icon.png': 'href="../apple-touch-icon.png',
      'href="estilos.css?v=': 'href="../estilos.css?v=',
      'src="demo.js?v=': 'src="../demo.js?v=',
      '<a href="privacidad/">Privacy</a>': '<a href="privacy/">Privacy</a>',
      '<a class="idioma" id="idioma" href="en/" hreflang="en" lang="en">English</a>':
        '<a class="idioma" id="idioma" href="../" hreflang="es" lang="es">Español</a>',
    },
  },
  {
    origen: join("docs", "index.html"),
    destino: ["en", "docs", "index.html"],
    propios: {
      [`<link rel="canonical" href="${BASE}docs/">`]: `<link rel="canonical" href="${BASE}en/docs/">`,
      [`<meta property="og:url" content="${BASE}docs/">`]: `<meta property="og:url" content="${BASE}en/docs/">`,
      [`"item": "${BASE}" }`]: `"item": "${BASE}en/" }`,
      [`"item": "${BASE}docs/" }`]: `"item": "${BASE}en/docs/" }`,
      'href="../logo.svg': 'href="../../logo.svg',
      'href="../favicon.ico': 'href="../../favicon.ico',
      'href="../apple-touch-icon.png': 'href="../../apple-touch-icon.png',
      'href="../estilos.css?v=': 'href="../../estilos.css?v=',
      '<a href="../privacidad/">Privacy</a>': '<a href="../privacy/">Privacy</a>',
      '<a class="idioma" id="idioma" href="../en/docs/" hreflang="en" lang="en">English</a>':
        '<a class="idioma" id="idioma" href="../../docs/" hreflang="es" lang="es">Español</a>',
    },
  },
  {
    origen: join("privacidad", "index.html"),
    destino: ["en", "privacy", "index.html"],
    propios: {
      [`<link rel="canonical" href="${BASE}privacidad/">`]: `<link rel="canonical" href="${BASE}en/privacy/">`,
      [`<meta property="og:url" content="${BASE}privacidad/">`]: `<meta property="og:url" content="${BASE}en/privacy/">`,
      [`"item": "${BASE}" }`]: `"item": "${BASE}en/" }`,
      [`"name": "Privacidad", "item": "${BASE}privacidad/" }`]: `"name": "Privacy", "item": "${BASE}en/privacy/" }`,
      'href="../logo.svg': 'href="../../logo.svg',
      'href="../favicon.ico': 'href="../../favicon.ico',
      'href="../apple-touch-icon.png': 'href="../../apple-touch-icon.png',
      'href="../estilos.css?v=': 'href="../../estilos.css?v=',
      '<a class="idioma" id="idioma" href="../en/privacy/" hreflang="en" lang="en">English</a>':
        '<a class="idioma" id="idioma" href="../../privacidad/" hreflang="es" lang="es">Español</a>',
    },
  },
];

/** Que claves de ATRIBUTOS ha encontrado de verdad alguna pagina. */
const usadas = new Set();

for (const pagina of PAGINAS) {
  const rutaOrigen = join(aqui, pagina.origen);
  let original = await readFile(rutaOrigen, "utf8");

  // La pagina espanola es el origen, asi que su bloque de preguntas se reescribe aqui
  // mismo: es la unica forma de que el JSON-LD y lo que se ve no se separen nunca.
  const puesto = versionar(construirFaq(original, "es"), huellas);
  if (puesto !== original) {
    await writeFile(rutaOrigen, puesto, "utf8");
    original = puesto;
    console.log(`${pagina.origen}: actualizado`);
  }

  let html = construirFaq(traducir(original), "en");
  for (const [es, en] of Object.entries(ATRIBUTOS)) {
    if (html.includes(es)) usadas.add(es);
    html = html.replaceAll(es, en);
  }
  for (const [de, a] of Object.entries(pagina.propios)) {
    if (!html.includes(de)) throw new Error(`${pagina.origen}: no aparece ${de}`);
    html = html.replaceAll(de, a);
  }

  const salida = join(aqui, ...pagina.destino);
  await mkdir(dirname(salida), { recursive: true });
  await writeFile(salida, html, "utf8");

  const pendientes = (html.match(/data-en=/g) ?? []).length;
  console.log(
    `${pagina.destino.join("/")} generado${pendientes ? ` (quedan ${pendientes} sin traducir)` : ""}`,
  );
}

// Una clave que no aparece en ninguna pagina es una frase que se ha quedado en espannol al
// otro lado, y sin esto no se entera nadie: la pagina se genera igual y parece correcta.
const huerfanas = Object.keys(ATRIBUTOS).filter((es) => !usadas.has(es));
if (huerfanas.length) {
  console.error("Estas frases ya no estan en el HTML espannol, asi que su traduccion no se aplica:");
  for (const es of huerfanas) console.error(`  ${es}`);
  console.error("Cambia la clave en ATRIBUTOS por el texto que hay ahora, o quitala.");
  process.exit(1);
}
