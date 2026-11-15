// Mira la web como la vería alguien, sin abrir nada en el escritorio: Chromium sin ventana.
//
//   node frontlaxweb/probar-web.mjs                      fotos de la portada en claro y oscuro
//   node frontlaxweb/probar-web.mjs novedades/ docs/     otras páginas (la portada siempre va)
//   node frontlaxweb/probar-web.mjs --entera             la página completa, no solo lo de arriba
//   node frontlaxweb/probar-web.mjs --cruceta            además, mide la cruceta de la portada
//
// En cada página, además, se mira que la barra se compacte al bajar sin mover la página, que
// el menú del móvil crezca desde la barra y se recoja con Escape y tocando fuera, y
// cada orden de la terminal se copia con el ratón y con Intro, mirando el portapapeles y que
// salga el aviso de copiado.
// En la portada, las pestañas de la comparativa: una a la vista y la suya, con el ratón, las
// flechas y el dedo (la fila se desliza de lado), también sin JavaScript.
//
// Cada página sale en oscuro y en claro, a 1920 (la pantalla de Munir), a 1440, en un
// iPhone 13 y a 360, y en todas se apuntan los errores de consola y si algo se sale por un
// lado. En los dos móviles, pieza a pieza: nada se sale y lo que se pulsa mide 44 px o más.
// Y la demo en el móvil: se abre a pantalla completa, se recorta con el dedo, y Escape y
// Atrás la cierran sin salir de la página.
// Las fotos van a `.fotos-web\` en la raíz del repo, que git no ve.
//
// La cruceta se mide con la CPU cuatro veces más lenta (lo que pide la casa para dar algo por
// fluido): el ratón de Playwright cruza la portada durante dos segundos y se cuentan los
// fotogramas de la página y en cuántos se movió la cruceta. También comprueba que el cursor del sistema sigue a la vista,
// que los clics pasan a través y que con «?motion=off» va pegada al ratón, sin suavizado.
// Y con `--cruceta` se mide además el scroll de la portada a 1920, con la CPU a 4x, con y sin
// tarjeta gráfica: los fotogramas que llegan a la pantalla mientras baja la rueda, y que el
// modo ligero se ponga solo sin gráfica. `--publicada=<carpeta>` mide al lado la portada
// publicada, para separar lo que es de la web de lo que es el PC cargado.

