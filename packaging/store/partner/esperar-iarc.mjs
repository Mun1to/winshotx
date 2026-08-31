// Deja la pagina de la clasificacion por edades abierta y espera a que Munir marque la
// casilla de los terminos de IARC, que es una declaracion suya y no una tarea. En cuanto
// la marca, este guion hace el resto: guarda y manda el envio a certificacion.
import { ctx, page, ir } from "./partner.mjs";

await ir("submissions/1152921505701771888/ageratings", 15000);
await page.keyboard.press("End");
await page.waitForTimeout(2000);
console.log("Ventana abierta en la clasificacion por edades. Esperando la casilla.");

const casilla = page.locator('input[type="checkbox"]').last();
const hasta = Date.now() + 55 * 60 * 1000;
let marcada = false;
while (Date.now() < hasta) {
  await page.waitForTimeout(4000);
  if (await casilla.isChecked().catch(() => false)) { marcada = true; break; }
}
if (!marcada) { console.log("No se ha marcado en 55 minutos."); await new Promise(r=>setTimeout(r, 10*60*1000)); await ctx.close(); process.exit(0); }

console.log("MARCADA. Guardando la clasificacion.");
const g = page.locator('text="Guardar"').first();
await g.scrollIntoViewIfNeeded();
await g.click();
await page.waitForTimeout(18000);

await ir("overview", 15000);
const enviar = page.getByRole("button", { name: /enviar para certificación|submit to the store/i }).first();
console.log("boton enviar habilitado:", await enviar.isEnabled().catch(() => false));
if (await enviar.isEnabled().catch(() => false)) {
  await enviar.click();
  await page.waitForTimeout(20000);
  console.log("ENVIADO. URL:", page.url());
  console.log((await page.locator("body").innerText()).slice(0, 900).replace(/\n{2,}/g, "\n"));
  await page.screenshot({ path: `${process.env.PERFIL}/../enviado.png`, fullPage: true });
} else {
  const t = await page.locator("body").innerText();
  const i = t.indexOf("Precios y disponibilidad");
  console.log("SIGUE APAGADO:\n" + t.slice(i, i + 330).replace(/\n{2,}/g, "\n"));
}
await new Promise(r => setTimeout(r, 20 * 60 * 1000));
await ctx.close();
