/**
 * Escribe la comparativa de la portada («Frente a frente») a partir de comparativa.mjs.
 *
 *   node frontlaxweb/generar-comparativa.mjs     reescribe <section id="comparativa"> de index.html
 *
 * La corre generar-en.mjs antes de sacar la página inglesa, así que basta con correr esa.
 *
 * Una pestaña por herramienta («¿Cuál usas ahora?»), y en cada una: lo que pesa cada
 * instalador, lo que gana winshotx frente a esa, lo que gana esa, y lo que hacen las dos. La
 * de la Herramienta de Recortes lleva además las tres cifras medidas, que se midieron contra
 * ella. Las pestañas son botones de radio: cambian de panel sin JavaScript (con :has() en
 * el CSS), y con las flechas del teclado. Todo el texto de las diez está en el HTML, también
 * el de las pestañas que no se ven, para quien lee la página sin ejecutar nada.
 */
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { HERRAMIENTAS, FILAS, TEXTOS } from "./comparativa.mjs";

const aqui = dirname(fileURLToPath(import.meta.url));
const OTRAS = HERRAMIENTAS.filter((h) => h.id !== "wx");
const IDIOMAS = ["es", "en"];

/* ----------------------------------------------------------------- en los dos idiomas */
// Cada texto se escribe en español y, si cambia, con su inglés en data-en, que es lo que lee
// generar-en.mjs. El elemento que lleva data-en no puede llevar dentro otro de su misma
// etiqueta: generar-en.mjs lo cierra en el primer cierre que encuentra.
const escapar = (t) => t.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
function el(etiqueta, atributos, texto) {
  const { es, en } = texto;
  const marcas = /<[a-z]/i.test(es) || /<[a-z]/i.test(en);
  const ingles = es === en ? "" : `${marcas ? " data-html" : ""} data-en="${escapar(en)}"`;
  return `<${etiqueta}${atributos ? ` ${atributos}` : ""}${ingles}>${es}</${etiqueta}>`;
}
// Un texto hecho con una función de cada idioma: dos(L => ...) da { es, en }.
const dos = (f) => Object.fromEntries(IDIOMAS.map((L) => [L, f(L)]));

const nombre = (h, L, corto = false) => (corto && h.corto ? (L === "en" ? h.cortoEn : h.corto) : h[L]);
const txt = (v, L) => v[L === "en" ? 2 : 1];
// Tras dos puntos va en minúscula, salvo un nombre propio o unas siglas: «Snagit: Mac», «PC».
const PROPIOS = /^(Mac|Linux|Windows|Internet|Insiders|FFmpeg|WebSocket|[A-Z]{2,})\b/;
const minuscula = (s) => (PROPIOS.test(s) ? s : s.charAt(0).toLowerCase() + s.slice(1));
// Las listas de tres o más llevan punto y coma: sus cosas ya llevan comas dentro («una página
// larga, bajando sola»), y con comas no se ve dónde acaba cada una.
function lista(cosas, L) {
  if (cosas.length <= 1) return cosas.join("");
  if (cosas.length === 2) return cosas.join(L === "en" ? " and " : " y ");
  return cosas.slice(0, -1).join("; ") + (L === "en" ? "; and " : "; y ") + cosas.at(-1);
}
const icono = (id) => `<svg aria-hidden="true" class="frente-ic"><use href="#${id}"/></svg>`;

const cuantas = (f) => OTRAS.filter((h) => f.v[h.id]?.[0] === "si").length;

/* --------------------------------------------------------------------- las piezas */
function cabeza() {
  return `    <div class="cabeza-seccion">
      ${el("h2", "", TEXTOS.titulo)}
      ${el("p", 'class="entradilla"', TEXTOS.entradilla)}
      <div class="fan suelta" style="--giro:3deg">${el("p", "", TEXTOS.fan)}</div>
    </div>`;
}

// Las tres cifras medidas contra la Herramienta de Recortes, con sus barras.
function medidas() {
  const recortes = dos((L) => nombre(HERRAMIENTAS[1], L));
  const una = (x) => {
    const [dWx, dOtra] = x.dura ? x.dura.map((d) => `;--dura:${d}`) : ["", ""];
    return `          <div class="medida">
            <div class="medida-num">${x.num}</div>
            ${el("div", 'class="medida-que"', x)}
            <div class="barras">
              <div class="barra-medida nuestra"><span class="nombre">winshotx</span><span class="pista"><span class="llena" style="--largo:${x.largo}%${dWx}"></span></span><span class="cuanto">${x.num}</span></div>
              <div class="barra-medida">${el("span", 'class="nombre"', recortes)}<span class="pista"><span class="llena" style="--largo:100%${dOtra}"></span></span><span class="cuanto">${x.otra}</span></div>
            </div>
          </div>`;
  };
  return `        <div class="medidas" id="medidas">
${TEXTOS.medidas.map(una).join("\n")}
        </div>
        ${el("p", 'class="fuentes"', TEXTOS.medido)}`;
}