import { chromium, devices } from "playwright";
import { mkdirSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const entera = args.includes("--entera");
const medirCruceta = args.includes("--cruceta");
const paginas = ["", ...args.filter((a) => !a.startsWith("--"))];

const destino = resolve(aqui, "..", ".fotos-web");
mkdirSync(destino, { recursive: true });

const url = (ruta, extra = "") => pathToFileURL(join(aqui, ruta, "index.html")).href + extra;

const TAMANOS = [
  { nombre: "1920", contexto: { viewport: { width: 1920, height: 1080 } } },
  { nombre: "1440", contexto: { viewport: { width: 1440, height: 900 } } },
  { nombre: "movil", contexto: devices["iPhone 13"] },
  // El móvil más estrecho que se ve a menudo (muchos Android): si algo se sale, es aquí.
  { nombre: "360", contexto: { viewport: { width: 360, height: 780 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true } },
];

/* En el móvil, pieza a pieza (el `overflow-x: hidden` del cuerpo esconde lo que se sale, así que
   el ancho de la página no basta): qué se sale por un lado, y qué se pulsa con menos de 44 px
   de alto. No cuentan lo que va dentro de una frase (un enlace o una orden en mitad del texto),
   lo que está dentro de algo que recorta o se desliza, lo cerrado y la demo encogida, que a ese
   tamaño no se pulsa (se abre a pantalla completa). */
function repasoMovil() {
  const vw = document.documentElement.clientWidth;
  const seVe = (el) => {
    const r = el.getBoundingClientRect();
    const s = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && s.visibility !== "hidden" && Number(s.opacity) > 0 &&
      !el.closest("details:not([open]) > :not(summary), [inert], [hidden]");
  };
  const nombre = (el) => {
    const t = (el.getAttribute("aria-label") || el.textContent || "").trim().replace(/\s+/g, " ").slice(0, 28);
    return `${el.tagName.toLowerCase()}${el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\s+/).join(".") : ""} «${t}»`;
  };
  const salen = [];
  for (const el of document.body.querySelectorAll("*")) {
    if (!seVe(el)) continue;
    const r = el.getBoundingClientRect();
    if (r.right <= vw + 1 && r.left >= -1) continue;
    let recorta = false;
    for (let p = el.parentElement; p && p !== document.body; p = p.parentElement) {
      if (getComputedStyle(p).overflowX !== "visible") { recorta = true; break; }
    }
    if (!recorta) salen.push(`${nombre(el)} (${Math.round(r.left)} a ${Math.round(r.right)})`);
  }
  const chicos = [];
  for (const el of document.querySelectorAll("a[href], button, summary, [role=button], [role=tab], .copiable")) {
    if (!seVe(el)) continue;
    const enFrase = getComputedStyle(el).display === "inline" &&
      [...el.parentElement.childNodes].some((n) => n !== el && n.nodeType === 3 && n.data.trim());
    if (enFrase) continue;
    const r = el.getBoundingClientRect();
    if (r.height < 43.5) chicos.push(`${nombre(el)} ${Math.round(r.width)}x${Math.round(r.height)}`);
  }
  return { salen, chicos };
}

const navegador = await chromium.launch();
let fallos = 0;

for (const ruta of paginas) {
  for (const tema of ["dark", "light"]) {
    for (const tam of TAMANOS) {
      const ctx = await navegador.newContext({ ...tam.contexto, colorScheme: tema });
      const page = await ctx.newPage();
      const errores = [];
      page.on("console", (m) => { if (m.type() === "error") errores.push(m.text()); });
      page.on("pageerror", (e) => errores.push(String(e)));
      await page.goto(url(ruta), { waitUntil: "load" });
      await page.waitForTimeout(500);

      const nombre = `${(ruta || "portada").replaceAll("/", "-").replace(/-$/, "")}-${tema === "dark" ? "oscuro" : "claro"}-${tam.nombre}.png`;
      await page.screenshot({ path: join(destino, nombre), fullPage: entera });

      const { ancho, visible } = await page.evaluate(() => ({
        ancho: document.documentElement.scrollWidth,
        visible: document.documentElement.clientWidth,
      }));
      const desborda = ancho > visible + 1;
      // Una foto con la ruta mal no deja nada en la consola de file://: se ve rota y ya (pasó
      // con la de novedades en inglés). Así que se cargan todas, también las perezosas y las
      // escondidas a ese ancho, y se mira que cada una tenga píxeles.
      const rotas = await page.evaluate(async () => {
        const locales = [...document.images].filter((i) => i.src.startsWith("file:"));
        for (const i of locales) i.loading = "eager";
        const cargada = (i) => (i.complete ? null : new Promise((ok) => { i.onload = i.onerror = ok; setTimeout(ok, 8000); }));
        await Promise.all(locales.map(cargada));
        return locales.filter((i) => !i.naturalWidth).map((i) => i.getAttribute("src"));
      });
      // Los errores de red de file:// (el contador de GitHub, las fuentes) no son de la web.
      const propios = errores.filter((e) => !/net::ERR|Failed to load resource|api\.github\.com/i.test(e));
      const { salen, chicos } = tam.contexto.isMobile ? await page.evaluate(repasoMovil) : { salen: [], chicos: [] };
      if (desborda || propios.length || rotas.length || salen.length || chicos.length) fallos++;
      console.log(
        `${nombre.padEnd(34)} ${desborda ? `SE SALE (${ancho} > ${visible})` : "ancho bien"}` +
          `${tam.contexto.isMobile ? `${salen.length ? `  SE SALEN: ${salen.join(" | ")}` : ", nada se sale"}${chicos.length ? `  MENOS DE 44 PX: ${chicos.join(" | ")}` : ", todo lo que se pulsa con 44 px o más"}` : ""}` +
          `${rotas.length ? `  FOTOS ROTAS: ${rotas.join(", ")}` : ""}` +
          `${propios.length ? `  ERRORES: ${propios.join(" | ")}` : ""}`,
      );
      await ctx.close();
    }
  }
}

// La barra de arriba se compacta al bajar (como la de cattory.co/es): más estrecha y más
// baja pasados 40 px, sin mover la página, y con «?motion=off» se queda como está.
for (const ruta of paginas) {
  const mirar = async (extra) => {
    const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    await page.goto(url(ruta, extra), { waitUntil: "load" });
    await page.waitForTimeout(300);
    const medir = () => page.evaluate(() => {
      const p = document.querySelector(".barra-dentro").getBoundingClientRect();
      const nav = document.querySelector("nav.barra");
      return { ancho: p.width, alto: p.height, nav: nav.offsetHeight, debajo: nav.nextElementSibling.offsetTop };
    });
    const arriba = await medir();
    await page.evaluate(() => { document.documentElement.style.scrollBehavior = "auto"; scrollTo(0, 300); });
    // Se espera al cambio de estado y luego a que acabe la transición (0,55 s): con el PC
    // cargado, el evento de scroll puede llegar tarde y una espera fija daba un falso fallo.
    await page.waitForSelector("nav.barra.compacta", { timeout: 5000 }).catch(() => {});
    await page.waitForTimeout(900);
    const abajo = await medir();
    await ctx.close();
    return { arriba, abajo };
  };
  const { arriba, abajo } = await mirar("");
  const quieta = await mirar("?motion=off");
  const bien = abajo.ancho < arriba.ancho - 100 && abajo.alto < arriba.alto && abajo.nav === arriba.nav && abajo.debajo === arriba.debajo &&
    Math.abs(quieta.abajo.ancho - quieta.arriba.ancho) < 1;
  console.log(
    `${(ruta || "portada").padEnd(12)} la barra al bajar: de ${Math.round(arriba.ancho)}x${Math.round(arriba.alto)} a ${Math.round(abajo.ancho)}x${Math.round(abajo.alto)}` +
      `${abajo.debajo === arriba.debajo ? ", la página quieta" : `, LA PÁGINA SE MUEVE (${arriba.debajo} → ${abajo.debajo})`}` +
      `; con motion=off ${Math.abs(quieta.abajo.ancho - quieta.arriba.ancho) < 1 ? "se queda igual" : "SE MUEVE"}${bien ? "" : "  MAL"}`,
  );
  if (!bien) fallos++;
}

// El menú del móvil: al tocar «Menú», la misma píldora de la barra crece hacia abajo con las
// páginas dentro (no una tarjeta suelta debajo: Munito, 15-nov), el foco va a la primera, y
// Escape (devolviendo el foco a «Menú») y tocar fuera la recogen, sin mover la página. Arriba
// del todo y con la barra ya compacta, a 390 y a 360. Se toca con el dedo en sus coordenadas:
// `locator.tap()` «trae a la vista» la barra pegada arriba y eso sube la página (408 px).
for (const ruta of paginas) {
  for (const [ancho, bajada] of [[390, 0], [390, 900], [360, 900]]) {
    const ctx = await navegador.newContext({ viewport: { width: ancho, height: 800 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true });
    const page = await ctx.newPage();
    const errores = [];
    page.on("pageerror", (e) => errores.push(String(e)));
    await page.goto(url(ruta), { waitUntil: "load" });
    await page.evaluate((y) => { document.documentElement.style.scrollBehavior = "auto"; scrollTo(0, y); }, bajada);
    await page.waitForTimeout(700);
    const medir = () => page.evaluate(() => {
      const p = document.querySelector(".barra-dentro").getBoundingClientRect();
      const panel = document.querySelector(".menu-panel");
      const r = [...panel.querySelectorAll("a")].filter((a) => getComputedStyle(a).display !== "none").map((a) => a.getBoundingClientRect());
      return {
        alto: p.height, fondo: p.bottom, visible: getComputedStyle(panel).visibility === "visible",
        dentro: r.every((b) => b.top >= p.top - 0.5 && b.bottom <= p.bottom + 0.5 && b.left >= p.left - 0.5 && b.right <= p.right + 0.5),
        chico: Math.min(...r.map((b) => b.height)),
        foco: document.activeElement === panel.querySelector("a") ? "primera" : document.activeElement === document.querySelector(".menu summary") ? "menú" : "otro",
        y: scrollY, debajo: document.querySelector("nav.barra").nextElementSibling.getBoundingClientRect().top,
      };
    });
    const tocar = async () => {
      const b = await page.locator(".menu summary").boundingBox();
      await page.touchscreen.tap(b.x + b.width / 2, b.y + b.height / 2);
      await page.waitForTimeout(900);
    };
    const cerrado = await medir();
    await tocar();
    const abierto = await medir();
    await page.keyboard.press("Escape");
    await page.waitForTimeout(900);
    const trasEscape = await medir();
    await tocar();
    // Fuera: un punto debajo de la píldora abierta sin nada que se pulse en 24 px a la redonda, lo
    // que abarca un dedo. Al dedo, Chromium lo lleva al enlace que tenga a mano (en Novedades,
    // tocar junto a uno abría la versión 0.4.6 en GitHub), como pasa con un dedo de verdad.
    const punto = await page.evaluate(() => {
      const fondo = document.querySelector(".barra-dentro").getBoundingClientRect().bottom;
      const pulsable = "a, button, summary, label, [role=button], [role=tab], [tabindex], input, textarea, .copiable, #demo, .demo-movil, .escaparate, nav, figure, img";
      const libre = (x, y) => {
        const el = document.elementFromPoint(x, y);
        return el && !el.closest(pulsable) && getComputedStyle(el).cursor !== "pointer";
      };
      for (let y = fondo + 40; y < innerHeight - 30; y += 12) {
        for (let x = 30; x < innerWidth - 30; x += 15) {
          let todo = true;
          for (const dx of [-24, 0, 24]) for (const dy of [-24, 0, 24]) if (todo && !libre(x + dx, y + dy)) todo = false;
          if (todo) return [x, y];
        }
      }
      return null;
    });
    if (punto) await page.touchscreen.tap(...punto);
    await page.waitForTimeout(900);
    const trasFuera = await medir();
    await ctx.close();
    const bien = abierto.alto > cerrado.alto + 200 && abierto.visible && abierto.dentro && abierto.chico >= 44 && abierto.foco === "primera" &&
      trasEscape.alto === cerrado.alto && !trasEscape.visible && trasEscape.foco === "menú" && punto && trasFuera.alto === cerrado.alto &&
      abierto.y === cerrado.y && trasFuera.y === cerrado.y && abierto.debajo === cerrado.debajo && !errores.length;
    console.log(
      `${(ruta || "portada").padEnd(12)} el menú a ${ancho}${bajada ? ", con la barra compacta" : ""}: la píldora ${abierto.alto > cerrado.alto + 200 ? `crece de ${Math.round(cerrado.alto)} a ${Math.round(abierto.alto)} px` : "NO CRECE"}` +
        `${abierto.dentro ? " con las páginas dentro" : " CON LAS PÁGINAS FUERA"} (${abierto.chico >= 44 ? "de 44 px o más" : `UNA DE ${Math.round(abierto.chico)} PX`}), el foco ${abierto.foco === "primera" ? "en la primera" : "NO VA A LA PRIMERA"}` +
        `; Escape ${trasEscape.alto === cerrado.alto && trasEscape.foco === "menú" ? "la recoge y vuelve a Menú" : "NO VA BIEN"}; tocar fuera ${!punto ? "SIN SITIO LIBRE PARA TOCAR" : trasFuera.alto === cerrado.alto ? "la recoge" : "NO LA RECOGE"}` +
        `; la página ${abierto.y === cerrado.y && abierto.debajo === cerrado.debajo && trasFuera.y === cerrado.y ? "quieta" : "SE MUEVE"}${errores.length ? `  ERRORES: ${errores.join(" | ")}` : ""}${bien ? "" : "  MAL"}`,
    );
    if (!bien) fallos++;
  }
}

// La comparativa, «¿Cuál usas ahora?» (la estructura 2 que eligió Munito, 15-nov): una pestaña
// por herramienta y un solo panel a la vista, el suyo. A 1440, con el ratón por las diez y con
// las flechas del teclado, sin crecer más que la tabla de antes (1998 px). A 390 y a 360, con
// el dedo: la fila se desliza de lado sin mover la página ni sacar nada por el borde, y tocar
// la última (después de deslizar) enseña la suya. Y a 390 sin JavaScript, que cambia igual.
const ALTO_DE_ANTES = 1998;
for (const ruta of ["", "en/"]) {
  const visibles = (page) => page.evaluate(() =>
    [...document.querySelectorAll(".frente")].filter((p) => getComputedStyle(p).display !== "none").map((p) => p.id.replace("frente-", "")));
  const marcada = (page) => page.evaluate(() => document.querySelector(".rivales :checked")?.value);

  // En el escritorio, con el ratón y el teclado.
  {
    const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    const errores = [];
    page.on("pageerror", (e) => errores.push(String(e)));
    await page.goto(url(ruta), { waitUntil: "load" });
    // Las secciones llevan content-visibility: lejos de la pantalla no se pintan y no se ven.
    await page.evaluate(() => { document.documentElement.style.scrollBehavior = "auto"; document.getElementById("comparativa").scrollIntoView(); });
    await page.waitForTimeout(300);
    const deSalida = { visibles: await visibles(page), medidas: await page.locator("#medidas").isVisible() };
    const mal = [];
    let alto = 0;
    const ids = await page.evaluate(() => [...document.querySelectorAll(".rivales input")].map((i) => i.value));
    for (const id of ids) {
      await page.locator(`.rivales input[value="${id}"]`).click();
      await page.waitForTimeout(80);
      const v = await visibles(page);
      const nombreBien = await page.evaluate((id) => {
        const pestana = document.querySelector(`.rivales input[value="${id}"]`).closest(".pestana").textContent.trim();
        const titulos = document.querySelectorAll(`#frente-${id} .frente-sub`);
        return titulos.length === 2 && titulos[1].textContent.includes(pestana);
      }, id);
      alto = Math.max(alto, await page.evaluate(() => document.getElementById("comparativa").getBoundingClientRect().height));
      if (v.length !== 1 || v[0] !== id || !nombreBien) mal.push(`${id} (se ve ${v.join("+") || "nada"}${nombreBien ? "" : ", sin su nombre"})`);
    }
    // Con el teclado: desde la primera, la flecha a la derecha pasa a la segunda y vuelve.
    await page.locator('.rivales input[value="recortes"]').click();
    await page.keyboard.press("ArrowRight");
    const trasDerecha = [await marcada(page), (await visibles(page)).join("+")];
    await page.keyboard.press("ArrowLeft");
    const trasIzquierda = [await marcada(page), (await visibles(page)).join("+")];
    await ctx.close();
    const teclado = trasDerecha[0] === ids[1] && trasDerecha[1] === ids[1] && trasIzquierda[0] === ids[0] && trasIzquierda[1] === ids[0];
    const bien = deSalida.visibles.join() === "recortes" && deSalida.medidas && !mal.length && teclado && alto <= ALTO_DE_ANTES && !errores.length;
    console.log(
      `${(ruta || "portada").padEnd(12)} la comparativa a 1440: ${deSalida.visibles.join() === "recortes" && deSalida.medidas ? "sale la de Recortes, con las cifras medidas" : `SALE ${deSalida.visibles.join("+") || "NADA"}`}` +
        `; ${mal.length ? `MAL AL PULSAR: ${mal.join(" | ")}` : `las ${ids.length} pestañas enseñan solo la suya, con su nombre`}` +
        `; con las flechas ${teclado ? "va y vuelve" : `NO (${trasDerecha.join(" ")}, ${trasIzquierda.join(" ")})`}` +
        `; mide ${Math.round(alto)} px como mucho${alto <= ALTO_DE_ANTES ? "" : `, MÁS QUE LOS ${ALTO_DE_ANTES} DE ANTES`}${errores.length ? `  ERRORES: ${errores.join(" | ")}` : ""}${bien ? "" : "  MAL"}`,
    );
    if (!bien) fallos++;
  }

  // En el móvil, con el dedo (y a 390, además, sin JavaScript).
  for (const [ancho, js] of [[390, true], [360, true], [390, false]]) {
    const ctx = await navegador.newContext({ viewport: { width: ancho, height: 800 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true, javaScriptEnabled: js });
    const page = await ctx.newPage();
    const errores = [];
    page.on("pageerror", (e) => errores.push(String(e)));
    await page.goto(url(ruta), { waitUntil: "load" });
    // La fila, a media pantalla, como la tendría quien está leyendo la sección.
    await page.evaluate(() => {
      document.documentElement.style.scrollBehavior = "auto";
      const fila = document.querySelector(".rivales");
      scrollTo(0, fila.getBoundingClientRect().top + scrollY - innerHeight / 2);
    });
    await page.waitForTimeout(500);
    const medir = () => page.evaluate(() => {
      const fila = document.querySelector(".rivales");
      const f = fila.getBoundingClientRect();
      const pestanas = [...fila.querySelectorAll(".pestana")].map((p) => p.getBoundingClientRect());
      return {
        y: scrollY, x: scrollX, izquierda: fila.scrollLeft, sobra: fila.scrollWidth - fila.clientWidth, fila: [f.left, f.right, f.top + f.height / 2],
        bajito: Math.min(...pestanas.map((p) => p.height)),
        ancho: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      };
    });
    const antes = await medir();
    // Deslizar con el dedo: tocar en mitad de la fila, arrastrar a la izquierda y soltar. (El
    // gesto de scroll de Chromium, `Input.synthesizeScrollGesture` con dedo, no mueve nada sin
    // ventana; los toques sueltos sí, como un dedo de verdad.)
    const cdp = js ? await ctx.newCDPSession(page) : null;
    const deslizar = async () => {
      const [izq, der, y] = (await medir()).fila;
      const sesion = cdp || await ctx.newCDPSession(page);
      const x0 = der - 50, recorrido = der - izq - 110, pasos = 12;
      await sesion.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: x0, y }] });
      for (let i = 1; i <= pasos; i++) {
        await sesion.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: x0 - (recorrido * i) / pasos, y }] });
        await page.waitForTimeout(16);
      }
      // El dedo se para antes de soltar: soltándolo en marcha la fila sigue corriendo por
      // inercia un par de segundos, y el primer toque solo la frena (en un móvil, igual).
      await page.waitForTimeout(150);
      await sesion.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: x0 - recorrido, y }] });
      await page.waitForTimeout(50);
      await sesion.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
      await page.waitForTimeout(400);
    };
    await deslizar();
    const trasUno = await medir();
    const sigue = await marcada(page); // arrastrar no elige ninguna
    await deslizar();
    await deslizar();
    const alFinal = await medir();
    // La última, con el dedo en sus coordenadas.
    const ultima = await page.locator('.rivales input[value="snagit"]').evaluate((i) => {
      const r = i.closest(".pestana").getBoundingClientRect();
      const x = r.left + r.width / 2, y = r.top + r.height / 2;
      return [x, y, r.right, document.elementFromPoint(x, y) === i];
    });
    await page.touchscreen.tap(ultima[0], ultima[1]);
    await page.waitForTimeout(700);
    const v = await visibles(page);
    const tras = await medir();
    await ctx.close();
    const desliza = antes.sobra > 0 && trasUno.izquierda > antes.izquierda + 40 && alFinal.izquierda >= alFinal.sobra - 2 && sigue === "recortes";
    const quieta = trasUno.y === antes.y && tras.y === antes.y && tras.x === 0 && tras.ancho <= 0;
    const bien = desliza && quieta && v.join() === "snagit" && antes.bajito >= 44 && ultima[2] <= ancho + 1 && !errores.length;
    console.log(
      `${(ruta || "portada").padEnd(12)} la comparativa a ${ancho}${js ? "" : " sin JavaScript"}: la fila ${desliza ? `se desliza con el dedo hasta el final (${Math.round(antes.sobra)} px de más) sin elegir ninguna` : `NO SE DESLIZA BIEN (${antes.izquierda} → ${trasUno.izquierda} → ${alFinal.izquierda} de ${antes.sobra}, marcada ${sigue})`}` +
        `, la página ${quieta ? "quieta y sin salirse" : `SE MUEVE O SE SALE (y ${antes.y} → ${tras.y}, x ${tras.x}, ${tras.ancho} px de más)`}` +
        `; tocar Snagit ${v.join() === "snagit" ? "enseña la suya" : `ENSEÑA ${v.join("+") || "NADA"}${ultima[3] ? "" : " (EL DEDO NO CAE EN ELLA)"}`}${ultima[2] <= ancho + 1 ? "" : " (CORTADA EN EL BORDE)"}` +
        `; pestañas de ${antes.bajito >= 44 ? "44 px o más" : `${Math.round(antes.bajito)} PX`}${errores.length ? `  ERRORES: ${errores.join(" | ")}` : ""}${bien ? "" : "  MAL"}`,
    );
    if (!bien) fallos++;
  }
}

