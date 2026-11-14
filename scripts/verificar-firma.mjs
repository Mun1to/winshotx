// Comprueba que la firma del actualizador vale de verdad, con la clave publica del proyecto.
//
//   node scripts/verificar-firma.mjs                     la ultima version publicada
//   node scripts/verificar-firma.mjs v0.2.23             una version concreta
//   node scripts/verificar-firma.mjs --local <exe> <sig> un par de archivos del disco
//
// Por que hace falta: si la firma no cuadra con la clave publica que lleva la aplicacion
// dentro, **el actualizador rechaza la descarga** y todas las copias instaladas se quedan
// donde estan. No hay aviso: el boton de actualizar simplemente no acaba nunca. Y eso no lo
// ve ninguna prueba del repo, porque la firma se genera al empaquetar, fuera de cargo.
//
// De una release se comprueban todas las plataformas de su latest.json (Windows y, cuando
// salga, Linux). Como se lee y se comprueba una firma esta en `minisign.mjs`.

import { readFileSync } from "node:fs";
import { comprobarFirma, leerClave, leerFirma } from "./minisign.mjs";

const args = process.argv.slice(2);
const local = args.includes("--local");

const conf = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const clave = conf.plugins?.updater?.pubkey;
if (!clave) {
  console.error("tauri.conf.json no lleva plugins.updater.pubkey.");
  process.exit(1);
}
const pub = leerClave(clave);
console.log(`Clave publica de la app: ${pub.algoritmo} id ${pub.id}`);

// Cada cosa a comprobar: de donde sale, sus bytes y el contenido de su .sig.
const pares = [];
if (local) {
  const [, rutaExe, rutaSig] = args;
  pares.push({ de: rutaExe, bytes: readFileSync(rutaExe), firma: readFileSync(rutaSig, "utf8").trim() });
} else {
  const tag = args.find((a) => !a.startsWith("--")) ?? "latest";
  const base = tag === "latest"
    ? "https://github.com/Mun1to/winshotx/releases/latest/download"
    : `https://github.com/Mun1to/winshotx/releases/download/${tag}`;
  const latest = await (await fetch(`${base}/latest.json`)).json();
  console.log(`Version publicada: ${latest.version} (${latest.pub_date})`);
  // Varias claves de Linux apuntan al mismo AppImage: se baja una vez.
  const porUrl = new Map();
  for (const [plataforma, { url, signature }] of Object.entries(latest.platforms)) {
    const visto = porUrl.get(url);
    if (visto) visto.plataformas.push(plataforma);
    else porUrl.set(url, { plataformas: [plataforma], firma: signature });
  }
  for (const [url, { plataformas, firma }] of porUrl) {
    const bytes = Buffer.from(await (await fetch(url)).arrayBuffer());
    pares.push({ de: `${plataformas.join(", ")}: ${url}`, bytes, firma });
  }
}

let malas = 0;
for (const { de, bytes, firma } of pares) {
  const leida = leerFirma(firma);
  const resultado = comprobarFirma(bytes, firma, clave);
  console.log(`\n${de} (${bytes.length} bytes)`);
  console.log(`  firma ${leida.algoritmo} id ${leida.id}`);
  if (resultado.vale) {
    console.log("  FIRMA VALIDA: el actualizador aceptara esta descarga.");
  } else {
    malas++;
    console.log(`  FIRMA NO VALIDA (${resultado.motivo}): el actualizador la RECHAZARA.`);
  }
}
process.exit(malas ? 1 : 0);
