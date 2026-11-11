/**
 * Prueba el editor en un navegador de verdad: recortar desde la línea de tiempo, reproducir
 * lo recortado y guardarlo, con un ratón de verdad y un vídeo de verdad.
 *
 *   pnpm build
 *   node scripts/probar-editor.mjs                    genera un vídeo de prueba con ffmpeg
 *   node scripts/probar-editor.mjs --video=x.mp4 --editor=x.png --duracion=30
 *   node scripts/probar-editor.mjs --fotos=<carpeta>  deja ahí las fotos de cada paso
 *
 * Usa Edge, que es el motor con el que Windows pinta las ventanas de la app (WebView2), y
 * sin ventana: no se abre nada en el escritorio de nadie ni se le mueve el ratón. Monta la
 * pantalla con `ver-ventana.mjs --servir`, que sirve el `dist/` de verdad con un Rust de
 * mentira, y lo que el editor le pide a Rust queda apuntado para comprobarlo.
 *
 * Lo que comprueba, y por qué:
 *  1. Que arrastrar las asas recorta donde se sueltan, y que la vista previa enseña ese
 *     sitio mientras se arrastra.
 *  2. Que el play empieza en el principio de lo recortado y se para en su final, sin
 *     pasarse. Antes se pasaba hasta un cuarto de segundo: el aviso de tiempo del vídeo
 *     llega cuatro veces por segundo.
 *  3. Que «Guardar» manda a Rust justo el tramo recortado, en milisegundos.
 *  4. Que con diez minutos de grabación la línea de tiempo sigue yendo fluida: cuánto
 *     tarda en salir y cuánto tarda cada movimiento del asa.
 *
 * Sale con 0 si todo cuadra y con 1 si algo no.
 */
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const bandera = (nombre, pordefecto) => {
  const encontrada = args.find((a) => a.startsWith(`--${nombre}=`));
  return encontrada ? encontrada.slice(nombre.length + 3) : pordefecto;
};

const fotos = bandera("fotos", null);
if (fotos) mkdirSync(fotos, { recursive: true });
let video = bandera("video", null);
let fotograma = bandera("editor", null);
const duracion = Number(bandera("duracion", "30"));

// Sin vídeo dado, se fabrica uno con ffmpeg: una carta de ajuste que se mueve, para que
// cada instante se distinga del de al lado.
if (!video || !fotograma) {
  const carpeta = join(tmpdir(), "winshotx-probar-editor");
  mkdirSync(carpeta, { recursive: true });
  video = join(carpeta, `prueba-${duracion}s.mp4`);
  fotograma = join(carpeta, "fotograma.png");
  if (!existsSync(video)) {
    const hecho = spawnSync("ffmpeg", [
      "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi",
      "-i", `testsrc2=size=1280x800:rate=30:duration=${duracion}`,
      "-pix_fmt", "yuv420p", "-c:v", "libx264", "-preset", "veryfast", video,
    ]);
    if (hecho.status !== 0) {
      console.error("no hay vídeo de prueba y ffmpeg no lo ha podido hacer: pasa --video y --editor");
      process.exit(1);
    }
  }
  if (!existsSync(fotograma)) {
    spawnSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", "-ss", "3", "-i", video, "-frames:v", "1", fotograma]);
  }
}

/** Levanta la pantalla del editor y devuelve su dirección y cómo apagarla. */
function servir(segundos, mas = []) {
  return new Promise((listo, falla) => {
    const hijo = spawn(process.execPath, [
      join(RAIZ, "scripts", "ver-ventana.mjs"),
      "--servir",
      `--editor=${fotograma}`,
      `--video=${video}`,
      `--duracion=${segundos}`,
      ...mas,
    ]);
    hijo.stdout.on("data", (trozo) => {
      const url = String(trozo).match(/http:\/\/\S+/)?.[0];
      if (url) listo({ url, apagar: () => hijo.kill() });
    });
    hijo.on("exit", (codigo) => falla(new Error(`el banco se ha parado con ${codigo}`)));
  });
}

const fallos = [];
const comprueba = (bien, que) => {
  console.log(`${bien ? "  BIEN" : "  MAL "}  ${que}`);
  if (!bien) fallos.push(que);
};

const navegador = await chromium.launch({ channel: "msedge", headless: true });

