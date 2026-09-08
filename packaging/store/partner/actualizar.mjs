// Abre el envio de actualizacion de un producto que YA esta publicado en la Store.
//
//   PERFIL=<perfil> node packaging/store/partner/actualizar.mjs            solo mira
//   PERFIL=<perfil> PULSAR=1 node packaging/store/partner/actualizar.mjs   lo crea
//
// Cuando la app esta publicada, su envio queda de SOLO LECTURA («Presencia en Store») y
// no hay nada que editar: los campos que se ven no se pueden tocar y es facil creerse que
// Partner Center esta roto. Para cambiar cualquier cosa, hasta una coma de la descripcion,
// hay que crear un envio nuevo desde «Lanzamiento del producto → Iniciar actualizacion».
//
// Ese envio nuevo tiene un ID distinto del anterior, y es el que necesitan los demas
// guiones de esta carpeta: este imprime el ID que salga, para no adivinarlo.
//
// Por defecto NO pulsa nada. Crear el envio no publica nada ni toca lo que ven los
// usuarios, pero deja el producto con un borrador abierto, y eso se hace a proposito.

import { chromium } from "playwright";

const PERFIL = process.env.PERFIL;
if (!PERFIL) {
  console.error("Falta PERFIL=<carpeta con la sesion>. Se crea con abrir-sesion.mjs.");
  process.exit(1);
}
const ID = process.env.STORE_ID ?? "9P1NKWNRXD6Z";
const PULSAR = process.env.PULSAR === "1";
const FOTOS = `${PERFIL}/..`;

const ctx = await chromium.launchPersistentContext(PERFIL, {
  channel: "chrome",
  headless: false,
  viewport: null,
  args: ["--disable-blink-features=AutomationControlled", "--window-size=1500,1000"],
});
const page = ctx.pages()[0] ?? (await ctx.newPage());

await page.goto(`https://partner.microsoft.com/es-es/dashboard/products/${ID}/overview`, {
  waitUntil: "domcontentloaded",
  timeout: 60000,
});
await page.waitForTimeout(9000);

/** Los envios que la pagina enlaza, que es de donde sale el ID nuevo. */
async function envios() {
  return page.locator('a[href*="/submissions/"]').evaluateAll((as) =>
    [...new Set(as.map((a) => a.getAttribute("href")))]
      .map((h) => h.match(/\/submissions\/(\d+)/)?.[1])
      .filter(Boolean));
}

console.log("Envios enlazados antes:", await envios());

// El boton se llama «Iniciar actualizacion» en espannol y «Start update» en ingles, y no
// siempre es un <button>: en esta pantalla es un enlace con pinta de boton.
const boton = page.locator('text="Iniciar actualización"').or(page.locator('text="Start update"')).first();
const hay = await boton.count();
console.log("Boton de actualizar encontrado:", hay > 0);

if (!hay) {
  await page.screenshot({ path: `${FOTOS}/store-sin-boton.png`, fullPage: true });
  console.log("No esta el boton. Foto en store-sin-boton.png");
  await ctx.close();
  process.exit(1);
}

if (!PULSAR) {
  console.log("Modo mirar: no se pulsa nada. Repite con PULSAR=1 para crear el envio.");
  await ctx.close();
  process.exit(0);
}

await boton.click();
await page.waitForTimeout(12000);
await page.screenshot({ path: `${FOTOS}/store-actualizacion.png`, fullPage: true });
console.log("URL despues de pulsar:", page.url());

const nuevo = page.url().match(/\/submissions\/(\d+)/)?.[1] ?? (await envios())[0];
console.log("ID DEL ENVIO NUEVO:", nuevo ?? "no encontrado");
const texto = (await page.locator("body").innerText().catch(() => "")).replace(/\n{2,}/g, "\n");
console.log(texto.slice(0, 1800));

await ctx.close();
