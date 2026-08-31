// Solo las caracteristicas de la ficha, y comprobandolo despues.
//
// Van aparte porque se perdieron una vez: se guardo la pagina cuando las filas todavia no
// habian terminado de cargar, y el formulario las mando vacias sin quejarse de nada. Aqui
// se recarga al final y se cuenta lo que hay de verdad.
import { ctx, page } from "./partner.mjs";
import { readFileSync } from "node:fs";
const FICHA = JSON.parse(readFileSync("C:/proyectos/winshotx/packaging/store/ficha.json", "utf8"));
const idioma = process.env.IDIOMA, langid = process.env.LANGID;
const lista = FICHA[idioma].caracteristicas;
const URL = `https://partner.microsoft.com/es-es/dashboard/products/9P1NKWNRXD6Z/submissions/1152921505701771888/listings?languageid=${langid}&languagecode=${idioma}`;

await page.goto(URL, { waitUntil: "domcontentloaded", timeout: 60000 });
await page.waitForTimeout(18000);

const filas = () => page.evaluate(() => [...document.querySelectorAll("input[type=text]")]
  .filter(el => { let p=el,t=""; for(let n=0;n<8&&p;n++){p=p.parentElement; if(p){t=p.innerText||""; if(t.trim().length>12)break;}} return t.includes("Agregar más"); })
  .map(e => e.value));

const base = await page.evaluate(() => {
  const els = [...document.querySelectorAll("input[type=text]")];
  for (let i=0;i<els.length;i++){ let p=els[i],t=""; for(let n=0;n<8&&p;n++){p=p.parentElement; if(p){t=p.innerText||""; if(t.trim().length>12)break;}} if(t.includes("Agregar más")) return i; }
  return -1;
});
console.log("primera fila en el indice", base, "| filas ahora:", (await filas()).length);

await page.locator("input[type=text]").nth(base).fill(lista[0]);
for (let i = 1; i < lista.length; i++) {
  await page.locator('text="Agregar más"').first().click();
  await page.waitForTimeout(900);
  await page.locator("input[type=text]").nth(base + i).fill(lista[i]);
}
console.log("escritas:", (await filas()).filter(Boolean).length, "de", lista.length);

const g = page.locator('text="Guardar"').first();
await g.scrollIntoViewIfNeeded();
await g.click();
await page.waitForTimeout(20000);

// Y comprobarlo de verdad: recargar y contar.
await page.goto(URL, { waitUntil: "domcontentloaded", timeout: 60000 });
await page.waitForTimeout(18000);
const quedan = (await filas()).filter(Boolean);
console.log(`GUARDADAS ${quedan.length} de ${lista.length}`);
if (quedan.length) console.log("  primera:", quedan[0].slice(0, 50));
await ctx.close();
