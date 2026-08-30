// Monta el paquete MSIX de winshotx, que es lo que se sube a la Microsoft Store.
//
//   node scripts/msix.mjs             monta el paquete y lo firma para probarlo aqui
//   node scripts/msix.mjs --tienda    lo monta para subirlo, SIN firmar
//
// Por que MSIX y no el instalador de siempre: la Store acepta las dos cosas, pero un .exe
// hay que firmarlo con un certificado de una autoridad de pago, y un MSIX lo firma
// Microsoft gratis cuando pasa la revision. Para una app de una persona, esa es toda la
// diferencia entre poder estar y no.
//
// La firma de prueba de este script NO sirve para subir nada: solo permite instalar el
// paquete en este equipo para comprobar que la app funciona empaquetada. Al subirlo, la
// Store tira la firma que traiga y pone la suya, y por eso `--tienda` no firma.

import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import sharp from "sharp";

const raiz = process.cwd();
const paraLaTienda = process.argv.includes("--tienda");

const paquete = JSON.parse(readFileSync(join(raiz, "package.json"), "utf8"));
const version = paquete.version;

// El numero de MSIX lleva cuatro partes y la cuarta es de Microsoft: tiene que ir a cero
// o la Store rechaza el paquete sin decir cual de las reglas se ha saltado.
const versionMsix = `${version}.0`;

const identidad = JSON.parse(readFileSync(join(raiz, "packaging/store/identidad.json"), "utf8"));

const exe = "C:/ct/release/winshotx.exe";
if (!existsSync(exe)) {
  console.error(`No hay binario en ${exe}.`);
  console.error("Compilalo antes: pnpm build && cargo build --release --manifest-path src-tauri/Cargo.toml");
  process.exit(1);
}

// Que el binario sea de esta version y no de la anterior. Es el fallo que no avisa:
// el paquete se monta igual de bien con el exe de hace tres versiones.
const enCargo = readFileSync(join(raiz, "src-tauri/Cargo.toml"), "utf8").match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (enCargo !== version) {
  console.error(`package.json dice ${version} y Cargo.toml dice ${enCargo}. Ponlas iguales.`);
  process.exit(1);
}

const sdk = "C:/Program Files (x86)/Windows Kits/10/bin/10.0.26100.0/x64";
const makeappx = join(sdk, "makeappx.exe");
if (!existsSync(makeappx)) {
  console.error(`No encuentro makeappx.exe en ${sdk}. Hace falta el SDK de Windows 10/11.`);
  process.exit(1);
}

const salida = join(raiz, "packaging/store/paquete");
rmSync(salida, { recursive: true, force: true });
mkdirSync(join(salida, "Assets"), { recursive: true });

// --- Los iconos -------------------------------------------------------------------
//
// El icono de la app mide 512 px, asi que aqui solo se generan los tamannos que caben
// dentro de esos 512: estirar un icono para rellenar el mosaico grande se ve, y ademas
// las escalas de MSIX son opcionales. Mas vale tener menos y nitidas.
const fuente = join(raiz, "src-tauri/icons/icon.png");
const TECHO = 512;

/** Cuadrados: nombre base y su lado en la escala 100. */
const CUADRADOS = [
  ["StoreLogo", 50],
  ["Square44x44Logo", 44],
  ["Square150x150Logo", 150],
  ["SmallTile", 71],
  ["LargeTile", 310],
];
const ESCALAS = [100, 125, 150, 200, 400];

/** Los que Windows pide por tamanno exacto: la barra de tareas y la lista de aplicaciones. */
const POR_TAMANO = [16, 24, 32, 48, 256];

let hechos = 0;
for (const [nombre, lado] of CUADRADOS) {
  for (const escala of ESCALAS) {
    const px = Math.round((lado * escala) / 100);
    if (px > TECHO) continue;
    const archivo = escala === 100 ? `${nombre}.png` : `${nombre}.scale-${escala}.png`;
    await sharp(fuente).resize(px, px, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png().toFile(join(salida, "Assets", archivo));
    hechos++;
  }
}

for (const px of POR_TAMANO) {
  for (const sufijo of ["", ".altform-unplated", ".altform-lightunplated"]) {
    const archivo = `Square44x44Logo.targetsize-${px}${sufijo}.png`;
    await sharp(fuente).resize(px, px, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png().toFile(join(salida, "Assets", archivo));
    hechos++;
  }
}

// El mosaico ancho no es un cuadrado: el icono va centrado y manda el alto, que es la
// mitad del ancho. Escalarlo al ancho lo dejaria cortado por arriba y por abajo.
for (const escala of [100, 150]) {
  const ancho = Math.round((310 * escala) / 100);
  const alto = Math.round((150 * escala) / 100);
  if (alto > TECHO) continue;
  const icono = await sharp(fuente).resize(alto, alto).png().toBuffer();
  const archivo = escala === 100 ? "Wide310x150Logo.png" : `Wide310x150Logo.scale-${escala}.png`;
  await sharp({ create: { width: ancho, height: alto, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } } })
    .composite([{ input: icono, gravity: "center" }])
    .png().toFile(join(salida, "Assets", archivo));
  hechos++;
}

console.log(`Iconos: ${hechos}`);

// --- El binario -------------------------------------------------------------------
cpSync(exe, join(salida, "winshotx.exe"));

// --- El manifiesto ----------------------------------------------------------------
const plantilla = readFileSync(join(raiz, "packaging/store/AppxManifest.plantilla.xml"), "utf8");
const manifiesto = plantilla
  .replace("{{IDENTITY_NAME}}", identidad.identityName)
  .replace("{{PUBLISHER}}", identidad.publisher)
  .replace("{{VERSION}}", versionMsix)
  .replace("{{PUBLISHER_DISPLAY}}", identidad.publisherDisplayName)
  .replace("{{DESCRIPTION}}", paquete.description ?? "Screen capture and GIF/MP4 recording, local and open source.");

for (const hueco of manifiesto.match(/\{\{[A-Z_]+\}\}/g) ?? []) {
  console.error(`Ha quedado un hueco sin rellenar en el manifiesto: ${hueco}`);
  process.exit(1);
}
writeFileSync(join(salida, "AppxManifest.xml"), manifiesto, "utf8");

// --- Empaquetar -------------------------------------------------------------------
const msix = join(raiz, `packaging/store/winshotx_${versionMsix}_x64.msix`);
rmSync(msix, { force: true });
execFileSync(makeappx, ["pack", "/d", salida, "/p", msix, "/o"], { stdio: "inherit" });

const mb = (statSync(msix).size / 1024 / 1024).toFixed(2);
console.log(`\nPaquete: ${msix} (${mb} MB)`);

if (paraLaTienda) {
  if (!identidad.reservado) {
    console.log("\nAVISO: `reservado` sigue en false en packaging/store/identidad.json.");
    console.log("Los datos de identidad son provisionales y la Store rechazara el paquete.");
    console.log("Los de verdad salen de Partner Center, en 'Identidad de la aplicacion'.");
  }
  console.log("\nSin firmar, que es como lo quiere la Store: la pone ella al certificar.");
} else {
  console.log("\nFalta firmarlo para poder instalarlo aqui: node scripts/msix.mjs --firmar-prueba");
}
