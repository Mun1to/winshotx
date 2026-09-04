// Lee el estado del envio de winshotx en Partner Center, con la sesion ya iniciada.
//
// Prueba varias rutas porque el panel cambia de sitio segun donde este el producto: la
// pagina de envios no ensenna nada si no hay envio en marcha, y la de informacion general
// es la que siempre dice en que estado esta.

import { chromium } from "playwright";

const PERFIL = process.env.PERFIL;
const ID = "9P1NKWNRXD6Z";
const BASE = `https://partner.microsoft.com/es-es/dashboard/products/${ID}`;

const RUTAS = [
  ["general", `${BASE}/overview`],
  ["envios", `${BASE}/submissions`],
  ["paquetes", `${BASE}/submissions/1152921505701771888/packages`],
];

const ctx = await chromium.launchPersistentContext(PERFIL, {
  channel: "chrome",
  headless: false,
  viewport: { width: 1500, height: 1000 },
  args: ["--disable-blink-features=AutomationControlled"],
});
const page = ctx.pages()[0] ?? (await ctx.newPage());

for (const [nombre, url] of RUTAS) {
  try {
    await page.goto(url, { waitUntil: "domcontentloaded", timeout: 60000 });
    // El panel se pinta despues del HTML; sin esta espera se lee el menu lateral y ya.
    await page.waitForTimeout(12000);
    const texto = (await page.locator("body").innerText().catch(() => "")).replace(/\n{2,}/g, "\n");
    console.log(`===== ${nombre.toUpperCase()} =====`);
    console.log(page.url());
    console.log(texto.slice(0, 2500));
    await page.screenshot({ path: `${PERFIL}/../store-${nombre}.png`, fullPage: true }).catch(() => {});
    console.log(`FOTO store-${nombre}.png`);
  } catch (e) {
    console.log(`===== ${nombre.toUpperCase()} ===== FALLO: ${e.message.slice(0, 200)}`);
  }
}

await ctx.close();
console.log("FIN");