// Las flechas de las notas de la portada (las dibuja leer.js): la punta llega a su texto sin
// tocarlo y unida al trazo, en la dirección en la que llega; la flecha no pisa ningún texto
// ni botón; y la cruceta que pasa por detrás se corta antes de tocarla (se hacen dos fotos,
// con la línea cruzando y sin ella, y ningún píxel cambiado puede caer a menos de 2 px del
// trazo). Munito mandó dos veces la de los disquetes rota, 14-nov.
for (const ruta of ["", "en/"]) {
  for (const ancho of [1180, 1440, 1920, 2560]) {
    for (const tema of ["dark", "light"]) {
      const ctx = await navegador.newContext({ viewport: { width: ancho, height: 1000 }, colorScheme: tema });
      const page = await ctx.newPage();
      await page.goto(url(ruta), { waitUntil: "load" });
      await page.waitForTimeout(400);
      const datos = await page.evaluate(() => {
        const lienzo = document.querySelector(".portada .garabatos");
        const base = lienzo.getBoundingClientRect();
        const cajas = [];
        const env = document.querySelector(".portada > .env");
        const paseo = document.createTreeWalker(env, NodeFilter.SHOW_TEXT);
        for (let n = paseo.nextNode(); n; n = paseo.nextNode()) {
          if (!n.data.trim()) continue;
          const r = document.createRange();
          r.selectNodeContents(n);
          cajas.push(...[...r.getClientRects()].filter((c) => c.width && c.height));
        }
        cajas.push(...[...env.querySelectorAll(".btn")].map((b) => b.getBoundingClientRect()));
        const letra = (el, alFinal) => {
          const textos = [];
          const w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
          for (let n = w.nextNode(); n; n = w.nextNode()) if (n.data.trim()) textos.push(n);
          const n = alFinal ? textos.at(-1) : textos[0];
          const i = alFinal ? n.data.trimEnd().length - 1 : n.data.search(/\S/);
          const r = document.createRange();
          r.setStart(n, i);
          r.setEnd(n, i + 1);
          return r.getBoundingClientRect();
        };
        return [...document.querySelectorAll(".portada .fan[data-flecha]")].map((nota) => {
          const trazo = [...lienzo.querySelectorAll(".trazo")][[...document.querySelectorAll(".portada .fan[data-flecha]")].indexOf(nota)];
          const d = trazo.getAttribute("d") || "";
          const num = d.match(/-?[\d.]+/g)?.map(Number) || [];
          // M ini C c1 c2 fin M p1 L punta L p2
          const [, , , , c2x, c2y, fx, fy, p1x, p1y, px, py, p2x, p2y] = num;
          const alFinal = nota.dataset.punta === "final";
          const t = letra(document.querySelector(nota.dataset.flecha), alFinal);
          const punta = { x: fx + base.left, y: fy + base.top };
          const hueco = alFinal ? punta.x - t.right : t.left - punta.x;
          const unida = num.length === 14 && Math.hypot(px - fx, py - fy) < 0.01;
          const llega = Math.atan2(fy - c2y, fx - c2x);
          const pata = (x, y) => Math.atan2(fy - y, fx - x);
          const dif = (a, b) => Math.abs(((a - b + 3 * Math.PI) % (2 * Math.PI)) - Math.PI);
          const torcida = Math.abs(dif(pata(p1x, p1y), llega) - dif(pata(p2x, p2y), llega)) * 180 / Math.PI;
          const largo = trazo.getTotalLength();
          let pisa = 0;
          for (let s = 0; s <= largo; s += 2) {
            const q = trazo.getPointAtLength(s);
            const x = q.x + base.left, y = q.y + base.top;
            if (cajas.some((c) => x > c.left - 2 && x < c.right + 2 && y > c.top - 2 && y < c.bottom + 2)) pisa++;
          }
          const medio = trazo.getPointAtLength(largo * 0.35);
          return {
            hueco, alto: punta.y >= t.top && punta.y <= t.bottom, unida, torcida, pisa,
            luz: lienzo.classList.contains("con-luz"), cruce: { x: medio.x + base.left, y: medio.y + base.top },
          };
        });
      });
      // La cruceta por encima de cada flecha: con la línea vertical y sin ella.
      const cruza = [];
      for (const [i, f] of datos.entries()) {
        const x = Math.round(f.cruce.x) + 0.5;
        const clip = { x: Math.round(f.cruce.x) - 14, y: Math.round(f.cruce.y) - 14, width: 28, height: 28 };
        // El ratón lejos del cruce (la cruceta deja un hueco a su alrededor) y dentro de la
        // cabecera, que es donde se dibuja.
        const fondo = await page.evaluate(() => document.querySelector(".portada").getBoundingClientRect().bottom);
        const y = f.cruce.y + 70 < fondo - 10 ? f.cruce.y + 70 : f.cruce.y - 70;
        await page.mouse.move(x, y + 30);
        await page.mouse.move(x, y, { steps: 6 });
        await page.waitForTimeout(700);
        const con = (await page.screenshot({ clip })).toString("base64");
        await page.mouse.move(2, 2);
        await page.waitForTimeout(900);
        const sin = (await page.screenshot({ clip })).toString("base64");
        cruza.push(await page.evaluate(async ({ con, sin, clip, i }) => {
          const pix = async (b64) => {
            const img = new Image();
            img.src = `data:image/png;base64,${b64}`;
            await img.decode();
            const c = document.createElement("canvas");
            c.width = img.width;
            c.height = img.height;
            const g = c.getContext("2d");
            g.drawImage(img, 0, 0);
            return g.getImageData(0, 0, c.width, c.height).data;
          };
          const [a, b] = [await pix(con), await pix(sin)];
          const lienzo = document.querySelector(".portada .garabatos");
          const base = lienzo.getBoundingClientRect();
          const cerca = document.createElementNS("http://www.w3.org/2000/svg", "path");
          cerca.setAttribute("d", lienzo.querySelectorAll(".trazo")[i].getAttribute("d"));
          cerca.style.cssText = "stroke: #000; stroke-width: 7px; fill: none; stroke-linecap: round; stroke-linejoin: round";
          lienzo.append(cerca);
          let cambiados = 0;
          let tocan = 0;
          for (let y = 0; y < clip.height; y++) for (let x = 0; x < clip.width; x++) {
            const k = (y * clip.width + x) * 4;
            if (Math.max(Math.abs(a[k] - b[k]), Math.abs(a[k + 1] - b[k + 1]), Math.abs(a[k + 2] - b[k + 2])) <= 8) continue;
            cambiados++;
            if (cerca.isPointInStroke(new DOMPoint(clip.x + x + 0.5 - base.left, clip.y + y + 0.5 - base.top))) tocan++;
          }
          cerca.remove();
          return { cambiados, tocan };
        }, { con, sin, clip, i }));
      }
      await ctx.close();
      datos.forEach((f, i) => {
        const c = cruza[i];
        const bien = f.hueco >= 4 && f.hueco <= 22 && f.alto && f.unida && f.torcida < 8 && !f.pisa && f.luz && c.cambiados > 4 && !c.tocan;
        console.log(
          `flecha ${i + 1} ${(ruta || "es").padEnd(3)} ${ancho} ${tema === "dark" ? "oscuro" : "claro "}: punta a ${f.hueco.toFixed(1)} px de su texto` +
            `${f.alto ? "" : " (A OTRA ALTURA)"}, ${f.unida ? "unida al trazo" : "SUELTA DEL TRAZO"}, patas ${f.torcida < 8 ? "derechas" : `TORCIDAS ${f.torcida.toFixed(0)}°`}` +
            `, ${f.pisa ? `PISA TEXTO EN ${f.pisa} PUNTOS` : "no pisa nada"}; la cruceta ${c.cambiados > 4 ? (c.tocan ? `LA TOCA (${c.tocan} píxeles)` : "pasa por detrás sin tocarla") : "NO SE VE EN LA FOTO"}` +
            `${f.luz ? "" : ", SIN LA LUZ DEL HALO"}${bien ? "" : "  MAL"}`,
        );
        if (!bien) fallos++;
      });
    }
  }
}

