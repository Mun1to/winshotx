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
