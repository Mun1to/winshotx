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
// El formato es el de minisign, que es lo que usa Tauri:
//
//   - La clave publica de `tauri.conf.json` es base64 de un archivo de clave publica entero
//     (con su comentario). Dentro: 2 bytes de algoritmo, 8 de identificador y 32 de clave.
//   - El `.sig` es base64 de un archivo de firma entero. Dentro: 2 bytes de algoritmo, 8 de
//     identificador y 64 de firma.
//   - `Ed` firma el contenido tal cual; `ED` firma su BLAKE2b-512. Tauri usa la segunda.
//
// Si los identificadores no coinciden, la firma es de OTRA clave: eso es lo que pasa cuando
// se firma con una clave distinta de la que lleva la app compilada, y es el fallo que deja
// huerfanas a todas las instalaciones sin que nadie se entere.

import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";

const args = process.argv.slice(2);
const local = args.includes("--local");

/** Saca las tres partes de un archivo minisign (clave o firma) que viene en base64. */
function partes(base64Entero, bytesFinales) {
  const texto = Buffer.from(base64Entero, "base64").toString("utf8");
  const linea = texto.split("\n").map((l) => l.trim()).filter((l) => l && !l.startsWith("untrusted comment:"))[0];
  const cru = Buffer.from(linea, "base64");
  return {
    algoritmo: cru.subarray(0, 2).toString("latin1"),
    id: cru.subarray(2, 10).toString("hex"),
    dato: cru.subarray(10, 10 + bytesFinales),
  };
}

/** La clave publica de ed25519 en el formato que entiende Node. */
function claveDe(bytes32) {
  const der = Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), bytes32]);
  return createPublicKey({ key: der, format: "der", type: "spki" });
}

const conf = JSON.parse(readFileSync("C:/proyectos/winshotx/src-tauri/tauri.conf.json", "utf8"));
const pubkeyB64 = conf.plugins?.updater?.pubkey;
if (!pubkeyB64) {
  console.error("tauri.conf.json no lleva plugins.updater.pubkey.");
  process.exit(1);
}
const pub = partes(pubkeyB64, 32);
console.log(`Clave publica de la app: ${pub.algoritmo} id ${pub.id}`);

let exe, sigB64, de;
if (local) {
  const [, rutaExe, rutaSig] = args;
  exe = readFileSync(rutaExe);
  sigB64 = readFileSync(rutaSig, "utf8").trim();
  de = rutaExe;
} else {
  const tag = args.find((a) => !a.startsWith("--")) ?? "latest";
  const base = tag === "latest"
    ? "https://github.com/Mun1to/winshotx/releases/latest/download"
    : `https://github.com/Mun1to/winshotx/releases/download/${tag}`;
  const latest = await (await fetch(`${base}/latest.json`)).json();
  const plat = latest.platforms["windows-x86_64"];
  console.log(`Version publicada: ${latest.version} (${latest.pub_date})`);
  exe = Buffer.from(await (await fetch(plat.url)).arrayBuffer());
  sigB64 = plat.signature;
  de = plat.url;
}

const firma = partes(sigB64, 64);
console.log(`Firma del instalador   : ${firma.algoritmo} id ${firma.id}`);
console.log(`Instalador             : ${de} (${exe.length} bytes)`);

if (firma.id !== pub.id) {
  console.error("\nLA FIRMA ES DE OTRA CLAVE. El actualizador la rechazara.");
  process.exit(1);
}

// «ED» firma el BLAKE2b-512 del archivo; «Ed» firma el archivo entero.
const mensaje = firma.algoritmo === "ED" ? createHash("blake2b512").update(exe).digest() : exe;
const vale = verify(null, mensaje, claveDe(pub.dato), firma.dato);

console.log(`\nFIRMA ${vale ? "VALIDA" : "NO VALIDA"}: el actualizador ${vale ? "aceptara" : "RECHAZARA"} esta descarga.`);
process.exit(vale ? 0 : 1);