// ── Recortar, reproducir y guardar, con una grabación de `duracion` segundos ──────────
{
  const { url, apagar } = await servir(duracion);
  const pagina = await navegador.newPage({ viewport: { width: 1440, height: 842 } });
  await pagina.goto(url);
  const pista = pagina.locator("[data-pista]");
  await pista.waitFor();
  await pagina.waitForFunction(() => (document.querySelector("video")?.readyState ?? 0) >= 2);
  const caja = await pista.boundingBox();
  const total = duracion * 1000;
  const xDe = (parte) => caja.x + caja.width * parte;
  const yMedio = caja.y + caja.height / 2;

  /** Arrastra un asa hasta esa parte de la tira, en pasos, como una mano. */
  async function arrastrar(nombre, parte) {
    const asa = await pagina.getByRole("slider", { name: nombre }).boundingBox();
    await pagina.mouse.move(asa.x + asa.width / 2, asa.y + asa.height / 2);
    await pagina.mouse.down();
    await pagina.mouse.move(xDe(parte), yMedio, { steps: 12 });
    await pagina.mouse.up();
  }

  console.log(`\nRecortar una grabación de ${duracion} s en Edge, a 1440 x 842`);
  if (fotos) await pagina.screenshot({ path: join(fotos, "1-abierto.png") });

  await arrastrar(/Principio/, 0.25);
  const tiempoEnElCorte = await pagina.evaluate(() => document.querySelector("video").currentTime);
  comprueba(
    Math.abs(tiempoEnElCorte * 1000 - total * 0.25) < 150,
    `al soltar el principio en el 25 %, la vista previa enseña ${tiempoEnElCorte.toFixed(2)} s (debía ${(total * 0.25) / 1000} s)`,
  );
  await arrastrar(/Final/, 0.7);
  const desde = Number(await pagina.getByRole("slider", { name: /Principio/ }).getAttribute("aria-valuenow"));
  const hasta = Number(await pagina.getByRole("slider", { name: /Final/ }).getAttribute("aria-valuenow"));
  comprueba(Math.abs(desde - total * 0.25) < 150, `el principio queda en ${desde} ms`);
  comprueba(Math.abs(hasta - total * 0.7) < 150, `el final queda en ${hasta} ms`);
  const texto = await pagina.getByText(/Te quedas con/).textContent();
  comprueba(/Te quedas con 0:13\.\d de 0:30\.0/.test(texto) || duracion !== 30, `debajo dice «${texto}»`);
  if (fotos) await pagina.screenshot({ path: join(fotos, "2-recortado.png") });

  // Reproducir: se apunta el tiempo del vídeo en cada pintada, desde el clic hasta que
  // se para solo. Así se ve dónde empezó y hasta dónde llegó de verdad.
  // Se apunta hasta que se pare solo después de haber sonado, no un número fijo de
  // pintadas: Edge sin ventana pinta a unas 240 por segundo, y un tope de 2000 se llenaba
  // a los ocho segundos, parecía que el vídeo se paraba antes de tiempo y era el medidor.
  await pagina.evaluate(() => {
    const v = document.querySelector("video");
    v.currentTime = 0;
    window.__tiempos = [];
    const empieza = performance.now();
    const apuntar = () => {
      window.__tiempos.push({ t: v.currentTime * 1000, parado: v.paused });
      const sono = window.__tiempos.some((m) => !m.parado);
      if (!(sono && v.paused) && performance.now() - empieza < 120_000) requestAnimationFrame(apuntar);
    };
    requestAnimationFrame(apuntar);
  });
  await pagina.getByRole("button", { name: "Reproducir" }).click();
  const margen = 3000 + (hasta - desde);
  await pagina.waitForFunction(
    () => {
      const v = document.querySelector("video");
      return v.paused && window.__tiempos.some((m) => !m.parado);
    },
    undefined,
    { timeout: margen + 5000 },
  );
  const tiempos = await pagina.evaluate(() => window.__tiempos);
  const sonando = tiempos.filter((m) => !m.parado).map((m) => m.t);
  const empezo = Math.min(...sonando);
  const llego = Math.max(...tiempos.map((m) => m.t));
  comprueba(Math.abs(empezo - desde) < 120, `el play empieza en ${Math.round(empezo)} ms (el corte está en ${desde})`);
  comprueba(llego <= hasta + 40, `y se para en ${Math.round(llego)} ms, sin pasarse del final (${hasta})`);
  comprueba(llego >= hasta - 150, `y llega hasta el final, no se para antes`);

  await pagina.getByRole("button", { name: "Guardar MP4" }).click();
  await pagina.waitForFunction(() => (window.__llamadas ?? []).some((l) => l.cmd === "export_media"));
  const peticion = await pagina.evaluate(
    () => window.__llamadas.findLast((l) => l.cmd === "export_media").args.request,
  );
  comprueba(
    peticion.desdeMs === desde && peticion.hastaMs === hasta,
    `«Guardar MP4» manda a Rust el tramo ${peticion.desdeMs}-${peticion.hastaMs} ms`,
  );
  await pagina.getByText(/Guardado/).waitFor();
  if (fotos) await pagina.screenshot({ path: join(fotos, "3-guardado.png") });

  await pagina.getByRole("button", { name: "Más ajustes" }).click();
  await pagina.waitForTimeout(400);
  if (fotos) await pagina.screenshot({ path: join(fotos, "4-mas-ajustes.png") });
  await pagina.getByRole("button", { name: "Más ajustes" }).click();

  // El estudio: elegir un fondo lo pinta en la vista previa, con aire y esquinas, y es lo
  // que viaja a Rust al guardar.
  await pagina.getByRole("button", { name: "Noche" }).click();
  await pagina.waitForTimeout(300);
  const marco = await pagina.evaluate(() => {
    const el = document.querySelector('[data-marco="noche"]');
    const imagen = el?.firstElementChild;
    return el && imagen
      ? {
          fondo: getComputedStyle(el).backgroundImage,
          aire: parseFloat(getComputedStyle(el).paddingLeft),
          esquinas: parseFloat(getComputedStyle(imagen).borderTopLeftRadius),
          sombra: getComputedStyle(imagen).boxShadow,
        }
      : null;
  });
  comprueba(marco !== null && marco.fondo.includes("gradient"), `la vista previa pinta el fondo elegido (${marco?.fondo.slice(0, 40)}…)`);
  comprueba((marco?.aire ?? 0) > 8, `con aire alrededor: ${marco?.aire} px`);
  comprueba((marco?.esquinas ?? 0) > 2, `y las esquinas redondeadas: ${marco?.esquinas} px`);
  comprueba(Boolean(marco?.sombra) && marco.sombra !== "none", "y la sombra");
  await pagina.getByRole("switch", { name: /Esconderlo cuando no se mueve/ }).click();
  if (fotos) await pagina.screenshot({ path: join(fotos, "6-estudio.png") });
  await pagina.getByRole("button", { name: "Guardar MP4" }).click();
  await pagina.waitForFunction(() => window.__llamadas.filter((l) => l.cmd === "export_media").length >= 2);
  const conEstudio = await pagina.evaluate(
    () => window.__llamadas.findLast((l) => l.cmd === "export_media").args.request,
  );
  comprueba(
    conEstudio.background === "noche" && conEstudio.margin > 0 && conEstudio.corners > 0,
    `«Guardar» manda el fondo noche con ${conEstudio.margin} px de aire y ${conEstudio.corners} de esquinas`,
  );
  comprueba(
    conEstudio.smoothCursor === true && conEstudio.hideIdleCursor === true,
    "y el puntero suave y que se esconde",
  );

  await pagina.close();
  apagar();
}

