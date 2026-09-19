// Comprueba que winshotx ARRANCA DEL TODO y sigue respondiendo, sin tocar el ratón de nadie.
//
//   node scripts/arranca-bien.mjs                  un arranque del binario de C:\ct\release
//   node scripts/arranca-bien.mjs --vueltas=6      seis seguidos
//   node scripts/arranca-bien.mjs --carga          con la máquina a tope, que es cuando
//                                                  salen las carreras entre hilos
//   node scripts/arranca-bien.mjs otra\ruta.exe    cualquier otro binario
//
// Existe por el cuelgue de la 0.2.13: la aplicación arrancaba, se quedaba a medio crear el
// pool de overlays y no volvía a responder a un solo mensaje de Windows. Desde fuera no se
// veía nada raro (el proceso estaba ahí y el icono de la bandeja también), y ninguna prueba
// de `cargo test` podía verlo, porque el fallo no está en ninguna función: está en que un
// hilo le preguntaba algo al bucle de eventos y lo esperaba sin plazo.
//
// Lo que mira, y por qué cada cosa:
//
//   1. Que el proceso RESPONDE. `Responding` de Windows es preguntarle a la ventana si
//      atiende mensajes; a un hilo principal bloqueado le sale que no.
//   2. Que ninguna ventana se quedó a 800x600, que es el tamaño con el que nacen y que
//      `precrear_overlays` cambia enseguida por el del monitor. Una que se quede así es una
//      ventana a la que no le dio tiempo a terminar de crearse.
//   3. Que hay al menos una ventana de captura, o sea que el pool llegó a montarse.
//
// Espera más de lo que tarda el menú de la bandeja en precalentarse (`tray_menu.rs`), que
// es justo cuando aparecía este fallo. Y se repite, porque una sola pasada en verde no dice
// nada: la primera vez que se probó esto salió bien seis veces seguidas y en la máquina de
// Munir se colgó a la primera.

import { execFileSync, spawn } from "node:child_process";
import { existsSync } from "node:fs";

const banderas = process.argv.slice(2).filter((a) => a.startsWith("--"));
const exe =
  process.argv.slice(2).find((a) => !a.startsWith("--")) ?? "C:\\ct\\release\\winshotx.exe";
const vueltas = Number(banderas.find((a) => a.startsWith("--vueltas="))?.split("=")[1] ?? 1);
const conCarga = banderas.includes("--carga");

if (!existsSync(exe)) {
  console.error(`No encuentro ${exe}`);
  process.exit(1);
}

/** Segundos de espera antes de mirar. El precalentado del menú tarda 6, más margen. */
const ESPERA = 20;
/** El tamaño con el que nace una ventana de Tauri a la que no se le ha dicho otra cosa. */
const SIN_TERMINAR = "800x600";

const ps = (guion) =>
  execFileSync("powershell", ["-NoProfile", "-Command", guion], { encoding: "utf8" }).trim();

/**
 * Lo mismo, pero sin reventar si falla.
 *
 * `Stop-Process` de algo que ya no existe devuelve error aunque se le pida silencio, y eso
 * tumbaba este guion entero justo al final, despues de que las vueltas hubieran salido
 * bien: el resultado se perdia por culpa de la limpieza.
 */
const psSuave = (guion) => {
  try {
    return ps(guion);
  } catch {
    return "";
  }
};

/** Mata un proceso por su identificador, sin quejarse si ya no esta. */
const matar = (pid) => psSuave(`try { Stop-Process -Id ${pid} -Force -ErrorAction Stop } catch {}`);

const MIRAR = (pid) => `
$src = @"
using System;using System.Text;using System.Runtime.InteropServices;using System.Collections.Generic;
public class VW {
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int L, T, R, B; }
  delegate bool EnumProc(IntPtr h, IntPtr l);
  public static List<string> Listar(uint pid) {
    var res = new List<string>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid) {
        var sb = new StringBuilder(256); GetWindowText(h, sb, 256);
        RECT r; GetWindowRect(h, out r);
        res.Add(sb.ToString() + "|" + (r.R-r.L) + "x" + (r.B-r.T));
      }
      return true;
    }, IntPtr.Zero);
    return res;
  }
}
"@
if (-not ("VW" -as [type])) { Add-Type -TypeDefinition $src }
$p = Get-Process -Id ${pid} -ErrorAction SilentlyContinue
if (-not $p) { Write-Output "MUERTO"; exit }
Write-Output ("RESPONDE:" + $p.Responding)
Write-Output ("MB:" + [math]::Round($p.WorkingSet64/1MB,1))
[VW]::Listar(${pid}) | ForEach-Object { Write-Output ("VENTANA:" + $_) }
`;

