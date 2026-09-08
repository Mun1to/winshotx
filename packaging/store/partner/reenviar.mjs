// Comprueba el envio entero y, si esta todo, lo manda a certificacion.
//
//   PERFIL=<perfil> SOLO_MIRAR=1 node packaging/store/partner/reenviar.mjs
//   PERFIL=<perfil> node packaging/store/partner/reenviar.mjs
//
// Se mira ANTES de pulsar: la certificacion tarda dias, y mandarla con algo a medias es
// perder esa ronda entera. Con `SOLO_MIRAR=1` solo informa y no pulsa nada.
//
// La version que se espera sale de `package.json`, no escrita aqui dentro: con las
// versiones a mano, el guion seguia comprobando que el paquete fuese el de hace dos
// lanzamientos y daba por bueno un envio con el paquete equivocado.
//
// El boton no siempre se llama igual: en un envio nuevo es «Enviar para certificacion» y
// despues de un rechazo es «Volver a enviar para la certificacion».

import { chromium } from "playwright";
import { readFileSync } from "node:fs";

const PERFIL = process.env.PERFIL;
if (!PERFIL) {
  console.error("Falta PERFIL=<carpeta con la sesion>.");
  process.exit(1);
}
const ID = process.env.STORE_ID ?? "9P1NKWNRXD6Z";
const SOLO_MIRAR = process.env.SOLO_MIRAR === "1";
const VERSION = process.env.VERSION ??
  JSON.parse(readFileSync("C:/proyectos/winshotx/package.json", "utf8")).version;

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
const versiones = [...new Set([...texto.matchAll(/winshotx_([\d.]+)_x64\.msix/g)].map((m) => m[1]))];
const esperada = `${VERSION}.0`;
const soloLaBuena = versiones.length === 1 && versiones[0] === esperada;

console.log("===== COMPROBACIONES =====");
console.log("  ¿alguna sección incompleta?", incompleto);
console.log("  paquetes en el envío:", versiones.length ? versiones.join(", ") : "ninguno a la vista");
console.log(`  ¿solo el ${esperada}?`, soloLaBuena);

await page.screenshot({ path: `${PERFIL}/../antes-de-reenviar.png`, fullPage: true }).catch(() => {});

if (SOLO_MIRAR) {
  console.log("SOLO MIRAR: no se pulsa nada");
  await ctx.close();
  process.exit(0);
}

if (incompleto) {
  console.log("HAY ALGO INCOMPLETO: no se envía.");
  await ctx.close();
  process.exit(1);
}
if (versiones.length && !soloLaBuena) {
  console.log(`EL PAQUETE NO ES EL QUE TOCA (se esperaba ${esperada}): no se envía.`);
  await ctx.close();
  process.exit(1);
}

const boton = page.locator(
  'text=/Enviar para certificación|Volver a enviar para la certificación|Resubmit to the Store|Submit to the Store/');
console.log("botones de envío encontrados:", await boton.count());
if ((await boton.count()) === 0) {
  console.log("NO hay botón de enviar.");
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