// ── La pista de zooms y las tijeras: lo que se pone, se mueve y se corta llega a Rust ─
{
  const { url, apagar } = await servir(duracion);
  const pagina = await navegador.newPage({ viewport: { width: 1440, height: 842 } });
  await pagina.goto(url);
  await pagina.locator("[data-pista]").waitFor();
  await pagina.waitForFunction(() => (document.querySelector("video")?.readyState ?? 0) >= 2);
  const total = duracion * 1000;
  const tira = await pagina.locator("[data-pista]").boundingBox();
  const pistaDeZoom = await pagina.locator("[data-pista-zoom]").boundingBox();
  const xDe = (parte) => tira.x + tira.width * parte;
  /** Pulsa «Guardar MP4» y devuelve lo que le llega a Rust. */
  const guardar = async () => {
    const antes = await pagina.evaluate(() => (window.__llamadas ?? []).filter((l) => l.cmd === "export_media").length);
    await pagina.getByRole("button", { name: "Guardar MP4" }).click();
    await pagina.waitForFunction(
      (n) => window.__llamadas.filter((l) => l.cmd === "export_media").length > n,
      antes,
    );
    return pagina.evaluate(() => window.__llamadas.findLast((l) => l.cmd === "export_media").args.request);
  };
  console.log("\nLa pista de zooms y las tijeras, en Edge");

  const sinTocar = await guardar();
  comprueba(
    sinTocar.zooms === null && sinTocar.tramos === null,
    `sin tocar la pista, «Guardar» manda zooms ${JSON.stringify(sinTocar.zooms)} y tramos ${JSON.stringify(sinTocar.tramos)}: lo de siempre`,
  );

  // Un clic en la pista pone un zoom de dos segundos centrado en el clic.
  await pagina.mouse.click(xDe(0.5), pistaDeZoom.y + pistaDeZoom.height / 2);
  const bloque = pagina.locator("[data-zoom]").first();
  await bloque.waitFor();
  if (fotos) await pagina.screenshot({ path: join(fotos, "7-zoom-puesto.png") });
  let peticion = await guardar();
  const puesto = peticion.zooms?.[0];
  comprueba(
    peticion.zooms?.length === 1 && Math.abs((puesto.desdeMs + puesto.hastaMs) / 2 - total * 0.5) < 300,
    `un clic en la pista pone un zoom de ${puesto?.desdeMs} a ${puesto?.hastaMs} ms, centrado en el clic`,
  );

  // Arrastrarlo por el medio lo mueve entero.
  const caja = await bloque.boundingBox();
  await pagina.mouse.move(caja.x + caja.width / 2, caja.y + caja.height / 2);
  await pagina.mouse.down();
  await pagina.mouse.move(caja.x + caja.width / 2 + tira.width * 0.2, caja.y + caja.height / 2, { steps: 10 });
  await pagina.mouse.up();
  peticion = await guardar();
  const movido = peticion.zooms[0];
  comprueba(
    Math.abs(movido.desdeMs - (puesto.desdeMs + total * 0.2)) < 300 &&
      movido.hastaMs - movido.desdeMs === puesto.hastaMs - puesto.desdeMs,
    `arrastrarlo lo lleva a ${movido.desdeMs}-${movido.hastaMs} ms, sin cambiar lo que dura`,
  );

  // Fijo: la imagen entera con el recuadro, y el punto se arrastra.
  await pagina.getByRole("button", { name: "Fijo" }).click();
  const punto = pagina.locator("[data-punto-fijo] [role=slider]");
  await punto.waitFor();
  const imagen = await pagina.locator("[data-punto-fijo]").boundingBox();
  const dondeEsta = await punto.boundingBox();
  await pagina.mouse.move(dondeEsta.x + dondeEsta.width / 2, dondeEsta.y + dondeEsta.height / 2);
  await pagina.mouse.down();
  await pagina.mouse.move(imagen.x + imagen.width * 0.2, imagen.y + imagen.height * 0.3, { steps: 8 });
  await pagina.mouse.up();
  if (fotos) await pagina.screenshot({ path: join(fotos, "8-zoom-fijo.png") });
  peticion = await guardar();
  const fijo = peticion.zooms[0];
  comprueba(
    fijo.modo === "fijo" && Math.abs(fijo.x - 0.2) < 0.02 && Math.abs(fijo.y - 0.3) < 0.02,
    `en «Fijo» el punto se arrastra sobre la imagen y queda en ${fijo.x}, ${fijo.y}`,
  );

  // La lupa: el mismo bloque, que ahora amplía sin mover la vista.
  await pagina.getByRole("button", { name: "Lupa" }).click();
  await pagina.getByRole("button", { name: "Rectángulo redondeado" }).click();
  if (fotos) await pagina.screenshot({ path: join(fotos, "9-lupa.png") });
  peticion = await guardar();
  comprueba(
    peticion.zooms[0].lupa?.forma === "redondeado" && peticion.zooms[0].lupa?.marco === true,
    `y como lupa, con su forma y su marco: ${JSON.stringify(peticion.zooms[0].lupa)}`,
  );
  await pagina.getByRole("button", { name: "Listo" }).click();

  // Las tijeras: dos cortes y el trozo del medio fuera.
  const ir = (parte) => pagina.mouse.click(xDe(parte), tira.y + tira.height / 2);
  await ir(0.4);
  await pagina.getByRole("button", { name: /Cortar aquí/ }).click();
  await ir(0.6);
  await pagina.getByRole("button", { name: /Cortar aquí/ }).click();
  await ir(0.5);
  await pagina.getByRole("button", { name: "Quitar este trozo" }).click();
  await ir(0.8);
  await pagina.getByRole("button", { name: "2×" }).click();
  if (fotos) await pagina.screenshot({ path: join(fotos, "10-tijeras.png") });
  peticion = await guardar();
  // Dos cortes son tres trozos: el del medio fuera y el último, del 60 % al final, al doble.
  const [antesDelCorte, acelerado] = peticion.tramos ?? [];
  comprueba(
    peticion.tramos?.length === 2 &&
      Math.abs(antesDelCorte.hastaMs - total * 0.4) < 150 &&
      antesDelCorte.velocidad === 1 &&
      Math.abs(acelerado.desdeMs - total * 0.6) < 150 &&
      acelerado.velocidad === 2,
    `quitar el trozo del medio y acelerar el último manda ${JSON.stringify(peticion.tramos)}`,
  );

  // Reproducir se salta lo quitado, como el archivo.
  await ir(0.35);
  await pagina.evaluate(() => {
    const v = document.querySelector("video");
    window.__sonando = [];
    const apuntar = () => {
      if (!v.paused) window.__sonando.push(v.currentTime * 1000);
      if (window.__sonando.length < 100_000) requestAnimationFrame(apuntar);
    };
    requestAnimationFrame(apuntar);
  });
  await pagina.getByRole("button", { name: "Reproducir" }).click();
  await pagina.waitForFunction((hasta) => window.__sonando.some((t) => t > hasta), total * 0.63, { timeout: 15_000 });
  await pagina.evaluate(() => document.querySelector("video").pause());
  const sonando = await pagina.evaluate(() => window.__sonando);
  const dentro = sonando.filter((t) => t > total * 0.4 + 120 && t < total * 0.6 - 120);
  comprueba(
    dentro.length === 0 && sonando.some((t) => t < total * 0.4),
    `al reproducir se salta lo quitado: ${dentro.length} de ${sonando.length} pintadas cayeron dentro`,
  );

  await pagina.close();
  apagar();
}

