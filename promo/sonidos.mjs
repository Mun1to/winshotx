/**
 * Baja de Mixkit los efectos de sonido del vídeo y los deja listos en `public/sonido/mixkit/`.
 *
 *   node sonidos.mjs
 *
 * Munito, 15-nov a las 22:30: «tienes que poner efectos de sonido y hacerlo mejor». Por la
 * mañana había dicho «los efectos de sonido no me gustan» de cinco clics genéricos (Kenney):
 * lo malo eran esos. Estos se eligieron midiéndolos (scratchpad de S41): los «whoosh», los más
 * oscuros y cortos de la tienda (de 1,1 a 2,6 kHz de brillo, de 250 a 600 ms); los clics, cortos
 * y sin cola; el obturador, el más grave de los cinco; y cuatro variantes de whoosh, dos de
 * tecla y dos de tic para que nada suene dos veces seguidas igual.
 *
 * Licencia: Mixkit Sound Effects Free License (releída el 15-nov-2026 en
 * https://mixkit.co/license/modal/sfxFree/): gratis en vídeos para redes, anuncios y proyectos
 * comerciales, sin atribución; lo que NO se puede es redistribuirlos sueltos. Por eso los
 * archivos no van al repositorio (`.gitignore`): aquí solo va de dónde sale cada uno.
 *
 * A cada uno se le quita el silencio de delante y la cola por debajo de −60 dB, se pasa a
 * 48 kHz estéreo y se iguala a −1 dBFS de pico. `efectos.json` dice cuánto dura y en qué
 * milisegundo cae su pico, para que el vídeo pueda hacer caer el pico de un whoosh justo en
 * el corte.
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { setPriority, constants } from "node:os";

const aqui = dirname(fileURLToPath(import.meta.url));
setPriority(constants.priority.PRIORITY_BELOW_NORMAL);
const salida = join(aqui, "public", "sonido", "mixkit");
const bajados = join(salida, "originales");

/** nombre: [id de Mixkit, título en Mixkit, trozo opcional en segundos]. */
const EFECTOS = {
  "whoosh-1": [1462, "Quick swoosh"],
  "whoosh-2": [1464, "Quick swoosh accent"],
  "whoosh-3": [166, "Fast small sweep transition"],
  "whoosh-4": [168, "Fast air sweep transition"],
  sube: [1466, "Elevating swoosh"],
  zoom: [1489, "Air woosh"],
  clic: [1113, "Mouse click close"],
  interruptor: [2582, "Quick switch click"],
  obturador: [1434, "Nikon camera shutter"],
  // Las dos primeras pulsaciones de un mismo teclado, y la tercera, más grave, para la S.
  "tecla-1": [2534, "Keyboard key presses", [0.2, 0.42]],
  "tecla-2": [2534, "Keyboard key presses", [0.415, 0.75]],
  "tecla-3": [2533, "Single key type"],
  "tic-1": [3124, "Modern technology select"],
  "tic-2": [2585, "On or off light switch tap"],
};

function ff(args, texto = false) {
  const r = spawnSync("ffmpeg", ["-hide_banner", "-nostats", ...args], { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`ffmpeg: ${r.stderr}`);
  return texto ? r.stderr : "";
}

mkdirSync(bajados, { recursive: true });
const medidas = {};
for (const [nombre, [id, titulo, trozo]] of Object.entries(EFECTOS)) {
  const original = join(bajados, `${id}.wav`);
  if (!existsSync(original)) {
    const r = await fetch(`https://assets.mixkit.co/active_storage/sfx/${id}/${id}.wav`);
    if (!r.ok) throw new Error(`Mixkit ${id}: ${r.status}`);
    writeFileSync(original, Buffer.from(await r.arrayBuffer()));
  }
  const crudo = join(salida, `${nombre}.crudo.wav`);
  const filtro = [
    ...(trozo ? [`atrim=start=${trozo[0]}:end=${trozo[1]}`, "asetpts=PTS-STARTPTS"] : []),
    "silenceremove=start_periods=1:start_threshold=-50dB",
    "areverse",
    "silenceremove=start_periods=1:start_threshold=-60dB",
    "areverse",
    "aresample=48000",
  ].join(",");
  ff(["-y", "-i", original, "-af", filtro, "-ac", "2", "-c:a", "pcm_s16le", crudo]);
  const pico = Number(ff(["-i", crudo, "-af", "volumedetect", "-f", "null", "-"], true).match(/max_volume: (-?[\d.]+) dB/)[1]);
  const listo = join(salida, `${nombre}.wav`);
  // Un fundido de 15 ms al final, para que el corte de la cola no dé un chasquido.
  const dura = Number(spawnSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", crudo], { encoding: "utf8" }).stdout);
  ff(["-y", "-i", crudo, "-af", `volume=${(-1 - pico).toFixed(2)}dB,afade=t=out:st=${Math.max(0, dura - 0.015)}:d=0.015`, "-c:a", "pcm_s16le", listo]);
  rmSync(crudo);
  // Dónde cae el pico, en milisegundos: la envolvente en ventanas de 5 ms.
  const pcm = spawnSync("ffmpeg", ["-v", "error", "-i", listo, "-ac", "1", "-f", "s16le", "-"], { maxBuffer: 1 << 26 }).stdout;
  const muestras = new Int16Array(pcm.buffer, pcm.byteOffset, pcm.length >> 1);
  let mejor = 0;
  let donde = 0;
  for (let i = 0; i < muestras.length; i += 240) {
    let m = 0;
    for (let j = i; j < Math.min(i + 240, muestras.length); j++) m = Math.max(m, Math.abs(muestras[j]));
    if (m > mejor) [mejor, donde] = [m, i];
  }
  medidas[nombre] = { id, titulo, ms: Math.round(dura * 1000), picoMs: Math.round((donde / 48000) * 1000) };
  console.log(`${nombre.padEnd(12)} ${String(id).padStart(5)} ${titulo.padEnd(30)} ${medidas[nombre].ms} ms, pico a ${medidas[nombre].picoMs} ms`);
}
writeFileSync(join(salida, "efectos.json"), JSON.stringify(medidas, null, 1));
rmSync(bajados, { recursive: true, force: true });
console.log(`listos en ${salida}`);
