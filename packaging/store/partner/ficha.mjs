import { ctx, page } from "./partner.mjs";
import { readFileSync } from "node:fs";
const FICHA = JSON.parse(readFileSync("C:/proyectos/winshotx/packaging/store/ficha.json", "utf8"));
const idioma = process.env.IDIOMA, langid = process.env.LANGID, T = process.env.TIENDA;
const d = FICHA[idioma];

await page.goto(`https://partner.microsoft.com/es-es/dashboard/products/9P1NKWNRXD6Z/submissions/1152921505701771888/listings?languageid=${langid}&languagecode=${idioma}`, { waitUntil: "domcontentloaded", timeout: 60000 });
await page.waitForTimeout(14000);

const idx = (frag, tag) => page.evaluate(([f, t]) => {
  const els = [...document.querySelectorAll(t)];
  for (let i = 0; i < els.length; i++) {
    let p = els[i], txt = "";
    for (let n = 0; n < 8 && p; n++) { p = p.parentElement; if (p) { txt = p.innerText || ""; if (txt.trim().length > 12) break; } }
    if (txt.replace(/\s+/g, " ").includes(f)) return i;
  }
  return -1;
}, [frag, tag]);
async function campo(frag, tag) {
  const i = await idx(frag, tag);
  if (i < 0) throw new Error(`no encuentro ${tag} con "${frag}"`);
  return page.locator(tag).nth(i);
}

await (await campo("Descripción", "textarea")).fill(d.descripcion);
await (await campo("descripción más breve y pegadiza", "textarea")).fill(d.corta);
await (await campo("copyright y marca registrada", "input[type=text]")).fill(d.copyright);
await (await campo("Desarrollado por", "input[type=text]")).fill(d.desarrollado);
const base = await idx("Agregar más", "input[type=text]");
await page.locator("input[type=text]").nth(base).fill(d.caracteristicas[0]);
for (let i = 1; i < d.caracteristicas.length; i++) {
  await page.getByRole("button", { name: /agregar más|add more/i }).first().click();
  await page.waitForTimeout(800);
  await page.locator("input[type=text]").nth(base + i).fill(d.caracteristicas[i]);
}
console.log("textos y", d.caracteristicas.length, "caracteristicas");

const capturas = page.locator("input[type=file]").nth(0);
for (const f of ["1-overlay", "4-editor", "2-grabar", "3-app", "5-captura"]) {
  await capturas.setInputFiles(`${T}/${f}.png`);
  for (let i = 0; i < 25; i++) {
    await page.waitForTimeout(3000);
    if ((await page.locator("body").innerText()).includes(`${f}.png`)) break;
  }
  console.log("  subida", f);
}
const iIcono = await idx("Icono de la aplicación 1:1 300 x 300", "input[type=file]");
if (iIcono >= 0) {
  await page.locator("input[type=file]").nth(iIcono).setInputFiles(`${T}/icono300.png`);
  await page.waitForTimeout(12000);
  console.log("  icono 300 subido");
}

const guardar = page.getByRole("button", { name: /^(guardar|save)$/i }).first();
if (await guardar.count()) { await guardar.click(); await page.waitForTimeout(18000); console.log("GUARDADO"); }
else console.log("SIN boton guardar");
await page.screenshot({ path: `${process.env.PERFIL}/../ficha-${idioma}.png`, fullPage: true });
await ctx.close();
