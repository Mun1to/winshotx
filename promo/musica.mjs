/**
 * Le pone la música al vídeo mudo que saca `node render.mjs Promo`, sin volver a renderizar.
 *
 *   node musica.mjs <pista>                     out/Promo.mp4 + la pista -> out/Promo-<pista>.mp4
 *   node musica.mjs <pista> --arranque=20       solo los primeros 20 s (para elegir pista)
 *   node musica.mjs <pista> --desde=12          la pista empezando en su segundo 12
 *
 * La pista se iguala a −16 LUFS con el pico por debajo de −1,5 dBTP (las redes normalizan hacia
 * −14, y así no la suben a golpes), entra en 1,5 s y se va en los últimos 2,5. Va aparte del
 * render porque la elige Munito entre varias: así cambiar de pista son diez segundos.
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { basename, dirname, extname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { setPriority, constants } from "node:os";

const aqui = dirname(fileURLToPath(import.meta.url));
setPriority(constants.priority.PRIORITY_BELOW_NORMAL);

const args = process.argv.slice(2);
const pista = args.find((a) => !a.startsWith("--"));
const opcion = (n) => args.find((a) => a.startsWith(`--${n}=`))?.split("=")[1];
const arranque = opcion("arranque") ? Number(opcion("arranque")) : null;
const desde = Number(opcion("desde") ?? 0);
const video = join(aqui, "out", "Promo.mp4");

if (!pista || !existsSync(pista)) {
  console.error("Uso: node musica.mjs <pista> [--arranque=s] [--desde=s]");
  process.exit(1);
}
if (!existsSync(video)) {
  console.error("falta out/Promo.mp4: corre antes `node render.mjs Promo`");
  process.exit(1);
}

const dura = (f) =>
  Number(spawnSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", f], { encoding: "utf8" }).stdout.trim());
const largo = arranque ?? dura(video);
const nombre = basename(pista, extname(pista));
const salida = join(aqui, "out", arranque ? `arranque-${nombre}.mp4` : `Promo-${nombre}.mp4`);

const filtro = [
  `atrim=start=${desde}:duration=${largo}`,
  "asetpts=PTS-STARTPTS",
  "loudnorm=I=-16:TP=-1.5:LRA=11",
  "afade=t=in:st=0:d=1.5",
  `afade=t=out:st=${Math.max(0, largo - 2.5)}:d=2.5`,
  "aresample=48000",
].join(",");

const r = spawnSync(
  "ffmpeg",
  ["-hide_banner", "-loglevel", "error", "-y", "-i", video, "-stream_loop", "-1", "-i", pista,
    "-t", String(largo), "-map", "0:v", "-map", "1:a", "-c:v", "copy", "-af", filtro, "-c:a", "aac", "-b:a", "192k", salida],
  { stdio: "inherit" },
);
if (r.status !== 0) process.exit(r.status ?? 1);
console.log(`hecho: ${salida}`);
