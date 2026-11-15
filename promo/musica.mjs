/**
 * Le pone la música al vídeo mudo que saca `node render.mjs Promo`, sin volver a renderizar.
 *
 *   node musica.mjs <pista>                     out/Promo.mp4 + la pista -> out/Promo-<pista>.mp4
 *   node musica.mjs <pista> --arranque=20       solo los primeros 20 s (para elegir pista)
 *   node musica.mjs <pista> --desde=12          la pista empezando en su segundo 12
 *   node musica.mjs <pista> --sube=5.6          baja (−8 dB) hasta el segundo 5,6 y entera desde ahí
 *   node musica.mjs <pista> --video=Promo-127   sobre out/Promo-127.mp4 (el corte a 127,5 golpes)
 *   node musica.mjs <pista> --efectos=1         los efectos 3 dB más bajos que de costumbre (4)
 *   node musica.mjs <pista> --variante=clics    con la pista de efectos sin whoosh, en out/<video>-<pista>-clics.mp4
 *   node musica.mjs <pista> --sin-efectos       solo la música
 *
 * Los efectos de sonido son la pista `out/<video>-efectos.wav` que saca `render.mjs` a la vez que
 * el vídeo: van debajo de la música, cada uno en su fotograma, y no pasan por el `loudnorm`
 * (que es solo para igualar la música); el limitador del final los coge a los dos juntos.
 *
 * La pista se iguala a −14 LUFS con el pico por debajo de −1,5 dBTP: es la mediana de los 21
 * vídeos de su carpeta de referencias (−14,2) y lo que piden las redes, que normalizan hacia ahí.
 * Como en ellas, la música va baja mientras se cuenta el problema y entra entera con el nombre
 * (`--sube`, el segundo del logo). Entra en 0,8 s y se va en el último 1,5. Va aparte del render
 * porque la elige Munito entre varias: así cambiar de pista son diez segundos.
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
const sube = opcion("sube") ? Number(opcion("sube")) : null;
const base = opcion("video") ?? "Promo";
if (!/^[\w-]+$/.test(base)) process.exit(1);
const video = join(aqui, "out", `${base}.mp4`);
const variante = opcion("variante");
if (variante && !/^[\w-]+$/.test(variante)) process.exit(1);
const pistaEfectos = join(aqui, "out", `${base}-efectos${variante ? `-${variante}` : ""}.wav`);
const conEfectos = !args.includes("--sin-efectos") && existsSync(pistaEfectos);
if (variante && !conEfectos) {
  console.error(`falta ${pistaEfectos}: corre antes \`node render.mjs ${base} sonido\``);
  process.exit(1);
}
// Remotion no deja pasar de volumen 1, así que los efectos salen del render 4 dB por debajo de
// donde van y se suben aquí: con eso los clics quedan a la altura del nivel normal de la música,
// como en los vídeos de lanzamiento de fuera (medido el 15-nov).
const efectosDb = Number(opcion("efectos") ?? 4);

if (!pista || !existsSync(pista)) {
  console.error("Uso: node musica.mjs <pista> [--arranque=s] [--desde=s] [--sube=s] [--video=id] [--efectos=dB] [--sin-efectos]");
  process.exit(1);
}
if (!existsSync(video)) {
  console.error(`falta out/${base}.mp4: corre antes \`node render.mjs ${base}\``);
  process.exit(1);
}

const dura = (f) =>
  Number(spawnSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", f], { encoding: "utf8" }).stdout.trim());
const largo = arranque ?? dura(video);
const nombre = basename(pista, extname(pista));
const salida = join(aqui, "out", arranque ? `arranque-${nombre}.mp4` : `${base}-${nombre}${variante ? `-${variante}` : ""}.mp4`);

const musica = [
  `atrim=start=${desde}:duration=${largo}`,
  "asetpts=PTS-STARTPTS",
  "loudnorm=I=-14:TP=-1.5:LRA=11",
  // −8 dB (0,4) durante la historia y una subida de 0,6 s que acaba justo en el logo.
  ...(sube ? [`volume='if(lt(t,${sube - 0.6}),0.4,if(lt(t,${sube}),0.4+(t-${sube - 0.6})/0.6*0.6,1))':eval=frame`] : []),
  "afade=t=in:st=0:d=0.8",
  `afade=t=out:st=${Math.max(0, largo - 1.5)}:d=1.5`,
].join(",");
const final = [
  // loudnorm de una pasada no asegura el pico, y el AAC lo sube: un limitador a −3 dB lo deja
  // entre −1,6 y −2,6 dBTP en el archivo final; la pista más brillante (Electron 01) se queda
  // en −0,8 (medido con ebur128): por debajo de 0, no satura. Y a 256 kbps, no a 192: con
  // Gummies, el AAC de ffmpeg a 192 subía el pico a +2,5 dB sobre una mezcla limitada a −3.
  "alimiter=limit=0.71:attack=5:release=50:level=false",
  "aresample=48000",
].join(",");

// La música por su cadena; los efectos, con su ganancia, sumados tal cual (sin normalizar la
// suma, que bajaría la música a la mitad); y el limitador y la salida, a los dos juntos.
const grafo = conEfectos
  ? `[1:a]${musica}[m];[2:a]atrim=duration=${largo},volume=${efectosDb}dB[e];[m][e]amix=inputs=2:normalize=0:duration=first,${final}[a]`
  : `[1:a]${musica},${final}[a]`;
const r = spawnSync(
  "ffmpeg",
  ["-hide_banner", "-loglevel", "error", "-y", "-i", video, "-stream_loop", "-1", "-i", pista, ...(conEfectos ? ["-i", pistaEfectos] : []),
    "-t", String(largo), "-filter_complex", grafo, "-map", "0:v", "-map", "[a]", "-c:v", "copy", "-c:a", "aac", "-b:a", "256k", salida],
  { stdio: "inherit" },
);
if (r.status !== 0) process.exit(r.status ?? 1);
console.log(`hecho: ${salida}${conEfectos ? ` (con efectos a ${efectosDb} dB)` : " (sin efectos)"}`);
