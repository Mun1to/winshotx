// Mira la web como la vería alguien, sin abrir nada en el escritorio: Chromium sin ventana.
//
//   node frontlaxweb/probar-web.mjs                      fotos de la portada en claro y oscuro
//   node frontlaxweb/probar-web.mjs novedades/ docs/     otras páginas (la portada siempre va)
//   node frontlaxweb/probar-web.mjs --entera             la página completa, no solo lo de arriba
//   node frontlaxweb/probar-web.mjs --cruceta            además, mide la cruceta de la portada
//
// En cada página, además, se mira que la barra se compacte al bajar sin mover la página, y
// cada orden de la terminal se copia con el ratón y con Intro, mirando el portapapeles y que
// salga el aviso de copiado.
//
// Cada página sale en oscuro y en claro, a 1920 (la pantalla de Munir), a 1440 y en un
// iPhone 13, y en las tres se apuntan los errores de consola y si algo se sale por un lado.
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
];

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
      if (desborda || propios.length || rotas.length) fallos++;
      console.log(
        `${nombre.padEnd(34)} ${desborda ? `SE SALE (${ancho} > ${visible})` : "ancho bien"}` +
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
