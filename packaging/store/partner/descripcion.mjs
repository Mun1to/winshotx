// Cambia la descripcion de la ficha en los dos idiomas, de principio a fin.
//
// El camino real, descubierto pulsando en el menu: «Descripciones de Store» no lleva a la
// ficha, lleva a `/managelanguages`, que es la lista de idiomas, y desde ahi se entra a cada
// uno. La URL `/listings` a secas se queda en blanco para siempre.

import { chromium } from "playwright";
import { readFileSync } from "node:fs";

const PERFIL = process.env.PERFIL;
const ID = "9P1NKWNRXD6Z";
const ENVIO = "1152921505701771888";
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

await page.goto(`${BASE}/managelanguages?producttype=app`, { waitUntil: "domcontentloaded", timeout: 90000 });
await page.waitForTimeout(20000);
console.log("=== PANTALLA DE IDIOMAS ===");
const texto = (await page.locator("body").innerText().catch(() => "")).replace(/\n{2,}/g, "\n");
const i0 = texto.indexOf("Envío 1");
console.log(texto.slice(i0 >= 0 ? i0 : 0, (i0 >= 0 ? i0 : 0) + 1200));

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
  process.exit(0);
}

for (const { t: nombre, h } of enlaces) {
  const url = h.startsWith("http") ? h : `https://partner.microsoft.com${h}`;
  console.log(`=== ${nombre || url} ===`);
  await page.goto(url, { waitUntil: "domcontentloaded", timeout: 90000 });
  if (!(await esperarFicha())) {
    console.log("  la ficha no ha cargado, la salto");
    continue;
  }

  const areas = await page.locator("textarea").all();
  let iDesc = 0;
  let largo = -1;
  for (let i = 0; i < areas.length; i++) {
    const n = ((await areas[i].inputValue().catch(() => "")) || "").length;
    if (n > largo) {
      largo = n;
      iDesc = i;
    }
  }
  const actual = (await areas[iDesc].inputValue().catch(() => "")) || "";
  const code = (url.match(/languagecode=([a-zA-Z-]+)/) || [])[1]?.toLowerCase();
  const clave = code && FICHA[code] ? code : actual.startsWith("winshotx is") ? "en-us" : "es-es";
  console.log(`  idioma: ${clave} | campo [${iDesc}] con ${largo} chars`);

  if (actual.startsWith(FICHA[clave].descripcion.split("\n")[0])) {
    console.log("  ya tiene la declaración puesta, no se toca");
    continue;
  }

  await areas[iDesc].fill(FICHA[clave].descripcion);
  console.log("  puesto:", ((await areas[iDesc].inputValue()) || "").slice(0, 70).replace(/\n/g, " "));

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