// La demo de la portada se encoge y se redondea al bajar, y se sigue usando encogida: con
// la página a media demo (85-95 %), se arrastra una selección, se mueve la captura anclada y
// se escribe en el editor, y se mira que cada cosa caiga donde está el ratón. Se repite con
// la demo a su tamaño, para comparar. Con «?motion=off» no se encoge.
{
  const probarDemo = async (cuanto, extra = "") => {
    const ctx = await navegador.newContext({ viewport: { width: 1440, height: 1300 } });
    const page = await ctx.newPage();
    await page.goto(url("", extra), { waitUntil: "load" });
    await page.evaluate(() => (document.documentElement.style.scrollBehavior = "auto"));
    // Sin que salga el puntero de mentira que enseña la demo la primera vez.
    await page.evaluate(() => window.dispatchEvent(new Event("pointerdown")));
    await page.evaluate((f) => {
      const env = document.querySelector("#demo .env");
      scrollTo(0, env.getBoundingClientRect().top + scrollY - 90 + f * env.offsetHeight);
    }, cuanto);
    await page.waitForTimeout(400);
    const escala = await page.evaluate(() => { const e = document.querySelector("#demo .env"); return e.getBoundingClientRect().width / e.offsetWidth; });
    const lejos = (a, b) => Math.max(...a.map((v, i) => Math.abs(v - b[i])));
    const notas = {};
    // 1. Arrastrar sobre la pantalla de la demo, en la parte que se ve.
    const caja = await page.locator("#caja").boundingBox();
    const desde = [caja.x + caja.width * 0.3, caja.y + caja.height * 0.72];
    const hasta = [caja.x + caja.width * 0.55, caja.y + caja.height * 0.9];
    await page.mouse.move(...desde);
    await page.mouse.down();
    await page.mouse.move(...hasta, { steps: 8 });
    await page.mouse.up();
    await page.waitForTimeout(200);
    const sel = await page.locator("#sel").boundingBox();
    notas.seleccion = lejos([sel.x, sel.y, sel.x + sel.width, sel.y + sel.height], [...desde, ...hasta]);
    // 2. Anclarla y moverla con el ratón.
    await page.evaluate(() => document.querySelector('#herramientas [data-accion="anclar"]').click());
    await page.waitForTimeout(300);
    const antes = await page.locator(".anclada").boundingBox();
    const agarre = [antes.x + antes.width / 2, antes.y + antes.height / 2];
    await page.mouse.move(...agarre);
    await page.mouse.down();
    await page.mouse.move(agarre[0] + 90, agarre[1] - 30, { steps: 6 });
    await page.mouse.up();
    const despues = await page.locator(".anclada").boundingBox();
    notas.anclada = lejos([despues.x - antes.x, despues.y - antes.y], [90, -30]);
    // 3. El editor: el texto se escribe donde se pincha.
    await page.evaluate(() => {
      document.querySelector('.pestana[data-panel="p-edi"]').click();
      document.querySelector('[data-marca="texto"]').click();
    });
    await page.waitForTimeout(400);
    const lienzo = await page.locator("#p-edi .edi-fondo canvas").first().boundingBox();
    const punto = [lienzo.x + lienzo.width * 0.6, Math.min(lienzo.y + lienzo.height * 0.8, 1250)];
    await page.mouse.click(...punto);
    await page.waitForTimeout(200);
    // El campo sale donde se pincha, y se puede escribir en él (antes se cerraba solo).
    const hayCampo = (await page.locator(".edi-escribe").count()) > 0;
    const campo = hayCampo ? await page.locator(".edi-escribe").boundingBox() : null;
    if (hayCampo) await page.keyboard.type("Hola");
    const escrito = hayCampo ? await page.locator(".edi-escribe").inputValue() : "";
    notas.texto = campo && escrito === "Hola" ? lejos([campo.x, campo.y], punto) : 999;
    await ctx.close();
    return { escala, notas };
  };
  for (const [cuanto, extra, que] of [[0, "", "a su tamaño"], [0.45, "", "encogida"], [0.45, "?motion=off", "con motion=off"]]) {
    const { escala, notas } = await probarDemo(cuanto, extra);
    const desvio = Math.max(...Object.values(notas));
    const bien = desvio <= 3 && (que !== "encogida" || escala < 0.95) && (que !== "con motion=off" || escala > 0.999);
    console.log(
      `demo ${que.padEnd(15)} al ${Math.round(escala * 100)} %: la selección se desvía ${notas.seleccion.toFixed(1)} px del ratón, ` +
        `la anclada ${notas.anclada.toFixed(1)} px y el texto ${notas.texto.toFixed(1)} px${bien ? "" : "  MAL"}`,
    );
    if (!bien) fallos++;
  }
}

