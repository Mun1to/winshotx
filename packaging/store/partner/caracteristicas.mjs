// Solo las caracteristicas de la ficha, en los dos idiomas, y comprobandolo despues.
//
//   PERFIL=<perfil> ENVIO=<id> SOLO_MIRAR=1 node .../caracteristicas.mjs   cuenta lo que hay
//   PERFIL=<perfil> ENVIO=<id> node .../caracteristicas.mjs                las escribe
//
// Van aparte porque se perdieron una vez: se guardo la pagina cuando las filas todavia no
// habian terminado de cargar, y el formulario las mando vacias sin quejarse de nada. Aqui
// se recarga al final y se cuenta lo que hay de verdad.
//
// Las filas no tienen nada que las identifique: son `input[type=text]` sueltos. Lo unico
// estable es que su bloque contiene el boton «Agregar mas», y por ahi se encuentran.

import { chromium } from "playwright";
import { readFileSync } from "node:fs";

const PERFIL = process.env.PERFIL;
const ENVIO = process.env.ENVIO;
const SOLO_MIRAR = process.env.SOLO_MIRAR === "1";
if (!PERFIL || !ENVIO) {
  console.error("Faltan PERFIL=<carpeta con la sesion> y ENVIO=<id del envio>.");
  process.exit(1);
}
const ID = process.env.STORE_ID ?? "9P1NKWNRXD6Z";
const FICHA = JSON.parse(readFileSync("C:/proyectos/winshotx/packaging/store/ficha.json", "utf8"));
const IDIOMAS = [["es-es", 15], ["en-us", 4]];

const ctx = await chromium.launchPersistentContext(PERFIL, {
  channel: "chrome",
  headless: false,
  viewport: { width: 1500, height: 1100 },
  args: ["--disable-blink-features=AutomationControlled"],
});
const page = ctx.pages()[0] ?? (await ctx.newPage());

/** Indice del primer `input` de caracteristica, o -1. Su bloque lleva «Agregar mas». */
const primeraFila = () => page.evaluate(() => {
  const els = [...document.querySelectorAll("input[type=text]")];
  for (let i = 0; i < els.length; i++) {
    let p = els[i], t = "";
    for (let n = 0; n < 8 && p; n++) { p = p.parentElement; if (p) { t = p.innerText || ""; if (t.trim().length > 12) break; } }
    if (t.includes("Agregar más") || t.includes("Add more")) return i;
  }
  return -1;
});

const valores = () => page.evaluate(() => [...document.querySelectorAll("input[type=text]")]
  .filter((el) => {
    let p = el, t = "";
    for (let n = 0; n < 8 && p; n++) { p = p.parentElement; if (p) { t = p.innerText || ""; if (t.trim().length > 12) break; } }
    return t.includes("Agregar más") || t.includes("Add more");
  })
  .map((e) => e.value));

for (const [idioma, langid] of IDIOMAS) {
  const URL = `https://partner.microsoft.com/es-es/dashboard/products/${ID}/submissions/${ENVIO}/listings?languageid=${langid}&languagecode=${idioma}`;
  const lista = FICHA[idioma].caracteristicas;

  await page.goto(URL, { waitUntil: "domcontentloaded", timeout: 90000 });
  await page.waitForTimeout(22000);

  const base = await primeraFila();
  const hay = (await valores()).filter(Boolean);
  console.log(`\n=== ${idioma} === primera fila en el índice ${base} | con texto: ${hay.length} de ${lista.length}`);
  for (const v of hay) console.log(`   · ${v.slice(0, 90)}`);

  if (SOLO_MIRAR) continue;
  if (base < 0) {
    console.log("  no encuentro las filas, la salto");
    continue;
  }

  const iguales = hay.length === lista.length && hay.every((v, i) => v.trim() === lista[i].trim());
  if (iguales) {
    console.log("  ya son las de ficha.json, no se toca");
    continue;
  }

  await page.locator("input[type=text]").nth(base).fill(lista[0]);
  for (let i = 1; i < lista.length; i++) {
    if ((await valores()).length <= i) {
      await page.locator('text="Agregar más"').first().click();
      await page.waitForTimeout(900);
    }
    await page.locator("input[type=text]").nth(base + i).fill(lista[i]);
  }
  console.log("  escritas:", (await valores()).filter(Boolean).length, "de", lista.length);

  const g = page.locator('text="Guardar"').first();
  await g.scrollIntoViewIfNeeded().catch(() => {});
  await g.click().catch((e) => console.log("  fallo al guardar:", e.message.slice(0, 90)));
  await page.waitForTimeout(20000);

  // Y comprobarlo de verdad: recargar y contar.
  await page.goto(URL, { waitUntil: "domcontentloaded", timeout: 90000 });
  await page.waitForTimeout(22000);
  const quedan = (await valores()).filter(Boolean);
  console.log(`  GUARDADAS ${quedan.length} de ${lista.length}`);
  if (quedan.length) console.log("  primera:", quedan[0].slice(0, 60));
}

await ctx.close();
