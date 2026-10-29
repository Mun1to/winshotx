// Las palabras clave de la ficha, que son con lo que la Store te encuentra buscando.
//
//   PERFIL=<perfil> ENVIO=<id> node packaging/store/partner/claves.mjs
//   PERFIL=<perfil> ENVIO=<id> SOLO=es-es node ...     un idioma solo
//
// Van aparte porque el control no es un campo de texto: es un `he-select` multiple con
// `freeform`, dentro de `#search-terms`. No sale al listar los `input[type=text]` de la
// pagina, y por eso el primer envio se fue sin ellas.
//
// Deja EXACTAMENTE las de `ficha.json`: quita las que sobran, anade las que faltan y solo
// guarda si el conjunto cuadra. La version anterior no conseguia quitar ninguna, porque el
// boton de quitar vive dentro del shadow root de cada `he-option` y lo que pulsaba era otra
// cosa: el 29 de octubre de 2026 las fichas tenian 11 claves, las 7 viejas (con las dos que
// Microsoft rechazo por nombrar productos ajenos) mas las nuevas, algunas pegadas sin espacio.
import { ctx, page } from "./partner.mjs";
import { readFileSync } from "node:fs";
const FICHA = JSON.parse(readFileSync("C:/proyectos/winshotx/packaging/store/ficha.json", "utf8"));
// El envio cambia en cada actualizacion: con el del primer envio escrito aqui, las claves se
// escribian en un envio de solo lectura.
const ENVIO = process.env.ENVIO;
if (!ENVIO) {
  console.error("Falta ENVIO=<id del envio>. Lo imprime actualizar.mjs.");
  process.exit(1);
}
const BASE = `https://partner.microsoft.com/es-es/dashboard/products/9P1NKWNRXD6Z/submissions/${ENVIO}`;
// Los numeros de idioma de winshotx en Partner Center (ver el LEEME, trampa 6).
const IDIOMAS = [["en-us", 4], ["es-es", 15]].filter(([c]) => !process.env.SOLO || c === process.env.SOLO);

// Las etiquetas puestas, por su atributo: el texto del campo mezcla las sugerencias de la IA.
const leer = () =>
  page.locator('#search-terms he-option[slot^="selected-"]').evaluateAll((os) => os.map((o) => o.getAttribute("value") ?? ""));

async function cargar(url) {
  await page.goto(url, { waitUntil: "domcontentloaded", timeout: 60000 });
  for (let i = 0; i < 60; i++) {
    await page.waitForTimeout(4000);
    const n = await page
      .evaluate(() => [...document.querySelectorAll("textarea")].filter((t) => t.value.length > 50).length)
      .catch(() => 0);
    if (n) break;
  }
  // Las etiquetas llegan despues que los textos: se espera a que su numero deje de cambiar.
  let antes = -1;
  for (let i = 0; i < 20; i++) {
    await page.waitForTimeout(3000);
    const n = (await leer()).length;
    if (n === antes && n > 0) break;
    antes = n;
  }
}

async function quitar(valor) {
  const op = valor
    ? page.locator(`#search-terms he-option[slot^="selected-"][value="${valor.replace(/"/g, '\\"')}"]`).last()
    : page.locator('#search-terms he-option[slot^="selected-"]:not([value]), #search-terms he-option[slot^="selected-"][value=""]').first();
  await op.scrollIntoViewIfNeeded().catch(() => {});
  await op.locator("he-button").first().click({ timeout: 8000 }).catch(() => {});
  await page.waitForTimeout(900);
}

let todoBien = true;
for (const [idioma, langid] of IDIOMAS) {
  const quiero = FICHA[idioma].palabrasClave ?? [];
  const url = `${BASE}/listings?languageid=${langid}&languagecode=${idioma}`;
  await cargar(url);
  console.log(`\n=== ${idioma}\n  habia: ${(await leer()).join(" | ") || "(ninguna)"}`);

  // Fuera lo que sobre, repetidas y vacias incluidas: el limite son 7 y el control no deja
  // guardar si se pasa, pero la ficha queda en «Incompleto» sin decir por que.
  for (let vuelta = 0; vuelta < 25; vuelta++) {
    const hay = await leer();
    const i = hay.findIndex((v, j) => !quiero.includes(v) || hay.indexOf(v) !== j);
    if (i < 0) break;
    await quitar(hay[i]);
    const ahora = (await leer()).length;
    console.log(`  quitar «${hay[i]}» -> ${ahora < hay.length ? "fuera" : "SIGUE"}`);
    if (ahora >= hay.length) break;
  }

  // Se teclea, no se rellena de golpe: con `fill` el texto acaba mezclado con las palabras
  // que el propio control recomienda al abrirse, y sale una sola clave pegada sin sentido.
  //
  // Y despues del Enter va un **Tab**, que es el hallazgo. El Enter pinta la etiqueta y
  // parece hecho, pero sin sacar el foco del campo la palabra no se confirma: se guardaba
  // sin error y al recargar no habia ninguna.
  //
  // Algunas no entran nunca y dejan una etiqueta vacia («grabador de mp4», tres intentos):
  // se quita la vacia y, si no entra, el conjunto no cuadra y no se guarda nada.
  const caja = page.locator("#search-terms").getByRole("combobox").first();
  for (const palabra of quiero) {
    for (let intento = 1; intento <= 3 && !(await leer()).includes(palabra); intento++) {
      while ((await leer()).includes("")) await quitar("");
      await caja.click();
      await page.waitForTimeout(1500);
      await page.keyboard.press("Escape");
      await page.waitForTimeout(500);
      await caja.click();
      await page.waitForTimeout(700);
      await caja.pressSequentially(palabra, { delay: 60 });
      await page.waitForTimeout(900);
      await page.keyboard.press("Enter");
      await page.waitForTimeout(900);
      await page.keyboard.press("Tab");
      await page.waitForTimeout(1200);
      console.log(`  anadir «${palabra}», intento ${intento} -> ${(await leer()).includes(palabra) ? "dentro" : "NO ENTRO"}`);
    }
  }
  while ((await leer()).includes("")) await quitar("");

  const final = await leer();
  const cuadra = final.length === quiero.length && quiero.every((p) => final.includes(p));
  console.log(`  antes de guardar: ${final.join(" | ")} -> ${cuadra ? "CUADRA" : "NO CUADRA, no guardo"}`);
  if (!cuadra) {
    todoBien = false;
    continue;
  }
  await page.locator("h2").first().click({ force: true }).catch(() => {});
  await page.waitForTimeout(1500);
  const g = page.locator('text="Guardar"').first();
  await g.scrollIntoViewIfNeeded();
  await g.click();
  await page.waitForTimeout(20000);

  // Recargar y contar de verdad, que un guardado sin error no es un guardado.
  await cargar(url);
  const leido = await leer();
  const avisos = await page.locator('#search-terms he-message-bar[appearance="error"]').allInnerTexts();
  const bien = leido.length === quiero.length && quiero.every((p) => leido.includes(p)) && !avisos.length;
  console.log(`  tras recargar: ${leido.join(" | ")} -> ${bien ? "BIEN" : "MAL"}${avisos.length ? `; avisos: ${avisos.join(" / ")}` : ""}`);
  if (!bien) todoBien = false;
}
console.log(todoBien ? "\nTODO BIEN" : "\nALGO NO CUADRA");
await ctx.close();
process.exit(todoBien ? 0 : 1);
