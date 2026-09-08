// Cancela la certificacion en marcha y devuelve el envio a borrador.
//
//   PERFIL=<perfil> node packaging/store/partner/cancelar.mjs            solo mira
//   PERFIL=<perfil> PULSAR=1 node packaging/store/partner/cancelar.mjs   cancela
//
// Sirve para cuando se ha mandado algo con una errata: **es barato mientras la certificacion
// acaba de empezar**. El envio vuelve a «En borrador», los campos se pueden volver a tocar y
// se reenvia con `reenviar.mjs`. Lo que ya esta publicado en la Store no se toca: los
// usuarios siguen con la version de siempre mientras tanto.
//
// El boton se llama «Cancelar el certificado» (una traduccion mala de «Cancel certification»)
// y pide confirmacion en un dialogo con dos botones, «Si» y «No». Ese dialogo **no lleva
// `role="dialog"`**: buscarlo por el rol se lleva otro trozo de la pagina (salio uno con
// «Inicio de sesion» dentro). Lo que si es fiable es su texto, y que el boton de confirmar
// se llama «Si» a secas, que no aparece en ningun otro sitio del panel.

import { chromium } from "playwright";

const PERFIL = process.env.PERFIL;
if (!PERFIL) {
  console.error("Falta PERFIL=<carpeta con la sesion>.");
  process.exit(1);
}
const ID = process.env.STORE_ID ?? "9P1NKWNRXD6Z";
const PULSAR = process.env.PULSAR === "1";
const FOTOS = `${PERFIL}/..`;

const ctx = await chromium.launchPersistentContext(PERFIL, {
  channel: "chrome",
  headless: false,
  viewport: { width: 1500, height: 1100 },
  args: ["--disable-blink-features=AutomationControlled"],
});
const page = ctx.pages()[0] ?? (await ctx.newPage());

const cuerpo = async () => (await page.locator("body").innerText().catch(() => "")).replace(/\n{2,}/g, "\n");

await page.goto(`https://partner.microsoft.com/es-es/dashboard/products/${ID}/overview`, {
  waitUntil: "domcontentloaded",
  timeout: 90000,
});
await page.waitForTimeout(18000);

let texto = await cuerpo();
const enCertificacion = /en certificación|in certification/i.test(texto);
console.log("¿hay una certificación en marcha?", enCertificacion);

const boton = page.locator('text=/Cancelar el certificado|Cancel certification/').first();
console.log("botón de cancelar encontrado:", (await boton.count()) > 0);

if (!PULSAR || !enCertificacion || (await boton.count()) === 0) {
  if (!PULSAR) console.log("Modo mirar: no se pulsa nada. Repite con PULSAR=1.");
  await ctx.close();
  process.exit(enCertificacion ? 0 : 1);
}

await boton.click();
await page.waitForTimeout(5000);
await page.screenshot({ path: `${FOTOS}/cancelar-dialogo.png`, fullPage: true }).catch(() => {});

// El dialogo de confirmacion: «¿Quiere cancelar el certificado?» con «Sí» y «No».
const preguntado = /cancelar el certificado\?|cancel certification\?/i.test(await cuerpo());
console.log("¿ha salido el diálogo de confirmación?", preguntado);

// Los botones de Partner Center son componentes propios (`he-button`), asi que un
// `locator('button')` no los ve. Por el arbol de accesibilidad si salen, y si eso tampoco
// funciona queda buscar por texto exacto en cualquier etiqueta.
let confirmar = page.getByRole("button", { name: /^(Sí|Si|Yes)$/ }).first();
if ((await confirmar.count()) === 0) confirmar = page.getByText(/^(Sí|Yes)$/).first();
if ((await confirmar.count()) === 0) {
  const candidatos = await page.evaluate(() =>
    [...document.querySelectorAll("*")]
      .filter((e) => (e.innerText || "").trim().length && (e.innerText || "").trim().length < 12)
      .map((e) => `${e.tagName.toLowerCase()}:${(e.innerText || "").trim()}`)
      .slice(-25));
  console.log("candidatos al final de la página:", JSON.stringify(candidatos));
}
if (!preguntado || (await confirmar.count()) === 0) {
  console.log("No encuentro el botón de confirmar; la foto está en cancelar-dialogo.png");
  await ctx.close();
  process.exit(1);
}
await confirmar.click();
await page.waitForTimeout(20000);

texto = await cuerpo();
const i = texto.indexOf("Lanzamiento del producto");
console.log("=== ESTADO AHORA ===");
console.log(texto.slice(i >= 0 ? i : 0, (i >= 0 ? i : 0) + 900));
await page.screenshot({ path: `${FOTOS}/tras-cancelar.png`, fullPage: true }).catch(() => {});

await ctx.close();