// ── S4: las marcas nuevas, dibujadas con el ratón y mandadas a Rust al guardar ─────────
{
  const { url, apagar } = await servir(duracion);
  const pagina = await navegador.newPage({ viewport: { width: 1440, height: 842 } });
  await pagina.goto(url);
  await pagina.locator("[data-pista]").waitFor();
  console.log("\nLas marcas nuevas: flecha curva, línea, elipse, lápiz y foco");

  /** Elige una herramienta y hace un trazo sobre la vista previa, de parte a parte (0 a 1). */
  async function dibujar(nombre, camino) {
    await pagina.getByRole("button", { name: nombre, exact: true }).click();
    const capa = await pagina.locator("svg.cursor-crosshair").boundingBox();
    const en = ([x, y]) => [capa.x + capa.width * x, capa.y + capa.height * y];
    await pagina.mouse.move(...en(camino[0]));
    await pagina.mouse.down();
    for (const p of camino.slice(1)) await pagina.mouse.move(...en(p), { steps: 4 });
    await pagina.mouse.up();
  }
  const arco = Array.from({ length: 13 }, (_, i) => [0.1 + (i / 12) * 0.35, 0.85 - Math.sin((i / 12) * Math.PI) * 0.4]);
  const onda = Array.from({ length: 25 }, (_, i) => [0.55 + (i / 24) * 0.35, 0.15 + Math.sin(i / 2) * 0.04]);
  await dibujar("Foco", [[0.52, 0.35], [0.95, 0.68]]);
  await dibujar("Elipse", [[0.56, 0.4], [0.9, 0.62]]);
  await dibujar("Línea", [[0.52, 0.8], [0.95, 0.8]]);
  await dibujar("Flecha curva", arco);
  await dibujar("Lápiz", onda);
  if (fotos) await pagina.screenshot({ path: join(fotos, "11-marcas-nuevas.png") });

  const pintadas = await pagina.evaluate(() =>
    ["focus", "ellipse", "line", "curve", "pen"].filter((k) => document.querySelector(`[data-marca="${k}"]`)),
  );
  comprueba(pintadas.length === 5, `la vista previa enseña las cinco: ${pintadas.join(", ")}`);

  await pagina.getByRole("button", { name: "Guardar MP4" }).click();
  await pagina.waitForFunction(() => (window.__llamadas ?? []).some((l) => l.cmd === "export_media"));
  const marcas = await pagina.evaluate(
    () => window.__llamadas.findLast((l) => l.cmd === "export_media").args.request.annotations,
  );
  const de = (k) => marcas.find((m) => m.kind === k);
  comprueba(
    ["focus", "ellipse", "line", "curve", "pen"].every((k) => de(k)),
    `«Guardar» manda las cinco a Rust: ${marcas.map((m) => m.kind).join(", ")}`,
  );
  comprueba(de("curve")?.puntos?.length === 1, "la curva viaja con su punto de control");
  const medio = de("curve") && (() => {
    const { x1, y1, x2, y2, puntos: [[cx, cy]] } = de("curve");
    return [0.25 * x1 + 0.5 * cx + 0.25 * x2, 0.25 * y1 + 0.5 * cy + 0.25 * y2];
  })();
  comprueba(
    medio && Math.abs(medio[0] - 0.275) < 0.03 && Math.abs(medio[1] - 0.45) < 0.03,
    `y pasa a la mitad por donde pasó el ratón (${medio?.map((v) => v.toFixed(3))}, debía 0.275, 0.450)`,
  );
  comprueba(
    (de("pen")?.puntos?.length ?? 0) >= 10 && de("pen").puntos.length < 100,
    `el lápiz viaja con su trazo aclarado: ${de("pen")?.puntos?.length} puntos`,
  );
  await pagina.close();
  apagar();
}

