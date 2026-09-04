// Comprueba el envio entero y, si esta todo, lo reenvia a certificacion.
//
// Se mira ANTES de pulsar: reenviar arranca una certificacion que tarda dias, y mandarla con
// algo a medias es perder esa ronda. Con `SOLO_MIRAR=1` solo informa y no pulsa nada.

import { chromium } from "playwright";

const PERFIL = process.env.PERFIL;
const ID = "9P1NKWNRXD6Z";
const SOLO_MIRAR = process.env.SOLO_MIRAR === "1";

const ctx = await chromium.launchPersistentContext(PERFIL, {
  channel: "chrome",
  headless: false,
  viewport: { width: 1500, height: 1100 },
  args: ["--disable-blink-features=AutomationControlled"],
});
const page = ctx.pages()[0] ?? (await ctx.newPage());

await page.goto(`https://partner.microsoft.com/es-es/dashboard/products/${ID}/overview`, {
  waitUntil: "domcontentloaded",
  timeout: 90000,
});
await page.waitForTimeout(20000);

const texto = (await page.locator("body").innerText().catch(() => "")).replace(/\n{2,}/g, "\n");
const i0 = texto.indexOf("Lanzamiento del producto");
console.log("===== ESTADO DEL ENVIO =====");
console.log(texto.slice(i0 >= 0 ? i0 : 0, (i0 >= 0 ? i0 : 0) + 1800));

const incompleto = /Incompleto|Incomplete/.test(texto);
const paqueteNuevo = texto.includes("0.2.21.0");
const paqueteViejo = texto.includes("0.2.11.0");
console.log("===== COMPROBACIONES =====");
console.log("  ¿alguna sección incompleta?", incompleto);
console.log("  ¿el paquete es el 0.2.21?", paqueteNuevo);
console.log("  ¿queda rastro del 0.2.11?", paqueteViejo);

await page.screenshot({ path: `${PERFIL}/../antes-de-reenviar.png`, fullPage: true }).catch(() => {});

if (SOLO_MIRAR) {
  console.log("SOLO MIRAR: no se pulsa nada");
  await ctx.close();
  process.exit(0);
}

if (incompleto) {
  console.log("HAY ALGO INCOMPLETO: no se reenvia.");
  await ctx.close();
  process.exit(1);
}

const boton = page.locator('text=/Volver a enviar para la certificación|Resubmit to the Store|Enviar a la Store|Submit to the Store/');
console.log("botones de reenvio encontrados:", await boton.count());
if ((await boton.count()) === 0) {
  console.log("NO hay botón de reenviar.");
  await ctx.close();
  process.exit(1);
}
await boton.first().scrollIntoViewIfNeeded().catch(() => {});
await boton.first().click({ timeout: 30000 });
await page.waitForTimeout(25000);
console.log("PULSADO. Estado ahora:");
const despues = (await page.locator("body").innerText().catch(() => "")).replace(/\n{2,}/g, "\n");
const i1 = despues.indexOf("Lanzamiento del producto");
console.log(despues.slice(i1 >= 0 ? i1 : 0, (i1 >= 0 ? i1 : 0) + 1200));
await page.screenshot({ path: `${PERFIL}/../tras-reenviar.png`, fullPage: true }).catch(() => {});

await ctx.close();
console.log("FIN");
