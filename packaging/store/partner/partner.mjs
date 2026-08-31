// El envio de winshotx a la Store, paso a paso. Cada paso deja foto y texto: si la pagina
// no es la que se esperaba, se ve en vez de seguir a ciegas.
import { chromium } from "playwright";
const PERFIL = process.env.PERFIL;
const FOTOS = `${PERFIL}/..`;
const ID = "9P1NKWNRXD6Z";
const ENVIO = "1152921505701771888";
export const RUTA = `products/${ID}/submissions/${ENVIO}`;

export const ctx = await chromium.launchPersistentContext(PERFIL, {
  channel: "chrome", headless: false, viewport: null,
  args: ["--disable-blink-features=AutomationControlled", "--window-size=1500,1000"],
});
export const page = ctx.pages()[0] ?? (await ctx.newPage());

export async function ir(ruta, espera = 9000) {
  await page.goto(`https://partner.microsoft.com/es-es/dashboard/products/${ID}/${ruta}`, { waitUntil: "domcontentloaded", timeout: 60000 });
  await page.waitForTimeout(espera);
}
export async function mirar(nombre, corte = 1500) {
  await page.screenshot({ path: `${FOTOS}/${nombre}.png`, fullPage: true });
  const t = (await page.locator("body").innerText().catch(() => "")).replace(/\n{2,}/g, "\n");
  console.log(`--- ${nombre} --- ${page.url()}`);
  console.log(t.slice(0, corte));
}
