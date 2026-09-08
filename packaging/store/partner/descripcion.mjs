// Cambia la descripcion y las novedades de la ficha, en los dos idiomas, de principio a fin.
//
//   PERFIL=<perfil> ENVIO=<id> node packaging/store/partner/descripcion.mjs
//   PERFIL=<perfil> ENVIO=<id> SOLO_MIRAR=1 node ...    vuelca los campos y no toca nada
//
// El camino real, descubierto pulsando en el menu: «Descripciones de Store» no lleva a la
// ficha, lleva a `/managelanguages`, que es la lista de idiomas, y desde ahi se entra a cada
// uno. La URL `/listings` a secas se queda en blanco para siempre.
//
// El ENVIO cambia cada vez que se actualiza el producto (lo crea `actualizar.mjs` y lo
// imprime). Tenerlo escrito aqui dentro significaba que el guion seguia escribiendo en el
// envio del mes pasado, que ademas es de solo lectura en cuanto la app se publica.

import { chromium } from "playwright";
import { readFileSync } from "node:fs";

const PERFIL = process.env.PERFIL;
const ENVIO = process.env.ENVIO;
const SOLO_MIRAR = process.env.SOLO_MIRAR === "1";
if (!PERFIL || !ENVIO) {
  console.error("Faltan PERFIL=<carpeta con la sesion> y ENVIO=<id del envio>.");
  console.error("El id del envio lo imprime actualizar.mjs.");
  process.exit(1);
}
const ID = process.env.STORE_ID ?? "9P1NKWNRXD6Z";
const BASE = `https://partner.microsoft.com/es-es/dashboard/products/${ID}/submissions/${ENVIO}`;
const FICHA = JSON.parse(readFileSync("C:/proyectos/winshotx/packaging/store/ficha.json", "utf8"));

const ctx = await chromium.launchPersistentContext(PERFIL, {
  channel: "chrome",
  headless: false,
  viewport: { width: 1500, height: 1100 },
  args: ["--disable-blink-features=AutomationControlled"],
});
const page = ctx.pages()[0] ?? (await ctx.newPage());

/** Espera a que la pagina tenga un textarea con texto de verdad dentro. */
async function esperarFicha(segundos = 180) {
  for (let i = 0; i < segundos / 5; i++) {
    await page.waitForTimeout(5000);
    for (const a of await page.locator("textarea").all()) {
      if (((await a.inputValue().catch(() => "")) || "").length > 200) return true;
    }
  }
  return false;
}

/**
 * Los textarea de la ficha con la etiqueta que tienen encima. No hay `label` que valga:
 * se lee el texto del bloque que los envuelve, que es lo unico estable de esta pagina.
 */
async function campos() {
  return page.locator("textarea").evaluateAll((as) =>
    as.map((a, i) => {
      let n = a, etiqueta = "";
      for (let s = 0; s < 5 && n; s++) {
        n = n.parentElement;
        const t = (n?.innerText || "").trim();
        if (t && t.length < 400) etiqueta = t.split("\n")[0];
        if (etiqueta) break;
      }
      return { i, etiqueta, largo: (a.value || "").length, cabeza: (a.value || "").slice(0, 60) };
    }));
}

await page.goto(`${BASE}/managelanguages?producttype=app`, { waitUntil: "domcontentloaded", timeout: 90000 });
await page.waitForTimeout(20000);

const enlaces = await page.evaluate(() =>
  [...document.querySelectorAll("a")]
    .map((a) => ({ t: (a.innerText || "").trim(), h: a.getAttribute("href") || "" }))
    .filter((x) => /listings|languagecode|languageid/i.test(x.h)),
);
console.log("=== ENLACES A FICHAS ===");
for (const e of enlaces) console.log(`  "${e.t}" -> ${e.h}`);

if (!enlaces.length) {
  await page.screenshot({ path: `${PERFIL}/../idiomas.png`, fullPage: true }).catch(() => {});
  console.log("NO hay enlaces de ficha; queda la foto idiomas.png para mirarla");
  await ctx.close();
  process.exit(1);
}

for (const { t: nombre, h } of enlaces) {
  const url = h.startsWith("http") ? h : `https://partner.microsoft.com${h}`;
  console.log(`\n=== ${nombre || url} ===`);
  await page.goto(url, { waitUntil: "domcontentloaded", timeout: 90000 });
  if (!(await esperarFicha())) {
    console.log("  la ficha no ha cargado, la salto");
    continue;
  }

  const lista = await campos();
  const desc = lista.reduce((a, b) => (b.largo > a.largo ? b : a), lista[0]);
  const code = (url.match(/languagecode=([a-zA-Z-]+)/) || [])[1]?.toLowerCase();
  const clave = code && FICHA[code] ? code : desc.cabeza.startsWith("Requires") ? "en-us" : "es-es";
  console.log(`  idioma: ${clave}`);
  for (const c of lista) console.log(`   [${c.i}] ${c.largo} chars · ${c.etiqueta.slice(0, 60)}`);

  // Las novedades son otro textarea. Su etiqueta NO dice «Novedades»: dice «Proporciona
  // notas de la version que indican lo que ha cambiado», asi que se busca por ahi.
  const nov = lista.find((c) => c.i !== desc.i &&
    /notas de la versi|release notes|novedad|what's new/i.test(c.etiqueta));
  console.log(`  descripcion -> [${desc.i}] | novedades -> ${nov ? `[${nov.i}]` : "no encontrado"}`);

  if (SOLO_MIRAR) continue;

  const areas = await page.locator("textarea").all();
  let tocado = false;

  // Se compara el texto ENTERO. Antes se miraba solo la primera linea, que es la
  // declaracion de dependencias y no cambia nunca: cualquier correccion posterior se
  // daba por hecha y el guion decia «ya esta» sin haber escrito nada.
  const actual = (await areas[desc.i].inputValue().catch(() => "")) || "";
  if (actual.trim() === FICHA[clave].descripcion.trim()) {
    console.log("  descripcion: ya es la de ficha.json");
  } else {
    await areas[desc.i].fill(FICHA[clave].descripcion);
    console.log("  descripcion: puesta");
    tocado = true;
  }

  if (nov && FICHA[clave].novedades) {
    const hoy = (await areas[nov.i].inputValue().catch(() => "")) || "";
    if (hoy.trim() === FICHA[clave].novedades.trim()) {
      console.log("  novedades: ya son las de ficha.json");
    } else {
      await areas[nov.i].fill(FICHA[clave].novedades);
      console.log("  novedades: puestas");
      tocado = true;
    }
  }

  if (!tocado) continue;

  await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
  await page.waitForTimeout(2500);
  const guardar = page.locator('text=/^\\s*(Save|Guardar)\\s*$/');
  if ((await guardar.count()) > 0) {
    await guardar.first().scrollIntoViewIfNeeded().catch(() => {});
    await guardar.first().click({ timeout: 25000 }).catch((e) => console.log("  fallo:", e.message.slice(0, 100)));
    await page.waitForTimeout(20000);
    console.log("  GUARDADO");
  } else {
    console.log("  SIN botón de guardar");
  }
  await page.screenshot({ path: `${PERFIL}/../ficha-${clave}.png`, fullPage: true }).catch(() => {});
}

await ctx.close();
console.log("FIN");