// La demo en el móvil: tocarla la abre a pantalla completa (fija encima, la página de debajo
// quieta y sin poder tocarse), dentro se recorta con el dedo, y Escape y Atrás la cierran sin
// salir de la página ni moverla. El dedo se para antes de soltar: si suelta en marcha,
// Chromium lo toma por un lanzamiento y se come el toque siguiente (pasa en una página vacía).
for (const ruta of ["", "en/"]) {
  const ctx = await navegador.newContext(devices["iPhone 13"]);
  const page = await ctx.newPage();
  const errores = [];
  page.on("pageerror", (e) => errores.push(String(e)));
  await page.goto(url(ruta), { waitUntil: "load" });
  await page.evaluate(() => {
    document.documentElement.style.scrollBehavior = "auto";
    scrollTo(0, document.querySelector("#demo").getBoundingClientRect().top + scrollY - 96);
  });
  await page.waitForTimeout(400);
  const y0 = await page.evaluate(() => scrollY);
  const estado = () => page.evaluate(() => ({
    abierta: document.querySelector("#demo").classList.contains("en-grande"),
    dialogo: document.querySelector("#demo .env").getAttribute("role") === "dialog",
    dormidas: [...document.body.children].filter((e) => e.inert).length,
    quieta: getComputedStyle(document.documentElement).overflow === "hidden",
    nuestra: Boolean(history.state && history.state.capaDemo),
    y: scrollY,
  }));
  const caja = await page.locator("#pantalla-demo").boundingBox();
  await page.touchscreen.tap(caja.x + caja.width / 2, caja.y + caja.height / 2);
  await page.waitForTimeout(400);
  const abierta = await estado();
  // Recortar con el dedo dentro de la capa.
  await page.evaluate(() => { tocado = true; pararFantasma(); limpiar(); });
  const cdp = await ctx.newCDPSession(page);
  const dedo = (type, x, y) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x, y }] });
  const zona = await page.locator("#caja").boundingBox();
  const desde = [zona.x + zona.width * 0.25, zona.y + zona.height * 0.3];
  const hasta = [zona.x + zona.width * 0.6, zona.y + zona.height * 0.7];
  await dedo("touchStart", ...desde);
  for (let i = 1; i <= 8; i++) {
    await dedo("touchMove", desde[0] + ((hasta[0] - desde[0]) * i) / 8, desde[1] + ((hasta[1] - desde[1]) * i) / 8);
    await page.waitForTimeout(25);
  }
  await page.waitForTimeout(250);
  await dedo("touchEnd");
  await page.waitForTimeout(250);
  const sel = await page.locator("#sel").boundingBox();
  const desvio = Math.max(Math.abs(sel.x - desde[0]), Math.abs(sel.y - desde[1]), Math.abs(sel.x + sel.width - hasta[0]), Math.abs(sel.y + sel.height - hasta[1]));
  const barra = await page.locator("#herramientas").isVisible();
  // Escape: la primera vez quita la selección (como en la app); la segunda cierra.
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(400);
  const trasEscape = await estado();
  // Con el botón de debajo, y Atrás.
  await page.locator('[data-capa="abrir"]').tap();
  await page.waitForTimeout(400);
  const otraVez = await estado();
  const tamanos = await page.evaluate(() => [...document.querySelectorAll(".capa-boton")].map((b) => b.getBoundingClientRect().height));
  const direccion = page.url();
  await page.goBack();
  await page.waitForTimeout(400);
  const trasAtras = await estado();
  tamanos.push(await page.evaluate(() => document.querySelector(".demo-movil .btn").getBoundingClientRect().height));
  await ctx.close();
  const bien = abierta.abierta && abierta.dialogo && abierta.dormidas > 5 && abierta.quieta && abierta.nuestra && abierta.y === y0 &&
    desvio <= 3 && barra && !trasEscape.abierta && !trasEscape.nuestra && !trasEscape.dormidas && trasEscape.y === y0 &&
    otraVez.abierta && !trasAtras.abierta && !trasAtras.dormidas && page.url() === direccion && trasAtras.y === y0 &&
    tamanos.every((h) => h >= 44) && !errores.length;
  console.log(
    `${(ruta || "portada").padEnd(12)} la demo en el móvil: tocarla ${abierta.abierta && abierta.dialogo ? "la abre a pantalla completa" : "NO LA ABRE"}` +
      `${abierta.dormidas > 5 && abierta.quieta ? " con la página quieta debajo" : " CON LA PÁGINA SUELTA"}; recortar con el dedo se desvía ${desvio.toFixed(1)} px` +
      `${barra ? "" : " (SIN LA BARRA)"}; Escape ${!trasEscape.abierta && !trasEscape.nuestra ? "la cierra" : "NO LA CIERRA BIEN"}; Atrás ` +
      `${!trasAtras.abierta && page.url() === direccion ? "la cierra sin salir de la página" : "NO LA CIERRA BIEN"}; la página ${trasAtras.y === y0 ? "no se ha movido" : `SE MOVIÓ (${y0} → ${trasAtras.y})`}` +
      `; sus botones ${tamanos.every((h) => h >= 44) ? "de 44 px o más" : `DE MENOS DE 44 PX (${tamanos.map(Math.round).join(", ")})`}` +
      `${errores.length ? `  ERRORES: ${errores.join(" | ")}` : ""}${bien ? "" : "  MAL"}`,
  );
  if (!bien) fallos++;
}

