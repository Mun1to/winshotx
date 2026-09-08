// Deja la sesion de Partner Center guardada en un perfil, para que los demas guiones de
// esta carpeta puedan trabajar despues sin volver a pedir nada.
//
//   PERFIL=<carpeta del perfil> node packaging/store/partner/abrir-sesion.mjs
//
// Abre Chrome, espera a que Munir entre con su cuenta (hasta diez minutos) y en cuanto
// detecta el panel del producto guarda el perfil y se va. El perfil es persistente, asi
// que la sesion sobrevive al cierre de la ventana: esto se corre UNA vez y despues valen
// `estado.mjs`, `descripcion.mjs`, `ficha.mjs` y `reenviar.mjs`.
//
// Por que existe: el inicio de sesion de Microsoft pide correo, contrasenna y el segundo
// factor. Nada de eso lo puede teclear un agente, ni debe. Lo unico automatizable es
// esperar bien: mirar la URL Y el contenido, porque `dashboard/products/...` aparece en la
// barra mucho antes de que la pagina sea la del producto, y con solo la URL se da por
// buena una pantalla de carga.

import { chromium } from "playwright";
import { mkdirSync } from "node:fs";

const PERFIL = process.env.PERFIL;
if (!PERFIL) {
  console.error("Falta PERFIL=<carpeta donde guardar la sesion>.");
  process.exit(1);
}
const ID = process.env.STORE_ID ?? "9P1NKWNRXD6Z";
const MINUTOS = Number(process.env.MINUTOS ?? 10);

mkdirSync(PERFIL, { recursive: true });

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

console.log(`Ventana abierta. Esperando el inicio de sesion, hasta ${MINUTOS} minutos.`);

const limite = Date.now() + MINUTOS * 60 * 1000;
let dentro = false;
while (Date.now() < limite) {
  await page.waitForTimeout(4000);
  const url = page.url();
  const texto = await page.locator("body").innerText().catch(() => "");
  const enPanel = url.includes("/dashboard/products/") && !url.includes("login.microsoftonline");
  // El nombre del producto solo sale cuando la pagina ya es la suya, no mientras carga.
  if (enPanel && /winshotx/i.test(texto)) {
    dentro = true;
    break;
  }
  process.stdout.write(".");
}

if (dentro) {
  console.log(`\nSesion guardada en ${PERFIL}. Ya puedes correr los demas guiones con ese PERFIL.`);
} else {
  console.log(`\nNo se llego al panel en ${MINUTOS} minutos. Vuelve a correrlo cuando puedas entrar.`);
}

await ctx.close();
process.exit(dentro ? 0 : 1);
