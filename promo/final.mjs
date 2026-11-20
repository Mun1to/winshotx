/**
 * El vídeo de la 0.6.0 de una vez, para cuando cambie algo de lo que enseña: el peso del
 * instalador (`cifras.json`) o las fotos de Linux (`public/linux/`).
 *
 *   node capturas.mjs --raiz=<árbol de la versión>   solo si cambió el peso o la app (las fotos de la app)
 *   node final.mjs                                    el vídeo mudo, la mezcla y la foto para la página
 *
 * Deja `out/winshotx-0.6.0.mp4` (Be Happy, solo los clics, como el de la 0.5) y
 * `out/0.6.0/linux.jpg`, la foto de la escena nueva que sale en `out/0.6.0/index.html`. Con otro
 * nombre que el de la 0.5 (`out/Promo-3-be-happy-clics.mp4`), que no se pisa.
 *
 * Antes de nada mira que este PC aguante lo pesado: que el
 * Fedora de WSL esté apagado, que no haya un cargo ni un rustc en marcha y que Windows tenga 10 GB
 * libres o más. Si falla una, para sin renderizar.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { freemem } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const PISTA = "musica-candidatas/3-be-happy.mp3";
/** La pista entra en su segundo 6,56 para que su subida caiga en el logo del cierre (golpe 84, 42 s)
    y cada corte en un golpe; con el cierre de la 0.5 (golpe 82) era 7,56. */
const DESDE = 6.56;
/** El primer logo, el de la historia (golpe 11): la música va baja hasta ahí. */
const SUBE = 5.5;
/** Un fotograma de la escena de Linux con la de GNOME nítida y la respuesta entera. */
const FOTO_LINUX = 615;

function lanza(orden, args) {
  console.log(`> ${orden} ${args.join(" ")}`);
  const r = spawnSync(orden, args, { cwd: aqui, stdio: "inherit", shell: process.platform === "win32" });
  if (r.status !== 0) process.exit(r.status ?? 1);
}

/* Lo de la memoria. */
const wsl = spawnSync("wsl", ["-l", "--running", "-q"], { encoding: "utf16le" });
const encendidas = (wsl.stdout ?? "").replace(/\0/g, "").trim();
const tareas = spawnSync("tasklist", ["/FO", "CSV", "/NH"], { encoding: "utf8" }).stdout ?? "";
const compilando = tareas.split(/\r?\n/).filter((l) => /^"(cargo|rustc)\.exe"/i.test(l)).length;
const libres = freemem() / 1024 ** 3;
const pegas = [
  encendidas && `WSL está encendido (${encendidas.split(/\s+/).join(", ")})`,
  compilando && `hay ${compilando} cargo o rustc en marcha`,
  libres < 10 && `Windows tiene ${libres.toFixed(1)} GB libres (hacen falta 10)`,
].filter(Boolean);
if (pegas.length) {
  console.error(`no renderizo: ${pegas.join("; ")}. Espera y vuelve a lanzarlo.`);
  process.exit(2);
}
if (!existsSync(join(aqui, PISTA))) {
  console.error(`falta ${PISTA} (fuera de git: es la que eligió Munito, de Mixkit)`);
  process.exit(1);
}
const peso = JSON.parse(readFileSync(join(aqui, "cifras.json"), "utf8")).pesoInstalador;
console.log(`libre: ${libres.toFixed(1)} GB; el instalador, a ${peso} MB`);

lanza("node", ["render.mjs", "Promo"]);
lanza("node", ["musica.mjs", PISTA, `--desde=${DESDE}`, `--sube=${SUBE}`, "--variante=clics", "--salida=winshotx-0.6.0"]);
// `render.mjs` acaba de empaquetar el proyecto en out/paquete: la foto sale de ese paquete.
lanza("pnpm", ["exec", "remotion", "still", "out/paquete", "Promo", "out/0.6.0/linux.jpg", `--frame=${FOTO_LINUX}`, "--jpeg-quality=88"]);
console.log("hecho: out/winshotx-0.6.0.mp4 y out/0.6.0/linux.jpg (la página: out/0.6.0/index.html)");