// ── S7: el cursor y los clics. Los mandos van en el panel y lo que piden llega a Rust ────
{
  const { url, apagar } = await servir(duracion);
  const pagina = await navegador.newPage({ viewport: { width: 1440, height: 842 } });
  await pagina.goto(url);
  await pagina.locator("[data-pista]").waitFor();
  await pagina.waitForFunction(() => (document.querySelector("video")?.readyState ?? 0) >= 2);
  console.log("\nEl cursor y los clics (S7), en Edge");
  const mandos = pagina.locator("[data-ajustes-cursor]");
  if ((await mandos.count()) === 0) {
    console.log("  (los mandos del cursor aún no están en el panel: se engancha AjustesCursor y esto se prueba solo)");
  } else {
    await mandos.scrollIntoViewIfNeeded();
    // La onda en la vista previa: parado a los 3,3 s, 300 ms después del clic del centro.
    // Se cuenta el azul del lienzo del estudio sin la onda y con ella.
    const tira = await pagina.locator("[data-pista]").boundingBox();
    await pagina.mouse.click(tira.x + (tira.width * 3300) / (duracion * 1000), tira.y + tira.height / 2);
    await pagina.waitForTimeout(400);
    const azulEnElLienzo = () =>
      pagina.evaluate(() => {
        const c = document.querySelector("[data-capa-estudio]");
        const { data } = c.getContext("2d").getImageData(0, 0, c.width, c.height);
        let n = 0;
        for (let i = 0; i < data.length; i += 4) if (data[i + 3] > 20 && data[i + 2] > 150 && data[i] < 120) n++;
        return n;
      });
    const sinOnda = await azulEnElLienzo();
    await pagina.getByRole("button", { name: "Mucho" }).click();
    await pagina.getByRole("switch", { name: /Una onda en cada clic/ }).click();
    await pagina.waitForTimeout(300);
    const conOnda = await azulEnElLienzo();
    comprueba(conOnda > sinOnda + 50, `la vista previa pinta la onda del clic: ${sinOnda} píxeles azules sin ella, ${conOnda} con ella`);
    await pagina.getByRole("switch", { name: /Sonido de clic/ }).click();
    await pagina.getByRole("switch", { name: /Esconderlo mientras escribes/ }).click();
    await pagina.getByRole("switch", { name: /Acercarse mientras escribes/ }).click();
    await pagina.getByRole("slider", { name: "Estela al moverse" }).fill("60");
    if (fotos) await pagina.screenshot({ path: join(fotos, "s7-mandos.png") });

    await pagina.getByRole("button", { name: "Guardar MP4" }).click();
    await pagina.waitForFunction(() => (window.__llamadas ?? []).some((l) => l.cmd === "export_media"));
    const peticion = await pagina.evaluate(
      () => window.__llamadas.findLast((l) => l.cmd === "export_media").args.request,
    );
    comprueba(peticion.suavizado === "mucho", `«Guardar» manda el suavizado ${peticion.suavizado}`);
    comprueba(peticion.ondaClic === true && peticion.esconderAlEscribir === true, "y la onda y esconderlo al escribir");
    comprueba(peticion.sonidoClic > 0 && peticion.sonidoClic <= 0.3, `y el sonido de clic, que nace bajo: ${peticion.sonidoClic}`);
    comprueba(Math.abs(peticion.estela - 0.6) < 0.01, `y la estela: ${peticion.estela}`);
    comprueba(peticion.zoomAlEscribir === true && peticion.zooms === null, "y acercarse al escribir, con la pista sin tocar");
    const pedidos = await pagina.evaluate(() =>
      window.__llamadas.filter((l) => l.cmd === "session_zooms" && l.args.alEscribir === true).length,
    );
    comprueba(pedidos > 0, `la pista pide a Rust los zooms de escribir (${pedidos} veces)`);
  }
  await pagina.close();
  apagar();
}

