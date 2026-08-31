// Publica una version de winshotx: prepara los archivos que espera el
// actualizador y, si se le dice, crea la release en GitHub.
//
//   node scripts/publicar.mjs            solo prepara y enseña lo que haria
//   node scripts/publicar.mjs --publicar  crea la release de verdad
//
// El actualizador de Tauri pide tres cosas en el mismo sitio: el instalador,
// su firma y un latest.json que apunte a ellos. Hacer ese JSON a mano es pedir
// que un dia falte una firma y nadie se entere hasta que alguien no pueda
// actualizar.

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const raiz = process.cwd();
const paquete = JSON.parse(readFileSync(join(raiz, "package.json"), "utf8"));
const version = paquete.version;
const tag = `v${version}`;
const repo = "Mun1to/winshotx";

// package.json manda, pero Cargo.toml lleva la suya y de ella salen el instalador y el
// numero que ve el actualizador. Si se separan, se publica una version con el nombre de otra.
const cargo = readFileSync(join(raiz, "src-tauri", "Cargo.toml"), "utf8");
const enCargo = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (enCargo !== version) {
  console.error(`package.json dice ${version} y src-tauri/Cargo.toml dice ${enCargo}.`);
  console.error("Ponlas iguales antes de publicar.");
  process.exit(1);
}

// Los binarios no salen dentro del proyecto: CARGO_TARGET_DIR los manda a C:\ct.
const target = process.env.CARGO_TARGET_DIR ?? join(raiz, "src-tauri", "target");
const nsis = join(target, "release", "bundle", "nsis");

const instalador = join(nsis, `winshotx_${version}_x64-setup.exe`);
const firma = `${instalador}.sig`;
// Nombre fijo para que el enlace de descarga del README no cambie nunca.
const estable = join(nsis, "winshotx-setup.exe");

for (const archivo of [instalador, firma]) {
  if (!existsSync(archivo)) {
    console.error(`Falta ${archivo}`);
    console.error(
      "Compila antes con la clave de firma puesta:\n" +
        '  TAURI_SIGNING_PRIVATE_KEY_PATH="$HOME/.tauri/winshotx.key" pnpm tauri build',
    );
    process.exit(1);
  }
}

// Compilar sin la clave de firma no falla: deja en su sitio el .sig de la version anterior.
// Esa firma no valida el instalador nuevo, asi que el actualizador rechazaria la descarga y no
// nos enterariamos hasta que a alguien le fallara la actualizacion.
const nacimientoExe = statSync(instalador).mtimeMs;
const nacimientoSig = statSync(firma).mtimeMs;
if (nacimientoSig < nacimientoExe - 5000) {
  console.error(`La firma es mas vieja que el instalador:`);
  console.error(`  ${instalador}  ${new Date(nacimientoExe).toISOString()}`);
  console.error(`  ${firma}  ${new Date(nacimientoSig).toISOString()}`);
  console.error("Es el .sig de otra compilacion. Vuelve a compilar con la clave puesta:");
  console.error('  export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/winshotx.key)"');
  process.exit(1);
}

// El tamanno del instalador esta escrito a mano en los dos README y en la web, y ahi se
// queda cuando cambia: llegaron a convivir cuatro cifras para el mismo archivo (2.3, 2,2,
// 2,35 y 2,50), y una hasta se contradecia con la imagen de su propia insignia. Nadie mira
// eso, asi que lo mira esto, que es el unico sitio donde el numero de verdad esta a mano.
const megas = (statSync(instalador).size / 1e6).toFixed(2);
const bien = new RegExp(`^${megas.replace(".", "[.,]")}$`);

// No vale con buscar cualquier «N,N MB»: la pagina habla tambien de los 3,2 MB por segundo
// que escribe el anillo y de los 438,7 MB que llega a ocupar en disco. Solo se miran las
// cifras que tienen la palabra instalador al lado, que son las que anuncian ESTE archivo.
const CERCA = 70;
const ANUNCIAN = [
  "README.md",
  "README.es.md",
  join("frontlaxweb", "index.html"),
  join("frontlaxweb", "docs", "index.html"),
  join("frontlaxweb", "llms.txt"),
];
const desfasados = [];
let veces = 0;
for (const archivo of ANUNCIAN) {
  const ruta = join(raiz, archivo);
  if (!existsSync(ruta)) continue;
  const texto = readFileSync(ruta, "utf8");
  for (const m of texto.matchAll(/(\d+[.,]\d+)\s*(?:%20)?MB/g)) {
    const ventana = texto.slice(Math.max(0, m.index - CERCA), m.index + CERCA);
    if (!/instalador|installer/i.test(ventana)) continue;
    if (bien.test(m[1])) veces++;
    else desfasados.push(`${archivo}: «${m[0]}» en «...${ventana.replace(/\s+/g, " ").slice(20, 95)}...»`);
  }
}
if (desfasados.length) {
  console.error(`El instalador pesa ${megas} MB y esto dice otra cosa:`);
  for (const d of [...new Set(desfasados)]) console.error(`  ${d}`);
  console.error("Cambialo antes de publicar, o la version nueva se anuncia con el peso de otra.");
  process.exit(1);
}
if (!veces) {
  console.error("No he encontrado el tamanno del instalador anunciado en ningun sitio.");
  console.error("O se ha dejado de anunciar, o la palabra «instalador» ya no esta al lado.");
  process.exit(1);
}
console.log(`Instalador: ${megas} MB, y asi lo dicen los ${veces} sitios que lo anuncian.`);

copyFileSync(instalador, estable);

const latest = {
  version,
  notes: `winshotx ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      url: `https://github.com/${repo}/releases/download/${tag}/winshotx-setup.exe`,
      signature: readFileSync(firma, "utf8").trim(),
    },
  },
};

const destino = join(nsis, "latest.json");
writeFileSync(destino, JSON.stringify(latest, null, 2));

const activos = [instalador, estable, firma, destino];
console.log(`winshotx ${version} preparado:`);
for (const a of activos) console.log(`  ${a}`);

if (!process.argv.includes("--publicar")) {
  console.log("\nNada subido. Repite con --publicar cuando quieras crear la release.");
  process.exit(0);
}

execFileSync(
  "gh",
  [
    "release",
    "create",
    tag,
    ...activos,
    "--repo",
    repo,
    "--title",
    `winshotx ${version}`,
    "--notes",
    `Actualización automática desde la propia app: Ajustes → Actualizaciones.`,
  ],
  { stdio: "inherit" },
);
