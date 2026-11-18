/**
 * La barra de arriba, el pie y los dibujos de todas las páginas salen de la portada.
 *
 * Se escriben UNA vez, en index.html, y de ahí se copian a la guía, la privacidad, el
 * soporte, las novedades y la 404 con este módulo. Escritos a mano en cada página se separaban: la guía
 * llevaba un menú con secciones de la portada que ya no existían. Lo usa generar-en.mjs
 * (que lo aplica antes de traducir, y el despliegue para si al correrlo cambia algo) y
 * generar-novedades.mjs (que escribe su página entera).
 *
 * Los dibujos (los <symbol> del sprite) se copian solo los que usa cada página, y tienen
 * que existir en la portada: una página que pida uno que no está es un error, no un hueco.
 */
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { TODAS } from "./paginas.mjs";

/** Cada página, dónde vive y cómo se llega desde ella a la raíz y a su versión inglesa. */
export const PAGINAS_CROMO = {
  docs: { archivo: join("docs", "index.html"), raiz: "../", ingles: "../en/docs/", aqui: "docs/" },
  privacidad: { archivo: join("privacidad", "index.html"), raiz: "../", ingles: "../en/privacy/", aqui: null },
  soporte: { archivo: join("soporte", "index.html"), raiz: "../", ingles: "../en/support/", aqui: "soporte/" },
  novedades: { archivo: join("novedades", "index.html"), raiz: "../", ingles: "../en/changelog/", aqui: "novedades/" },
  // La 404 se sirve en cualquier dirección, así que todo va desde la raíz del dominio.
  "404": { archivo: "404.html", raiz: "/", ingles: "/en/", aqui: null },
  // Las páginas por problema y las «alternativa a» (paginas.mjs): las escribe generar-paginas.mjs.
  ...Object.fromEntries(TODAS.map((p) => [p.es, { archivo: join(p.es, "index.html"), raiz: "../", ingles: `../en/${p.en}/`, aqui: null }])),
};

const SPRITE = /<svg style="display:none"[^>]*>[\s\S]*?<\/svg>/;
const BARRA = /<nav class="barra">[\s\S]*?<\/nav>/;
const PIE = /<footer class="pie">[\s\S]*?<\/footer>/;

/** Lo que hay en la portada, leído una vez. */
export async function leerPortada(aqui) {
  const portada = await readFile(join(aqui, "index.html"), "utf8");
  const sacar = (patron, que) => {
    const hallado = portada.match(patron);
    if (!hallado) throw new Error(`la portada no tiene ${que}`);
    return hallado[0];
  };
  const sprite = sacar(SPRITE, "el sprite de dibujos");
  const simbolos = new Map(
    [...sprite.matchAll(/<symbol id="([^"]+)"[\s\S]*?<\/symbol>/g)].map(([entero, id]) => [id, entero]),
  );
  return { barra: sacar(BARRA, "la barra de arriba"), pie: sacar(PIE, "el pie"), simbolos };
}

/**
 * Los enlaces de la portada, vistos desde otra página: lo relativo pasa por su raíz, el
 * botón de idioma va a su versión inglesa y la página en la que estás lleva su raya.
 */
function recolocar(trozo, pagina, marcarAqui) {
  return trozo.replace(/<a\b([^>]*?)\shref="([^"]*)"([^>]*)>/g, (todo, antes, href, despues) => {
    const atributos = `${antes} ${despues}`;
    // La válvula del movimiento se queda en la propia página: «?motion=off» y «./».
    if (/class="valvula/.test(atributos) || /^(https?:|mailto:|\?)/.test(href)) return todo;
    let nuevo;
    if (/id="idioma"/.test(atributos)) nuevo = pagina.ingles;
    else if (href === "./") nuevo = pagina.raiz;
    else nuevo = pagina.raiz + href;
    let etiqueta = `<a${antes} href="${nuevo}"${despues}>`;
    if (marcarAqui && pagina.aqui && href === pagina.aqui && !/class=/.test(atributos)) {
      etiqueta = etiqueta.replace(/^<a/, '<a class="aqui" aria-current="page"');
    }
    return etiqueta;
  });
}

/** El sprite con solo los dibujos que usa esta página. */
function spritePara(html, simbolos, nombre) {
  const usados = [...new Set([...html.matchAll(/<use href="#([^"]+)"/g)].map(([, id]) => id))];
  const faltan = usados.filter((id) => !simbolos.has(id));
  if (faltan.length) throw new Error(`${nombre}: usa dibujos que la portada no tiene: ${faltan.join(", ")}`);
  return `<svg style="display:none" xmlns="http://www.w3.org/2000/svg">
${usados.map((id) => simbolos.get(id).trim()).join("\n")}
</svg>`;
}

/** La barra y el pie de una página, ya recolocados. */
export function cromoPara(portada, nombre) {
  const pagina = PAGINAS_CROMO[nombre];
  // La raya de «estás aquí» va en la barra; en el pie el enlace se queda normal.
  return { barra: recolocar(portada.barra, pagina, true), pie: recolocar(portada.pie, pagina, false) };
}

/** Pone la barra, el pie y el sprite de la portada en una página que ya los tiene. */
export function aplicarCromo(html, portada, nombre) {
  const { barra, pie } = cromoPara(portada, nombre);
  for (const [patron, que] of [[SPRITE, "sprite"], [BARRA, "barra"], [PIE, "pie"]]) {
    if (!patron.test(html)) throw new Error(`${nombre}: no tiene ${que} donde ponerlo`);
  }
  // El sprite se calcula al final, con la barra y el pie ya dentro: también usan dibujos.
  const conCromo = html.replace(BARRA, () => barra).replace(PIE, () => pie);
  return conCromo.replace(SPRITE, () => spritePara(conCromo.replace(SPRITE, ""), portada.simbolos, nombre));
}

/** Pone el cromo de la portada en la guía, la privacidad, el soporte y la 404. Devuelve las que cambian. */
export async function sincronizarCromo(aqui) {
  const portada = await leerPortada(aqui);
  const cambiadas = [];
  for (const nombre of ["docs", "privacidad", "soporte", "404"]) {
    const ruta = join(aqui, PAGINAS_CROMO[nombre].archivo);
    const antes = await readFile(ruta, "utf8");
    const despues = aplicarCromo(antes, portada, nombre);
    if (despues !== antes) {
      await writeFile(ruta, despues, "utf8");
      cambiadas.push(PAGINAS_CROMO[nombre].archivo);
    }
  }
  return cambiadas;
}

export { spritePara };
