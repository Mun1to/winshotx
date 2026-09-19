// Sube el numero de version en los dos sitios que lo llevan escrito, a la vez.
//
//   node scripts/version.mjs 0.1.15   lo pone en package.json y en src-tauri/Cargo.toml
//   node scripts/version.mjs parche   sube el ultimo numero
//
// Existe porque son dos archivos con dos formatos distintos y basta con olvidarse de uno
// para publicar una version con el nombre de otra. `scripts/publicar.mjs` lo detecta y se
// planta, pero eso pasa despues de haber compilado, que son catorce minutos tirados.

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const raiz = process.cwd();
const rutaPaquete = join(raiz, "package.json");
const rutaCargo = join(raiz, "src-tauri", "Cargo.toml");

const paquete = JSON.parse(readFileSync(rutaPaquete, "utf8"));
const actual = paquete.version;

const pedida = process.argv[2];
if (!pedida) {
  console.error(`Ahora mismo es la ${actual}.`);
  console.error("Dime cuál quieres: node scripts/version.mjs 0.1.15");
  console.error("O sube el último número: node scripts/version.mjs parche");
  process.exit(1);
}

const [mayor, menor, parche] = actual.split(".").map(Number);
const NUEVA = {
  parche: `${mayor}.${menor}.${parche + 1}`,
  menor: `${mayor}.${menor + 1}.0`,
  mayor: `${mayor + 1}.0.0`,
}[pedida] ?? pedida;

if (!/^\d+\.\d+\.\d+$/.test(NUEVA)) {
  console.error(`"${NUEVA}" no es un número de versión.`);
  process.exit(1);
}

// Solo la primera línea `version` de Cargo.toml: es la del paquete. Las de las
// dependencias vienen después y no se tocan.
const cargo = readFileSync(rutaCargo, "utf8");
const enCargo = cargo.match(/^version\s*=\s*"([^"]+)"/m);
if (!enCargo) {
  console.error("No encuentro la versión en src-tauri/Cargo.toml.");
  process.exit(1);
}

paquete.version = NUEVA;
writeFileSync(rutaPaquete, `${JSON.stringify(paquete, null, 2)}\n`);
writeFileSync(rutaCargo, cargo.replace(enCargo[0], `version = "${NUEVA}"`));

// Y el candado, que lleva la versión dentro y si no se regenera sale un cambio suelto en
// el siguiente commit que nadie sabe de dónde viene.
try {
  execFileSync("cargo", ["update", "--package", "winshotx", "--precise", NUEVA], {
    cwd: join(raiz, "src-tauri"),
    stdio: "ignore",
  });
} catch {
  // `cargo update` no vale para el paquete local: se regenera compilando, y `cargo check`
  // es lo más barato que lo hace.
  try {
    execFileSync("cargo", ["check", "--quiet"], {
      cwd: join(raiz, "src-tauri"),
      stdio: "ignore",
    });
  } catch {
    console.error("Ojo: no he podido regenerar Cargo.lock. Corre `cargo check` antes de commitear.");
  }
}

console.log(`${actual} → ${NUEVA}`);
console.log("package.json y src-tauri/Cargo.toml al día.");
