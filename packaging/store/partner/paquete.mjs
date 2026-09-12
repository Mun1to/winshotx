// Sube el MSIX nuevo al envio y quita el viejo.
//
//   PERFIL=<perfil> ENVIO=<id> SOLO_MIRAR=1 node packaging/store/partner/paquete.mjs
//   PERFIL=<perfil> ENVIO=<id> MSIX=<ruta> node packaging/store/partner/paquete.mjs
//
// Las dos trampas de esta pantalla, que costaron una tarde:
//
//  - **El boton «Save» esta al fondo del todo**, por debajo de lo que se ve, y Playwright lo
//    da por invisible: al enumerar `button:visible` no sale. Hay que bajar hasta el final y
//    buscarlo por texto en cualquier elemento, no solo en `button`. Y no es el mismo boton
//    que «Guardar borrador» de las otras pantallas.
//  - **Quitar un paquete no lo quita: lo marca.** Se queda tachado con un aviso de que hay
//    que guardar para confirmar, y hasta que no se pulsa Save el paquete viejo sigue en el
//    envio. Un envio con dos paquetes del mismo arquitectura no pasa: se queda el mayor,
//    pero es mejor no dejarlo a la suerte.

import { chromium } from "playwright";
import { existsSync } from "node:fs";

const PERFIL = process.env.PERFIL;
const ENVIO = process.env.ENVIO;
const SOLO_MIRAR = process.env.SOLO_MIRAR === "1";
// Para reintentar solo el guardado cuando el paquete ya esta subido, sin subirlo dos veces.
const SOLO_GUARDAR = process.env.SOLO_GUARDAR === "1";
const MSIX = process.env.MSIX;
if (!PERFIL || !ENVIO) {
  console.error("Faltan PERFIL=<carpeta con la sesion> y ENVIO=<id del envio>.");
  process.exit(1);
}
if (!SOLO_MIRAR && !SOLO_GUARDAR && (!MSIX || !existsSync(MSIX))) {
  console.error(`Falta MSIX=<ruta al paquete>, o no existe: ${MSIX}`);
  process.exit(1);
}
const ID = process.env.STORE_ID ?? "9P1NKWNRXD6Z";
const URL = `https://partner.microsoft.com/es-es/dashboard/products/${ID}/submissions/${ENVIO}/packages`;
const FOTOS = `${PERFIL}/..`;

const ctx = await chromium.launchPersistentContext(PERFIL, {
  channel: "chrome",
  headless: false,
  viewport: { width: 1500, height: 1100 },
  args: ["--disable-blink-features=AutomationControlled"],
});
const page = ctx.pages()[0] ?? (await ctx.newPage());

async function cuerpo() {
  return (await page.locator("body").innerText().catch(() => "")).replace(/\n{2,}/g, "\n");
}
/** Los .msix que la pantalla dice tener, en el orden en que salen. */
function paquetesDe(texto) {
  return [...texto.matchAll(/winshotx_[\d.]+_x64\.msix/g)].map((m) => m[0]);
}

await page.goto(URL, { waitUntil: "domcontentloaded", timeout: 90000 });
await page.waitForTimeout(20000);

let texto = await cuerpo();
console.log("PAQUETES ANTES:", paquetesDe(texto));
await page.screenshot({ path: `${FOTOS}/paquetes-antes.png`, fullPage: true }).catch(() => {});

if (SOLO_MIRAR) {
  const i = texto.search(/Paquetes|Packages/);
  console.log(texto.slice(i >= 0 ? i : 0, (i >= 0 ? i : 0) + 1200));
  const inputs = await page.locator('input[type="file"]').count();
  console.log(`inputs de archivo: ${inputs}`);
  await ctx.close();
  process.exit(0);
}

if (!SOLO_GUARDAR) {
  // --- Subir el nuevo --------------------------------------------------------------
  const file = page.locator('input[type="file"]').first();
  if ((await file.count()) === 0) {
    console.log("No hay input de archivo en esta pantalla.");
    await ctx.close();
    process.exit(1);
  }
  await file.setInputFiles(MSIX);
  console.log("Subiendo", MSIX);

  // La validacion del paquete tarda: se espera a que su nombre aparezca en la pantalla.
  const nombre = MSIX.split(/[\\/]/).pop();
  let subido = false;
  for (let i = 0; i < 40; i++) {
    await page.waitForTimeout(6000);
    texto = await cuerpo();
    if (texto.includes(nombre)) {
      subido = true;
      break;
    }
    process.stdout.write(".");
  }
  console.log(`\n${subido ? "Subido" : "NO aparece"}: ${nombre}`);
  console.log("PAQUETES AHORA:", paquetesDe(await cuerpo()));

  // --- Quitar los viejos -----------------------------------------------------------
  for (const viejo of paquetesDe(await cuerpo()).filter((p) => p !== nombre)) {
    console.log("Quitando", viejo);
    const fila = page.locator(`tr:has-text("${viejo}"), div:has-text("${viejo}")`).last();
    const quitar = fila.locator('button:has-text("Quitar"), button:has-text("Remove"), [aria-label*="liminar"], [aria-label*="emove"]').first();
    if ((await quitar.count()) === 0) {
      // Partner Center tacha el paquete viejo por su cuenta en cuanto hay uno mayor que
    // sirve a los mismos clientes, y lo quita al guardar. Entonces no hay boton, y no falta.
    console.log("  sin boton: la pantalla ya lo da por quitado al guardar");
      continue;
    }
    await quitar.click().catch((e) => console.log("  fallo:", e.message.slice(0, 80)));
    await page.waitForTimeout(4000);
  }
}

// --- Guardar ---------------------------------------------------------------------
// El boton de verdad es un <button> al fondo del todo. Buscarlo por texto a secas tambien
// encuentra el «Save.» suelto del aviso de arriba, que no se puede pulsar: sale un timeout
// y la version anterior de esto lo cantaba igualmente como «GUARDADO». Se coge por rol, y
// si el click normal no entra se pulsa desde el DOM.
await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
await page.waitForTimeout(3000);
const guardar = page.getByRole("button", { name: /^\s*(Save|Guardar)\s*$/ }).last();
if ((await guardar.count()) === 0) {
  console.log("NO hay boton de guardar: el envio se queda como estaba.");
} else {
  await guardar.scrollIntoViewIfNeeded().catch(() => {});
  const pulsado = await guardar
    .click({ timeout: 25000 })
    .then(() => true)
    .catch(async (e) => {
      console.log("  el click normal fallo:", e.message.slice(0, 80));
      return guardar
        .evaluate((el) => el.click())
        .then(() => true)
        .catch(() => false);
    });
  console.log(pulsado ? "Guardar pulsado" : "NO se pudo pulsar Guardar");
  await page.waitForTimeout(20000);
}

// Lo unico que dice si se guardo es volver a leer la pantalla: al guardar desaparece el
// paquete viejo, que Partner Center deja tachado con su aviso hasta que se confirma.
await page.reload({ waitUntil: "domcontentloaded" }).catch(() => {});
await page.waitForTimeout(20000);
await page.screenshot({ path: `${FOTOS}/paquetes-despues.png`, fullPage: true }).catch(() => {});
const final = await cuerpo();
console.log("PAQUETES DESPUES:", paquetesDe(final));
console.log(
  /will be removed after you save|se quitara despues de guardar/i.test(final)
    ? "SIN GUARDAR: el paquete viejo sigue esperando confirmacion."
    : "GUARDADO: en el envio solo queda el paquete nuevo.",
);
await ctx.close();
