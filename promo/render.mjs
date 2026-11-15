/**
 * Saca los vídeos y las fotos de `promo/`, con la prioridad baja (el PC de Munito va cargado).
 *
 *   node render.mjs muestras            el arranque de cada dirección y sus fotos, para elegir
 *   node render.mjs Promo               el vídeo elegido, MUDO, en out/Promo.mp4, y sus efectos de sonido
 *                                       aparte en out/Promo-efectos.wav (la música y la mezcla, musica.mjs)
 *   node render.mjs Promo-127           el mismo con los cortes a 127,5 golpes por minuto (Karma)
 *   node render.mjs Promo 0-599         solo esos fotogramas (el arranque, para probar músicas)
 *   node render.mjs Direccion2-Tirada   una dirección entera, en out/<id>.mp4
 *
 * Empaqueta el proyecto UNA vez (`remotion bundle`) y renderiza desde ese paquete: cada
 * `remotion render` sobre `src/index.ts` lo volvería a empaquetar.
 *
 * El proceso se baja a «por debajo de lo normal» antes de lanzar nada, y en Windows lo que
 * lanza un proceso así hereda esa prioridad: Remotion y su Chrome incluidos.
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, existsSync } from "node:fs";
import { setPriority, constants } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
setPriority(constants.priority.PRIORITY_BELOW_NORMAL);

const DIRECCIONES = {
  "Direccion1-Carrera": { trozo: [0, 287], fotos: [70, 230, 420, 570, 760, 1060] },
  "Direccion2-Tirada": { trozo: [0, 314], fotos: [30, 120, 200, 262, 400, 640] },
  "Direccion3-LoHiceYo": { trozo: [0, 407], fotos: [40, 235, 360, 495, 640, 780] },
};

const [que = "muestras", trozo] = process.argv.slice(2);
const PROMOS = ["Promo", "Promo-127"];
if (que !== "muestras" && !PROMOS.includes(que) && !(que in DIRECCIONES)) {
  console.error(`Uso: node render.mjs muestras | ${[...PROMOS, ...Object.keys(DIRECCIONES)].join(" | ")}`);
  process.exit(1);
}
if (!existsSync(join(aqui, "public", "app", "medidas.json"))) {
  console.error("faltan los fotogramas de la app: corre antes `node capturas.mjs`");
  process.exit(1);
}

/* Una línea por el shell: en Windows `pnpm` es un `.cmd`. Lo de fuera (el nombre) ya se ha
   cotejado contra su lista, así que no hay nada que escapar. */
function lanza(linea) {
  console.log(`> ${linea}`);
  const r = spawnSync(linea, { cwd: aqui, stdio: "inherit", shell: true });
  if (r.status !== 0) process.exit(r.status ?? 1);
}

const paquete = join("out", "paquete");
mkdirSync(join(aqui, "out", "muestras"), { recursive: true });
lanza(`pnpm exec remotion bundle src/index.ts --out-dir=${paquete}`);

if (que === "muestras") {
  for (const [id, { trozo, fotos }] of Object.entries(DIRECCIONES)) {
    lanza(`pnpm exec remotion render ${paquete} ${id} out/muestras/${id}.mp4 --frames=${trozo[0]}-${trozo[1]} --crf=20`);
    for (const f of fotos) {
      lanza(`pnpm exec remotion still ${paquete} ${id} out/muestras/${id}-${String(f).padStart(4, "0")}.jpg --frame=${f} --jpeg-quality=88`);
    }
  }
} else if (PROMOS.includes(que)) {
  // La imagen, muda; los efectos de sonido, aparte y sin comprimir; y la música se le pone
  // después con `musica.mjs`, que la iguala, mete los efectos debajo y hace el fundido. Así
  // cambiar de pista no pide volver a renderizar.
  if (trozo && !/^\d+-\d+$/.test(trozo)) process.exit(1);
  const base = trozo ? `out/${que}-${trozo}` : `out/${que}`;
  const frames = trozo ? ` --frames=${trozo}` : "";
  lanza(`pnpm exec remotion render ${paquete} ${que} ${base}.mp4 --crf=18 --muted${frames}`);
  lanza(`pnpm exec remotion render ${paquete} ${que} ${base}-efectos.wav --codec=wav${frames}`);
} else {
  lanza(`pnpm exec remotion render ${paquete} ${que} out/${que}.mp4 --crf=18`);
}
