import { ctx, page } from "./partner.mjs";
const idioma = process.env.IDIOMA, langid = process.env.LANGID, T = process.env.TIENDA;
await page.goto(`https://partner.microsoft.com/es-es/dashboard/products/9P1NKWNRXD6Z/submissions/1152921505701771888/listings?languageid=${langid}&languagecode=${idioma}`, { waitUntil: "domcontentloaded", timeout: 60000 });
await page.waitForTimeout(15000);

const cuantas = async () => Number(((await page.locator("body").innerText()).match(/Escritorio\s*\((\d+)\)/) ?? [])[1] ?? -1);
console.log("capturas al empezar:", await cuantas());

// Fuera las que hubiera: se vuelven a subir todas en orden, porque la primera es la que
// se ve en la ficha y la que quedo suelta era justo la menos representativa.
for (let i = 0; i < 12; i++) {
  const n = await cuantas();
  if (n <= 0) break;
  const papelera = page.locator('[title*="liminar"], button[aria-label*="liminar"]').first();
  if (!(await papelera.count())) break;
  await papelera.click();
  await page.waitForTimeout(3500);
}
console.log("tras vaciar:", await cuantas());

// Cada subida crea un hueco nuevo al final, asi que el input que toca es el numero de
// capturas que ya hay. Reusar siempre el primero solo reemplazaba la de antes.
const orden = ["1-overlay", "4-editor", "2-grabar", "3-app", "5-captura"];
for (let k = 0; k < orden.length; k++) {
  const antes = await cuantas();
  await page.locator("input[type=file]").nth(antes).setInputFiles(`${T}/${orden[k]}.png`);
  for (let i = 0; i < 30; i++) {
    await page.waitForTimeout(3000);
    if ((await cuantas()) > antes) break;
  }
  console.log(`  ${orden[k]} -> ${await cuantas()}`);
}

const guardar = page.getByRole("button", { name: /^(guardar|save)$/i }).first();
if (await guardar.count()) { await guardar.click(); await page.waitForTimeout(18000); console.log("GUARDADO"); }
else console.log("SIN boton guardar");
await ctx.close();