// Las órdenes de la terminal (winget, PowerShell) se copian con un clic: cada una se pulsa
// con el ratón y con Intro, y se mira lo que llega al portapapeles y que salga su aviso.
for (const ruta of paginas) {
  const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 } });
  await ctx.grantPermissions(["clipboard-read", "clipboard-write"]);
  const page = await ctx.newPage();
  await page.goto(url(ruta), { waitUntil: "load" });
  const piezas = page.locator(".copiable");
  const total = await piezas.count();
  if (!total) { await ctx.close(); continue; }
  const malas = [];
  const rotas = new Set();
  for (let i = 0; i < total; i++) {
    const pieza = piezas.nth(i);
    // Las de las preguntas van plegadas: se abre la suya.
    if (await pieza.evaluate((e) => { const d = e.closest("details"); if (d && !d.open) { d.querySelector("summary").click(); return true; } return false; })) {
      await page.waitForTimeout(500);
    }
    const orden = await pieza.evaluate((e) => e.getAttribute("aria-label").replace(/^[^:]+: /, ""));
    for (const como of ["ratón", "teclado"]) {
      await page.evaluate(() => navigator.clipboard.writeText("-"));
      await pieza.scrollIntoViewIfNeeded();
      if (como === "ratón") await pieza.click();
      else { await pieza.focus(); await page.keyboard.press("Enter"); }
      await page.waitForTimeout(300);
      const llego = await page.evaluate(() => navigator.clipboard.readText());
      const aviso = await pieza.evaluate((e) => {
        const a = e.querySelector(".copiado") || document.querySelector(".copiado-globo");
        const r = a.getBoundingClientRect();
        return { se_ve: getComputedStyle(a).opacity === "1", dentro: r.left >= 0 && r.right <= innerWidth && r.top >= 0 };
      });
      if (llego !== orden) malas.push(`${orden} (${como}: llegó «${llego}»)`);
      else if (!aviso.se_ve || !aviso.dentro) malas.push(`${orden} (${como}: el aviso ${aviso.se_ve ? "se sale" : "no sale"})`);
      else continue;
      rotas.add(i);
    }
  }
  console.log(`${(ruta || "portada").padEnd(12)} órdenes que se copian con un clic: ${total - rotas.size} de ${total}${malas.length ? `  MAL: ${malas.join(" | ")}` : ""}`);
  if (malas.length) fallos++;
  await ctx.close();
}