/** Un arranque entero: lanzar, esperar, mirar y matar. Devuelve la lista de fallos. */
async function unArranque(vuelta) {
  // Con otra instancia viva, la que se lance se cierra sola (single-instance) y esto no
  // mediría nada: sería dar por buena una prueba que no llegó a correr.
  const vivos = ps("(Get-Process -Name winshotx -ErrorAction SilentlyContinue).Count");
  if (vivos && Number(vivos) > 0) {
    console.error("Hay un winshotx abierto. Ciérralo: con otro vivo, este se cerraría solo.");
    process.exit(1);
  }

  console.log(`\nVuelta ${vuelta}: arrancando y esperando ${ESPERA} s...`);
  const hijo = spawn(exe, { detached: true, stdio: "ignore" });
  hijo.unref();
  const pid = hijo.pid;

  await new Promise((listo) => setTimeout(listo, ESPERA * 1000));

  const lineas = ps(MIRAR(pid)).split(/\r?\n/).map((l) => l.trim());
  matar(pid);

  const fallos = [];
  if (lineas.includes("MUERTO")) {
    fallos.push("el proceso se ha muerto solo antes de que lo mirara");
    return fallos;
  }

  const responde = lineas.find((l) => l.startsWith("RESPONDE:"))?.slice(9);
  if (responde !== "True") {
    fallos.push(`NO responde a los mensajes de Windows (Responding=${responde})`);
  }

  const ventanas = lineas.filter((l) => l.startsWith("VENTANA:")).map((l) => l.slice(8));
  const aMedias = ventanas.filter((v) => v.endsWith(`|${SIN_TERMINAR}`));
  if (aMedias.length) {
    fallos.push(
      `${aMedias.length} ventana(s) se han quedado a ${SIN_TERMINAR}, creadas y sin terminar`,
    );
  }

  // Los overlays son las ventanas «winshotx» grandes, una por monitor. La de ajustes lleva
  // otro título, así que no se cuela aquí.
  const overlays = ventanas.filter((v) => {
    const [titulo, medida] = v.split("|");
    return titulo === "winshotx" && Number(medida.split("x")[0]) >= 800;
  });
  if (overlays.length === 0) {
    fallos.push("no hay ni una ventana de captura: el pool de overlays no se ha creado");
  }

  const mb = lineas.find((l) => l.startsWith("MB:"))?.slice(3);
  console.log(
    `  responde=${responde}  ventanas=${ventanas.length}  captura=${overlays.length}  ${mb} MB`,
  );
  return fallos;
}

/** Procesos que se comen la CPU, para que el arranque compita como en un día malo. */
function encenderCarga() {
  const hijos = [];
  for (let i = 0; i < 6; i++) {
    const p = spawn(
      "powershell",
      ["-NoProfile", "-Command", "$x=0; while($true){ $x=[math]::Sqrt($x+1) }"],
      { detached: true, stdio: "ignore" },
    );
    hijos.push(p);
  }
  return () => hijos.forEach((p) => matar(p.pid));
}

const apagarCarga = conCarga ? encenderCarga() : () => {};
if (conCarga) console.log("Máquina cargada a propósito: seis procesos quemando CPU.");

let rotas = 0;
try {
  for (let v = 1; v <= vueltas; v++) {
    const fallos = await unArranque(v);
    for (const f of fallos) console.error(`  ROTO: ${f}`);
    if (fallos.length) rotas++;
  }
} finally {
  apagarCarga();
  psSuave("try { Stop-Process -Name winshotx -Force -ErrorAction Stop } catch {}");
}

if (rotas) {
  console.error(`\nARRANQUE ROTO en ${rotas} de ${vueltas} vueltas.`);
  process.exit(1);
}
console.log(`\nArranque correcto en las ${vueltas} vueltas.`);