// ── S8: la webcam en la vista previa, arrastrada y estirada, llega a Rust ─────────────
{
  const { url, apagar } = await servir(duracion, ["--camara-estudio"]);
  const pagina = await navegador.newPage({ viewport: { width: 1440, height: 842 } });
  await pagina.goto(url);
  await pagina.locator("[data-pista]").waitFor();
  await pagina.waitForFunction(() => (document.querySelector("video")?.readyState ?? 0) >= 2);
  const burbuja = pagina.locator("[data-camara]");
  await burbuja.waitFor();
  console.log("\nLa webcam en la vista previa");
  /** Pulsa «Guardar MP4» y devuelve lo que le llega a Rust. */
  const guardar = async () => {
    const antes = await pagina.evaluate(() => (window.__llamadas ?? []).filter((l) => l.cmd === "export_media").length);
    await pagina.getByRole("button", { name: "Guardar MP4" }).click();
    await pagina.waitForFunction(
      (n) => window.__llamadas.filter((l) => l.cmd === "export_media").length > n,
      antes,
    );
    return pagina.evaluate(() => window.__llamadas.findLast((l) => l.cmd === "export_media").args.request);
  };
  if (fotos) await pagina.screenshot({ path: join(fotos, "12-camara-de-fabrica.png") });

  let peticion = await guardar();
  const fabrica = peticion.camara;
  comprueba(
    fabrica?.forma === "circulo" && Math.abs(fabrica.tam - 0.18) < 1e-6 && fabrica.x > 0.8 && fabrica.y > 0.7,
    `sin tocar nada, «Guardar» manda la cámara de fábrica, pequeña y abajo a la derecha: ${JSON.stringify(fabrica)}`,
  );

  // Arrastrada por el medio hasta arriba a la izquierda.
  const lienzo = await pagina.locator("[data-capa-camara]").boundingBox();
  let caja = await burbuja.boundingBox();
  await pagina.mouse.move(caja.x + caja.width / 2, caja.y + caja.height / 2);
  await pagina.mouse.down();
  await pagina.mouse.move(lienzo.x + lienzo.width * 0.25, lienzo.y + lienzo.height * 0.3, { steps: 10 });
  await pagina.mouse.up();
  peticion = await guardar();
  comprueba(
    Math.abs(peticion.camara.x - 0.25) < 0.01 && Math.abs(peticion.camara.y - 0.3) < 0.01,
    `arrastrarla la deja en ${peticion.camara.x.toFixed(3)}, ${peticion.camara.y.toFixed(3)} (soltada en 0.25, 0.3)`,
  );

  // Estirada por la esquina que mira al centro, que ahora es la de abajo a la derecha.
  caja = await burbuja.boundingBox();
  await pagina.mouse.move(caja.x + caja.width / 2, caja.y + caja.height / 2);
  const asa = await pagina.locator("[data-asa-camara]").boundingBox();
  await pagina.mouse.move(asa.x + asa.width / 2, asa.y + asa.height / 2);
  await pagina.mouse.down();
  await pagina.mouse.move(asa.x + asa.width / 2 + 80, asa.y + asa.height / 2 + 80, { steps: 8 });
  await pagina.mouse.up();
  peticion = await guardar();
  const despues = await burbuja.boundingBox();
  comprueba(
    peticion.camara.tam > fabrica.tam + 0.03 && Math.abs(despues.x - caja.x) < 2 && Math.abs(despues.y - caja.y) < 2,
    `la esquina la estira de ${fabrica.tam} a ${peticion.camara.tam.toFixed(3)} con la de enfrente quieta (se movió ${(despues.x - caja.x).toFixed(1)}, ${(despues.y - caja.y).toFixed(1)} px)`,
  );
  if (fotos) await pagina.screenshot({ path: join(fotos, "13-camara-movida.png") });

  // Reproduciendo, la cara va con la pantalla (aquí las dos son el mismo vídeo, sin desfase).
  await pagina.getByRole("button", { name: "Reproducir" }).click();
  await pagina.waitForTimeout(1500);
  const [pantalla, cara] = await pagina.evaluate(() => [
    document.querySelector("video").currentTime,
    document.querySelector("[data-camara] video").currentTime,
  ]);
  await pagina.getByRole("button", { name: "Pausar" }).click();
  comprueba(
    pantalla > 0.5 && Math.abs(pantalla - cara) < 0.2,
    `reproduciendo, la cara va con la pantalla: ${pantalla.toFixed(2)} s y ${cara.toFixed(2)} s`,
  );

  // Con una herramienta de dibujar puesta, la cámara no se agarra: lo que se arrastra es la marca.
  await pagina.getByRole("button", { name: /^Flecha · / }).click();
  const quieta = await burbuja.evaluate((b) => getComputedStyle(b).pointerEvents);
  comprueba(quieta === "none", `con la flecha puesta, la cámara deja pasar el ratón (pointer-events: ${quieta})`);
  await pagina.close();
  apagar();
}