if (medirCruceta) {
  const medir = async (extra, cpu) => {
    const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    const errores = [];
    page.on("pageerror", (e) => errores.push(String(e)));
    page.on("console", (m) => { if (m.type() === "error") errores.push(m.text()); });
    await page.goto(url("", extra), { waitUntil: "load" });
    const cdp = await ctx.newCDPSession(page);
    if (cpu > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate: cpu });
    await page.mouse.move(200, 300);
    await page.waitForTimeout(400);

    // Un fotograma es cada vuelta de `requestAnimationFrame` de la página: si la cruceta
    // pesara, se alargarían. Y se apunta en cuántos de ellos se movió de verdad la línea.
    await page.evaluate(() => {
      window.__cuadros = [];
      window.__movidos = 0;
      const linea = document.querySelector(".cr-izq");
      let antes = "";
      const contar = (t) => {
        window.__cuadros.push(t);
        const ahora = linea ? linea.style.transform : "";
        if (ahora !== antes) { window.__movidos++; antes = ahora; }
        if (!window.__parar) requestAnimationFrame(contar);
      };
      requestAnimationFrame(contar);
    });
    const t0 = Date.now();
    let i = 0;
    while (Date.now() - t0 < 2000) {
      const x = 200 + ((i * 23) % 1000);
      const y = 250 + Math.round(Math.sin(i / 6) * 120);
      await page.mouse.move(x, y);
      i++;
    }
    const datos = await page.evaluate(() => {
      window.__parar = true;
      const c = window.__cuadros;
      const huecos = c.slice(1).map((t, k) => t - c[k]);
      const capa = document.querySelector(".cruceta");
      const cursor = getComputedStyle(document.body).cursor;
      const html = getComputedStyle(document.documentElement).cursor;
      return {
        hay: !!capa,
        cuadros: c.length,
        movidos: window.__movidos,
        ms: c.length > 1 ? c[c.length - 1] - c[0] : 0,
        peor: huecos.length ? Math.max(...huecos) : 0,
        lentos: huecos.filter((h) => h > 20).length,
        cursorTapado: cursor === "none" || html === "none",
        pointerEvents: capa ? getComputedStyle(capa).pointerEvents : "",
        directa: capa ? capa.dataset.directo === "1" : false,
      };
    });
    await ctx.close();
    return { ...datos, errores };
  };

  for (const [extra, cpu, que] of [["", 1, "normal"], ["", 4, "CPU 4x"], ["?motion=off", 4, "motion=off, CPU 4x"]]) {
    const d = await medir(extra, cpu);
    const fps = d.ms ? (d.cuadros - 1) / (d.ms / 1000) : 0;
    console.log(
      `cruceta ${que.padEnd(20)} ${d.hay ? "" : "NO ESTÁ  "}${fps.toFixed(1)} fps, peor hueco ${d.peor.toFixed(1)} ms, ` +
        `${d.lentos} huecos de más de 20 ms, la línea se movió en ${d.movidos} de ${d.cuadros}; ` +
        `cursor ${d.cursorTapado ? "TAPADO" : "a la vista"}; ` +
        `clics ${d.pointerEvents === "none" ? "pasan a través" : "SE LOS QUEDA LA CRUCETA"}` +
        `${extra ? `; ${d.directa ? "pegada al ratón, sin suavizado ni onda" : "SIGUE CON SUAVIZADO"}` : ""}` +
        `${d.errores.length ? `  ERRORES: ${d.errores.join(" | ")}` : ""}`,
    );
    if (!d.hay || !d.movidos || d.cursorTapado || d.pointerEvents !== "none" || d.errores.length) fallos++;
    if (extra && !d.directa) fallos++;
  }

  // El scroll por la portada (la barra que se compacta, la demo que se encoge al irse), con la
  // CPU a 4x, con tarjeta gráfica y sin ella (como el ordenador de clase de Munito). Se cuentan
  // los fotogramas que LLEGAN A LA PANTALLA con el scroll movido (la emisión de pantalla de
  // Chrome, que trae la posición de cada uno), no los de la página: el scroll del navegador va
  // en otro hilo y los de la página mienten. Es la medida de `medir-scroll-visible.mjs` de la
  // skill frontlaxweb. Sin gráfica hace falta el Chromium entero sin ventana (`channel:
  // "chromium"`): el de siempre dibuja con gráfica aunque se le diga que no.
  const medirScroll = async (sinGrafica, destino = url("")) => {
    const nav = await chromium.launch({ channel: "chromium", args: sinGrafica ? ["--disable-gpu", "--disable-gpu-compositing"] : [] });
    const ctx = await nav.newContext({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
    const page = await ctx.newPage();
    const errores = [];
    page.on("pageerror", (e) => errores.push(String(e)));
    await page.goto(destino, { waitUntil: "load" });
    const grafica = await page.evaluate(() => Boolean(document.createElement("canvas").getContext("webgl", { failIfMajorPerformanceCaveat: true })));
    await page.mouse.move(900, 500);
    await page.mouse.move(960, 600, { steps: 5 });
    await page.waitForTimeout(1500);
    // El modo ligero (leer.js): se pone solo sin gráfica, después del primer pintado.
    const ligero = await page.evaluate(() => document.documentElement.hasAttribute("data-ligero"));
    const cdp = await ctx.newCDPSession(page);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
    const cuadros = [];
    cdp.on("Page.screencastFrame", (f) => {
      cuadros.push({ t: f.metadata.timestamp * 1000, y: f.metadata.scrollOffsetY });
      cdp.send("Page.screencastFrameAck", { sessionId: f.sessionId }).catch(() => {});
    });
    await cdp.send("Page.startScreencast", { format: "jpeg", quality: 5, everyNthFrame: 1, maxWidth: 192, maxHeight: 108 });
    await page.waitForTimeout(400);
    const t0 = Date.now();
    // De arriba del todo hasta pasada la demo: la barra se compacta y la demo se encoge entera.
    await cdp.send("Input.synthesizeScrollGesture", { x: 960, y: 600, yDistance: -2600, speed: 1200, gestureSourceType: "mouse" });
    let ultimo = -1;
    for (let i = 0, quieto = 0; i < 40 && quieto < 3; i++) {
      await page.waitForTimeout(250);
      const y = await page.evaluate(() => Math.round(scrollY));
      quieto = y === ultimo ? quieto + 1 : 0;
      ultimo = y;
    }
    const t1 = Date.now();
    await cdp.send("Page.stopScreencast");
    const escala = await page.evaluate(() => { const e = document.querySelector("#demo .env"); return e.getBoundingClientRect().width / e.offsetWidth; });
    await nav.close();
    const movidos = [];
    let antes = null;
    for (const c of cuadros.filter((c) => c.t >= t0 && c.t <= t1)) {
      if (antes !== null && Math.abs(c.y - antes) > 0.5) movidos.push(c);
      antes = c.y;
    }
    let paron = 0;
    for (let i = 1; i < movidos.length; i++) paron = Math.max(paron, movidos[i].t - movidos[i - 1].t);
    const seg = movidos.length > 1 ? (movidos.at(-1).t - movidos[0].t) / 1000 : 0;
    return { fps: seg ? movidos.length / seg : 0, paron, llego: ultimo, grafica, ligero, escala, errores };
  };
  // Con `--publicada=<carpeta de la web publicada>` (sacada con `git archive origin/main
  // frontlaxweb`) se mide también esa portada en la misma tanda: con el PC cargado, la cifra
  // suelta engaña, y lo que vale es la diferencia entre las dos medidas a la vez.
  const publicada = args.find((a) => a.startsWith("--publicada="));
  // Una pasada que no acaba (el PC ahogado puede dejar el gesto de scroll sin terminar) cuenta
  // como 0 fotogramas, en vez de dejar la prueba colgada.
  const conTope = (promesa) => Promise.race([promesa, new Promise((ok) => setTimeout(() => ok(null), 90000))]);
  const vacia = { fps: 0, paron: 9999, llego: 0, grafica: null, ligero: null, escala: 1, errores: ["más de 90 s"] };
  const mediana = (lista, k) => [...lista].sort((a, b) => a[k] - b[k])[Math.floor(lista.length / 2)];
  for (const sinGrafica of [false, true]) {
    const modo = sinGrafica ? "sin gráfica, CPU 4x" : "con gráfica, CPU 4x";
    // Lo fluido de la casa: cerca de 60 fotogramas con gráfica y de 45 sin ella (lo que da
    // cloudflare.com en las mismas condiciones), y ningún parón que se vea.
    const minimo = sinGrafica ? 38 : 50;
    const nuevas = [];
    const viejas = [];
    // Con la publicada al lado, tres pasadas de cada, intercaladas, y la mediana: así el PC
    // cargado pesa igual en las dos.
    for (let vez = 0; vez < (publicada ? 3 : 1); vez++) {
      if (publicada) viejas.push((await conTope(medirScroll(sinGrafica, pathToFileURL(join(resolve(publicada.slice(12)), "index.html")).href))) || vacia);
      nuevas.push((await conTope(medirScroll(sinGrafica))) || vacia);
    }
    const d = { ...mediana(nuevas, "fps"), paron: mediana(nuevas, "paron").paron };
    const p = publicada ? { fps: mediana(viejas, "fps").fps, paron: mediana(viejas, "paron").paron } : null;
    if (p) console.log(`scroll ${modo}  la publicada: ${p.fps.toFixed(1)} fotogramas por segundo, el parón más largo ${Math.round(p.paron)} ms (mediana de ${viejas.map((v) => v.fps.toFixed(1)).join(", ")})`);
    // Fluida: llega a lo de la casa, o, con el PC tan cargado que ni la publicada llega, le
    // gana por un 10 % sin parones mayores que los suyos.
    const fluida = (d.fps >= minimo && d.paron < 120) || (p && p.fps < minimo && d.fps >= p.fps * 1.1 && d.paron <= Math.max(120, p.paron));
    const bien = fluida && nuevas.every((n) => n.grafica === !sinGrafica && n.ligero === sinGrafica && n.escala < 0.9 && !n.errores.length);
    const errores = [...new Set(nuevas.flatMap((n) => n.errores))];
    console.log(
      `scroll ${modo}  ${d.fps.toFixed(1)} fotogramas por segundo en pantalla, ` +
        `el parón más largo ${Math.round(d.paron)} ms (hasta ${d.llego} px; la demo acabó al ${Math.round(d.escala * 100)} %)` +
        `${publicada ? ` (mediana de ${nuevas.map((n) => n.fps.toFixed(1)).join(", ")})` : ""}` +
        `${d.fps < minimo && fluida ? `, por debajo de ${minimo} pero un ${Math.round((d.fps / p.fps - 1) * 100)} % por encima de la publicada` : ""}` +
        `${d.ligero ? ", modo ligero" : ""}${nuevas.every((n) => n.ligero === sinGrafica) ? "" : "  EL MODO LIGERO NO SE PUSO COMO TOCA"}` +
        `${nuevas.every((n) => n.grafica === !sinGrafica) ? "" : "  OJO: NO DIBUJA COMO SE PIDIÓ"}${errores.length ? `  ERRORES: ${errores.join(" | ")}` : ""}${bien ? "" : "  MAL"}`,
    );
    if (!bien) fallos++;
  }
}

await navegador.close();
console.log(fallos ? `\n${fallos} con algo que mirar. Fotos en ${destino}` : `\nTodo en orden. Fotos en ${destino}`);
process.exit(fallos ? 1 : 0);
