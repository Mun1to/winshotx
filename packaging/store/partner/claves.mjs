// Las palabras clave de la ficha, que son con lo que la Store te encuentra buscando.
//
// Van aparte porque el control no es un campo de texto: es un `he-select` multiple con
// `freeform`, dentro de `#search-terms`. No sale al listar los `input[type=text]` de la
// pagina, y por eso el primer envio se fue sin ellas.
import { ctx, page } from "./partner.mjs";
import { readFileSync } from "node:fs";
const FICHA = JSON.parse(readFileSync("C:/proyectos/winshotx/packaging/store/ficha.json", "utf8"));
const idioma = process.env.IDIOMA, langid = process.env.LANGID;
const claves = FICHA[idioma].palabrasClave ?? [];
const URL = `https://partner.microsoft.com/es-es/dashboard/products/9P1NKWNRXD6Z/submissions/1152921505701771888/listings?languageid=${langid}&languagecode=${idioma}`;

await page.goto(URL, { waitUntil: "domcontentloaded", timeout: 60000 });
await page.waitForTimeout(17000);

const caja = page.locator("#search-terms").getByRole("combobox").first();
console.log("campo encontrado:", await caja.count());
await caja.scrollIntoViewIfNeeded();

// Fuera lo que hubiera: el limite son 7 y este control no avisa de que se pasa.
for (let i = 0; i < 12; i++) {
  const equis = page.locator("#search-terms he-icon[name='cancel'], #search-terms [class*='remove']").first();
  if (!(await equis.count())) break;
  await equis.click().catch(() => {});
  await page.waitForTimeout(700);
}

// Se teclea, no se rellena de golpe: con `fill` el texto acaba mezclado con las palabras
// que el propio control recomienda al abrirse, y sale una sola clave pegada sin sentido.
//
// Y despues del Enter va un **Tab**, que es el hallazgo. El Enter pinta la etiqueta y
// parece hecho, pero sin sacar el foco del campo la palabra no se confirma: se guardaba
// sin error y al recargar no habia ninguna. Las que se eligen de la lista de sugerencias
// si persistian, y esa fue la pista de que el problema era el texto escrito a mano.
for (const palabra of claves) {
  await caja.click();
  await page.waitForTimeout(500);
  await caja.pressSequentially(palabra, { delay: 45 });
  await page.waitForTimeout(700);
  await page.keyboard.press("Enter");
  await page.waitForTimeout(700);
  await page.keyboard.press("Tab");
  await page.waitForTimeout(900);
  const dentro = await page.locator("#search-terms").innerText();
  console.log(`  «${palabra}» -> ${dentro.includes(palabra) ? "ok" : "NO ENTRO"}`);
}

await page.locator("h2").first().click({ force: true }).catch(() => {});
await page.waitForTimeout(1500);

const g = page.locator('text="Guardar"').first();
await g.scrollIntoViewIfNeeded();
await g.click();
await page.waitForTimeout(20000);

// Recargar y contar de verdad, que un guardado sin error no es un guardado.
await page.goto(URL, { waitUntil: "domcontentloaded", timeout: 60000 });
await page.waitForTimeout(17000);
const puestas = await page.evaluate(() => {
  const z = document.querySelector("#search-terms");
  if (!z) return [];
  const rec = (r, out) => { r.querySelectorAll("*").forEach(el => {
    const t = (el.textContent||"").trim();
    if (el.children.length === 0 && t && t.length < 45) out.push(t);
    if (el.shadowRoot) rec(el.shadowRoot, out);
  }); return out; };
  return [...new Set(rec(z, []))];
});
const cuantas = claves.filter((c) => puestas.some(p => p === c)).length;
console.log(`GUARDADAS: ${cuantas} de ${claves.length}`);
console.log("  en el campo:", puestas.filter(p => claves.includes(p)).join(" | ") || "(ninguna)");
await ctx.close();