// ── Diez minutos de grabación: lo que tarda en salir y lo que cuesta arrastrar ─────────
{
  const segundos = 600;
  const { url, apagar } = await servir(segundos);
  const pagina = await navegador.newPage({ viewport: { width: 1440, height: 842 } });
  console.log(`\nLa línea de tiempo con ${segundos / 60} minutos dentro (${segundos * 30} fotogramas)`);
  const empieza = Date.now();
  await pagina.goto(url);
  const pista = pagina.locator("[data-pista]");
  await pista.waitFor();
  const tardo = Date.now() - empieza;
  const imagenes = await pagina.evaluate(
    () => document.querySelectorAll("[data-pista] img, [data-pista] [data-miniatura-pendiente]").length,
  );
  comprueba(tardo < 3000, `sale en ${tardo} ms`);
  comprueba(imagenes < 40, `con ${imagenes} miniaturas en la página, no una por fotograma`);

  // Se arrastra el asa del final de punta a punta y se mide cuánto tarda la página en
  // pintar cada paso: es lo que se nota como «va fluido» o «va a saltos».
  const caja = await pista.boundingBox();
  const asa = await pagina.getByRole("slider", { name: /Final/ }).boundingBox();
  await pagina.evaluate(() => {
    window.__pintadas = [];
    let antes = performance.now();
    const medir = (ahora) => {
      window.__pintadas.push(ahora - antes);
      antes = ahora;
      if (window.__pintadas.length < 600) requestAnimationFrame(medir);
    };
    requestAnimationFrame(medir);
  });
  await pagina.mouse.move(asa.x + asa.width / 2, asa.y + asa.height / 2);
  await pagina.mouse.down();
  const pasos = 60;
  const t0 = Date.now();
  for (let i = 1; i <= pasos; i++) {
    await pagina.mouse.move(caja.x + caja.width * (1 - (0.8 * i) / pasos), caja.y + caja.height / 2);
  }
  await pagina.mouse.up();
  const porPaso = (Date.now() - t0) / pasos;
  const pintadas = await pagina.evaluate(() => window.__pintadas.slice(2));
  const peor = Math.max(...pintadas);
  comprueba(porPaso < 34, `arrastrar el asa: ${porPaso.toFixed(1)} ms por movimiento`);
  comprueba(peor < 100, `la pintada más lenta mientras se arrastra tarda ${peor.toFixed(0)} ms`);
  if (fotos) await pagina.screenshot({ path: join(fotos, "5-diez-minutos.png") });
  await pagina.close();
  apagar();
}

await navegador.close();
console.log(fallos.length ? `\n${fallos.length} cosas no cuadran` : "\nTodo cuadra");
process.exit(fallos.length ? 1 : 0);