function panel(rival) {
  const id = rival.id;
  const gana = [], pierde = [], empatan = [];
  for (const f of FILAS) {
    if (!f.vende) continue;
    const a = f.v.wx?.[0], b = f.v[id]?.[0];
    if (!a || !b || b === "nv") continue; // sin dato oficial: esa fila no se cuenta en esta pestaña
    if (f.pierde) { if (b === "si") pierde.push(f); continue; }
    if (a === "si" && b !== "si") gana.push(f);
    else if (a === "si" && b === "si" && !f.sinEmpate) empatan.push(f);
  }
  // Lo que menos herramientas hacen, arriba; el precio, siempre primero.
  gana.sort((x, y) => (y.primero ? 1 : 0) - (x.primero ? 1 : 0) || cuantas(x) - cuantas(y));
  const TOPE = 6;

  const linea = (f, deLaOtra) => {
    const v = f.v[id];
    const dicho = dos((L) => `${nombre(rival, L)}: ${minuscula(txt(v, L))}${deLaOtra && f.id === "idiomas" ? ` ${TEXTOS.idiomas[L]}` : ""}`);
    return `              <li>${icono(f.icono)}<div>${el("b", "", dos((L) => f.vende[L]))}${el("span", "", dicho)}</div></li>`;
  };
  const tam = FILAS.find((f) => f.id === "instalador");
  const cifra = (h) => {
    const v = tam.v[h.id];
    const palabras = !/MB/.test(v[1]);
    return `            <div class="frente-cifra${h.id === "wx" ? " nuestra" : ""}">${el("div", `class="medida-num${palabras ? " palabras" : ""}"`, dos((L) => txt(v, L)))}${el("div", 'class="medida-que"', dos((L) => nombre(h, L)))}</div>`;
  };
  const resto = gana.slice(TOPE);
  const ademas = resto.length
    ? `\n          ${el("p", 'class="frente-mas"', dos((L) => `<b>${TEXTOS.ademas[L]}</b> ${lista(resto.map((f) => minuscula(f.vende[L])), L)}.`))}`
    : "";
  const tituloOtra = dos((L) => {
    const art = L === "es" ? rival.art : rival.artEn;
    return TEXTOS.ganaOtra[L].replace("{x}", `${art ? `${art} ` : ""}${nombre(rival, L)}`);
  });
  const empate = empatan.length
    ? `\n        ${el("p", 'class="frente-empatan"', dos((L) => `<b>${TEXTOS.lasDos[L]}</b> ${lista(empatan.map((f) => minuscula(f[L])), L)}.`))}`
    : "";

  return `      <div class="frente" id="frente-${id}">
${id === "recortes" ? `${medidas()}\n` : ""}        <div class="frente-dos">
          <div class="frente-cifras">
${cifra(HERRAMIENTAS[0])}
${cifra(rival)}
            ${el("p", 'class="frente-cifras-que"', TEXTOS.instalador)}
          </div>
          <div>
            ${el("h3", 'class="frente-sub"', TEXTOS.gana)}
            <ul class="frente-lista">
${gana.slice(0, TOPE).map((f) => linea(f, false)).join("\n")}
            </ul>${ademas}
          </div>
          <div>
            ${el("h3", 'class="frente-sub"', tituloOtra)}
            <ul class="frente-lista pierde">
${pierde.map((f) => linea(f, true)).join("\n") || `              ${el("li", 'class="frente-nada"', TEXTOS.nada)}`}
            </ul>
          </div>
        </div>${empate}
      </div>`;
}

export function seccion() {
  const pestana = (h, i) =>
    `      <label class="pestana"><input type="radio" name="rival" value="${h.id}"${i === 0 ? " checked" : ""}>${el("span", "", dos((L) => nombre(h, L, true)))}</label>`;
  return `<section id="comparativa" class="seccion comparativa">
  <div class="env">
${cabeza()}

    ${el("p", 'class="rival-pregunta" id="rival-pregunta"', TEXTOS.pregunta)}
    <div class="pestanas rivales" role="radiogroup" aria-labelledby="rival-pregunta">
${OTRAS.map(pestana).join("\n")}
    </div>
    <div class="frentes">
${OTRAS.map(panel).join("\n")}
    </div>
    ${el("p", 'class="fuentes"', TEXTOS.fuentes)}
  </div>
</section>`;
}

/* ------------------------------------------------------------ escribirla en la portada */
const ruta = join(aqui, "index.html");
const portada = await readFile(ruta, "utf8");
const trozo = /<section id="comparativa"[\s\S]*?<\/section>/;
if (!trozo.test(portada)) throw new Error("index.html no tiene <section id=\"comparativa\">");
const nueva = portada.replace(trozo, () => seccion());
// Cada icono que se usa tiene que estar en el sprite de la portada: uno que falta no da error,
// sale un hueco. (El de «No pide cuenta» no está porque hoy esa fila nunca llega a la lista.)
const sinDibujo = [...new Set([...nueva.matchAll(/<use href="#([^"]+)"/g)].map((m) => m[1]))].filter((id) => !nueva.includes(`<symbol id="${id}"`));
if (sinDibujo.length) throw new Error(`a la portada le faltan estos iconos en el sprite: ${sinDibujo.join(", ")}`);
if (nueva !== portada) {
  await writeFile(ruta, nueva);
  console.log("index.html: la comparativa, desde comparativa.mjs");
}
