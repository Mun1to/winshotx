// Cronometra el camino del atajo en la aplicacion DE VERDAD: desde que se pide la captura
// hasta que cada pantalla tiene su imagen pintada.
//
//   node scripts/cronometrar-atajo.mjs                 seis capturas con C:\ct\release\winshotx.exe
//   node scripts/cronometrar-atajo.mjs --vueltas=10
//   node scripts/cronometrar-atajo.mjs otra\ruta.exe
//
// Como funciona: para cualquier winshotx que haya, arranca el binario con `--crono` (que
// escribe cada etapa en %TEMP%\winshotx\crono.log, ver src-tauri/src/crono.rs), espera a que
// el pool de overlays y el menu de la bandeja esten calientes, y despues repite: dispara una
// captura con `winshotx.exe --capture`, espera, la cierra con `--cancel`, y lee las marcas.
//
// **Abre el overlay encima del escritorio** durante un par de segundos por vuelta: solo se
// corre con el usuario avisado. Al terminar vuelve a arrancar la aplicacion instalada.
//
// Existe porque este camino se midio tres veces mal (trampas 33 y 36): con binarios sin
// interfaz dentro y con `println!` en release, donde no hay consola. Aqui las marcas van a un
// archivo, y el binario tiene que venir de `pnpm tauri build`.

import { execFileSync, spawn } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

const banderas = process.argv.slice(2).filter((a) => a.startsWith("--"));
const exe =
  process.argv.slice(2).find((a) => !a.startsWith("--")) ?? "C:\\ct\\release\\winshotx.exe";
const vueltas = Number(banderas.find((a) => a.startsWith("--vueltas="))?.split("=")[1] ?? 6);
const INSTALADA = "C:\\Apps\\Random APPS\\winshotx\\winshotx.exe";
const LOG = join(tmpdir(), "winshotx", "crono.log");

if (!existsSync(exe)) {
  console.error(`No encuentro ${exe}`);
  process.exit(1);
}

const espera = (ms) => new Promise((r) => setTimeout(r, ms));
const ps = (guion) => {
  try {
    return execFileSync("powershell", ["-NoProfile", "-Command", guion], { encoding: "utf8" }).trim();
  } catch {
    return "";
  }
};
const matarTodos = () => ps("Get-Process winshotx -ErrorAction SilentlyContinue | Stop-Process -Force");

/** Lanza una segunda instancia con un argumento: la que corre lo recibe y la nueva se cierra. */
const orden = (arg) => {
  try {
    execFileSync(exe, [arg], { stdio: "ignore", timeout: 10_000 });
  } catch {
    // La segunda instancia sale con codigo distinto de cero al pasar el testigo. Da igual.
  }
};

/** Las marcas de una vuelta, en milisegundos desde `atajo`. */
function leerMarcas() {
  if (!existsSync(LOG)) return null;
  const lineas = readFileSync(LOG, "utf8").trim().split(/\r?\n/).filter(Boolean);
  const marcas = lineas.map((l) => {
    const [ms, etapa] = l.split("\t");
    return { ms: Number(ms), etapa };
  });
  const atajo = marcas.find((m) => m.etapa === "atajo");
  if (!atajo) return null;
  const relativas = {};
  for (const m of marcas) {
    if (m.ms < atajo.ms) continue;
    relativas[m.etapa] = Math.round(m.ms - atajo.ms);
  }
  return relativas;
}

const mediana = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  return s.length ? s[Math.floor(s.length / 2)] : null;
};

console.log(`Binario: ${exe}`);
console.log("Cerrando cualquier winshotx que haya...");
matarTodos();
await espera(1500);
rmSync(LOG, { force: true });

console.log("Arrancando con --crono y esperando 15 s a que se caliente el pool...");
const app = spawn(exe, ["--crono"], { detached: true, stdio: "ignore" });
app.unref();
await espera(15_000);

const vueltasLeidas = [];
for (let i = 1; i <= vueltas; i++) {
  rmSync(LOG, { force: true });
  orden("--capture");
  await espera(2500);
  orden("--cancel");
  await espera(1200);
  const marcas = leerMarcas();
  if (!marcas) {
    console.log(`  vuelta ${i}: sin marcas (¿el binario lleva --crono y la interfaz dentro?)`);
    continue;
  }
  vueltasLeidas.push(marcas);
  const resumen = Object.entries(marcas)
    .sort((a, b) => a[1] - b[1])
    .map(([k, v]) => `${k}=${v}`)
    .join("  ");
  console.log(`  vuelta ${i}: ${resumen}`);
}

if (vueltasLeidas.length) {
  console.log("\nMedianas (ms desde el atajo):");
  const etapas = [...new Set(vueltasLeidas.flatMap((m) => Object.keys(m)))];
  const filas = etapas
    .map((e) => [e, mediana(vueltasLeidas.filter((m) => e in m).map((m) => m[e]))])
    .sort((a, b) => a[1] - b[1]);
  for (const [e, v] of filas) console.log(`  ${String(v).padStart(5)}  ${e}`);
  const pintados = filas.filter(([e]) => e.startsWith("js-pintado")).map(([, v]) => v);
  if (pintados.length) console.log(`\nLa ultima pantalla pintada: ${Math.max(...pintados)} ms`);
}

console.log("\nCerrando el binario de pruebas y volviendo a arrancar la instalada...");
matarTodos();
await espera(1000);
if (existsSync(INSTALADA)) {
  const instalada = spawn(INSTALADA, [], { detached: true, stdio: "ignore" });
  instalada.unref();
}
